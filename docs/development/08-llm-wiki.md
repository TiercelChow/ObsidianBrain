# 阅境轩·书籍知识库（Book Wiki）— 开发设计文档 v3

> **文档编号**: DEV-08
> **版本**: v3.0
> **状态**: MVP 2 已落地，后台 Worker 与知识变更集待实现
> **最后更新**: 2026-09-12
> **对应需求**: [REQ-08](../requirement/08-llm-wiki.md)
> **关联需求**: [REQ-10 阅境轩书架](../requirement/10-reader-bookshelf.md)

---

## 0. 当前实施状态（2026-09-12）

首个可运行纵切已经完成：书架 JSON 可无损迁移到正规表；每本书可建立独立知识库；Markdown 文件夹可扫描为带文件和行号引用的数据库章节实体；五个新页面已接通真实 API；研究任务、书籍配置文档和 Runtime Profile 均保存到 SQLite。自然语言检索目前采用轻量查询规整与数据库正文召回。知识问答已经通过官方 Rust SDK 接入 DeepSeek Harness ACP v1：后端先召回当前书籍的数据库证据，再启动一次性 ACP 会话生成带 `[S#]` 来源编号的回答，并把输入、输出和失败状态记录到 `agent_runs`。前端会把编号映射为可定位、可打开的证据卡片，不使用不安全的 HTML 注入。

研究任务已经形成首个执行闭环：草稿由用户显式点击运行；后端在单书边界内召回证据，通过同一只读 Harness Patch 生成带引用报告，持久化任务状态、报告和 `agent_runs` 审计记录；失败任务可重试，已完成报告刷新页面后仍能恢复来源映射。当前执行采用同步请求，不冒充后台队列，也不会自动修改正式知识实体。

当前不会伪造 Agent 结果。未检测到 Harness 时，知识问答降级为展示真实命中的证据；PDF 只完成来源登记并明确标记为等待版面提取。PDF 版面提取、结构化论断/关系生成、耐久化任务 Worker、取消与进度事件、审核、备份、导出和旧后端清理仍按本文后续阶段实施。

本地开发使用固定版本命令 `npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp`。调用前必须在 Harness Web 的 Models 页面保存密钥，或在启动 ObsidianBrain 的环境中提供 `DEEPSEEK_API_KEY`；密钥不写入 Runtime Profile 或知识库数据库。配置页将“启动命令检测”和需要一次最小模型请求的“连接验证”明确分开。模型字段留空时使用 Harness Profile 默认值，填写后通过 ACP `session/set_config_option` 覆盖。所有 `session/request_permission` 默认返回取消；问答和任务会话使用独立临时空目录，并物化专用 Patch 关闭 Bash、PowerShell、文件系统、联网、Skill、任务和子 Agent 工具，避免书籍原文件或主机环境暴露给模型。

---

## 1. 架构目标

实现一套数据库原生、每书隔离、Agent Runtime 可替换的书籍知识系统。

必须坚持四个边界：

1. 原书是只读来源，不是 Agent 工作区。
2. SQLite 是生成知识的唯一事实来源，不与 Markdown Wiki 双写。
3. Rust 后端拥有业务状态、校验、事务、审核和权限。
4. DeepSeek Harness / Claude Code 只负责 Agent 执行，不拥有知识数据。

---

## 2. 总体架构

```text
┌──────────────────────────────────────────────────────────────┐
│ Vue 3                                                        │
│ 书架 │ 知识库 │ Wiki 工作台 │ 问答 │ 研究任务 │ 配置         │
└──────────────────────────┬───────────────────────────────────┘
                           │ HTTP + SSE
┌──────────────────────────▼───────────────────────────────────┐
│ Axum API / Tool Registry                                     │
├──────────────────────────────────────────────────────────────┤
│ Core                                                         │
│ BookService          KnowledgeBaseService                     │
│ SourceService        KnowledgeEntryService                    │
│ RetrievalService     ReviewService                            │
│ ResearchTaskService  AgentRunService                          │
│ ConfigDocumentService / SkillService                         │
├──────────────────────────────────────────────────────────────┤
│ Infra                                                        │
│ SQLite Repositories + FTS5 │ Source Extractors │ Backup       │
│ Harness Adapters           │ Process/Sandbox    │ Event Stream │
└───────────────┬───────────────────────────────┬───────────────┘
                │                               │
        local source files            DeepSeek Harness Sidecar
        PDF / Markdown                or Claude Code process
```

---

## 3. 模块与目录规划

