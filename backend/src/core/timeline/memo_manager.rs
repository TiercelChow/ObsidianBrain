//! SQLite-authoritative memos; attachment changes and cleanup are coordinated.
use chrono::{Local, Utc};
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::timeline_images::TimelineImages;
use crate::models::timeline::{BrowseTimelineRequest, Memo, MemoCreateRequest, MemoQuery};

#[cfg(test)]
#[path = "memo_manager_tests.rs"]
mod tests;

pub struct MemoManager {
    db: Arc<SqliteStore>,
    pub images: Arc<TimelineImages>,
}

impl MemoManager {
    pub fn new(db: Arc<SqliteStore>, images: Arc<TimelineImages>) -> Self {
        Self { db, images }
    }

    fn validate(&self, request: &MemoCreateRequest, old: &[String]) -> Result<(), BrainError> {
        if request.content.trim().is_empty() && request.images.is_empty() {
            return Err(BrainError::MemoValidation("请填写内容或添加图片".into()));
        }
        if request.content.len() > 100_000 || request.images.len() > 9 || request.tags.len() > 30 {
            return Err(BrainError::MemoValidation(
                "内容过长、图片超过 9 张或标签超过 30 个".into(),
            ));
        }
        for path in &request.images {
            if !old.contains(path) && self.images.asset(path).is_err() {
                return Err(BrainError::MemoValidation(format!(
                    "图片不存在或尚未完成上传: {path}"
                )));
            }
        }
        Ok(())
    }

    pub async fn create_memo(&self, request: MemoCreateRequest) -> Result<Memo, BrainError> {
        let _guard = self.images.mutation.lock().await;
        self.validate(&request, &[])?;
        let now = Utc::now();
        let memo = Memo {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            date: Local::now().format("%Y-%m-%d").to_string(),
            content: request.content,
            images: request.images,
            tags: request.tags,
            file_path: String::new(),
            created_at: now,
            revision: 1,
        };
        self.db.transaction(|conn| {
            conn.execute("INSERT INTO memos(id,timestamp,date,content,images,tags,file_path,created_at,revision) VALUES(?1,?2,?3,?4,?5,?6,'',?2,1)",params![memo.id,memo.timestamp.to_rfc3339(),memo.date,memo.content,json(&memo.images)?,json(&memo.tags)?])?;
            publish_references(conn,&memo)?;
            Ok(())
        })?;
        Ok(memo)
    }

    pub fn get_memo(&self, id: &str) -> Result<Memo, BrainError> {
        let rows = self.db.query_memos("SELECT id,timestamp,date,content,images,tags,file_path,created_at,revision FROM memos WHERE id=?", &[id.into()])?;
        rows.into_iter()
            .next()
            .map(|r| self.row_to_memo(r))
            .ok_or_else(|| BrainError::MemoNotFound(id.into()))
    }

    pub async fn update_memo(
        &self,
        id: &str,
        revision: i64,
        request: MemoCreateRequest,
    ) -> Result<Memo, BrainError> {
        let _guard = self.images.mutation.lock().await;
        let old = self.get_memo(id)?;
        self.validate(&request, &old.images)?;
        let updated = Memo {
            content: request.content,
            images: request.images,
            tags: request.tags,
            revision: old.revision + 1,
            ..old.clone()
        };
        self.db.transaction(|conn| {
            if conn.execute("UPDATE memos SET content=?1,images=?2,tags=?3,revision=revision+1 WHERE id=?4 AND revision=?5",params![updated.content,json(&updated.images)?,json(&updated.tags)?,id,revision])? !=1 { return Err(BrainError::MemoConflict("小记已被修改，请刷新后重试".into())); }
            queue_old_images(conn,&old)?;
            publish_references(conn,&updated)?;
            Ok(())
        })?;
        let pending = self.images.collect_locked().await?;
        if pending > 0 {
            tracing::warn!(pending, "小记保存成功，部分图片清理待重试");
        }
        Ok(updated)
    }

