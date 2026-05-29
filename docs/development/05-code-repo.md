# 代码仓聚合面板 (Code Repository Hub) — 开发设计文档

> **模块编号**: 05 | **版本**: v1.0 | **最后更新**: 2026-05-29 | **状态**: 开发设计中  
> **上游文档**: [需求设计文档](../requirement/05-code-repo.md) | [顶层设计文档](../top_design.md) §5.3

---

## 1. 技术架构详细设计

### 1.1 模块在系统中的位置

CodeRepo 模块位于核心服务层（`src/core/`），通过工具注册表（Tool Registry）暴露给 LLM，依赖基础设施层的 SQLite、FileWatcher、LLM Client 等组件。

```
┌─────────────────────────────────────────────────────────┐
│                      API 层 (Axum)                       │
│              handlers/code_repo.rs                       │
└────────────────────────┬────────────────────────────────┘
                         │ 调用
                         ▼
┌─────────────────────────────────────────────────────────┐
│                  CodeRepoService                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │ RepoManager  │  │ NoteLinker   │  │ DocGenerator  │ │
│  │ (仓库管理器)  │  │ (笔记关联器)  │  │ (文档生成器)   │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬────────┘ │
│         │                 │                  │          │
│  ┌──────┴───────┐  ┌──────┴───────┐  ┌──────┴────────┐ │
│  │ GitExtractor │  │ VscodeOpener │  │ RepoWatcher   │ │
│  │ (Git信息提取) │  │ (VSCode集成) │  │ (状态监控)     │ │
│  └──────────────┘  └──────────────┘  └───────────────┘ │
└────────────────────────┬────────────────────────────────┘
                         │ 依赖
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    基础设施层                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────┐ │
│  │ SQLite   │ │FileWatcher│ │ LLM Client│ │ Timeline │ │
│  │(rusqlite)│ │ (notify)  │ │ (reqwest) │ │ Service  │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────┘ │
└─────────────────────────────────────────────────────────┘
```

### 1.2 核心依赖关系

```
CodeRepoService
    ├── RepoManager ──────────→ git2 (Git 操作)
    │                    ──→ rusqlite (SQLite 持久化)
    │                    ──→ RepoWatcher (文件监控)
    ├── NoteLinker ───────────→ rusqlite (关联记录)
    │                    ──→ std::fs (笔记文件写入)
    │                    ──→ MemoryService (索引通知)
    ├── DocGenerator ─────────→ RepoManager (仓库信息提取)
    │                    ──→ LlmClient (LLM 调用)
    │                    ──→ std::fs (文档文件写入)
    ├── GitExtractor ─────────→ git2 (核心 Git 操作)
    ├── VscodeOpener ─────────→ std::process (系统调用)
    └── RepoWatcher ──────────→ notify (文件监控)
                         ──→ tokio (异步调度)
```

### 1.3 数据流总览

```
                    ┌──────────────────┐
                    │   LLM Tool Call  │
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
     ┌─────────────┐ ┌────────────┐ ┌──────────────┐
     │add_code_repo│ │ list_repos │ │generate_docs │
     └──────┬──────┘ └─────┬──────┘ └──────┬───────┘
            │              │               │
            ▼              ▼               ▼
    ┌───────────────────────────────────────────────┐
    │              RepoManager                       │
    │                                               │
    │  注册: path → git2::open → 提取元数据          │
    │  列表: SQLite 读取 + git2 实时刷新              │
    │  详情: git2 完整信息提取                        │
    │  监控: .git/HEAD → notify → 刷新缓存           │
    └───────┬───────────────────────────────────────┘
            │
     ┌──────┴──────────────┐
     ▼                     ▼
┌──────────┐        ┌────────────┐
│  SQLite  │        │  GitExtractor
│(持久化)   │        │  (git2 操作)
└──────────┘        └────────────┘
            │
            ▼
    ┌───────────────┐
    │  事件总线      │
    │  (Timeline)   │
    └───────────────┘
```

---

## 2. 目录与文件组织

### 2.1 文件布局

```
src/
├── core/
│   └── code_repo/
│       ├── mod.rs              # 模块入口，导出 CodeRepoService
│       ├── service.rs          # CodeRepoService 主结构体与 trait 实现
│       ├── manager.rs          # RepoManager — 仓库注册/查询/生命周期管理
│       ├── git_extractor.rs    # GitExtractor — git2 封装，Git 信息提取
│       ├── note_linker.rs      # NoteLinker — 笔记关联与引用块管理
│       ├── doc_generator.rs    # DocGenerator — LLM 文档生成
│       ├── vscode.rs           # VscodeOpener — VSCode URI 生成与唤起
│       ├── watcher.rs          # RepoWatcher — 仓库状态监控
│       └── language_detect.rs  # 语言检测与统计
├── api/
│   └── handlers/
│       └── code_repo.rs        # HTTP 请求处理器（调用 CodeRepoService）
├── models/
│   └── repo.rs                 # 共享数据模型定义
└── infra/
    ├── sqlite_store.rs         # SQLite 操作封装（含 code_repos 表操作）
    └── file_watcher.rs         # notify 文件监控封装
migrations/
    └── 001_init.sql            # SQLite schema（含 code_repos + note_repo_links）
config/
    ├── default.toml            # 默认配置（含 [code_repo] 段）
    └── doc_template.md         # 文档生成模板
```

### 2.2 模块职责说明

| 文件 | 职责 | 行数预估 |
|---|---|---|
| `mod.rs` | 模块声明、re-export | ~20 行 |
| `service.rs` | CodeRepoService 结构体、工具方法注册、对外接口编排 | ~200 行 |
| `manager.rs` | RepoManager：注册流程、列表查询、详情查询、缓存管理 | ~300 行 |
| `git_extractor.rs` | GitExtractor：分支、commit、工作区状态、远端信息提取 | ~250 行 |
| `note_linker.rs` | NoteLinker：关联/取消关联、引用块插入、自动建议 | ~200 行 |
| `doc_generator.rs` | DocGenerator：信息收集、prompt 组装、LLM 调用、文件写入 | ~250 行 |
| `vscode.rs` | VscodeOpener：URI 生成、跨平台系统调用 | ~80 行 |
| `watcher.rs` | RepoWatcher：HEAD 监控、定时刷新调度 | ~150 行 |
| `language_detect.rs` | 语言检测：文件遍历、扩展名映射、行数统计 | ~200 行 |

---

## 3. 关键数据结构

### 3.1 共享数据模型 (`src/models/repo.rs`)

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 已注册的代码仓库（SQLite 持久化 + 运行时缓存）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRepo {
    /// 仓库显示名称（唯一标识）
    pub name: String,
    /// 本地绝对路径
    pub path: PathBuf,
    /// 当前分支名
    pub current_branch: String,
    /// 语言构成：语言名 → 占比 (0.0~1.0)
    pub language_stats: HashMap<String, f32>,
    /// 工作区是否有未提交更改
    pub is_dirty: bool,
    /// 最近 commit 摘要列表
    pub recent_commits: Vec<CommitSummary>,
    /// 最后活动时间（最新 commit 时间）
    pub last_activity: DateTime<Utc>,
    /// 关联的笔记路径列表（vault 内相对路径）
    pub linked_notes: Vec<PathBuf>,
    /// 注册时间
    pub registered_at: DateTime<Utc>,
    /// 仓库状态
    pub status: RepoStatus,
}

/// 仓库状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    /// 正常可用
    Active,
    /// 路径不可访问
    Inactive,
}

/// Commit 摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    /// commit hash（短格式，7 位）
    pub hash: String,
    /// 作者名称
    pub author: String,
    /// commit message（第一行）
    pub message: String,
    /// commit 时间
    pub timestamp: DateTime<Utc>,
}

/// 仓库详细信息（扩展自 CodeRepo）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDetail {
    /// 基础信息
    #[serde(flatten)]
    pub base: CodeRepo,
    /// 本地分支列表
    pub branches: Vec<String>,
    /// 远端 URL 列表
    pub remote_urls: Vec<String>,
    /// 工作区详细状态
    pub working_dir_status: WorkingDirStatus,
    /// VSCode 打开 URI
    pub vscode_uri: String,
    /// HEAD commit hash（完整 40 位）
    pub head_hash: String,
    /// 总 commit 数（近似值）
    pub total_commits: usize,
    /// 贡献者列表
    pub contributors: Vec<String>,
}

/// 工作区详细状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkingDirStatus {
    /// 已修改文件数
    pub modified: usize,
    /// 已暂存新增文件数
    pub added: usize,
    /// 已删除文件数
    pub deleted: usize,
    /// 未追踪文件数
    pub untracked: usize,
    /// 总变更文件数
    pub total: usize,
}

/// 仓库卡片信息（用于列表展示，轻量版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCard {
    pub name: String,
    pub path: PathBuf,
    pub current_branch: String,
    pub latest_commit: Option<CommitSummary>,
    pub is_dirty: bool,
    pub languages: HashMap<String, f32>,
    pub linked_notes_count: usize,
    pub last_activity: DateTime<Utc>,
    pub status: RepoStatus,
}

/// 自动关联建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkSuggestion {
    pub note_path: PathBuf,
    pub suggested_repo: String,
    pub confidence: f32,
    pub reason: String,
}

/// 文档生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocGenerationResult {
    pub repo_name: String,
    pub doc_path: PathBuf,
    pub generated_at: DateTime<Utc>,
    pub word_count: usize,
    pub sections: Vec<String>,
}
```

### 3.2 SQLite 存储层封装

```rust
/// code_repos 表行数据（内部使用）
#[derive(Debug, Clone)]
pub struct CodeRepoRow {
    pub name: String,
    pub path: String,
    pub registered_at: String,     // ISO 8601
    pub metadata: String,          // JSON 序列化的 RepoMetadataCache
}

/// 元数据缓存（序列化存储到 SQLite metadata 列）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMetadataCache {
    pub current_branch: String,
    pub language_stats: HashMap<String, f32>,
    pub is_dirty: bool,
    pub latest_commit: Option<CommitSummary>,
    pub last_activity: String,     // ISO 8601
    pub head_hash: String,
    pub updated_at: String,        // ISO 8601，缓存更新时间
}

/// note_repo_links 表行数据
#[derive(Debug, Clone)]
pub struct NoteRepoLinkRow {
    pub note_path: String,
    pub repo_name: String,
    pub linked_at: String,         // ISO 8601
}
```

### 3.3 配置结构

```rust
/// CodeRepo 模块配置（从 config/default.toml [code_repo] 段加载）
#[derive(Debug, Clone, Deserialize)]
pub struct CodeRepoConfig {
    /// 定时刷新间隔（秒），默认 300
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_seconds: u64,

    /// 语言统计刷新间隔（秒），默认 3600
    #[serde(default = "default_language_refresh_interval")]
    pub language_refresh_interval_seconds: u64,

    /// 详情中展示的最近 commit 数，默认 20
    #[serde(default = "default_max_recent_commits")]
    pub max_recent_commits: usize,

    /// 默认文档输出目录（vault 内相对路径），默认 "code-docs"
    #[serde(default = "default_doc_target_dir")]
    pub doc_target_dir: String,

    /// 文档模板文件路径，默认 "config/doc_template.md"
    #[serde(default = "default_doc_template_path")]
    pub doc_template_path: String,

    /// 是否启用自动关联建议，默认 true
    #[serde(default = "default_true")]
    pub auto_suggest_enabled: bool,

    /// 自动关联建议的匹配阈值，默认 5
    #[serde(default = "default_auto_suggest_threshold")]
    pub auto_suggest_threshold: u32,

    /// 语言统计排除的目录
    #[serde(default = "default_exclude_dirs")]
    pub exclude_dirs: Vec<String>,

