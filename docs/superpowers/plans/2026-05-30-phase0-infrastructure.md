# Phase 0: Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build all infrastructure modules (config, SQLite, file watcher, Qdrant, embedding, LLM, Tantivy) and integrate them into AppContext so the backend boots with full component initialization.

**Architecture:** Layered infrastructure under `src/infra/`, each module behind a trait or struct with clear boundaries. All modules depend on `error.rs` (BrainError) and `config.rs` (AppConfig sub-configs). The `main.rs` orchestrates initialization and injects everything into `Arc<AppContext>`.

**Tech Stack:** Rust 2021, Axum 0.7, Tokio, config 0.14, rusqlite 0.31 (bundled), notify 6, reqwest 0.12, tantivy 0.22, tantivy-jieba 0.2, async-trait 0.1.

**Reference docs:** Read `docs/development/01-infrastructure.md` before each task for design rationale.

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add Phase 0 dependencies |
| Modify | `src/config.rs` | TOML parsing + validation |
| Modify | `src/error.rs` | Add `From<rusqlite::Error>`, `From<reqwest::Error>` |
| Modify | `src/main.rs` | Initialize all infra, build full AppContext |
| Modify | `src/infra/mod.rs` | Export all infra sub-modules |
| Modify | `src/api/handlers/health.rs` | Return real component status |
| Create | `src/infra/sqlite_store.rs` | SQLite connection + migrations + CRUD |
| Create | `src/infra/file_watcher.rs` | notify + debounce + event channel |
| Create | `src/infra/qdrant_client.rs` | Qdrant REST API client |
| Create | `src/infra/embedding.rs` | EmbeddingProvider trait + OpenAI/Ollama |
| Create | `src/infra/llm_client.rs` | LlmProvider trait + OpenAI/Ollama |
| Create | `src/infra/tantivy_index.rs` | Tantivy schema + jieba + search |
| Create | `migrations/001_code_repos.sql` | code_repos + note_repo_links tables |
| Create | `migrations/002_radar_items.sql` | radar_items table |
| Create | `migrations/003_inspiration.sql` | inspiration_history table |
| Create | `migrations/004_timeline.sql` | timeline_events table |
| Create | `migrations/005_app_state.sql` | app_state table |
| Delete | `backend/migrations/.gitkeep` | Replaced by real SQL files |

---

### Task 1: Add Dependencies + Config System

**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `backend/src/config.rs`
- Modify: `backend/src/error.rs`

- [ ] **Step 1: Add Phase 0 dependencies to Cargo.toml**

Uncomment and add these lines in the `[dependencies]` section of `backend/Cargo.toml`:

```toml
# Phase 0 dependencies
config = "0.14"
rusqlite = { version = "0.31", features = ["bundled"] }
notify = "6"
reqwest = { version = "0.12", features = ["json", "stream"] }
tantivy = "0.22"
tantivy-jieba = "0.2"
async-trait = "0.1"
futures = "0.3"
```

- [ ] **Step 2: Run `cargo check` to verify deps resolve**

Run: `cd backend && cargo check 2>&1`
Expected: Compiles with unused dependency warnings (that's fine — we'll use them in subsequent tasks).

- [ ] **Step 3: Rewrite config.rs with TOML parsing and validation**

Replace the contents of `backend/src/config.rs` with:

```rust
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::BrainError;

// ── Top-level ──

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub qdrant: QdrantConfig,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl AppConfig {
    /// Load from config/default.toml, then local override, then env vars.
    pub fn load() -> Result<Self, BrainError> {
        let builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(
                Environment::with_prefix("OBRAIN")
                    .separator("__")
                    .try_parsing(true),
            );

        let config = builder
            .build()
            .map_err(|e| BrainError::ConfigError(format!("配置加载失败: {e}")))?;

        let app_config: AppConfig = config
            .try_deserialize()
            .map_err(|e| BrainError::ConfigError(format!("配置解析失败: {e}")))?;

        app_config.validate()?;
        Ok(app_config)
    }

    pub fn validate(&self) -> Result<(), BrainError> {
        if self.server.port < 1024 {
            return Err(BrainError::ConfigError(format!(
                "端口号不能低于 1024: {}",
                self.server.port
            )));
        }
        if self.memory.chunk_min_tokens >= self.memory.chunk_max_tokens {
            return Err(BrainError::ConfigError(
                "chunk_min_tokens 必须小于 chunk_max_tokens".to_string(),
            ));
        }
        if self.qdrant.vector_size == 0 {
            return Err(BrainError::ConfigError(
                "向量维度不能为 0".to_string(),
            ));
        }
        Ok(())
    }
}

// ── Sub-configs ──

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default = "default_vault_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub watch_enabled: bool,
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            name: default_vault_name(),
            watch_enabled: true,
            exclude_patterns: default_exclude_patterns(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default = "default_collection_name")]
    pub collection_name: String,
    #[serde(default = "default_vector_size")]
    pub vector_size: usize,
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            collection_name: default_collection_name(),
            vector_size: default_vector_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_openai")]
    pub provider: String,
    #[serde(default = "default_embedding_model")]
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_openai(),
            model: default_embedding_model(),
            api_key_env: None,
            base_url: None,
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    #[serde(default = "default_openai")]
    pub provider: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_openai(),
            model: default_llm_model(),
            api_key_env: None,
            base_url: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryConfig {
    #[serde(default = "default_chunk_min")]
    pub chunk_min_tokens: usize,
    #[serde(default = "default_chunk_max")]
    pub chunk_max_tokens: usize,
    #[serde(default = "default_top_k")]
    pub search_top_k: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chunk_min_tokens: default_chunk_min(),
            chunk_max_tokens: default_chunk_max(),
            search_top_k: default_top_k(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_index_path")]
    pub index_path: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            index_path: default_index_path(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
        }
    }
}

// ── Default value functions ──

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 9876 }
fn default_vault_name() -> String { "brain".to_string() }
fn default_true() -> bool { true }
fn default_exclude_patterns() -> Vec<String> {
    vec![
        ".obsidian/".to_string(),
        "templates/".to_string(),
        ".trash/".to_string(),
    ]
}
fn default_qdrant_url() -> String { "http://127.0.0.1:6333".to_string() }
fn default_collection_name() -> String { "obsidian_brain".to_string() }
fn default_vector_size() -> usize { 1536 }
fn default_openai() -> String { "openai".to_string() }
fn default_embedding_model() -> String { "text-embedding-3-small".to_string() }
fn default_llm_model() -> String { "gpt-4o-mini".to_string() }
fn default_max_tokens() -> u32 { 2048 }
fn default_temperature() -> f64 { 0.7 }
fn default_batch_size() -> usize { 100 }
fn default_chunk_min() -> usize { 300 }
fn default_chunk_max() -> usize { 800 }
fn default_top_k() -> usize { 5 }
fn default_db_path() -> PathBuf { PathBuf::from("./data/brain.db") }
fn default_index_path() -> PathBuf { PathBuf::from("./data/tantivy_index") }
fn default_log_level() -> String { "info".to_string() }
```

