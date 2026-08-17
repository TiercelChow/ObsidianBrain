# 个人任务管理模块 (Tasks) — 开发设计文档

> **文档编号**: DEV-09 | **版本**: v1.1 | **状态**: MVP 已实现 | **最后更新**: 2026-08-18
>
> **对应需求**: [个人任务管理需求设计](../requirement/09-task-management.md) | **上游设计**: [顶层设计](../top_design.md) §5.6

---

## 1. 设计目标

本模块在不改变 ObsidianBrain “Obsidian 为权威数据源”原则的前提下，提供可交互、可检索、可重建的个人任务管理能力。

技术目标：

1. 短期待办和长期任务使用统一领域模型，避免前后端出现两套状态语义。
2. Markdown 文件可被人直接阅读、备份和有限度地手工修改。
3. SQLite 仅保存可重建投影，用于列表、筛选、日历和排序，不成为第二权威数据源。
4. 所有更新采用 revision + 内容哈希的文档版本控制，避免应用与 Obsidian 同时编辑时静默覆盖。
5. 长期任务树支持多级拆分，同时具备深度、循环和跨根移动保护。
6. 桌面端和手机端共享数据与视觉语言，但针对输入方式和屏幕宽度采用不同交互。

### 1.1 不采用的方案

| 方案 | 不采用原因 |
|---|---|
| 每个子任务单独一个 Markdown 文件 | 文件数量膨胀，移动和重排需要多文件事务，离线阅读上下文割裂 |
| SQLite 为主、定时导出 Markdown | 导出失败会让 Vault 数据滞后，不符合 Obsidian 优先原则 |
| 用标题或文件路径作为任务 ID | 重命名会破坏引用和进展归属 |
| 直接在 YAML 中保存嵌套树 | 深层编辑和节点移动会产生大范围 diff，解析与迁移更脆弱 |
| 首版引入完整日历组件 | 体积和定制成本高；MVP 只需月视图、跨日条和日程列表 |

---

## 2. 总体架构

```text
Tasks.vue / LLM Tool API
          │
          ▼
      TaskService
  ┌───────┼───────────┐
  │       │           │
  ▼       ▼           ▼
Validator TreeEngine ProgressCalculator
  │
  ├──────────────┐
  ▼              ▼
TaskDocumentStore  TaskIndexStore
(Obsidian adapter) (SQLite adapter)
  │              │
  ▼              ▼
Tasks/*.md    task_* tables
权威数据源      可重建投影
```

写入顺序固定为：

```text
校验请求 → 读取最新 Markdown → 校验文档版本 → 生成新文档
       → 写入 Obsidian → 更新 SQLite 投影 → 返回结果
```

- Obsidian 写入失败：整个操作失败，SQLite 不得产生新记录。
- Obsidian 写入成功、SQLite 更新失败：任务写入仍视为成功，响应附带 `index_out_of_sync` 警告；优先登记到持久同步队列，数据库整体不可用时暂存到有界内存 dirty set，并在下次启动执行增量扫描。
- 查询默认走 SQLite；索引不可用时，对单任务读取可回退到 Markdown，批量查询返回明确的降级或同步提示。

---

## 3. 代码组织

### 3.1 后端

```text
backend/src/
├── models/
│   └── task.rs
├── core/
│   └── tasks/
│       ├── mod.rs
│       ├── service.rs
│       ├── validation.rs
│       ├── tree.rs
│       ├── progress.rs
│       ├── markdown_codec.rs
│       ├── document_store.rs
│       ├── index_store.rs
│       └── sync.rs
├── infra/
│   ├── obsidian_task_store.rs
│   └── sqlite_task_store.rs
└── tools/handlers/
    └── task_handlers.rs

backend/migrations/
└── 009_tasks.sql
```

需要同时修改：

- `backend/src/models/mod.rs`
- `backend/src/core/mod.rs`
- `backend/src/infra/mod.rs`
- `backend/src/tools/definitions.rs`
- `backend/src/tools/registry.rs`
- `backend/src/app_context.rs`（以仓库实际 AppContext 所在文件为准）
- `backend/Cargo.toml`：增加 `serde_yaml`，用于 YAML frontmatter 编解码

### 3.2 前端

```text
frontend/src/
├── views/
│   └── Tasks.vue
├── components/tasks/
│   ├── TaskToolbar.vue
│   ├── TaskList.vue
│   ├── TaskCard.vue
│   ├── TaskDetail.vue
│   ├── TaskTree.vue
│   ├── TaskNodeEditor.vue
│   ├── TaskCloseSheet.vue
│   ├── ProgressTimeline.vue
│   ├── TaskCalendar.vue
│   ├── CalendarMonth.vue
│   └── DayAgenda.vue
├── stores/
│   └── tasks.ts
├── api/
│   └── tasks.ts
└── utils/
    ├── taskDates.ts
    └── taskTree.ts
```