    /// 语言统计最大采样文件数，默认 500
    #[serde(default = "default_max_sample_files")]
    pub language_sample_max_files: usize,
}

fn default_refresh_interval() -> u64 { 300 }
fn default_language_refresh_interval() -> u64 { 3600 }
fn default_max_recent_commits() -> usize { 20 }
fn default_doc_target_dir() -> String { "code-docs".to_string() }
fn default_doc_template_path() -> String { "config/doc_template.md".to_string() }
fn default_true() -> bool { true }
fn default_auto_suggest_threshold() -> u32 { 5 }
fn default_max_sample_files() -> usize { 500 }

fn default_exclude_dirs() -> Vec<String> {
    vec![
        ".git", "node_modules", "target", "vendor",
        "__pycache__", ".venv", "venv", "dist", "build",
        ".next", ".nuxt", ".cache", ".tox", "Pods",
        ".idea", ".vscode", "coverage", ".DS_Store",
    ].into_iter().map(String::from).collect()
}
```

---

## 4. 各子模块详细设计

### 4.1 CodeRepoService — 主服务入口 (`service.rs`)

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// CodeRepo 模块主服务，对外暴露工具方法
pub struct CodeRepoService {
    /// 仓库管理器
    manager: Arc<RepoManager>,
    /// 笔记关联器
    note_linker: Arc<NoteLinker>,
    /// 文档生成器
    doc_generator: Arc<DocGenerator>,
    /// VSCode 集成器
    vscode_opener: Arc<VscodeOpener>,
    /// 仓库状态监控
    watcher: Arc<RepoWatcher>,
    /// 模块配置
    config: CodeRepoConfig,
}

impl CodeRepoService {
    /// 创建并初始化 CodeRepoService
    pub async fn new(
        db: Arc<SqliteStore>,
        llm_client: Arc<LlmClient>,
        file_watcher: Arc<FileWatcher>,
        timeline: Arc<TimelineService>,
        memory: Arc<MemoryService>,
        config: CodeRepoConfig,
        vault_path: PathBuf,
    ) -> Result<Self, BrainError> {
        let manager = Arc::new(RepoManager::new(
            db.clone(), config.clone(),
        ));
        let note_linker = Arc::new(NoteLinker::new(
            db.clone(), memory.clone(), vault_path.clone(),
        ));
        let doc_generator = Arc::new(DocGenerator::new(
            manager.clone(), llm_client, config.clone(),
            vault_path.clone(), memory.clone(), timeline.clone(),
        ));
        let vscode_opener = Arc::new(VscodeOpener::new());
        let watcher = Arc::new(RepoWatcher::new(
            manager.clone(), file_watcher, config.clone(),
        ));

        let service = Self {
            manager, note_linker, doc_generator,
            vscode_opener, watcher, config,
        };

        // 初始化：加载已注册仓库，设置监控
        service.initialize().await?;

        Ok(service)
    }

    /// 初始化：从 SQLite 加载已注册仓库，恢复文件监控
    async fn initialize(&self) -> Result<(), BrainError> {
        let repos = self.manager.list_from_db().await?;
        for repo in &repos {
            self.watcher.watch_repo(&repo.name, &repo.path).await?;
        }
        // 启动定时刷新任务
        self.watcher.start_periodic_refresh().await;
        tracing::info!("CodeRepoService initialized with {} repos", repos.len());
        Ok(())
    }

    // === 工具方法（对应 LLM Tool API）===

    pub async fn add_code_repo(
        &self, path: &str, name: &str,
    ) -> Result<CodeRepo, BrainError> { /* ... */ }

    pub async fn list_code_repos(&self) -> Result<Vec<RepoCard>, BrainError> { /* ... */ }

    pub async fn get_repo_detail(
        &self, name: &str,
    ) -> Result<RepoDetail, BrainError> { /* ... */ }

    pub async fn link_note_to_repo(
        &self, note_path: &str, repo_name: &str,
    ) -> Result<NoteRepoLink, BrainError> { /* ... */ }

    pub async fn generate_docs(
        &self, repo_name: &str, target_path: Option<&str>,
    ) -> Result<DocGenerationResult, BrainError> { /* ... */ }

    pub async fn open_in_vscode(
        &self, repo_name: &str,
    ) -> Result<VscodeResult, BrainError> { /* ... */ }

    /// 获取自动关联建议（供其他模块调用）
    pub async fn get_link_suggestions(
        &self, note_path: &str, note_content: &str,
    ) -> Result<Vec<LinkSuggestion>, BrainError> { /* ... */ }
}
```

---

### 4.2 RepoManager — 仓库管理器 (`manager.rs`)

#### 4.2.1 注册流程

```
路径校验 → git2 打开仓库 → 提取元数据 → 唯一性检查 → 写入 SQLite → 设置监控 → 发送事件
```

```rust
use git2::Repository;
use std::path::{Path, PathBuf};

pub struct RepoManager {
    db: Arc<SqliteStore>,
    config: CodeRepoConfig,
    /// 运行时仓库缓存（避免频繁读 SQLite）
    cache: RwLock<HashMap<String, CachedRepoInfo>>,
}

/// 运行时缓存信息
struct CachedRepoInfo {
    pub metadata: RepoMetadataCache,
    pub status: RepoStatus,
    pub updated_at: DateTime<Utc>,
}

impl RepoManager {
    pub fn new(db: Arc<SqliteStore>, config: CodeRepoConfig) -> Self {
        Self {
            db,
            config,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// 注册新仓库（完整流程）
    pub async fn register(
        &self,
        path: &str,
        name: &str,
        timeline: &TimelineService,
        watcher: &RepoWatcher,
    ) -> Result<CodeRepo, BrainError> {
        // Step 1: 路径校验
        let abs_path = self.validate_path(path)?;

        // Step 2: git2 打开仓库，验证合法性
        let repo = Repository::open(&abs_path).map_err(|e| {
            BrainError::GitError {
                path: abs_path.clone(),
                detail: format!("无法打开 Git 仓库: {}", e),
            }
        })?;

        // Step 3: 唯一性检查
        self.check_uniqueness(name, &abs_path).await?;

        // Step 4: 提取初始元数据
        let extractor = GitExtractor::new(&repo);
        let metadata = RepoMetadataCache {
            current_branch: extractor.current_branch()?,
            language_stats: HashMap::new(), // 语言统计异步单独做，较重
            is_dirty: extractor.is_dirty()?,
            latest_commit: extractor.latest_commit()?,
            last_activity: extractor.last_activity()?,
            head_hash: extractor.head_hash()?,
            updated_at: Utc::now().to_rfc3339(),
        };

        // Step 4.1: 异步提取语言统计（较重操作）
        let lang_stats = tokio::task::spawn_blocking({
            let path = abs_path.clone();
            let exclude = self.config.exclude_dirs.clone();
            let max_files = self.config.language_sample_max_files;
            move || {
                LanguageDetector::detect(&path, &exclude, max_files)
            }
        }).await.unwrap_or_default();

        let metadata = RepoMetadataCache {
            language_stats: lang_stats,
            ..metadata
        };

        // Step 5: 写入 SQLite
        let row = CodeRepoRow {
            name: name.to_string(),
            path: abs_path.to_string_lossy().to_string(),
            registered_at: Utc::now().to_rfc3339(),
            metadata: serde_json::to_string(&metadata)
                .map_err(|e| BrainError::Internal(e.to_string()))?,
        };
        self.db.insert_code_repo(&row).await?;

        // Step 6: 更新运行时缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(name.to_string(), CachedRepoInfo {
                metadata: metadata.clone(),
                status: RepoStatus::Active,
                updated_at: Utc::now(),
            });
        }

        // Step 7: 设置文件监控
        watcher.watch_repo(name, &abs_path).await?;

        // Step 8: 发送时间线事件
        let _ = timeline.emit_event(TimelineEvent {
            date: Utc::now().date_naive(),
            event_type: EventType::RepoRegistered,
            title: format!("注册代码仓库: {}", name),
            summary: format!(
                "路径: {}, 语言: {}",
                abs_path.display(),
                metadata.language_stats.iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                    .map(|(k, _)| k.as_str())
                    .unwrap_or("unknown")
            ),
            tags: vec!["code-repo".into(), "setup".into()],
            related_paths: vec![abs_path.clone()],
        }).await;

        // 构建返回结果
        self.build_code_repo(name, &abs_path, &metadata).await
    }

    /// 路径校验：存在、是目录、转为绝对路径
    fn validate_path(&self, path: &str) -> Result<PathBuf, BrainError> {
        let p = Path::new(path);
        if !p.exists() {
            return Err(BrainError::RepoNotFound(p.to_path_buf()));
        }
        if !p.is_dir() {
            return Err(BrainError::Internal(format!(
                "路径 '{}' 不是一个目录", path
            )));
        }
        // 规范化为绝对路径
        let abs = std::fs::canonicalize(p)
            .map_err(|e| BrainError::IoError(e))?;
        // 检查读权限
        std::fs::read_dir(&abs)
            .map_err(|_| BrainError::Internal(format!(
                "无法读取路径 '{}'，请检查目录权限", abs.display()
            )))?;
        Ok(abs)
    }

    /// 唯一性检查
    async fn check_uniqueness(
        &self, name: &str, path: &PathBuf,
    ) -> Result<(), BrainError> {
        // 检查名称是否重复
        if let Some(existing) = self.db.get_code_repo_by_name(name).await? {
            return Err(BrainError::Internal(format!(
                "名称 '{}' 已被仓库 '{}' 使用", name, existing.path
            )));
        }
        // 检查路径是否重复
        let path_str = path.to_string_lossy().to_string();
        if let Some(existing) = self.db.get_code_repo_by_path(&path_str).await? {
            return Err(BrainError::Internal(format!(
                "路径 '{}' 已注册为仓库 '{}'", path_str, existing.name
            )));
        }
        Ok(())
    }

    /// 列表查询：SQLite 读取 + 实时刷新
    pub async fn list(&self) -> Result<Vec<RepoCard>, BrainError> {
        let rows = self.db.list_code_repos().await?;
        let mut cards = Vec::with_capacity(rows.len());

        for row in rows {
            let card = self.build_card(&row).await;
            cards.push(card);
        }

        Ok(cards)
    }

    /// 构建卡片信息（SQLite 缓存 + git2 轻量刷新）
    async fn build_card(&self, row: &CodeRepoRow) -> RepoCard {
        let path = PathBuf::from(&row.path);
        let metadata: RepoMetadataCache = serde_json::from_str(&row.metadata)
            .unwrap_or_default();

        // 尝试实时刷新关键状态
        let (branch, dirty, latest) = match Repository::open(&path) {
            Ok(repo) => {
                let ext = GitExtractor::new(&repo);
                (
                    ext.current_branch().unwrap_or(metadata.current_branch.clone()),
                    ext.is_dirty().unwrap_or(metadata.is_dirty),
                    ext.latest_commit().unwrap_or(metadata.latest_commit.clone()),
                )
            }
            Err(_) => {
                // 仓库不可访问，使用缓存
                (
                    metadata.current_branch.clone(),
                    metadata.is_dirty,
                    metadata.latest_commit.clone(),
                )
            }
        };

        let status = if Repository::open(&path).is_ok() {
            RepoStatus::Active
        } else {
            RepoStatus::Inactive
        };

        let linked_count = self.db.count_note_links(&row.name)
            .await.unwrap_or(0);

        RepoCard {
            name: row.name.clone(),
            path,
            current_branch: branch,
            latest_commit: latest,
            is_dirty: dirty,
            languages: metadata.language_stats.clone(),
            linked_notes_count: linked_count,
            last_activity: DateTime::parse_from_rfc3339(&metadata.last_activity)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            status,
        }
    }

    /// 详情查询：完整 git2 信息提取
    pub async fn detail(&self, name: &str) -> Result<RepoDetail, BrainError> {
        let row = self.db.get_code_repo_by_name(name).await?
            .ok_or_else(|| BrainError::RepoNotFound(
                PathBuf::from(name)
            ))?;

        let path = PathBuf::from(&row.path);
        let repo = Repository::open(&path).map_err(|e| {
            BrainError::GitError {
                path: path.clone(),
                detail: format!("仓库路径不可访问: {}", e),
            }
        })?;

        let extractor = GitExtractor::new(&repo);

        // 完整信息提取
        let branches = extractor.branches()?;
        let remote_urls = extractor.remote_urls()?;
        let working_dir = extractor.working_dir_status()?;
        let recent_commits = extractor.recent_commits(
            self.config.max_recent_commits
        )?;
        let head_hash = extractor.head_hash()?;
        let total_commits = extractor.total_commits()?;
        let contributors = extractor.contributors(20)?;

        // 获取关联笔记
        let linked_notes = self.db.get_linked_notes(&row.name).await?
            .into_iter()
            .map(PathBuf::from)
            .collect();

        let metadata: RepoMetadataCache = serde_json::from_str(&row.metadata)
            .unwrap_or_default();

        let base = CodeRepo {
            name: row.name.clone(),
            path: path.clone(),
            current_branch: extractor.current_branch()?,
            language_stats: metadata.language_stats,
            is_dirty: extractor.is_dirty()?,
            recent_commits,
            last_activity: extractor.last_activity()?,
            linked_notes,
            registered_at: DateTime::parse_from_rfc3339(&row.registered_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            status: RepoStatus::Active,
        };

        // 异步刷新元数据缓存
        self.refresh_cache(&row.name, &path).await;

        Ok(RepoDetail {
            base,
            branches,
            remote_urls,
            working_dir_status: working_dir,
            vscode_uri: VscodeOpener::make_uri(&path),
            head_hash,
            total_commits,
            contributors,
        })
    }

    /// 刷新元数据缓存（异步，不阻塞主流程）
    async fn refresh_cache(&self, name: &str, path: &Path) {
        let path = path.to_path_buf();
        let db = self.db.clone();
        let name = name.to_string();

        tokio::spawn(async move {
            if let Ok(repo) = Repository::open(&path) {
                let ext = GitExtractor::new(&repo);
                let metadata = RepoMetadataCache {
                    current_branch: ext.current_branch().unwrap_or_default(),
                    language_stats: HashMap::new(), // 语言统计单独刷新
                    is_dirty: ext.is_dirty().unwrap_or(false),
                    latest_commit: ext.latest_commit().unwrap_or(None),
                    last_activity: ext.last_activity()
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default(),
                    head_hash: ext.head_hash().unwrap_or_default(),
                    updated_at: Utc::now().to_rfc3339(),
                };
                if let Ok(json) = serde_json::to_string(&metadata) {
                    let _ = db.update_repo_metadata(&name, &json).await;
                }
            }
        });
    }
}
```