```text
backend/src/
├── models/
│   ├── book_wiki.rs
│   ├── knowledge.rs
│   ├── knowledge_task.rs
│   └── agent_run.rs
├── core/
│   └── book_wiki/
│       ├── mod.rs
│       ├── service.rs
│       ├── source_service.rs
│       ├── ingest_service.rs
│       ├── entry_service.rs
│       ├── retrieval_service.rs
│       ├── review_service.rs
│       ├── task_service.rs
│       ├── config_service.rs
│       └── validation.rs
├── infra/
│   ├── book_wiki_store.rs
│   ├── knowledge_fts.rs
│   ├── source_extractors/
│   │   ├── mod.rs
│   │   ├── markdown.rs
│   │   └── pdf.rs
│   ├── harness/
│   │   ├── mod.rs
│   │   ├── deepseek.rs
│   │   ├── claude_code.rs
│   │   ├── acp.rs
│   │   ├── process.rs
│   │   └── workspace.rs
│   └── backup.rs
├── api/handlers/
│   └── book_wiki.rs
└── tools/handlers/
    └── book_wiki.rs

frontend/src/
├── views/knowledge/
│   ├── KnowledgeBases.vue
│   ├── WikiWorkspace.vue
│   ├── KnowledgeChat.vue
│   ├── KnowledgeTasks.vue
│   └── WikiSettings.vue
├── components/knowledge/
├── stores/knowledge.ts
└── api/knowledge.ts

sidecars/deepseek-harness/
├── package.json
├── profile/
│   └── cordis.yml
└── plugins/
    └── obsidianbrain-knowledge-tools/
```

`core` 不依赖 DeepSeek Harness 或 Claude Code 的具体类型；具体执行器只存在于 `infra/harness`。

---

## 4. 数据库设计

### 4.1 通用约定

- 主键使用 UUID 字符串。
- 时间统一以 UTC RFC3339 字符串保存，并由同一模块统一转换。
- 布尔值用 SQLite INTEGER 0/1。
- 枚举在 Rust 中定义并在入库前校验，数据库增加 `CHECK` 约束。
- 所有书籍知识表包含 `knowledge_base_id`，唯一约束与查询索引必须带该字段。
- 正式写入使用 `BEGIN IMMEDIATE` 单事务。
- 业务对象使用 `revision INTEGER NOT NULL` 做乐观并发控制。
- JSON 只保存开放扩展元数据，不代替可查询的核心字段。

### 4.2 书架迁移

#### `reader_books`

| 字段 | 说明 |
|---|---|
| `id` | 沿用旧 ReaderBook ID |
| `path` | 规范化绝对路径，唯一 |
| `kind` | folder / pdf |
| `name` | 书名 |
| `description` | 描述 |
| `category` | 类别 |
| `added_at` | 加入时间 |
| `progress_json` | 现有阅读进度，暂不在本轮拆表 |
| `shelf_state` | active / removed；保留知识库时从书架软删除 |
| `removed_at` | 软删除时间，可空 |
| `revision` | 并发版本 |

迁移读取 `app_state.reader_books`，在同一事务中按 ID 和路径去重插入，迁移后保存原 JSON 到 `reader_books_legacy_backup`。

删除书籍时，只有选择“同时删除知识库”才允许物理删除记录；保留或归档知识库时将 `shelf_state` 改为 `removed`，从书架查询中隐藏。

### 4.3 知识库

#### `knowledge_bases`

```sql
CREATE TABLE knowledge_bases (
    id                TEXT PRIMARY KEY,
    book_id           TEXT NOT NULL UNIQUE REFERENCES reader_books(id),
    lifecycle         TEXT NOT NULL CHECK (lifecycle IN
                        ('uninitialized','active','paused','archived')),
    sync_state        TEXT NOT NULL CHECK (sync_state IN
                        ('clean','outdated','scanning','extracting','ingesting','failed')),
    health_state      TEXT NOT NULL CHECK (health_state IN
                        ('healthy','warning','needs_review')),
    template_id       TEXT,
    config_profile_id TEXT,
    last_synced_at    TEXT,
    last_scanned_at   TEXT,
    revision          INTEGER NOT NULL DEFAULT 1,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
```

状态变更只允许通过领域服务，不提供通用字段更新接口。

### 4.4 来源

#### `source_documents`

- `id`, `knowledge_base_id`
- `source_type`: pdf / markdown
- `original_path`, `relative_path`
- `title`, `mime_type`, `ordinal`
- `current_version_id`
- `sync_status`: current / changed / missing / failed
- `metadata_json`, `created_at`, `updated_at`