- [ ] **Step 4: Add From impls to error.rs**

Add these to the end of `backend/src/error.rs` (before the closing `}` of `impl IntoResponse`):

```rust
impl From<rusqlite::Error> for BrainError {
    fn from(e: rusqlite::Error) -> Self {
        BrainError::Internal(format!("SQLite 错误: {e}"))
    }
}

impl From<reqwest::Error> for BrainError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            BrainError::Internal(format!("HTTP 超时: {e}"))
        } else if e.is_connect() {
            BrainError::Internal(format!("连接失败: {e}"))
        } else {
            BrainError::Internal(format!("HTTP 错误: {e}"))
        }
    }
}
```

- [ ] **Step 5: Update main.rs load call to handle Result**

In `backend/src/main.rs`, change the config loading line from:

```rust
let config = AppConfig::load();
```

to:

```rust
let config = AppConfig::load().unwrap_or_else(|e| {
    tracing::warn!("配置加载失败: {e}，使用默认配置");
    AppConfig::default()
});
```

Also add `impl Default for AppConfig` at the end of `config.rs`:

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            vault: VaultConfig::default(),
            qdrant: QdrantConfig::default(),
            embedding: EmbeddingConfig::default(),
            llm: LlmConfig::default(),
            memory: MemoryConfig::default(),
            storage: StorageConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}
