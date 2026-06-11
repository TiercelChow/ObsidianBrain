# 基础设施层（Infrastructure）需求设计文档

> **版本**: v0.2 | **最后更新**: 2026-06-12 | **状态**: 设计中  
> **关联文档**: [顶层设计文档](../top_design.md)

---

## 1. 模块概述与定位

### 1.1 定位

基础设施层是 ObsidianBrain 系统的底座，为上层核心服务层（Memory Service、Timeline Service、CodeRepo Service、Inspiration Service、Radar Service）和 API 层提供通用的技术支撑能力。它不包含业务逻辑，而是封装所有与外部系统、存储、IO 相关的交互细节。

### 1.2 职责范围

基础设施层承担以下五大职责：

| 序号 | 子模块 | 核心职责 |
|------|--------|----------|
| 1 | 配置管理 | 应用配置的加载、校验、热重载 |
| 2 | SQLite 元数据存储 | 结构化元数据的持久化与查询 |
| 3 | 日志系统 | 结构化日志记录与输出 |
| 4 | Obsidian REST API 客户端 | 与 Obsidian Local REST API 插件交互，实现笔记的 CRUD 与搜索 |
| 5 | 外部服务客户端 | LLM 调用（OpenAI / Anthropic / Ollama） |

### 1.3 在架构中的位置

```
┌──────────────────────────────────────────────┐
│              API 层 / 工具层                   │
├──────────────────────────────────────────────┤
│              核心服务层                        │
│  (Memory / Timeline / CodeRepo / ...)        │
├──────────────────────────────────────────────┤
│           ▶ 基础设施层（本文档范围）◀           │
│  Config │ SQLite │ Logger │ ObsidianClient   │
│  LLM Client                                 │
├──────────────────────────────────────────────┤
│  外部系统：Obsidian REST API / SQLite / LLM   │
└──────────────────────────────────────────────┘
```

### 1.4 设计原则

- **P-01 隔离性**：上层模块通过 trait 接口使用基础设施，不直接依赖具体实现
- **P-02 可替换性**：每个子模块支持多实现（如 LLM 支持 OpenAI / Anthropic / Ollama），运行时可通过配置切换
- **P-03 容错性**：所有外部调用均有超时、重试、降级策略，单个组件故障不导致系统崩溃
- **P-04 可观测性**：所有关键操作均有 tracing span/log，便于问题排查
- **P-05 低侵入**：不修改外部系统的内部状态，仅通过标准 API 交互

---

## 2. 功能需求

### 2.1 配置管理（Config）

#### FR-C01 TOML 配置加载

系统启动时，从 `config/default.toml` 加载主配置文件，解析为强类型的 Config 结构体。配置项涵盖：

- **server**：HTTP 服务监听地址、端口、协议模式（mcp / http / both）
- **vault**：Obsidian Vault 路径、名称、排除模式列表
- **obsidian**：Obsidian Local REST API 地址、API Key 环境变量名、TLS 证书校验策略
- **llm**：LLM Provider 选择（openai / anthropic / ollama）、模型名称、生成参数
- **memory**：搜索参数（top_k）
- **timeline**：日期格式匹配列表
- **radar**：拉取间隔、相关性阈值、每源最大条目数
- **storage**：SQLite 数据库文件路径
- **logging**：日志级别、日志文件路径

#### FR-C02 多环境配置

支持通过环境变量 `OBSIDIANBRAIN_ENV` 或命令行参数 `--env` 切换配置环境：

- `default`：默认配置（`config/default.toml`）
- `dev`：开发环境（`config/dev.toml`），加载更详细的日志
- `prod`：生产环境（`config/prod.toml`），关闭 debug 日志

多环境配置采用层叠覆盖策略：先加载 `default.toml`，再加载环境特定文件覆盖。环境变量 `OBSIDIANBRAIN_*` 可进一步覆盖文件配置。

#### FR-C03 配置热重载

支持在运行时通过以下方式触发配置重载：

- 监听 `config/` 目录的 TOML 文件变更
- 接收 `SIGHUP` 信号