---

### 4.3 GitExtractor — Git 信息提取器 (`git_extractor.rs`)

使用 `git2` crate 封装所有 Git 操作。每次提取创建新的 `GitExtractor` 实例，借用 `Repository` 引用。

```rust
use git2::{Repository, StatusOptions, BranchType, Sort};
use chrono::{DateTime, Utc, TimeZone};

/// Git 信息提取器，封装 git2 操作
pub struct GitExtractor<'a> {
    repo: &'a Repository,
}

impl<'a> GitExtractor<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    // ─── 分支信息 ──────────────────────────────────────────────

    /// 获取当前分支名
    /// 若处于 detached HEAD 状态，返回 "HEAD (detached)"
    pub fn current_branch(&self) -> Result<String, BrainError> {
        let head = self.repo.head().map_err(|e| self.git_err(e))?;
        if head.is_branch() {
            head.shorthand()
                .map(|s| s.to_string())
                .ok_or_else(|| self.internal_err("无法获取分支名"))
        } else {
            // detached HEAD
            let hash = head.target()
                .map(|oid| &oid.to_string()[..7])
                .unwrap_or("unknown");
            Ok(format!("HEAD (detached at {})", hash))
        }
    }

    /// 获取所有本地分支列表
    pub fn branches(&self) -> Result<Vec<String>, BrainError> {
        let branches = self.repo.branches(Some(BranchType::Local))
            .map_err(|e| self.git_err(e))?;

        let mut result = Vec::new();
        for branch in branches {
            let (branch, _) = branch.map_err(|e| self.git_err(e))?;
            if let Some(name) = branch.name().map_err(|e| self.git_err(e))? {
                result.push(name.to_string());
            }
        }
        Ok(result)
    }

    // ─── Commit 历史 ───────────────────────────────────────────

    /// 获取最近 N 条 commit 摘要
    pub fn recent_commits(
        &self, n: usize,
    ) -> Result<Vec<CommitSummary>, BrainError> {
        let mut revwalk = self.repo.revwalk()
            .map_err(|e| self.git_err(e))?;
        revwalk.set_sorting(Sort::TIME)
            .map_err(|e| self.git_err(e))?;
        revwalk.push_head()
            .map_err(|e| self.git_err(e))?;

        let mut commits = Vec::with_capacity(n);
        for oid in revwalk.take(n) {
            let oid = oid.map_err(|e| self.git_err(e))?;
            let commit = self.repo.find_commit(oid)
                .map_err(|e| self.git_err(e))?;

            commits.push(CommitSummary {
                hash: commit.id().to_string()[..7].to_string(),
                author: commit.author().name()
                    .unwrap_or("unknown").to_string(),
                message: commit.summary()
                    .unwrap_or("").to_string(),
                timestamp: Utc.timestamp_opt(
                    commit.time().seconds(), 0
                ).single().unwrap_or_else(Utc::now),
            });
        }
        Ok(commits)
    }

    /// 获取最新一条 commit
    pub fn latest_commit(&self) -> Result<Option<CommitSummary>, BrainError> {
        let commits = self.recent_commits(1)?;
        Ok(commits.into_iter().next())
    }

    /// 获取总 commit 数（遍历计数，大仓库可能较慢）
    /// 注意：这是近似值，不包含合并 commit 的去重
    pub fn total_commits(&self) -> Result<usize, BrainError> {
        let mut revwalk = self.repo.revwalk()
            .map_err(|e| self.git_err(e))?;
        revwalk.push_head()
            .map_err(|e| self.git_err(e))?;
        Ok(revwalk.count())
    }

    /// 获取贡献者列表（从最近 N 条 commit 中提取去重）
    pub fn contributors(&self, sample_size: usize) -> Result<Vec<String>, BrainError> {
        let commits = self.recent_commits(sample_size)?;
        let mut authors: Vec<String> = commits.into_iter()
            .map(|c| c.author)
            .collect();
        authors.sort();
        authors.dedup();
        Ok(authors)
    }

    // ─── 工作区状态 ────────────────────────────────────────────

    /// 快速检测工作区是否有未提交更改
    pub fn is_dirty(&self) -> Result<bool, BrainError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false)  // 不深入 untracked 目录，提速
            .include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))
            .map_err(|e| self.git_err(e))?;
        Ok(!statuses.is_empty())
    }

    /// 详细工作区状态
    pub fn working_dir_status(&self) -> Result<WorkingDirStatus, BrainError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false)
            .include_ignored(false);

        let statuses = self.repo.statuses(Some(&mut opts))
            .map_err(|e| self.git_err(e))?;

        let mut status = WorkingDirStatus::default();
        for entry in statuses.iter() {
            let s = entry.status();
            if s.is_wt_modified() || s.is_index_modified() {
                status.modified += 1;
            }
            if s.is_index_new() {
                status.added += 1;
            }
            if s.is_wt_deleted() || s.is_index_deleted() {
                status.deleted += 1;
            }
            if s.is_wt_new() {
                status.untracked += 1;
            }
        }
        status.total = status.modified + status.added
            + status.deleted + status.untracked;
        Ok(status)
    }

    // ─── 远端信息 ──────────────────────────────────────────────

    /// 获取所有远端 URL
    pub fn remote_urls(&self) -> Result<Vec<String>, BrainError> {
        let remotes = self.repo.remotes()
            .map_err(|e| self.git_err(e))?;
        let mut urls = Vec::new();
        for remote_name in remotes.iter() {
            if let Some(name) = remote_name {
                if let Ok(remote) = self.repo.find_remote(name) {
                    if let Some(url) = remote.url() {
                        urls.push(url.to_string());
                    }
                }
            }
        }
        Ok(urls)
    }

    // ─── 元信息 ────────────────────────────────────────────────

    /// 获取 HEAD commit hash（完整 40 位）
    pub fn head_hash(&self) -> Result<String, BrainError> {
        let head = self.repo.head().map_err(|e| self.git_err(e))?;
        head.target()
            .map(|oid| oid.to_string())
            .ok_or_else(|| self.internal_err("无法获取 HEAD hash"))
    }

    /// 获取最后活动时间（最新 commit 的时间）
    pub fn last_activity(&self) -> Result<DateTime<Utc>, BrainError> {
        match self.latest_commit()? {
            Some(c) => Ok(c.timestamp),
            None => Ok(Utc::now()),
        }
    }

    // ─── 辅助方法 ──────────────────────────────────────────────

    fn git_err(&self, e: git2::Error) -> BrainError {
        BrainError::GitError {
            path: self.repo.path().to_path_buf(),
            detail: e.message().to_string(),
        }
    }

    fn internal_err(&self, msg: &str) -> BrainError {
        BrainError::Internal(msg.to_string())
    }
}
```

---

### 4.4 LanguageDetector — 语言检测与统计 (`language_detect.rs`)

#### 4.4.1 算法设计

**策略**: 遍历仓库文件树，按文件扩展名分类，统计每种语言的文件数量和代码行数，计算占比。

**核心算法**:

```
1. 从仓库根目录开始递归遍历
2. 跳过排除目录（.git, node_modules, target, vendor 等）
3. 跳过二进制文件和无扩展名文件
4. 按扩展名映射到语言名称
5. 统计每种语言的代码行数（排除空行和注释行）
6. 计算占比：language_lines / total_lines
7. 过滤占比 < 1% 的语言
8. 返回排序后的 HashMap<语言名, 占比>
```

#### 4.4.2 扩展名映射表

