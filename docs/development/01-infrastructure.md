# 基础设施层（Infrastructure）开发设计文档

> **版本**: v0.2 | **最后更新**: 2026-06-12 | **状态**: 设计中
> **关联文档**: [顶层设计文档](../top_design.md) | [需求设计文档](../requirement/01-infrastructure.md)
>
> **架构说明**：项目已从混合搜索架构（Tantivy + Qdrant + Embedding）简化为直接使用 Obsidian Local REST API。不再需要本地索引、向量存储或 Embedding 服务。

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
│  │ Config  │ │ SQLite   │ │ File     │ │ Obsidian       │  │
│  │ Manager │ │ Store    │ │ Watcher  │ │ Client (新增)  │  │
│  │         │ │          │ │ (可选)   │ │                │  │
│  └─────────┘ └──────────┘ └──────────┘ └────────────────┘  │
│  ┌─────────┐                                                │
│  │ LLM     │                                                │
│  │ Client  │                                                │
│  └─────────┘                                                │
├─────────────────────────────────────────────────────────────┤
│  外部系统: 文件系统 │ SQLite │ Obsidian REST API │ OpenAI │ Ollama │
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
    ├──→ FileWatcher（文件变更感知，可选）
    │       ↑
    │       └── Memory Service（触发索引更新）
    │
    ├──→ ObsidianClient（Obsidian REST API 客户端）
    │       ↑
    │       └── Memory Service（搜索、笔记读写）
    │
    └──→ LlmClient（LLM 调用）
            ↑
            ├── Inspiration Service
            ├── Radar Service
            └── CodeRepo Service（文档生成）
```

### 1.3 并发模型

系统基于 Tokio 异步运行时：

- **主线程**：Axum HTTP/MCP 服务
- **Worker Pool**：Tokio 多线程调度器处理工具调用
- **后台任务**：
  - FileWatcher 事件循环（独立线程 + channel 通知，可选）
  - Radar 定时拉取（tokio-cron-scheduler）
- **Channel 通信**：
  - `tokio::sync::mpsc`：FileWatcher → Memory Service 变更事件
  - `tokio::sync::broadcast`：全局事件总线

---

## 2. 目录与文件组织

### 2.1 文件结构

```
backend/src/
├── main.rs                  # 入口：启动、优雅关闭        (~80 行)
├── config.rs                # 配置管理                    (~250 行)
├── error.rs                 # 统一错误类型                (~200 行)
├── infra/
│   ├── mod.rs               # 模块导出                    (~30 行)
│   ├── sqlite_store.rs      # SQLite 元数据存储           (~400 行)
│   ├── file_watcher.rs      # 文件监控（可选）            (~300 行)
│   ├── obsidian_client.rs   # Obsidian REST API 封装     (~350 行)
│   └── llm_client.rs        # LLM 调用封装               (~400 行)
```

### 2.2 各文件职责

| 文件 | 职责 | 核心类型 |
|------|------|----------|
| `config.rs` | 配置加载、校验、环境覆盖 | `AppConfig`, `ServerConfig`, `VaultConfig` |
| `error.rs` | 统一错误枚举、转换、降级 | `BrainError` |
| `sqlite_store.rs` | 连接管理、迁移、CRUD | `SqliteStore` |
| `file_watcher.rs` | Vault 文件监控、防抖、事件分发 | `FileWatcher`, `FileChangeEvent` |
| `obsidian_client.rs` | Obsidian Local REST API 封装 | `ObsidianClient` |
| `llm_client.rs` | 多 Provider LLM 调用 | `LlmProvider`, `OpenAiProvider` |

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
    pub obsidian: ObsidianConfig,
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

/// Obsidian REST API 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ObsidianConfig {
    pub enabled: bool,
    pub url: String,
    pub api_key: String,
}

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
}

fn default_chunk_min() -> usize { 300 }
fn default_chunk_max() -> usize { 800 }
fn default_top_k() -> usize { 5 }

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
}

fn default_db_path() -> PathBuf { PathBuf::from("./data/brain.db") }

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

        // 校验 Obsidian API 配置
        if self.obsidian.enabled && self.obsidian.url.is_empty() {
            return Err(BrainError::ConfigError(
                "Obsidian API URL 不能为空".to_string()
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

> **注意**：FileWatcher 当前标记为 `#[allow(dead_code)]`，在运行时未启用。搜索和笔记读写通过 Obsidian REST API 完成，无需本地文件监控。此模块保留以备未来需要实时文件变更感知的场景。

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