    pub async fn delete_memo(&self, id: &str, revision: i64) -> Result<u64, BrainError> {
        let _guard = self.images.mutation.lock().await;
        let old = self.get_memo(id)?;
        self.db.transaction(|conn| {
            if conn.execute(
                "DELETE FROM memos WHERE id=?1 AND revision=?2",
                params![id, revision],
            )? != 1
            {
                return Err(BrainError::MemoConflict(
                    "小记已被修改，请刷新后重试".into(),
                ));
            }
            queue_old_images(conn, &old)?;
            Ok(())
        })?;
        self.images.collect_locked().await
    }

    /// 统计小记总数
    pub fn count_memos(&self) -> Result<u32, BrainError> {
        self.db.count_memos()
    }

    /// 浏览时间线
    pub async fn browse_timeline(
        &self,
        request: BrowseTimelineRequest,
    ) -> Result<Vec<Memo>, BrainError> {
        let mut sql = String::from(
            "SELECT id, timestamp, date, content, images, tags, file_path, created_at, revision FROM memos WHERE 1=1",
        );
        let mut params = Vec::new();

        if let Some(ref start) = request.start_date {
            sql.push_str(" AND date >= ?");
            params.push(start.clone());
        }
        if let Some(ref end) = request.end_date {
            sql.push_str(" AND date <= ?");
            params.push(end.clone());
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params.push(request.limit.to_string());
        params.push(request.offset.to_string());

        let rows = self.db.query_memos(&sql, &params)?;
        let memos = rows.into_iter().map(|row| self.row_to_memo(row)).collect();

        Ok(memos)
    }

    /// 搜索小记
    pub async fn search_memos(&self, query: MemoQuery) -> Result<Vec<Memo>, BrainError> {
        let mut sql = String::from(
            "SELECT id, timestamp, date, content, images, tags, file_path, created_at, revision FROM memos WHERE (content LIKE ? OR tags LIKE ?)",
        );
        let mut params = vec![
            format!("%{}%", query.query.clone().unwrap_or_default()),
            format!("%{}%", query.query.unwrap_or_default()),
        ];

        if let Some(ref start) = query.start_date {
            sql.push_str(" AND date >= ?");
            params.push(start.clone());
        }
        if let Some(ref end) = query.end_date {
            sql.push_str(" AND date <= ?");
            params.push(end.clone());
        }
        if let Some(ref tags) = query.tags {
            for tag in tags {
                sql.push_str(" AND tags LIKE ?");
                params.push(format!("%{}%", tag));
            }
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");
        params.push(query.limit.to_string());
        params.push(query.offset.to_string());

        let rows = self.db.query_memos(&sql, &params)?;
        let memos = rows.into_iter().map(|row| self.row_to_memo(row)).collect();

        Ok(memos)
    }

    fn row_to_memo(
        &self,
        (id, timestamp, date, content, images, tags, file_path, created_at, revision): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
        ),
    ) -> Memo {
        Memo {
            id,
            timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default(),
            date,
            content,
            images: serde_json::from_str(&images).unwrap_or_default(),
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            file_path,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default(),
            revision,
        }
    }
}

fn json(items: &[String]) -> Result<String, BrainError> {
    serde_json::to_string(items).map_err(|e| BrainError::Internal(e.to_string()))
}

fn queue_old_images(conn: &Connection, memo: &Memo) -> Result<(), BrainError> {
    for path in &memo.images {
        conn.execute(
            "INSERT OR IGNORE INTO timeline_image_gc(path) VALUES(?1)",
            [path],
        )?;
    }
    conn.execute("INSERT OR IGNORE INTO timeline_image_gc(path) SELECT path FROM timeline_images WHERE instr(?1,path)>0",[&memo.content])?;
    Ok(())
}

fn publish_references(conn: &Connection, memo: &Memo) -> Result<(), BrainError> {
    for path in &memo.images {
        conn.execute("UPDATE timeline_images SET pending=0 WHERE path=?1", [path])?;
        conn.execute("DELETE FROM timeline_image_gc WHERE path=?1", [path])?;
    }
    conn.execute(
        "UPDATE timeline_images SET pending=0 WHERE instr(?1,path)>0",
        [&memo.content],
    )?;
    Ok(())
}