唯一约束：`(knowledge_base_id, original_path)`。

#### `source_versions`

- `id`, `source_document_id`
- `content_hash`, `size_bytes`, `modified_at`
- `extraction_version`, `extraction_status`, `error`
- `created_at`

相同来源的相同哈希不得重复建版本。

#### `source_spans`

- `id`, `knowledge_base_id`, `source_version_id`
- `ordinal`, `page_number`
- `heading`, `anchor`, `line_start`, `line_end`
- `content`, `content_hash`, `token_estimate`

索引：`(knowledge_base_id, source_version_id, ordinal)`。

### 4.5 知识实体

#### `knowledge_entries`

```sql
CREATE TABLE knowledge_entries (
    id                 TEXT PRIMARY KEY,
    knowledge_base_id  TEXT NOT NULL REFERENCES knowledge_bases(id),
    entry_type         TEXT NOT NULL,
    slug               TEXT NOT NULL,
    title              TEXT NOT NULL,
    summary            TEXT NOT NULL DEFAULT '',
    content_md         TEXT NOT NULL DEFAULT '',
    status             TEXT NOT NULL CHECK (status IN
                         ('draft','verified','stale','archived')),
    confidence         REAL,
    revision           INTEGER NOT NULL DEFAULT 1,
    created_by_run_id  TEXT,
    updated_by_run_id  TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    UNIQUE (knowledge_base_id, entry_type, slug)
);
```

附属表：

- `knowledge_entry_aliases(entry_id, alias_normalized, alias_display)`
- `knowledge_tags(id, knowledge_base_id, name, normalized_name)`
- `knowledge_entry_tags(entry_id, tag_id)`
- `knowledge_entry_versions(id, entry_id, revision, snapshot_json, reason, run_id, created_at)`

### 4.6 论断、关系与引用

#### `knowledge_claims`

- `id`, `knowledge_base_id`, `entry_id`
- `subject_entry_id`, `predicate`
- `object_entry_id` 或 `object_text`
- `claim_text`, `confidence`
- `verification_status`: unverified / supported / disputed / rejected / stale
- `valid_from`, `valid_to`, `revision`

#### `knowledge_relations`

- `id`, `knowledge_base_id`
- `from_entry_id`, `to_entry_id`
- `relation_type`, `strength`, `evidence`
- `created_by_run_id`, `revision`

相同知识库内 `(from_entry_id, to_entry_id, relation_type)` 唯一。数据库触发器或领域校验禁止跨库关系。

#### `knowledge_citations`

- `id`, `knowledge_base_id`
- `entry_id`, 可选 `claim_id`
- `source_span_id`
- `quote_text`, `relevance`, `citation_role`

`citation_role` 至少支持 `supports`、`contradicts`、`context`。

### 4.7 审核与变更集

#### `knowledge_change_sets`

- `id`, `knowledge_base_id`, `agent_run_id`
- `title`, `reason`, `risk_level`
- `status`: proposed / approved / rejected / applied / conflicted
- `created_at`, `resolved_at`, `resolved_by`

#### `knowledge_changes`

- `id`, `change_set_id`, `ordinal`
- `operation`: create / update / merge / split / archive / restore
- `object_type`, `object_id`
- `expected_revision`
- `before_json`, `after_json`

变更应用器必须先验证整个集合，再在单事务中全部提交；禁止部分成功。

### 4.8 对话、任务与运行

- `knowledge_conversations`
- `knowledge_conversation_scopes`
- `knowledge_messages`
- `knowledge_message_citations`
- `knowledge_tasks`
- `knowledge_task_dependencies`
- `agent_runs`
- `agent_run_events`
- `agent_run_artifacts`

`knowledge_conversation_scopes` 显式记录参与查询的知识库。任何检索工具从 Run Capability 中读取范围，不接受 Agent 自由传入其他知识库 ID。

### 4.9 配置与 Skill

- `wiki_config_profiles`
- `wiki_config_documents`
- `wiki_config_document_versions`
- `agent_runtime_profiles`
- `llm_profiles`
- `skills`
- `skill_versions`
- `skill_files`
- `knowledge_base_skill_bindings`

API Key 只保存 `credential_ref`，真实密钥来自环境变量或系统密钥服务。

---

## 5. 全文检索与图谱

### 5.1 FTS5

建立两张虚拟表：

