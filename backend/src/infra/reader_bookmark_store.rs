//! Per-book annotations; never modifies a source document or reading progress.
use crate::{
    error::BrainError,
    infra::{reader_progress_store::belongs, sqlite_store::SqliteStore},
    models::reader_bookmark::{BookmarkLocation, BookmarkWrite, ReaderBookmark},
};
use rusqlite::{params, OptionalExtension};
use std::sync::Arc;

pub struct ReaderBookmarkStore {
    db: Arc<SqliteStore>,
}

fn invalid(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(format!("书签：{message}"))
}

fn active_book(conn: &rusqlite::Connection, id: &str) -> Result<(String, String), BrainError> {
    conn.query_row(
        "SELECT path,kind FROM reader_books WHERE id=?1 AND shelf_state='active'",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| BrainError::KnowledgeNotFound(format!("书籍 {id}")))
}

impl ReaderBookmarkStore {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }
    pub fn list(&self, book_id: &str) -> Result<Vec<ReaderBookmark>, BrainError> {
        self.db.with_connection(|conn| {
            active_book(conn, book_id)?;
            let mut statement = conn.prepare("SELECT id,book_id,file_path,title,note_md,location_json,revision,created_at,updated_at FROM reader_bookmarks WHERE book_id=?1 ORDER BY file_path,created_at,id")?;
            let rows = statement.query_map([book_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,i64>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?)))?;
            rows.map(|row| {
                let (id,book_id,file_path,title,note_md,json,revision,created_at,updated_at) = row?;
                let location = serde_json::from_str(&json).map_err(|error| BrainError::Internal(format!("书签定位无法解析: {error}")))?;
                Ok(ReaderBookmark { id,book_id,file_path,title,note_md,location,revision,created_at,updated_at })
            }).collect()
        })
    }

    pub fn save(&self, mut draft: BookmarkWrite) -> Result<ReaderBookmark, BrainError> {
        draft.title = draft.title.trim().to_string();
        if draft.book_id.is_empty()
            || draft.book_id.len() > 200
            || draft.title.is_empty()
            || draft.title.chars().count() > 240
            || draft.note_md.len() > 128 * 1024
        {
            return Err(invalid("标题必填（最多 240 字），笔记最多 128KiB"));
        }
        if draft.id.is_some() != draft.revision.is_some() || draft.revision.is_some_and(|v| v < 1) {
            return Err(invalid("更新必须提供书签 ID 与版本"));
        }
        let unit = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
        let kind = match &draft.location {
            BookmarkLocation::Epub { chapter, ratio } => {
                if !unit(*ratio)
                    || chapter.is_empty()
                    || chapter.len() > 4096
                    || chapter.contains(['\\', '\0', ':'])
                    || chapter.starts_with('/')
                    || chapter.split('/').any(|p| p == "..")
                {
                    return Err(invalid("EPUB 定位不合法"));
                }
                "epub"
            }
            BookmarkLocation::Md {
                ratio,
                block_index,
                text,
                offset,
            } => {
                if !unit(*ratio)
                    || !unit(*offset)
                    || *block_index > 1_000_000
                    || text.chars().count() > 240
                {
                    return Err(invalid("Markdown 定位不合法"));
                }
                "md"
            }
            BookmarkLocation::Pdf { page, offset, x } => {
                if !(1..=1_000_000).contains(page) || !unit(*offset) || !unit(*x) {
                    return Err(invalid("PDF 定位不合法"));
                }
                "pdf"
            }
        };
        let lower = draft.file_path.to_ascii_lowercase();
        if (kind == "pdf" && !lower.ends_with(".pdf"))
            || (kind == "epub" && !lower.ends_with(".epub"))
            || (kind == "md" && !lower.ends_with(".md") && !lower.ends_with(".markdown"))
        {
            return Err(invalid("文件类型与定位类型不一致"));
        }
        let location_json = serde_json::to_string(&draft.location)
            .map_err(|e| BrainError::Internal(e.to_string()))?;
        self.db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            let (path,kind) = active_book(&tx, &draft.book_id)?;
            if !belongs(&draft.file_path,&path,&kind)? { return Err(invalid("文件不属于这本书")); }
            let (count,bytes):(i64,i64) = tx.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(note_md AS BLOB))),0) FROM reader_bookmarks WHERE book_id=?1 AND id<>?2", params![draft.book_id,draft.id.as_deref().unwrap_or("")], |r|Ok((r.get(0)?,r.get(1)?)))?;
            if count >= 1000 || bytes + draft.note_md.len() as i64 > 8 * 1024 * 1024 { return Err(invalid("每书最多 1000 条书签，笔记总量最多 8MiB")); }
            let now = chrono::Utc::now().to_rfc3339();
            let id = draft.id.clone().unwrap_or_else(||uuid::Uuid::new_v4().to_string());
            let (revision,created_at) = if let Some(revision) = draft.revision {
                let created:Option<String> = tx.query_row("SELECT created_at FROM reader_bookmarks WHERE id=?1 AND book_id=?2 AND revision=?3", params![id,draft.book_id,revision], |r|r.get(0)).optional()?;
                let created = created.ok_or_else(||BrainError::KnowledgeConflict("书签已更改或删除，请刷新后重试；当前草稿未覆盖".into()))?;
                tx.execute("UPDATE reader_bookmarks SET title=?1,note_md=?2,file_path=?3,location_json=?4,revision=revision+1,updated_at=?5 WHERE id=?6 AND book_id=?7 AND revision=?8",params![draft.title,draft.note_md,draft.file_path,location_json,now,id,draft.book_id,revision])?;
                (revision+1,created)
            } else {
                tx.execute("INSERT INTO reader_bookmarks (id,book_id,file_path,title,note_md,location_json,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",params![id,draft.book_id,draft.file_path,draft.title,draft.note_md,location_json,now])?;
                (1,now.clone())
            };
            tx.commit()?;
            Ok(ReaderBookmark {id,book_id:draft.book_id,file_path:draft.file_path,title:draft.title,note_md:draft.note_md,location:draft.location,revision,created_at,updated_at:now})
        })
    }

    pub fn delete(&self, book_id: &str, id: &str, revision: i64) -> Result<(), BrainError> {
        self.db.with_connection(|conn| {
            let tx = conn.unchecked_transaction()?;
            active_book(&tx, book_id)?;
            let changed = tx.execute(
                "DELETE FROM reader_bookmarks WHERE book_id=?1 AND id=?2 AND revision=?3",
                params![book_id, id, revision],
            )?;
            if changed != 1 {
                return Err(BrainError::KnowledgeConflict(
                    "书签已更改或删除，请刷新后重试".into(),
                ));
            }
            tx.commit()?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "reader_bookmark_store_tests.rs"]
mod tests;
