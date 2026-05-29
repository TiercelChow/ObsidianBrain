# 基础设施层（Infrastructure）开发设计文档

> **版本**: v0.1 | **最后更新**: 2026-05-29 | **状态**: 设计中
> **关联文档**: [顶层设计文档](../top_design.md) | [需求设计文档](../requirement/01-infrastructure.md)

---

## 1. 技术架构详细设计

### 1.1 整体架构定位

```
┌─────────────────────────────────────────────────────────────┐
│                    LLM 前端 (Claude / ChatGPT)               │
└──────────────────────────┬──────────────────────────────────┘
                           │  HTTP / MCP (127.0.0.1:9876)
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  API 层 (Axum) ──── 工具注册表 ──── 技能编排器 ── 事件总线  │
├─────────────────────────────────────────────────────────────┤
│  核心服务层                                                  │
│  Memory │ Timeline │ CodeRepo │ Inspiration │ Radar         │
├─────────────────────────────────────────────────────────────┤
│  ▶ 基础设施层（本文档范围）◀                                  │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Config  │ │ SQLite   │ │ File     │ │ Embedding      │  │
│  │ Manager │ │ Store    │ │ Watcher  │ │ Provider       │  │
│  └─────────┘ └──────────┘ └──────────┘ └────────────────┘  │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐                     │
│  │ LLM     │ │ Qdrant   │ │ Tantivy  │                     │
│  │ Client  │ │ Store    │ │ Index    │                     │
│  └─────────┘ └──────────┘ └──────────┘                     │
├─────────────────────────────────────────────────────────────┤
│  外部系统: 文件系统 │ SQLite │ Qdrant │ OpenAI │ Ollama     │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 子模块依赖关系

```
AppConfig ←──── 所有模块（配置注入）
    │
    ├──→ SqliteStore（元数据持久化）
    │       ↑
    │       ├── CodeRepo Service
    │       ├── Radar Service
    │       └── Inspiration Service
    │
    ├──→ FileWatcher（文件变更感知）
    │       ↑
    │       └── Memory Service（触发索引更新）
    │
    ├──→ EmbeddingProvider（向量化）
    │       ↑
    │       ├── Memory Service
    │       └── Radar Service
    │
    ├──→ LlmClient（LLM 调用）
    │       ↑
    │       ├── Inspiration Service
    │       ├── Radar Service
    │       └── CodeRepo Service（文档生成）
    │
    ├──→ QdrantStore（向量存储）
    │       ↑
    │       ├── Memory Service
    │       └── Radar Service
    │
    └──→ TantivyIndex（全文索引）
            ↑
            └── Memory Service
```

### 1.3 并发模型

系统基于 Tokio 异步运行时：

- **主线程**：Axum HTTP/MCP 服务
- **Worker Pool**：Tokio 多线程调度器处理工具调用
- **后台任务**：
  - FileWatcher 事件循环（独立线程 + channel 通知）
  - Radar 定时拉取（tokio-cron-scheduler）
  - Embedding 批处理队列（tokio::spawn）
- **Channel 通信**：
  - `tokio::sync::mpsc`：FileWatcher → Memory Service 变更事件
  - `tokio::sync::broadcast`：全局事件总线

---

## 2. 目录与文件组织

### 2.1 文件结构

```
src/
├── main.rs                  # 入口：启动、优雅关闭        (~80 行)
├── config.rs                # 配置管理                    (~250 行)
├── error.rs                 # 统一错误类型                (~200 行)
├── infra/
│   ├── mod.rs               # 模块导出                    (~30 行)
│   ├── sqlite_store.rs      # SQLite 元数据存储           (~400 行)
│   ├── file_watcher.rs      # 文件监控                    (~300 行)
│   ├── embedding.rs         # Embedding 生成              (~350 行)
│   ├── llm_client.rs        # LLM 调用封装               (~400 行)
│   ├── qdrant_client.rs     # Qdrant 向量操作            (~350 行)
│   └── tantivy_index.rs     # Tantivy 全文索引           (~400 行)
```

### 2.2 各文件职责

| 文件 | 职责 | 核心类型 |
|------|------|----------|
| `config.rs` | 配置加载、校验、环境覆盖 | `AppConfig`, `ServerConfig`, `VaultConfig` |
| `error.rs` | 统一错误枚举、转换、降级 | `BrainError` |
| `sqlite_store.rs` | 连接管理、迁移、CRUD | `SqliteStore` |
| `file_watcher.rs` | Vault 文件监控、防抖、事件分发 | `FileWatcher`, `FileChangeEvent` |
| `embedding.rs` | 多 Provider Embedding 生成 | `EmbeddingProvider`, `OpenAiEmbedder` |
| `llm_client.rs` | 多 Provider LLM 调用 | `LlmProvider`, `OpenAiProvider` |
| `qdrant_client.rs` | 向量存储操作 | `QdrantStore` |
| `tantivy_index.rs` | 全文索引管理 | `TantivyIndex` |

---

## 3. 配置管理 (config.rs)

### 3.1 配置结构体定义

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用顶层配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub vault: VaultConfig,
    pub qdrant: QdrantConfig,
    pub embedding: EmbeddingConfig,
    pub llm: LlmConfig,
    pub memory: MemoryConfig,
    pub timeline: TimelineConfig,
    pub radar: RadarConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
}

/// 服务配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: ProtocolType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Mcp,
    Http,
    Both,
}

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 9876 }
fn default_protocol() -> ProtocolType { ProtocolType::Mcp }

/// Vault 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    pub name: String,
    #[serde(default = "default_true")]
    pub watch_enabled: bool,
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,
}

fn default_true() -> bool { true }
fn default_exclude_patterns() -> Vec<String> {
    vec![
        ".obsidian/".to_string(),
        "templates/".to_string(),
        ".trash/".to_string(),
    ]
}

/// Qdrant 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default = "default_collection_name")]
    pub collection_name: String,
    #[serde(default = "default_vector_size")]
    pub vector_size: usize,
}

fn default_qdrant_url() -> String { "http://127.0.0.1:6333".to_string() }
fn default_collection_name() -> String { "obsidian_brain".to_string() }
fn default_vector_size() -> usize { 1536 }

/// Embedding 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    pub provider: EmbeddingProviderType,
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProviderType {
    Openai,
    Ollama,
    Onnx,
}

fn default_batch_size() -> usize { 100 }

/// LLM 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    pub provider: LlmProviderType,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProviderType {
    Openai,
    Anthropic,
    Ollama,
}

fn default_max_tokens() -> u32 { 2048 }
fn default_temperature() -> f64 { 0.7 }

/// 记忆引擎配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryConfig {
    #[serde(default = "default_chunk_min")]
    pub chunk_min_tokens: usize,
    #[serde(default = "default_chunk_max")]
    pub chunk_max_tokens: usize,
    #[serde(default = "default_top_k")]
    pub search_top_k: usize,
    #[serde(default = "default_true")]
    pub hybrid_search: bool,
    #[serde(default = "default_rrf_k")]
    pub rrf_k: u32,
}

fn default_chunk_min() -> usize { 300 }
fn default_chunk_max() -> usize { 800 }
fn default_top_k() -> usize { 5 }
fn default_rrf_k() -> u32 { 60 }

/// 时间线配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimelineConfig {
    #[serde(default = "default_date_formats")]
    pub date_formats: Vec<String>,
}

fn default_date_formats() -> Vec<String> {
    vec![
        "%Y-%m-%d".to_string(),
        "%Y/%m/%d".to_string(),
        "%Y%m%d".to_string(),
    ]
}

/// 雷达配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RadarConfig {
    #[serde(default = "default_fetch_interval")]
    pub fetch_interval_hours: u64,
    #[serde(default = "default_relevance_threshold")]
    pub relevance_threshold: f32,
    #[serde(default = "default_max_items")]
    pub max_items_per_source: usize,
    #[serde(default = "default_true")]
    pub readability_enabled: bool,
}

fn default_fetch_interval() -> u64 { 6 }
fn default_relevance_threshold() -> f32 { 0.7 }
fn default_max_items() -> usize { 20 }

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_index_path")]
    pub index_path: PathBuf,
}

fn default_db_path() -> PathBuf { PathBuf::from("./data/brain.db") }
fn default_index_path() -> PathBuf { PathBuf::from("./data/tantivy_index") }

/// 日志配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
}

fn default_log_level() -> String { "info".to_string() }
```

