# 阅境轩·书籍知识库（Book Wiki）— 开发设计文档 v3

> **文档编号**: DEV-08
> **版本**: v3.14
> **状态**: Book Wiki v3 核心闭环、授权外部研究与旧模块退役已落地
> **最后更新**: 2026-09-19
> **对应需求**: [REQ-08](../requirement/08-llm-wiki.md)
> **关联需求**: [REQ-10 阅境轩书架](../requirement/10-reader-bookshelf.md)

---

## 0. 当前实施状态（2026-09-15）

迁移 016–018 已把章节索引升级所需的数据边界补齐。来源同步采用文档版本和不可变片段，重复同步保持 revision 幂等，文件更新不会级联删除旧证据；会话与成果引用保存当时的实体/路径快照。旧知识库继续保持 `chapter/not_started`，只有用户点击智能编译才会发生模型调用。

`compile_semantic_wiki` 按当前来源分批执行，超长章节会继续切块而不是静默截断。单批来源正文预算为 20,000 字符，最多保留 5 个跨片段归并后的高价值候选，并通过 Harness 供应商 Patch 将首次调用的输出上限设为 8,192 token，必要时单次恢复调用设为 12,288 token。编译专用路由显式关闭推理等级、将供应商自动重试从默认最多 5 次压缩为最多 1 次；这些限制不影响知识问答和研究任务。模型只返回短摘要、别名、原子论断、关系和引用，不再重复撰写 `content_md`；服务端根据摘要和论断生成可检索正文，并再次执行字段限长。知识摄入请求的单批截止时间为 180 秒。每批读取已有语义 Wiki 和前批候选，归并概念、别名、论断与关系；ACP 已结束但没有正文时记录停止原因，并以更高输出预算重试一次；无效 JSON 也最多重试一次，两种情况合计每批不超过两次模型调用。拒绝回答和用户取消不重试。模型结果必须通过服务端 entry type、当前 span 引用和数量限制校验，再写入 `knowledge_change_sets`。批准时再次校验来源与 expected revision，并在一个 SQLite 事务中写入条目、论断、关系、引用和版本；人工保护条目的修改始终为高风险。

语义编译的来源批次已经完整内联到提示词，变更集也由 `BookWikiService` 校验和持久化，因此编译 Run 不挂载 MCP 工具目录，避免无用的工具发现、工具 Schema token 和额外 Agent 回合。问答与研究任务仍按最小权限挂载知识工具；能力令牌 TTL 至少覆盖对应运行超时并额外保留 60 秒收尾窗口。ACP 适配器会记录进程启动、连接建立、会话就绪和请求提交等阶段，即使第三方兼容端点只在整段回答结束时推送文本，前端也能区分“尚未连接”和“模型已收到请求”。

迁移 023 为知识库增加编译阶段、说明、来源/批次进度、当前 Run、变更集、启动时间、心跳和取消标记。`compile_book_knowledge_base` 只原子领取任务并立即返回，实际编译由与 HTTP 请求生命周期解耦的 Tokio 任务执行；后台 Supervisor 捕获异常退出并落库为失败。运行事件被投影为准备、连接 Runtime、思考、工具、生成、重试、收尾和待审核阶段，前端定时读取知识库摘要。取消会同时写入持久化请求并通知活跃 ACP Run；服务启动时仍为 `compiling` 的历史记录会转为明确失败，允许用户重新编译。

迁移 019 增加 `knowledge_compile_checkpoints`、新版 `knowledge_entries_fts` 和 `source_spans_fts`。编译只读取检查点缺失或版本不同的当前来源，变更集创建成功后记录文档版本；驳回候选会删除其检查点，避免来源被永久跳过。FTS 写入与来源/实体事务同步，连续中文同时写入二元组，查询按标题、别名、摘要和正文加权；升级数据库在首次检索时一次性重建旧内容索引。

问答和研究检索把正式语义条目排在 `source_section` 前，同时保留章节兜底。问答会话、Markdown 回答和来源预览可恢复，用户可把完成回答保存为待审核 `synthesis` 候选。确定性 lint 会检查来源、语义层、实体引用、论断直接证据和重复标题，不调用模型、不修改知识。

研究任务由进程内单 Worker 从 SQLite 耐久队列领取。排队后 HTTP 立即返回；页面离开不会停止任务。支持主动 ACP 取消、失败/取消后重试，以及服务启动时把中断的 running 任务恢复为 queued（已请求取消的恢复为 cancelled）。文本增量、思考阶段、工具开始/结束和上下文 Usage 会写入有序运行事件；任务页面按任务读取最近运行的真实阶段。

