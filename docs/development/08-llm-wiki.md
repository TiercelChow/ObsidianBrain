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

迁移 017 的 Skill Registry 支持内置 Skill、自定义 Markdown 指令和安全 ZIP 导入。ZIP 限制文件数、单文件/展开大小，拒绝路径穿越、符号链接、二进制与脚本；所有文本资源版本化存入 SQLite。问答与研究运行时只把当前书籍、当前用途已启用的指令物化为本次临时目录中的 `SKILL.md`，由固定版本 Harness 的 `skill-filesystem`/`tool-skill` 按需发现和读取；`includeDefaultRoots: false` 防止扫描用户自己的 Skill 目录。业务权限仍由能力令牌决定，不开放 Shell、任意文件、Web 或子 Agent。智能编译和演示策划仍按各自的既有 Prompt 注入合同运行。

研究任务可以选择 `presentation` 交付物。这类任务现在分为两个可审计 Run：首先按普通研究任务生成保留证据、数据口径、竞争解释与边界的完整 Markdown 报告；然后由无工具权限的 `knowledge_task_presentation_plan` Run 注入 `book-presentation` Skill，把报告编辑为严格 JSON `PresentationSpec`。服务端校验受众、中心主张、页数、文本密度、版式多样性、版式专用字段和 `S<n>` 引用；无效规格会带精确错误完整修复一次，不静默截取或降级回旧圆点模板。

通过校验后，受控 Rust 生成器将开场、强观点、左右对照、流程、数字、受限横向数据图、中心关系图、引语、证据与收束等布局渲染为可编辑 OOXML `.pptx`，并检查核心包、主题、母版、页面数和构图标记。数据图仅接受 2–6 个同口径非负数值，关系图仅接受一个中心概念和 2–4 个带关系标签的相关概念；二者都使用 PPTX 文本框、形状和连接线，不生成截图。二进制文件存入应用受管 artifacts 目录，SQLite 保存哈希、大小、策划 Run/Skill、引用快照及结构化质量报告；研究 Run 仍是任务主报告，不会被较新的策划 JSON 替代。前端展示总页数、构图数量、引用覆盖、主题和逐项检查，也能直接进入演示策划 Run 的检查器；当前只提供下载，不提供 PPTX 应用内预览。

如果演示策划、修复或本地渲染失败，任务仍标记为失败并允许重新运行，但 `result_summary` 会保留已经完成的 Markdown 研究报告，并在顶部附加 PPTX 失败原因。前端对这类任务优先开放“查看报告”，不会因为二进制交付失败而把已完成研究内容隐藏在检查器后面。

工作台读写边界已在 Store 而非仅在页面实现：`active` 才能创建任务、启动 Run、生成或处理变更集，暂停/归档保留只读查询；同步和编译还会实时检查书架路径。实体浏览返回 `offset/limit/total/has_more`，关系查询严格限定单一知识库并用有界 BFS 查找最多八层路径。人工编辑和阅境轩摘录都创建审计 Run 与 `human_protected` 高风险变更集，批准前不触碰正式实体；阅境轩摘录必须重新定位到当前 source span，无法定位时拒绝伪造引用。

结构化编辑复用同一变更集应用器。合并候选包含目标和来源实体的期望 revision，事务内先保存版本、合并引用/论断/出向关系，再归档来源并把未参与合并实体的入向关系重定向到目标；任何 revision 冲突都会使整组变更失败。拆分候选在同一事务中归档原实体并创建两个至十二个继承原始引用的新实体，避免把未经核验的旧论断和关系自动复制到每个部分。桌面关系画布只查询连接度最高的有限节点和这些节点之间的边，关闭弹窗后卸载快照；手机端不请求画布快照，只使用洞察与有界路径接口。

`SqliteStore` 通过 rusqlite Online Backup API 生成一致快照，不直接复制主文件和 WAL。受管备份名称经过严格解析，下载接口不能解析任意路径；恢复要求精确输入 `RESTORE`，先验证 `integrity_check`、外键和迁移版本，并在替换前生成 `pre-restore` 安全快照。恢复完成后再次执行迁移、完整性检查并清除 FTS 内容版本标记，随后由核心表重建两个全文索引；若恢复后迁移或校验失败，会自动换回操作前快照。迁移、知识库删除和超过五十项的大变更集应用前也会自动快照，默认保留七份，可通过 `storage.backup_retention` 调整。

`book_wiki_export` 在数据库锁定的一致读取窗口内直接把各表逐行写入压缩包。JSON 导出使用 JSONL 避免把来源片段全集装入内存；Markdown 导出生成入口、实体页、来源清单、问答与研究任务报告。导出文件只进入数据库相邻的受管 `exports` 目录，不位于任何 Reader 原书目录，因此不会被同步器反向摄入。

迁移 021 为研究任务增加 `attempt_count/max_attempts/next_attempt_at/lease_expires_at/last_heartbeat_at`。领取任务时原子建立 45 秒租约，执行期每 15 秒续期；租约过期会重新排队，失败按 5 秒起始的指数退避重试，耗尽三次才进入 failed。部分唯一索引限制每个知识库最多一个 running 任务。DeepSeek Harness 仍采用每次 Run 独立 ACP 子进程，会话结束、取消或超时后连接和子进程立即释放，不维持空闲常驻池。

实体分页已经下推为 SQLite `LIMIT/OFFSET`，查询总数单独聚合；FTS 查询不再为了补足 limit 再执行正文 `LIKE` 全表扫描。发布前显式规模测试构造十万来源片段，验证目标召回和 `VIRTUAL TABLE INDEX` 查询计划；默认测试保留该用例但标为显式规模回归，避免每次开发循环重复生成大库。

迁移 022 为研究任务增加显式外部研究开关、域名白名单、请求额度与已用次数。只有带开关的 `knowledge_task_research` Run 才会获得 `book_fetch_external`；MCP 网关先验证 Run Capability，再原子消费额度，最后解析 DNS 并拒绝非公网地址。读取器固定 HTTPS/443、关闭重定向、只接受文本 MIME、限制 15 秒与 512 KiB，并把访问摘要写入 Run 事件。`book-synthesis` 和 `markdown-collection` 同批进入内置 Skill Registry；它们仍只能改变分析/输出格式，不能扩大工具权限。脚本型 Skill 继续保持不可执行，直到有独立沙箱、审批和资源配额。