```

- [ ] **Step 6: Write a config parsing test**

Add to the bottom of `backend/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_port_rejected() {
        let mut config = AppConfig::default();
        config.server.port = 80;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_chunk_params_rejected_when_min_ge_max() {
        let mut config = AppConfig::default();
        config.memory.chunk_min_tokens = 1000;
        config.memory.chunk_max_tokens = 500;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_falls_back_to_defaults() {
        // No config file in test env — should still work
        let config = AppConfig::load();
        assert!(config.is_ok());
    }
}
```

- [ ] **Step 7: Run tests**

Run: `cd backend && cargo test --lib config::tests -- --nocapture`
Expected: All 4 tests pass.

- [ ] **Step 8: Verify clippy**

Run: `cd backend && cargo clippy -- -D warnings`
Expected: Zero warnings. Fix any issues.

- [ ] **Step 9: Commit**

```bash
git add backend/Cargo.toml backend/src/config.rs backend/src/error.rs backend/src/main.rs
git commit -m "feat(infra): implement config system with TOML parsing and validation"
```

---

### Task 2: SQLite Metadata Store

**Files:**
- Create: `backend/src/infra/sqlite_store.rs`
- Create: `backend/migrations/001_code_repos.sql`
- Create: `backend/migrations/002_radar_items.sql`
- Create: `backend/migrations/003_inspiration.sql`
- Create: `backend/migrations/004_timeline.sql`
- Create: `backend/migrations/005_app_state.sql`
- Delete: `backend/migrations/.gitkeep`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Create migration SQL files**

Create `backend/migrations/001_code_repos.sql`:

```sql
CREATE TABLE IF NOT EXISTS code_repos (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    registered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata    TEXT
);

CREATE TABLE IF NOT EXISTS note_repo_links (
    note_path   TEXT NOT NULL,
    repo_name   TEXT NOT NULL REFERENCES code_repos(name) ON DELETE CASCADE,
    linked_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (note_path, repo_name)
);

CREATE INDEX IF NOT EXISTS idx_note_repo_links_repo ON note_repo_links(repo_name);
```

Create `backend/migrations/002_radar_items.sql`:

```sql
CREATE TABLE IF NOT EXISTS radar_items (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    summary     TEXT,
    source      TEXT NOT NULL,
    url         TEXT NOT NULL UNIQUE,
    embedding_id TEXT,
    status      TEXT DEFAULT 'new' CHECK(status IN ('new','read','saved','dismissed')),
    relevance_score REAL,
    related_notes TEXT,
    fetched_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    published_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_radar_items_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_items_score ON radar_items(relevance_score DESC);
CREATE INDEX IF NOT EXISTS idx_radar_items_source ON radar_items(source);
```

Create `backend/migrations/003_inspiration.sql`:

```sql
CREATE TABLE IF NOT EXISTS inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL CHECK(type IN ('concept_combo','reverse_question','counterpoint')),
    input_refs  TEXT,
    output      TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_inspiration_type ON inspiration_history(type);
CREATE INDEX IF NOT EXISTS idx_inspiration_created ON inspiration_history(created_at DESC);
```

Create `backend/migrations/004_timeline.sql`:

```sql
CREATE TABLE IF NOT EXISTS timeline_events (
    id          TEXT PRIMARY KEY,
    date        TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    title       TEXT NOT NULL,
    summary     TEXT,
    tags        TEXT,
    related_paths TEXT,
    source_path TEXT,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_timeline_date ON timeline_events(date);
CREATE INDEX IF NOT EXISTS idx_timeline_type ON timeline_events(event_type);
CREATE INDEX IF NOT EXISTS idx_timeline_date_type ON timeline_events(date, event_type);
```

Create `backend/migrations/005_app_state.sql`:

```sql
CREATE TABLE IF NOT EXISTS app_state (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

Delete `backend/migrations/.gitkeep`.

- [ ] **Step 2: Write the failing test for SqliteStore**

Create `backend/src/infra/sqlite_store.rs`:

```rust
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::BrainError;

/// SQLite metadata store with WAL mode and versioned migrations.
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

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

        for migration in MIGRATIONS {
            if migration.version > current_version {
                tracing::info!(
                    "执行迁移 v{}: {}",
                    migration.version,
                    migration.description
                );
                conn.execute_batch(migration.sql)
                    .map_err(|e| {
                        BrainError::Internal(format!(
                            "迁移 v{} 执行失败: {e}",
                            migration.version
                        ))
                    })?;
                conn.execute(
                    "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                    params![migration.version, migration.description],
                )
                .map_err(|e| {
                    BrainError::Internal(format!(
                        "迁移 v{} 记录失败: {e}",
                        migration.version
                    ))
                })?;
            }
        }

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
        conn.execute_batch("SELECT 1;").is_ok()
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

        // All 5 migrations should have run
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

        // First open: runs migrations
        let _store1 = SqliteStore::new(&db_path).unwrap();
        // Second open: skips all (already applied)
        let store2 = SqliteStore::new(&db_path).unwrap();
        assert!(store2.health_check());
    }

    #[test]
    fn test_app_state_crud() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        // Read missing key
        assert_eq!(store.get_state("missing").unwrap(), None);

        // Write + read
        store.set_state("test_key", "hello").unwrap();
        assert_eq!(
            store.get_state("test_key").unwrap(),
            Some("hello".to_string())
        );

        // Overwrite
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
```

- [ ] **Step 3: Register module in infra/mod.rs**

Replace `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
// pub mod file_watcher;   // Task 3
// pub mod qdrant_client;  // Task 4
// pub mod embedding;      // Task 5
// pub mod llm_client;     // Task 6
// pub mod tantivy_index;  // Task 7
```

- [ ] **Step 4: Run tests**

Run: `cd backend && cargo test --lib infra::sqlite_store::tests -- --nocapture`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/src/infra/ backend/migrations/
git commit -m "feat(infra): add SQLite store with WAL mode and versioned migrations"
```

---

### Task 3: File Watcher

**Files:**
- Create: `backend/src/infra/file_watcher.rs`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Write FileWatcher with tests**

Create `backend/src/infra/file_watcher.rs`:

```rust
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::error::BrainError;

/// Type of filesystem change.
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

/// A single file change event.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub change_type: FileChangeType,
    pub path: PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

struct PendingEvent {
    event_type: FileChangeType,
    last_seen: Instant,
}

/// Watches a directory for `.md` file changes, debounces, and sends events.
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    pub rx: Arc<Mutex<Option<mpsc::Receiver<FileChangeEvent>>>>,
}

impl FileWatcher {
    /// Start watching `vault_path`. Events arrive on the returned receiver.
    /// Only `.md` files are tracked. Paths matching any `exclude_patterns` substring are skipped.
    pub fn new(
        vault_path: &Path,
        exclude_patterns: Vec<String>,
        debounce_ms: u64,
    ) -> Result<Self, BrainError> {
        let (tx, rx) = mpsc::channel::<FileChangeEvent>(1024);

        let pending: Arc<Mutex<HashMap<PathBuf, PendingEvent>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn flush loop
        let pending_clone = pending.clone();
        let tx_flush = tx.clone();
        let debounce_dur = Duration::from_millis(debounce_ms);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let now = Instant::now();
                let mut to_flush = Vec::new();

                {
                    let mut map = pending_clone.lock().unwrap();
                    map.retain(|path, event| {
                        if now.duration_since(event.last_seen) >= debounce_dur {
                            to_flush.push(FileChangeEvent {
                                change_type: event.event_type.clone(),
                                path: path.clone(),
                                timestamp: chrono::Utc::now(),
                            });
                            false
                        } else {
                            true
                        }
                    });
                }

                for ev in to_flush {
                    if tx_flush.send(ev).await.is_err() {
                        return; // receiver dropped
                    }
                }
            }
        });

        let pending_cb = pending.clone();
        let exclude = exclude_patterns.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in &event.paths {
                    let path_str = path.to_string_lossy();

                    // Skip excluded
                    if exclude.iter().any(|p| path_str.contains(p.as_str())) {
                        continue;
                    }

                    // Only .md files
                    if path.extension().map(|e| e != "md").unwrap_or(true) {
                        continue;
                    }

                    let change_type = match event.kind {
                        EventKind::Create(_) => FileChangeType::Created,
                        EventKind::Modify(_) => FileChangeType::Modified,
                        EventKind::Remove(_) => FileChangeType::Deleted,
                        _ => continue,
                    };

                    let mut map = pending_cb.lock().unwrap();
                    let now = Instant::now();
                    map.entry(path.clone())
                        .and_modify(|e| {
                            e.last_seen = now;
                            if matches!(change_type, FileChangeType::Deleted) {
                                e.event_type = FileChangeType::Deleted;
                            }
                        })
                        .or_insert(PendingEvent {
                            event_type: change_type,
                            last_seen: now,
                        });
                }
            }
        })
        .map_err(|e| BrainError::Internal(format!("文件监控初始化失败: {e}")))?;

        watcher
            .watch(vault_path, RecursiveMode::Recursive)
            .map_err(|e| BrainError::Internal(format!("Vault 监控启动失败: {e}")))?;

        tracing::info!("文件监控启动: {:?}", vault_path);

        Ok(FileWatcher {
            _watcher: watcher,
            rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    /// Take the receiver out (can only be called once).
    pub fn take_receiver(&self) -> Option<mpsc::Receiver<FileChangeEvent>> {
        self.rx.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_watcher_detects_md_creation() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(
            dir.path(),
            vec![],
            100, // short debounce for testing
        )
        .unwrap();

        let mut rx = watcher.take_receiver().unwrap();

        // Create a .md file
        std::fs::write(dir.path().join("test.md"), "# Hello").unwrap();

        // Wait for debounce + flush
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timed out waiting for event")
            .expect("Channel closed");

        assert_eq!(event.change_type, FileChangeType::Created);
        assert!(event.path.to_string_lossy().contains("test.md"));
    }

    #[tokio::test]
    async fn test_file_watcher_ignores_non_md() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(dir.path(), vec![], 100).unwrap();
        let mut rx = watcher.take_receiver().unwrap();

        // Create a .txt file — should be ignored
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "Should not receive event for .txt file");
    }

    #[tokio::test]
    async fn test_file_watcher_excludes_patterns() {
        let dir = TempDir::new().unwrap();
        let trash = dir.path().join(".trash");
        std::fs::create_dir_all(&trash).unwrap();

        let watcher = FileWatcher::new(
            dir.path(),
            vec![".trash/".to_string()],
            100,
        )
        .unwrap();
        let mut rx = watcher.take_receiver().unwrap();

        // Create .md in excluded dir
        std::fs::write(trash.join("deleted.md"), "gone").unwrap();

        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        assert!(result.is_err(), "Should not receive event for excluded path");
    }
}
```

- [ ] **Step 2: Register module**

Update `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
pub mod file_watcher;
// pub mod qdrant_client;  // Task 4
// pub mod embedding;      // Task 5
// pub mod llm_client;     // Task 6
// pub mod tantivy_index;  // Task 7
```

- [ ] **Step 3: Run tests**

Run: `cd backend && cargo test --lib infra::file_watcher::tests -- --nocapture`
Expected: All 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/src/infra/file_watcher.rs backend/src/infra/mod.rs
git commit -m "feat(infra): add file watcher with debounce and event filtering"
```

