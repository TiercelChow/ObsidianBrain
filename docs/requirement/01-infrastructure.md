# 基础设施层（Infrastructure）需求设计文档

> **版本**: v0.1 | **最后更新**: 2026-05-29 | **状态**: 设计中  
> **关联文档**: [顶层设计文档](../top_design.md)

---

## 1. 模块概述与定位

### 1.1 定位

基础设施层是 ObsidianBrain 系统的底座，为上层核心服务层（Memory Service、Timeline Service、CodeRepo Service、Inspiration Service、Radar Service）和 API 层提供通用的技术支撑能力。它不包含业务逻辑，而是封装所有与外部系统、存储、IO 相关的交互细节。

### 1.2 职责范围

基础设施层承担以下六大职责：

| 序号 | 子模块 | 核心职责 |
|------|--------|----------|
| 1 | 配置管理 | 应用配置的加载、校验、热重载 |
| 2 | SQLite 元数据存储 | 结构化元数据的持久化与查询 |
| 3 | 日志系统 | 结构化日志记录与输出 |
| 4 | 文件监控 | Obsidian Vault 文件变更的实时感知 |
| 5 | Docker Compose 编排 | Qdrant 等外部依赖的容器化部署 |
| 6 | 外部服务客户端 | Embedding 生成、LLM 调用、Qdrant 向量操作、Tantivy 全文索引 |

### 1.3 在架构中的位置

```
┌──────────────────────────────────────────────┐
│              API 层 / 工具层                   │
├──────────────────────────────────────────────┤
│              核心服务层                        │
│  (Memory / Timeline / CodeRepo / ...)        │
├──────────────────────────────────────────────┤
│           ▶ 基础设施层（本文档范围）◀           │
│  Config │ SQLite │ Logger │ FileWatcher       │
│  Embedding │ LLM │ Qdrant │ Tantivy          │
├──────────────────────────────────────────────┤
│  外部系统：文件系统 / SQLite / Qdrant / API    │
└──────────────────────────────────────────────┘
```

### 1.4 设计原则

- **P-01 隔离性**：上层模块通过 trait 接口使用基础设施，不直接依赖具体实现
- **P-02 可替换性**：每个子模块支持多实现（如 Embedding 支持 OpenAI / Ollama / ONNX），运行时可通过配置切换
- **P-03 容错性**：所有外部调用均有超时、重试、降级策略，单个组件故障不导致系统崩溃
- **P-04 可观测性**：所有关键操作均有 tracing span/log，便于问题排查
- **P-05 低侵入**：不修改外部系统的内部状态，仅通过标准 API 交互

---

## 2. 功能需求

### 2.1 配置管理（Config）

#### FR-C01 TOML 配置加载

系统启动时，从 `config/default.toml` 加载主配置文件，解析为强类型的 Config 结构体。配置项涵盖：

- **server**：HTTP 服务监听地址、端口、协议模式（mcp / http / both）
- **vault**：Obsidian Vault 路径、名称、文件监控开关、排除模式列表
- **qdrant**：Qdrant 服务地址、collection 名称、向量维度
- **embedding**：Embedding Provider 选择（openai / ollama / onnx）、模型名称、API Key 环境变量名
- **llm**：LLM Provider 选择（openai / anthropic / ollama）、模型名称、生成参数
- **memory**：分块策略参数（最小/最大 token 数）、搜索参数（top_k、RRF k 值）
- **timeline**：日期格式匹配列表
- **radar**：拉取间隔、相关性阈值、每源最大条目数
- **storage**：SQLite 数据库文件路径、Tantivy 索引目录路径
- **logging**：日志级别、日志文件路径

#### FR-C02 多环境配置

支持通过环境变量 `OBSIDIANBRAIN_ENV` 或命令行参数 `--env` 切换配置环境：

- `default`：默认配置（`config/default.toml`）
- `dev`：开发环境（`config/dev.toml`），加载更详细的日志
- `prod`：生产环境（`config/prod.toml`），关闭 debug 日志

多环境配置采用层叠覆盖策略：先加载 `default.toml`，再加载环境特定文件覆盖。环境变量 `OBSIDIANBRAIN_*` 可进一步覆盖文件配置。

#### FR-C03 配置热重载