迁移 024 增加 `agent_run_inspections`。`BookWikiService::run_audited` 在启动 Runtime 前保存最终有效 Prompt、稳定哈希、字符数、Skill/配置内容快照、工具白名单和预载证据引用；快照与 Run 一对一且运行后不可随当前配置变化。Skill 详情接口从 `skills/skill_versions/skill_files` 返回当前版本和文本资源；内置 Skill 保持只读，前端通过复制生成新的自定义 Skill。问答、研究选中的 Skill 标记为 `harness_native_available`（可用不代表模型一定读取）；智能编译和演示策划仍标记为 `prompt_injected`。实际工具读取证据另由迁移 037 的 `agent_run_evidence` 按 Run 记录。

迁移 025 给来源编译检查点增加 `compile_fingerprint`，并把 Skill 绑定范围扩展为 `qa/research/both/ingest/all`。指纹由编译协议 revision、Runtime Profile 与模型、有效配置文档内容哈希、实际启用 Skill 的 revision 与内容哈希组成。准备和执行阶段都按当前指纹查询待编译来源；任一输入变化都会让全书来源重新进入编译，但相同指纹仍只处理内容版本变化的来源。没有启用 `ingest` Skill 时回退到内置 `book-ingest`；启用自定义编译 Skill 后只注入这些 Skill，硬编码的安全、引用和 JSON Schema 约束仍位于其上层并由服务端二次校验。

迁移 026 为 `skill_versions` 增加发布状态、父版本和变更说明，通过 `skill_version_origins` 保存许可证与适配来源；固定评测集、用例和结果分别存入 `skill_evaluation_suites/cases/runs`。评测器首版是确定性文本契约检查，按权重比较候选与当前版本，只允许通过的候选切换为当前版本；回滚只接受历史 `published` 版本。首批 v2 候选为项目独立重写内容，许可证不明确的开源项目不进入可发布来源。

迁移 027 为变更集和单项变更增加分类、引用审计和影响快照。服务端根据正式实体是否存在复核 `new/update`，仅允许模型显式把既有实体标为 `disputed`；争议变更应用后，其原子论断以 `disputed` 验证状态保存。引用审计统计条目引用、显式论断引用与继承引用，级联影响在生成候选时冻结现有论断、关系和引用数量，审核界面据此展示应用范围。模型返回空候选时创建状态为 `applied` 的 `no_material` 记录，照常推进来源检查点并把知识库标记为 ready，而不增加待审核数量。

迁移 028 增加 `skill_benchmark_cases/runs/case_results`。每次真实模型基准只发起两次聚合 Harness 调用：当前已发布版本与候选版本各处理同一评测集，避免逐样例启动进程。`skill_benchmark` Run 固定无 MCP 工具和能力令牌，Prompt 只包含所测版本指令与合成样例；输出只进入基准结果表，不进入知识变更集。服务端按必需概念、禁用内容、引用覆盖、引用有效性和非空输出确定性评分，保存逐样例回答和原始 Agent Run 引用。发布门槛同时要求离线结构评测通过、候选真实分数达到 suite 门槛、至少 80% 的样例通过且相对基线回退不超过 0.03。服务重启会把未完成基准明确标为失败，用户可重新运行，不会无限停在排队状态。普通迁移和单元测试只使用种子样例与假 Runtime，不依赖在线供应商。

迁移 029 将仓库内 `backend/skills/book-ingest/SKILL.md` 作为语义建库指令的唯一正文来源，写入 `book-ingest` v3 候选版的 `skill_files`，并保存实际 SHA-256、字节数、父版本及固定的开源方法参考。该指令明确批次输入边界、证据清单、跨片段主题归并、新增/更新/争议分流、严格 JSON 输出和提交前自检。v3 保持 `candidate`，不修改当前发布版本和运行时指纹；本轮仅验证迁移与资源可读性，不运行 Skill 质量评测或真实模型基准。后续发布仍遵守既有门槛。

迁移 030 应产品所有者要求，将 `book-ingest` v3 与其余六个内置 Skill 的完整新版正文直接发布为当前版本，不经过本次质量评测或真实模型基准。七份原文均保存在 `backend/skills/*/SKILL.md`，迁移在同一事务中写入版本文件、内容 SHA-256 和字节数。迁移 031 原地更新建库、问答和研究 Skill，迁移 032 按开发阶段约束删除所有非当前版本，迁移 033 继续原地覆盖 `book-presentation` v3，将 PPT Master 的 MIT 项目记录为方法来源，迁移 034 再把受限数据图和中心关系图合同写入同一当前版本，迁移 035 为成果记录增加结构化校验详情。这里只借鉴策略、中间规格、质量门禁和可编辑产物思路，不复制其脚本、不执行 Skill 中的代码。每次原地替换前都删除基于旧正文的质量结果，用户自定义 Skill 和书籍绑定不受影响。

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
- 基于 Markdown AST 保留相对路径、真实标题层级和行号；支持 Setext 标题，引用、代码及公式中的伪标题不参与章节切分。
- 识别 frontmatter 边界，原文和 CRLF/LF、末尾换行不改写。相同内容哈希且抽取版本相同才复用来源版本；当前抽取协议为 `markdown-v3-structured`。
- 编译片段按结构切分，完整保留表格、代码块、显示公式和行内公式/代码边界。不可拆分结构可超过目标片段大小，但必须通过模型容量预检，不能静默裁掉。
- 迁移 046 的 `source_span_structures` 保存不可变标题路径、前后片段及行位置；旧片段没有位置记录时返回空对象，不伪造定位。导航标题有界，来源正文保留原样。
- 原文搜索仍使用既有 FTS/CJK 词法排序，改为从数据库读取命中附近的短预览与字符偏移；预览不是已读引用。原文工具按 Unicode 字符分页返回内容与实际行位置，所有查询保持当前来源版本及同书 scope。
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

#### 当前来源复核闭环（迁移 048）

`knowledge_source_impacts` 持久记录实体、过期来源版本、当前版本、直接受影响根实体及检测时间。同步后同时检查条目引用与论断引用，沿显式“依赖”关系传播；对比、支持等关系不被当作依赖，循环依赖也不会无限展开。原文变化、原文缺失和抽取定位变化分别记录，不把“需复核”当作“已证伪”。升级时回填已有失效依据，不改写书架或知识正文。