迁移 020 增加 `agent_run_capabilities` 与知识库范围表。能力令牌采用高熵随机值，SQLite 只保存 SHA-256，验证同时检查 Run 仍在运行、过期时间、知识库范围和工具白名单。Harness 通过 `@deepseek-ai/dsh-mcp-client` 的 `streamable-http` 配置访问 `127.0.0.1` 专用 MCP 端点；问答、摄入和研究使用不同的最小工具集，未知字段、跨书访问、过期或已撤销令牌均被拒绝。

迁移 017 的 Skill Registry 支持内置 Skill、自定义 Markdown 指令和安全 ZIP 导入。ZIP 限制文件数、单文件/展开大小，拒绝路径穿越、符号链接、二进制与脚本；所有文本资源版本化存入 SQLite。运行时只注入当前书籍、当前用途已启用的指令，不开放 Harness 自带 Skill、文件、Shell、Web 或子 Agent 权限。

研究任务可以选择 `presentation` 交付物。这类任务现在分为两个可审计 Run：首先按普通研究任务生成保留证据、数据口径、竞争解释与边界的完整 Markdown 报告；然后由无工具权限的 `knowledge_task_presentation_plan` Run 注入 `book-presentation` Skill，把报告编辑为严格 JSON `PresentationSpec`。服务端校验受众、中心主张、页数、文本密度、版式多样性、版式专用字段和 `S<n>` 引用；无效规格会带精确错误完整修复一次，不静默截取或降级回旧圆点模板。

通过校验后，受控 Rust 生成器将开场、强观点、左右对照、流程、数字、引语、证据与收束等布局渲染为可编辑 OOXML `.pptx`，并检查核心包、主题、母版、页面数和构图标记。二进制文件存入应用受管 artifacts 目录，SQLite 保存哈希、大小、策划 Run/Skill 和引用快照；研究 Run 仍是任务主报告，不会被较新的策划 JSON 替代。前端可以直接下载成果，当前不提供 PPTX 应用内预览。

工作台读写边界已在 Store 而非仅在页面实现：`active` 才能创建任务、启动 Run、生成或处理变更集，暂停/归档保留只读查询；同步和编译还会实时检查书架路径。实体浏览返回 `offset/limit/total/has_more`，关系查询严格限定单一知识库并用有界 BFS 查找最多八层路径。人工编辑和阅境轩摘录都创建审计 Run 与 `human_protected` 高风险变更集，批准前不触碰正式实体；阅境轩摘录必须重新定位到当前 source span，无法定位时拒绝伪造引用。

结构化编辑复用同一变更集应用器。合并候选包含目标和来源实体的期望 revision，事务内先保存版本、合并引用/论断/出向关系，再归档来源并把未参与合并实体的入向关系重定向到目标；任何 revision 冲突都会使整组变更失败。拆分候选在同一事务中归档原实体并创建两个至十二个继承原始引用的新实体，避免把未经核验的旧论断和关系自动复制到每个部分。桌面关系画布只查询连接度最高的有限节点和这些节点之间的边，关闭弹窗后卸载快照；手机端不请求画布快照，只使用洞察与有界路径接口。

`SqliteStore` 通过 rusqlite Online Backup API 生成一致快照，不直接复制主文件和 WAL。受管备份名称经过严格解析，下载接口不能解析任意路径；恢复要求精确输入 `RESTORE`，先验证 `integrity_check`、外键和迁移版本，并在替换前生成 `pre-restore` 安全快照。恢复完成后再次执行迁移、完整性检查并清除 FTS 内容版本标记，随后由核心表重建两个全文索引；若恢复后迁移或校验失败，会自动换回操作前快照。迁移、知识库删除和超过五十项的大变更集应用前也会自动快照，默认保留七份，可通过 `storage.backup_retention` 调整。

`book_wiki_export` 在数据库锁定的一致读取窗口内直接把各表逐行写入压缩包。JSON 导出使用 JSONL 避免把来源片段全集装入内存；Markdown 导出生成入口、实体页、来源清单、问答与研究任务报告。导出文件只进入数据库相邻的受管 `exports` 目录，不位于任何 Reader 原书目录，因此不会被同步器反向摄入。

迁移 021 为研究任务增加 `attempt_count/max_attempts/next_attempt_at/lease_expires_at/last_heartbeat_at`。领取任务时原子建立 45 秒租约，执行期每 15 秒续期；租约过期会重新排队，失败按 5 秒起始的指数退避重试，耗尽三次才进入 failed。部分唯一索引限制每个知识库最多一个 running 任务。DeepSeek Harness 仍采用每次 Run 独立 ACP 子进程，会话结束、取消或超时后连接和子进程立即释放，不维持空闲常驻池。