### 3.2 配置加载流程

```rust
use config::{Config, Environment, File};

impl AppConfig {
    /// 加载配置：TOML 文件 → 环境变量覆盖
    pub fn load(config_path: Option<&str>) -> Result<Self, BrainError> {
        let path = config_path.unwrap_or("config/default.toml");

        let builder = Config::builder()
            // 1. 加载默认配置
            .add_source(File::with_name("config/default").required(false))
            // 2. 加载用户指定配置（覆盖默认值）
            .add_source(File::with_name(path).required(false))
            // 3. 加载本地覆盖配置（不提交到版本控制）
            .add_source(File::with_name("config/local").required(false))
            // 4. 环境变量覆盖（OBRAIN_SERVER__PORT=9877）
            .add_source(
                Environment::with_prefix("OBRAIN")
                    .separator("__")
                    .try_parsing(true),
            );

        let config = builder.build()
            .map_err(|e| BrainError::ConfigError(format!("配置加载失败: {e}")))?;

        let app_config: AppConfig = config.try_deserialize()
            .map_err(|e| BrainError::ConfigError(format!("配置解析失败: {e}")))?;

        app_config.validate()?;
        Ok(app_config)
    }
}
```

### 3.3 校验逻辑

```rust
/// 配置校验 trait
pub trait Validate {
    fn validate(&self) -> Result<(), BrainError>;
}

impl Validate for AppConfig {
    fn validate(&self) -> Result<(), BrainError> {
        // 校验 vault 路径
        if !self.vault.path.exists() {
            return Err(BrainError::ConfigError(
                format!("Vault 路径不存在: {:?}", self.vault.path)
            ));
        }
        if !self.vault.path.is_dir() {
            return Err(BrainError::ConfigError(
                format!("Vault 路径不是目录: {:?}", self.vault.path)
            ));
        }

        // 校验端口范围
        if self.server.port < 1024 || self.server.port > 65535 {
            return Err(BrainError::ConfigError(
                format!("端口号超出范围: {}", self.server.port)
            ));
        }

        // 校验 Embedding API Key
        if matches!(self.embedding.provider, EmbeddingProviderType::Openai) {
            if let Some(ref env_key) = self.embedding.api_key_env {
                if std::env::var(env_key).is_err() {
                    return Err(BrainError::ConfigError(
                        format!("环境变量 {} 未设置", env_key)
                    ));
                }
            }
        }

        // 校验向量维度
        if self.qdrant.vector_size == 0 {
            return Err(BrainError::ConfigError(
                "向量维度不能为 0".to_string()
            ));
        }

        // 校验分块参数
        if self.memory.chunk_min_tokens >= self.memory.chunk_max_tokens {
            return Err(BrainError::ConfigError(
                "chunk_min_tokens 必须小于 chunk_max_tokens".to_string()
            ));
        }

        // 校验雷达参数
        if !(0.0..=1.0).contains(&self.radar.relevance_threshold) {
            return Err(BrainError::ConfigError(
                "relevance_threshold 必须在 0.0~1.0 之间".to_string()
            ));
        }

        Ok(())
    }
}
```

---

## 4. SQLite 元数据存储 (sqlite_store.rs)

### 4.1 核心结构

```rust
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// SQLite 存储管理器
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// 初始化 SQLite 连接并执行迁移
    pub fn new(db_path: &Path) -> Result<Self, BrainError> {
        // 确保父目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BrainError::IoError(e))?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| BrainError::Internal(format!("SQLite 打开失败: {e}")))?;

        // 启用 WAL 模式（提升并发读性能）
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| BrainError::Internal(format!("WAL 设置失败: {e}")))?;

        // 设置 busy timeout（防止并发写冲突）
        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .map_err(|e| BrainError::Internal(format!("busy_timeout 设置失败: {e}")))?;

        let store = SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.run_migrations()?;
        Ok(store)
    }

    /// 执行事务
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
                conn.execute_batch("ROLLBACK;").ok();
                Err(e)
            }
        }
    }
}
```

### 4.2 迁移框架

```rust
/// 迁移定义
struct Migration {
    version: u32,
    description: &'static str,
    sql: &'static str,
}

/// 所有迁移脚本
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "创建代码仓库表",
        sql: include_str!("../../migrations/001_code_repos.sql"),
    },
    Migration {
        version: 2,
        description: "创建雷达条目表",
        sql: include_str!("../../migrations/002_radar_items.sql"),
    },
    Migration {
        version: 3,
        description: "创建灵感历史表",
        sql: include_str!("../../migrations/003_inspiration.sql"),
    },
    Migration {
        version: 4,
        description: "创建时间线事件表",
        sql: include_str!("../../migrations/004_timeline.sql"),
    },
    Migration {
        version: 5,
        description: "创建应用状态表",
        sql: include_str!("../../migrations/005_app_state.sql"),
    },
];

impl SqliteStore {
    fn run_migrations(&self) -> Result<(), BrainError> {
        let conn = self.conn.lock().unwrap();

        // 创建迁移版本追踪表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );"
        ).map_err(|e| BrainError::Internal(format!("迁移表创建失败: {e}")))?;

        // 获取当前版本
        let current_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // 执行待应用的迁移
        for migration in MIGRATIONS {
            if migration.version > current_version {
                tracing::info!(
                    "执行迁移 v{}: {}",
                    migration.version,
                    migration.description
                );
                conn.execute_batch(migration.sql)
                    .map_err(|e| BrainError::Internal(
                        format!("迁移 v{} 执行失败: {e}", migration.version)
                    ))?;
                conn.execute(
                    "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                    params![migration.version, migration.description],
                ).map_err(|e| BrainError::Internal(
                    format!("迁移 v{} 记录失败: {e}", migration.version)
                ))?;
            }
        }

        Ok(())
    }
}
```