支持在运行时通过以下方式触发配置重载：

- 监听 `config/` 目录的 TOML 文件变更（复用文件监控模块）
- 接收 `SIGHUP` 信号

热重载规则：
- **可热重载项**：日志级别、雷达拉取间隔、搜索参数（top_k、rrf_k）、LLM 参数（temperature、max_tokens）
- **不可热重载项**：服务端口、Vault 路径、数据库路径、Qdrant 地址（变更需重启）
- 热重载通过内部事件总线通知各消费方，消费方可选择接受或忽略

#### FR-C04 配置校验

配置加载完成后执行校验，校验失败时：
- 在启动阶段：输出详细错误信息并退出（exit code 1）
- 在热重载阶段：记录警告日志，保留旧配置不变

校验规则示例：
- `vault.path` 必须存在且为目录
- `server.port` 范围 1024-65535
- `qdrant.vector_size` 必须为正整数
- `embedding.provider` 必须为枚举值之一
- 当 `embedding.provider = "openai"` 时，`embedding.api_key_env` 对应的环境变量必须存在

#### FR-C05 敏感信息管理

API Key 等敏感信息不直接写入配置文件，而是通过环境变量名引用。配置中存储的是环境变量名（如 `api_key_env = "OPENAI_API_KEY"`），运行时从环境变量读取实际值。

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
| `app_state` | 应用状态与键值元信息（含迁移版本号） |

#### FR-S02 CRUD 操作封装

为上层服务提供类型安全的 CRUD 操作接口：

- **code_repos**：注册/注销/查询/列表/更新元信息
- **note_repo_links**：关联/取消关联/按笔记查询/按仓库查询
- **radar_items**：插入/更新状态/按条件查询/分页列表/去重检查（基于 URL）
- **inspiration_history**：插入/按时间范围查询/按类型查询
- **app_state**：键值读写（用于存储迁移版本、最后索引时间等全局状态）

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
- 支持 span 嵌套追踪，例如：`tool_call{tool=search_memory} → search{backend=qdrant} → embedding{provider=openai}`
- 日志格式支持两种：
  - **pretty**：人类可读格式（开发环境默认）
  - **json**：机器解析格式（生产环境默认）

#### FR-L02 日志级别控制

- 支持全局日志级别配置（trace / debug / info / warn / error）
- 支持按模块设置不同日志级别，例如：
  ```
  RUST_LOG=obsidianbrain=info,obsidianbrain::infra::file_watcher=debug
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

### 2.4 文件监控（File Watcher）

#### FR-F01 Vault 文件变更监听

使用 `notify` crate 监听 Obsidian Vault 目录的文件系统事件：

- 监听的事件类型：文件创建、文件修改、文件删除、文件重命名
- 仅监听 `.md` 文件（Markdown 笔记）
- 支持配置排除模式（glob 匹配），默认排除：`.obsidian/`、`templates/`、`.trash/`
- 递归监听 Vault 目录下所有子目录

#### FR-F02 事件防抖

文件系统事件通常会在短时间内产生多个（如编辑器保存文件时可能触发多次写入事件）。需提供防抖机制：

- 防抖窗口：300ms（可配置）
- 对同一路径的连续事件合并为单个事件
- 事件合并规则：
  - Create + Modify → Create
  - Modify + Modify → Modify
  - Modify + Remove → Remove
  - Rename(src, dst) → 生成 Remove(src) + Create(dst)

#### FR-F03 变更类型枚举

对外暴露统一的变更事件类型：

| 变更类型 | 含义 | 触发场景 |
|----------|------|----------|
| `Created` | 新文件创建 | 用户在 Obsidian 中新建笔记 |
| `Modified` | 文件内容修改 | 用户编辑并保存笔记 |
| `Removed` | 文件被删除 | 用户删除笔记 |
| `Renamed` | 文件重命名/移动 | 用户重命名或移动笔记 |

每个变更事件需携带：
- 文件路径（PathBuf）
- 变更类型
- 事件时间戳
- 对于 Renamed：旧路径和新路径

#### FR-F04 回调注册机制

支持上层服务注册变更事件的回调函数：

- 支持多个回调订阅者（Memory Service、Timeline Service 等可同时订阅）
- 回调以异步方式执行（tokio spawn）
- 单个回调失败不影响其他回调的执行
- 回调按注册顺序串行执行（避免并发写入冲突）

#### FR-F05 启动时全量扫描

文件监控模块启动时，需执行一次全量扫描：

- 遍历 Vault 目录下所有 `.md` 文件
- 将文件列表与 SQLite 中记录的已索引文件列表对比
- 识别出新增、修改（基于 mtime 比较）、删除的文件
- 生成对应的变更事件，推送给回调订阅者
- 全量扫描完成后切换到实时监控模式

#### FR-F06 监控容错

- 当底层 notify watcher 出错（如文件系统权限变更）时，自动尝试重连
- 重连间隔采用指数退避策略（1s → 2s → 4s → 8s → 最大 60s）
- 重连成功后执行一次全量扫描以补偿遗漏的事件
- 所有异常均记录到日志系统

---

### 2.5 Docker Compose 编排

#### FR-D01 Qdrant 容器

提供 `docker-compose.yml` 文件，编排 Qdrant 向量数据库容器：

- 镜像：`qdrant/qdrant:latest`（建议锁定具体版本）
- 端口映射：`127.0.0.1:6333:6333`（REST API）
- 数据持久化：`./data/qdrant_storage:/qdrant/storage`
- 资源限制：内存上限 2GB，CPU 上限 2 核
- 健康检查：`curl http://localhost:6333/healthz`，每 10 秒检测一次
- 自动重启策略：`restart: unless-stopped`

