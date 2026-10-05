//! Managed originals and byte-bounded, persistent LRU thumbnails.

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::Mutex;

pub const DEFAULT_CACHE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

pub struct TimelineImages {
    db: Arc<SqliteStore>,
    root: PathBuf,
    budget: AtomicU64,
    /// Serialize attachment publication, memo changes and GC; never hold a SQLite transaction across await.
    pub(crate) mutation: Mutex<()>,
    cache_lock: Mutex<()>,
}

#[derive(Serialize)]
pub struct ImageStorageStats {
    pub directory: String,
    pub originals_bytes: u64,
    pub cache_bytes: u64,
    pub cache_limit_bytes: u64,
    pub missing_images: Vec<String>,
    pub pending_cleanup: u64,
}

#[derive(Default, Serialize)]
pub struct ImageImportReport {
    pub copied: usize,
    pub missing: Vec<String>,
}

impl TimelineImages {
    pub fn new(db: Arc<SqliteStore>, root: PathBuf, budget: u64) -> Result<Self, BrainError> {
        std::fs::create_dir_all(root.join("images"))?;
        std::fs::create_dir_all(root.join("cache"))?;
        for directory in ["images", "cache"] {
            if std::fs::symlink_metadata(root.join(directory))?
                .file_type()
                .is_symlink()
            {
                return Err(BrainError::ConfigError(
                    "图片存储子目录不能为符号链接".into(),
                ));
            }
        }
        Ok(Self {
            db,
            root: root.canonicalize()?,
            budget: AtomicU64::new(budget),
            mutation: Mutex::new(()),
            cache_lock: Mutex::new(()),
        })
    }

    pub async fn upload(&self, bytes: &[u8]) -> Result<String, BrainError> {
        let _guard = self.mutation.lock().await;
        let mime = validate_image(bytes.to_vec()).await?;
        let filename = format!("{}.{}", uuid::Uuid::new_v4(), extension(&mime));
        let path = format!("Timeline/images/{filename}");
        self.publish(&path, &filename, bytes, &mime, true).await?;
        Ok(path)
    }

    async fn publish(
        &self,
        path: &str,
        filename: &str,
        bytes: &[u8],
        mime: &str,
        pending: bool,
    ) -> Result<(), BrainError> {
        let dest = self.root.join("images").join(filename);
        let mut temporary = tempfile::NamedTempFile::new_in(self.root.join("images"))?;
        use std::io::Write;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        // Never overwrite an existing original, including one left by a crashed publication.
        temporary
            .persist_noclobber(&dest)
            .map_err(|e| BrainError::IoError(e.error))?;
        let result = self.db.with_connection(|conn| {
            let mut sql = "INSERT INTO timeline_images(path,filename,content_type,size_bytes,created_at,pending) VALUES(?1,?2,?3,?4,?5,?6)".to_string();
            if !pending {
                // import_one has already confirmed the old local original is missing.
                sql.push_str(" ON CONFLICT(path) DO UPDATE SET filename=excluded.filename,content_type=excluded.content_type,size_bytes=excluded.size_bytes,created_at=excluded.created_at,pending=excluded.pending");
            }
            conn.execute(&sql, params![path,filename,mime,bytes.len() as u64,chrono::Utc::now().timestamp(),pending])?;
            Ok(())
        });
        if result.is_err() {
            let _ = tokio::fs::remove_file(dest).await;
        }
        result
    }