路由增加 `/tasks`，桌面端详情选中态使用查询参数 `?task=<uuid>`，视图使用 `?view=list|calendar`。手机端仍保持相同 URL，使浏览器前进/后退可恢复上下文。

---

## 4. 后端领域模型

### 4.1 枚举

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    Root,
    Subtask,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskImportance {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Planned,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
}
```

合法组合必须在领域校验层收口：

- `short + root`：仅允许 `open/completed/cancelled`。
- `long + root|subtask`：仅允许 `planned/in_progress/blocked/completed/cancelled`。
- `short + subtask` 永远非法。

### 4.2 核心结构

```rust
pub struct TaskNode {
    pub id: Uuid,
    pub root_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: TaskKind,
    pub role: TaskRole,
    pub title: String,
    pub description: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub importance: TaskImportance,
    pub status: TaskStatus,
    pub position: i32,
    pub closure_note: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revision: i64,
    pub archived_at: Option<DateTime<Utc>>,
}

pub struct ProgressEntry {
    pub id: Uuid,
    pub root_id: Uuid,
    pub task_id: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub note: String,
    pub percent_after: Option<u8>,
    pub created_at: DateTime<Utc>,
}

pub struct AuditEvent {
    pub id: Uuid,
    pub root_id: Uuid,
    pub task_id: Uuid,
    pub event_type: TaskEventType,
    pub from_status: Option<TaskStatus>,
    pub to_status: Option<TaskStatus>,
    pub note: Option<String>,
    pub occurred_at: DateTime<Utc>,
}
```

### 4.3 文档聚合

```rust
pub struct TaskDocument {
    #[serde(rename = "obsidianbrain_schema")]
    pub schema: String,              // "tasks-short/v1" | "tasks-long/v1"
    pub document_kind: DocumentKind, // ShortMonth | LongTask
    pub storage_month: Option<YearMonth>,
    pub root_id: Option<Uuid>,
    pub revision: i64,
    pub tasks: Vec<TaskNode>,
    pub progress: Vec<ProgressEntry>,
    pub audit: Vec<AuditEvent>,
    pub extra: BTreeMap<String, serde_yaml::Value>,
    pub freeform_notes: Option<String>,
}
```

`extra` 保留未知的顶层 frontmatter 字段，保证新旧版本或用户自定义字段经过应用编辑后不会被无意删除。

### 4.4 API 读模型

写模型保持规范化；返回前组装读模型：

```rust
pub struct TaskView {
    pub node: TaskNode,
    pub derived: TaskDerivedState,
    pub progress_percent: u8,
    pub completed_leaf_count: u32,
    pub effective_leaf_count: u32,
    pub child_count: u32,
    pub storage_path: String,
}
```

`overdue`、`active_today` 等派生字段以请求所带时区计算，不落盘。

并发令牌使用明确类型：

```rust
pub struct DocumentVersion {
    pub revision: i64,
    pub content_hash: String,
}
```

---

## 5. 校验与树算法

### 5.1 字段校验

| 字段 | 规则 |
|---|---|
| 标题 | trim 后 1–200 Unicode 字符 |
| 描述 | 不超过 10000 字符 |
| 关闭说明 | 不超过 10000 字符 |
| 进展说明 | trim 后 1–10000 字符 |
| 百分比 | `0..=100` |
| 日期 | `end_date >= start_date` |
| position | 同级内归一化为连续的 `0..n-1` |

长度按 Unicode 标量值计算，不按 UTF-8 字节数计算，避免中文内容过早触发限制。

### 5.2 树构建

长期任务在 Markdown 中平铺保存，读取后一次构建：

1. 建立 `id -> node` 映射，拒绝重复 ID。
2. 确认唯一根节点，且 `root.id == root.root_id`。
3. 确认每个子节点的 `parent_id` 存在，并且 `root_id` 与根一致。
4. 按 `(position, created_at, id)` 稳定排序同级节点。
5. 用三色 DFS 检测循环并计算深度；深度超过 20 返回校验错误。
6. 拒绝不可达的孤儿节点。

时间复杂度 `O(n)`，排序总成本不超过 `O(n log n)`。

### 5.3 节点移动

`move_subtask(task_id, new_parent_id, new_position, expected_version)`：

- 根节点不可移动。
- 新父节点必须与节点属于同一 `root_id`。
- 新父节点不可是节点自身或后代。
- 预计算移动后深度，任何后代超过 20 层则拒绝。
- 同一文档内一次性更新原同级、新同级和移动节点 position。
- 整个移动只产生一次 Markdown revision 增量，便于撤销与冲突提示。

### 5.4 进度计算

采用后序遍历，自底向上计算：

```text
completed                => 100
cancelled                => 不纳入父节点分母
最新 progress.percent    => 使用显式百分比
没有显式值且有有效子项    => 有效直接子项百分比的等权平均
没有显式值的活动叶子      => 0
```

结果四舍五入到整数。显式进度代表用户判断，优先级高于子节点聚合，但界面同时显示叶子完成数，避免误读。

---

## 6. Obsidian 文档协议

### 6.1 路径

```text
Tasks/
├── Short/
│   ├── 2026-08.md
│   └── 2026-09.md
└── Long/
    ├── redesign-reader--7d82ac11.md
    └── learn-rust--ed43b203.md
