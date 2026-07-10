// DB functions naturally take many parameters and return complex rusqlite types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::BrainError;

/// SQLite metadata store with WAL mode and versioned migrations.
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

#[allow(dead_code)] // Internal helper for migrations
struct Migration {
    version: u32,
    description: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "code_repos + note_repo_links",
        sql: include_str!("../../migrations/001_code_repos.sql"),
    },
    Migration {
        version: 2,
        description: "radar_items",
        sql: include_str!("../../migrations/002_radar_items.sql"),
    },
    Migration {
        version: 3,
        description: "inspiration_history",
        sql: include_str!("../../migrations/003_inspiration.sql"),
    },
    Migration {
        version: 4,
        description: "timeline_events",
        sql: include_str!("../../migrations/004_timeline.sql"),
    },
    Migration {
        version: 5,
        description: "app_state",
        sql: include_str!("../../migrations/005_app_state.sql"),
    },
    Migration {
        version: 6,
        description: "inspiration_history (Phase 3)",
        sql: include_str!("../../migrations/006_inspiration_history.sql"),
    },
    Migration {
        version: 7,
        description: "radar_items (Phase 3)",
        sql: include_str!("../../migrations/007_radar_items.sql"),
    },
    Migration {
        version: 8,
        description: "memos (Time Machine)",
        sql: include_str!("../../migrations/008_memos.sql"),
    },
];