```sql
CREATE VIRTUAL TABLE knowledge_entries_fts USING fts5(
    entry_id UNINDEXED,
    knowledge_base_id UNINDEXED,
    title,
    aliases,
    summary,
    content,
    tags,
    cjk_terms,
    tokenize = 'unicode61'
);

CREATE VIRTUAL TABLE source_spans_fts USING fts5(
    span_id UNINDEXED,
    knowledge_base_id UNINDEXED,
    source_title,
    heading,
    content,
    cjk_terms,
    tokenize = 'unicode61'
);
```

应用在写入 FTS 时把连续中文文本转换为空格分隔的字符二元组并写入 `cjk_terms`；查询时用同一规范化函数生成二元组，再与标题、别名精确命中和标题前缀结果合并。只处理查询而不索引二元组无法被 `unicode61` 正确召回，禁止采用该错误实现。首版不引入难以跨平台稳定打包的外部分词扩展。

FTS 更新与正式实体事务绑定。启动时执行 FTS 完整性自检，提供单书与全局重建命令。

### 5.2 检索管线

```text
查询规范化
  → 标题/别名精确匹配
  → Entry FTS + Source FTS
  → 关系图一至二跳扩展
  → 去重与按库隔离
  → 来源覆盖与可信度重排
  → 上下文预算裁剪
  → Agent 生成带引用回答
```

向量检索是可选增强。若启用，向量记录必须带 `knowledge_base_id` 和内容版本哈希；向量索引失效不能阻断 FTS 降级路径。

### 5.3 图谱

图谱直接消费 `knowledge_relations`。首版提供邻居、路径、孤立节点、桥接节点和按类型过滤；社区发现和复杂相关性评分放在后续阶段。

---

## 6. 来源扫描与抽取

### 6.1 抽象

```rust
#[async_trait]
pub trait SourceExtractor: Send + Sync {
    fn supports(&self, kind: &SourceKind) -> bool;
    async fn inspect(&self, source: &SourceDescriptor)
        -> Result<SourceFingerprint, BrainError>;
    async fn extract(&self, source: &SourceDescriptor)
        -> Result<ExtractedDocument, BrainError>;
}
```

`ExtractedDocument` 必须返回稳定顺序的 `ExtractedSpan`，每个 Span 含定位信息和内容哈希。

### 6.2 Markdown 文件夹

- 递归扫描 `.md`，忽略隐藏、临时和配置目录。
- 保留相对路径、标题层级和行号。
- 解析 frontmatter，但原文不改写。
- 相同内容哈希直接复用旧来源版本和片段。
- 删除文件时标记来源缺失，相关知识进入影响评估，不立即级联删除。

### 6.3 PDF

- 使用现有 Range 流式端点服务阅读，不把整份 PDF 读入前端内存。
- 后端抽取按页保留页码、文本顺序和内容哈希。
- 扫描型 PDF 进入 `needs_ocr`，由配置的 OCR Provider 处理。
- 公式、表格和图片无法可靠抽取时保留警告，不生成伪造文本。
- 替换 PDF 后以文件哈希产生新版本，旧引用继续指向旧版本并标记可能过时。

### 6.4 增量同步

```text
扫描书籍
  → 对比文档路径、哈希和版本
  → 建立 added / changed / missing 集合
  → 只抽取新增和变化文档
  → 计算受影响实体与论断
  → 创建摄入任务
```

文件监听只用于提示 `outdated`；正式同步必须重新扫描确认，不能把不可靠的监听事件直接当事实。

---

## 7. 两阶段摄入

### 7.1 阶段一：结构化分析

Agent 读取 purpose、schema、已有相关实体和本次来源片段，输出严格 JSON：

- 来源摘要
- 候选实体及别名
- 候选论断
- 候选关系
- 来源引用
- 与已有知识的重复、补充或冲突
- 推荐新增、更新、合并或审核动作

分析结果保存为 Run Artifact，不修改正式知识。

### 7.2 阶段二：变更生成

Agent 基于分析结果生成 `KnowledgeMutationSet`：

```rust
pub struct KnowledgeMutationSet {
    pub schema_version: u32,
    pub knowledge_base_id: String,
    pub idempotency_key: String,
    pub rationale: String,
    pub mutations: Vec<KnowledgeMutation>,
}
```

每个 Mutation 必须包含目标、预期版本、正文或结构化字段、引用和风险提示。

### 7.3 校验

Rust 后端执行：