热重载规则：
- **可热重载项**：日志级别、雷达拉取间隔、搜索参数（top_k）、LLM 参数（temperature、max_tokens）
- **不可热重载项**：服务端口、Vault 路径、数据库路径、Obsidian API 地址（变更需重启）
- 热重载通过内部事件总线通知各消费方，消费方可选择接受或忽略

#### FR-C04 配置校验

配置加载完成后执行校验，校验失败时：
- 在启动阶段：输出详细错误信息并退出（exit code 1）
- 在热重载阶段：记录警告日志，保留旧配置不变

校验规则示例：
- `vault.path` 必须存在且为目录
- `server.port` 范围 1024-65535
- `obsidian.url` 必须为有效的 HTTPS URL
- `obsidian.api_key_env` 对应的环境变量必须存在
- `llm.provider` 必须为枚举值之一

#### FR-C05 敏感信息管理

API Key 等敏感信息不直接写入配置文件，而是通过环境变量名引用。配置中存储的是环境变量名（如 `api_key_env = "OBSIDIAN_API_KEY"`），运行时从环境变量读取实际值。

---

### 2.2 SQLite 元数据存储（SQLite Store）

#### FR-S01 Schema 初始化与迁移

系统使用内嵌的 SQLite 数据库（`./data/brain.db`）存储结构化元数据。需提供：

- **自动建表**：首次启动时自动创建所有必要的表
- **Schema 迁移**：支持版本化的 Schema 迁移机制，每次启动时检查并执行待执行的迁移
- **迁移脚本管理**：迁移脚本存放在 `migrations/` 目录下，命名格式为 `V{version}__{description}.sql`（如 `V001__initial_schema.sql`）

当前 Schema 需包含以下表（详见[顶层设计文档 §4.6](../top_design.md)）：

| 表名 | 用途 |
|------|------|
| `code_repos` | 代码仓库注册信息 |
| `note_repo_links` | 笔记与仓库的关联关系 |
| `radar_items` | 智识雷达条目缓存 |
| `inspiration_history` | 灵感生成历史记录 |
| `timeline_events` | 时间线事件记录 |
| `app_state` | 应用状态与键值元信息（含迁移版本号） |

#### FR-S02 CRUD 操作封装

为上层服务提供类型安全的 CRUD 操作接口：

- **code_repos**：注册/注销/查询/列表/更新元信息
- **note_repo_links**：关联/取消关联/按笔记查询/按仓库查询
- **radar_items**：插入/更新状态/按条件查询/分页列表/去重检查（基于 URL）
- **inspiration_history**：插入/按时间范围查询/按类型查询
- **timeline_events**：插入/按日期范围查询/按事件类型查询
- **app_state**：键值读写（用于存储迁移版本等全局状态）

#### FR-S03 连接管理

- 使用单个 SQLite 连接（SQLite 为嵌入式数据库，无需连接池）
- 启用 WAL（Write-Ahead Logging）模式以提高并发读性能
- 设置合理的 busy_timeout（5 秒），避免锁冲突
- 所有写操作使用事务包裹，保证原子性

#### FR-S04 数据完整性

- 外键约束：`note_repo_links.repo_name` 引用 `code_repos.name`，启用 `PRAGMA foreign_keys = ON`
- 唯一约束：`code_repos.path`、`radar_items.url` 设置唯一索引
- 所有删除操作需考虑级联影响

---

### 2.3 日志系统（Logger）

#### FR-L01 结构化日志

基于 `tracing` + `tracing-subscriber` 构建结构化日志系统：

- 每条日志记录包含：时间戳、日志级别、目标模块、span 上下文、消息内容、结构化字段
- 支持 span 嵌套追踪，例如：`tool_call{tool=search_notes} → obsidian_api{endpoint=/search/}`
- 日志格式支持两种：
  - **pretty**：人类可读格式（开发环境默认）
  - **json**：机器解析格式（生产环境默认）

#### FR-L02 日志级别控制

- 支持全局日志级别配置（trace / debug / info / warn / error）
- 支持按模块设置不同日志级别，例如：
  ```
  RUST_LOG=obsidianbrain=info,obsidianbrain::infra::obsidian_client=debug
  ```
- 支持运行时动态调整日志级别（通过配置热重载或 `SIGHUP` 信号）

#### FR-L03 日志输出目标