## 6. Obsidian REST API 客户端 (obsidian_client.rs)

### 6.1 核心结构

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Obsidian Local REST API 客户端
pub struct ObsidianClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

/// 搜索结果项
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResultItem {
    pub filename: String,
    pub score: f64,
    #[serde(default)]
    pub matches: Vec<SearchMatch>,
}

/// 搜索匹配片段
#[derive(Debug, Clone, Deserialize)]
pub struct SearchMatch {
    pub context: String,
    #[serde(rename = "match")]
    pub matched_text: SearchMatchText,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchMatchText {
    pub start: usize,
    pub end: usize,
}

/// 文件信息
#[derive(Debug, Clone, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub stat: FileStat,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileStat {
    pub ctime: u64,
    pub mtime: u64,
    pub size: u64,
}
```

### 6.2 初始化

```rust
impl ObsidianClient {
    pub fn new(config: &ObsidianConfig) -> Result<Self, BrainError> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // 自签名证书
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BrainError::Internal(format!("HTTP 客户端创建失败: {e}")))?;

        Ok(Self {
            client,
            base_url: config.url.clone(),
            api_key: config.api_key.clone(),
        })
    }

    /// 健康检查
    pub async fn health_check(&self) -> Result<bool, BrainError> {
        let resp = self.client
            .get(format!("{}/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("连接失败: {e}")))?;

        Ok(resp.status().is_success())
    }
}
```

### 6.3 搜索操作

```rust
impl ObsidianClient {
    /// 搜索笔记（JsonLogic 查询）
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, BrainError> {
        // POST /search/ with JsonLogic
        let body = serde_json::json!({
            "in": [query, {"var": "content"}]
        });

        let response = self.client
            .post(format!("{}/search/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/vnd.olrapi.jsonlogic+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("搜索请求失败: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("搜索 API 错误 {status}: {err_body}")
            ));
        }

        let results: Vec<SearchResultItem> = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(format!("搜索响应解析失败: {e}")))?;

        tracing::debug!(
            query = %query,
            result_count = results.len(),
            "Obsidian 搜索完成"
        );

        // 截取 top-K
        let limited = if results.len() > limit {
            results[..limit].to_vec()
        } else {
            results
        };

        Ok(limited)
    }
}
```

### 6.4 文件 CRUD

```rust
impl ObsidianClient {
    /// 读取文件内容
    pub async fn read_file(&self, path: &str) -> Result<String, BrainError> {
        let response = self.client
            .get(format!("{}/vault/{}", self.base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("读取文件失败: {e}")))?;

        if response.status().as_u16() == 404 {
            return Err(BrainError::NoteNotFound(
                std::path::PathBuf::from(path)
            ));
        }

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("读取文件错误 {status}: {err_body}")
            ));
        }

        response.text().await
            .map_err(|e| BrainError::ObsidianApiError(format!("响应读取失败: {e}")))
    }

    /// 写入文件
    pub async fn write_file(&self, path: &str, content: &str) -> Result<(), BrainError> {
        let response = self.client
            .put(format!("{}/vault/{}", self.base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("写入文件失败: {e}")))?;

        if !response.status().is_success() && response.status().as_u16() != 204 {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("写入文件错误 {status}: {err_body}")
            ));
        }

        Ok(())
    }

    /// 列出目录文件
    pub async fn list_files(&self, dir: &str) -> Result<Vec<String>, BrainError> {
        let url = if dir.is_empty() {
            format!("{}/vault/", self.base_url)
        } else {
            format!("{}/vault/{}/", self.base_url, dir)
        };

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("列出文件失败: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("列出文件错误 {status}: {err_body}")
            ));
        }

        #[derive(Deserialize)]
        struct FileListResponse {
            files: Vec<String>,
            folders: Vec<String>,
        }

        let resp: FileListResponse = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(format!("响应解析失败: {e}")))?;

        Ok(resp.files)
    }
}
```

### 6.5 周期笔记与命令

```rust
impl ObsidianClient {
    /// 获取今日周期笔记
    pub async fn get_periodic_note(&self) -> Result<Option<String>, BrainError> {
        let response = self.client
            .get(format!("{}/periodic/daily/", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("获取周期笔记失败: {e}")))?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("获取周期笔记错误 {status}: {err_body}")
            ));
        }

        #[derive(Deserialize)]
        struct PeriodicNote {
            content: String,
        }

        let note: PeriodicNote = response.json().await
            .map_err(|e| BrainError::ObsidianApiError(format!("响应解析失败: {e}")))?;

        Ok(Some(note.content))
    }

    /// 执行 Obsidian 命令
    pub async fn execute_command(&self, command_id: &str) -> Result<(), BrainError> {
        let response = self.client
            .post(format!("{}/commands/{}/", self.base_url, command_id))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| BrainError::ObsidianApiError(format!("执行命令失败: {e}")))?;

        if !response.status().is_success() && response.status().as_u16() != 204 {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(BrainError::ObsidianApiError(
                format!("执行命令错误 {status}: {err_body}")
            ));
        }

        Ok(())
    }
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
        // 构造请求、发送、解析响应（省略完整代码，与原文一致）
        todo!()
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        // SSE 流式解析（省略完整代码，与原文一致）
        todo!()
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
        // Ollama /api/chat 端点（省略完整代码，与原文一致）
        todo!()
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<StreamChunk>, BrainError> {
        todo!()
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

## 8. 统一错误处理 (error.rs)

### 8.1 BrainError 枚举

```rust
use std::fmt;

/// 全局统一错误类型
#[derive(Debug)]
pub enum BrainError {
    // ── 配置与启动 ──
    ConfigError(String),

    // ── Vault / 笔记 ──
    VaultNotFound(std::path::PathBuf),
    NoteNotFound(std::path::PathBuf),
    ParseError {
        path: std::path::PathBuf,
        detail: String,
    },

    // ── Obsidian API ──
    ObsidianApiError(String),

    // ── 代码仓 ──
    RepoNotFound(std::path::PathBuf),
    GitError {
        path: std::path::PathBuf,
        detail: String,
    },

    // ── 外部服务 ──
    LlmApiError {
        provider: String,
        detail: String,
    },
    FetchError {
        url: String,
        detail: String,
    },

    // ── 通用 ──
    IoError(std::io::Error),
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
            BrainError::ObsidianApiError(msg) => write!(f, "Obsidian API 错误: {msg}"),
            BrainError::RepoNotFound(p) => write!(f, "仓库不存在: {:?}", p),
            BrainError::GitError { path, detail } =>
                write!(f, "Git 错误 {:?}: {detail}", path),
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

### 8.2 From 转换

```rust
impl From<std::io::Error> for BrainError {
    fn from(e: std::io::Error) -> Self { BrainError::IoError(e) }
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

### 8.3 错误码映射

```rust
impl BrainError {
    pub fn error_code(&self) -> &'static str {
        match self {
            BrainError::ConfigError(_) => "CONFIG_ERROR",
            BrainError::VaultNotFound(_) => "VAULT_NOT_FOUND",
            BrainError::NoteNotFound(_) => "NOTE_NOT_FOUND",
            BrainError::ParseError { .. } => "PARSE_ERROR",
            BrainError::ObsidianApiError(_) => "OBSIDIAN_API_ERROR",
            BrainError::RepoNotFound(_) => "REPO_NOT_FOUND",
            BrainError::GitError { .. } => "GIT_ERROR",
            BrainError::LlmApiError { .. } => "LLM_API_ERROR",
            BrainError::FetchError { .. } => "FETCH_ERROR",
            BrainError::IoError(_) => "IO_ERROR",
            BrainError::Internal(_) => "INTERNAL_ERROR",
        }
    }

    pub fn is_degradable(&self) -> bool {
        matches!(self,
            BrainError::ObsidianApiError(_)
            | BrainError::LlmApiError { .. }
            | BrainError::FetchError { .. }
        )
    }
}
```

---

## 9. 数据流图

### 9.1 配置加载流程

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

### 9.2 文件变更处理流程

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
    ├──→ Memory Service：通知变更
    ├──→ Timeline Service：记录事件
    └──→ CodeRepo Service：检查关联
```

### 9.3 搜索请求流程

```
search_notes(query)
    │
    ▼
ObsidianClient::search(query, limit)
    │
    ▼
POST /search/ (JsonLogic)
    │
    ▼
Top-K 结果（含 Obsidian URI）
```

---

## 10. 性能优化策略

| 策略 | 适用模块 | 实现方式 |
|------|----------|----------|
| 连接池 | SQLite | WAL 模式 + busy_timeout 5s |
| HTTP 连接复用 | reqwest | `pool_max_idle_per_host(5)` |
| 防抖合并 | file_watcher | 300ms 窗口合并多次变更 |

---

## 11. 测试策略

### 11.1 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_load_defaults() {
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

            [obsidian]
            enabled = true
            url = "https://127.0.0.1:27124"
            api_key = "test-key"

            [llm]
            provider = "openai"
            model = "gpt-4o-mini"

            [memory]
            chunk_min_tokens = 300
            chunk_max_tokens = 800

            [storage]
            db_path = "/tmp/test.db"
        "#).unwrap();

        let config = AppConfig::load(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(config.server.port, 9999);
        assert_eq!(config.vault.name, "test");
        assert!(config.obsidian.enabled);
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut config = create_test_config();
        config.server.port = 80;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_sqlite_store_migration() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        store.set_state("test_key", "test_value").unwrap();
        let val = store.get_state("test_key").unwrap();
        assert_eq!(val, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_obsidian_client_search() {
        use wiremock::{MockServer, Mock, matchers, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/search/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                vec![serde_json::json!({
                    "filename": "test-note.md",
                    "score": 1.0,
                    "matches": []
                })]
            ))
            .mount(&mock_server)
            .await;

        let config = ObsidianConfig {
            enabled: true,
            url: mock_server.uri(),
            api_key: "test-key".to_string(),
        };

        let client = ObsidianClient::new(&config).unwrap();
        let results = client.search("测试查询", 5).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "test-note.md");
    }
}
```

### 11.2 Mock 方案

使用 `wiremock` 对 Obsidian REST API 和 LLM API 做 HTTP mock：

```rust
use wiremock::{MockServer, Mock, matchers, ResponseTemplate};

#[tokio::test]
async fn test_obsidian_client_read_file() {
    let mock_server = MockServer::start().await;

    Mock::given(matchers::method("GET"))
        .and(matchers::path("/vault/test-note.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# 测试笔记\n\n内容"))
        .mount(&mock_server)
        .await;

    let config = ObsidianConfig {
        enabled: true,
        url: mock_server.uri(),
        api_key: "test-key".to_string(),
    };

    let client = ObsidianClient::new(&config).unwrap();
    let content = client.read_file("test-note.md").await.unwrap();

    assert!(content.contains("测试笔记"));
}
```

### 11.3 测试覆盖目标

| 模块 | 目标覆盖率 | 关键测试场景 |
|------|-----------|-------------|
| config.rs | 90% | 加载、校验、默认值、环境变量覆盖 |
| sqlite_store.rs | 85% | 迁移、CRUD、事务、并发 |
| file_watcher.rs | 75% | 防抖、过滤、事件类型判断 |
| obsidian_client.rs | 80% | Mock HTTP、搜索、文件读写、健康检查 |
| llm_client.rs | 80% | Mock API 调用、流式解析 |

---

## 12. 依赖清单

### 12.1 Cargo.toml

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

# HTTP 客户端（Obsidian REST API + LLM API）
reqwest = { version = "0.12", features = ["json", "stream"] }

# Markdown 解析
pulldown-cmark = "0.10"
gray_matter = "0.2"

# Git 操作
git2 = "0.19"

# 文件监控（可选）
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

### 12.2 依赖关系图

```
config ──────────────→ serde, serde_json
rusqlite ────────────→ 无（bundled 编译）
reqwest ─────────────→ serde_json, tokio（用于 ObsidianClient + LlmProvider）
notify ──────────────→ tokio::sync::mpsc
pulldown-cmark ──────→ 无
gray_matter ─────────→ serde_yaml
git2 ────────────────→ 无（系统 libgit2）
tracing ─────────────→ tracing-subscriber
chrono ──────────────→ serde
uuid ────────────────→ 无
```