#### FR-D02 一键启动

- `docker compose up -d`：后台启动所有依赖服务
- `docker compose down`：停止所有服务
- 提供 `Makefile` 或 `justfile` 封装常用命令：
  - `make dev`：启动 Docker 依赖 + cargo run
  - `make test`：启动 Docker 依赖 + cargo test
  - `make clean`：清理数据目录和 Docker 卷

---

### 2.6 外部服务客户端

#### FR-E01 Embedding 生成

为文本内容生成向量表示，支持多 Provider：

- **OpenAI**：调用 `https://api.openai.com/v1/embeddings`，支持 `text-embedding-3-small`（1536 维）和 `text-embedding-3-large`（3072 维）
- **Ollama**：调用本地 Ollama 服务 `http://127.0.0.1:11434/api/embeddings`，支持 `nomic-embed-text` 等模型
- **ONNX（预留）**：加载本地 ONNX 模型文件进行推理，无需网络调用

通用需求：
- 批量处理：支持一次调用处理多条文本（OpenAI API 原生支持，Ollama 需循环调用）
- 重试策略：网络失败时指数退避重试（最多 3 次，间隔 1s/2s/4s）
- 超时控制：单次 API 调用超时 30 秒
- 维度一致性：同一 collection 内所有向量维度必须一致，启动时校验

#### FR-E02 LLM 调用

封装多 Provider 的 LLM API 调用能力：

- **OpenAI**：调用 Chat Completions API（`gpt-4o-mini`、`gpt-4o` 等）
- **Anthropic Claude**：调用 Messages API（`claude-3-5-sonnet` 等）
- **Ollama**：调用本地 Ollama 服务的 Chat API

通用需求：
- 流式响应：支持 SSE 流式输出，通过 `tokio::sync::mpsc` channel 逐 token 推送
- 非流式响应：一次性返回完整结果
- Token 计数：估算输入/输出 token 数量（用于成本统计和上下文窗口管理）
- 超时控制：单次请求超时 30 秒（可配置）
- 重试策略：速率限制（429）时按 Retry-After 头等待后重试
- 错误分类：区分网络错误、认证错误、速率限制、内容过滤等

#### FR-E03 Qdrant 向量操作

封装 Qdrant REST/gRPC API 的客户端操作：

- **连接管理**：启动时建立连接，验证 Qdrant 服务可达
- **Collection 操作**：创建 collection（指定向量维度和距离度量）、检查 collection 是否存在、获取 collection 信息
- **向量写入**：upsert（插入或更新）向量及其 payload
- **向量搜索**：按向量相似度搜索（支持过滤条件、top_k 参数）
- **向量删除**：按 ID 或过滤条件删除向量
- **Payload 管理**：更新向量的 payload 字段

#### FR-E04 Tantivy 全文索引