impl SqliteStore {
    /// Open (or create) the database at `db_path`, enable WAL, run pending migrations.
    pub fn new(db_path: &Path) -> Result<Self, BrainError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| BrainError::Internal(format!("SQLite 打开失败: {e}")))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| BrainError::Internal(format!("WAL 设置失败: {e}")))?;

        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .map_err(|e| BrainError::Internal(format!("busy_timeout 设置失败: {e}")))?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| BrainError::Internal(format!("foreign_keys 设置失败: {e}")))?;

        let store = SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .map_err(|e| BrainError::Internal(format!("迁移表创建失败: {e}")))?;

        let current_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let pending: Vec<&Migration> = MIGRATIONS
            .iter()
            .filter(|m| m.version > current_version)
            .collect();

        if pending.is_empty() {
            return Ok(());
        }

        // Wrap all pending migrations in a single transaction
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|e| BrainError::Internal(format!("迁移事务开始失败: {e}")))?;

        for migration in &pending {
            tracing::info!("执行迁移 v{}: {}", migration.version, migration.description);
            if let Err(e) = conn.execute_batch(migration.sql) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(BrainError::Internal(format!(
                    "迁移 v{} 执行失败: {e}",
                    migration.version
                )));
            }
            if let Err(e) = conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                params![migration.version, migration.description],
            ) {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(BrainError::Internal(format!(
                    "迁移 v{} 记录失败: {e}",
                    migration.version
                )));
            }
        }

        conn.execute_batch("COMMIT;")
            .map_err(|e| BrainError::Internal(format!("迁移事务提交失败: {e}")))?;

        Ok(())
    }

    /// Execute a closure inside a transaction.
    pub fn transaction<F, T>(&self, f: F) -> Result<T, BrainError>
    where
        F: FnOnce(&Connection) -> Result<T, BrainError>,
    {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN IMMEDIATE;")
            .map_err(|e| BrainError::Internal(format!("事务开始失败: {e}")))?;

        match f(&conn) {
            Ok(result) => {
                conn.execute_batch("COMMIT;")
                    .map_err(|e| BrainError::Internal(format!("事务提交失败: {e}")))?;
                Ok(result)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    }

    // ── App state helpers ──

    pub fn get_state(&self, key: &str) -> Result<Option<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM app_state WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("状态查询失败: {e}"))),
        }
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_state (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )
        .map_err(|e| BrainError::Internal(format!("状态写入失败: {e}")))?;
        Ok(())
    }

    /// Check if the store is healthy (can execute a simple query).
    pub fn health_check(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT 1", [], |_| Ok(true))
            .unwrap_or(false)
    }

    // ── Code Repos ──

    pub fn insert_code_repo(
        &self,
        name: &str,
        path: &str,
        metadata: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO code_repos (name, path, metadata) VALUES (?1, ?2, ?3)",
            params![name, path, metadata],
        )
        .map_err(|e| BrainError::Internal(format!("插入代码仓失败: {e}")))?;
        Ok(())
    }

    pub fn get_code_repo_by_name(
        &self,
        name: &str,
    ) -> Result<Option<(String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT name, path, metadata FROM code_repos WHERE name = ?1",
            params![name],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("查询代码仓失败: {e}"))),
        }
    }

    pub fn list_code_repos(&self) -> Result<Vec<(String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name, path, metadata FROM code_repos")
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询代码仓失败: {e}")))?;
        let mut repos = Vec::new();
        for row in rows {
            repos.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(repos)
    }

    pub fn delete_code_repo(&self, name: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_changed = conn
            .execute("DELETE FROM code_repos WHERE name = ?1", params![name])
            .map_err(|e| BrainError::Internal(format!("删除代码仓失败: {e}")))?;
        Ok(rows_changed > 0)
    }

    pub fn update_repo_metadata(&self, name: &str, metadata: &str) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE code_repos SET metadata = ?1 WHERE name = ?2",
            params![metadata, name],
        )
        .map_err(|e| BrainError::Internal(format!("更新代码仓元数据失败: {e}")))?;
        Ok(())
    }

    // ── Note-Repo Links ──

    pub fn insert_note_repo_link(
        &self,
        note_path: &str,
        repo_name: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO note_repo_links (note_path, repo_name) VALUES (?1, ?2)",
            params![note_path, repo_name],
        )
        .map_err(|e| BrainError::Internal(format!("插入笔记-仓库关联失败: {e}")))?;
        Ok(())
    }

    pub fn get_linked_notes(&self, repo_name: &str) -> Result<Vec<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT note_path FROM note_repo_links WHERE repo_name = ?1")
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![repo_name], |row| row.get::<_, String>(0))
            .map_err(|e| BrainError::Internal(format!("查询关联笔记失败: {e}")))?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(notes)
    }

    pub fn count_note_links(&self, repo_name: &str) -> Result<usize, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM note_repo_links WHERE repo_name = ?1",
                params![repo_name],
                |row| row.get(0),
            )
            .map_err(|e| BrainError::Internal(format!("统计关联笔记失败: {e}")))?;
        Ok(count)
    }

    // ── Timeline Events ──

    pub fn insert_timeline_event(
        &self,
        id: &str,
        date: &str,
        event_type: &str,
        title: &str,
        summary: &str,
        tags: &str,
        related_paths: &str,
        source: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO timeline_events (id, date, event_type, title, summary, tags, related_paths, source_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, date, event_type, title, summary, tags, related_paths, source],
        )
        .map_err(|e| BrainError::Internal(format!("插入时间线事件失败: {e}")))?;
        Ok(())
    }

    pub fn get_timeline_events(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        BrainError,
    > {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, date, event_type, title, summary, tags, related_paths, source_path
                 FROM timeline_events
                 WHERE date >= ?1 AND date <= ?2
                 ORDER BY date",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![start_date, end_date], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询时间线事件失败: {e}")))?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(events)
    }

    pub fn delete_timeline_events_before(&self, before_date: &str) -> Result<usize, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_deleted = conn
            .execute(
                "DELETE FROM timeline_events WHERE date < ?1",
                params![before_date],
            )
            .map_err(|e| BrainError::Internal(format!("删除时间线事件失败: {e}")))?;
        Ok(rows_deleted)
    }

    // ── Inspiration History ──

    pub fn insert_inspiration(
        &self,
        id: &str,
        insp_type: &str,
        input_refs: &str,
        output: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO inspiration_history (id, type, input_refs, output) VALUES (?1, ?2, ?3, ?4)",
            params![id, insp_type, input_refs, output],
        )
        .map_err(|e| BrainError::Internal(format!("插入灵感记录失败: {e}")))?;
        Ok(())
    }

    pub fn get_recent_inspirations(
        &self,
        insp_type: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, type, input_refs, output FROM inspiration_history
                 WHERE type = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![insp_type, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询灵感记录失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    // ── Radar Items ──

    pub fn insert_radar_item(
        &self,
        id: &str,
        title: &str,
        summary: &str,
        source_name: &str,
        url: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO radar_items (id, title, summary, source_name, url) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, summary, source_name, url],
        )
        .map_err(|e| BrainError::Internal(format!("插入雷达条目失败: {e}")))?;
        Ok(())
    }

    pub fn get_radar_items(
        &self,
        status: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, String, String, String)>, BrainError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, summary, source_name, url, status FROM radar_items
                 WHERE status = ?1 ORDER BY fetched_at DESC LIMIT ?2",
            )
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let rows = stmt
            .query_map(params![status, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询雷达条目失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    // ── Memos (Time Machine) ──

    pub fn insert_memo(
        &self,
        id: &str,
        timestamp: &str,
        date: &str,
        content: &str,
        images: &str,
        tags: &str,
        file_path: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memos (id, timestamp, date, content, images, tags, file_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, timestamp, date, content, images, tags, file_path],
        )
        .map_err(|e| BrainError::Internal(format!("插入小记失败: {e}")))?;
        Ok(())
    }

    /// Count total memos in the database.
    pub fn count_memos(&self) -> Result<u32, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM memos", [], |row| row.get(0))
            .map_err(|e| BrainError::Internal(format!("统计小记数量失败: {e}")))?;
        Ok(count)
    }

    /// Insert or update a memo (for sync from Obsidian files).
    pub fn upsert_memo(
        &self,
        id: &str,
        timestamp: &str,
        date: &str,
        content: &str,
        images: &str,
        tags: &str,
        file_path: &str,
    ) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memos (id, timestamp, date, content, images, tags, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               timestamp = excluded.timestamp,
               date = excluded.date,
               content = excluded.content,
               images = excluded.images,
               tags = excluded.tags,
               file_path = excluded.file_path",
            params![id, timestamp, date, content, images, tags, file_path],
        )
        .map_err(|e| BrainError::Internal(format!("同步小记失败: {e}")))?;
        Ok(())
    }

    /// Find an existing memo ID by timestamp (for dedup during sync).
    /// Compares only the date+time portion (first 19 chars: YYYY-MM-DDTHH:MM:SS)
    /// to handle different timezone offsets and microsecond precision.
    pub fn find_memo_id_by_timestamp(&self, timestamp: &str) -> Result<Option<String>, BrainError> {
        let conn = self.conn.lock().unwrap();
        // Normalize: take first 19 chars (YYYY-MM-DDTHH:MM:SS)
        let normalized = if timestamp.len() >= 19 {
            &timestamp[..19]
        } else {
            timestamp
        };
        let pattern = format!("{}%", normalized);
        let result = conn.query_row(
            "SELECT id FROM memos WHERE timestamp LIKE ?1 LIMIT 1",
            params![pattern],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(BrainError::Internal(format!("查询小记 ID 失败: {e}"))),
        }
    }

    /// Delete memos whose date is in synced_dates but whose ID is NOT in keep_ids.
    /// Used during sync to remove memos deleted from Obsidian.
    pub fn delete_memos_not_by_ids(
        &self,
        synced_dates: &std::collections::HashSet<String>,
        keep_ids: &[String],
    ) -> Result<u32, BrainError> {
        if synced_dates.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();

        // Build date IN clause
        let date_placeholders: Vec<String> = synced_dates
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let date_in = date_placeholders.join(", ");

        // Build ID NOT IN clause
        let id_placeholders: Vec<String> = keep_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + synced_dates.len() + 1))
            .collect();
        let id_not_in = if id_placeholders.is_empty() {
            "''".to_string()
        } else {
            id_placeholders.join(", ")
        };

        let sql = format!(
            "DELETE FROM memos WHERE date IN ({}) AND id NOT IN ({})",
            date_in, id_not_in
        );

        // Collect all params: dates first, then IDs
        let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for d in synced_dates {
            all_params.push(Box::new(d.clone()));
        }
        for id in keep_ids {
            all_params.push(Box::new(id.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        let deleted = conn
            .execute(&sql, param_refs.as_slice())
            .map_err(|e| BrainError::Internal(format!("删除过期小记失败: {e}")))?;

        Ok(deleted as u32)
    }

    pub fn query_memos(
        &self,
        sql: &str,
        params: &[String],
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )>,
        BrainError,
    > {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| BrainError::Internal(format!("准备查询失败: {e}")))?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| BrainError::Internal(format!("查询小记失败: {e}")))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| BrainError::Internal(format!("读取行失败: {e}")))?);
        }
        Ok(results)
    }

    pub fn update_radar_status(&self, id: &str, status: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let rows_changed = conn
            .execute(
                "UPDATE radar_items SET status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| BrainError::Internal(format!("更新雷达状态失败: {e}")))?;
        Ok(rows_changed > 0)
    }

    pub fn radar_url_exists(&self, url: &str) -> Result<bool, BrainError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM radar_items WHERE url = ?1",
                params![url],
                |row| row.get(0),
            )
            .map_err(|e| BrainError::Internal(format!("查询雷达URL失败: {e}")))?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_creates_db_and_runs_migrations() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        let conn = store.conn.lock().unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 8);
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");

        let _store1 = SqliteStore::new(&db_path).unwrap();
        let store2 = SqliteStore::new(&db_path).unwrap();
        assert!(store2.health_check());
    }

    #[test]
    fn test_app_state_crud() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        assert_eq!(store.get_state("missing").unwrap(), None);

        store.set_state("test_key", "hello").unwrap();
        assert_eq!(
            store.get_state("test_key").unwrap(),
            Some("hello".to_string())
        );

        store.set_state("test_key", "world").unwrap();
        assert_eq!(
            store.get_state("test_key").unwrap(),
            Some("world".to_string())
        );
    }

    #[test]
    fn test_tables_exist() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();

        for table in &[
            "code_repos",
            "note_repo_links",
            "radar_items",
            "inspiration_history",
            "timeline_events",
            "app_state",
            "memos",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "Table {table} should exist");
        }
    }
}