### 4.3 完整迁移 SQL

**migrations/001_code_repos.sql**:
```sql
CREATE TABLE IF NOT EXISTS code_repos (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    registered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata    TEXT  -- JSON 格式的缓存元信息
);

CREATE TABLE IF NOT EXISTS note_repo_links (
    note_path   TEXT NOT NULL,
    repo_name   TEXT NOT NULL REFERENCES code_repos(name) ON DELETE CASCADE,
    linked_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (note_path, repo_name)
);

CREATE INDEX IF NOT EXISTS idx_note_repo_links_repo ON note_repo_links(repo_name);
```

**migrations/002_radar_items.sql**:
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
    related_notes TEXT,  -- JSON 数组
    fetched_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    published_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_radar_items_status ON radar_items(status);
CREATE INDEX IF NOT EXISTS idx_radar_items_score ON radar_items(relevance_score DESC);
CREATE INDEX IF NOT EXISTS idx_radar_items_source ON radar_items(source);
```

**migrations/003_inspiration.sql**:
```sql
CREATE TABLE IF NOT EXISTS inspiration_history (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL CHECK(type IN ('concept_combo','reverse_question','counterpoint')),
    input_refs  TEXT,   -- JSON：输入的笔记/仓库引用
    output      TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_inspiration_type ON inspiration_history(type);
CREATE INDEX IF NOT EXISTS idx_inspiration_created ON inspiration_history(created_at DESC);
```

**migrations/004_timeline.sql**:
```sql
CREATE TABLE IF NOT EXISTS timeline_events (
    id          TEXT PRIMARY KEY,
    date        TEXT NOT NULL,          -- YYYY-MM-DD
    event_type  TEXT NOT NULL,
    title       TEXT NOT NULL,
    summary     TEXT,
    tags        TEXT,                   -- JSON 数组
    related_paths TEXT,                 -- JSON 数组
    source_path TEXT,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_timeline_date ON timeline_events(date);
CREATE INDEX IF NOT EXISTS idx_timeline_type ON timeline_events(event_type);
CREATE INDEX IF NOT EXISTS idx_timeline_date_type ON timeline_events(date, event_type);
```

**migrations/005_app_state.sql**:
```sql
CREATE TABLE IF NOT EXISTS app_state (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### 4.4 CRUD 封装

```rust
/// 应用状态操作
impl SqliteStore {
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
        ).map_err(|e| BrainError::Internal(format!("状态写入失败: {e}")))?;
        Ok(())
    }
}
```

---

## 5. 文件监控 (file_watcher.rs)

### 5.1 核心类型

```rust
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// 文件变更类型
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf, to: PathBuf },
}

/// 文件变更事件
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub change_type: FileChangeType,
    pub path: PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub extension: Option<String>,
}

/// 文件监控回调函数类型
pub type WatchCallback = Arc<dyn Fn(FileChangeEvent) + Send + Sync>;

/// 文件监控器
pub struct FileWatcher {
    _watcher: notify::RecommendedWatcher,
    tx: mpsc::Sender<FileChangeEvent>,
    exclude_patterns: Vec<String>,
    debounce_ms: u64,
}
```

### 5.2 防抖实现

```rust
/// 防抖器：合并短时间内的多次变更
struct Debouncer {
    pending: Arc<Mutex<std::collections::HashMap<PathBuf, PendingEvent>>>,
    debounce_duration: Duration,
    tx: mpsc::Sender<FileChangeEvent>,
}

struct PendingEvent {
    event_type: FileChangeType,
    first_seen: Instant,
    last_seen: Instant,
}

impl Debouncer {
    fn new(debounce_ms: u64, tx: mpsc::Sender<FileChangeEvent>) -> Self {
        let debouncer = Debouncer {
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
            debounce_duration: Duration::from_millis(debounce_ms),
            tx,
        };

        // 启动后台刷新任务
        let pending = debouncer.pending.clone();
        let duration = debouncer.debounce_duration;
        let flush_tx = debouncer.tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let now = Instant::now();
                let mut to_flush = Vec::new();

                {
                    let mut map = pending.lock().unwrap();
                    map.retain(|path, event| {
                        if now.duration_since(event.last_seen) >= duration {
                            to_flush.push(FileChangeEvent {
                                change_type: event.event_type.clone(),
                                path: path.clone(),
                                timestamp: chrono::Utc::now(),
                                extension: path.extension()
                                    .map(|e| e.to_string_lossy().to_string()),
                            });
                            false // 移除已刷新的条目
                        } else {
                            true // 保留未过期的条目
                        }
                    });
                }

                for event in to_flush {
                    let _ = flush_tx.send(event).await;
                }
            }
        });

        debouncer
    }

    fn record(&self, path: PathBuf, change_type: FileChangeType) {
        let mut map = self.pending.lock().unwrap();
        let now = Instant::now();

        map.entry(path)
            .and_modify(|e| {
                e.last_seen = now;
                // 如果先创建后修改，保留 Created 类型
                // 如果先修改后删除，变为 Deleted
                if matches!(change_type, FileChangeType::Deleted) {
                    e.event_type = FileChangeType::Deleted;
                }
            })
            .or_insert(PendingEvent {
                event_type: change_type,
                first_seen: now,
                last_seen: now,
            });
    }
}
```

### 5.3 FileWatcher 实现

```rust
impl FileWatcher {
    pub fn new(
        vault_path: &Path,
        exclude_patterns: Vec<String>,
        debounce_ms: u64,
    ) -> Result<(Self, mpsc::Receiver<FileChangeEvent>), BrainError> {
        let (tx, rx) = mpsc::channel(1024);

        let debouncer = Debouncer::new(debounce_ms, tx.clone());
        let debouncer_arc = Arc::new(debouncer);
        let exclude = exclude_patterns.clone();

        let mut watcher = notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        for path in &event.paths {
                            // 过滤排除模式
                            let path_str = path.to_string_lossy();
                            if exclude.iter().any(|p| path_str.contains(p)) {
                                continue;
                            }
                            // 仅监控 Markdown 文件
                            if path.extension()
                                .map(|e| e != "md")
                                .unwrap_or(true)
                            {
                                continue;
                            }

                            let change_type = match event.kind {
                                EventKind::Create(_) => FileChangeType::Created,
                                EventKind::Modify(_) => FileChangeType::Modified,
                                EventKind::Remove(_) => FileChangeType::Deleted,
                                _ => continue,
                            };

                            debouncer_arc.record(path.clone(), change_type);
                        }
                    }
                    Err(e) => {
                        tracing::error!("文件监控错误: {e}");
                    }
                }
            }
        ).map_err(|e| BrainError::Internal(format!("文件监控初始化失败: {e}")))?;

        watcher.watch(vault_path, RecursiveMode::Recursive)
            .map_err(|e| BrainError::Internal(format!("Vault 监控启动失败: {e}")))?;

        tracing::info!("文件监控启动: {:?}", vault_path);

        Ok((
            FileWatcher {
                _watcher: watcher,
                tx,
                exclude_patterns,
                debounce_ms,
            },
            rx,
        ))
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        tracing::info!("文件监控停止");
    }
}
```

---

## 6. Embedding 生成 (embedding.rs)

### 6.1 Trait 定义

```rust
use async_trait::async_trait;

/// Embedding Provider 统一接口
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// 单文本向量化
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;

    /// 批量向量化（默认分批 100 条）
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let mut results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.batch_size()) {
            let batch = self.embed_batch_inner(chunk).await?;
            results.extend(batch);
        }
        Ok(results)
    }

    /// 内部批量实现（由具体 Provider 实现）
    async fn embed_batch_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError>;

    /// 向量维度
    fn dimensions(&self) -> usize;

    /// 批量大小
    fn batch_size(&self) -> usize { 100 }
}
```

### 6.2 OpenAI 实现

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct OpenAiEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    base_url: String,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    usage: EmbeddingUsage,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Deserialize)]
struct EmbeddingUsage {
    prompt_tokens: u32,
    total_tokens: u32,
}

impl OpenAiEmbedder {
    pub fn new(config: &EmbeddingConfig) -> Result<Self, BrainError> {
        let api_key = config.api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .ok_or_else(|| BrainError::ConfigError(
                "OpenAI API Key 未配置".to_string()
            ))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OpenAiEmbedder {
            client,
            api_key,
            model: config.model.clone(),
            dimensions: 1536,  // text-embedding-3-small
            base_url: config.base_url.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbedder {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let results = self.embed_batch_inner(&[text.to_string()]).await?;
        results.into_iter().next()
            .ok_or_else(|| BrainError::EmbeddingError("空响应".to_string()))
    }

    async fn embed_batch_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
        };

        let response = retry_with_backoff(3, || async {
            self.client
                .post(format!("{}/embeddings", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .await
        }).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BrainError::EmbeddingError(
                format!("OpenAI API 错误 {status}: {body}")
            ));
        }

        let embedding_resp: EmbeddingResponse = response.json().await
            .map_err(|e| BrainError::EmbeddingError(format!("响应解析失败: {e}")))?;

        tracing::debug!(
            "Embedding 完成: {} 条文本, {} tokens",
            texts.len(),
            embedding_resp.usage.total_tokens
        );

        // 按 index 排序（API 可能乱序返回）
        let mut data = embedding_resp.data;
        data.sort_by_key(|d| d.index);
        Ok(data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimensions(&self) -> usize { self.dimensions }
}
```

### 6.3 Ollama 实现

```rust
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
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OllamaEmbedder {
            client,
            model: config.model.clone(),
            base_url: config.base_url.clone()
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

        let response = self.client
            .post(format!("{}/api/embed", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| BrainError::EmbeddingError(format!("Ollama 请求失败: {e}")))?;

        let resp: OllamaEmbedResponse = response.json().await
            .map_err(|e| BrainError::EmbeddingError(format!("Ollama 响应解析失败: {e}")))?;

        resp.embeddings.into_iter().next()
            .ok_or_else(|| BrainError::EmbeddingError("Ollama 空响应".to_string()))
    }

    async fn embed_batch_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        // Ollama 不支持批量 API，逐条调用
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize { 768 }  // nomic-embed-text 维度
}
```

### 6.4 ONNX 预留 + 工厂

```rust
/// ONNX 本地模型（预留）
pub struct OnnxEmbedder {
    _model_path: PathBuf,
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbedder {
    async fn embed_text(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        unimplemented!("ONNX Embedding 尚未实现")
    }

    async fn embed_batch_inner(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
        unimplemented!("ONNX Embedding 尚未实现")
    }

    fn dimensions(&self) -> usize { 384 }
}

/// Embedding Provider 工厂
pub struct EmbeddingFactory;

impl EmbeddingFactory {
    pub fn create(config: &EmbeddingConfig) -> Result<Box<dyn EmbeddingProvider>, BrainError> {
        match config.provider {
            EmbeddingProviderType::Openai => {
                Ok(Box::new(OpenAiEmbedder::new(config)?))
            }
            EmbeddingProviderType::Ollama => {
                Ok(Box::new(OllamaEmbedder::new(config)?))
            }
            EmbeddingProviderType::Onnx => {
                Err(BrainError::ConfigError(
                    "ONNX Embedding 尚未实现，请使用 openai 或 ollama".to_string()
                ))
            }
        }
    }
}
```

### 6.5 重试策略

```rust
use std::future::Future;

/// 指数退避重试
async fn retry_with_backoff<F, Fut, T>(max_retries: u32, f: F) -> Result<T, BrainError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
            tracing::warn!("重试第 {} 次，等待 {:?}...", attempt, delay);
            tokio::time::sleep(delay).await;
        }

        match f().await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                let is_retryable = e.is_timeout()
                    || e.is_connect()
                    || e.status().map(|s| s.as_u16() >= 500).unwrap_or(false);

                if !is_retryable {
                    return Err(BrainError::EmbeddingError(format!("不可重试的错误: {e}")));
                }
                last_error = Some(e);
            }
        }
    }

    Err(BrainError::EmbeddingError(
        format!("重试 {} 次后仍然失败: {:?}", max_retries, last_error)
    ))
}
```

---

## 7. LLM 客户端 (llm_client.rs)

### 7.1 核心类型

```rust
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// 聊天消息
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