实体分页已经下推为 SQLite `LIMIT/OFFSET`，查询总数单独聚合；FTS 查询不再为了补足 limit 再执行正文 `LIKE` 全表扫描。发布前显式规模测试构造十万来源片段，验证目标召回和 `VIRTUAL TABLE INDEX` 查询计划；默认测试保留该用例但标为显式规模回归，避免每次开发循环重复生成大库。

迁移 022 为研究任务增加显式外部研究开关、域名白名单、请求额度与已用次数。只有带开关的 `knowledge_task_research` Run 才会获得 `book_fetch_external`；MCP 网关先验证 Run Capability，再原子消费额度，最后解析 DNS 并拒绝非公网地址。读取器固定 HTTPS/443、关闭重定向、只接受文本 MIME、限制 15 秒与 512 KiB，并把访问摘要写入 Run 事件。`book-synthesis` 和 `markdown-collection` 同批进入内置 Skill Registry；它们仍只能改变分析/输出格式，不能扩大工具权限。脚本型 Skill 继续保持不可执行，直到有独立沙箱、审批和资源配额。

迁移 024 增加 `agent_run_inspections`。`BookWikiService::run_audited` 在启动 Runtime 前保存最终有效 Prompt、稳定哈希、字符数、Skill/配置内容快照、工具白名单和证据引用；快照与 Run 一对一且运行后不可随当前配置变化。Skill 详情接口从 `skills/skill_versions/skill_files` 返回完整版本和文本资源；内置 Skill 保持只读，前端通过复制生成新的自定义 Skill。问答、研究与智能编译都把本次实际注入的 Skill 记录为 `prompt_injected`。

迁移 025 给来源编译检查点增加 `compile_fingerprint`，并把 Skill 绑定范围扩展为 `qa/research/both/ingest/all`。指纹由编译协议 revision、Runtime Profile 与模型、有效配置文档内容哈希、实际启用 Skill 的 revision 与内容哈希组成。准备和执行阶段都按当前指纹查询待编译来源；任一输入变化都会让全书来源重新进入编译，但相同指纹仍只处理内容版本变化的来源。没有启用 `ingest` Skill 时回退到内置 `book-ingest`；启用自定义编译 Skill 后只注入这些 Skill，硬编码的安全、引用和 JSON Schema 约束仍位于其上层并由服务端二次校验。

迁移 026 为 `skill_versions` 增加发布状态、父版本和变更说明，通过 `skill_version_origins` 保存许可证与适配来源；固定评测集、用例和结果分别存入 `skill_evaluation_suites/cases/runs`。评测器首版是确定性文本契约检查，按权重比较候选与当前版本，只允许通过的候选切换为当前版本；回滚只接受历史 `published` 版本。首批 v2 候选为项目独立重写内容，许可证不明确的开源项目不进入可发布来源。

迁移 027 为变更集和单项变更增加分类、引用审计和影响快照。服务端根据正式实体是否存在复核 `new/update`，仅允许模型显式把既有实体标为 `disputed`；争议变更应用后，其原子论断以 `disputed` 验证状态保存。引用审计统计条目引用、显式论断引用与继承引用，级联影响在生成候选时冻结现有论断、关系和引用数量，审核界面据此展示应用范围。模型返回空候选时创建状态为 `applied` 的 `no_material` 记录，照常推进来源检查点并把知识库标记为 ready，而不增加待审核数量。

迁移 028 增加 `skill_benchmark_cases/runs/case_results`。每次真实模型基准只发起两次聚合 Harness 调用：当前已发布版本与候选版本各处理同一评测集，避免逐样例启动进程。`skill_benchmark` Run 固定无 MCP 工具和能力令牌，Prompt 只包含所测版本指令与合成样例；输出只进入基准结果表，不进入知识变更集。服务端按必需概念、禁用内容、引用覆盖、引用有效性和非空输出确定性评分，保存逐样例回答和原始 Agent Run 引用。发布门槛同时要求离线结构评测通过、候选真实分数达到 suite 门槛、至少 80% 的样例通过且相对基线回退不超过 0.03。服务重启会把未完成基准明确标为失败，用户可重新运行，不会无限停在排队状态。普通迁移和单元测试只使用种子样例与假 Runtime，不依赖在线供应商。

迁移 029 将仓库内 `backend/skills/book-ingest/SKILL.md` 作为语义建库指令的唯一正文来源，写入 `book-ingest` v3 候选版的 `skill_files`，并保存实际 SHA-256、字节数、父版本及固定的开源方法参考。该指令明确批次输入边界、证据清单、跨片段主题归并、新增/更新/争议分流、严格 JSON 输出和提交前自检。v3 保持 `candidate`，不修改当前发布版本和运行时指纹；本轮仅验证迁移与资源可读性，不运行 Skill 质量评测或真实模型基准。后续发布仍遵守既有门槛。

