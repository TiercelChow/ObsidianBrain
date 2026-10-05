//! Explicit, one-shot image migration without starting any application services.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{Connection, DatabaseName, OpenFlags};
use serde::Serialize;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::timeline_images::{TimelineImages, DEFAULT_CACHE_BYTES};

#[derive(Serialize)]
pub struct TimelineMigrationReport {
    pub database: PathBuf,
    pub source: PathBuf,
    pub directory: PathBuf,
    pub backup: PathBuf,
    pub copied: usize,
    pub missing: Vec<String>,
}

pub async fn migrate_timeline_images(
    database: &Path,
    source: &Path,
) -> Result<TimelineMigrationReport, BrainError> {
    // Validate both targets before opening a writable connection or creating directories.
    if !database.is_file() || !source.is_dir() {
        return Err(BrainError::KnowledgeValidation(
            "必须指定已存在的数据库文件和旧图片根目录".into(),
        ));
    }
    let database = database.canonicalize()?;
    let source = source.canonicalize()?;
    let conn = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let _: i64 = conn.query_row("SELECT COUNT(*) FROM memos", [], |row| row.get(0))?;
    let parent = database
        .parent()
        .ok_or_else(|| BrainError::KnowledgeValidation("数据库父目录无效".into()))?;
    let backup_dir = parent.join("backups");
    std::fs::create_dir_all(&backup_dir)?;
    let backup = backup_dir.join(format!(
        "timeline-import-{}-{}.sqlite3",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().simple()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // SQLite's backup API includes WAL writes; copying the DB file alone would not.
    drop(options.open(&backup)?);
    conn.backup(DatabaseName::Main, &backup, None)?;
    drop(conn);
    // FTS5 validation needs a transient write lock. Check the private snapshot,
    // not the live DB: a read-only PRAGMA would falsely report a corrupt index.
    let validation = SqliteStore::validate_database_file(&backup)?;
    if !validation.integrity_ok || validation.foreign_key_violations > 0 {
        return Err(BrainError::Internal(format!(
            "迁移前快照校验未通过，未迁移图片: {}, 外键异常 {} 条",
            validation.integrity_message, validation.foreign_key_violations
        )));
    }

    // Keep every existing snapshot during this manual operation, including the automatic
    // pre-schema snapshot. This explicit backup is outside the managed pruning namespace.
    let db = Arc::new(SqliteStore::new_with_backup_retention(
        &database,
        usize::MAX,
    )?);
    let budget = db
        .get_state("timeline_cache_limit_bytes")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CACHE_BYTES);
    let directory = crate::paths::timeline_dir(&database);
    let images = TimelineImages::new(db.clone(), directory.clone(), budget)?;
    let report = images.import_referenced(&source).await?;
    db.set_state("timeline_legacy_directory", &source.to_string_lossy())?;
    Ok(TimelineMigrationReport {
        database,
        source,
        directory,
        backup,
        copied: report.copied,
        missing: report.missing,
    })
}