封装 Tantivy 搜索引擎的操作：

- **Schema 定义**：定义笔记索引的字段结构（title、content、path、tags、created_at、updated_at）
- **中文分词**：集成 jieba-rs 作为中文分词器，支持自定义词典（从 vault 标签生成）
- **索引写入**：添加/更新/删除文档
- **搜索查询**：构建查询（全文搜索、布尔查询、短语查询、模糊查询）
- **索引维护**：定期合并段（segment merge）以优化搜索性能

---

## 3. 非功能需求

### 3.1 启动性能

| 指标 | 目标 | 测量方式 |
|------|------|----------|
| 冷启动时间 | < 3 秒（不含全量索引） | 从进程启动到 HTTP 服务可接受请求 |
| 配置加载 | < 50ms | 从读取文件到 Config 结构体可用 |
| SQLite 初始化 | < 100ms | 从打开数据库到迁移完成 |
| Qdrant 连接 | < 500ms | 从发起到健康检查通过 |
| Tantivy 索引打开 | < 200ms | 从加载索引目录到可搜索 |

### 3.2 内存占用

| 状态 | 目标 | 说明 |
|------|------|------|
| 空闲状态 | < 100MB | 不含 Qdrant 容器 |
| 索引状态 | < 300MB | 10,000 篇笔记全量索引后 |
| 峰值 | < 500MB | 批量处理文件变更时 |

### 3.3 可靠性

- **数据持久化**：SQLite 写入使用事务，确保数据不丢失
- **SQLite 数据库文件损坏恢复**：提供 `integrity_check` PRAGMA 检查，检测数据库完整性
- **Qdrant 连接中断**：自动重连，上层操作自动重试
- **文件监控中断**：自动重连 + 全量扫描补偿
- **进程崩溃恢复**：通过 systemd/launchd 自动重启，SQLite WAL 模式保证数据一致性

### 3.4 安全性

- **网络隔离**：HTTP 服务仅监听 `127.0.0.1`，不暴露到外部网络
- **API Key 保护**：敏感信息不写入配置文件、不写入日志、不出现在错误消息中
- **文件路径安全**：所有 Vault 文件操作限制在配置的 vault.path 内，防止路径穿越
- **SQLite 文件权限**：数据库文件权限设置为 0600（仅所有者可读写）

### 3.5 可测试性

- 所有外部依赖（Qdrant、LLM API、Embedding API）通过 trait 抽象，支持 mock 测试
- SQLite 操作支持内存数据库（`:memory:`）进行单元测试
- 文件监控模块支持注入临时目录进行集成测试
- 配置模块支持从字符串/内存加载配置，无需实际文件

---

## 4. 用户故事与使用场景

### US-01 开发者首次部署

> **作为**一名新用户，**我希望**通过 `docker compose up` 一键启动所有依赖服务，**以便**快速开始使用 ObsidianBrain。

**验收条件**：
1. 执行 `docker compose up -d` 后，Qdrant 容器自动启动并通过健康检查
2. 执行 `cargo run` 后，应用自动完成 SQLite 初始化、Tantivy 索引创建、Qdrant collection 创建
3. 访问 `http://127.0.0.1:9876/v1/health` 返回 200 OK

### US-02 修改配置无需重启

> **作为**一名用户，**我希望**修改搜索参数（如 top_k）后立即生效，**以便**不用中断正在进行的工作。

**验收条件**：
1. 修改 `config/default.toml` 中 `memory.search_top_k` 的值并保存
2. 系统在 1 秒内检测到文件变更
3. 后续的工具调用使用新的 top_k 值
4. 日志中输出配置热重载的记录

### US-03 Vault 笔记变更自动索引

> **作为**一名 Obsidian 用户，**我希望**在 Obsidian 中新建或修改笔记后，系统自动更新索引，**以便**搜索结果始终包含最新内容。

**验收条件**：
1. 在 Vault 中新建一篇 `.md` 笔记
2. 300ms 防抖窗口后，系统开始处理
3. 500ms 内完成 Tantivy 索引更新和 Qdrant 向量写入
4. 通过 `search_notes` 工具可搜索到新笔记

### US-04 Qdrant 不可用时的降级搜索