迁移 030 应产品所有者要求，将 `book-ingest` v3 与其余六个内置 Skill 的完整新版正文直接发布为当前版本，不经过本次质量评测或真实模型基准。七份原文均保存在 `backend/skills/*/SKILL.md`，迁移在同一事务中写入版本文件、内容 SHA-256 和字节数。迁移 031 原地更新建库、问答和研究 Skill，迁移 032 按开发阶段约束删除所有非当前版本，迁移 033 继续原地覆盖 `book-presentation` v3，将 PPT Master 的 MIT 项目记录为方法来源，但仅借鉴策略、中间规格、质量门禁和可编辑产物思路，不复制其脚本、不执行 Skill 中的代码。每次原地替换前都删除基于旧正文的质量结果，用户自定义 Skill 和书籍绑定不受影响。

原 `Memory`、`WikiDashboard`、`WikiWorkbench`、`Explore`、`Ingest` 页面及 Wiki/Explore/Knowledge Insights handlers、旧 Markdown Wiki Engine 已移除。Vue Router 仍保留一个发布周期的静态跳转；旧 `Wiki/*.md` 不删除、不自动导入，新系统也不再读取。由此避免 SQLite Book Wiki 与 Obsidian Markdown Wiki 双写。

本地开发仍使用固定命令 `npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp`。Runtime Profile 可将 OpenAI Chat Completions、OpenAI Responses 或 Anthropic Messages 兼容供应商注入 Harness；真实密钥只从用户指定的环境变量读取。Book Wiki 只处理 Markdown 文件夹并固定使用 DeepSeek Harness。ACP 的 `UsageUpdate` 只作为真实上下文占用展示；供应商未上报输入/输出明细时，计费统计继续标为 `estimated`。

---

## 1. 架构目标

实现一套数据库原生、每书隔离、由 DeepSeek Harness 驱动的 Markdown 书籍知识系统。

必须坚持四个边界：

1. 原书是只读来源，不是 Agent 工作区。
2. SQLite 是生成知识的唯一事实来源，不与 Markdown Wiki 双写。
3. Rust 后端拥有业务状态、校验、事务、审核和权限。
4. DeepSeek Harness 只负责 Agent 执行，不拥有知识数据。

---

## 2. 总体架构