```rust
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// 语言检测器
pub struct LanguageDetector;

impl LanguageDetector {
    /// 检测仓库的语言构成
    ///
    /// # 参数
    /// - `root`: 仓库根目录
    /// - `exclude_dirs`: 排除的目录名列表
    /// - `max_files`: 最大采样文件数（防止超大仓库耗时过长）
    ///
    /// # 返回
    /// 语言名 → 占比 (0.0~1.0) 的 HashMap
    pub fn detect(
        root: &Path,
        exclude_dirs: &[String],
        max_files: usize,
    ) -> HashMap<String, f32> {
        let mut lang_lines: HashMap<String, u64> = HashMap::new();
        let mut total_lines: u64 = 0;
        let mut file_count: usize = 0;

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                // 排除指定目录
                if e.file_type().is_dir() {
                    let name = e.file_name().to_string_lossy();
                    return !exclude_dirs.iter().any(|d| d == name.as_ref());
                }
                true
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if file_count >= max_files {
                break;
            }

            let path = entry.path();
            let ext = match path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
            {
                Some(ext) => ext,
                None => continue,
            };

            let language = match Self::ext_to_language(&ext) {
                Some(lang) => lang.to_string(),
                None => continue,
            };

            // 统计行数
            let lines = Self::count_lines(path);
            if lines > 0 {
                *lang_lines.entry(language).or_insert(0) += lines;
                total_lines += lines;
                file_count += 1;
            }
        }

        // 计算占比
        let mut result = HashMap::new();
        if total_lines > 0 {
            for (lang, lines) in &lang_lines {
                let ratio = *lines as f32 / total_lines as f32;
                if ratio >= 0.01 {
                    // 过滤占比 < 1% 的语言
                    result.insert(lang.clone(), (ratio * 100.0).round() / 100.0);
                }
            }
        }

        result
    }

    /// 文件扩展名 → 语言名映射
    fn ext_to_language(ext: &str) -> Option<&'static str> {
        match ext {
            // Rust
            "rs" => Some("Rust"),
            // Python
            "py" | "pyi" | "pyx" => Some("Python"),
            // JavaScript / TypeScript
            "js" | "mjs" | "cjs" | "jsx" => Some("JavaScript"),
            "ts" | "mts" | "cts" | "tsx" => Some("TypeScript"),
            // Java / Kotlin / Scala
            "java" => Some("Java"),
            "kt" | "kts" => Some("Kotlin"),
            "scala" | "sc" => Some("Scala"),
            // C / C++ / Objective-C
            "c" | "h" => Some("C"),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("C++"),
            "m" => Some("Objective-C"),
            "swift" => Some("Swift"),
            // Go
            "go" => Some("Go"),
            // Ruby
            "rb" | "erb" => Some("Ruby"),
            // PHP
            "php" => Some("PHP"),
            // Shell
            "sh" | "bash" | "zsh" | "fish" => Some("Shell"),
            // Web
            "html" | "htm" => Some("HTML"),
            "css" | "scss" | "sass" | "less" => Some("CSS"),
            "vue" | "svelte" => Some("Vue"),
            // 配置 / 标记
            "toml" => Some("TOML"),
            "yaml" | "yml" => Some("YAML"),
            "json" => Some("JSON"),
            "xml" => Some("XML"),
            "md" | "mdx" => Some("Markdown"),
            // 数据 / 科学
            "sql" => Some("SQL"),
            "r" | "R" => Some("R"),
            "jl" => Some("Julia"),
            "ipynb" => Some("Jupyter Notebook"),
            // 其他
            "dart" => Some("Dart"),
            "lua" => Some("Lua"),
            "zig" => Some("Zig"),
            "ex" | "exs" => Some("Elixir"),
            "hs" => Some("Haskell"),
            "ml" | "mli" => Some("OCaml"),
            "proto" => Some("Protocol Buffers"),
            "tf" => Some("Terraform"),
            "dockerfile" => Some("Dockerfile"),
            _ => None,
        }
    }

    /// 统计文件有效行数（排除空行）
    fn count_lines(path: &Path) -> u64 {
        match std::fs::read_to_string(path) {
            Ok(content) => content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count() as u64,
            Err(_) => 0, // 二进制文件或读取失败，跳过
        }
    }
}
```

#### 4.4.3 采样策略

| 仓库文件数 | 策略 |
|---|---|
| < 500 | 全量遍历 |
| 500 ~ 5000 | 采样前 500 个文件（按目录遍历顺序） |
| > 5000 | 采样前 500 个文件 + 顶层目录统计推算 |

> 采样通过 `max_files` 参数控制，默认 500。对于超大仓库（如 Linux kernel），采样结果可能存在偏差，但对于"卡片级信息展示"的定位已足够。

---

### 4.5 NoteLinker — 笔记关联器 (`note_linker.rs`)

```rust
use std::path::{Path, PathBuf};

pub struct NoteLinker {
    db: Arc<SqliteStore>,
    memory: Arc<MemoryService>,
    vault_path: PathBuf,
}

impl NoteLinker {
    pub fn new(
        db: Arc<SqliteStore>,
        memory: Arc<MemoryService>,
        vault_path: PathBuf,
    ) -> Self {
        Self { db, memory, vault_path }
    }

    /// 建立笔记与仓库的关联
    pub async fn link(
        &self,
        note_path: &str,
        repo_name: &str,
        repo: &CodeRepo,
    ) -> Result<NoteRepoLink, BrainError> {
        // Step 1: 校验笔记文件存在
        let full_path = self.vault_path.join(note_path);
        if !full_path.exists() {
            return Err(BrainError::NoteNotFound(
                PathBuf::from(note_path)
            ));
        }

        // Step 2: 去重检查
        let existing = self.db.get_note_repo_link(note_path, repo_name).await?;
        if existing.is_some() {
            // 已存在关联，直接返回
            return Ok(NoteRepoLink {
                note_path: note_path.to_string(),
                repo_name: repo_name.to_string(),
                linked_at: existing.unwrap().linked_at,
                vscode_uri: VscodeOpener::make_uri(&repo.path),
            });
        }

        // Step 3: 插入引用块到笔记末尾
        self.insert_reference_block(&full_path, repo)?;

        // Step 4: 记录关联到 SQLite
        let linked_at = Utc::now().to_rfc3339();
        self.db.insert_note_repo_link(note_path, repo_name, &linked_at).await?;

        // Step 5: 通知记忆引擎重新索引
        let _ = self.memory.reindex_note(note_path).await;

        Ok(NoteRepoLink {
            note_path: note_path.to_string(),
            repo_name: repo_name.to_string(),
            linked_at,
            vscode_uri: VscodeOpener::make_uri(&repo.path),
        })
    }

    /// 在笔记末尾插入标准引用块
    fn insert_reference_block(
        &self,
        note_full_path: &Path,
        repo: &CodeRepo,
    ) -> Result<(), BrainError> {
        let mut content = std::fs::read_to_string(note_full_path)
            .map_err(BrainError::IoError)?;

        // 检查是否已有该仓库的引用块（防重复）
        if content.contains(&format!("**{}**", repo.name))
            && content.contains("相关代码仓库")
        {
            return Ok(());
        }

        let vscode_uri = VscodeOpener::make_uri(&repo.path);
        let last_activity = repo.last_activity.format("%Y-%m-%d");

        // 构建引用块
        let block = if content.contains("## 🔗 相关代码仓库") {
            // 已有引用块段落，追加新仓库
            format!(
                "\n- **{}** — `{}`\n  - [在 VSCode 中打开]({})\n  - 最后活动: {} | 分支: {}\n",
                repo.name,
                repo.path.display(),
                vscode_uri,
                last_activity,
                repo.current_branch,
            )
        } else {
            // 新建引用块段落
            format!(
                "\n\n---\n## 🔗 相关代码仓库\n- **{}** — `{}`\n  - [在 VSCode 中打开]({})\n  - 最后活动: {} | 分支: {}\n",
                repo.name,
                repo.path.display(),
                vscode_uri,
                last_activity,
                repo.current_branch,
            )
        };

        content.push_str(&block);
        std::fs::write(note_full_path, content)
            .map_err(BrainError::IoError)?;
        Ok(())
    }

    /// 自动关联建议：关键词匹配
    pub async fn suggest_links(
        &self,
        note_path: &str,
        note_title: &str,
        note_content: &str,
    ) -> Result<Vec<LinkSuggestion>, BrainError> {
        let repos = self.db.list_code_repos().await?;
        let mut suggestions = Vec::new();

        // 提取笔记关键词（简单分词：按空白和标点分割）
        let note_keywords = Self::extract_keywords(
            &format!("{} {}", note_title, note_content)
        );

        for repo_row in &repos {
            let metadata: RepoMetadataCache = serde_json::from_str(
                &repo_row.metadata
            ).unwrap_or_default();

            let mut score: u32 = 0;
            let mut reasons = Vec::new();

            // 规则 1: 精确匹配仓库名（权重 10）
            let repo_name_lower = repo_row.name.to_lowercase();
            if note_title.to_lowercase().contains(&repo_name_lower) {
                score += 10;
                reasons.push(format!(
                    "笔记标题包含仓库名 '{}'", repo_row.name
                ));
            } else if note_content.to_lowercase().contains(&repo_name_lower) {
                score += 5;
                reasons.push(format!(
                    "笔记内容包含仓库名 '{}'", repo_row.name
                ));
            }

            // 规则 2: 匹配 commit message 关键词（权重 × 出现次数）
            if let Some(ref commit) = metadata.latest_commit {
                let commit_words: Vec<String> = commit.message
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(|w| w.to_string())
                    .collect();
                for word in &commit_words {
                    if note_keywords.contains(word) {
                        score += 2;
                        reasons.push(format!(
                            "笔记与 commit message '{}' 共享关键词 '{}'",
                            commit.message, word
                        ));
                    }
                }
            }

            // 规则 3: 匹配主要语言名称（权重 2）
            if let Some((primary_lang, _)) = metadata.language_stats
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            {
                if note_content.to_lowercase()
                    .contains(&primary_lang.to_lowercase())
                {
                    score += 2;
                    reasons.push(format!(
                        "笔记提及主要语言 '{}'", primary_lang
                    ));
                }
            }

            if score >= 5 {
                // 阈值判定
                let confidence = (score as f32 / 20.0).min(1.0);
                suggestions.push(LinkSuggestion {
                    note_path: PathBuf::from(note_path),
                    suggested_repo: repo_row.name.clone(),
                    confidence,
                    reason: reasons.join("; "),
                });
            }
        }

        // 按 confidence 降序排列
        suggestions.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap()
        });

        Ok(suggestions)
    }

    /// 简单关键词提取（去停用词 + 小写化）
    fn extract_keywords(text: &str) -> Vec<String> {
        let stop_words = [
            "the", "a", "an", "is", "are", "was", "were",
            "and", "or", "but", "in", "on", "at", "to",
            "for", "of", "with", "by", "this", "that",
            "it", "be", "as", "from", "not",
        ];
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 3 && !stop_words.contains(w))
            .map(|w| w.to_string())
            .collect()
    }
}
```

---

### 4.6 DocGenerator — 文档生成器 (`doc_generator.rs`)