- **控制台**：始终输出（开发调试用）
- **文件**：可选输出到 `./data/obsidianbrain.log`
- 日志文件支持轮转（rolling）：
  - 单文件最大 10MB
  - 保留最近 5 个历史文件
  - 使用 `tracing-appender` 实现非阻塞写入

#### FR-L04 请求追踪

每个工具调用请求生成独立的 tracing span，包含：

- `request_id`：UUID v4 唯一标识
- `tool_name`：工具名称
- 请求参数摘要（脱敏）
- 处理耗时
- 响应状态（success / error）

---

### 2.4 Obsidian Local REST API 客户端

#### FR-O01 连接管理

封装与 Obsidian Local REST API 插件的 HTTP 通信：

- **连接建立**：启动时验证 Obsidian REST API 可达（通过 `/vault/` 端点检测）
- **TLS 处理**：Obsidian Local REST API 默认使用自签名证书，客户端需支持跳过证书校验或配置信任证书
- **认证**：通过 `Authorization: Bearer <api_key>` 头进行认证，API Key 从配置的环境变量中读取
- **超时控制**：单次请求超时 30 秒（可配置）
- **重试策略**：网络失败时指数退避重试（最多 3 次，间隔 1s/2s/4s）

#### FR-O02 笔记 CRUD 操作

通过 Obsidian REST API 实现笔记的完整 CRUD：

- **读取笔记**：`GET /vault/{path}` — 获取笔记完整内容（Markdown 原文）
- **写入笔记**：`PUT /vault/{path}` — 创建或覆盖笔记内容
- **追加内容**：`POST /vault/{path}` — 在笔记末尾追加内容
- **删除笔记**：`DELETE /vault/{path}` — 删除指定笔记
- **列表文件**：递归遍历 Vault 目录结构，获取所有 `.md` 文件列表

#### FR-O03 搜索操作

通过 Obsidian REST API 的搜索端点实现笔记搜索：

- **搜索接口**：`POST /search/` — 使用 JsonLogic 查询语法搜索笔记
- **查询格式**：`{"in": ["关键词", {"var": "content"}]}` 实现关键词匹配搜索
- **搜索结果**：返回匹配的笔记路径和上下文信息
- **按标签过滤**：支持在 frontmatter 中按 tags 字段过滤

#### FR-O04 附加功能

- **活跃文件**：`GET /active/` — 获取当前 Obsidian 中打开的文件
- **命令执行**：`POST /commands/{commandId}` — 执行 Obsidian 命令（如打开文件）
- **定期笔记**：`GET /periodic/{type}/` — 获取定期笔记（daily/weekly/monthly）

---

### 2.5 外部服务客户端

#### FR-E01 LLM 调用

封装多 Provider 的 LLM API 调用能力：

- **OpenAI**：调用 Chat Completions API（`gpt-4o-mini`、`gpt-4o` 等）
- **Anthropic Claude**：调用 Messages API（`claude-3-5-sonnet` 等）
- **Ollama**：调用本地 Ollama 服务的 Chat API

通用需求：
- 流式响应：支持 SSE 流式输出，通过 `tokio::sync::mpsc` channel 逐 token 推送
- 非流式响应：一次性返回完整结果
- Token 计数：估算输入/输出 token 数量（用于成本管理和上下文窗口管理）
- 超时控制：单次请求超时 30 秒（可配置）
- 重试策略：速率限制（429）时按 Retry-After 头等待后重试
- 错误分类：区分网络错误、认证错误、速率限制、内容过滤等

---

## 3. 非功能需求

### 3.1 启动性能

| 指标 | 目标 | 测量方式 |
|------|------|----------|
| 冷启动时间 | < 3 秒 | 从进程启动到 HTTP 服务可接受请求 |
| 配置加载 | < 50ms | 从读取文件到 Config 结构体可用 |
| SQLite 初始化 | < 100ms | 从打开数据库到迁移完成 |
| Obsidian API 连通 | < 500ms | 从发起到健康检查通过 |

### 3.2 内存占用

| 状态 | 目标 | 说明 |
|------|------|------|
| 空闲状态 | < 50MB | 仅核心服务运行 |
| 搜索状态 | < 100MB | 执行搜索请求时 |
| 峰值 | < 200MB | 批量处理操作时 |