- 受影响主题变为 `stale`，保留正文、人工保护、引用与前后版本；旧问答仍按冻结 Run 快照回看。当前问答目录及取证拒绝这些过期主题，包括证据写入事务内的二次检查。
- 工作台显式使用 `include_stale: true` 查看历史主题及来源原因；默认实体列表、问答目录仍排除过期知识。详情最多展示二十项影响并给出总量；体检给出完整受影响数量及有界可点击实体列表。
- 增量编译不仅选取变化来源，还重读受影响主题及其依赖根主题引用的当前来源；跳过无关章节、旧来源版本及其他书籍。已分析的同一版本不会无限重跑；新的来源版本会重新触发相关输入。
- 分析检查点不等于审核通过。无实质新增不能清除过期状态；只有真实候选审核后、引用均为当前且依赖目标可用，才能解除对应影响。未解除影响时保持健康警告，不把编译显示为正式可用。
- 模型候选即使省略人工保护字段或请求 `agent_managed`，审核更新也保留已有 `human_protected`。内容审核不是解除人工保护的授权。
- 工作台提供用户显式的 `retry_knowledge_source_review`：只重读待复核主题及依赖根主题的当前贡献来源，不删除既有分析检查点，不重读无关章节/旧版本/其他书籍。普通编译仍跳过相同已分析输入，不自动无限重试。无可用当前来源时明确要求先同步或补回；已有候选待审、未同步、暂停或正在编译时拒绝，并在入队数据库更新内检查执行/同步/审核状态。
- 显式复核有独立报告身份与尝试幂等键，不误返回之前同版本的已应用“无实质结果”。失败保留旧知识、检查点及过期影响；成功分析也不自动解除影响，只有当前依据的实际候选审核后才能恢复可用。
- `propose_knowledge_entry_archive` 形成只改状态的高风险人工审核。迁移 050 保存服务端归档授权、同书实体/基线及完整前后快照哈希；授权和候选在同一事务内生成，候选里的自称历史标记不构成权限。创建与批准检查 revision、活动知识库和载荷完全匹配，拒绝改写事实、并发过期基线、来源章节、已归档或重复待审操作。
- 批准历史归档仅更新状态/人工保护、revision 和历史版本，保留正文、别名、论断的原验证状态、关系和历史引用，不重新分配证据、不伪装事实核验、不清除过期影响或宣称编译完成。归档主题退出当前问答；依赖主题和缺失来源仍可能保持警告。手机确认抽屉与电脑弹窗复用 MotionModal，提交候选后仍需独立审核；提前取消不写入，关闭等待界面也不会被迟到回应强制重新打开。

---

## 7. 两阶段摄入

### 7.0 当前编译输出协议（semantic-contract-v5-knowledge-body）

`backend/prompts/wiki/compile.md` 定义强制编译规则，`semantic-output.schema.json` 同时用于提示词与 Rust 的 JSON Schema 校验。方法指导来自实际启用的 ingest Skill；硬协议优先级高于书籍配置和 Skill。合法示例使用单一枚举值，不再把 `new|update|disputed` 当作可复制的字段值。优先跨片段归并，但有独立价值的单片段允许产出主题，避免把“单章”误判成无内容。

- 顶层为一个对象，`entries` 最多十二项。每个主题必须同时提供导航摘要 `summary`、独立可读的 Markdown 正文 `content_md` 和原子证据 `claims/citations`。按主题类型保留机制、前提、步骤、完整公式及变量含义、条件与反例，不用摘要拼成正文。正文 30,000 字符是存储安全边界，不是篇幅目标。必填类型、枚举、文本长度、数值范围和数组数量均由 Schema 检查，额外字段拒绝。
- 每条目及每条 claim 均须显式引用**本批** span ID；论断引用必须包含在条目引用中。关系只能指向已提供的其他主题或本批其他候选，不允许自连、重复 slug 和猜测目标。
- `entries: []` 必须附具体 `no_material_reason`。空正文、无理由空数组和“本批无实质新增”是不同结果；后者写入运行事件并继续既有 no-material 审计流程，不凭空制造知识。
- 解析器只接受一个完整 JSON 对象；可兼容单一围栏或无歧义的外围说明，并记录包装归一化。多个对象、截断 JSON、非法转义或字段/引用错误进入修复，不能取第一个对象就默认为成功，也不能用字符串替换猜测公式转义。
- 每批依旧最多两次模型调用。修复 Prompt 带具体字段路径、校验原因及有界的失败响应片段，明确重写整个对象。空正文的恢复与格式/字段修复共用一次重试额度；拒绝、取消和非空答案之外的运行故障不额外重试。最终失败不写入来源检查点。
- 只允许无损引用集合修复，不再截短合法正文、论断或限制条件，不静默舍弃信封内的论断。跨批相同论断合并引用，不同谓词/对象条件分别保留；完整正文归并超过存储安全边界时明确失败，而不是截掉尾部后推进检查点。
- 输出通过校验后才生成变更候选，正式条目仍需审核。结构校验只能保证契约与引用范围，不能证明自然语言事实正确。

编译批次按模型声明容量、保守 token 估算、完整契约/Skill/配置、身份信息、输出和一次修复余量动态分配。未声明容量时使用明确标记的应用护栏（32,768），不把它当成模型真实容量；批次输出及期限随来源量和结构复杂度调整，并受模型上限与应用安全边界约束。所有不可拆分结构先预检，再开始模型调用；过长规则/配置明确报容量问题，不省略尾部。实际资源分配及依据写入 Run 输入，估算不冒充计费。

身份目录只加载当前书籍的轻量元数据，包含待重编译的旧身份，不把其旧正文当作新增证据；不再按前 500/20 个条目截断。每批按来源中的标题、别名及 slug 词匹配选取相关身份，在 token 预算内发送完整 JSON 行及前批候选。该筛选是规则式对齐提示，不是完备语义去重；模型与审核仍需判断真实对象。跨批相同 slug 不可静默改变已有实体类型。协议 revision、规则/Schema 哈希、供应商容量/输出/推理配置纳入指纹，升级后仅在用户下次触发编译时重新处理来源。

迁移 047 原地更新仓库默认的编译、问答、研究 Skill 正文及哈希，不新建历史版本；完整性校验及保留后续已发布版本的护栏继续生效。来源影响、显式复核和历史归档见 6.3，增量/跨批主题归并见下节；分析覆盖报告与归并结果展示已接入。批次 3 的实现/契约验证不等同于真实模型事实质量证明。

#### 完整主题归并（增量与跨批）