```text
┌──────────────────────────────────────────────────────────────┐
│ Vue 3                                                        │
│ 书架 │ 知识库 │ Wiki 工作台 │ 问答 │ 研究任务 │ 配置         │
└──────────────────────────┬───────────────────────────────────┘
                           │ HTTP + SSE + 持久化状态轮询
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
        Markdown source files         DeepSeek Harness Sidecar
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
│   │   └── markdown.rs
│   ├── harness/
│   │   ├── mod.rs
│   │   ├── deepseek.rs
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

`core` 不依赖 DeepSeek Harness 的进程实现类型；具体执行器只存在于 `infra/harness`。

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
| `kind` | Reader 兼容 folder / pdf；Book Wiki 初始化仅允许 folder |
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
- `source_type`: markdown；数据库旧约束中的 `pdf` 仅为已发布迁移兼容，不再产生新记录
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

### 6.3 增量同步

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

### 7.0 当前编译输出协议（semantic-contract-v3）

`backend/prompts/wiki/compile.md` 定义强制编译规则，`semantic-output.schema.json` 同时用于提示词与 Rust 的 JSON Schema 校验。方法指导来自实际启用的 ingest Skill；硬协议优先级高于书籍配置和 Skill。合法示例使用单一枚举值，不再把 `new|update|disputed` 当作可复制的字段值。优先跨片段归并，但有独立价值的单片段允许产出主题，避免把“单章”误判成无内容。

- 顶层为一个对象，`entries` 最多五项。必填类型、枚举、文本长度、数值范围和数组数量均由 Schema 检查，额外字段拒绝。
- 每条目及每条 claim 均须显式引用**本批** span ID；论断引用必须包含在条目引用中。关系只能指向已提供的其他主题或本批其他候选，不允许自连、重复 slug 和猜测目标。
- `entries: []` 必须附具体 `no_material_reason`。空正文、无理由空数组和“本批无实质新增”是不同结果；后者写入运行事件并继续既有 no-material 审计流程，不凭空制造知识。
- 解析器只接受一个完整 JSON 对象；可兼容单一围栏或无歧义的外围说明，并记录包装归一化。多个对象、截断 JSON、非法转义或字段/引用错误进入修复，不能取第一个对象就默认为成功，也不能用字符串替换猜测公式转义。
- 每批依旧最多两次模型调用。修复 Prompt 带具体字段路径、校验原因及有界的失败响应片段，明确重写整个对象。空正文的恢复与格式/字段修复共用一次重试额度；拒绝、取消和非空答案之外的运行故障不额外重试。最终失败不写入来源检查点。
- 输出通过校验后才生成 `content_md` 和变更候选，正式条目仍需审核。结构校验只能保证契约与引用范围，不能证明自然语言事实正确。

编译来源正文上限仍为每批 20,000 字符；实际批次预算从 32,000 字符 Prompt 总预算扣除协议、选中 Skill、配置与已有知识后计算，并计入来源元数据。所有分块正文完整发送，不再在构建 Prompt 时静默裁掉来源末尾。协议 revision 及强制规则/Schema 内容哈希纳入指纹，升级后仅在用户下次触发编译时重新处理来源。

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

> 当前落地版通过官方 ACP stdio 为每次调用创建隔离临时目录，同时注入短时 Run Capability 和本机 MCP 工具桥。Rust 服务继续预召回一组确定证据作为低延迟上下文，Harness 也可以在同一本书范围内按需继续检索；所有正式写入仍必须进入结构化变更集和人工审核。

#### 部署

- Node Sidecar 与应用版本一起固定，不运行 `npx latest`。
- 每个应用版本记录兼容的 Harness 版本和 Profile 哈希。
- 使用绝对可执行路径和独立 `DSH_HOME`，不读取用户全局 Profile 与凭据。
- Sidecar 按需启动，空闲保温时间可配置，默认到期自动退出；进程常驻不是业务正确性的前提。

#### 通信

- Rust 启动 Sidecar 并通过 ACP JSON-RPC/stdio 通信。
- 每个运行创建独立 Session 和绝对临时 `cwd`。
- ACP 管理提示、一次性审批和取消。
- 工具产生的进度写入 ObsidianBrain `agent_run_events`，目标前端通过 SSE 消费；当前前端使用任务状态轮询恢复。
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

模型供应商通过 Harness 的通用 `llm-pi-ai` 适配器注入，当前允许 `openai-completions`、`openai-responses` 与 `anthropic-messages` 三种明确协议。Provider Patch 只包含供应商元数据和凭据环境变量名；API Key 值永不物化到临时文件。ACP 模型选项使用 Harness 公布的 JSON 路由值 `[provider, model]`，不能只传模型名。

#### 工具桥

Sidecar 插件通过 `127.0.0.1` 调用后端专用端点，使用 Run Capability Bearer Token。插件不获取数据库路径。

### 8.4 临时工作区物化

```text
<temp>/obsidianbrain-agent-runs/{run-id}/
├── purpose.md
├── schema.md
├── instructions.md
├── AGENTS.md
├── .agents/skills/.../SKILL.md
└── outputs/
```

文件来自数据库中的确定版本。任务记录其哈希。原书不复制进可写工作区，Agent 通过工具读取来源。

任务结束后按保留策略清理；诊断 Artifact 先导入数据库或受管日志目录。

---

## 9. Agent 工具契约

Harness 只能看到当前 Run Capability 中列出的受控工具；Rust 服务继续拥有知识库边界、参数校验、额度和审核：

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
| `book_fetch_external` | conditional read | 仅显式授权的专题研究可见；读取允许域名的 HTTPS 文本并消耗任务额度 |

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

一个 Skill 由 `skills` 身份、当前 `skill_versions` 记录和对应的多行 `skill_files` 组成。候选在评测、发布期间可临时与当前记录共存，发布或编辑完成后立即删除旧记录。相对路径必须通过安全路径校验；禁止绝对路径、`..`、符号链接和可执行文件默认执行。

启用 Skill 前验证：

- 存在且仅有一个入口 `SKILL.md`
- 名称和描述可解析
- 引用文件存在
- 要求工具在目标 Harness Profile 中可用
- 声明权限没有超过知识库策略

Skill 详情查询必须一次返回当前绑定状态、当前内容及候选评测期间必要的临时版本和文件资源，不提供可长期恢复的历史正文。页面允许查看内置 Skill 的原始文本、资源哈希、权限和依赖，但保存接口继续拒绝修改内置记录；“复制为自定义”会创建独立 slug，之后编辑时 revision 递增但旧正文被删除。

绑定范围中 `both` 只代表问答与研究，`ingest` 仅进入智能编译，`all` 覆盖三类场景。语义编译把最终选中 Skill 的结构化快照直接放入 `<compile_skills>`，并把同一组 Skill ID 写入 Run 输入与检查快照，页面所见与 Runtime 所收保持一致。

### 10.4 运行检查快照

每次 `run_audited` 在调用 DeepSeek Harness 之前写入一条不可变检查快照，包含：

- 实际发送给 Runtime 的最终 Prompt、稳定哈希和字符数；
- 当前有效配置文档的 ID、作用域、名称、revision 和正文；
- 选中 Skill 的 ID、slug、revision、instructions、权限、依赖与应用方式；
- 本 Run 允许使用的工具名和显式证据 ID；
- 与 `agent_run_events` 组合后的可恢复时间线。

旧运行允许没有快照，查询接口返回 `snapshot: null`，前端显示“历史运行未记录检查快照”，不得推测当时 Prompt。检查快照用于可解释性和问题定位，不替代能力令牌校验，也不能反向授予工具权限。

迁移 031 按产品所有者要求，直接覆盖 `book-ingest`、`book-query`、`book-research` 当前内置版本的正文、内容哈希和变更说明，不创建新版本，也不保留本次替换前的 Skill 正文。三个内置身份继续指向原有 v3 版本 ID，因此页面、绑定关系和运行时选择无需迁移。由于版本 ID 被复用，迁移会删除以这三个旧正文为候选或基线的固定评测与真实模型基准结果，避免旧分数误用于新正文；逐项基准结果随父记录级联删除，独立 Agent Run 审计仍保留。候选后续发布时必须基于新基线重新评测。此轮不调用真实模型基准，也不调整页面发布自定义候选的门槛。

迁移 032 实施开发期的单版本策略：验证每个 Skill 的 `current_version_id` 后，删除所有非当前 `skill_versions`，旧 `skill_files`、来源元数据和基准明细按外键级联清理；引用旧候选或旧基线的评测/模型基准一并失效，独立 Agent Run 审计仍保留。新建数据库最终只有 7 个内置 Skill、7 个当前版本和 7 份 `SKILL.md`。后续编辑自定义 Skill、发布候选或执行回滚成功后也会立即清理同一 Skill 的其他版本，因此页面只展示当前生效内容；候选在发布前可与当前基线短暂共存，以完成质量检查。该策略用于当前快速迭代阶段，未来若重新启用历史版本，需要新增迁移与产品交互设计，不能假设旧正文仍可恢复。

Skill 区块总预算 8,000 字符，单 Skill 上限 6,000，多个 Skill 共享预算；配置总预算 4,000。正文裁剪标注 `[内容已截断]`，最终运行 Prompt 是实际生效内容的权威记录。

---

## 11. 问答实现

### 11.1 会话事实来源

对话和消息保存在 ObsidianBrain SQLite。Harness Session ID 仅为一次运行的诊断关联，不作为恢复对话的唯一依据。

当前实现使用 `knowledge_conversations`、`knowledge_conversation_scopes`、`knowledge_messages` 和 `knowledge_message_citations`。每次成功问答在同一事务中写入用户消息、助手消息及有序来源；页面加载时按知识库列出会话并恢复最近一项。继续会话时最多取最近八条消息辅助理解指代，但这些历史消息不能替代本轮召回的数据库证据。

DeepSeek Harness Adapter 读取 ACP `AgentMessageChunk`，后端在保存同一增量事件的同时通过 `/v1/knowledge/chat/stream` 发送 SSE。前端把 delta 直接追加到 Markdown 消息，完成事件再以最终答案校正正文与来源；不再对完整回答做二次播放动画。`AgentThoughtChunk` 只映射为“正在分析”阶段，不保存或展示模型内部推理文本。

### 11.2 单次问答

> 当前链路为“Rust 限定范围并预召回 → Capability 授权同书工具 → ACP 按需检索并流式生成 → 保存回答/引用”。

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

当前预载条目按 `[S<n>]` 编号并保存来源卡片。Prompt 限定模型只能使用本轮真实编号；工具补查但未预分配编号的资料须列真实 entry/span ID、标题和路径，不得伪造新编号。当前链路尚不自动把补查工具结果转换为新的可点击来源卡片，也不声称已经对每句回答完成语义引用核验。

`backend/prompts/wiki/answer.md` 为未绑定 Skill 的问答也提供基础质量规则：按问题类型作答、保留条件与版本、区分直接事实/综合/推断、比较时统一维度、先给部分可回答内容再指出具体缺口，并在关键结论旁引用。当前问题先注入，历史最多八条且每条限 480 字符，证据按剩余预算公平分配；不会由第一条长正文占满所有后续证据的空间。Prompt 还显式传入本次知识库 ID，供同书工具使用。真实流式输出链路不变。

### 11.4 阅读跳转

- 问答页点击 `[S#]` 或来源卡片时，先读取实体详情并打开可关闭的来源预览弹窗。
- 预览复用阅境轩安全 Markdown 渲染管线，并展示实体引用的路径和行号。
- 只有用户点击弹窗中的明确操作后才切换页面，避免核对来源时丢失当前问答上下文。
- Markdown：`book_id + relative_path + heading/anchor`。
- 前端通过 Reader 路由状态打开书籍，再执行定位。

### 11.5 Token 用量统计

迁移 015 在 `agent_runs` 增加输入、输出、推理、缓存读写 Token 与 `usage_source`。所有聚合只读取已完成运行，按 `finished_at` 日期和归一化调用方筛选：`knowledge_qa` 对应知识问答，`knowledge_task_*` 聚合为研究任务。

当前 ACP bridge 不转发供应商 Usage，因此问答和研究任务使用与 LLM 客户端一致的中英文启发式估算，并保存为 `estimated`；迁移前运行保持 `unavailable`，统计页单列为未上报。后续 Runtime Adapter 若收到真实 Usage，应写入 `measured`，无需改变统计 API。`total_tokens` 只计算输入加输出，推理和缓存桶作为明细展示，避免重复计数。

---

## 12. 研究任务与调度

`backend/prompts/wiki/research.md` 与覆盖后的 `book-research` 内置 Skill 共同定义研究质量规则：以少量决定结论的子问题建立证据矩阵，检查反例、竞争解释、同源重复和版本条件；证据不足时输出有限结论及具体核验方案。`refresh` 区分有旧版依据的变化与当前观察，`review` 按支持/反对/条件成立/不足逐项核验，专题研究按问题综合。presentation 交付物在此阶段也保留完整报告，不再为旧生成器提前压成短要点。外部研究仍需逐任务授权，不能编造已访问 URL。分析、自检只指导内部工作，不要求披露思维链。

`backend/prompts/wiki/presentation.md` 定义高于 Skill 的证据、安全、页数、字段和 JSON 合同；`book-presentation` 提供受众、叙事、构图、密度与逐页引用方法。策划 Run 不挂载 MCP 工具，仅使用研究报告和服务端提供的 `S<n>` 证据目录。服务端严格反序列化并拒绝未知字段、尾随文字、无效枚举、过载页、缺少专用载荷或越界引用。

### 12.1 任务状态机

状态转换由 `KnowledgeTaskService` 控制。`running` 任务必须关联一个活跃 Run；进程异常退出后转 `failed` 或 `waiting_review`，不得自动标为完成。

### 12.2 Worker

当前 Worker 为单进程、全局并发 1 的 SQLite 队列，按下一次尝试时间、更新时间和创建时间领取。运行中取消会通过 watch 通道立即发送 ACP `session/cancel`，ActiveSession 退出后 Harness 子进程组随连接回收。任务使用 45 秒租约、15 秒心跳、过期重领和最多三次指数退避；数据库部分唯一索引保证每个知识库最多一个 running 任务。

- 数据库队列按优先级和创建时间领取任务。
- 使用租约字段避免重复领取。
- 默认全局并发 1，可配置到 2；同一本书默认最多一个写任务。
- 只读问答可以与写任务并发，但读取固定 revision 快照。
- 重试使用退避并设置最大次数。

### 12.3 幂等

每个任务尝试生成独立 `run_id`，业务动作使用稳定 `task_id + attempt + phase` 幂等键。Harness 重试只能再次提交同一变更候选，不能重复应用。

---

## 13. API 设计

当前页面继续复用已有 Tool API Envelope；Skill ZIP 上传和成果下载使用资源型 HTTP 端点。下面列出的是逐步迁移后的目标资源 API，不代表每条路由已经存在。

当前新增 Tool 包括 `compile_book_knowledge_base`、`cancel_book_knowledge_compile`、`list_knowledge_change_sets`、`resolve_knowledge_change_set`、`lint_book_knowledge_base`、`save_knowledge_answer`、`cancel_knowledge_task`、`list_wiki_skills`、`get_wiki_skill_detail`、`save_custom_wiki_skill`、`evaluate_wiki_skill_version`、`start_wiki_skill_benchmark`、`get_wiki_skill_benchmark`、`publish_wiki_skill_version`、`rollback_wiki_skill_version`、`set_wiki_skill_binding`、`get_agent_run_events` 和 `get_agent_run_inspection`；智能编译、真实模型基准和研究任务的启动 Tool 都只负责入队。资源端点覆盖 Skill ZIP 上传、成果下载、数据库快照下载/上传恢复与单书 JSON/Markdown 导出；`book_fetch_external` 仅存在于带能力令牌的本机 Agent MCP 端点，不进入普通页面 Tool API。

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

问答已通过 SSE 消费原生增量；研究任务继续使用持久化 `agent_run_events`，智能编译使用知识库上的持久化阶段与进度，两者都由前端状态轮询恢复，因此允许离开页面。SSE 连接断开会触发问答运行取消，持久化事件仍是审计和恢复事实来源。

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
- 快照目录始终以实际数据库路径为基准，避免自定义数据库与默认数据目录错位。
- 只清理符合受管命名规则的快照，不触碰同目录其他文件。

### 16.2 恢复

恢复前执行 `PRAGMA integrity_check`，恢复后校验迁移版本、外键和 FTS；FTS 可从核心表重建，但实体、引用、配置和版本不可丢失。

恢复入口分为受管快照和最大 2 GB 的流式上传文件。两者都拒绝在 `agent_runs` 或 `knowledge_tasks` 处于 queued/running 时执行；上传写入临时目录，接口不接受服务端任意路径。

### 16.3 导出

导出器从数据库生成标准 Markdown Wiki 或 JSON Bundle。导出目录不能被 Agent 自动反向摄入，以免形成循环。

JSON Bundle 以 `manifest.json` 加多个 JSONL 数据集组成，覆盖来源、片段、实体、论断、关系、引用、对话和研究任务；Markdown Wiki 以 `README.md`、实体目录、逐实体页面、来源清单和报告目录组成。

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
- 普通 Run 的工具列表不含外部读取；只有任务输入持久化了显式授权后才加入 Capability。
- 外部读取使用精确域名/子域边界、原子额度、DNS 公网检查、禁重定向和响应大小/MIME/超时限制。

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
8. 删除旧页面、工具和 Wiki Engine；保留一版静态路由跳转和旧数据说明。

### 18.2 旧数据

旧 `Wiki/*.md` 不作为正式迁移依赖，不删除、不自动导入且新系统不再读取。若以后需要保留其内容，应另行提供一次性“旧 Wiki 导入”命令，将页面作为 `legacy_import` 来源进入新的审核流程。

### 18.3 可回滚性

新旧页面切换期间使用 Feature Flag。数据库迁移只新增表，不删除旧表或 app_state 键；旧代码删除前保留至少一个可回滚版本。

---

## 19. 测试策略

### 19.1 单元测试

- 状态机合法与非法转换
- 配置继承和版本冲突
- 路径规范化与知识库隔离
- 来源哈希和增量差异
- Markdown Span 定位
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

真实 DeepSeek Harness 只进入可选集成测试，不让普通单测依赖网络和真实密钥。

### 19.4 前端与端到端

- 五个页面主路径
- 两本书隔离
- 书架建库与状态同步
- Markdown 引用跳转
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

- 问答、研究任务和导出恢复。
- 门槛：DeepSeek Harness 契约稳定；导出与备份可恢复。

### Phase F：清理

- 删除旧前后端、更新顶层设计和用户手册。
- 门槛：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、前端测试与构建全部通过。

---

## 21. 关键风险

| 风险 | 对策 |
|---|---|
| DeepSeek Harness 仍处 Developer Preview | 固定版本、Sidecar 隔离、适配层、契约测试，禁止业务类型依赖其内部模型 |
| 数据库成为不可重建事实来源 | 在线备份、迁移前快照、版本表、JSON/Markdown 导出 |
| Agent 输出不稳定 | 严格 Schema、受控工具、两阶段处理、变更集和事务校验 |
| 不同书籍数据串联 | 能力令牌、每表 knowledge_base_id、复合唯一约束和跨库测试 |
| 长任务占用资源 | 队列、同书写任务互斥、并发上限、取消、超时和进程组回收 |
| Skill 携带危险指令或脚本 | 数据库版本、权限声明、导入校验、默认不执行脚本；脚本型 Skill 在独立沙箱前不可用 |
| 外部研究被滥用为 SSRF 或无限爬取 | 默认无工具、逐任务显式授权、域名和公网 DNS 校验、禁重定向、原子额度、文本/大小/超时限制 |
| 外部 GPL 项目许可证影响 | 借鉴方法与交互，独立实现；不直接复制代码，除非项目明确接受 GPLv3 |

---

## 22. 完成定义

只有同时满足以下条件，Book Wiki v3 才视为完成：

- 书籍与知识库一对一且隔离。
- SQLite 是所有生成知识的唯一事实来源。
- 实体、论断、关系、引用和版本可在页面完整查看与管理。
- Markdown 引用可以稳定跳回阅境轩。
- Agent 无法直接写数据库或越权访问其他书籍。
- DeepSeek Harness 完成正式接入并通过契约测试。
- 问答和研究任务可追溯到来源、配置和 Skill 版本。
- 审核、幂等、取消、恢复、备份和导出均经过验证。
- 旧知识五页面和旧 Wiki 工作流完成退役。

Skill 上游筛选、许可证结论与首批评测门槛见 [Book Wiki 开源 Skill 来源审查](../superpowers/specs/2026-09-16-wiki-skill-source-review.md)。第三方候选在固定评测与人工发布机制完成前不得替换当前内置版本。