---

### Task 4: Qdrant Client

**Files:**
- Create: `backend/src/infra/qdrant_client.rs`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Write QdrantStore with tests**

Create `backend/src/infra/qdrant_client.rs`:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::config::QdrantConfig;
use crate::error::BrainError;

/// Qdrant vector store client (REST API).
pub struct QdrantStore {
    client: Client,
    base_url: String,
    collection_name: String,
    vector_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Value,
}

/// Payload stored alongside each vector in Qdrant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    pub note_path: String,
    pub chunk_index: usize,
    pub content: String,
    pub title: String,
    pub tags: Vec<String>,
    pub heading_path: Vec<String>,
    pub word_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

impl QdrantStore {
    pub fn new(config: &QdrantConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BrainError::QdrantError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(QdrantStore {
            client,
            base_url: config.url.clone(),
            collection_name: config.collection_name.clone(),
            vector_size: config.vector_size,
        })
    }

    /// Check if Qdrant is reachable.
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/healthz", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Create the collection if it does not exist.
    pub async fn ensure_collection(&self) -> Result<(), BrainError> {
        let resp = self
            .client
            .get(format!(
                "{}/collections/{}",
                self.base_url, self.collection_name
            ))
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("查询 collection 失败: {e}")))?;

        if resp.status().is_success() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct CreateReq {
            vectors: VectorParams,
            hnsw_config: HnswConfig,
        }
        #[derive(Serialize)]
        struct VectorParams {
            size: usize,
            distance: String,
        }
        #[derive(Serialize)]
        struct HnswConfig {
            m: usize,
            ef_construct: usize,
        }

        let body = CreateReq {
            vectors: VectorParams {
                size: self.vector_size,
                distance: "Cosine".to_string(),
            },
            hnsw_config: HnswConfig {
                m: 16,
                ef_construct: 200,
            },
        };

        self.client
            .put(format!(
                "{}/collections/{}",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("创建 collection 失败: {e}")))?;

        tracing::info!("Qdrant collection 创建: {}", self.collection_name);
        Ok(())
    }

    /// Insert or update vector points.
    pub async fn upsert_points(&self, points: Vec<VectorPoint>) -> Result<(), BrainError> {
        if points.is_empty() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct UpsertReq {
            points: Vec<PointData>,
        }
        #[derive(Serialize)]
        struct PointData {
            id: String,
            vector: Vec<f32>,
            payload: Value,
        }

        let body = UpsertReq {
            points: points
                .into_iter()
                .map(|p| PointData {
                    id: p.id,
                    vector: p.vector,
                    payload: p.payload,
                })
                .collect(),
        };

        self.client
            .put(format!(
                "{}/collections/{}/points",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("Upsert 失败: {e}")))?;

        Ok(())
    }

    /// Search by vector.
    pub async fn search(
        &self,
        query_vector: &[f32],
        top_k: usize,
        filter: Option<Value>,
    ) -> Result<Vec<SearchResult>, BrainError> {
        #[derive(Serialize)]
        struct SearchReq {
            vector: Vec<f32>,
            limit: usize,
            with_payload: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            filter: Option<Value>,
        }

        let body = SearchReq {
            vector: query_vector.to_vec(),
            limit: top_k,
            with_payload: true,
            filter,
        };

        let resp = self
            .client
            .post(format!(
                "{}/collections/{}/points/search",
                self.base_url, self.collection_name
            ))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("搜索失败: {e}")))?;

        #[derive(Deserialize)]
        struct SearchResp {
            result: Vec<SearchResultItem>,
        }
        #[derive(Deserialize)]
        struct SearchResultItem {
            id: String,
            score: f32,
            payload: Value,
        }

        let search_resp: SearchResp = resp
            .json()
            .await
            .map_err(|e| BrainError::QdrantError(format!("响应解析失败: {e}")))?;

        Ok(search_resp
            .result
            .into_iter()
            .map(|r| SearchResult {
                id: r.id,
                score: r.score,
                payload: r.payload,
            })
            .collect())
    }

    /// Delete points by ID.
    pub async fn delete_points(&self, ids: &[String]) -> Result<(), BrainError> {
        if ids.is_empty() {
            return Ok(());
        }

        #[derive(Serialize)]
        struct DeleteReq {
            points: Vec<String>,
        }

        self.client
            .post(format!(
                "{}/collections/{}/points/delete",
                self.base_url, self.collection_name
            ))
            .json(&DeleteReq {
                points: ids.to_vec(),
            })
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("删除失败: {e}")))?;

        Ok(())
    }
}
```

- [ ] **Step 2: Register module**

Update `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
pub mod file_watcher;
pub mod qdrant_client;
// pub mod embedding;      // Task 5
// pub mod llm_client;     // Task 6
// pub mod tantivy_index;  // Task 7
```

- [ ] **Step 3: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compiles clean.

- [ ] **Step 4: Commit**

```bash
git add backend/src/infra/qdrant_client.rs backend/src/infra/mod.rs
git commit -m "feat(infra): add Qdrant vector store client via REST API"
```

---

### Task 5: Embedding Provider

**Files:**
- Create: `backend/src/infra/embedding.rs`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Write EmbeddingProvider trait + OpenAI/Ollama implementations**

Create `backend/src/infra/embedding.rs`:

```rust
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::EmbeddingConfig;
use crate::error::BrainError;

/// Unified interface for embedding text into vectors.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError>;
    fn dimensions(&self) -> usize;
}

// ── OpenAI ──

pub struct OpenAiEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    base_url: String,
    batch_size: usize,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

