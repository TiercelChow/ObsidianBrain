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
        assert_eq!(count, 5);
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