```rust
use std::path::{Path, PathBuf};

pub struct DocGenerator {
    manager: Arc<RepoManager>,
    llm_client: Arc<LlmClient>,
    config: CodeRepoConfig,
    vault_path: PathBuf,
    memory: Arc<MemoryService>,
    timeline: Arc<TimelineService>,
}

impl DocGenerator {
    /// 为仓库生成文档笔记
    pub async fn generate(
        &self,
        repo_name: &str,
        target_path: Option<&str>,
    ) -> Result<DocGenerationResult, BrainError> {
        // Step 1: 获取仓库详情
        let detail = self.manager.detail(repo_name).await?;

        // Step 2: 提取仓库上下文信息
        let context = self.extract_repo_context(&detail).await?;

        // Step 3: 加载文档模板（若存在）
        let template = self.load_template();

        // Step 4: 组装 LLM prompt
        let prompt = self.build_prompt(&context, &template);

        // Step 5: 调用 LLM 生成文档
        let doc_content = self.llm_client
            .generate(&prompt, 4096)
            .await
            .map_err(|e| BrainError::LlmApiError {
                provider: self.llm_client.provider_name(),
                detail: e.to_string(),
            })?;

        // Step 6: 后处理（确保有 frontmatter）
        let final_content = self.post_process(&doc_content, &detail);

        // Step 7: 写入文件
        let target_dir = target_path
            .unwrap_or(&self.config.doc_target_dir);
        let doc_filename = format!("{}-docs.md", repo_name);
        let doc_path = PathBuf::from(target_dir).join(&doc_filename);
        let full_path = self.vault_path.join(&doc_path);

        // 确保目标目录存在
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(BrainError::IoError)?;
        }

        std::fs::write(&full_path, &final_content)
            .map_err(BrainError::IoError)?;

        let word_count = final_content.chars().count();

        // Step 8: 自动建立关联
        // （通知 NoteLinker，此处省略，由 Service 层编排）

        // Step 9: 通知记忆引擎索引
        let _ = self.memory.reindex_note(
            &doc_path.to_string_lossy()
        ).await;

        // Step 10: 发送时间线事件
        let _ = self.timeline.emit_event(TimelineEvent {
            date: Utc::now().date_naive(),
            event_type: EventType::RepoDocumented,
            title: format!("生成文档: {}", repo_name),
            summary: format!("文档路径: {}", doc_path.display()),
            tags: vec![
                "code-repo".into(),
                "documentation".into(),
                repo_name.to_string(),
            ],
            related_paths: vec![doc_path.clone()],
        }).await;

        // 解析生成的章节
        let sections = self.extract_sections(&final_content);

        Ok(DocGenerationResult {
            repo_name: repo_name.to_string(),
            doc_path,
            generated_at: Utc::now(),
            word_count,
            sections,
        })
    }

    /// 提取仓库上下文信息（供 LLM 使用）
    async fn extract_repo_context(
        &self,
        detail: &RepoDetail,
    ) -> Result<RepoContext, BrainError> {
        // 目录结构（深度限制 3 层）
        let tree = Self::directory_tree(
            &detail.base.path,
            &self.config.exclude_dirs,
            3,
        );

        // README 内容
        let readme = Self::read_file_if_exists(
            &detail.base.path, &["README.md", "README", "readme.md"]
        );

        // 项目配置文件
        let (config_name, config_content) = Self::read_project_config(
            &detail.base.path
        );

        // 核心源文件头部注释
        let headers = Self::source_file_headers(
            &detail.base.path,
            &self.config.exclude_dirs,
            10, // 最多 10 个文件
        );

        // 最近 commit 文本
        let commits_text = detail.base.recent_commits.iter()
            .map(|c| format!(
                "- {} {} ({})",
                &c.hash, c.message,
                c.timestamp.format("%Y-%m-%d")
            ))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(RepoContext {
            repo_name: detail.base.name.clone(),
            repo_path: detail.base.path.display().to_string(),
            current_branch: detail.base.current_branch.clone(),
            language_stats: format!(
                "{}",
                detail.base.language_stats.iter()
                    .map(|(k, v)| format!("{}: {:.0}%", k, v * 100.0))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            last_activity: detail.base.last_activity
                .format("%Y-%m-%d %H:%M").to_string(),
            directory_tree: tree,
            readme_content: readme.unwrap_or_else(
                || "（未找到 README 文件）".to_string()
            ),
            config_file_name: config_name
                .unwrap_or_else(|| "无".to_string()),
            config_file_content: config_content
                .unwrap_or_else(|| "（未找到项目配置文件）".to_string()),
            source_file_headers: headers,
            recent_commits_text: commits_text,
            head_hash: detail.head_hash.clone(),
        })
    }

    /// 生成目录树（排除指定目录，深度限制）
    fn directory_tree(
        root: &Path,
        exclude: &[String],
        max_depth: usize,
    ) -> String {
        let mut output = String::new();
        Self::walk_tree(root, exclude, max_depth, 0, &mut output);
        output
    }

    fn walk_tree(
        dir: &Path,
        exclude: &[String],
        max_depth: usize,
        depth: usize,
        output: &mut String,
    ) {
        if depth > max_depth {
            return;
        }
        let indent = "  ".repeat(depth);
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        let mut entries: Vec<_> = entries
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if exclude.contains(&name) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                output.push_str(&format!("{}{}/\n", indent, name));
                Self::walk_tree(&path, exclude, max_depth, depth + 1, output);
            } else {
                output.push_str(&format!("{}{}\n", indent, name));
            }
        }
    }

    /// 读取项目配置文件（Cargo.toml / package.json / pyproject.toml 等）
    fn read_project_config(
        root: &Path,
    ) -> (Option<String>, Option<String>) {
        let config_files = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "setup.py",
            "go.mod",
            "Gemfile",
            "pom.xml",
            "build.gradle",
            "CMakeLists.txt",
            "Makefile",
        ];
        for name in &config_files {
            let path = root.join(name);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // 限制长度，避免 prompt 过长
                    let truncated = if content.len() > 3000 {
                        format!("{}... (truncated)", &content[..3000])
                    } else {
                        content
                    };
                    return (
                        Some(name.to_string()),
                        Some(truncated),
                    );
                }
            }
        }
        (None, None)
    }

    /// 读取核心源文件的头部注释（前 20 行）
    fn source_file_headers(
        root: &Path,
        exclude: &[String],
        max_files: usize,
    ) -> String {
        let source_exts = [
            "rs", "py", "js", "ts", "go", "java",
            "kt", "rb", "c", "cpp", "h", "hpp", "swift",
        ];
        let mut result = String::new();
        let mut count = 0;

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| {
                if e.file_type().is_dir() {
                    let name = e.file_name().to_string_lossy();
                    return !exclude.iter().any(|d| d == name.as_ref());
                }
                true
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if count >= max_files {
                break;
            }
            let path = entry.path();
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !source_exts.contains(&ext) {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                let header: String = content.lines()
                    .take(20)
                    .collect::<Vec<_>>()
                    .join("\n");
                let rel_path = path.strip_prefix(root)
                    .unwrap_or(path)
                    .display();
                result.push_str(&format!(
                    "\n### {}\n```\n{}\n```\n",
                    rel_path, header
                ));
                count += 1;
            }
        }

        if result.is_empty() {
            result = "（未找到源文件或无头部注释）".to_string();
        }
        result
    }

    /// 组装 LLM prompt
    fn build_prompt(
        &self,
        ctx: &RepoContext,
        template: &Option<String>,
    ) -> String {
        let base_prompt = format!(
r#"你是一个项目文档生成助手。请根据以下代码仓库的信息，生成一份结构化的项目文档。
文档将保存为 Obsidian Markdown 笔记。

## 仓库基本信息
- 名称: {repo_name}
- 路径: {repo_path}
- 当前分支: {current_branch}
- 语言构成: {language_stats}
- 最近活动: {last_activity}

## 目录结构
```
{directory_tree}
```

## README 内容
{readme_content}

## 项目配置文件
### {config_file_name}
```
{config_file_content}
```

## 核心源文件头部注释
{source_file_headers}

## 最近提交历史
{recent_commits_text}

---

请生成一份 Markdown 格式的项目文档，包含以下章节：

1. **项目概述**：基于 README 和项目配置，总结项目的目的、核心功能和定位
2. **技术栈**：列出使用的主要语言、框架和工具
3. **目录结构说明**：解释主要目录和文件的用途
4. **核心模块**：识别并描述项目的核心模块/组件及其职责
5. **依赖列表**：从项目配置文件中提取关键依赖及其用途
6. **开发状态**：基于最近提交历史，总结当前的开发动态

要求：
- 使用中文撰写
- 保持简洁，每个章节 3-10 行
- 在适当位置使用 Obsidian 标签（如 #project, #{primary_language}）
- 在文档开头添加 YAML frontmatter，包含:
  - title: {repo_name} 项目文档
  - source_repo: {repo_path}
  - generated_at: {current_timestamp}
  - head_commit: {head_hash}
  - tags: [project-doc, {primary_language}]"#,
            repo_name = ctx.repo_name,
            repo_path = ctx.repo_path,
            current_branch = ctx.current_branch,
            language_stats = ctx.language_stats,
            last_activity = ctx.last_activity,
            directory_tree = ctx.directory_tree,
            readme_content = ctx.readme_content,
            config_file_name = ctx.config_file_name,
            config_file_content = ctx.config_file_content,
            source_file_headers = ctx.source_file_headers,
            recent_commits_text = ctx.recent_commits_text,
            head_hash = &ctx.head_hash[..7.min(ctx.head_hash.len())],
            primary_language = ctx.language_stats
                .split(',')
                .next()
                .and_then(|s| s.split(':').next())
                .unwrap_or("unknown")
                .trim()
                .to_lowercase(),
            current_timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        );

        // 附加用户自定义模板
        match template {
            Some(tmpl) => format!(
                "{}\n\n## 用户自定义格式参考\n\n{}",
                base_prompt, tmpl
            ),
            None => base_prompt,
        }
    }

    /// 加载文档模板
    fn load_template(&self) -> Option<String> {
        let path = Path::new(&self.config.doc_template_path);
        std::fs::read_to_string(path).ok()
    }

    /// 后处理：确保文档有 frontmatter
    fn post_process(
        &self,
        content: &str,
        detail: &RepoDetail,
    ) -> String {
        if content.starts_with("---") {
            // 已有 frontmatter，直接使用
            content.to_string()
        } else {
            // 补充 frontmatter
            format!(
                "---\ntitle: {} 项目文档\nsource_repo: {}\ngenerated_at: {}\nhead_commit: {}\ntags:\n  - project-doc\n---\n\n{}",
                detail.base.name,
                detail.base.path.display(),
                Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                &detail.head_hash[..7.min(detail.head_hash.len())],
                content,
            )
        }
    }

    /// 从生成的文档中提取章节标题
    fn extract_sections(&self, content: &str) -> Vec<String> {
        content.lines()
            .filter(|line| line.starts_with("## "))
            .map(|line| line.trim_start_matches("## ").to_string())
            .collect()
    }
}

/// 仓库上下文信息（用于 LLM prompt）
struct RepoContext {
    repo_name: String,
    repo_path: String,
    current_branch: String,
    language_stats: String,
    last_activity: String,
    directory_tree: String,
    readme_content: String,
    config_file_name: String,
    config_file_content: String,
    source_file_headers: String,
    recent_commits_text: String,
    head_hash: String,
}
```

---

### 4.7 VscodeOpener — VSCode 集成器 (`vscode.rs`)

```rust
use std::path::Path;
use std::process::Command;

pub struct VscodeOpener;

/// VSCode 操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VscodeResult {
    pub repo_name: String,
    pub vscode_uri: String,
    pub opened: bool,
    pub message: String,
}

impl VscodeOpener {
    pub fn new() -> Self {
        Self
    }

    /// 生成 VSCode URI
    pub fn make_uri(repo_path: &Path) -> String {
        format!("vscode://file{}", repo_path.display())
    }

    /// 打开仓库
    pub fn open(&self, repo_name: &str, repo_path: &Path) -> VscodeResult {
        let uri = Self::make_uri(repo_path);

        // 尝试通过 URI 唤起 VSCode
        let opened = self.try_open_uri(&uri);

        let message = if opened {
            format!("已在 VSCode 中打开 {}", repo_name)
        } else {
            // 回退：尝试 code 命令
            let fallback = self.try_code_command(repo_path);
            if fallback {
                format!("已通过 code 命令打开 {}", repo_name)
            } else {
                format!(
                    "无法自动打开 VSCode，请手动使用以下链接: {}",
                    uri
                )
            }
        };

        VscodeResult {
            repo_name: repo_name.to_string(),
            vscode_uri: uri,
            opened: opened,
            message,
        }
    }

    /// 通过 URI 唤起 VSCode（跨平台）
    fn try_open_uri(&self, uri: &str) -> bool {
        let result = if cfg!(target_os = "macos") {
            Command::new("open").arg(uri).spawn()
        } else if cfg!(target_os = "linux") {
            Command::new("xdg-open").arg(uri).spawn()
        } else if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", uri])
                .spawn()
        } else {
            return false;
        };

        match result {
            Ok(mut child) => {
                // 不等待子进程完成
                let _ = child.wait();
                true
            }
            Err(e) => {
                tracing::warn!("无法打开 VSCode URI: {}", e);
                false
            }
        }
    }

    /// 回退：使用 code 命令行打开
    fn try_code_command(&self, repo_path: &Path) -> bool {
        match Command::new("code")
            .arg(repo_path.as_os_str())
            .spawn()
        {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("code 命令不可用: {}", e);
                false
            }
        }
    }
}
```