> **作为**一名用户，**我希望**即使 Qdrant 服务不可用，系统仍能提供全文搜索结果，**以便**不会因为单个组件故障而完全丧失搜索能力。

**验收条件**：
1. 手动停止 Qdrant 容器
2. 调用 `search_notes` 工具
3. 返回基于 Tantivy 全文搜索的结果
4. 日志中记录 Qdrant 不可用的警告信息

### US-05 Embedding API 失败时的容错

> **作为**一名用户，**我希望**即使 Embedding API 调用失败，笔记仍然能被全文索引，**以便**不影响基本的搜索功能。

**验收条件**：
1. 配置错误的 OpenAI API Key
2. 新增一篇笔记
3. Tantivy 全文索引正常更新
4. Qdrant 向量写入被跳过
5. 日志中记录 Embedding 失败的错误信息
6. `search_notes` 仍可返回全文搜索结果

### US-06 首次启动大量笔记索引

> **作为**一名已有大量 Obsidian 笔记的用户，**我希望**系统首次启动时能自动扫描并索引所有笔记，**以便**我可以立即搜索历史内容。

**验收条件**：
1. 配置包含 1000 篇笔记的 Vault 路径
2. 系统启动后执行全量扫描
3. 日志中显示扫描进度（已处理 X / 总共 Y 篇）
4. 所有笔记被正确分块、向量化、索引
5. 索引过程中 HTTP 服务可正常接受请求

### US-07 数据库 Schema 升级

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
    
    // === app_state ===
    async fn get_state(&self, key: &str) -> Result<Option<String>>;
    async fn set_state(&self, key: &str, value: &str) -> Result<()>;
}
```

**消费方**：CodeRepo Service、Radar Service、Inspiration Service、Memory Service。

### 5.3 文件监控接口

```rust
/// 文件变更事件
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub timestamp: DateTime<Utc>,
    pub old_path: Option<PathBuf>,  // 仅 Renamed 时有值
}

pub enum FileChangeType {
    Created,
    Modified,
    Removed,
    Renamed,
}

/// 文件监控对外接口
pub trait FileWatcherService: Send + Sync {
    /// 注册变更事件回调
    fn on_change(&self, callback: Box<dyn Fn(FileChangeEvent) + Send + Sync>);
    
    /// 启动监控（含全量扫描）
    async fn start(&self) -> Result<()>;
    
    /// 停止监控
    async fn stop(&self) -> Result<()>;
}
```

**消费方**：Memory Service（索引更新）、Timeline Service（事件记录）。

### 5.4 Embedding 接口

```rust
/// Embedding 生成对外接口
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// 生成单条文本的向量
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    
    /// 批量生成向量
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    
    /// 获取向量维度
    fn dimension(&self) -> usize;
    
    /// 获取 Provider 名称
    fn provider_name(&self) -> &str;
}
```

**消费方**：Memory Service（记忆向量化）、Radar Service（文章向量化）。

### 5.5 LLM 客户端接口

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

### 5.6 Qdrant 客户端接口

```rust
/// 向量搜索请求
pub struct VectorSearchRequest {
    pub vector: Vec<f32>,
    pub top_k: usize,
    pub filter: Option<QdrantFilter>,
    pub with_payload: bool,
}

/// 向量搜索结果
pub struct VectorSearchResult {
    pub id: String,
    pub score: f32,
    pub payload: HashMap<String, serde_json::Value>,
}

/// Qdrant 操作对外接口
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn ensure_collection(&self, name: &str, dimension: usize) -> Result<()>;
    async fn upsert(&self, collection: &str, id: &str, vector: Vec<f32>, payload: HashMap<String, serde_json::Value>) -> Result<()>;
    async fn search(&self, collection: &str, request: &VectorSearchRequest) -> Result<Vec<VectorSearchResult>>;
    async fn delete(&self, collection: &str, ids: &[String]) -> Result<()>;
    async fn delete_by_filter(&self, collection: &str, filter: QdrantFilter) -> Result<()>;
    async fn collection_info(&self, collection: &str) -> Result<CollectionInfo>;
}
```

**消费方**：Memory Service（向量存储与搜索）、Radar Service（文章向量存储）。

### 5.7 Tantivy 索引接口

```rust
/// 索引文档
pub struct IndexDocument {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 搜索结果
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub doc: IndexDocument,
    pub highlights: Vec<String>,  // 匹配片段高亮
}