- JSON Schema 与业务类型校验
- knowledge base scope 校验
- 来源片段存在性校验
- 引用与对象外键校验
- Slug 和别名冲突校验
- revision 校验
- 最大变更数量和正文大小校验
- 风险策略校验
- 幂等键校验

只有通过校验的变更集才可进入审核或自动应用。

---

## 8. Agent Harness 架构

### 8.1 统一接口

```rust
#[async_trait]
pub trait AgentHarness: Send + Sync {
    async fn health_check(&self) -> Result<HarnessHealth, BrainError>;
    async fn start_run(
        &self,
        request: AgentRunRequest,
        events: tokio::sync::mpsc::Sender<AgentEvent>,
    ) -> Result<AgentRunHandle, BrainError>;
    async fn send_input(&self, run_id: &str, input: AgentInput)
        -> Result<(), BrainError>;
    async fn cancel(&self, run_id: &str) -> Result<(), BrainError>;
}
```

业务层只依赖该 trait。Harness 返回文本、状态和工具事件，正式业务结果必须通过受控工具或结构化 Artifact 提交。

### 8.2 运行能力令牌

每个 Agent Run 创建不可预测的短期令牌，绑定：

- `run_id`
- 允许访问的 `knowledge_base_ids`
- 允许的工具集合
- 允许的操作类型
- 过期时间

工具端从令牌读取权限范围，忽略 Agent 试图扩大范围的参数。

### 8.3 DeepSeek Harness Adapter

#### 部署

- Node Sidecar 与应用版本一起固定，不运行 `npx latest`。
- 每个应用版本记录兼容的 Harness 版本和 Profile 哈希。
- 使用绝对可执行路径和独立 `DSH_HOME`，不读取用户全局 Profile 与凭据。
- Sidecar 按需启动，空闲保温时间可配置，默认到期自动退出；进程常驻不是业务正确性的前提。

#### 通信

- Rust 启动 Sidecar 并通过 ACP JSON-RPC/stdio 通信。
- 每个运行创建独立 Session 和绝对临时 `cwd`。
- ACP 管理提示、一次性审批和取消。
- 工具产生的进度写入 ObsidianBrain `agent_run_events`，前端通过 SSE 消费。
- Harness 会话日志只作诊断，不作为问答或任务的权威存储。

#### 专用 Profile

`obsidianbrain-wiki` 只挂载：

- Agent Loop、模型适配器
- Session 与必要的 checkpoint
- Skill registry/tool
- Workflow/goal/compaction/token meter
- Sandbox 与 approval
- ObsidianBrain knowledge tools

默认不挂载任意 Bash、无限制网络、全盘文件搜索、自修改和与知识任务无关的开发工具。

#### 工具桥

Sidecar 插件通过 `127.0.0.1` 调用后端专用端点，使用 Run Capability Bearer Token。插件不获取数据库路径。

### 8.4 Claude Code Adapter

- 优先 Agent SDK，CLI 作为兼容后备。
- 使用隔离模式，不加载未声明的全局配置。
- 显式传入工作目录、模型、最大轮次、Skill 和工具权限。
- 使用 MCP 或同一 HTTP Tool Bridge 调用知识工具。
- 会话输出映射为统一 `AgentEvent`。

### 8.5 临时工作区物化

```text
<temp>/obsidianbrain-agent-runs/{run-id}/
├── purpose.md
├── schema.md
├── instructions.md
├── AGENTS.md
├── CLAUDE.md
├── .agents/skills/.../SKILL.md
├── .claude/skills/.../SKILL.md
└── outputs/
```

文件来自数据库中的确定版本。任务记录其哈希。原书不复制进可写工作区，Agent 通过工具读取来源。

任务结束后按保留策略清理；诊断 Artifact 先导入数据库或受管日志目录。

---

## 9. Agent 工具契约

首版向 Harness 提供：

| 工具 | 权限 | 说明 |
|---|---|---|
| `book_get_context` | read | 当前书籍、知识库、purpose/schema 摘要 |
| `book_list_sources` | read | 列出授权书籍来源和版本 |
| `book_search_sources` | read | FTS 搜索来源片段 |
| `book_read_source_span` | read | 读取指定来源片段及上下文 |
| `knowledge_search_entries` | read | 搜索当前授权知识库实体 |
| `knowledge_get_entry` | read | 读取实体、论断、关系和引用 |
| `knowledge_get_neighbors` | read | 读取图谱邻居 |
| `knowledge_propose_changes` | propose | 提交结构化变更集 |
| `knowledge_create_task` | propose | 提交研究任务候选 |
| `knowledge_report_progress` | event | 更新阶段、百分比和说明 |
| `knowledge_get_review_result` | read | 审核后继续运行时读取结果 |