---

### 4.8 RepoWatcher — 仓库状态监控 (`watcher.rs`)

```rust
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub struct RepoWatcher {
    manager: Arc<RepoManager>,
    file_watcher: Arc<FileWatcher>,
    config: CodeRepoConfig,
    /// 每个仓库的 HEAD 文件路径 → 仓库名 的映射
    head_map: RwLock<HashMap<PathBuf, String>>,
    /// notify watcher 实例（保持存活）
    _notify_watcher: RwLock<Option<RecommendedWatcher>>,
}

impl RepoWatcher {
    pub fn new(
        manager: Arc<RepoManager>,
        file_watcher: Arc<FileWatcher>,
        config: CodeRepoConfig,
    ) -> Self {
        Self {
            manager,
            file_watcher,
            config,
            head_map: RwLock::new(HashMap::new()),
            _notify_watcher: RwLock::new(None),
        }
    }

    /// 为仓库设置 .git/HEAD 文件监控
    pub async fn watch_repo(
        &self,
        repo_name: &str,
        repo_path: &Path,
    ) -> Result<(), BrainError> {
        let head_path = repo_path.join(".git").join("HEAD");
        if !head_path.exists() {
            tracing::warn!(
                "仓库 {} 的 .git/HEAD 不存在: {}",
                repo_name,
                head_path.display()
            );
            return Ok(());
        }

        // 记录映射
        {
            let mut map = self.head_map.write().await;
            map.insert(head_path.clone(), repo_name.to_string());
        }

        // 通过 FileWatcher 注册监控
        self.file_watcher.watch_file(&head_path, {
            let manager = self.manager.clone();
            let name = repo_name.to_string();
            let path = repo_path.to_path_buf();
            move || {
                let manager = manager.clone();
                let name = name.clone();
                let path = path.clone();
                tokio::spawn(async move {
                    tracing::info!(
                        "检测到 {} 的 HEAD 变更，刷新缓存",
                        name
                    );
                    manager.refresh_cache(&name, &path).await;
                });
            }
        }).await?;

        tracing::debug!("已监控仓库 {} 的 HEAD: {}",
            repo_name, head_path.display());
        Ok(())
    }

    /// 取消仓库监控
    pub async fn unwatch_repo(&self, repo_name: &str) {
        let mut map = self.head_map.write().await;
        map.retain(|_, v| v != repo_name);
    }

    /// 启动定时刷新任务
    pub async fn start_periodic_refresh(&self) {
        let manager = self.manager.clone();
        let refresh_secs = self.config.refresh_interval_seconds;
        let lang_refresh_secs = self.config.language_refresh_interval_seconds;

        // 元信息定时刷新
        tokio::spawn({
            let manager = manager.clone();
            async move {
                let mut timer = interval(
                    Duration::from_secs(refresh_secs)
                );
                loop {
                    timer.tick().await;
                    if let Ok(repos) = manager.list_from_db().await {
                        for repo in repos {
                            let path = PathBuf::from(&repo.path);
                            manager.refresh_cache(
                                &repo.name, &path
                            ).await;
                        }
                    }
                }
            }
        });

        // 语言统计定时刷新（间隔更长）
        tokio::spawn({
            let manager = manager.clone();
            let exclude = self.config.exclude_dirs.clone();
            let max_files = self.config.language_sample_max_files;
            async move {
                let mut timer = interval(
                    Duration::from_secs(lang_refresh_secs)
                );
                loop {
                    timer.tick().await;
                    if let Ok(repos) = manager.list_from_db().await {
                        for repo in repos {
                            let path = PathBuf::from(&repo.path);
                            let exclude = exclude.clone();
                            tokio::task::spawn_blocking(move || {
                                let langs = LanguageDetector::detect(
                                    &path, &exclude, max_files,
                                );
                                // TODO: 更新 SQLite 缓存中的语言统计
                            });
                        }
                    }
                }
            }
        });

        tracing::info!(
            "定时刷新已启动: 元信息 {}s, 语言统计 {}s",
            refresh_secs, lang_refresh_secs
        );
    }
}
```

---

## 5. SQLite 操作层 (`infra/sqlite_store.rs` 扩展)

### 5.1 迁移脚本 (`migrations/001_init.sql`)

```sql
-- 代码仓库注册信息
CREATE TABLE IF NOT EXISTS code_repos (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    registered_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata    JSON            -- 缓存的元信息 (RepoMetadataCache JSON)
);

-- 笔记与仓库的关联
CREATE TABLE IF NOT EXISTS note_repo_links (
    note_path   TEXT NOT NULL,
    repo_name   TEXT NOT NULL REFERENCES code_repos(name) ON DELETE CASCADE,
    linked_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (note_path, repo_name)
);

-- 索引优化
CREATE INDEX IF NOT EXISTS idx_code_repos_path ON code_repos(path);
CREATE INDEX IF NOT EXISTS idx_note_repo_links_repo ON note_repo_links(repo_name);
CREATE INDEX IF NOT EXISTS idx_note_repo_links_note ON note_repo_links(note_path);
```

### 5.2 关键查询方法

```rust
impl SqliteStore {
    // ─── code_repos 表操作 ─────────────────────────────

    /// 插入新仓库记录
    pub async fn insert_code_repo(
        &self, row: &CodeRepoRow,
    ) -> Result<(), BrainError> {
        self.execute(
            "INSERT INTO code_repos (name, path, registered_at, metadata)
             VALUES (?1, ?2, ?3, ?4)",
            params![row.name, row.path, row.registered_at, row.metadata],
        ).await
    }

    /// 按名称查询仓库
    pub async fn get_code_repo_by_name(
        &self, name: &str,
    ) -> Result<Option<CodeRepoRow>, BrainError> {
        self.query_optional(
            "SELECT name, path, registered_at, metadata
             FROM code_repos WHERE name = ?1",
            params![name],
        ).await
    }

    /// 按路径查询仓库（用于去重检查）
    pub async fn get_code_repo_by_path(
        &self, path: &str,
    ) -> Result<Option<CodeRepoRow>, BrainError> {
        self.query_optional(
            "SELECT name, path, registered_at, metadata
             FROM code_repos WHERE path = ?1",
            params![path],
        ).await
    }

    /// 列出所有仓库
    pub async fn list_code_repos(
        &self,
    ) -> Result<Vec<CodeRepoRow>, BrainError> {
        self.query_all(
            "SELECT name, path, registered_at, metadata
             FROM code_repos ORDER BY registered_at DESC",
            params![],
        ).await
    }

    /// 更新仓库元数据缓存
    pub async fn update_repo_metadata(
        &self, name: &str, metadata_json: &str,
    ) -> Result<(), BrainError> {
        self.execute(
            "UPDATE code_repos SET metadata = ?1 WHERE name = ?2",
            params![metadata_json, name],
        ).await
    }

    /// 删除仓库
    pub async fn delete_code_repo(
        &self, name: &str,
    ) -> Result<(), BrainError> {
        self.execute(
            "DELETE FROM code_repos WHERE name = ?1",
            params![name],
        ).await
    }

    // ─── note_repo_links 表操作 ────────────────────────

    /// 插入关联记录
    pub async fn insert_note_repo_link(
        &self,
        note_path: &str,
        repo_name: &str,
        linked_at: &str,
    ) -> Result<(), BrainError> {
        self.execute(
            "INSERT OR IGNORE INTO note_repo_links
             (note_path, repo_name, linked_at)
             VALUES (?1, ?2, ?3)",
            params![note_path, repo_name, linked_at],
        ).await
    }

    /// 查询笔记与仓库的关联
    pub async fn get_note_repo_link(
        &self,
        note_path: &str,
        repo_name: &str,
    ) -> Result<Option<NoteRepoLinkRow>, BrainError> {
        self.query_optional(
            "SELECT note_path, repo_name, linked_at
             FROM note_repo_links
             WHERE note_path = ?1 AND repo_name = ?2",
            params![note_path, repo_name],
        ).await
    }

    /// 获取仓库关联的所有笔记
    pub async fn get_linked_notes(
        &self, repo_name: &str,
    ) -> Result<Vec<String>, BrainError> {
        let rows = self.query_all(
            "SELECT note_path FROM note_repo_links
             WHERE repo_name = ?1",
            params![repo_name],
        ).await?;
        Ok(rows.into_iter().map(|r| r.note_path).collect())
    }

    /// 统计仓库关联的笔记数
    pub async fn count_note_links(
        &self, repo_name: &str,
    ) -> Result<usize, BrainError> {
        self.query_scalar(
            "SELECT COUNT(*) FROM note_repo_links
             WHERE repo_name = ?1",
            params![repo_name],
        ).await
    }
}
```

---

## 6. 错误处理

### 6.1 错误类型扩展 (`src/error.rs`)

CodeRepo 模块复用顶层设计中定义的 `BrainError`，主要使用以下变体：

```rust
pub enum BrainError {
    // ─── 代码仓相关 ──────────────────────────────
    /// 仓库路径不存在
    RepoNotFound(PathBuf),

    /// Git 操作失败
    GitError {
        path: PathBuf,
        detail: String,
    },

    /// 笔记不存在
    NoteNotFound(PathBuf),

    /// LLM API 调用失败
    LlmApiError {
        provider: String,
        detail: String,
    },

    /// IO 错误
    IoError(std::io::Error),

    /// 内部错误（参数校验失败等）
    Internal(String),
}
```

### 6.2 错误处理策略

| 场景 | 处理方式 | 日志级别 |
|---|---|---|
| 路径无效 / 非 Git 仓库 | 立即返回错误，附带建议信息 | WARN |
| git2 操作失败（单个仓库） | 返回该仓库的错误，不影响其他仓库 | WARN |
| SQLite 写入失败 | 重试 3 次（指数退避），仍失败则返回错误 | ERROR |
| LLM API 调用失败 | 返回错误，建议用户稍后重试或切换模型 | WARN |
| 文件写入失败 | 返回错误，包含权限检查建议 | ERROR |
| 语言统计超时/失败 | 返回空 HashMap，不影响仓库注册 | DEBUG |
| 文件监控设置失败 | 日志告警，不影响主功能（降级为定时刷新） | WARN |
| 仓库路径失效（inactive） | 标记 inactive，列表查询不报错 | INFO |

### 6.3 工具调用错误响应格式

```json
{
  "tool": "add_code_repo",
  "status": "error",
  "error": {
    "code": "NOT_A_GIT_REPO",
    "message": "路径 '/Users/me/projects/not-git' 不是一个有效的 Git 仓库（缺少 .git 目录）",
    "suggestion": "请确认路径指向一个已初始化的 Git 仓库，可运行 'git init' 初始化仓库"
  }
}
```

---

## 7. 性能优化

### 7.1 元信息缓存策略

```
┌──────────────────────────────────────────────────────┐
│                  缓存层次结构                         │
│                                                      │
│  L1: 运行时内存缓存 (RwLock<HashMap>)                 │
│      ├── 读取延迟: < 1ms                             │
│      ├── 存储: CachedRepoInfo（完整元数据）            │
│      └── 更新: HEAD 变更 / 定时刷新 触发              │
│                                                      │
│  L2: SQLite 持久化缓存 (metadata JSON 列)             │
│      ├── 读取延迟: < 5ms                             │
│      ├── 存储: RepoMetadataCache（序列化 JSON）       │
│      └── 更新: 异步写入，不阻塞主流程                 │
│                                                      │
│  L3: git2 实时读取                                    │
│      ├── 读取延迟: 10-100ms                          │
│      ├── 场景: get_repo_detail 强制刷新              │
│      └── 范围: 分支、commit、工作区状态               │
└──────────────────────────────────────────────────────┘
```