/// LLM 响应
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

/// 流式响应块
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_final: bool,
}
```

### 7.2 Provider Trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 同步聊天（等待完整响应）
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError>;

    /// 流式聊天
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError>;

    /// 简单文本生成（便捷方法）
    async fn generate(&self, prompt: &str) -> Result<String, BrainError> {
        let messages = vec![
            ChatMessage {
                role: MessageRole::User,
                content: prompt.to_string(),
            },
        ];
        let resp = self.chat(&messages).await?;
        Ok(resp.content)
    }

    /// Token 估算
    fn estimate_tokens(&self, text: &str) -> u32 {
        // 粗略估算：英文 ~4 chars/token，中文 ~2 chars/token
        let char_count = text.chars().count();
        let cjk_count = text.chars()
            .filter(|c| *c as u32 > 0x4E00 && *c as u32 < 0x9FFF)
            .count();
        let non_cjk = char_count - cjk_count;
        ((non_cjk as f64 / 4.0) + (cjk_count as f64 / 2.0)).ceil() as u32
    }
}
```

### 7.3 OpenAI Provider

```rust
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
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiMessageResp {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl OpenAiProvider {
    pub fn new(config: &LlmConfig) -> Result<Self, BrainError> {
        let api_key = config.api_key_env
            .as_ref()
            .and_then(|env| std::env::var(env).ok())
            .ok_or_else(|| BrainError::ConfigError(
                "OpenAI API Key 未配置".to_string()
            ))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OpenAiProvider {
            client,
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            base_url: config.base_url.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<ChatResponse, BrainError> {
        let request = OpenAiChatRequest {
            model: self.model.clone(),
            messages: messages.iter().map(|m| OpenAiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
            }).collect(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: false,
        };

        let response = self.client
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

        let resp: OpenAiChatResponse = response.json().await
            .map_err(|e| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: format!("响应解析失败: {e}"),
            })?;

        let choice = resp.choices.into_iter().next()
            .ok_or_else(|| BrainError::LlmApiError {
                provider: "openai".to_string(),
                detail: "空响应".to_string(),
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
            messages: messages.iter().map(|m| OpenAiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
            }).collect(),
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
                    let _ = tx.send(StreamChunk {
                        content: format!("错误: {e}"),
                        is_final: true,
                    }).await;
                    return;
                }
            };

            // SSE 流解析
            let mut stream = resp.bytes_stream();
            use futures::StreamExt;
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    // 按行解析 SSE data
                    while let Some(line_end) = buffer.find('\n') {
                        let line = buffer[..line_end].trim().to_string();
                        buffer = buffer[line_end + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                let _ = tx.send(StreamChunk {
                                    content: String::new(),
                                    is_final: true,
                                }).await;
                                return;
                            }

                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                    let _ = tx.send(StreamChunk {
                                        content: content.to_string(),
                                        is_final: false,
                                    }).await;
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
```