所有 JSON Schema 均设置 `additionalProperties: false`，Rust 输入类型使用 `deny_unknown_fields`，避免额外字段造成工具参数漂移。

---

## 10. 配置与 Skill 实现

### 10.1 合并规则

配置解析顺序：

1. 系统默认
2. 场景模板
3. 知识库覆盖
4. 单次运行覆盖

合并结果生成不可变 `EffectiveWikiConfig`，运行开始后不受配置页面修改影响。

### 10.2 配置文档

`purpose/schema/instructions/AGENTS/CLAUDE` 使用稳定 `document_type` 标识，不用文件名作为主键。保存时：

1. 校验大小和 Markdown 编码。
2. revision 比对。
3. 写入版本表。
4. 更新当前内容和 revision。
5. 标记受影响知识库需要重新维护，但不自动重建。

### 10.3 Skill

一个 Skill 由 `skills` 身份、`skill_versions` 版本和多行 `skill_files` 组成。相对路径必须通过安全路径校验；禁止绝对路径、`..`、符号链接和可执行文件默认执行。

启用 Skill 前验证：

- 存在且仅有一个入口 `SKILL.md`
- 名称和描述可解析
- 引用文件存在
- 要求工具在目标 Harness Profile 中可用
- 声明权限没有超过知识库策略

---

## 11. 问答实现

### 11.1 会话事实来源

对话和消息保存在 ObsidianBrain SQLite。Harness Session ID 仅为一次运行的诊断关联，不作为恢复对话的唯一依据。

### 11.2 单次问答

```text
保存用户消息
  → 确定显式知识库范围
  → 检索候选实体与来源
  → 建立上下文预算
  → 启动 Harness Run
  → Agent 按需继续调用检索工具
  → 保存回答和引用
  → SSE 结束事件
```

### 11.3 引用校验

Agent 回答只能引用工具返回的 entry/span ID。后端在保存前验证 ID 属于会话范围；无法解析的引用从正式引用列表剔除并记录警告，不能伪造跳转。

### 11.4 阅读跳转

- PDF：`book_id + source_document_id + page_number`。
- Markdown：`book_id + relative_path + heading/anchor`。
- 前端通过 Reader 路由状态打开书籍，再执行定位。

---

## 12. 研究任务与调度

### 12.1 任务状态机

状态转换由 `KnowledgeTaskService` 控制。`running` 任务必须关联一个活跃 Run；进程异常退出后转 `failed` 或 `waiting_review`，不得自动标为完成。

### 12.2 Worker

- 数据库队列按优先级和创建时间领取任务。
- 使用租约字段避免重复领取。
- 默认全局并发 1，可配置到 2；同一本书默认最多一个写任务。
- 只读问答可以与写任务并发，但读取固定 revision 快照。
- 重试使用退避并设置最大次数。

### 12.3 幂等

每个任务尝试生成独立 `run_id`，业务动作使用稳定 `task_id + attempt + phase` 幂等键。Harness 重试只能再次提交同一变更候选，不能重复应用。

---

## 13. API 设计

面向前端提供资源型 HTTP API；面向 Harness 提供小而稳定的 Tool API。不要让前端页面复用 Agent Tool Envelope。

### 13.1 前端 API

```text
GET    /v1/knowledge-bases
POST   /v1/knowledge-bases
GET    /v1/knowledge-bases/:id
PATCH  /v1/knowledge-bases/:id
POST   /v1/knowledge-bases/:id/scan
POST   /v1/knowledge-bases/:id/sync
POST   /v1/knowledge-bases/:id/lint
DELETE /v1/knowledge-bases/:id

GET    /v1/knowledge-bases/:id/entries
GET    /v1/knowledge-entries/:id
PATCH  /v1/knowledge-entries/:id
GET    /v1/knowledge-entries/:id/versions
GET    /v1/knowledge-bases/:id/graph
GET    /v1/knowledge-bases/:id/sources

GET    /v1/knowledge-reviews
POST   /v1/knowledge-reviews/:id/resolve

GET    /v1/knowledge-tasks
POST   /v1/knowledge-tasks
POST   /v1/knowledge-tasks/:id/cancel
POST   /v1/knowledge-tasks/:id/retry

GET    /v1/knowledge-conversations
POST   /v1/knowledge-conversations
POST   /v1/knowledge-conversations/:id/messages
GET    /v1/agent-runs/:id/events

GET    /v1/wiki-settings
PUT    /v1/wiki-settings
GET    /v1/wiki-config-documents
PUT    /v1/wiki-config-documents/:id
GET    /v1/wiki-skills
POST   /v1/wiki-skills/import
```