    pub(crate) fn asset(&self, path: &str) -> Result<(PathBuf, String), BrainError> {
        safe_relative(path)?;
        self.db.with_connection(|conn| {
            let result = conn
                .query_row(
                    "SELECT filename,content_type FROM timeline_images WHERE path=?1",
                    [path],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let (filename, mime) =
                result.ok_or_else(|| BrainError::NoteNotFound(PathBuf::from(path)))?;
            if Path::new(&filename).components().count() != 1
                || !matches!(
                    Path::new(&filename).components().next(),
                    Some(Component::Normal(_))
                )
            {
                return Err(BrainError::ConfigError("图片元数据路径非法".into()));
            }
            let file = self.root.join("images").join(filename);
            if std::fs::symlink_metadata(&file)?.file_type().is_symlink() {
                return Err(BrainError::ConfigError("不支持符号链接图片".into()));
            }
            Ok((file, mime))
        })
    }

    pub async fn original(&self, path: &str) -> Result<(Vec<u8>, String), BrainError> {
        let _guard = self.mutation.lock().await;
        let (file, mime) = self.asset(path)?;
        Ok((tokio::fs::read(file).await?, mime))
    }

    pub async fn thumbnail(&self, path: &str) -> Result<(Vec<u8>, String), BrainError> {
        let _guard = self.mutation.lock().await;
        let (file, mime) = self.asset(path)?;
        match self.cache_get(path).await {
            Ok(Some(bytes)) => return Ok((bytes, "image/jpeg".into())),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "缓存不可用，从原图生成缩略图"),
        }
        let bytes = tokio::fs::read(file).await?;
        let generated = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, BrainError> {
            let img = decode_image(&bytes)?;
            let thumbnail = img
                .resize(400, 400, image::imageops::FilterType::Triangle)
                .to_rgb8();
            let mut output = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(thumbnail)
                .write_to(&mut output, image::ImageFormat::Jpeg)
                .map_err(|e| BrainError::Internal(format!("缩略图生成失败: {e}")))?;
            Ok(output.into_inner())
        })
        .await
        .map_err(|e| BrainError::Internal(e.to_string()))?;
        match generated {
            Ok(bytes) => {
                if let Err(error) = self.cache_put(path, &bytes).await {
                    tracing::warn!(%error, "缩略图缓存写入失败，仍返回图片");
                }
                Ok((bytes, "image/jpeg".into()))
            }
            Err(error) => {
                tracing::warn!(%error, "缩略图生成失败，返回原图");
                Ok((tokio::fs::read(self.asset(path)?.0).await?, mime))
            }
        }
    }

    fn cache_file(&self, key: &str) -> PathBuf {
        self.root
            .join("cache")
            .join(hex::encode(Sha256::digest(key.as_bytes())))
    }

    async fn cache_get(&self, key: &str) -> Result<Option<Vec<u8>>, BrainError> {
        let _guard = self.cache_lock.lock().await;
        let file = self.cache_file(key);
        let expected = self.db.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT size_bytes FROM timeline_image_cache WHERE key=?1",
                    [key],
                    |r| r.get::<_, u64>(0),
                )
                .optional()?)
        })?;
        let metadata = tokio::fs::symlink_metadata(&file).await.ok();
        if expected.is_none()
            || metadata.as_ref().is_some_and(|m| {
                !m.is_file()
                    || Some(m.len()) != expected
                    || m.len() > self.budget.load(Ordering::Relaxed)
            })
        {
            if metadata.is_some() {
                remove_if_present(&file).await?;
            }
            self.db.with_connection(|conn| {
                conn.execute("DELETE FROM timeline_image_cache WHERE key=?1", [key])?;
                Ok(())
            })?;
            return Ok(None);
        }
        match tokio::fs::read(file).await {
            Ok(bytes) => {
                self.db.with_connection(|conn| {
                    conn.execute("UPDATE timeline_image_cache SET last_used=(SELECT COALESCE(MAX(last_used),0)+1 FROM timeline_image_cache) WHERE key=?1", [key])?;
                    Ok(())
                })?;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.db.with_connection(|conn| {
                    conn.execute("DELETE FROM timeline_image_cache WHERE key=?1", [key])?;
                    Ok(())
                })?;
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn cache_put(&self, key: &str, bytes: &[u8]) -> Result<(), BrainError> {
        let _guard = self.cache_lock.lock().await;
        if bytes.len() as u64 > self.budget.load(Ordering::Relaxed) {
            return Ok(());
        }
        // Evict before publishing so even transient on-disk cache size stays bounded.
        self.evict_locked(bytes.len() as u64).await?;
        let mut temporary = tempfile::NamedTempFile::new_in(self.root.join("cache"))?;
        use std::io::Write;
        temporary.write_all(bytes)?;
        temporary
            .persist(self.cache_file(key))
            .map_err(|e| BrainError::IoError(e.error))?;
        let result = self.db.with_connection(|conn| {
            conn.execute("INSERT INTO timeline_image_cache(key,size_bytes,last_used) VALUES(?1,?2,(SELECT COALESCE(MAX(last_used),0)+1 FROM timeline_image_cache)) ON CONFLICT(key) DO UPDATE SET size_bytes=excluded.size_bytes,last_used=excluded.last_used",params![key,bytes.len() as u64])?;
            Ok(())
        });
        if result.is_err() {
            let _ = remove_if_present(&self.cache_file(key)).await;
        }
        result?;
        self.evict_locked(0).await
    }

    async fn evict_locked(&self, reserve: u64) -> Result<(), BrainError> {
        let entries = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT key,size_bytes FROM timeline_image_cache ORDER BY last_used ASC,key ASC",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;
        let mut size: u64 = entries.iter().map(|(_, s)| s).sum();
        for (key, bytes) in entries {
            if size.saturating_add(reserve) <= self.budget.load(Ordering::Relaxed) {
                break;
            }
            remove_if_present(&self.cache_file(&key)).await?;
            self.db.with_connection(|conn| {
                conn.execute("DELETE FROM timeline_image_cache WHERE key=?1", [&key])?;
                Ok(())
            })?;
            size = size.saturating_sub(bytes);
        }
        Ok(())
    }

    pub async fn set_budget(&self, bytes: u64) -> Result<(), BrainError> {
        let _guard = self.cache_lock.lock().await;
        self.budget.store(bytes, Ordering::Relaxed);
        self.evict_locked(0).await
    }

    pub async fn clear_cache(&self) -> Result<(), BrainError> {
        let budget = self.budget.load(Ordering::Relaxed);
        let _guard = self.cache_lock.lock().await;
        self.budget.store(0, Ordering::Relaxed);
        let result = self.evict_locked(0).await;
        self.budget.store(budget, Ordering::Relaxed);
        result
    }

    pub async fn import_one(&self, legacy: &Path, path: &str) -> Result<(), BrainError> {
        let _guard = self.mutation.lock().await;
        if self.asset(path).is_ok() {
            return Ok(());
        }
        safe_relative(path)?;
        let root = legacy.canonicalize()?;
        let source = root.join(path).canonicalize()?;
        if !source.starts_with(&root) || !source.is_file() {
            return Err(BrainError::ConfigError("旧图片路径超出所选目录".into()));
        }
        if tokio::fs::metadata(&source).await?.len() > MAX_UPLOAD_BYTES as u64 {
            return Err(BrainError::ConfigError(
                "旧图片超过 20 MB，未改动原图".into(),
            ));
        }
        let bytes = tokio::fs::read(source).await?;
        let mime = validate_image(bytes.clone()).await?;
        let filename = format!("{}.{}", uuid::Uuid::new_v4(), extension(&mime));
        let _cache_guard = self.cache_lock.lock().await;
        remove_if_present(&self.cache_file(path)).await?;
        self.db.with_connection(|conn| {
            conn.execute("DELETE FROM timeline_image_cache WHERE key=?1", [path])?;
            Ok(())
        })?;
        self.publish(path, &filename, &bytes, &mime, false).await
    }

    pub async fn import_referenced(
        &self,
        directory: &Path,
    ) -> Result<ImageImportReport, BrainError> {
        let paths = self.referenced_paths()?;
        let mut report = ImageImportReport::default();
        for path in paths {
            if self.asset(&path).is_ok() {
                continue;
            }
            match self.import_one(directory, &path).await {
                Ok(()) => {
                    report.copied += 1;
                    // A memo can be deleted while this background copy is in progress.
                    // Never leave the newly copied original permanently orphaned.
                    let _guard = self.mutation.lock().await;
                    self.db.with_connection(|conn| {
                        if !Self::referenced(conn, &path)? {
                            conn.execute(
                                "INSERT OR IGNORE INTO timeline_image_gc(path) VALUES(?1)",
                                [&path],
                            )?;
                        }
                        Ok(())
                    })?;
                    self.collect_locked().await?;
                }
                Err(error) => {
                    tracing::warn!(%path,%error,"旧小记图片未迁移，保留记录并可重试");
                    report.missing.push(path);
                }
            }
        }
        Ok(report)
    }

    fn referenced_paths(&self) -> Result<Vec<String>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT DISTINCT value FROM memos,json_each(CASE WHEN json_valid(images) THEN images ELSE '[]' END) WHERE json_each.type='text'")?;
            let mut paths = stmt.query_map([], |r| r.get::<_,String>(0))?.collect::<Result<std::collections::BTreeSet<_>,_>>()?;
            let mut stmt = conn.prepare("SELECT content FROM memos")?;
            let contents = stmt.query_map([], |r| r.get::<_,String>(0))?;
            for content in contents {
                let content = content?;
                for embed in content.split("![[").skip(1) {
                    if let Some((raw, _)) = embed.split_once("]]") {
                        let path = raw.split('|').next().unwrap_or("").trim();
                        if safe_relative(path).is_ok() { paths.insert(path.to_string()); }
                    }
                }
                for event in pulldown_cmark::Parser::new(&content) {
                    if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image { dest_url, .. }) = event {
                        if safe_relative(&dest_url).is_ok() { paths.insert(dest_url.to_string()); }
                    }
                }
            }
            Ok(paths.into_iter().collect())
        })
    }

    pub(crate) fn referenced(conn: &Connection, path: &str) -> Result<bool, BrainError> {
        Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM memos WHERE EXISTS(SELECT 1 FROM json_each(CASE WHEN json_valid(images) THEN images ELSE '[]' END) WHERE value=?1) OR instr(content,?1)>0 OR (NOT json_valid(images) AND instr(images,?1)>0))",[path],|r| r.get(0))?)
    }

    pub async fn collect(&self) -> Result<u64, BrainError> {
        let _guard = self.mutation.lock().await;
        self.collect_locked().await
    }

    pub(crate) async fn collect_locked(&self) -> Result<u64, BrainError> {
        self.db.with_connection(|conn| {
            conn.execute("INSERT OR IGNORE INTO timeline_image_gc(path) SELECT path FROM timeline_images WHERE pending=1 AND created_at<?1", [chrono::Utc::now().timestamp()-86400])?;
            Ok(())
        })?;
        let queue = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT path FROM timeline_image_gc")?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;
        for path in queue {
            if self
                .db
                .with_connection(|conn| Self::referenced(conn, &path))?
            {
                self.db.with_connection(|conn| {
                    conn.execute("DELETE FROM timeline_image_gc WHERE path=?1", [&path])?;
                    Ok(())
                })?;
                continue;
            }
            // Registered filenames only. Never delete a file from the legacy directory.
            let asset = self.db.with_connection(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT filename FROM timeline_images WHERE path=?1",
                        [&path],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?)
            })?;
            if let Some(filename) = asset {
                if Path::new(&filename).components().count() != 1
                    || !matches!(
                        Path::new(&filename).components().next(),
                        Some(Component::Normal(_))
                    )
                {
                    continue;
                }
                let result = async {
                    let _cache_guard = self.cache_lock.lock().await;
                    remove_if_present(&self.cache_file(&path)).await?;
                    remove_if_present(&self.root.join("images").join(filename)).await?;
                    self.db.transaction(|conn| {
                        conn.execute("DELETE FROM timeline_image_cache WHERE key=?1", [&path])?;
                        conn.execute("DELETE FROM timeline_images WHERE path=?1", [&path])?;
                        conn.execute("DELETE FROM timeline_image_gc WHERE path=?1", [&path])?;
                        Ok(())
                    })
                }
                .await;
                if let Err(error) = result {
                    tracing::warn!(%path,%error,"图片清理未完成，保留重试队列");
                }
            } else {
                self.db.with_connection(|conn| {
                    conn.execute("DELETE FROM timeline_image_gc WHERE path=?1", [&path])?;
                    Ok(())
                })?;
            }
        }
        self.pending_cleanup()
    }

    pub async fn discard(&self, paths: &[String]) -> Result<u64, BrainError> {
        let _guard = self.mutation.lock().await;
        self.db.transaction(|conn| {
            for path in paths {
                // Discard can only release uncommitted uploads, never someone else's published attachment.
                conn.execute("INSERT OR IGNORE INTO timeline_image_gc SELECT path FROM timeline_images WHERE path=?1 AND pending=1",[path])?;
            }
            Ok(())
        })?;
        self.collect_locked().await
    }

    fn pending_cleanup(&self) -> Result<u64, BrainError> {
        self.db.with_connection(|conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM timeline_image_gc", [], |r| r.get(0))?)
        })
    }

    pub async fn stats(&self) -> Result<ImageStorageStats, BrainError> {
        let missing_images = self
            .referenced_paths()?
            .into_iter()
            .filter(|p| self.asset(p).is_err())
            .collect();
        let (originals_bytes, cache_bytes) = self.db.with_connection(|conn| {
            Ok((
                conn.query_row(
                    "SELECT COALESCE(SUM(size_bytes),0) FROM timeline_images",
                    [],
                    |r| r.get(0),
                )?,
                conn.query_row(
                    "SELECT COALESCE(SUM(size_bytes),0) FROM timeline_image_cache",
                    [],
                    |r| r.get(0),
                )?,
            ))
        })?;
        Ok(ImageStorageStats {
            directory: self.root.display().to_string(),
            originals_bytes,
            cache_bytes,
            cache_limit_bytes: self.budget.load(Ordering::Relaxed),
            missing_images,
            pending_cleanup: self.pending_cleanup()?,
        })
    }

    /// Reconcile files left by a crash, enforce a changed limit, and retry durable deletions.
    pub async fn maintain(&self) -> Result<(), BrainError> {
        let _guard = self.mutation.lock().await;
        self.collect_locked().await?;
        let _cache_guard = self.cache_lock.lock().await;
        let (originals, caches) = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT filename FROM timeline_images")?;
            let originals = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<std::collections::HashSet<_>, _>>()?;
            let mut stmt = conn.prepare("SELECT key FROM timeline_image_cache")?;
            let caches = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .map(|r| r.map(|key| hex::encode(Sha256::digest(key.as_bytes()))))
                .collect::<Result<std::collections::HashSet<_>, _>>()?;
            Ok((originals, caches))
        })?;
        for (directory, known, grace) in [("images", originals, 86400), ("cache", caches, 0)] {
            let mut entries = tokio::fs::read_dir(self.root.join(directory)).await?;
            while let Some(entry) = entries.next_entry().await? {
                if known.contains(&entry.file_name().to_string_lossy().to_string())
                    || !entry.file_type().await?.is_file()
                {
                    continue;
                }
                let age = entry
                    .metadata()
                    .await?
                    .modified()?
                    .elapsed()
                    .unwrap_or_default()
                    .as_secs();
                if age >= grace {
                    remove_if_present(&entry.path()).await?;
                }
            }
        }
        self.evict_locked(0).await
    }
}