初次来源分析只获得相关身份信息，不把旧知识正文混成新来源。相同主题跨批积累后，或本轮命中已有主题时，增加独立的只读归并变换：`reconcile.md` 定义完整正文组织与冲突报告契约；启用的书籍配置/Skill 仍作为组织指导，但不能改变硬契约。

- 程序无损合并有效旧论断与新论断，精确保留各自来源。主题快照不再把所有条目引用拷贝给每条论断。相同措辞但谓词/对象条件不同仍保留不同原子；模型只负责组织 summary/content_md 和自报覆盖/冲突，不获准删除或改写结构化事实集合。
- 有效基线的完整正文进入归并 Prompt，包括后半段、公式、步骤和例外；过期基线正文不进入当前事实材料。混合过期主题只保留引用仍有效的旧原子，失效原子留在历史对照，不能拿新引用给旧正文“换皮”。已归档主题不被自动恢复。
- 输出仅允许 `summary/content_md/covered_claim_indices/conflicts`。论断索引必须完整且唯一，冲突只能引用真实不同原子；冲突以高风险候选保留双方，不代替人工裁决。模型自报正文覆盖不是独立事实核验。已 disputed/rejected 的同一原子也不能借重新润色自动解除争议。
- 上下文、输出和期限继续按模型能力及完整材料估算；配置、旧正文和新材料不截尾。超过容量或存储边界明确失败；空结果/格式/覆盖问题共用一次修复机会，失败不替换正式正文、不建立成功检查点。
- 候选携带读取基线时的 revision（新主题显式预期不存在），创建候选事务内检查是否仍匹配，防止迟到归并覆盖并发人工更新；审核仍有独立的乐观锁与当前引用复核。Run 保存主题、基线、预算、归并结果和冲突报告，页面通过以下分析报告展示实际完成范围。

#### 分析覆盖报告（迁移 049）

`knowledge_compile_reports` 保存一次编译的独立身份、编译指纹、当时来源总量、本次选定来源/章节数及审核关联；`knowledge_compile_report_fragments` 保存完整计划位置、来源版本、行范围、片段定位、批次状态、关联 Run、批次候选引用及无实质结果原因；`knowledge_compile_report_topics` 按主题逐条保存归并、冲突、归档排除或格式修复耗尽的结果。报告只存元数据，不复制来源正文、正式知识正文或完整运行 Artifact。

- 在预检模型容量前建立报告，资源不足时保留待规划来源及明确错误；结构化批次生成后记录完整分析计划。片段状态区分尚未分析、正在分析、已分析、无实质结果、当前批次未完成。
- 已分析是 Runtime 输出通过本批契约校验，不等于每个片段的重要知识已完整提取。同一 span 的片段可能共用候选引用，报告明确标为批次关联，不将其当作每个片段的事实覆盖证明。无实质结果保留模型给出的具体理由，不能伪装成知识已建立。
- 完成过的批次和主题在后续分析/归并失败或取消时保留；服务重启及后台异常结束关闭运行态报告，不自动重新调用模型。后续尚未开始的批次不被标成已失败或已完成。
- 审核状态从所关联变更集读取；待审核、已应用、驳回和冲突分别展示。“当次审核已应用”是历史事实，不代表当前来源仍有效；健康与来源复核仍以当前数据库状态为准。
- `get_knowledge_compile_report` 按同书 scope 返回最新或指定历史报告。来源和主题有独立偏移与完整数据库计数，单页最多一百项；前端每页二十项，可查看全部分页、较早/较新报告，并打开准确对应的待审核变更。旧运行没有报告时明确未知，不补造覆盖结果。
- 知识库卡片与 Wiki 工作台复用同一 MotionModal；电脑端弹窗、手机端底部抽屉，内容独立滚动，操作区固定，长路径换行。仅可见的运行中报告轮询，关闭后释放内容和计时器，隐藏窗口不请求焦点。

报告及契约测试不替代真实模型质量核验。来源需复核但已经完成无实质分析时，可以从工作台显式重试；不再需要的历史实体可提交保留依据的归档审核，见 6.3。

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

旧运行允许没有快照，查询接口返回 `snapshot: null`，前端显示“历史运行未记录检查快照”，不得推测当时 Prompt。研究任务检查器另外展示本次实际读取证据的对象 ID、版本和必要元数据，并区分“已注入 Prompt”与“Harness 可按需读取”的 Skill；后者不等于模型必然调用了该 Skill。检查快照用于可解释性和问题定位，不替代能力令牌校验，也不能反向授予工具权限。

迁移 031 按产品所有者要求，直接覆盖 `book-ingest`、`book-query`、`book-research` 当前内置版本的正文、内容哈希和变更说明，不创建新版本，也不保留本次替换前的 Skill 正文。三个内置身份继续指向原有 v3 版本 ID，因此页面、绑定关系和运行时选择无需迁移。由于版本 ID 被复用，迁移会删除以这三个旧正文为候选或基线的固定评测与真实模型基准结果，避免旧分数误用于新正文；逐项基准结果随父记录级联删除，独立 Agent Run 审计仍保留。候选后续发布时必须基于新基线重新评测。此轮不调用真实模型基准，也不调整页面发布自定义候选的门槛。

迁移 032 实施开发期的单版本策略：验证每个 Skill 的 `current_version_id` 后，删除所有非当前 `skill_versions`，旧 `skill_files`、来源元数据和基准明细按外键级联清理；引用旧候选或旧基线的评测/模型基准一并失效，独立 Agent Run 审计仍保留。新建数据库最终只有 7 个内置 Skill、7 个当前版本和 7 份 `SKILL.md`。后续编辑自定义 Skill、发布候选或执行回滚成功后也会立即清理同一 Skill 的其他版本，因此页面只展示当前生效内容；候选在发布前可与当前基线短暂共存，以完成质量检查。该策略用于当前快速迭代阶段，未来若重新启用历史版本，需要新增迁移与产品交互设计，不能假设旧正文仍可恢复。

迁移 038 只在内置问答/研究 Skill 仍以仓库默认 v3 为当前版本时原地更新正文。若数据库已发布后续版本（例如 v4，且 v3 已按单版本策略清理），迁移先验证当前版本及 `SKILL.md` 完整，再保留后续版本的正文和质量记录，不把用户发布的内容覆盖回默认 v3。