```

- `slug` 仅在创建文件时生成：保留安全的 Unicode 字母与数字，把空白归一为连字符，移除路径分隔符、控制字符和平台保留名称，最长 48 个 Unicode 字符；无可用字符时使用 `task`。
- `id8` 为 UUID 去连字符后的前 8 位。
- 路径由服务端生成，Tool API 不接受任意写入路径。

### 6.2 文档结构

文档分为三部分：

```markdown
---
<机器可读 YAML frontmatter>
---

<!-- obsidianbrain:generated:start -->
<供人在 Obsidian 中阅读的生成快照>
<!-- obsidianbrain:generated:end -->

<!-- obsidianbrain:notes:start -->
<用户自由笔记，仅长期任务存在，重写时原样保留>
<!-- obsidianbrain:notes:end -->
```

规则：

- frontmatter 是任务结构的权威表示。
- generated 区域每次保存整体重建，不从这里反向解析结构。
- notes 区域逐字保留；如果标记损坏，同步报告错误而不是猜测并覆盖。
- 短期月文件不提供自由 notes 区，避免月份文件同时承担任务外笔记。
- YAML 使用固定键顺序和 2 空格缩进，集合按 position/时间排序，减少无意义 diff。

### 6.3 文档版本

- 每个短期月份文件有一个 `revision`；其中任一待办更新都会使文件 revision 加 1。
- 每个长期任务文件有独立 `revision`；根、子任务、进展或审计更新共享该 revision。
- 节点上的 `revision` 同步记录最近修改该节点时的文档 revision，用于界面展示；并发校验以文档 revision 为准。
- API 请求中的 `expected_version` 同时包含文档 revision 与完整 Markdown 内容哈希，避免应用内并发更新或未递增 revision 的 Obsidian 外部编辑被静默覆盖。

### 6.4 编解码策略

`TaskMarkdownCodec` 是纯函数组件：

```rust
pub trait TaskMarkdownCodec: Send + Sync {
    fn parse(&self, path: &str, markdown: &str) -> Result<TaskDocument, BrainError>;
    fn render(&self, document: &TaskDocument) -> Result<String, BrainError>;
}
```

解析流程：

1. 分离 frontmatter、generated 和 notes 区域。
2. 用 `serde_yaml` 反序列化并校验 `schema`。
3. 执行字段、身份、树、状态组合校验。
4. 对文档做规范化，但同步读取阶段不自动回写。
5. 返回结构与原始 notes。

必须提供 golden tests，验证“解析 → 渲染 → 再解析”的语义等价和 notes/未知字段保留。

---

## 7. 存储抽象与 SQLite 投影

### 7.1 外部依赖抽象

```rust
#[async_trait]
pub trait TaskDocumentStore: Send + Sync {
    async fn read(&self, path: &str) -> Result<Option<String>, BrainError>;
    async fn write(&self, path: &str, content: &str) -> Result<(), BrainError>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>, BrainError>;
}

#[async_trait]
pub trait TaskIndexStore: Send + Sync {
    async fn get_document_meta(&self, path: &str) -> Result<Option<TaskDocumentMeta>, BrainError>;
    async fn replace_document(&self, projection: &TaskProjection) -> Result<(), BrainError>;
    async fn remove_document(&self, path: &str) -> Result<(), BrainError>;
    async fn query(&self, query: &TaskQuery) -> Result<TaskPage, BrainError>;
}
```

`ObsidianTaskStore` 适配现有 `ObsidianClient`；测试使用内存 fake。`SqliteTaskStore` 使用现有连接池并在事务内替换单文档的全部投影。

### 7.2 Migration 009

```sql
CREATE TABLE task_documents (
    path            TEXT PRIMARY KEY,
    document_kind   TEXT NOT NULL CHECK (document_kind IN ('short_month', 'long_task')),
    root_id         TEXT,
    storage_month   TEXT,
    revision        INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,
    indexed_at      TEXT NOT NULL,
    sync_error      TEXT
);