impl OpenAiEmbedder {
    pub fn new(config: &EmbeddingConfig) -> Result<Self, BrainError> {
        let api_key = config
            .api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .unwrap_or_default();

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| BrainError::EmbeddingError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OpenAiEmbedder {
            client,
            api_key,
            model: config.model.clone(),
            dimensions: 1536,
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            batch_size: config.batch_size,
        })
    }

    async fn embed_batch_inner(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, BrainError> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
        };

        let mut last_err = None;
        for attempt in 0..3 {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }

            let resp = self
                .client
                .post(format!("{}/embeddings", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let body: EmbeddingResponse = r.json().await.map_err(|e| {
                        BrainError::EmbeddingError(format!("响应解析失败: {e}"))
                    })?;
                    let mut data = body.data;
                    data.sort_by_key(|d| d.index);
                    return Ok(data.into_iter().map(|d| d.embedding).collect());
                }
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    last_err = Some(BrainError::EmbeddingError(format!(
                        "API 错误 {status}: {text}"
                    )));
                }
                Err(e) => {
                    last_err = Some(BrainError::EmbeddingError(format!(
                        "请求失败: {e}"
                    )));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            BrainError::EmbeddingError("重试耗尽".to_string())
        }))
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let results = self.embed_batch_inner(&[text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| BrainError::EmbeddingError("空响应".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.batch_size) {
            let batch = self.embed_batch_inner(chunk).await?;
            results.extend(batch);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

// ── Ollama ──

pub struct OllamaEmbedder {
    client: Client,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    pub fn new(config: &EmbeddingConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BrainError::EmbeddingError(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OllamaEmbedder {
            client,
            model: config.model.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string()),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let request = OllamaEmbedRequest {
            model: self.model.clone(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/api/embed", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::EmbeddingError(format!("Ollama 请求失败: {e}")))?;

        let resp: OllamaEmbedResponse = response.json().await.map_err(|e| {
            BrainError::EmbeddingError(format!("Ollama 响应解析失败: {e}"))
        })?;

        resp.embeddings
            .into_iter()
            .next()
            .ok_or_else(|| BrainError::EmbeddingError("Ollama 空响应".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        768
    }
}

// ── Factory ──

pub struct EmbeddingFactory;

impl EmbeddingFactory {
    pub fn create(config: &EmbeddingConfig) -> Result<Box<dyn EmbeddingProvider>, BrainError> {
        match config.provider.as_str() {
            "openai" => Ok(Box::new(OpenAiEmbedder::new(config)?)),
            "ollama" => Ok(Box::new(OllamaEmbedder::new(config)?)),
            other => Err(BrainError::ConfigError(format!(
                "未知的 Embedding provider: {other}"
            ))),
        }
    }
}
```

- [ ] **Step 2: Register module and add async-trait import**

Update `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
pub mod file_watcher;
pub mod qdrant_client;
pub mod embedding;
// pub mod llm_client;     // Task 6
// pub mod tantivy_index;  // Task 7
```

- [ ] **Step 3: Verify compilation**

Run: `cd backend && cargo check`
Expected: Compiles clean.

- [ ] **Step 4: Commit**

```bash
git add backend/src/infra/embedding.rs backend/src/infra/mod.rs
git commit -m "feat(infra): add EmbeddingProvider trait with OpenAI and Ollama implementations"
```

---

### Task 6: LLM Client

**Files:**
- Create: `backend/src/infra/llm_client.rs`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Write LlmProvider trait + OpenAI/Ollama implementations**

Create `backend/src/infra/llm_client.rs`:

```rust
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::LlmConfig;
use crate::error::BrainError;

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Complete chat response.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// A single chunk from a streaming response.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_final: bool,
}

/// Unified LLM interface.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError>;

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError>;

    /// Convenience: single user message → text.
    async fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: prompt.to_string(),
        }];
        let resp = self.chat(&messages).await?;
        Ok(resp.content)
    }

    /// Rough token count estimate.
    fn estimate_tokens(&self, text: &str) -> u32 {
        let char_count = text.chars().count();
        let cjk_count = text
            .chars()
            .filter(|c| *c as u32 > 0x4E00 && *c as u32 < 0x9FFF)
            .count();
        let non_cjk = char_count - cjk_count;
        ((non_cjk as f64 / 4.0) + (cjk_count as f64 / 2.0)).ceil() as u32
    }
}

// ── OpenAI ──

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f64,
    base_url: String,
}

#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f64,
    stream: bool,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
    model: String,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageResp,
}

#[derive(Deserialize)]
struct OpenAiMessageResp {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

fn role_to_string(role: &MessageRole) -> String {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    }
    .to_string()
}

impl OpenAiProvider {
    pub fn new(config: &LlmConfig) -> Result<Self, BrainError> {
        let api_key = config
            .api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .unwrap_or_default();

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| {
                BrainError::LlmApiError {
                    provider: "openai".to_string(),
                    detail: format!("HTTP 客户端创建失败: {e}"),
                }
            })?;

        Ok(OpenAiProvider {
            client,
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("请求失败: {e}"),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("API 错误 {status}: {body}"),
            });
        }

        let resp: OpenAiChatResponse = response.json().await.map_err(|e| {
            BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("响应解析失败: {e}"),
            }
        })?;

        let choice = resp.choices.into_iter().next().ok_or_else(|| {
            BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: "空响应".to_string(),
            }
        })?;

        Ok(ChatResponse {
            content: choice.message.content,
            model: resp.model,
            usage: TokenUsage {
                prompt_tokens: resp.usage.prompt_tokens,
                completion_tokens: resp.usage.completion_tokens,
                total_tokens: resp.usage.total_tokens,
            },
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        let (tx, rx) = mpsc::channel(64);

        let request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: true,
        };

        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let resp = client
                .post(format!("{}/chat/completions", base_url))
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request)
                .send()
                .await;

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx
                        .send(StreamChunk {
                            content: format!("错误: {e}"),
                            is_final: true,
                        })
                        .await;
                    return;
                }
            };

            let mut stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    while let Some(line_end) = buffer.find('\n') {
                        let line = buffer[..line_end].trim().to_string();
                        buffer = buffer[line_end + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                let _ = tx
                                    .send(StreamChunk {
                                        content: String::new(),
                                        is_final: true,
                                    })
                                    .await;
                                return;
                            }

                            if let Ok(json) =
                                serde_json::from_str::<serde_json::Value>(data)
                            {
                                if let Some(content) =
                                    json["choices"][0]["delta"]["content"].as_str()
                                {
                                    let _ = tx
                                        .send(StreamChunk {
                                            content: content.to_string(),
                                            is_final: false,
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

// ── Ollama ──

pub struct OllamaProvider {
    client: Client,
    model: String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(config: &LlmConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| BrainError::LlmApiError {
                provider: "ollama".to_string(),
                detail: format!("HTTP 客户端创建失败: {e}"),
            })?;

        Ok(OllamaProvider {
            client,
            model: config.model.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434".to_string()),
        })
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
        #[derive(Serialize)]
        struct Req {
            model: String,
            messages: Vec<OpenAiMessage>,
            stream: bool,
        }

        let request = Req {
            model: self.model.clone(),
            messages: messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                })
                .collect(),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: "ollama".to_string(),
                detail: format!("请求失败: {e}"),
            })?;

        #[derive(Deserialize)]
        struct Resp {
            message: OpenAiMessageResp,
            model: String,
        }

        let resp: Resp = response.json().await.map_err(|e| BrainError::LlmApiError {
            provider: "ollama".to_string(),
            detail: format!("响应解析失败: {e}"),
        })?;

        Ok(ChatResponse {
            content: resp.message.content,
            model: resp.model,
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        // Simplified: get full response then send as single chunk
        let (tx, rx) = mpsc::channel(4);
        let response = self.chat(messages).await?;
        let _ = tx
            .send(StreamChunk {
                content: response.content,
                is_final: true,
            })
            .await;
        Ok(rx)
    }
}

// ── Factory ──

pub struct LlmClientFactory;

impl LlmClientFactory {
    pub fn create(config: &LlmConfig) -> Result<Box<dyn LlmProvider>, BrainError> {
        match config.provider.as_str() {
            "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),
            "ollama" => Ok(Box::new(OllamaProvider::new(config)?)),
            other => Err(BrainError::ConfigError(format!(
                "未知的 LLM provider: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_english() {
        let provider = OpenAiProvider {
            client: Client::new(),
            api_key: String::new(),
            model: String::new(),
            max_tokens: 0,
            temperature: 0.0,
            base_url: String::new(),
        };
        // ~4 chars per token for English
        let estimate = provider.estimate_tokens("hello world this is a test");
        assert!(estimate > 3 && estimate < 10);
    }

    #[test]
    fn test_estimate_tokens_chinese() {
        let provider = OpenAiProvider {
            client: Client::new(),
            api_key: String::new(),
            model: String::new(),
            max_tokens: 0,
            temperature: 0.0,
            base_url: String::new(),
        };
        // ~2 chars per token for CJK
        let estimate = provider.estimate_tokens("你好世界");
        assert!(estimate >= 2 && estimate <= 4);
    }
}
```

- [ ] **Step 2: Register module**

Update `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
pub mod file_watcher;
pub mod qdrant_client;
pub mod embedding;
pub mod llm_client;
// pub mod tantivy_index;  // Task 7
```

- [ ] **Step 3: Run tests**

Run: `cd backend && cargo test --lib infra::llm_client::tests -- --nocapture`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/src/infra/llm_client.rs backend/src/infra/mod.rs
git commit -m "feat(infra): add LlmProvider trait with OpenAI and Ollama implementations"
```

---

### Task 7: Tantivy Full-Text Index

**Files:**
- Create: `backend/src/infra/tantivy_index.rs`
- Modify: `backend/src/infra/mod.rs`

- [ ] **Step 1: Write TantivyIndex with tests**

Create `backend/src/infra/tantivy_index.rs`:

```rust
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

use crate::error::BrainError;

/// Field references for the Tantivy schema.
struct FieldMap {
    title: Field,
    content: Field,
    path: Field,
    tags: Field,
}

/// A document to be indexed.
pub struct NoteDocument {
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
}

/// Search parameters.
pub struct SearchParams {
    pub query: String,
    pub top_k: usize,
    pub tag_filter: Option<Vec<String>>,
}

/// A single search result.
pub struct TantivySearchResult {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub tags: Vec<String>,
}

/// Tantivy full-text search index with Chinese (jieba) support.
pub struct TantivyIndex {
    index: Index,
    fields: FieldMap,
    writer: std::sync::Mutex<IndexWriter>,
    reader: IndexReader,
}

impl TantivyIndex {
    fn build_schema() -> (Schema, FieldMap) {
        let mut schema_builder = Schema::builder();

        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let title = schema_builder.add_text_field("title", text_options.clone());
        let content = schema_builder.add_text_field("content", text_options);

        let path = schema_builder.add_text_field("path", STRING | STORED);

        let tag_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("simple")
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();
        let tags = schema_builder.add_text_field("tags", tag_options);

        let schema = schema_builder.build();
        let fields = FieldMap {
            title,
            content,
            path,
            tags,
        };
        (schema, fields)
    }

    /// Open or create the index at `index_path`.
    pub fn new(index_path: &Path) -> Result<Self, BrainError> {
        let (schema, fields) = Self::build_schema();

        std::fs::create_dir_all(index_path)?;

        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path)
                .map_err(|e| BrainError::SearchError(format!("索引打开失败: {e}")))?
        } else {
            Index::create_in_dir(index_path, schema.clone())
                .map_err(|e| BrainError::SearchError(format!("索引创建失败: {e}")))?
        };

        // Register jieba tokenizer
        let jieba_tokenizer = tantivy_jieba::JiebaTokenizer {};
        index
            .tokenizers()
            .register("jieba", jieba_tokenizer);

        let writer = index
            .writer(50_000_000)
            .map_err(|e| BrainError::SearchError(format!("Writer 创建失败: {e}")))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| BrainError::SearchError(format!("Reader 创建失败: {e}")))?;

        Ok(TantivyIndex {
            index,
            fields,
            writer: std::sync::Mutex::new(writer),
            reader,
        })
    }

    /// Add a document to the index.
    pub fn add_document(&self, note: &NoteDocument) -> Result<(), BrainError> {
        let mut tantivy_doc = doc!(
            self.fields.title => note.title.clone(),
            self.fields.content => note.content.clone(),
            self.fields.path => note.path.clone(),
            self.fields.tags => note.tags.join(" "),
        );
        let _ = tantivy_doc; // suppress unused warning

        let writer = self.writer.lock().unwrap();
        writer
            .add_document(doc!(
                self.fields.title => note.title.clone(),
                self.fields.content => note.content.clone(),
                self.fields.path => note.path.clone(),
                self.fields.tags => note.tags.join(" "),
            ))
            .map_err(|e| BrainError::SearchError(format!("文档添加失败: {e}")))?;
        Ok(())
    }

    /// Update a document (delete old + add new).
    pub fn update_document(&self, note: &NoteDocument) -> Result<(), BrainError> {
        self.delete_document(&note.path)?;
        self.add_document(note)?;
        Ok(())
    }

    /// Delete a document by path.
    pub fn delete_document(&self, path: &str) -> Result<(), BrainError> {
        let term = Term::from_field_text(self.fields.path, path);
        let writer = self.writer.lock().unwrap();
        writer.delete_term(term);
        Ok(())
    }

    /// Commit pending changes and reload the reader.
    pub fn commit(&self) -> Result<(), BrainError> {
        let mut writer = self.writer.lock().unwrap();
        writer
            .commit()
            .map_err(|e| BrainError::SearchError(format!("提交失败: {e}")))?;
        self.reader
            .reload()
            .map_err(|e| BrainError::SearchError(format!("Reader 刷新失败: {e}")))?;
        Ok(())
    }

    /// Full-text search with optional tag filter.
    pub fn search(
        &self,
        params: &SearchParams,
    ) -> Result<Vec<TantivySearchResult>, BrainError> {
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.content],
        );

        let text_query = query_parser
            .parse_query(&params.query)
            .map_err(|e| BrainError::SearchError(format!("查询解析失败: {e}")))?;

        // Build combined query with optional tag filter
        let final_query: Box<dyn tantivy::query::Query> =
            if let Some(ref tags) = params.tag_filter {
                let mut sub_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> =
                    vec![(Occur::Must, text_query)];

                for tag in tags {
                    let tag_term = Term::from_field_text(self.fields.tags, tag);
                    let tag_query: Box<dyn tantivy::query::Query> =
                        Box::new(TermQuery::new(tag_term, IndexRecordOption::Basic));
                    sub_queries.push((Occur::Must, tag_query));
                }

                Box::new(BooleanQuery::from(sub_queries))
            } else {
                text_query
            };

        let top_docs = searcher
            .search(&*final_query, &TopDocs::with_limit(params.top_k))
            .map_err(|e| BrainError::SearchError(format!("搜索执行失败: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| BrainError::SearchError(format!("文档获取失败: {e}")))?;

            let path = doc
                .get_first(self.fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = doc
                .get_first(self.fields.content)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let tags_text = doc
                .get_first(self.fields.tags)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let snippet = make_snippet(content, &params.query, 200);

            results.push(TantivySearchResult {
                path,
                title,
                snippet,
                score,
                tags: tags_text.split_whitespace().map(String::from).collect(),
            });
        }

        Ok(results)
    }

    /// Check if the index is operational.
    pub fn health_check(&self) -> bool {
        let searcher = self.reader.searcher();
        searcher.num_docs().is_ok()
    }
}

/// Generate a text snippet around the first query match.
fn make_snippet(content: &str, query: &str, max_len: usize) -> String {
    let lower = content.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = lower.find(&query_lower) {
        let start = pos.saturating_sub(max_len / 3);
        let end = (pos + query.len() + max_len * 2 / 3).min(content.len());
        format!("...{}...", content[start..end].trim())
    } else {
        let end = max_len.min(content.len());
        format!("{}...", content[..end].trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_and_search_chinese() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Rust 异步编程".to_string(),
                content: "Tokio 是 Rust 生态中最流行的异步运行时框架".to_string(),
                path: "programming/rust-async.md".to_string(),
                tags: vec!["rust".to_string(), "async".to_string()],
            })
            .unwrap();

        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "异步运行时".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].path.contains("rust-async"));
    }

    #[test]
    fn test_delete_document() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Test".to_string(),
                content: "This should be deleted".to_string(),
                path: "test.md".to_string(),
                tags: vec![],
            })
            .unwrap();
        index.commit().unwrap();

        index.delete_document("test.md").unwrap();
        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "deleted".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_tag_filter() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index
            .add_document(&NoteDocument {
                title: "Rust Notes".to_string(),
                content: "Rust programming language notes".to_string(),
                path: "rust.md".to_string(),
                tags: vec!["rust".to_string()],
            })
            .unwrap();
        index
            .add_document(&NoteDocument {
                title: "Python Notes".to_string(),
                content: "Python programming language notes".to_string(),
                path: "python.md".to_string(),
                tags: vec!["python".to_string()],
            })
            .unwrap();
        index.commit().unwrap();

        let results = index
            .search(&SearchParams {
                query: "programming".to_string(),
                top_k: 5,
                tag_filter: Some(vec!["rust".to_string()]),
            })
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].path.contains("rust"));
    }

    #[test]
    fn test_snippet_generation() {
        let snippet = make_snippet("Hello world this is a test of snippet generation", "test", 30);
        assert!(snippet.contains("test"));
        assert!(snippet.starts_with("..."));
    }
}
```

- [ ] **Step 2: Register module**

Update `backend/src/infra/mod.rs`:

```rust
pub mod sqlite_store;
pub mod file_watcher;
pub mod qdrant_client;
pub mod embedding;
pub mod llm_client;
pub mod tantivy_index;
```

- [ ] **Step 3: Run tests**

Run: `cd backend && cargo test --lib infra::tantivy_index::tests -- --nocapture`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/src/infra/tantivy_index.rs backend/src/infra/mod.rs
git commit -m "feat(infra): add Tantivy full-text index with jieba Chinese tokenizer"
```

---

### Task 8: Integration into AppContext

**Files:**
- Modify: `backend/src/main.rs`
- Modify: `backend/src/api/handlers/health.rs`

- [ ] **Step 1: Update AppContext with all infra fields**

Replace `backend/src/main.rs` entirely:

```rust
mod api;
mod config;
mod core;
mod error;
mod infra;
mod models;
mod tools;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::infra::embedding::{EmbeddingFactory, EmbeddingProvider};
use crate::infra::llm_client::{LlmClientFactory, LlmProvider};
use crate::infra::qdrant_client::QdrantStore;
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::tantivy_index::TantivyIndex;

/// Application shared context — injected into all handlers.
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub db: Arc<SqliteStore>,
    pub embedding: Arc<dyn EmbeddingProvider>,
    pub llm: Arc<dyn LlmProvider>,
    pub qdrant: Arc<QdrantStore>,
    pub tantivy: Arc<TantivyIndex>,
    // file_watcher is spawned separately as a background task
    pub components: Arc<std::sync::Mutex<ComponentStatus>>,
}

/// Tracks which components are operational.
#[derive(Debug, Clone, Default)]
pub struct ComponentStatus {
    pub server: String,
    pub sqlite: String,
    pub qdrant: String,
    pub tantivy: String,
    pub embedding: String,
    pub llm: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Init logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "obsidian_brain=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("ObsidianBrain 启动中...");

    // 2. Load config
    let config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("配置加载失败: {e}，使用默认配置");
        AppConfig::default()
    });
    let addr = SocketAddr::from(([127, 0, 0, 1], config.server.port));
    tracing::info!("配置加载完成: {}:{}", addr.ip(), addr.port());

    // 3. Initialize infrastructure
    let mut components = ComponentStatus {
        server: "ok".to_string(),
        ..Default::default()
    };

    // SQLite
    let db = match SqliteStore::new(&config.storage.db_path) {
        Ok(store) => {
            components.sqlite = "ok".to_string();
            tracing::info!("SQLite 初始化成功: {:?}", config.storage.db_path);
            Arc::new(store)
        }
        Err(e) => {
            tracing::error!("SQLite 初始化失败: {e}");
            components.sqlite = format!("error: {e}");
            return Err(Box::new(e));
        }
    };

    // Tantivy
    let tantivy = match TantivyIndex::new(&config.storage.index_path) {
        Ok(idx) => {
            components.tantivy = "ok".to_string();
            tracing::info!("Tantivy 初始化成功: {:?}", config.storage.index_path);
            Arc::new(idx)
        }
        Err(e) => {
            tracing::error!("Tantivy 初始化失败: {e}");
            components.tantivy = format!("error: {e}");
            return Err(Box::new(e));
        }
    };

    // Qdrant
    let qdrant = match QdrantStore::new(&config.qdrant) {
        Ok(store) => {
            // Try to create collection (Qdrant might not be running)
            match store.ensure_collection().await {
                Ok(()) => {
                    components.qdrant = "ok".to_string();
                    tracing::info!("Qdrant 初始化成功");
                }
                Err(e) => {
                    tracing::warn!("Qdrant collection 创建失败 (Qdrant 可能未启动): {e}");
                    components.qdrant = format!("degraded: {e}");
                }
            }
            Arc::new(store)
        }
        Err(e) => {
            tracing::warn!("Qdrant 客户端创建失败: {e}");
            components.qdrant = format!("error: {e}");
            // Non-fatal: create a dummy store
            Arc::new(QdrantStore::new(&config.qdrant).unwrap_or_else(|_| {
                panic!("Qdrant store creation should not fail with valid config")
            }))
        }
    };

    // Embedding
    let embedding: Arc<dyn EmbeddingProvider> =
        Arc::from(EmbeddingFactory::create(&config.embedding).unwrap_or_else(|e| {
            tracing::warn!("Embedding 初始化失败: {e}");
            components.embedding = format!("error: {e}");
            // Fallback: create OpenAI embedder with empty key (API calls will fail gracefully)
            let mut fallback_cfg = config.embedding.clone();
            fallback_cfg.provider = "openai".to_string();
            EmbeddingFactory::create(&fallback_cfg).expect("fallback embedder must succeed")
        }));
    if components.embedding != "error" {
        components.embedding = "ok".to_string();
    }
    tracing::info!("Embedding 初始化完成: {}", config.embedding.provider);

    // LLM
    let llm: Arc<dyn LlmProvider> =
        Arc::from(LlmClientFactory::create(&config.llm).unwrap_or_else(|e| {
            tracing::warn!("LLM 初始化失败: {e}");
            components.llm = format!("error: {e}");
            let mut fallback_cfg = config.llm.clone();
            fallback_cfg.provider = "openai".to_string();
            LlmClientFactory::create(&fallback_cfg).expect("fallback LLM must succeed")
        }));
    if components.llm != "error" {
        components.llm = "ok".to_string();
    }
    tracing::info!("LLM 初始化完成: {}", config.llm.provider);

    // 4. Build AppContext
    let ctx = Arc::new(AppContext {
        config: Arc::new(config),
        db,
        embedding,
        llm,
        qdrant,
        tantivy,
        components: Arc::new(std::sync::Mutex::new(components)),
    });

    // 5. Build router and serve
    let app = api::router::create_router(ctx);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("服务已启动: http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("ObsidianBrain 已关闭");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("收到 Ctrl+C 信号"),
        _ = terminate => tracing::info!("收到 SIGTERM 信号"),
    }
}
```

- [ ] **Step 2: Update health handler to report real status**

Replace `backend/src/api/handlers/health.rs`:

```rust
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn health_check(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let components = ctx.components.lock().unwrap();

    // Also do a live check on SQLite and Tantivy
    let sqlite_status = if ctx.db.health_check() {
        components.sqlite.clone()
    } else {
        "unhealthy".to_string()
    };

    let tantivy_status = if ctx.tantivy.health_check() {
        components.tantivy.clone()
    } else {
        "unhealthy".to_string()
    };

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "server": components.server,
            "sqlite": sqlite_status,
            "qdrant": components.qdrant,
            "tantivy": tantivy_status,
            "embedding": components.embedding,
            "llm": components.llm,
        }
    }))
}
```

- [ ] **Step 3: Verify everything compiles and clippy passes**

Run: `cd backend && cargo clippy -- -D warnings`
Expected: Zero warnings. Fix any issues (likely some unused imports or dead code from the new context fields).

- [ ] **Step 4: Run all tests**

Run: `cd backend && cargo test --lib`
Expected: All tests pass (config + sqlite + file_watcher + tantivy + llm_client tests).

- [ ] **Step 5: Boot test**

Run: `cd backend && cargo run`
Expected:
- Logs show: "SQLite 初始化成功", "Tantivy 初始化成功", etc.
- Server starts on `127.0.0.1:9876`
- `curl http://127.0.0.1:9876/v1/health` returns JSON with component statuses

- [ ] **Step 6: Commit**

```bash
git add backend/src/main.rs backend/src/api/handlers/health.rs
git commit -m "feat(infra): integrate all infrastructure into AppContext with health reporting"
```

---

## Post-Phase 0 Checklist

After completing all 8 tasks:

- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test --lib` all pass
- [ ] `cargo run` boots and health endpoint responds
- [ ] `curl http://127.0.0.1:9876/v1/health` shows sqlite=ok, tantivy=ok
- [ ] Git log shows 8 clean commits for Phase 0

Ready for Phase 1: Memory Engine MVP.