列表接口统一支持 cursor、limit、query、sort 和结构化 filters。

### 13.2 SSE

事件至少包括：

- `run.started`
- `run.phase_changed`
- `run.progress`
- `run.tool_started`
- `run.tool_finished`
- `run.review_required`
- `run.completed`
- `run.failed`
- `run.cancelled`

断线重连通过事件自增序号和 `Last-Event-ID` 补发数据库中的事件。

---

## 14. 前端实现原则

### 14.1 状态与性能

- 页面只请求当前可见范围，禁止知识库首页加载所有实体正文。
- 列表使用分页或虚拟滚动。
- 图谱组件按路由懒加载并在离开页面时释放实例。
- SSE 每个 Run 只维持一个连接，页面卸载后关闭。
- Markdown 实体正文复用共享渲染会话，切换实体时取消旧渲染。
- 大量状态统计由后端投影，不在前端遍历实体计算。

### 14.2 桌面

Wiki 工作台三栏可伸缩；列表压缩为窄轨时保留搜索、类型和状态信息。变更审核使用清晰的对象级差异，不直接展示原始 JSON。

### 14.3 手机

- 单列为主，辅助面板使用底部抽屉。
- 主要操作置于底部安全区。
- 图谱先展示洞察列表，手动进入全屏图谱。
- 审核差异上下排列。
- 长任务进度使用可收起状态胶囊，不遮挡正文。

---

## 15. 数据库并发与性能

当前 `SqliteStore` 的单个 `Arc<Mutex<Connection>>` 会串行化所有读写。Book Wiki 实施时拆出 Repository 层，并采用以下任一受测方案：

- 小型 rusqlite 连接池；或
- 单写线程 + 多只读连接。

要求：

- 写连接开启 WAL、foreign_keys 和 busy_timeout。
- 同一事务内写实体及 FTS。
- rusqlite 阻塞工作不直接占用 Tokio Core Worker。
- 高频统计使用增量投影表或受版本控制的缓存。
- `EXPLAIN QUERY PLAN` 验证实体列表、引用、任务队列和 FTS 查询。

---

## 16. 备份与恢复

### 16.1 自动备份

- 使用 SQLite Online Backup API 生成一致快照。
- 数据库迁移、知识库重建和大批量应用前自动备份。
- 默认保留最近 7 份，可配置。

### 16.2 恢复

恢复前执行 `PRAGMA integrity_check`，恢复后校验迁移版本、外键和 FTS；FTS 可从核心表重建，但实体、引用、配置和版本不可丢失。

### 16.3 导出

导出器从数据库生成标准 Markdown Wiki 或 JSON Bundle。导出目录不能被 Agent 自动反向摄入，以免形成循环。

---

## 17. 安全设计

- Sidecar 由 Rust 父进程管理，退出时回收整个进程组。
- 监督器记录 Sidecar 内存与退出原因，超过配置上限时停止接收新任务并有序回收。
- 每次运行使用隔离临时目录和独立能力令牌。
- 工具网关验证 run、scope、expiry 和 operation。
- 原书通过受控读取工具访问，不向 Agent 暴露任意路径读取。
- 删除、重建、网络访问和高风险变更需要独立权限。
- Harness stdout 保留协议，诊断只写 stderr 并脱敏。
- Harness 与 Skill 版本固定并校验哈希。
- 导入 Skill 不自动执行其中脚本。

---

## 18. 迁移与删除策略

### 18.1 迁移顺序

1. 新增数据库迁移和书架正规表。
2. 迁移书架 JSON，前端仍暂用旧界面。
3. 新增 Knowledge API 与后台状态页。
4. 完成摄入、审核和实体工作台。
5. 接入 DeepSeek Harness。
6. 完成问答、任务和配置。
7. 切换导航和 Reader 联动。
8. 删除旧页面、工具和 Wiki Engine。

### 18.2 旧数据

旧 `Wiki/*.md` 不作为正式迁移依赖。若需要保留，提供一次性“旧 Wiki 导入”命令，将页面作为 `legacy_import` 来源进入新的审核流程。

### 18.3 可回滚性