fn safe_relative(path: &str) -> Result<(), BrainError> {
    if path.is_empty()
        || path.contains('\\')
        || path.contains(':')
        || path.contains('\0')
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(BrainError::ConfigError("图片路径必须为安全相对路径".into()));
    }
    Ok(())
}

async fn remove_if_present(path: &Path) -> Result<(), BrainError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn decode_image(bytes: &[u8]) -> Result<image::DynamicImage, BrainError> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|e| BrainError::ConfigError(format!("图片格式或尺寸不支持: {e}")))
}

async fn validate_image(bytes: Vec<u8>) -> Result<String, BrainError> {
    if bytes.is_empty() || bytes.len() > MAX_UPLOAD_BYTES {
        return Err(BrainError::ConfigError("每张图片需在 20 MB 以内".into()));
    }
    tokio::task::spawn_blocking(move || {
        let mime = match image::guess_format(&bytes) {
            Ok(image::ImageFormat::Jpeg) => "image/jpeg",
            Ok(image::ImageFormat::Png) => "image/png",
            Ok(image::ImageFormat::Gif) => "image/gif",
            Ok(image::ImageFormat::WebP) => "image/webp",
            _ => {
                return Err(BrainError::ConfigError(
                    "仅支持 PNG、JPEG、GIF、WebP 图片".into(),
                ))
            }
        };
        decode_image(&bytes)?;
        Ok(mime.to_string())
    })
    .await
    .map_err(|e| BrainError::Internal(e.to_string()))?
}