### 3.3 可靠性

- **数据持久化**：SQLite 写入使用事务，确保数据不丢失
- **SQLite 数据库文件损坏恢复**：提供 `integrity_check` PRAGMA 检查，检测数据库完整性
- **Obsidian API 连接中断**：自动重连，上层操作自动重试
- **进程崩溃恢复**：通过 systemd/launchd 自动重启，SQLite WAL 模式保证数据一致性

### 3.4 安全性

- **网络隔离**：HTTP 服务仅监听 `127.0.0.1`，不暴露到外部网络
- **API Key 保护**：敏感信息不写入配置文件、不写入日志、不出现在错误消息中
- **文件路径安全**：所有 Vault 文件操作通过 Obsidian API 处理，由 Obsidian 负责路径校验
- **SQLite 文件权限**：数据库文件权限设置为 0600（仅所有者可读写）

### 3.5 可测试性

- 所有外部依赖（Obsidian API、LLM API）通过 trait 抽象，支持 mock 测试
- SQLite 操作支持内存数据库（`:memory:`）进行单元测试
- 配置模块支持从字符串/内存加载配置，无需实际文件

---

## 4. 用户故事与使用场景

### US-01 首次部署启动

> **作为**一名新用户，**我希望**启动 ObsidianBrain 后自动连接到 Obsidian，**以便**快速开始使用。

**验收条件**：
1. 确保 Obsidian 已运行且 Local REST API 插件已启用
2. 执行 `cargo run` 后，应用自动完成 SQLite 初始化、验证 Obsidian API 连通性
3. 访问 `http://127.0.0.1:9876/v1/health` 返回 200 OK

### US-02 修改配置无需重启

> **作为**一名用户，**我希望**修改搜索参数（如 top_k）后立即生效，**以便**不用中断正在进行的工作。

**验收条件**：
1. 修改 `config/default.toml` 中 `memory.search_top_k` 的值并保存
2. 系统在 1 秒内检测到文件变更
3. 后续的工具调用使用新的 top_k 值
4. 日志中输出配置热重载的记录

### US-03 通过 Obsidian API 搜索笔记

> **作为**一名 Obsidian 用户，**我希望**通过 LLM 工具搜索 Vault 中的笔记，**以便**快速找到相关内容。

**验收条件**：
1. 调用 `search_notes` 工具，传入查询关键词
2. 系统通过 Obsidian REST API 搜索端点执行搜索
3. 返回匹配的笔记列表及相关上下文
4. 搜索延迟 < 500ms

### US-04 Obsidian API 不可用时的容错

> **作为**一名用户，**我希望**当 Obsidian REST API 不可用时收到清晰的错误提示，**以便**知道问题所在。

**验收条件**：
1. Obsidian 未运行或 REST API 插件未启用
2. 调用 `search_notes` 工具
3. 返回 `OBSIDIAN_API_ERROR` 错误，附带建议（"请确保 Obsidian 正在运行且 Local REST API 插件已启用"）
4. 日志中记录连接失败的详细信息

### US-05 数据库 Schema 升级

> **作为**一名用户，**我希望**升级到新版本后数据库 Schema 自动迁移，**以便**不需要手动操作数据库。

**验收条件**：
1. 使用旧版本创建的 SQLite 数据库
2. 升级到新版本后启动
3. 系统自动检测并执行待执行的迁移脚本
4. 迁移完成后日志记录迁移的版本号
5. 所有数据完整保留

---

## 5. 与其他模块的接口约定

### 5.1 配置管理接口

```rust
/// 配置管理对外接口
pub trait ConfigProvider: Send + Sync {
    /// 获取当前配置（只读引用）
    fn config(&self) -> Arc<AppConfig>;
    
    /// 订阅配置变更通知
    fn subscribe(&self) -> tokio::sync::watch::Receiver<Arc<AppConfig>>;
}
```

**消费方**：所有需要读取配置的模块。

### 5.2 SQLite 存储接口