新旧页面切换期间使用 Feature Flag。数据库迁移只新增表，不删除旧表或 app_state 键；旧代码删除前保留至少一个可回滚版本。

---

## 19. 测试策略

### 19.1 单元测试

- 状态机合法与非法转换
- 配置继承和版本冲突
- 路径规范化与知识库隔离
- 来源哈希和增量差异
- Markdown/PDF Span 定位
- Mutation Schema 和领域校验
- 风险分级和自动审核策略
- FTS 查询规范化与中文二元组
- Citation 跳转解析
- Skill 路径安全与物化

### 19.2 Repository 测试

- 每个公共写方法正向、边界、冲突和回滚测试
- 外键、唯一约束与跨库拒绝
- 变更集全成全败
- FTS 与实体事务一致
- 任务租约和并发领取
- 备份、完整性检查和恢复

### 19.3 Harness 契约测试

使用假的 ACP/CLI 子进程验证：

- 启动、健康检查和版本不兼容
- 流式事件映射
- 权限请求允许与拒绝
- 取消、超时和进程异常退出
- 工具令牌越权拒绝
- 重复提交幂等
- Skill 与配置快照物化

真实 DeepSeek Harness 和 Claude Code 只进入可选集成测试，不让普通单测依赖网络和真实密钥。

### 19.4 前端与端到端

- 五个页面主路径
- 两本书隔离
- 书架建库与状态同步
- PDF/Markdown 引用跳转
- 问答流式、断线重连和引用
- 审核批准/驳回/冲突
- 任务取消和重试
- 手机底部抽屉与安全区
- 图谱卸载后的内存回收

---

## 20. 交付阶段与门槛

### Phase A：领域底座

- 数据迁移、Repository、状态机、备份。
- 门槛：书架迁移无损；知识库 CRUD 和隔离测试全绿。

### Phase B：摄入闭环

- Extractor、Span、实体模型、Mutation、审核、FTS。
- 门槛：同一来源重复同步零重复；引用可回原书。

### Phase C：DeepSeek Harness

- Sidecar、ACP、专用 Profile、工具桥、Skill、Run Worker。
- 门槛：建库任务可取消、可恢复、不可越权，失败不污染正式知识。

### Phase D：前端

- 五页、Reader 联动、桌面与手机交互。
- 门槛：核心用户旅程端到端通过；长列表和图谱无明显泄漏。

### Phase E：能力完整

- 问答、研究任务、Claude Code、导出恢复。
- 门槛：双 Harness 契约一致；导出与备份可恢复。

### Phase F：清理

- 删除旧前后端、更新顶层设计和用户手册。
- 门槛：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、前端测试与构建全部通过。

---

## 21. 关键风险

| 风险 | 对策 |
|---|---|
| DeepSeek Harness 仍处 Developer Preview | 固定版本、Sidecar 隔离、适配层、契约测试，禁止业务类型依赖其内部模型 |
| 数据库成为不可重建事实来源 | 在线备份、迁移前快照、版本表、JSON/Markdown 导出 |
| PDF 抽取质量不足 | 页级引用、质量标记、可选 OCR、失败不生成伪知识 |
| Agent 输出不稳定 | 严格 Schema、受控工具、两阶段处理、变更集和事务校验 |
| 不同书籍数据串联 | 能力令牌、每表 knowledge_base_id、复合唯一约束和跨库测试 |
| 长任务占用资源 | 队列、同书写任务互斥、并发上限、取消、超时和进程组回收 |
| Skill 携带危险指令或脚本 | 数据库版本、权限声明、导入校验、默认不执行脚本、运行目录隔离 |
| 外部 GPL 项目许可证影响 | 借鉴方法与交互，独立实现；不直接复制代码，除非项目明确接受 GPLv3 |

---

## 22. 完成定义

只有同时满足以下条件，Book Wiki v3 才视为完成：

- 书籍与知识库一对一且隔离。
- SQLite 是所有生成知识的唯一事实来源。
- 实体、论断、关系、引用和版本可在页面完整查看与管理。
- PDF 和 Markdown 引用可以稳定跳回阅境轩。
- Agent 无法直接写数据库或越权访问其他书籍。
- DeepSeek Harness 和 Claude Code 至少通过统一契约测试，DeepSeek Harness 完成正式接入。
- 问答和研究任务可追溯到来源、配置和 Skill 版本。
- 审核、幂等、取消、恢复、备份和导出均经过验证。
- 旧知识五页面和旧 Wiki 工作流完成退役。