### 7.4 Ollama Provider

```rust
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
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(OllamaProvider {
            client,
            model: config.model.clone(),
            base_url: config.base_url.clone()
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
            messages: messages.iter().map(|m| OpenAiMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content.clone(),
            }).collect(),
            stream: false,
        };

        let response = self.client
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

        let resp: Resp = response.json().await
            .map_err(|e| BrainError::LlmApiError {
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
        let (tx, rx) = mpsc::channel(64);
        // 简化实现：先获取完整响应再发送
        let response = self.chat(messages).await?;
        let _ = tx.send(StreamChunk {
            content: response.content,
            is_final: true,
        }).await;
        Ok(rx)
    }
}
```

### 7.5 LLM 工厂

```rust
pub struct LlmClientFactory;

impl LlmClientFactory {
    pub fn create(config: &LlmConfig) -> Result<Box<dyn LlmProvider>, BrainError> {
        match config.provider {
            LlmProviderType::Openai => {
                Ok(Box::new(OpenAiProvider::new(config)?))
            }
            LlmProviderType::Anthropic => {
                // Anthropic 实现类似 OpenAI，使用 /v1/messages 端点
                // 此处省略，结构与 OpenAiProvider 相同
                unimplemented!("Anthropic Provider 待实现")
            }
            LlmProviderType::Ollama => {
                Ok(Box::new(OllamaProvider::new(config)?))
            }
        }
    }
}
```

---

## 8. Qdrant 向量客户端 (qdrant_client.rs)

### 8.1 核心结构

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Qdrant 向量存储
pub struct QdrantStore {
    client: Client,
    base_url: String,
    collection_name: String,
    vector_size: usize,
}

/// 向量点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: Value,
}

/// 搜索结果
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Value,
}

/// Chunk Payload（存储在 Qdrant 中的元数据）
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
```

### 8.2 连接与 Collection 管理

```rust
impl QdrantStore {
    pub fn new(config: &QdrantConfig) -> Result<Self, BrainError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(QdrantStore {
            client,
            base_url: config.url.clone(),
            collection_name: config.collection_name.clone(),
            vector_size: config.vector_size,
        })
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, BrainError> {
        let resp = self.client
            .get(format!("{}/healthz", self.base_url))
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("连接失败: {e}")))?;
        Ok(resp.status().is_success())
    }

    /// 确保 Collection 存在
    pub async fn ensure_collection(&self) -> Result<(), BrainError> {
        // 检查是否存在
        let resp = self.client
            .get(format!("{}/collections/{}", self.base_url, self.collection_name))
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("查询失败: {e}")))?;

        if resp.status().is_success() {
            return Ok(());
        }

        // 创建 Collection
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
            .put(format!("{}/collections/{}", self.base_url, self.collection_name))
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("创建失败: {e}")))?;

        tracing::info!("Qdrant collection 创建: {}", self.collection_name);
        Ok(())
    }
}
```

### 8.3 向量操作

```rust
impl QdrantStore {
    /// 批量写入/更新向量
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
            points: points.into_iter().map(|p| PointData {
                id: p.id,
                vector: p.vector,
                payload: p.payload,
            }).collect(),
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

    /// 向量搜索
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

        let resp = self.client
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

        let search_resp: SearchResp = resp.json().await
            .map_err(|e| BrainError::QdrantError(format!("响应解析失败: {e}")))?;

        Ok(search_resp.result.into_iter().map(|r| SearchResult {
            id: r.id,
            score: r.score,
            payload: r.payload,
        }).collect())
    }

    /// 删除向量点
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
            .json(&DeleteReq { points: ids.to_vec() })
            .send()
            .await
            .map_err(|e| BrainError::QdrantError(format!("删除失败: {e}")))?;

        Ok(())
    }
}
```

---

## 9. Tantivy 全文索引 (tantivy_index.rs)

### 9.1 Schema 定义

```rust
use tantivy::{
    schema::*,
    Index, IndexWriter, IndexReader, Document,
    query::{QueryParser, BooleanQuery, TermQuery, Occur},
    collector::TopDocs,
    tokenizer::TextAnalyzer,
};