问答、研究的最终 Prompt 只列出已启用 Skill 的名称和说明，正文由 Harness 的原生 Skill 工具按需读取；检查快照保存物化前的选中指令版本。其它仍使用 Prompt 注入的运行受 Skill 区块总预算 8,000 字符、单 Skill 6,000 字符、配置总预算 4,000 字符约束；正文裁剪标注 `[内容已截断]`。当前原生接入只物化指令正文，不自动开放 ZIP 中的附加参考文件；如需引用资源，应先增加受控读取工具。

---

## 11. 问答实现

### 11.1 会话事实来源

对话和消息保存在 ObsidianBrain SQLite。Harness Session ID 仅为一次运行的诊断关联，不作为恢复对话的唯一依据。

当前实现使用 `knowledge_conversations`、`knowledge_conversation_scopes`、`knowledge_messages` 和 `knowledge_message_citations`。每次成功问答在同一事务中写入用户消息、助手消息及有序来源；页面加载时按知识库列出会话并恢复最近一项。继续会话时，目录规划 Run 最多读取最近十六条消息辅助理解指代，上一条助手回答最多保留六千字符，其余用户/助手消息分别最多保留一千/一千五百字符。完整历史不再进入回答 Run，历史消息不能替代本轮数据库证据。

迁移 044 增加独立的结构化会话记忆，只保存用户目标、明确约束、未解决问题及本书有效实体 ID，不把旧模型结论总结成事实。当前问题可以修改或清空旧目标/约束；简单寒暄不覆盖已有研究目标。保存必须对应同书已完成且最新的问答交换，并使用 revision 乐观锁；迟到的旧 Run、部分输出和并发冲突不能覆盖较新记忆。记忆写入失败不撤销已经保存的回答与历史。规划器读取小型意图记忆，最终回答只获得当前有效目标与约束，不继承旧引用编号。

DeepSeek Harness Adapter 读取 ACP `AgentMessageChunk`，后端在保存同一增量事件的同时通过 `/v1/knowledge/chat/stream` 发送 SSE。前端把 delta 直接追加到 Markdown 消息，完成事件再以最终答案校正正文与来源；不再对完整回答做二次播放动画。`AgentThoughtChunk` 只映射为“正在分析”阶段，不保存或展示模型内部推理文本。

### 11.2 单次问答

> 当前链路为“Rust 限定范围 → 按 token 预算浏览编译目录并制定本轮目标 → 分配自适应资源 → FTS 补充召回 → Capability 授权同书工具 → Harness 按缺口补查/报告覆盖/申请预算 → ACP 流式生成 → 校验并保存回答/引用”。应用不重写 Harness 的 Agent Loop。

```text
读取同书会话历史及意图记忆
  → 确定显式知识库范围
  → 按 token 分批浏览编译条目的标题、别名与短概述
  → 补全指代，制定目标/约束/子问题/证据类型/回答深度
  → 按目标、模型能力及实际目录规模分配资源
  → 用独立问题执行 FTS 补充召回
  → 构建隔离目录与旧结论的回答上下文
  → 启动 Harness Run，建立持久预算及实际已读证据账本
  → Agent 按需取证、报告覆盖、在硬上限内扩大软预算
  → 完成状态与引用校验后保存交换及意图记忆
  → SSE 结束事件
```

目录规划使用独立只读 Run，不开放知识工具。按序列化后的标题、别名、短概述估算 token 切页，不再使用每批一千条、最终七条等固定 top-k；每页只接受真实存在且属于该页的 ID。首个有效结果确定本轮目标、约束、子问题、证据要求、深度、范围和预期输出预算，后续页只补充候选，不覆盖已确定目标；多页候选交错合并，避免前页挤掉后页。规划有总估算负载和时间硬上限，检查器记录目录总量、实际浏览量、停止原因和对应规划 Run ID，未浏览部分不宣称已覆盖。

普通规划失败或结构无效时，保留已经验证的候选并以独立问题执行 FTS 兜底，后续 Agent 仍可补查；凭据拒绝、用户取消和授权失败不借降级重复请求权限。简单的明确寒暄可绕过目录规划直接回答，不开放知识工具。只有用户要求改变上一条回答表达且不新增书籍事实时才使用改写模式，并把旧回答标为待编辑文本、重新载入可用来源。

候选数量和初始正文数量分开：前者按必要子问题覆盖选择，后者按回答深度、书籍范围、模型容量和可用 Prompt 空间确定。正文预算公平分配，保留标题、来源定位、版本及争议提示；长正文支持后续分页读取。完整目录、未选择摘要、规划 JSON 和普通历史回答不进入最终回答上下文。最终 Prompt 只含当前原话、补全后的独立问题、有效计划/约束、实际可见证据、少量待补查 ID 及配置/Skill 名录。尚未读取的候选不是引用证据。

### 11.3 引用校验

预载正文和 MCP 实际返回的正文共用 Run 内稳定的 `[S<n>]` 账本。搜索/目录候选不分配引用；同一对象同一版本的后续分页保留编号并累积实际可见文本。工具结果先通过负载/上下文预算校验，再入账；超预算、跨书、空正文、来源版本不符或运行结束后的读取不产生虚假证据。引用记录保存对象版本、已读正文、路径/行或页区间及外部资料哈希。

完成前拒绝未分配的 `[S<n>]` 和本轮未实际读取的 `entry_id`/`span_id`。会话来源卡片及预览使用冻结账本；保存答案到 Wiki 前另检查来源当前版本，不允许旧版本证据冒充新版本。门禁验证引用归属、已读范围和版本，不等于逐句语义蕴含验证；Agent 自报的证据覆盖也不构成独立事实核验。

`backend/prompts/wiki/answer.md` 为未绑定 Skill 的问答也提供基础质量规则：按问题类型作答、保留条件与版本、区分直接事实/综合/推断、比较时统一维度、先给部分可回答内容再指出具体缺口，并在关键结论旁引用。当前问题和追问补全后的独立问题都会注入，后者只用于消解指代，冲突时以前者为准；上一轮引用编号不能继承。证据按剩余预算公平分配，不会由第一条长正文占满所有后续证据的空间。Prompt 还显式传入本次知识库 ID，供同书工具使用。真实流式输出链路不变。

### 11.4 阅读跳转

- 问答页点击 `[S#]` 或来源卡片时，先读取原 Run 对应的冻结证据并打开可关闭的来源预览弹窗，而非静默替换为现在的正文。
- 预览复用阅境轩安全 Markdown 渲染管线，并展示当时实际读取的正文、版本和来源位置。
- 旧运行未保存快照时明确告知；如提供当前版本查看入口，必须标注“非本轮证据”，不可伪造历史快照。
- 只有用户点击弹窗中的明确操作后才切换页面，避免核对来源时丢失当前问答上下文。
- Markdown：`book_id + relative_path + heading/anchor`。
- 前端通过 Reader 路由状态打开书籍，再执行定位。