CREATE TABLE task_nodes (
    id              TEXT PRIMARY KEY,
    root_id         TEXT NOT NULL,
    parent_id       TEXT,
    storage_path    TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('short', 'long')),
    role            TEXT NOT NULL CHECK (role IN ('root', 'subtask')),
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL,
    importance      TEXT NOT NULL,
    start_date      TEXT NOT NULL,
    end_date        TEXT NOT NULL,
    position        INTEGER NOT NULL,
    closure_note    TEXT,
    closed_at       TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    revision        INTEGER NOT NULL,
    archived_at     TEXT
);

CREATE TABLE task_progress (
    id              TEXT PRIMARY KEY,
    root_id         TEXT NOT NULL,
    task_id         TEXT NOT NULL REFERENCES task_nodes(id) ON DELETE CASCADE,
    storage_path    TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    recorded_at     TEXT NOT NULL,
    note            TEXT NOT NULL,
    percent_after   INTEGER CHECK (percent_after BETWEEN 0 AND 100),
    created_at      TEXT NOT NULL
);

CREATE TABLE task_audit_events (
    id              TEXT PRIMARY KEY,
    root_id         TEXT NOT NULL,
    task_id         TEXT NOT NULL,
    storage_path    TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    event_type      TEXT NOT NULL,
    from_status     TEXT,
    to_status       TEXT,
    note            TEXT,
    occurred_at     TEXT NOT NULL
);