/// Tantivy 全文索引管理器
pub struct TantivyIndex {
    index: Index,
    schema: Schema,
    fields: FieldMap,
    writer: std::sync::Mutex<IndexWriter>,
    reader: IndexReader,
}

/// 字段映射
struct FieldMap {
    title: Field,
    content: Field,
    path: Field,
    tags: Field,
    created_at: Field,
    updated_at: Field,
}

impl TantivyIndex {
    /// 构建 Schema
    fn build_schema() -> (Schema, FieldMap) {
        let mut schema_builder = Schema::builder();

        // 标题：索引+存储，使用中文分词
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions)
            )
            .set_stored();

        let title = schema_builder.add_text_field("title", text_options.clone());
        let content = schema_builder.add_text_field("content", text_options);

        // 路径：索引+存储，不分词
        let path = schema_builder.add_text_field("path", STRING | STORED);

        // 标签：索引+存储，使用简单分词（空格分隔）
        let tag_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("simple")
                    .set_index_option(IndexRecordOption::WithFreqs)
            )
            .set_stored();
        let tags = schema_builder.add_text_field("tags", tag_options);

        // 时间戳
        let created_at = schema_builder.add_date_field("created_at", STORED);
        let updated_at = schema_builder.add_date_field("updated_at", STORED);

        let schema = schema_builder.build();
        let fields = FieldMap {
            title, content, path, tags, created_at, updated_at,
        };

        (schema, fields)
    }
}
```

### 9.2 初始化与分词器注册

```rust
use tantivy_jieba::JiebaTokenizer;

impl TantivyIndex {
    pub fn new(index_path: &std::path::Path) -> Result<Self, BrainError> {
        let (schema, fields) = Self::build_schema();

        // 创建或打开索引
        let index = if index_path.exists() {
            Index::open_in_dir(index_path)
                .map_err(|e| BrainError::SearchError(format!("索引打开失败: {e}")))?
        } else {
            std::fs::create_dir_all(index_path)
                .map_err(|e| BrainError::IoError(e))?;
            Index::create_in_dir(index_path, schema.clone())
                .map_err(|e| BrainError::SearchError(format!("索引创建失败: {e}")))?
        };

        // 注册 Jieba 中文分词器
        let jieba = JiebaTokenizer {};
        let tokenizer = TextAnalyzer::from(jieba);
        index.tokenizers().register("jieba", tokenizer);

        // 创建 IndexWriter（50MB heap）
        let writer = index.writer(50_000_000)
            .map_err(|e| BrainError::SearchError(format!("Writer 创建失败: {e}")))?;

        // 创建 IndexReader
        let reader = index.reader()
            .map_err(|e| BrainError::SearchError(format!("Reader 创建失败: {e}")))?;

        Ok(TantivyIndex {
            index,
            schema,
            fields,
            writer: std::sync::Mutex::new(writer),
            reader,
        })
    }
}
```

### 9.3 索引操作

```rust
impl TantivyIndex {
    /// 添加文档到索引
    pub fn add_document(&self, doc: &NoteDocument) -> Result<(), BrainError> {
        let mut tantivy_doc = Document::default();
        tantivy_doc.add_text(self.fields.title, &doc.title);
        tantivy_doc.add_text(self.fields.content, &doc.content);
        tantivy_doc.add_text(self.fields.path, &doc.path);
        tantivy_doc.add_text(self.fields.tags, &doc.tags.join(" "));
        tantivy_doc.add_date(self.fields.created_at, doc.created_at);
        tantivy_doc.add_date(self.fields.updated_at, doc.updated_at);

        let writer = self.writer.lock().unwrap();
        writer.add_document(tantivy_doc)
            .map_err(|e| BrainError::SearchError(format!("文档添加失败: {e}")))?;

        Ok(())
    }

    /// 更新文档（先删后加）
    pub fn update_document(&self, doc: &NoteDocument) -> Result<(), BrainError> {
        self.delete_document(&doc.path)?;
        self.add_document(doc)?;
        Ok(())
    }

    /// 删除文档（按路径）
    pub fn delete_document(&self, path: &str) -> Result<(), BrainError> {
        let term = Term::from_field_text(self.fields.path, path);
        let writer = self.writer.lock().unwrap();
        writer.delete_term(term);
        Ok(())
    }

    /// 提交变更
    pub fn commit(&self) -> Result<(), BrainError> {
        let mut writer = self.writer.lock().unwrap();
        writer.commit()
            .map_err(|e| BrainError::SearchError(format!("提交失败: {e}")))?;
        self.reader.reload()
            .map_err(|e| BrainError::SearchError(format!("Reader 刷新失败: {e}")))?;
        Ok(())
    }
}
```

### 9.4 搜索查询

```rust
/// 搜索参数
pub struct SearchParams {
    pub query: String,
    pub top_k: usize,
    pub tag_filter: Option<Vec<String>>,
}

/// 搜索结果
pub struct TantivySearchResult {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub tags: Vec<String>,
}

impl TantivyIndex {
    /// 全文搜索
    pub fn search(&self, params: &SearchParams) -> Result<Vec<TantivySearchResult>, BrainError> {
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.content],
        );

        let text_query = query_parser.parse_query(&params.query)
            .map_err(|e| BrainError::SearchError(format!("查询解析失败: {e}")))?;

        // 构建组合查询（文本查询 + 标签过滤）
        let final_query: Box<dyn tantivy::query::Query> = if let Some(ref tags) = params.tag_filter {
            let mut sub_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
                (Occur::Must, text_query),
            ];

            for tag in tags {
                let tag_term = Term::from_field_text(self.fields.tags, tag);
                let tag_query = Box::new(TermQuery::new(tag_term, IndexRecordOption::Basic));
                sub_queries.push((Occur::Must, tag_query));
            }

            Box::new(BooleanQuery::from(sub_queries))
        } else {
            text_query
        };

        let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(params.top_k))
            .map_err(|e| BrainError::SearchError(format!("搜索执行失败: {e}")))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: Document = searcher.doc(doc_address)
                .map_err(|e| BrainError::SearchError(format!("文档获取失败: {e}")))?;

            let path = doc.get_first(self.fields.path)
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            let title = doc.get_first(self.fields.title)
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            let content = doc.get_first(self.fields.content)
                .and_then(|v| v.as_text())
                .unwrap_or("");

            let tags_text = doc.get_first(self.fields.tags)
                .and_then(|v| v.as_text())
                .unwrap_or("");

            // 生成摘要片段
            let snippet = generate_snippet(content, &params.query, 200);

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
}