**查询路径**:

```
list_code_repos:
  L1(内存) 存在且未过期 → 直接返回
  L1 不存在/过期 → L2(SQLite) 读取 + git2 轻量刷新(分支/is_dirty)
  
get_repo_detail:
  始终走 L3(git2 实时读取) + L2 补充(语言统计/关联笔记)
```

### 7.2 语言统计采样策略

| 优化项 | 措施 |
|---|---|
| 文件数限制 | `max_files` 参数（默认 500），超出后停止遍历 |
| 目录排除 | `exclude_dirs` 配置，跳过 .git/node_modules/target 等大目录 |
| 行数统计 | `read_to_string` 失败时（二进制文件）直接跳过，不计入 |
| 异步执行 | `spawn_blocking` 在 blocking 线程池执行，不阻塞 async runtime |
| 增量刷新 | 语言统计仅在 HEAD 变更（commit hash 变化）时重新计算 |
| 独立刷新周期 | 语言统计刷新间隔（1小时）独立于元信息刷新间隔（5分钟） |

### 7.3 并发安全

```rust
// RepoManager 中的缓存使用 RwLock 保证并发安全
pub struct RepoManager {
    cache: RwLock<HashMap<String, CachedRepoInfo>>,
    // ...
}

// 读操作：共享读锁
pub async fn get_cached(&self, name: &str) -> Option<CachedRepoInfo> {
    let cache = self.cache.read().await;
    cache.get(name).cloned()
}

// 写操作：独占写锁
pub async fn update_cached(&self, name: &str, info: CachedRepoInfo) {
    let mut cache = self.cache.write().await;
    cache.insert(name.to_string(), info);
}
```

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ─── GitExtractor 测试 ──────────────────────────

    #[test]
    fn test_current_branch() {
        // 使用 git2 创建临时仓库
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // 创建初始 commit（否则 HEAD 不存在）
        create_initial_commit(&repo);

        let extractor = GitExtractor::new(&repo);
        let branch = extractor.current_branch().unwrap();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_is_dirty_clean_repo() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_initial_commit(&repo);

        let extractor = GitExtractor::new(&repo);
        assert!(!extractor.is_dirty().unwrap());
    }

    #[test]
    fn test_is_dirty_with_untracked() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_initial_commit(&repo);

        // 创建未追踪文件
        std::fs::write(dir.path().join("new_file.txt"), "hello")
            .unwrap();

        let extractor = GitExtractor::new(&repo);
        assert!(extractor.is_dirty().unwrap());
    }

    #[test]
    fn test_recent_commits() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_initial_commit(&repo);
        create_commit(&repo, "second commit");
        create_commit(&repo, "third commit");

        let extractor = GitExtractor::new(&repo);
        let commits = extractor.recent_commits(5).unwrap();
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].message, "third commit");
    }

    #[test]
    fn test_branches() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_initial_commit(&repo);

        // 创建新分支
        let head = repo.head().unwrap();
        let target = head.target().unwrap();
        let commit = repo.find_commit(target).unwrap();
        repo.branch("feature-a", &commit, false).unwrap();
        repo.branch("feature-b", &commit, false).unwrap();

        let extractor = GitExtractor::new(&repo);
        let branches = extractor.branches().unwrap();
        assert!(branches.len() >= 3); // main + feature-a + feature-b
    }

    #[test]
    fn test_working_dir_status() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        create_initial_commit(&repo);

        // 创建各种状态的文件
        std::fs::write(dir.path().join("new.txt"), "new").unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "modified").unwrap();

        let extractor = GitExtractor::new(&repo);
        let status = extractor.working_dir_status().unwrap();
        assert!(status.untracked > 0 || status.modified > 0);
    }

    // ─── LanguageDetector 测试 ──────────────────────

    #[test]
    fn test_language_detect_rust_project() {
        let dir = TempDir::new().unwrap();
        // 创建 Rust 项目结构
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n"
        ).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n"
        ).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n"
        ).unwrap();

        let result = LanguageDetector::detect(
            dir.path(),
            &vec!["target".to_string()],
            500,
        );

        assert!(result.contains_key("Rust"));
        assert!(result["Rust"] > 0.5); // Rust 应占主导
    }

    #[test]
    fn test_language_detect_exclude_dirs() {
        let dir = TempDir::new().unwrap();
        // 创建 node_modules 中的 JS 文件（应被排除）
        std::fs::create_dir_all(
            dir.path().join("node_modules/pkg")
        ).unwrap();
        std::fs::write(
            dir.path().join("node_modules/pkg/index.js"),
            "console.log('excluded');\n"
        ).unwrap();
        // 创建主文件
        std::fs::write(
            dir.path().join("main.py"),
            "print('hello')\n"
        ).unwrap();

        let result = LanguageDetector::detect(
            dir.path(),
            &vec!["node_modules".to_string()],
            500,
        );

        assert!(result.contains_key("Python"));
        assert!(!result.contains_key("JavaScript")); // 应被排除
    }

    #[test]
    fn test_language_detect_empty_repo() {
        let dir = TempDir::new().unwrap();
        let result = LanguageDetector::detect(
            dir.path(), &vec![], 500,
        );
        assert!(result.is_empty());
    }

    // ─── VscodeOpener 测试 ──────────────────────────

    #[test]
    fn test_make_uri() {
        let path = Path::new("/Users/me/projects/my-app");
        let uri = VscodeOpener::make_uri(path);
        assert_eq!(uri, "vscode://file/Users/me/projects/my-app");
    }

    // ─── NoteLinker 测试 ────────────────────────────

    #[test]
    fn test_extract_keywords() {
        let text = "Rust async programming with tokio runtime";
        let keywords = NoteLinker::extract_keywords(text);
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"async".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(!keywords.contains(&"with".to_string())); // 停用词
    }

    #[test]
    fn test_reference_block_format() {
        let block = format!(
            "\n\n---\n## 🔗 相关代码仓库\n- **my-app** — `/path/to/repo`\n  - [在 VSCode 中打开](vscode://file/path/to/repo)\n  - 最后活动: 2026-05-28 | 分支: main\n"
        );
        assert!(block.contains("## 🔗 相关代码仓库"));
        assert!(block.contains("**my-app**"));
        assert!(block.contains("vscode://file/"));
    }

    // ─── 辅助函数 ──────────────────────────────────

    fn create_initial_commit(repo: &Repository) {
        let sig = repo.signature().unwrap_or(
            git2::Signature::now("Test", "test@test.com").unwrap()
        );
        let tree_id = {
            let mut index = repo.index().unwrap();
            // 创建一个空文件作为初始内容
            let path = repo.workdir().unwrap().join("README.md");
            std::fs::write(&path, "# Test").unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(
            Some("HEAD"), &sig, &sig,
            "initial commit", &tree, &[]
        ).unwrap();
    }

    fn create_commit(repo: &Repository, message: &str) {
        let sig = repo.signature().unwrap_or(
            git2::Signature::now("Test", "test@test.com").unwrap()
        );
        let head = repo.head().unwrap();
        let parent = repo.find_commit(head.target().unwrap()).unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            // 修改或添加文件
            let filename = format!("file_{}.txt",
                message.replace(' ', "_"));
            let path = repo.workdir().unwrap().join(&filename);
            std::fs::write(&path, message).unwrap();
            index.add_path(Path::new(&filename)).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(
            Some("HEAD"), &sig, &sig,
            message, &tree, &[&parent]
        ).unwrap();
    }
}
```

### 8.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 完整注册 → 查询 → 关联 → 文档生成流程
    #[tokio::test]
    async fn test_full_workflow() {
        // 准备：创建临时 Git 仓库和 Obsidian vault
        let repo_dir = TempDir::new().unwrap();
        let vault_dir = TempDir::new().unwrap();
        let db_path = vault_dir.path().join("test.db");

        // 初始化 Git 仓库
        let repo = Repository::init(repo_dir.path()).unwrap();
        create_initial_commit(&repo);

        // 创建 vault 笔记
        std::fs::write(
            vault_dir.path().join("test-note.md"),
            "# Test Note\nSome content about the project.",
        ).unwrap();

        // 初始化服务
        let config = CodeRepoConfig::default();
        let db = Arc::new(SqliteStore::new(&db_path).await.unwrap());
        db.run_migrations().await.unwrap();
        // ... 初始化其他依赖

        // 测试注册
        let result = service.add_code_repo(
            &repo_dir.path().to_string_lossy(),
            "test-repo",
        ).await;
        assert!(result.is_ok());

        // 测试列表
        let cards = service.list_code_repos().await.unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "test-repo");

        // 测试详情
        let detail = service.get_repo_detail("test-repo").await.unwrap();
        assert_eq!(detail.base.name, "test-repo");
        assert!(!detail.branches.is_empty());

        // 测试关联
        let link = service.link_note_to_repo(
            "test-note.md", "test-repo",
        ).await;
        assert!(link.is_ok());

        // 验证笔记内容包含引用块
        let note_content = std::fs::read_to_string(
            vault_dir.path().join("test-note.md")
        ).unwrap();
        assert!(note_content.contains("🔗 相关代码仓库"));
        assert!(note_content.contains("test-repo"));
    }

    /// 重复注册应报错
    #[tokio::test]
    async fn test_duplicate_registration() {
        // ... 准备
        let _ = service.add_code_repo(path, "repo-a").await;
        let result = service.add_code_repo(path, "repo-b").await;
        assert!(result.is_err());
    }

    /// 幂等关联测试
    #[tokio::test]
    async fn test_idempotent_link() {
        // ... 准备
        let _ = service.link_note_to_repo("note.md", "repo").await;
        let _ = service.link_note_to_repo("note.md", "repo").await;

        // 验证引用块只出现一次
        let content = std::fs::read_to_string(note_path).unwrap();
        let count = content.matches("**repo**").count();
        assert_eq!(count, 1);
    }
}
```

### 8.3 测试覆盖目标

| 模块 | 单元测试覆盖 | 关键场景 |
|---|---|---|
| GitExtractor | > 90% | 分支、commit、dirty、空仓库、detached HEAD |
| LanguageDetector | > 85% | 单语言、多语言、排除目录、空仓库、大文件 |
| NoteLinker | > 80% | 关联、去重、引用块格式、关键词提取 |
| DocGenerator | > 70% | prompt 组装、模板加载、后处理 |
| VscodeOpener | > 90% | URI 格式、路径编码 |
| RepoWatcher | > 60% | HEAD 监控、定时刷新（需 mock 时间） |
| RepoManager | > 80% | 注册、列表、详情、缓存刷新 |

---

## 9. 依赖清单

### 9.1 直接依赖 (Cargo.toml)

```toml
[dependencies]
# ─── Git 操作 ────────────────────────────
git2 = "0.19"                    # libgit2 Rust 绑定，核心 Git 操作

# ─── 数据库 ──────────────────────────────
rusqlite = { version = "0.31", features = ["bundled"] }
# 或使用 sqlx:
# sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio"] }

# ─── 异步运行时 ──────────────────────────
tokio = { version = "1", features = ["full"] }

# ─── 文件遍历 ────────────────────────────
walkdir = "2"                    # 递归目录遍历（语言统计使用）

# ─── 文件监控 ────────────────────────────
notify = "6"                     # 文件系统监控

# ─── 序列化 ──────────────────────────────
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# ─── 时间处理 ────────────────────────────
chrono = { version = "0.4", features = ["serde"] }

# ─── 日志 ────────────────────────────────
tracing = "0.1"
tracing-subscriber = "0.3"

# ─── 定时任务 ────────────────────────────
tokio-cron-scheduler = "0.10"    # 定时刷新调度

# ─── 错误处理 ────────────────────────────
thiserror = "1"
anyhow = "1"

# ─── 配置 ────────────────────────────────
config = "0.14"                  # TOML 配置加载

[dev-dependencies]
tempfile = "3"                   # 临时目录（测试用）
mockall = "0.12"                 # Mock 对象（LLM Client 等）
tokio-test = "0.4"               # Tokio 测试辅助
```