### 11.5 Token 用量统计

迁移 015 在 `agent_runs` 增加输入、输出、推理、缓存读写 Token 与 `usage_source`。所有聚合只读取已完成运行，按 `finished_at` 日期和归一化调用方筛选：`knowledge_qa` 与目录规划 Run `knowledge_qa_select` 均计入知识问答，`knowledge_task_*` 聚合为研究任务。

固定版本 Harness 的 ACP 可报告上下文占用和可选累计费用，不等于完整计费 token。占用事件用于更新上下文容量/压缩后的剩余空间；费用保持独立币种信息，不拆造成输入、输出或推理 token。当前 token 估算只覆盖应用可见的初始 Prompt 和最终输出，不包含 Harness 内部工具重放、推理或压缩，应明确标记 `estimated` 及范围；迁移前无数据的运行仍为 `unavailable`。工具返回负载另作保守业务预算估算，不与计费统计相加。后续 Runtime 若真正收到供应商账单 token 才可写入 `measured`。`total_tokens` 只计算输入加输出，推理和缓存桶作为明细展示，避免重复计数。

### 11.6 软/硬预算、取证覆盖与显式恢复

迁移 043 为每个取证问答 Run 保存独立自适应预算。深度、范围、子问题及规划器预期输出决定初始 Prompt、输出、检索负载、工具调用和运行时间；仍受供应商声明的最大输出/上下文及应用硬上限约束。问答与研究未声明上下文时使用 1,048,576 token 的可覆盖应用默认值，不将其写成真实模型能力；ACP 上报容量可取代未知默认值，已声明容量则与实测值取较小值，不把已配置的大容量另裁为 1M。共享输出安全护栏为 262,144，另受供应商最大输出及实际输入/工具预留约束，不再用上下文四分之一/三分之一冒充输出能力。上下文容量不等于最大输出量，也不表示应该把上下文填满。当前单次运行最长 600 秒，目录规划最长 210 秒，应用处理上限不等于模型普遍能力。

Agent 可调用 `knowledge_get_run_budget` 查看预算，调用 `knowledge_report_evidence_coverage` 按零基子问题报告缺失/部分/冲突/已有依据，引用使用已读账本中的一基编号。`knowledge_request_budget_extension` 必须给出具体缺口，只扩大软上限、不提高硬上限/截止时间或权限；扩展最多三次，后续扩展还要求有新增实际已读证据，重复读取不能伪造进展。管理工具有独立有界额度，数据取证达到软限后仍可请求扩展；管理负载也不能突破真实上下文硬上限。实时 ACP 占用允许跟随 Harness 压缩更新，累计负载计数不因压缩归零。

检查器展示目标、子问题、目录实际浏览量、预算、覆盖自报、扩展原因及限额停止原因，不展示隐藏思维链，也不把未报告的子问题推定为完成。迁移 045 同步默认问答 Skill 合同，遵循保留后续已发布版本的既有规则。

token/轮数截断保留部分答案并标记未完成，不保存为正式成功回答。前端提供明确的“继续完成完整答案”：携带原问题、原会话和失败 Run ID，服务端验证其同书范围及截断原因，建立新 Run、重新规划/取证，在模型硬能力内调整输出预算。旧草稿仅作待完善文本，旧 `[S<n>]` 移除且必须重新引用，新 Run 记录父 Run ID；用户取消、凭据拒绝和服务故障不自动套用此恢复路径，也不无限续写。

问答规划的 `expected_output_tokens` 是可见正文估计，推理策略非 `off` 时额外预留有界输出空间，供应商声明的最大输出优先。最终回答在 Harness 明确返回 `stop_reason=max_tokens` 且没有正文或只有部分正文时自动扩容，最多两次；保留本轮目标、计划和候选，但以新 Run 重装配输入与本轮证据，不重复目录规划，也不沿用失败 Run 的 S 编号。部分草稿剔除旧引用后仅作为完整重写的提示，流式前端遇到下一回答 Run 时丢弃旧增量，不能拼成伪完整答案。每次新 Run 保存父 Run、预算与扩容事件，运行时上报容量和明确配置仍约束下一次申请；成功后才写会话。达到硬上限或两次仍截断时明确区分诊断；显式恢复先复用失败 Run 的实测上下文容量并核对新模型输出上限，若无法超过上次申请则在目录规划及付费回答前拒绝同预算重放。前端保留未完成正文，硬上限时隐藏误导性的“继续完成”按钮；取消或其他错误也不会伪装成功。

非截断运行失败采用保守的临时故障白名单：Harness 的限流、明确 5xx、连接重置/中断和偶发空正文可重试；HTTP 状态码必须出现在 `HTTP`、`status` 或 `code` 等状态语境里，不能把正文中的 5000 token、503 条引用误读成 500/503 错误。401/403、凭据或额度不足、4xx 参数、拒绝、上下文/输出硬限、轮次上限和阶段超时不自动重复。问答仅在同一请求中重试一次回答 Run，保留原规划和证据，间隔两秒且流关闭时立即取消等待；失败 Run 与父子关系留在检查器。研究任务的 SQLite 退避队列仅在核心服务把错误判定为临时故障时才重新排队，其他失败直接呈现并保留已完成阶段；不能因为重试配额尚有余额就重复扣费执行确定性错误。

---

## 12. 研究任务与调度

### 创建前的用户简报（迁移 053）

任务创建现在分为目标填写和偏好确认两步。`preview_knowledge_task_brief` 是独立、无知识工具的短 Harness Run：只根据任务题目、说明、类型和交付形式，返回严格 JSON 的目标概述、建议值、至多四个需要用户决定的字段、各字段的具体选择问题及对成品的影响，以及真实歧义提示，不读取书籍内容、不启动研究。字段说明在确认页紧贴对应选择器展示；旧模型响应缺少说明时保留重点字段标记与手动选择，不因此阻断创建。预分析失败时前端开放手动设置，不能把模型建议当作用户授权。`ResearchBrief.confirmed` 仅由创建页面在用户确认后设置；无简报的旧任务以 false 迁移，不被误认为用户指定风格。