/// 生成搜索摘要片段
fn generate_snippet(content: &str, query: &str, max_len: usize) -> String {
    let lower_content = content.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_content.find(&lower_query) {
        let start = pos.saturating_sub(max_len / 3);
        let end = (pos + query.len() + max_len * 2 / 3).min(content.len());

        let snippet = &content[start..end];
        format!("...{}...", snippet.trim())
    } else {
        // 未找到匹配位置，返回开头
        let end = max_len.min(content.len());
        format!("{}...", &content[..end].trim())
    }
}
```

### 9.5 NoteDocument 结构

```rust
use chrono::{DateTime, Utc};
use tantivy::DateTime as TantivyDateTime;

/// 索引文档
pub struct NoteDocument {
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
    pub created_at: TantivyDateTime,
    pub updated_at: TantivyDateTime,
}

impl NoteDocument {
    pub fn from_chrono(
        title: String,
        content: String,
        path: String,
        tags: Vec<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        NoteDocument {
            title,
            content,
            path,
            tags,
            created_at: TantivyDateTime::from_timestamp_secs(created_at.timestamp()),
            updated_at: TantivyDateTime::from_timestamp_secs(updated_at.timestamp()),
        }
    }
}
```

---

## 10. 统一错误处理 (error.rs)

### 10.1 BrainError 枚举

```rust
use std::fmt;

/// 全局统一错误类型
#[derive(Debug)]
pub enum BrainError {
    // ── 配置与启动 ──
    /// 配置错误（路径无效、参数越界等）
    ConfigError(String),

    // ── Vault / 笔记 ──
    /// Vault 路径不存在
    VaultNotFound(std::path::PathBuf),
    /// 指定笔记不存在
    NoteNotFound(std::path::PathBuf),
    /// Markdown / Frontmatter 解析错误
    ParseError {
        path: std::path::PathBuf,
        detail: String,
    },

    // ── 搜索 ──
    /// 搜索引擎错误（Tantivy）
    SearchError(String),
    /// Embedding 生成错误
    EmbeddingError(String),

    // ── 代码仓 ──
    /// 代码仓库不存在
    RepoNotFound(std::path::PathBuf),
    /// Git 操作错误
    GitError {
        path: std::path::PathBuf,
        detail: String,
    },

    // ── 外部服务 ──
    /// Qdrant 操作错误
    QdrantError(String),
    /// LLM API 调用错误
    LlmApiError {
        provider: String,
        detail: String,
    },
    /// 外部数据抓取错误
    FetchError {
        url: String,
        detail: String,
    },

    // ── 通用 ──
    /// IO 错误
    IoError(std::io::Error),
    /// 内部错误
    Internal(String),
}

impl fmt::Display for BrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrainError::ConfigError(msg) => write!(f, "配置错误: {msg}"),
            BrainError::VaultNotFound(p) => write!(f, "Vault 不存在: {:?}", p),
            BrainError::NoteNotFound(p) => write!(f, "笔记不存在: {:?}", p),
            BrainError::ParseError { path, detail } =>
                write!(f, "解析错误 {:?}: {detail}", path),
            BrainError::SearchError(msg) => write!(f, "搜索错误: {msg}"),
            BrainError::EmbeddingError(msg) => write!(f, "Embedding 错误: {msg}"),
            BrainError::RepoNotFound(p) => write!(f, "仓库不存在: {:?}", p),
            BrainError::GitError { path, detail } =>
                write!(f, "Git 错误 {:?}: {detail}", path),
            BrainError::QdrantError(msg) => write!(f, "Qdrant 错误: {msg}"),
            BrainError::LlmApiError { provider, detail } =>
                write!(f, "LLM 错误 [{provider}]: {detail}"),
            BrainError::FetchError { url, detail } =>
                write!(f, "抓取错误 [{url}]: {detail}"),
            BrainError::IoError(e) => write!(f, "IO 错误: {e}"),
            BrainError::Internal(msg) => write!(f, "内部错误: {msg}"),
        }
    }
}

impl std::error::Error for BrainError {}
```

### 10.2 From 转换

```rust
impl From<std::io::Error> for BrainError {
    fn from(e: std::io::Error) -> Self {
        BrainError::IoError(e)
    }
}

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

impl From<serde_json::Error> for BrainError {
    fn from(e: serde_json::Error) -> Self {
        BrainError::Internal(format!("JSON 错误: {e}"))
    }
}