```rust
/// SQLite 存储对外接口
pub trait MetadataStore: Send + Sync {
    // === code_repos ===
    async fn register_repo(&self, repo: &CodeRepo) -> Result<()>;
    async fn unregister_repo(&self, name: &str) -> Result<()>;
    async fn get_repo(&self, name: &str) -> Result<Option<CodeRepo>>;
    async fn list_repos(&self) -> Result<Vec<CodeRepo>>;
    async fn update_repo_metadata(&self, name: &str, metadata: &serde_json::Value) -> Result<()>;
    
    // === note_repo_links ===
    async fn link_note_to_repo(&self, note_path: &str, repo_name: &str) -> Result<()>;
    async fn unlink_note_from_repo(&self, note_path: &str, repo_name: &str) -> Result<()>;
    async fn get_repos_for_note(&self, note_path: &str) -> Result<Vec<String>>;
    async fn get_notes_for_repo(&self, repo_name: &str) -> Result<Vec<String>>;
    
    // === radar_items ===
    async fn upsert_radar_item(&self, item: &RadarItem) -> Result<()>;
    async fn update_radar_status(&self, id: &str, status: RadarStatus) -> Result<()>;
    async fn query_radar_items(&self, filter: RadarFilter) -> Result<Vec<RadarItem>>;
    async fn radar_item_exists(&self, url: &str) -> Result<bool>;
    
    // === inspiration_history ===
    async fn save_inspiration(&self, record: &InspirationRecord) -> Result<()>;
    async fn query_inspirations(&self, filter: InspirationFilter) -> Result<Vec<InspirationRecord>>;
    
    // === timeline_events ===
    async fn save_timeline_event(&self, event: &TimelineEvent) -> Result<()>;
    async fn query_timeline_events(&self, filter: TimelineFilter) -> Result<Vec<TimelineEvent>>;
    
    // === app_state ===
    async fn get_state(&self, key: &str) -> Result<Option<String>>;
    async fn set_state(&self, key: &str, value: &str) -> Result<()>;
}
```

**消费方**：CodeRepo Service、Radar Service、Inspiration Service、Timeline Service、Memory Service。

### 5.3 Obsidian REST API 客户端接口

```rust
/// Obsidian 文件信息
pub struct ObsidianFileInfo {
    pub path: String,
    pub name: String,
    pub parent: Option<String>,
    pub stat: Option<FileStat>,
}

/// Obsidian 搜索结果
pub struct ObsidianSearchResult {
    pub path: String,
    pub context: String,
}

/// Obsidian REST API 对外接口
#[async_trait]
pub trait ObsidianClient: Send + Sync {
    /// 读取笔记内容
    async fn read_file(&self, path: &str) -> Result<String>;
    
    /// 写入/创建笔记
    async fn write_file(&self, path: &str, content: &str) -> Result<()>;
    
    /// 追加内容到笔记
    async fn append_to_file(&self, path: &str, content: &str) -> Result<()>;
    
    /// 删除笔记
    async fn delete_file(&self, path: &str) -> Result<()>;
    
    /// 列出所有文件
    async fn list_all_files(&self) -> Result<Vec<ObsidianFileInfo>>;
    
    /// 搜索笔记（JsonLogic 查询）
    async fn search(&self, query: &serde_json::Value) -> Result<Vec<ObsidianSearchResult>>;
    
    /// 获取当前活跃文件
    async fn get_active_file(&self) -> Result<Option<String>>;
    
    /// 健康检查 — 验证 API 可达
    async fn health_check(&self) -> Result<bool>;
}
```

**消费方**：Memory Service（笔记搜索与 CRUD）、Radar Service（文章纳藏）、Inspiration Service（素材查询）。

### 5.4 LLM 客户端接口

```rust
/// LLM 请求
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// LLM 响应
pub struct LlmResponse {
    pub content: String,
    pub usage: TokenUsage,
    pub finish_reason: String,
}

/// LLM 调用对外接口
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 非流式调用
    async fn chat(&self, request: &LlmRequest) -> Result<LlmResponse>;
    
    /// 流式调用，返回接收 token 流的 channel
    async fn chat_stream(&self, request: &LlmRequest) -> Result<tokio::sync::mpsc::Receiver<LlmStreamChunk>>;
}
```