创建弹窗每次打开会重新读取所选知识库的生命周期、来源同步、编译和待审核计数，并在付费的诉求预分析前展示确定性的“材料准备情况”。暂停或归档的库不能进行预分析或创建；来源目录缺失、同步/编译未就绪、只有章节索引或有待审核候选时给出具体提醒和知识库处理入口，但保留基于当前已保存资料继续研究的选择。刷新失败时暂停预分析和创建并允许重试。这里展示的是状态快照，不是模型对题目可回答性的判断；可检索条目数包含章节索引，不等于正式语义实体数，也不保证当前证据足够。

`knowledge_tasks.brief_json` 保存完整用户简报，后续阶段从同一个任务快照读取。服务端校验所有有限枚举和强调事项长度；研究规划的 `depth` 必须匹配已确认的深度，冲突走一次格式修复。章节和综合提示在证据合同之外接收受众/用途/语气，演示策划接收主题；PPT 结构校验额外检查主题是否匹配，不依赖模型自行遵守。简报是编辑要求，不是 [S<n>] 证据，也不能扩大外部网络或写库权限。

Prompt 将确认的 `purpose/audience/tone/depth` 展开成不同的材料蓝图：决策重判据、选项与条件性建议，教学重先修概念、机制与误解，查阅重定位与例外，理解重中心判断与证据链；演示不把研究子问题一题一页地搬运。规划的 `goal` 用于内部研究，`report_title` 用于报告主标题，每个 `question` 用于内部取证，`title` 是面向读者的章节标题。新规划要求单行、陈述式材料标题；已有规划缺少该字段时回退到 `goal`，不重跑已保存阶段。用户确认决策或查阅用途时，综合阶段按开篇执行摘要或要点速览的职责写作，报告组装把它放在章节之前；教学及未确认偏好的旧任务仍在末尾综合。所有章节范围和发现矩阵按实际组装顺序保存，供 PPT 全报告选材读取，不假定综合必在最后。确定性组装仅对完全相同的开头标题和已包含在正文中的摘要去重，原阶段正文、摘要元数据、冻结引用及章节范围仍完整保存。

`backend/prompts/wiki/research.md` 与覆盖后的 `book-research` 内置 Skill 共同定义研究质量规则：以少量决定结论的子问题建立证据矩阵，检查反例、竞争解释、同源重复和版本条件；证据不足时输出有限结论及具体核验方案。`refresh` 区分有旧版依据的变化与当前观察，`review` 按支持/反对/条件成立/不足逐项核验，专题研究按问题综合。presentation 交付物在此阶段也保留完整报告，不再为旧生成器提前压成短要点。外部研究仍需逐任务授权，不能编造已访问 URL。分析、自检只指导内部工作，不要求披露思维链。

研究初始 FTS 可为空，Harness 仍可通过同书的分页编译目录、实体详情、来源搜索和分页原文工具逐步找证据。目录页最多返回 200 条轻量候选，来源搜索只给短预览，原文每次最多返回 4,000 字符；预载证据与实际读取证据写入同一账本。迁移 051 的持久研究按主题接入第 11.6 节的自适应取证，旧能力令牌保留既有规则。新主题按深度、证据要求、expected_output_tokens 与实际模型能力分配上下文/输出/工具及期限；单阶段最多 600 秒是应用护栏，不再把整个研究固定为 80 次工具。超限或超时明确失败，不把未完成内容伪装为报告。队列、租约、取消和 PPTX 策划由应用负责，阶段内的多轮判断仍由 Harness 执行。

`backend/prompts/wiki/presentation.md` 定义高于 Skill 的证据、安全、页数、字段和 JSON 合同；`book-presentation` 提供受众、叙事、构图、密度与逐页引用方法。策划 Run 不挂载 MCP 工具，仅使用研究报告和服务端提供的 `S<n>` 证据目录。标题按页面职责编写：背景、定义、过程页可直接命名对象，只有真实发现页使用有证据的结论式标题；不把内部子问题逐页搬成问答。服务端严格反序列化并拒绝未知字段、尾随文字、无效枚举、重复页标题、标题与本页 takeaway 完全重复、过载页、缺少专用载荷、负数/全零图表、数据长度错位或越界引用；文本重复校验只覆盖规范化后完全相同的文字，不冒充语义质量评测。迁移 054 在已有数据库原地刷新默认演示 Skill 的正文与哈希并清除旧质量结果；若用户已发布后续内置版本，则保留该版本，不回退到仓库 v3。

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

### 12.4 持久研究交付（迁移 051/052）