impl From<git2::Error> for BrainError {
    fn from(e: git2::Error) -> Self {
        BrainError::GitError {
            path: std::path::PathBuf::from("unknown"),
            detail: e.to_string(),
        }
    }
}
```

### 10.3 错误码映射

```rust
impl BrainError {
    /// 映射到工具协议错误码
    pub fn error_code(&self) -> &'static str {
        match self {
            BrainError::ConfigError(_) => "CONFIG_ERROR",
            BrainError::VaultNotFound(_) => "VAULT_NOT_FOUND",
            BrainError::NoteNotFound(_) => "NOTE_NOT_FOUND",
            BrainError::ParseError { .. } => "PARSE_ERROR",
            BrainError::SearchError(_) => "SEARCH_ERROR",
            BrainError::EmbeddingError(_) => "EMBEDDING_ERROR",
            BrainError::RepoNotFound(_) => "REPO_NOT_FOUND",
            BrainError::GitError { .. } => "GIT_ERROR",
            BrainError::QdrantError(_) => "QDRANT_ERROR",
            BrainError::LlmApiError { .. } => "LLM_API_ERROR",
            BrainError::FetchError { .. } => "FETCH_ERROR",
            BrainError::IoError(_) => "IO_ERROR",
            BrainError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// 是否可降级处理
    pub fn is_degradable(&self) -> bool {
        matches!(self,
            BrainError::QdrantError(_)
            | BrainError::EmbeddingError(_)
            | BrainError::LlmApiError { .. }
            | BrainError::FetchError { .. }
        )
    }
}
```

---

## 11. 数据流图

### 11.1 配置加载流程

```
启动
  │
  ▼
config/default.toml ──── 基础默认值
  │
  ▼
config/<指定路径>.toml ── 用户覆盖
  │
  ▼
config/local.toml ────── 本地覆盖（不入版本控制）
  │
  ▼
环境变量 OBRAIN_* ────── 运行时覆盖
  │
  ▼
serde deserialize ────── 类型校验
  │
  ▼
Validate::validate() ─── 业务校验（路径存在、范围合理）
  │
  ▼
AppConfig (Arc<>) ────── 全局共享
```

### 11.2 文件变更处理流程

```
Vault 文件变更
    │
    ▼
notify 事件 (EventKind)
    │
    ▼
路径过滤（排除 .obsidian/ .trash/ 等）
    │
    ▼
扩展名过滤（仅 .md 文件）
    │
    ▼
Debouncer（300ms 防抖合并）
    │
    ▼
FileChangeEvent (via mpsc::channel)
    │
    ├──→ Memory Service：更新索引
    ├──→ Timeline Service：记录事件
    └──→ CodeRepo Service：检查关联
```

### 11.3 搜索请求流程

```
search_notes(query)
    │
    ▼
┌───────────────────────────┐
│ tokio::join! 并行执行      │
│                           │
│  ┌─────────┐ ┌─────────┐ │
│  │Tantivy  │ │Qdrant   │ │
│  │全文搜索  │ │语义搜索  │ │
│  │BM25排序 │ │余弦排序  │ │
│  └────┬────┘ └────┬────┘ │
│       │           │      │
│       ▼           ▼      │
│    top 20      top 20    │
│       │           │      │
│       ▼           ▼      │
│    RRF 融合排序          │
│    (k=60)                │
└───────────────────────────┘
    │
    ▼
Top-K 结果（含 Obsidian URI）
```

---

## 12. 性能优化策略

| 策略 | 适用模块 | 实现方式 |
|------|----------|----------|
| 连接池 | SQLite | WAL 模式 + busy_timeout 5s |
| HTTP 连接复用 | reqwest | `pool_max_idle_per_host(5)` |
| 批量 Embedding | embedding.rs | 100 条/批，减少 API 调用 |
| 防抖合并 | file_watcher | 300ms 窗口合并多次变更 |
| 索引增量更新 | tantivy | 仅重新索引变更的 chunk |
| 搜索并行化 | 混合搜索 | `tokio::join!` 全文+语义并行 |
| 内存缓存 | Qdrant results | DashMap 缓存热门查询（TTL 5min） |
| 写入批量提交 | Tantivy | 变更累积后一次 commit |

---

## 13. 测试策略

### 13.1 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_load_defaults() {
        // 创建临时配置文件
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("test.toml");
        std::fs::write(&config_path, r#"
            [server]
            host = "127.0.0.1"
            port = 9999
            protocol = "http"

            [vault]
            path = "/tmp/test_vault"
            name = "test"

            [qdrant]
            url = "http://localhost:6333"

            [embedding]
            provider = "openai"
            model = "text-embedding-3-small"
            api_key_env = "TEST_KEY"

            [llm]
            provider = "openai"
            model = "gpt-4o-mini"

            [memory]
            chunk_min_tokens = 300
            chunk_max_tokens = 800

            [storage]
            db_path = "/tmp/test.db"
            index_path = "/tmp/test_index"
        "#).unwrap();

        let config = AppConfig::load(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.server.port, 9999);
        assert_eq!(config.vault.name, "test");
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut config = create_test_config();
        config.server.port = 80; // 低于 1024
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_sqlite_store_migration() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        // 验证所有表已创建
        store.set_state("test_key", "test_value").unwrap();
        let val = store.get_state("test_key").unwrap();
        assert_eq!(val, Some("test_value".to_string()));
    }

    #[test]
    fn test_tantivy_index_search() {
        let dir = TempDir::new().unwrap();
        let index = TantivyIndex::new(dir.path()).unwrap();

        index.add_document(&NoteDocument {
            title: "Rust 异步编程".to_string(),
            content: "Tokio 是 Rust 生态中最流行的异步运行时".to_string(),
            path: "programming/rust-async.md".to_string(),
            tags: vec!["rust".to_string(), "async".to_string()],
            created_at: TantivyDateTime::from_timestamp_secs(1700000000),
            updated_at: TantivyDateTime::from_timestamp_secs(1700000000),
        }).unwrap();
        index.commit().unwrap();

        let results = index.search(&SearchParams {
            query: "异步运行时".to_string(),
            top_k: 5,
            tag_filter: None,
        }).unwrap();

        assert!(!results.is_empty());
        assert!(results[0].path.contains("rust-async"));
    }
}
```

### 13.2 Mock 方案

使用 `mockall` 进行 trait mock：

```rust
use mockall::automock;

#[automock]
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;
    async fn embed_batch_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError>;
    fn dimensions(&self) -> usize;
    fn batch_size(&self) -> usize { 100 }
}

#[tokio::test]
async fn test_memory_service_with_mock_embedding() {
    let mut mock = MockEmbeddingProvider::new();
    mock.expect_embed_text()
        .returning(|_| Ok(vec![0.1; 1536]));
    mock.expect_dimensions()
        .returning(|| 1536);

    // 使用 mock 测试 Memory Service
}
```

### 13.3 测试覆盖目标

| 模块 | 目标覆盖率 | 关键测试场景 |
|------|-----------|-------------|
| config.rs | 90% | 加载、校验、默认值、环境变量覆盖 |
| sqlite_store.rs | 85% | 迁移、CRUD、事务、并发 |
| file_watcher.rs | 75% | 防抖、过滤、事件类型判断 |
| embedding.rs | 80% | Mock API 调用、重试、批量 |
| llm_client.rs | 80% | Mock API 调用、流式解析 |
| qdrant_client.rs | 80% | Mock HTTP、upsert/search/delete |
| tantivy_index.rs | 85% | 索引/搜索/删除、中文分词 |

---

## 14. 依赖清单

### 14.1 Cargo.toml

```toml
[package]
name = "obsidian-brain"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web 框架
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# 配置
config = "0.14"

# 数据库
rusqlite = { version = "0.31", features = ["bundled"] }

# 全文搜索
tantivy = "0.22"
tantivy-jieba = "0.2"

# HTTP 客户端
reqwest = { version = "0.12", features = ["json", "stream"] }

# Markdown 解析
pulldown-cmark = "0.10"
gray_matter = "0.2"

# Git 操作
git2 = "0.19"

# 文件监控
notify = "6"

# RSS 解析
feed-rs = "1"

# 异步工具
async-trait = "0.1"
futures = "0.3"
tokio-cron-scheduler = "0.10"

# 时间
chrono = { version = "0.4", features = ["serde"] }

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# 工具
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"

[dev-dependencies]
tempfile = "3"
mockall = "0.12"
wiremock = "0.6"
tokio-test = "0.4"
```

### 14.2 依赖关系图

```
config ──────────────→ serde, serde_json
rusqlite ────────────→ 无（bundled 编译）
tantivy ─────────────→ tantivy-jieba
reqwest ─────────────→ serde_json, tokio
notify ──────────────→ tokio::sync::mpsc
pulldown-cmark ──────→ 无
gray_matter ─────────→ serde_yaml
git2 ────────────────→ 无（系统 libgit2）
tracing ─────────────→ tracing-subscriber
chrono ──────────────→ serde
uuid ────────────────→ 无
```