**消费方**：Inspiration Service（灵感生成）、CodeRepo Service（文档生成）、Skill Engine（技能编排中的 LLM 步骤）。

---

## 6. 约束与假设

### 6.1 约束

| 编号 | 约束 | 说明 |
|------|------|------|
| CON-01 | 单进程部署 | 整个 ObsidianBrain 运行为单个 Rust 进程，不使用多进程架构 |
| CON-02 | 本地运行 | 所有组件运行在用户本地机器上，不依赖云服务（LLM API 除外） |
| CON-03 | Rust 语言 | 全部使用 Rust 实现，不引入 FFI 依赖 |
| CON-04 | SQLite 单连接 | SQLite 使用单连接模式，不支持多进程并发写入 |
| CON-05 | Obsidian 依赖 | Obsidian 应用必须运行且 Local REST API 插件已启用 |
| CON-06 | 仅监听 127.0.0.1 | HTTP 服务不暴露到公网 |

### 6.2 假设

| 编号 | 假设 | 说明 |
|------|------|------|
| ASM-01 | Obsidian 已安装并运行 | 用户已安装 Obsidian 并启用 Local REST API 社区插件 |
| ASM-02 | Vault 为本地目录 | Obsidian Vault 为本地文件系统上的目录 |
| ASM-03 | 笔记为 Markdown | Vault 中的笔记文件为 `.md` 格式的 Markdown 文件 |
| ASM-04 | 笔记规模适中 | 个人用户的笔记数量在 10,000 篇以内，总文本量 < 500MB |
| ASM-05 | 网络可用 | 使用 LLM API 时需要网络连接可用 |
| ASM-06 | 磁盘空间充足 | 用户机器至少有 500MB 可用磁盘空间用于数据库 |

---

## 7. 验收标准

### 7.1 配置管理验收

- [ ] AC-C01：从 `config/default.toml` 加载配置，所有字段正确解析
- [ ] AC-C02：缺失必填配置项时，输出清晰的错误信息并退出
- [ ] AC-C03：配置文件变更后，可热重载的字段在 1 秒内生效
- [ ] AC-C04：不可热重载的字段变更时，输出警告日志
- [ ] AC-C05：环境变量覆盖文件配置的优先级正确

### 7.2 SQLite 存储验收

- [ ] AC-S01：首次启动时自动创建所有表和索引
- [ ] AC-S02：重启后数据完整保留
- [ ] AC-S03：迁移脚本按版本顺序执行，不重复执行
- [ ] AC-S04：所有 CRUD 操作在事务中执行，中途失败数据不会损坏
- [ ] AC-S05：外键约束生效，删除 code_repos 时关联的 note_repo_links 被级联处理

### 7.3 日志系统验收

- [ ] AC-L01：日志输出到控制台和文件（当配置了文件路径时）
- [ ] AC-L02：日志包含时间戳、级别、模块、消息等结构化字段
- [ ] AC-L03：tracing span 正确嵌套，可追踪请求的完整调用链
- [ ] AC-L04：日志文件超过 10MB 时自动轮转
- [ ] AC-L05：日志级别可通过配置和环境变量控制

### 7.4 Obsidian API 客户端验收

- [ ] AC-O01：成功连接 Obsidian REST API 并读取笔记内容
- [ ] AC-O02：搜索功能正确返回匹配笔记
- [ ] AC-O03：笔记 CRUD 操作（创建/读取/追加/删除）均正确执行
- [ ] AC-O04：Obsidian API 不可用时返回清晰的错误信息
- [ ] AC-O05：自签名 TLS 证书正确处理

### 7.5 外部服务客户端验收

- [ ] AC-E01：LLM 非流式调用正确返回完整结果
- [ ] AC-E02：LLM 流式调用正确逐 token 推送
- [ ] AC-E03：LLM API 超时时自动重试
- [ ] AC-E04：所有外部服务错误均有清晰的错误日志

### 7.6 性能验收

- [ ] AC-P01：冷启动时间 < 3 秒
- [ ] AC-P02：空闲状态内存 < 50MB
- [ ] AC-P03：Obsidian API 搜索延迟 < 500ms
- [ ] AC-P04：LLM 调用延迟 < 30 秒