/// Tantivy 全文索引对外接口
#[async_trait]
pub trait FullTextIndex: Send + Sync {
    async fn index_document(&self, doc: &IndexDocument) -> Result<()>;
    async fn update_document(&self, doc: &IndexDocument) -> Result<()>;
    async fn delete_document(&self, id: &str) -> Result<()>;
    async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>>;
    async fn search_with_filter(&self, query: &str, tags: &[String], top_k: usize) -> Result<Vec<SearchResult>>;
    async fn document_count(&self) -> Result<usize>;
}
```

**消费方**：Memory Service（全文搜索，与 Qdrant 语义搜索结果做 RRF 融合）。

---

## 6. 约束与假设

### 6.1 约束

| 编号 | 约束 | 说明 |
|------|------|------|
| CON-01 | 单进程部署 | 整个 ObsidianBrain 运行为单个 Rust 进程，不使用多进程架构 |
| CON-02 | 本地运行 | 所有组件运行在用户本地机器上，不依赖云服务（Embedding API 除外） |
| CON-03 | Rust 语言 | 全部使用 Rust 实现，不引入 FFI 依赖（ONNX Runtime 除外） |
| CON-04 | SQLite 单连接 | SQLite 使用单连接模式，不支持多进程并发写入 |
| CON-05 | Qdrant 外部依赖 | Qdrant 作为独立 Docker 容器运行，非嵌入模式 |
| CON-06 | 仅监听 127.0.0.1 | HTTP 服务不暴露到公网 |

### 6.2 假设

| 编号 | 假设 | 说明 |
|------|------|------|
| ASM-01 | Docker 已安装 | 用户机器上已安装 Docker 和 Docker Compose |
| ASM-02 | Vault 为本地目录 | Obsidian Vault 为本地文件系统上的目录，非远程同步盘 |
| ASM-03 | 笔记为 Markdown | Vault 中的笔记文件为 `.md` 格式的 Markdown 文件 |
| ASM-04 | 笔记规模适中 | 个人用户的笔记数量在 10,000 篇以内，总文本量 < 500MB |
| ASM-05 | 网络可用 | 使用 OpenAI Embedding/LLM 时需要网络连接可用 |
| ASM-06 | 磁盘空间充足 | 用户机器至少有 2GB 可用磁盘空间用于索引和数据库 |

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

### 7.4 文件监控验收

- [ ] AC-F01：在 Vault 中新建 `.md` 文件，300ms 后触发 Created 事件
- [ ] AC-F02：快速连续修改同一文件，仅触发一次 Modified 事件
- [ ] AC-F03：排除模式匹配的文件（如 `.obsidian/workspace.json`）不触发事件
- [ ] AC-F04：多个回调订阅者都能收到事件
- [ ] AC-F05：全量扫描正确识别新增、修改、删除的文件

### 7.5 Docker Compose 验收

- [ ] AC-D01：`docker compose up -d` 成功启动 Qdrant 容器
- [ ] AC-D02：Qdrant 容器健康检查通过
- [ ] AC-D03：Qdrant 数据持久化到 `./data/qdrant_storage`
- [ ] AC-D04：容器重启后数据不丢失

### 7.6 外部服务客户端验收

- [ ] AC-E01：Embedding 批量生成正确返回对应数量的向量
- [ ] AC-E02：Embedding API 超时时自动重试，3 次失败后跳过
- [ ] AC-E03：LLM 流式调用正确逐 token 推送
- [ ] AC-E04：Qdrant upsert + search 流程正确
- [ ] AC-E05：Qdrant 连接中断后自动重连
- [ ] AC-E06：Tantivy 中文搜索返回正确结果
- [ ] AC-E07：所有外部服务错误均有清晰的错误日志

### 7.7 性能验收

- [ ] AC-P01：冷启动时间 < 3 秒
- [ ] AC-P02：空闲状态内存 < 100MB
- [ ] AC-P03：全文搜索延迟 < 50ms
- [ ] AC-P04：语义搜索延迟 < 200ms
- [ ] AC-P05：文件变更处理延迟 < 500ms（含防抖时间）