### 9.2 依赖关系图

```
CodeRepo 模块依赖:

git2 0.19 ───────────→ libgit2 (C 库，bundled)
rusqlite 0.31 ───────→ SQLite (bundled)
walkdir 2 ───────────→ 标准库
notify 6 ────────────→ inotify (Linux) / FSEvents (macOS) / ReadDirectoryChanges (Windows)
tokio 1 ─────────────→ mio → epoll/kqueue/IOCP
chrono 0.4 ──────────→ 标准库
serde 1 ─────────────→ 标准库
tracing 0.1 ─────────→ 标准库
tokio-cron-scheduler → tokio
config 0.14 ─────────→ serde, toml
```

### 9.3 系统级依赖

| 依赖 | 说明 | 是否必需 |
|---|---|---|
| libgit2 | git2 crate 的底层 C 库，通常 bundled 编译 | 是（自动编译） |
| SQLite | 嵌入式数据库，rusqlite bundled 编译 | 是（自动编译） |
| VSCode | VSCode 集成功能需要 | 否（降级处理） |
| Git CLI | 不需要，git2 直接操作 .git 目录 | 否 |

### 9.4 与其他模块共享的依赖

| 依赖 | 提供方 | CodeRepo 使用方式 |
|---|---|---|
| `SqliteStore` | infra 层 | 仓库元数据持久化、关联记录 |
| `FileWatcher` | infra 层 | .git/HEAD 文件监控 |
| `LlmClient` | infra 层 | 文档生成的 LLM 调用 |
| `TimelineService` | core 层 | 发送仓库事件 |
| `MemoryService` | core 层 | 笔记重新索引通知 |

---

## 10. API Handler 层 (`api/handlers/code_repo.rs`)

```rust
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

/// add_code_repo 请求
#[derive(Deserialize)]
pub struct AddRepoRequest {
    pub path: String,
    pub name: String,
}

/// list_code_repos 响应
#[derive(Serialize)]
pub struct ListReposResponse {
    pub repos: Vec<RepoCard>,
    pub total: usize,
}

/// get_repo_detail 请求
#[derive(Deserialize)]
pub struct GetRepoDetailRequest {
    pub name: String,
}

/// link_note_to_repo 请求
#[derive(Deserialize)]
pub struct LinkNoteRequest {
    pub note_path: String,
    pub repo_name: String,
}

/// generate_docs 请求
#[derive(Deserialize)]
pub struct GenerateDocsRequest {
    pub repo_name: String,
    pub target_path: Option<String>,
}

/// open_in_vscode 请求
#[derive(Deserialize)]
pub struct OpenVscodeRequest {
    pub repo_name: String,
}

// ─── Handler 实现 ────────────────────────────────

pub async fn handle_add_code_repo(
    State(service): State<Arc<CodeRepoService>>,
    Json(req): Json<AddRepoRequest>,
) -> impl IntoResponse {
    match service.add_code_repo(&req.path, &req.name).await {
        Ok(repo) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "tool": "add_code_repo",
                "status": "success",
                "result": repo,
            })),
        ),
        Err(e) => (
            StatusCode::OK, // 工具调用错误仍返回 200，错误在 body 中
            Json(serde_json::json!({
                "tool": "add_code_repo",
                "status": "error",
                "error": {
                    "code": error_code(&e),
                    "message": e.to_string(),
                    "suggestion": error_suggestion(&e),
                },
            })),
        ),
    }
}

pub async fn handle_list_code_repos(
    State(service): State<Arc<CodeRepoService>>,
) -> impl IntoResponse {
    match service.list_code_repos().await {
        Ok(repos) => {
            let total = repos.len();
            (StatusCode::OK, Json(serde_json::json!({
                "tool": "list_code_repos",
                "status": "success",
                "result": { "repos": repos, "total": total },
            })))
        }
        Err(e) => error_response("list_code_repos", &e),
    }
}

pub async fn handle_get_repo_detail(
    State(service): State<Arc<CodeRepoService>>,
    Json(req): Json<GetRepoDetailRequest>,
) -> impl IntoResponse {
    match service.get_repo_detail(&req.name).await {
        Ok(detail) => (StatusCode::OK, Json(serde_json::json!({
            "tool": "get_repo_detail",
            "status": "success",
            "result": detail,
        }))),
        Err(e) => error_response("get_repo_detail", &e),
    }
}

pub async fn handle_link_note_to_repo(
    State(service): State<Arc<CodeRepoService>>,
    Json(req): Json<LinkNoteRequest>,
) -> impl IntoResponse {
    match service.link_note_to_repo(
        &req.note_path, &req.repo_name
    ).await {
        Ok(link) => (StatusCode::OK, Json(serde_json::json!({
            "tool": "link_note_to_repo",
            "status": "success",
            "result": link,
        }))),
        Err(e) => error_response("link_note_to_repo", &e),
    }
}

pub async fn handle_generate_docs(
    State(service): State<Arc<CodeRepoService>>,
    Json(req): Json<GenerateDocsRequest>,
) -> impl IntoResponse {
    match service.generate_docs(
        &req.repo_name,
        req.target_path.as_deref(),
    ).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "tool": "generate_docs",
            "status": "success",
            "result": result,
        }))),
        Err(e) => error_response("generate_docs", &e),
    }
}

pub async fn handle_open_in_vscode(
    State(service): State<Arc<CodeRepoService>>,
    Json(req): Json<OpenVscodeRequest>,
) -> impl IntoResponse {
    match service.open_in_vscode(&req.repo_name).await {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!({
            "tool": "open_in_vscode",
            "status": "success",
            "result": result,
        }))),
        Err(e) => error_response("open_in_vscode", &e),
    }
}

// ─── 辅助函数 ──────────────────────────────────

fn error_response(tool: &str, e: &BrainError) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "tool": tool,
            "status": "error",
            "error": {
                "code": error_code(e),
                "message": e.to_string(),
                "suggestion": error_suggestion(e),
            },
        })),
    )
}

fn error_code(e: &BrainError) -> &'static str {
    match e {
        BrainError::RepoNotFound(_) => "REPO_NOT_FOUND",
        BrainError::NoteNotFound(_) => "NOTE_NOT_FOUND",
        BrainError::GitError { .. } => "GIT_ERROR",
        BrainError::LlmApiError { .. } => "LLM_API_ERROR",
        BrainError::IoError(_) => "IO_ERROR",
        BrainError::Internal(msg) if msg.contains("已被") => "NAME_DUPLICATED",
        BrainError::Internal(msg) if msg.contains("已注册") => "PATH_DUPLICATED",
        BrainError::Internal(msg) if msg.contains("权限") => "PERMISSION_DENIED",
        _ => "INTERNAL_ERROR",
    }
}

fn error_suggestion(e: &BrainError) -> String {
    match e {
        BrainError::RepoNotFound(_) =>
            "请检查路径是否正确，可使用 add_code_repo 重新注册".into(),
        BrainError::NoteNotFound(_) =>
            "请确认笔记路径为 vault 内的相对路径".into(),
        BrainError::GitError { .. } =>
            "请确认路径为合法的 Git 仓库".into(),
        BrainError::LlmApiError { .. } =>
            "请稍后重试，或检查 LLM API 配置".into(),
        _ => String::new(),
    }
}
```

---

## 11. 工具注册 (Tool Registry)

CodeRepo 模块暴露的工具在 Tool Registry 中注册：

```rust
// src/tools/definitions.rs

pub fn code_repo_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "add_code_repo".into(),
            description: "注册一个本地 Git 代码仓库到系统。注册后可通过 list_code_repos 查看、通过 generate_docs 自动生成文档。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "仓库的本地绝对路径"
                    },
                    "name": {
                        "type": "string",
                        "description": "仓库的显示名称（唯一标识）"
                    }
                },
                "required": ["path", "name"]
            }),
        },
        ToolDefinition {
            name: "list_code_repos".into(),
            description: "列出所有已注册的代码仓库及其摘要信息（分支、最新提交、语言构成、工作区状态等）。".into(),
            parameters: json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "get_repo_detail".into(),
            description: "获取指定代码仓库的完整详细信息，包括 commit 历史、分支列表、工作区状态、贡献者等。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "仓库的显示名称"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "link_note_to_repo".into(),
            description: "将 Obsidian 笔记与代码仓库关联。关联后笔记末尾会插入仓库引用块（含 VSCode 打开链接）。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note_path": {
                        "type": "string",
                        "description": "笔记在 vault 内的相对路径"
                    },
                    "repo_name": {
                        "type": "string",
                        "description": "仓库的显示名称"
                    }
                },
                "required": ["note_path", "repo_name"]
            }),
        },
        ToolDefinition {
            name: "generate_docs".into(),
            description: "为代码仓库自动生成项目文档笔记，包含项目概述、技术栈、目录结构、核心模块等。文档保存到 Obsidian vault。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repo_name": {
                        "type": "string",
                        "description": "仓库的显示名称"
                    },
                    "target_path": {
                        "type": "string",
                        "description": "vault 内目标目录路径（默认 code-docs/）"
                    }
                },
                "required": ["repo_name"]
            }),
        },
        ToolDefinition {
            name: "open_in_vscode".into(),
            description: "在 VSCode 中打开指定的代码仓库。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repo_name": {
                        "type": "string",
                        "description": "仓库的显示名称"
                    }
                },
                "required": ["repo_name"]
            }),
        },
    ]
}
```

---

## 12. 实施计划

### 阶段划分

| 阶段 | 内容 | 预估工时 | 优先级 |
|---|---|---|---|
| Phase 2.1 | 数据模型 + SQLite schema + RepoManager 基础 | 2 天 | P0 |
| Phase 2.2 | GitExtractor 全部功能 | 2 天 | P0 |
| Phase 2.3 | LanguageDetector + 集成到 RepoManager | 1 天 | P0 |
| Phase 2.4 | API Handler + Tool 注册 | 1 天 | P0 |
| Phase 2.5 | NoteLinker（手动关联 + 引用块） | 1 天 | P1 |
| Phase 2.6 | VscodeOpener | 0.5 天 | P1 |
| Phase 2.7 | RepoWatcher（HEAD 监控 + 定时刷新） | 1.5 天 | P1 |
| Phase 2.8 | DocGenerator（LLM 文档生成） | 2 天 | P1 |
| Phase 2.9 | NoteLinker 自动建议 | 1 天 | P2 |
| Phase 2.10 | 单元测试 + 集成测试 | 2 天 | P0 |

**总计**: 约 14 天（与顶层设计 Phase 2 的 1-2 周估计一致）

### 实施顺序

```
Phase 2.1 (数据模型 + SQLite)
    │
    ├─→ Phase 2.2 (GitExtractor)
    │       │
    │       └─→ Phase 2.3 (LanguageDetector)
    │               │
    │               └─→ Phase 2.4 (API + Tool 注册)
    │                       │
    │                       ├─→ Phase 2.5 (NoteLinker)
    │                       ├─→ Phase 2.6 (VscodeOpener)
    │                       └─→ Phase 2.7 (RepoWatcher)
    │                               │
    │                               └─→ Phase 2.8 (DocGenerator)
    │                                       │
    │                                       └─→ Phase 2.9 (自动建议)
    │
    └─→ Phase 2.10 (测试，贯穿始终)
```