fn extension(mime: &str) -> &str {
    match mime {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(limit: u64) -> (TimelineImages, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = std::sync::Arc::new(
            crate::infra::sqlite_store::SqliteStore::new(&dir.path().join("brain.db")).unwrap(),
        );
        (
            TimelineImages::new(db, dir.path().join("timeline"), limit).unwrap(),
            dir,
        )
    }

    fn png() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(20, 20)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[tokio::test]
    async fn test_local_upload_unique_and_path_safe() {
        let (store, _dir) = store(1024);
        let a = store.upload(&png()).await.unwrap();
        let b = store.upload(&png()).await.unwrap();
        assert_ne!(a, b);
        assert_eq!(store.original(&a).await.unwrap().0, png());
        assert!(store.original("../../brain.db").await.is_err());
        assert!(store.upload(b"not an image").await.is_err());
    }

    #[tokio::test]
    async fn test_cache_evicts_lru_by_bytes_and_keeps_originals() {
        let (store, _dir) = store(8);
        store.cache_put("a", b"1234").await.unwrap();
        store.cache_put("b", b"5678").await.unwrap();
        assert!(store.cache_get("a").await.unwrap().is_some());
        store.cache_put("c", b"abcd").await.unwrap();
        assert!(store.cache_get("b").await.unwrap().is_none());
        assert!(store.cache_get("a").await.unwrap().is_some());
        store.cache_put("oversize", b"123456789").await.unwrap();
        assert!(store.cache_get("oversize").await.unwrap().is_none());
        let path = store.upload(&png()).await.unwrap();
        store.thumbnail(&path).await.unwrap();
        assert_eq!(store.original(&path).await.unwrap().0, png());
        assert!(store.stats().await.unwrap().cache_bytes <= 8);
        store.set_budget(4).await.unwrap();
        assert!(store.stats().await.unwrap().cache_bytes <= 4);
        store.clear_cache().await.unwrap();
        assert_eq!(store.stats().await.unwrap().cache_bytes, 0);
        store.set_budget(0).await.unwrap();
        assert!(!store.thumbnail(&path).await.unwrap().0.is_empty());
        assert_eq!(store.stats().await.unwrap().cache_bytes, 0);
        assert_eq!(store.original(&path).await.unwrap().0, png());
    }

    #[tokio::test]
    async fn test_migration_is_copy_only_retryable_and_rejects_escape() {
        let (store, dir) = store(1024);
        let legacy = dir.path().join("old-vault");
        std::fs::create_dir_all(legacy.join("Timeline/images")).unwrap();
        let path = "Timeline/images/中文 #问号?.png";
        std::fs::write(legacy.join(path), png()).unwrap();
        store.import_one(&legacy, path).await.unwrap();
        store.import_one(&legacy, path).await.unwrap();
        assert!(legacy.join(path).exists());
        assert_eq!(store.original(path).await.unwrap().0, png());
        assert!(store.import_one(&legacy, "../brain.db").await.is_err());
        assert!(store.import_one(&legacy, "missing.png").await.is_err());
    }

    #[tokio::test]
    async fn test_legacy_inline_images_are_discovered_and_invalid_json_is_safe() {
        let (store, dir) = store(1024);
        let legacy = dir.path().join("old-vault");
        let path = "Timeline/images/inline.png";
        std::fs::create_dir_all(legacy.join("Timeline/images")).unwrap();
        std::fs::write(legacy.join(path), png()).unwrap();
        store.db.with_connection(|conn| {
            conn.execute("INSERT INTO memos(id,timestamp,date,content,images,tags,file_path) VALUES('old','2026-10-04T00:00:00Z','2026-10-04',?1,'broken','[]','old.md')", [format!("![[{path}]]")])?;
            assert!(TimelineImages::referenced(conn, path)?);
            Ok(())
        }).unwrap();
        assert_eq!(store.import_referenced(&legacy).await.unwrap().copied, 1);
        assert_eq!(store.original(path).await.unwrap().0, png());
        assert!(legacy.join(path).exists());
    }

    #[tokio::test]
    async fn test_unwritable_cache_does_not_prevent_viewing_images() {
        let (store, _dir) = store(1024 * 1024);
        let path = store.upload(&png()).await.unwrap();
        // Simulate a failed cache directory without affecting original storage.
        std::fs::remove_dir(store.root.join("cache")).unwrap();
        std::fs::write(store.root.join("cache"), b"unavailable").unwrap();
        let (bytes, mime) = store.thumbnail(&path).await.unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(image::load_from_memory(&bytes).is_ok());
        assert_eq!(store.original(&path).await.unwrap().0, png());
    }

    #[tokio::test]
    async fn test_import_repairs_missing_original_without_changing_memo_reference() {
        let (store, dir) = store(1024 * 1024);
        let legacy = dir.path().join("old-vault");
        let path = "Timeline/images/repair.png";
        std::fs::create_dir_all(legacy.join("Timeline/images")).unwrap();
        std::fs::write(legacy.join(path), png()).unwrap();
        store.import_one(&legacy, path).await.unwrap();
        store.thumbnail(path).await.unwrap();
        std::fs::remove_file(store.asset(path).unwrap().0).unwrap();
        store.import_one(&legacy, path).await.unwrap();
        assert_eq!(store.original(path).await.unwrap().0, png());
        assert!(legacy.join(path).exists());
        assert_eq!(store.stats().await.unwrap().cache_bytes, 0);
    }

    #[tokio::test]
    async fn test_cache_rejects_unregistered_and_oversized_files() {
        let (store, _dir) = store(8);
        std::fs::write(store.cache_file("orphan"), b"1234").unwrap();
        assert!(store.cache_get("orphan").await.unwrap().is_none());
        assert!(!store.cache_file("orphan").exists());
        store.cache_put("changed", b"1234").await.unwrap();
        std::fs::write(store.cache_file("changed"), b"123456789").unwrap();
        assert!(store.cache_get("changed").await.unwrap().is_none());
        assert_eq!(store.stats().await.unwrap().cache_bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn test_storage_rejects_directory_symlinks_without_touching_target() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(SqliteStore::new(&dir.path().join("brain.db")).unwrap());
        let external = dir.path().join("external");
        std::fs::create_dir(&external).unwrap();
        std::fs::write(external.join("kept.txt"), "keep").unwrap();
        let root = dir.path().join("timeline");
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&external, root.join("images")).unwrap();
        assert!(TimelineImages::new(db, root, 0).is_err());
        assert_eq!(
            std::fs::read_to_string(external.join("kept.txt")).unwrap(),
            "keep"
        );
    }
}