CREATE TABLE task_sync_queue (
    path            TEXT PRIMARY KEY,
    reason          TEXT NOT NULL,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX idx_task_nodes_status ON task_nodes(status, archived_at);
CREATE INDEX idx_task_nodes_dates ON task_nodes(start_date, end_date);
CREATE INDEX idx_task_nodes_kind_importance ON task_nodes(kind, importance);
CREATE INDEX idx_task_nodes_root_parent ON task_nodes(root_id, parent_id, position);
CREATE INDEX idx_task_progress_task_time ON task_progress(task_id, recorded_at DESC);
```

`parent_id` 的存在性、同根关系和无环约束由服务层校验；SQLite 外键无法直接表达跨行的全部树约束。

### 7.3 内容哈希

- 使用完整 Markdown 内容的 SHA-256 作为 `content_hash`。
- 同步时哈希未变则跳过 YAML 解析和数据库写入。
- 哈希同时用于增量变更检测和文档版本并发校验；数据库索引仍以完整 SHA-256 保存。

---

## 8. TaskService

### 8.1 服务结构

```rust
pub struct TaskService<D, I, C>
where
    D: TaskDocumentStore,
    I: TaskIndexStore,
    C: TaskMarkdownCodec,
{
    documents: Arc<D>,
    index: Arc<I>,
    codec: Arc<C>,
    path_locks: PathLockMap,
    clock: Arc<dyn Clock>,
}
```

`Clock` 也通过 trait 注入，保证逾期、今天和时间戳测试稳定。

### 8.2 单文档写入算法

```text
1. 根据 task_id 从索引解析 storage_path
2. 获取 storage_path 对应的异步互斥锁
3. 从 Obsidian 重新读取当前文档
4. parse + validate
5. 比较 expected_version.revision 与 expected_version.content_hash
6. 应用领域命令并写审计事件
7. document.revision += 1
8. render
9. Obsidian PUT
10. SQLite transaction: replace_document
11. 返回新 document_version 与 TaskView
```

不能只基于 SQLite 中的旧快照更新，因为用户可能刚在 Obsidian 中修改过文件。

`PathLockMap` 只解决当前进程内并发；文档版本令牌负责发现读取后发生的跨请求或外部编辑。锁表使用弱引用或操作结束清理，不能随访问文件数永久增长。

### 8.3 短期月文件创建竞争

创建短期待办时：

1. 由服务端时间计算 `storage_month` 和路径。
2. 获取月文件锁。
3. 文件不存在则创建 revision 1 的空文档聚合，再加入任务。
4. 文件存在则读取当前 revision 后追加。
5. 新任务写入后对整个文件做一次原子替换语义的 PUT。

同进程内锁保证并行快速创建不会丢失；跨进程外部竞争通过写后同步和文档版本冲突提示处理。若未来 Obsidian API 支持 ETag，应升级为 `If-Match` 条件写入。

需要明确剩余风险：如果 Obsidian API 不提供条件写入，外部编辑器恰好在“应用校验文档版本”与“应用 PUT”之间写入，版本令牌仍无法完全消除这一极短竞争窗口。实施时应先确认 API 是否返回并接受 ETag；若不支持，采用写后重读校验、冲突日志和保留 Vault 版本历史降低风险，不宣称具备跨进程原子比较交换能力。

### 8.4 状态变更

状态变更使用领域命令而非通用 patch：

- `close_short_task`
- `set_long_task_status`
- `reopen_task`
- `archive_task`

这样可以集中执行关闭说明、`closed_at`、后代状态检查和审计记录等不变量。

### 8.5 列表与日历查询

列表查询：

```sql
WHERE archived_at IS NULL
  AND (:kind IS NULL OR kind = :kind)
  AND (:status_count = 0 OR status IN (...))
  AND (:importance_count = 0 OR importance IN (...))
  AND (
    :query IS NULL
    OR title LIKE :escaped_query ESCAPE '\\'
    OR description LIKE :escaped_query ESCAPE '\\'
    OR closure_note LIKE :escaped_query ESCAPE '\\'
    OR EXISTS (
      SELECT 1 FROM task_progress p
      WHERE p.task_id = task_nodes.id
        AND p.note LIKE :escaped_query ESCAPE '\\'
    )
  )
ORDER BY <validated_sort_expression>, id
LIMIT :limit + 1
```

日历范围相交条件：

```sql
start_date <= :visible_end AND end_date >= :visible_start
```

- 默认只返回根任务，`include_subtasks=true` 时加入长期子任务。
- 分页采用 `(sort_value, id)` 游标，不采用 offset，避免状态更新时翻页重复。
- 排序字段只从服务端枚举映射为 SQL，禁止直接拼接客户端字符串。

---

## 9. 同步与冲突处理

### 9.1 启动与手动同步

`sync_tasks` 执行：

1. 列出 `Tasks/Short/` 和 `Tasks/Long/` 下 Markdown 文件。
2. 读取文件，计算 hash，仅解析新增或变化文件。
3. 校验 schema、路径归属、ID、状态和树结构。
4. 在单文件数据库事务内替换投影。
5. 删除已经不存在文件对应的 SQLite 投影。
6. 汇总 `created/updated/unchanged/removed/errors/conflicts`。

单个损坏文件不阻断其他文件同步；错误要带 path、错误码和可操作提示。

### 9.2 重复 ID

- 相同任务 ID 出现在两个文件中属于 `TASK_DUPLICATE_ID`。
- 同步保留第一次成功索引的记录，后续冲突文件标记 `sync_error`，不静默覆盖。
- UI 提供“在 Obsidian 中打开文件”入口，引导用户修复；MVP 不自动重写用户文件中的 ID。

### 9.3 文档版本冲突

返回 HTTP/Tool 错误：

```json
{
  "code": "TASK_VERSION_CONFLICT",
  "message": "任务已在其他位置被修改",
  "details": {
    "task_id": "...",
    "expected_version": {"revision": 7, "content_hash": "sha256:old…"},
    "actual_version": {"revision": 8, "content_hash": "sha256:new…"},
    "storage_path": "Tasks/Long/example--12345678.md"
  }
}
```

前端保留用户尚未提交的表单内容，重新载入最新数据后允许复制或重新应用；不得自动用旧内容覆盖新版本。

### 9.4 用户手改 Markdown

- 结构合法：同步后作为最新事实进入索引。
- revision 未递增但内容 hash 改变：允许同步，并记录 `external_edit_without_revision` 警告；同步后返回的新版本令牌包含新 hash，下一次应用写入从当前 revision 加 1。
- YAML 损坏：保留原文件，只在 UI 显示同步错误。
- generated 快照与 frontmatter 不一致：以 frontmatter 为准，下次应用成功写入时重建快照。

---

## 10. Tool API 设计

### 10.1 工具清单

| 工具 | 主要输入 | 说明 |
|---|---|---|
| `create_task` | kind、title、description、dates、importance | 创建短期或长期根任务 |
| `list_tasks` | filters、sort、cursor、limit | 查询任务列表 |
| `get_task` | task_id、include_tree、include_progress | 获取任务详情 |
| `update_task` | task_id、patch、expected_version | 编辑通用字段 |
| `set_task_status` | task_id、status、closure_note、cascade、expected_version | 状态变更 |
| `add_subtask` | parent_id、fields、expected_version | 创建长期子任务 |
| `move_subtask` | task_id、new_parent_id、position、expected_version | 移动或排序子任务 |
| `add_task_progress` | task_id、note、percent_after、expected_version | 追加进展 |
| `get_task_calendar` | start_date、end_date、filters | 获取日历范围任务 |
| `archive_task` | task_id、archived、expected_version | 归档或恢复 |
| `sync_tasks` | dry_run | 从 Obsidian 重建/刷新索引 |

### 10.2 请求约束

- 日期字段使用 JSON string + `format: date`。
- 所有枚举在 JSON Schema 中列出合法值。
- `limit` 默认 50，最大 200。
- `get_task_calendar` 最长查询 366 天，防止无界范围扫描。
- `update_task.patch` 仅允许白名单字段，不允许修改 `id/root_id/parent_id/kind/role/storage_path`。
- `archive_task` 仅接受短期或长期根任务 ID；子任务随长期根任务归档状态展示。
- `expected_version` 必须同时包含最近读取的 revision 和 `sha256:` 内容哈希。
- 写工具成功响应统一返回 `task`、`document_version`、`storage_path`、`warnings`。

### 10.3 错误映射

| BrainError 变体 | HTTP | Tool code |
|---|---:|---|
| `TaskNotFound` | 404 | `TASK_NOT_FOUND` |
| `TaskValidation` | 422 | `TASK_VALIDATION_ERROR` |
| `TaskVersionConflict` | 409 | `TASK_VERSION_CONFLICT` |
| `TaskDuplicateId` | 409 | `TASK_DUPLICATE_ID` |
| `TaskDocumentCorrupt` | 422 | `TASK_DOCUMENT_CORRUPT` |
| Obsidian 不可用 | 503 | `OBSIDIAN_UNAVAILABLE` |

---

## 11. 前端状态与数据流

### 11.1 Store

`stores/tasks.ts` 按实体归一化：

```ts
interface TaskState {
  entities: Record<string, TaskNodeView>
  rootIds: string[]
  childrenByParent: Record<string, string[]>
  progressByTask: Record<string, ProgressEntry[]>
  selectedTaskId: string | null
  filters: TaskFilters
  viewMode: 'list' | 'calendar'
  calendarRange: DateRange
  loading: Record<string, boolean>
  errors: Record<string, TaskError | undefined>
}
```

- 列表请求返回摘要，打开详情后再请求树和进展，避免首页加载所有历史。
- 使用 request token 或 `AbortController` 忽略过期响应。
- 乐观更新仅用于纯 UI 状态（展开、选中）；任务写入等待服务端成功后落实体，避免 Obsidian 写入失败造成假成功。
- 保存期间只锁当前任务相关控件，其他任务仍可操作。

### 11.2 日期处理

禁止直接用 `new Date('2026-08-17')` 解析全天日期，因为浏览器会按 UTC 解释并在部分时区偏移一天。

`taskDates.ts` 提供：

```ts
parseLocalDate(value: string): LocalDateParts
formatLocalDate(parts: LocalDateParts): string
compareLocalDate(a: string, b: string): number
eachDayOfInterval(start: string, end: string): string[]
```

内部以 `{year, month, day}` 或 Temporal polyfill 表示日历日期；只有显示 RFC 3339 时间戳时才转换为 `Date`。

### 11.3 路由状态

- `view`、`task`、`date` 写入 URL。
- 搜索关键词和筛选可写入 query，便于刷新恢复；临时表单内容不进入 URL。
- 从日历进入详情后，返回操作恢复之前的月份和选中日期。

---

## 12. 任务视图

### 12.1 桌面端

采用 master-detail：

```text
┌─────────────────────────────────────────────────────┐
│ 标题 / 搜索 / 筛选 / 列表-日历切换 / 新建           │
├──────────────────────┬──────────────────────────────┤
│ 分组与任务列表        │ 任务详情                     │
│ 今天 / 逾期 / 之后    │ 描述、任务树、进展时间线       │
│                      │                              │
└──────────────────────┴──────────────────────────────┘
```

- 左栏最小 320px、建议 36%；详情区承担深层树与时间线。
- 选择任务只更新详情，不整页重载。
- 长期任务树使用缩进、连接线和展开按钮表达层级；重要程度只用克制的色点/标签，不整卡高饱和着色。
- 拖拽必须提供插入线、父级高亮、不可放置状态和 Esc 取消。

### 12.2 手机端

- 首页为单列任务列表，点击进入独立详情层；不压缩成左右栏。
- 筛选、新建、关闭、移动使用底部 sheet；sheet 可被下滑中断，并保留未提交输入。
- 长期树默认显示当前路径与直接子项；提供“展开全部”，避免 20 层树在窄屏横向溢出。
- 子任务缩进设置上限，超过后以路径面包屑而不是继续缩窄正文。
- 主要操作位于拇指可达的底部操作区；键盘弹出时操作区跟随可视 viewport，不遮挡文本框。

---

## 13. 日历视图

### 13.1 数据与布局

- 默认月视图，显示 6 周网格以保持页面高度稳定。
- 每次请求可见网格的完整日期范围，不只请求自然月。
- 单日任务显示为日程点/短条；跨日任务在周内连续绘制，跨周时分段。
- 同一日按 `urgent > high > normal > low`、开始日期、标题稳定排序。
- 每格最多直接展示 3 条，超出显示“还有 N 项”；点击后在日程面板展示全部。

### 13.2 桌面端

月历与右侧“选中日期日程”并排。点击任务打开详情；点击空白日期打开快速创建，并预填该日期。

### 13.3 手机端

顶部使用紧凑月份网格，只显示任务指示点和选中态；下方显示选中日的完整日程列表。左右滑动切月可中断，不能阻塞页面纵向滚动。

### 13.4 组件实现

MVP 使用 CSS Grid + 纯日期工具实现，不引入完整日历套件：

- 需求只有月网格、范围条和日程列表。
- 可复用现有主题变量，控制包体积。
- 通过 `ResizeObserver` 只在容器宽度变化时重新计算行宽，不在滚动中持续测量布局。

若后续加入周/日时间轴、拖拽改期、重复规则和外部日历同步，再评估引入专业日历库。

---

## 14. 动效、材质与无障碍

动效遵循项目现有 Apple 风格，但以状态反馈为目的：

| 场景 | 建议 |
|---|---|
| 列表新增/完成 | 160–220ms opacity + transform，完成后先反馈再移出筛选结果 |
| 详情切换 | 内容轻微淡入与 4–8px 位移，不对大面积容器做 blur 动画 |
| 树展开 | 高度变化由内容驱动；旋转展开图标，允许连续点击中断 |
| Sheet | 弹簧式位移，背景仅使用静态材质；支持拖动取消 |
| 切月 | 跟手位移，松手后按距离和速度决策完成或复位 |

实现要求：

- 动画优先只改变 `transform` 和 `opacity`，避免滚动过程中动画 `filter/backdrop-filter`。
- `prefers-reduced-motion: reduce` 时取消位移、弹簧和视差，仅保留即时或短淡变。
- 所有拖拽都有按钮/菜单等价路径，不能把拖拽作为唯一操作方式。
- 触控目标不小于 44×44 CSS px；键盘焦点清晰可见。
- 树使用 `role="tree"/"treeitem"`，支持方向键、Home/End、Enter/Space。
- 重要程度和状态不能只用颜色表达，必须配合文字或图标。
- 屏幕阅读器通过 `aria-live="polite"` 获知保存成功、冲突和任务完成。

---

## 15. 性能与资源控制

### 15.1 后端

- 列表只读 SQLite 摘要，不解析全部 Markdown。
- 单个长期任务详情只读取一个文件。
- 同步按 hash 增量解析；单文件事务更新。
- `list_tasks` 默认 50 条、最大 200 条。
- 任务树后端限制 5000 个节点/文档、20 层；超过限制返回明确错误，防止异常文件耗尽内存或栈。
- 树遍历使用迭代方式或受控深度，禁止对未校验输入做无界递归。
- Path lock、解析缓存和详情缓存都设置容量或生命周期上限。

### 15.2 前端

- 长列表超过 200 项启用虚拟化；普通列表不提前引入虚拟化复杂度。
- 详情切换时释放不再使用的监听器、drag preview、ResizeObserver 和 AbortController。
- 日历只渲染当前 6 周，不在 DOM 中保留前后无限月份。
- 进展时间线分页加载，首屏默认 30 条。
- Markdown 描述延迟渲染，仅选中任务显示完整内容。

### 15.3 性能目标

| 场景 | 目标（本地服务、索引已就绪） |
|---|---:|
| 10,000 个已索引节点的列表首屏（返回 50 条） | P95 < 300ms |
| 月历范围查询 | P95 < 200ms |
| 单任务详情（500 节点、1000 进展） | P95 < 500ms |
| 普通写入到界面确认 | P95 < 500ms，不含 Obsidian 外部异常 |
| 前端切换列表/日历 | 60fps 目标，无持续布局抖动 |

---

## 16. 测试设计

遵循 TDD：每个领域不变量先写失败测试，再实现最小代码并重构。

### 16.1 单元测试

**模型与校验**

- 短期待办拒绝 long-only 状态。
- 结束日期早于开始日期时失败。
- 中文标题按字符而非字节限制。
- 进度百分比边界 0/100 成功，越界失败。

**树**

- 平铺节点构建正确层级和顺序。
- 孤儿、重复 ID、循环、跨根父节点被拒绝。
- 移动到后代失败；移动后 position 连续。
- 第 20 层允许，第 21 层拒绝。

**进度**

- 完成节点为 100%。
- 取消节点不进入聚合分母。
- 最新显式进度覆盖子项聚合。
- 叶子完成数统计正确。

**Markdown codec**

- 短期和长期 golden fixture 往返等价。
- 自由 notes 和未知 frontmatter 字段被保留。
- 标记损坏、schema 不支持、重复 ID 返回精确错误。
- Markdown 特殊字符、中文、多行文本正确转义。

### 16.2 服务测试

用 fake `TaskDocumentStore`、fake `TaskIndexStore` 和 fake `Clock`：

- 创建短期任务写入正确月份。
- 修改日期不迁移月份文件。
- Obsidian 写入失败时索引不变。
- 索引更新失败时返回成功与同步警告。
- stale revision 或内容哈希返回冲突且不写文件。
- 根任务有活动后代时不能直接完成。
- 并行修改同一文件被路径锁串行化。
- 不同长期任务可并行写入。

### 16.3 同步测试

- 首次同步、增量未变化、文件更新、文件删除。
- 单个损坏文件不阻断其他文件。
- 跨文件重复 ID 被报告且不覆盖。
- 外部修改未增加 revision 时产生警告。
- SQLite 清空后可从 Vault 完整重建。

### 16.4 Tool 与前端测试

- JSON Schema 必填字段、枚举和 limit 上限。
- Tool 错误映射保持稳定。
- 列表筛选、游标分页、日期范围相交查询。
- 任务视图 URL 状态恢复。
- 手机端 sheet 焦点管理和键盘遮挡。
- 月历跨月、跨年、闰年、周起始日。
- `YYYY-MM-DD` 在 UTC±时区不偏移。
- reduced motion 下不运行位移动画。
- 端到端覆盖：创建短期 → 日历可见 → 关闭带说明；创建长期 → 两级拆解 → 添加进展 → 完成。

---

## 17. 可观测性

关键日志使用结构化字段：

```rust
tracing::info!(task_id = %task_id, kind = ?kind, path = %path, "任务创建完成");
tracing::debug!(path = %path, old_revision, new_revision, "任务文档已更新");
tracing::warn!(path = %path, error = %error, "任务索引更新失败，已加入同步队列");
tracing::warn!(path = %path, task_id = %task_id, "同步发现重复任务 ID");
```

不得记录完整描述、关闭说明或进展正文，避免个人信息进入日志。

建议指标：

- `task_write_duration_ms`
- `task_query_duration_ms`
- `task_sync_files_total{result}`
- `task_version_conflicts_total`
- `task_index_out_of_sync_total`

---

## 18. 实施顺序

### 阶段 A：领域与存储基础

1. 增加模型、枚举、校验和错误类型测试。
2. 实现树构建、移动和进度计算。
3. 实现 Markdown codec 与 fixtures。
4. 添加 migration 009 和 SQLite store。
5. 实现 Obsidian store adapter。

交付标准：领域/codec/store 单测通过，可从 fixture 重建索引。

### 阶段 B：服务与 Tool API

1. 实现 TaskService 创建、编辑、状态、子任务、进展和归档。
2. 加入 path lock、文档版本和 out-of-sync 机制。
3. 实现同步服务。
4. 注册 11 个 Tool 定义与 handler。
5. 接入 AppContext。

交付标准：通过 Tool API 完成两类任务核心闭环。

### 阶段 C：任务视图

1. 增加前端路由、侧栏入口、API 和 store。
2. 实现任务列表、详情、树、编辑、关闭和进展。
3. 完成桌面 master-detail 与手机 list-detail/sheet。
4. 补齐键盘、屏幕阅读器和 reduced motion。

交付标准：桌面与手机可完整管理短期/长期任务。

### 阶段 D：日历视图

1. 实现本地日期工具与范围查询。
2. 实现桌面月历、跨日条和日程侧栏。
3. 实现手机紧凑月历和日程列表。
4. 补齐跨月、闰年、时区与性能测试。

交付标准：两类任务在日历中准确展示并可进入编辑。

### 阶段 E：可靠性与验收

1. 完成损坏文件、重复 ID、冲突和恢复流程。
2. 进行 10,000 个索引节点、500 节点长期任务的性能验证。
3. 检查内存、事件监听器和 observer 释放。
4. 执行格式化、lint、全部测试和端到端回归。
5. 更新用户文档与示例文件。

---

## 19. 完成定义

- [ ] 需求文档 §10 的全部 MVP 验收项通过。
- [ ] Obsidian 中的短期月文件和长期单任务文件可独立阅读。
- [ ] SQLite 删除后能够无损重建任务查询投影。
- [ ] stale 文档版本、重复 ID、损坏 YAML 均不会静默丢数据。
- [ ] 桌面端和手机端完成创建、编辑、拆解、进展、关闭、日历闭环。
- [ ] 所有核心操作具备键盘/触控等价路径及 reduced-motion 支持。
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` 通过。
- [ ] 前端类型检查、单元测试、构建和关键 E2E 通过。
- [ ] 性能目标经过记录，不存在随切换任务持续增长的监听器或详情缓存。

---

## 20. 后续演进接口

MVP 数据模型为以下能力预留兼容空间，但暂不实现：

- `schedule` 扩展：全天日期升级到可选起止时间与时区。
- `recurrence` 扩展：基于 RRULE 的重复任务实例。
- `reminders` 扩展：提醒规则与系统通知适配器。
- `dependencies` 扩展：任务间依赖图与阻塞传播。
- `calendar_links` 扩展：外部日历事件 ID 和同步游标。
- `assignee` 扩展：若未来支持多人协作，新增负责人而不改变稳定 task ID。

任何扩展都必须通过新 schema 版本和显式迁移实现，不能在解析器中静默猜测旧数据。