- 保存不可变目标、约束、验收条件、术语与主题问题；规划/章节采用严格单 JSON，格式或引用错误最多修复一次。单主题在完整正文、finding 矩阵和实际引用通过后才保存；应用不会重新实现原生工具循环。
- `expected_output_tokens` 只估计最终正文：请求另加 JSON/发现矩阵结构预留及推理余量；推理关闭时不额外预留，其他策略按正文规模分配保守余量。若供应商输出硬限较小，检查器记录的是扣除估计正文与结构后的实际可申请推理余量，不显示超过请求上限的虚构预留。这些是初始请求估计，不是实际计费或要求模型凑够篇幅。初始预留、当前有效输出、容量来源、扩容次数及父 Run 均记录在检查器快照。
- 新规划在供应商声明 `max_output_tokens` 时，向规划器展示有效单次输出上限、章节结构成本和推理规划余量；每个子问题必须填写必要正文估计。保存前逐题校验：结构成本为 1024 加每项额外证据要求 768 token，推理策略非 `off` 时另预留有效上限的四分之一（最多 8192）；超过剩余正文空间则触发一次规划合同修复，要求拆分主题或调整模型配置，不把无法容纳的主题写入工作区。这个余量是规划启发式，不冒充实测推理用量；没有声明单次输出上限时不从 1M 默认上下文虚构上限。历史已保存规划继续按原阶段恢复，不因新规划校验而失效。
- 规划、每个章节和综合阶段分别最多进行两次输出截断扩容，与一次格式修复独立；扩容重新生成本阶段完整 JSON，不拼接不完整对象、不复用旧 S 编号、不重做已完成章节。输出按需倍增，同时扣除完整输入与工具空间，遵守配置和本轮 ACP 观测容量。每次失败 Run 保留部分正文，父子 Run 与预算变化可追溯。取消、拒绝、权限、存储故障不走输出扩容；达到硬限或阶段扩容耗尽时，把终止原因写到任务及失败阶段，避免阶段界面只显示原始 `max_tokens`，也不借全任务队列继续重复请求。任务全局三次退避仍用于其他可恢复故障。用户显式恢复规划、章节或综合阶段时，先读取失败 Run 的上次输出申请与实测上下文容量；当前模型无法提供更高有效输出上限时在调用前拒绝同预算重放，容量提升后才从更高预算继续该阶段。
- 迁移 052 修复已部署 v51 阶段表不允许 `synthesis` 的旧 CHECK 约束；不能通过改写 v51 升级已有数据库。升级前自动备份，在事务中重建阶段表及其历史版本子表，逐字段保留规划、正文、发现、证据、运行/成果身份、历史版本和冻结基线。外键保持开启，避免重建父表时级联丢失历史版本；书架和任务不重置。
- JSON 格式/字段/引用合同校验失败，且当前执行器仍拥有阶段写入权时，才允许一次模型修复。数据库、运行故障、取消或失去领取权直接返回原错误，不向模型发送存储错误、不让二次格式报错误盖原始故障。阶段由标签确定，模型不能额外输出 `kind/phase/type` 或外层包装。
- 阶段失败仅恢复未完成项；当前来源改变时只失效实际依赖的章节及下游报告/校验/PPT。历史正文和引用快照保留。`run_id` 表示当前阶段尝试，`content_run_id` 表示保留正文的原始成果身份，不能在恢复中误标新 Run。
- `research_execution_epoch` 每次领取/开始均递增且不因手动重试重置；重试次数独立用于退避和最大三次限制。阶段领取 UUID、epoch 和活跃租约共同保护阶段、任务完成/失败、候选回写、PPT 成果和 MCP；迟到任务不能延长过期租约、消耗新额度或移除新取消句柄，心跳覆盖 PPT 策划与渲染。
- review/refresh 规划可选真实编译对象 ID；保存时在同一事务冻结完整正文、具体主张、来源版本与原文。原生 `knowledge_get_research_baseline` 仅可按当前 Run 的 task/question 分页读取这些旧版输入，不分配 S 编号。每个选定对象必须有对应发现，主张 ID 必须属于该冻结对象；找不到基线时只允许 partial/missing，不宣称已核验或发现变化。
- 全部章节保存后进行独立 synthesis 阶段，输入完整发现/限制/版本及统一目标，按需通过 `knowledge_get_research_section` 分页读任意完整章节。旧编号中性化，新增事实引用仍须回读当前实体/原文；每个主题必须有精确对应版本的 section_checks，覆盖术语、条件/版本、比较维度、反例、同源重复与争议。判定是模型公开自报，不冒充独立事实证明；不同条件不强行统一，原始章节保留。综合中补读的来源也进入变更失效链；独立失败恢复只重做未完成综合，不重生成已完成章节。
- synthesis 的初始 `integration_manifest` 先扣除实际基础 Prompt 和保留空间，再按剩余预算投影：能完整容纳就保持原矩阵；否则先保留全部 `question_id`/revision/发现及引用对象计数，再按预算预载标题、摘要和少量发现，记录遗漏摘要/发现数量与 `complete=false`。若连身份与版本都无法容纳则在模型调用前明确失败，不静默截断。新增仅综合阶段可用的 `knowledge_get_research_manifest`，可用 `question_id` 选择单章或不指定选择全部，按字符偏移分页并返回范围、`has_more` 和内容哈希；它只返回旧章节成果，不产生本轮 S 引用。与 `knowledge_get_research_section` 一样受 Run Capability、任务和同书范围约束。Agent 对未核查部分不得自报一致；旧版完整矩阵仍保留在数据库，不因提示投影改写。
- synthesis 输出预算另按章节数计算：逐章对照 JSON 结构估计为 `512 + 128 × 章节数` token，最低综合正文估计为 `512 + 64 × 章节数`，推理未关闭时额外保留有效单次输出上限的四分之一（最多 8192）。这些是可行性规划余量，不是供应商实测账单。若最小结构、正文和推理余量无法同时放入当前有效输出上限，则在调用前返回 `research_synthesis_output_hard_limit`；已完成章节不重跑。能够容纳时把本阶段正文估计收敛到剩余空间，记录原估计与实际分配，并提示模型只压缩重复综合段落，不能删逐章检查或掩盖冲突；较大模型仍保留动态正文与推理请求空间。
- 完整报告确定性合并所有章节及综合对照，引用按对象/版本统一编号，同一证据的实际读取范围合并并保留原始 Run。组装是应用阶段，不伪造新的模型读取或费用。引用与结构检查只保证合同、来源版本及编号，不证明自然语言结论全部正确。
- PPT 容量允许时使用完整报告；超容量时根据保存的准确主题范围投影整份报告，保留所有主题的完整发现/限制、尾段及可容纳的结构片段，保护公式/表格/代码块，全局 S 编号不变。投影范围与未选字符数写入策划 Run，不将选材冒充全报告或凭空补齐未选事实。必要材料仍超过硬限则停止相同自动重试，完整报告保留。演示交付与报告检查点独立，成果通过结构/文件校验后才能完成。
- 演示策划 Run 若明确以 `max_tokens` 截断，在当前 PPT 阶段内按模型声明、ACP 实测上下文容量与完整输入空间最多扩容两次；每次失败 Run 和父子关系保留，重领阶段后只重做策划，不重跑研究章节。结构合同修复仍只有一次，与输出扩容分别计数。手动恢复时先检查上次截断的请求上限；若当前模型未提供更大输出空间，直接提示而不重放付费调用；配置提升后重新投影报告并从更高预算续做。取消、格式错误、供应商硬上限或必要输入过大不会触发同预算重复调用。

任务页面可查看业务规划、核验基线元数据、阶段状态/当前或历史成果、证据矩阵和原始 Run 快照；阶段失败可显式恢复，仍能回看旧报告。失败任务的主入口优先落在实际失败或过期的阶段及其错误说明；未建立阶段工作区的旧任务仍回退到运行检查器。阶段工作区首次打开优先定位运行中或需要处理的阶段，当前版本随轮询到的新状态自动更新；用户主动选择的历史版本保持不变，不被轮询抢回当前版。运行检查器显示实际取证、目标缺口、初始候选目标（非实际覆盖）、已配置容量或未知应用护栏、扩展/停止及 PPT 选材范围。复用系统弹窗/手机抽屉，内容滚动、操作固定；关闭/隐藏停止轮询，迟到响应不重开界面、不申请应用焦点。当前隔离契约测试不代表真实模型答案质量评测。

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
