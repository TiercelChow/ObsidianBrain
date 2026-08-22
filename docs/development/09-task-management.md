# 个人任务管理模块 (Tasks) — 开发设计文档

> **文档编号**: DEV-09 | **版本**: v1.2 | **状态**: MVP 已实现 | **最后更新**: 2026-08-22
>
> **对应需求**: [个人任务管理需求设计](../requirement/09-task-management.md) | **上游设计**: [顶层设计](../top_design.md) §5.6

---

## 1. 设计目标

本模块以 SQLite 为唯一权威任务存储，提供可交互、可检索的个人任务管理能力，不再读写任何 Obsidian Markdown 文件。

技术目标：

1. 短期待办和长期任务使用统一领域模型，避免前后端出现两套状态语义。
2. 任务数据保存在 SQLite `task_documents`、`task_nodes`、`task_progress`、`task_audit_events` 四张表中，是唯一权威存储。
3. SQLite 不可由 Vault 重建；数据库备份属基础设施话题，另行立项（这是脱离 Obsidian 存储的显式取舍）。
4. 所有更新采用 revision + 版本令牌的文档版本控制，避免并发写入时静默覆盖。
5. 长期任务树支持多级拆分，同时具备深度、循环和跨根移动保护。
6. 桌面端和手机端共享数据与视觉语言，但针对输入方式和屏幕宽度采用不同交互。

### 1.1 不采用的方案

| 方案 | 不采用原因 |
|---|---|
| Obsidian Markdown 为权威存储、SQLite 作可重建索引 | 双写一致性成本高（同步队列、脏路径、外部编辑检测），事务原子性受文件接口限制；已在任务中枢中放弃 |
| 用标题作为任务 ID | 标题修改会破坏引用和进展归属 |
| 在关系行之外再嵌套一份树形文本表示 | 深层编辑和节点移动会产生大范围 diff，两份表示易失同步 |
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
          ▼
     TaskIndexStore
     (SqliteTaskIndexStore)
          │
          ▼
      task_* tables
      唯一权威存储
```

写入顺序固定为：

```text
校验请求 → 从索引解析文档键 → 获取按路径的异步锁 → 锁内重取文档元数据
       → 从行装配文档 → 校验文档版本（OCC）→ 应用变更 → revision + 1
       → 生成新版本令牌 → 单事务 replace_document → 返回结果
```

- 写入在单个 SQLite 事务内整体替换文档的全部行（`task_documents` 头 + 节点 + 进展 + 审计）；事务失败即整体回滚，不产生部分写入，也没有“数据落盘但索引未更新”的降级路径。
- 查询直接走 SQLite；存储层即权威数据，没有第二份需要重建或同步的数据。

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
│       └── tree.rs
├── infra/
│   └── task_index_store.rs
└── tools/handlers/
    └── task_handlers.rs

backend/migrations/
├── 009_tasks.sql
└── 010_remove_task_sync.sql
```

需要同时修改：

- `backend/src/models/mod.rs`
- `backend/src/core/mod.rs`
- `backend/src/infra/mod.rs`
- `backend/src/tools/definitions.rs`
- `backend/src/tools/registry.rs`
- `backend/src/app_context.rs`（以仓库实际 AppContext 所在文件为准）

### 3.2 前端

```text
frontend/src/
├── views/
│   └── Tasks.vue
├── components/tasks/
│   ├── TaskTree.vue
│   └── TaskCalendar.vue
├── stores/
│   └── tasks.ts
├── api/
│   └── tasks.ts
└── utils/
    ├── taskDates.ts
    ├── taskHierarchy.ts
    └── taskPayloads.ts
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
    pub document_kind: TaskDocumentKind, // ShortMonth | LongTask
    pub storage_month: Option<String>,
    pub revision: i64,
    pub tasks: Vec<TaskNode>,
    pub progress: Vec<ProgressEntry>,
    pub audit: Vec<AuditEvent>,
}
```

- 文档是读、写、并发校验的基本单元：长期任务的根节点、全部子任务、进展和审计事件同属一个文档。
- 存量短期文档可能包含多个待办（历史月份文件迁移而来）；新建短期待办一律一任务一文档。
- `storage_month` 在创建时写定且不可变。

### 4.4 API 读模型

写模型保持规范化；返回前组装读模型：

```rust
pub struct TaskDetail {
    pub root: TaskNode,
    pub tasks: Vec<TaskNode>,
    pub progress: Vec<ProgressEntry>,
    pub audit: Vec<AuditEvent>,
    pub storage_path: String,
    pub document_version: DocumentVersion,
    pub progress_percent: u8,
    pub completed_leaf_count: u32,
    pub effective_leaf_count: u32,
}
```

`overdue`、`active_today` 等派生字段以请求所带时区计算，不落盘（节点行上的 `progress_percent`、`completed_leaf_count`、`effective_leaf_count` 为写入时物化的聚合指标）。

并发令牌使用明确类型：

```rust
pub struct DocumentVersion {
    pub revision: i64,
    pub content_hash: String,
}
```

写工具的统一响应只包含 `task`（`TaskWriteResponse { task }`）；`storage_path` 与 `document_version` 内嵌在 `TaskDetail` 中，没有独立的警告字段。

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

长期任务节点在文档内平铺保存（`task_nodes` 行按 `rowid` 排列，行序即文档顺序），装配后一次构建：

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
- 整个移动只使文档 revision 加 1，便于撤销与冲突提示。

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

## 6. 文档模型与版本协议

### 6.1 文档键

- 新建文档使用合成键：短期待办为 `db:short:{uuid}`（ShortMonth 种类，记录 `storage_month`），长期根任务为 `db:long:{uuid}`。
- 存量数据行保留 `Tasks/...` 形式的历史路径字符串作为 `task_documents.path` 主键；该字符串仅是文档主键，无文件系统语义，系统不再据此读写文件。
- 文档键由服务端生成；Tool API 不接受调用方指定文档键。

### 6.2 文档结构

- 文档没有文本表示；`load_document` 按 `rowid` 顺序读取 `task_nodes`、`task_progress`、`task_audit_events` 行装配聚合，行序即文档顺序。
- 文档没有 schema 标记字段、自由笔记区或未知字段保留区；结构演进完全由数据库迁移表达。
- 长期任务的树形结构不落盘为嵌套形态，由 `parent_id` + `position` 在装配时构建（见 §5.2）。

### 6.3 文档版本

- 每个文档有独立 `revision`；文档内任何节点、进展或审计更新都使该 revision 加 1。
- 节点上的 `revision` 记录最近修改该节点时的文档 revision，用于界面展示；并发校验以文档 revision + 版本令牌为准。
- 版本令牌由服务端在每次写入时重新生成：对 `"{path}:{revision}"` 取 SHA-256 十六进制摘要并加 `sha256:` 前缀。令牌随 revision 逐次写入轮换。
- API 请求中的 `expected_version` 必须与写入锁内重新获取的最新文档元数据完全一致，否则返回 `TASK_VERSION_CONFLICT`，用于发现读取之后发生的并发写入。

---

## 7. 存储层

### 7.1 存储抽象

```rust
#[async_trait]
pub trait TaskIndexStore: Send + Sync {
    async fn replace_document(&self, path: &str, content_hash: &str, document: &TaskDocument) -> Result<(), BrainError>;
    async fn find_document_by_task(&self, task_id: Uuid) -> Result<Option<TaskDocumentMeta>, BrainError>;
    async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError>;
    async fn list_tasks(&self, query: &TaskQuery, today: NaiveDate) -> Result<TaskListResponse, BrainError>;
    async fn calendar_tasks(&self, query: &CalendarTaskQuery, today: NaiveDate) -> Result<Vec<TaskSummary>, BrainError>;
}

pub struct TaskDocumentMeta {
    pub path: String,
    pub revision: i64,
    pub content_hash: String,
}
```

`SqliteTaskIndexStore` 是唯一实现，直接持有现有 `SqliteStore` 连接池；测试使用内存 fake。存储层同时承担权威数据的读写与查询投影，不再区分“文档存储”与“索引”两个角色。

### 7.2 Migration 009 + 010

`009_tasks.sql` 创建四张任务表（历史版本还带有同步队列与同步错误列），`010_remove_task_sync.sql` 删除了同步队列表和 `task_documents` 的同步错误列。当前最终结构：

```sql
CREATE TABLE task_documents (
    path            TEXT PRIMARY KEY,
    document_kind   TEXT NOT NULL CHECK (document_kind IN ('short_month', 'long_task')),
    root_id         TEXT,
    storage_month   TEXT,
    revision        INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,
    indexed_at      TEXT NOT NULL
);

CREATE TABLE task_nodes (
    id                      TEXT PRIMARY KEY,
    root_id                 TEXT NOT NULL,
    parent_id               TEXT,
    storage_path            TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    kind                    TEXT NOT NULL CHECK (kind IN ('short', 'long')),
    role                    TEXT NOT NULL CHECK (role IN ('root', 'subtask')),
    title                   TEXT NOT NULL,
    description             TEXT NOT NULL DEFAULT '',
    status                  TEXT NOT NULL,
    importance              TEXT NOT NULL,
    start_date              TEXT NOT NULL,
    end_date                TEXT NOT NULL,
    position                INTEGER NOT NULL,
    closure_note            TEXT,
    closed_at               TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    revision                INTEGER NOT NULL,
    archived_at             TEXT,
    progress_percent        INTEGER NOT NULL DEFAULT 0 CHECK (progress_percent BETWEEN 0 AND 100),
    completed_leaf_count    INTEGER NOT NULL DEFAULT 0,
    effective_leaf_count    INTEGER NOT NULL DEFAULT 0
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

CREATE INDEX idx_task_nodes_status ON task_nodes(status, archived_at);
CREATE INDEX idx_task_nodes_dates ON task_nodes(start_date, end_date);
CREATE INDEX idx_task_nodes_kind_importance ON task_nodes(kind, importance);
CREATE INDEX idx_task_nodes_root_parent ON task_nodes(root_id, parent_id, position);
CREATE INDEX idx_task_progress_task_time ON task_progress(task_id, recorded_at DESC);
```

`parent_id` 的存在性、同根关系和无环约束由服务层校验；SQLite 外键无法直接表达跨行的全部树约束。`progress_percent`、`completed_leaf_count`、`effective_leaf_count` 是写入时物化的聚合指标，避免列表查询实时递归计算。

### 7.3 内容哈希列

- `task_documents.content_hash` 保存当前版本令牌（见 §6.3），用于 OCC 并发校验。
- 令牌随每次写入轮换；没有第二数据源，因此不再承担增量变更检测职责。

---

## 8. TaskService

### 8.1 服务结构

```rust
pub struct TaskService {
    index: Arc<dyn TaskIndexStore>,
    path_locks: PathLockMap,
    clock: Arc<dyn TaskClock>,
}

impl TaskService {
    pub fn new(index: Arc<dyn TaskIndexStore>) -> Self;
    pub fn with_clock(index: Arc<dyn TaskIndexStore>, clock: Arc<dyn TaskClock>) -> Self;
}
```

`TaskClock` 通过 trait 注入，保证逾期、今天和时间戳测试稳定。

### 8.2 单文档写入算法

```text
1. 根据 task_id 从索引解析文档键（第一次查询只用于确定锁的 key）
2. 获取该文档键对应的异步互斥锁
3. 锁内重新获取文档元数据（OCC 必须基于锁后最新快照，锁外读到的 meta 可能已被并发写入越过）
4. load_document 从行装配当前文档（行按 rowid 排序）
5. 比较 expected_version 与锁内最新 revision + 版本令牌
6. 应用领域命令并写审计事件
7. document.revision += 1
8. 生成新版本令牌 sha256("{path}:{revision}")
9. 单事务 replace_document：删除并重建该文档的全部行
10. 返回 TaskDetail（内嵌新 document_version）
```

写入只涉及一个数据库事务：要么整文档替换成功，要么整体回滚。没有“权威写入成功但索引失败”的路径，也没有同步队列或脏路径集合。

`PathLockMap` 只解决当前进程内并发；文档版本令牌负责发现读取后发生的跨请求并发写入。锁表使用弱引用并在每次取锁时清理失效项，不随文档数永久增长。

### 8.3 短期任务创建

创建短期待办时：

1. 由服务端时间计算 `storage_month`。
2. 生成新 UUID 并构造 `db:short:{uuid}` 文档（revision 1，ShortMonth 种类）。
3. 单事务写入新文档。

新建待办不与他人共享文档，不存在月文件合并时代的同文档竞争；并行快速创建天然隔离。修改已有待办仍走 §8.2 的按路径锁 + OCC 流程。

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

## 9. 冲突处理

### 9.1 文档版本冲突

返回 HTTP/Tool 错误：

```json
{
  "code": "TASK_VERSION_CONFLICT",
  "message": "任务已在其他位置被修改",
  "details": {
    "task_id": "...",
    "expected_version": {"revision": 7, "content_hash": "sha256:old…"},
    "actual_version": {"revision": 8, "content_hash": "sha256:new…"},
    "storage_path": "db:long:57d6201e-527c-487f-9c79-e54c06ee1c6d"
  }
}
```

前端保留用户尚未提交的表单内容，重新载入最新数据后允许复制或重新应用；不得自动用旧内容覆盖新版本。

### 9.2 文档内完整性

- 同一文档内出现重复任务 ID 属于 `TASK_DUPLICATE_ID`，写入前校验并拒绝，不产生静默覆盖。
- 装配文档时发现根节点缺失、父子关系断裂等结构性损坏属于 `TASK_DOCUMENT_CORRUPT`，读写路径都会校验。

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

共 10 个任务工具。

### 10.2 请求约束

- 日期字段使用 JSON string + `format: date`。
- 所有枚举在 JSON Schema 中列出合法值。
- `limit` 默认 50，最大 200。
- `get_task_calendar` 最长查询 366 天，防止无界范围扫描。
- `update_task.patch` 仅允许白名单字段，不允许修改 `id/root_id/parent_id/kind/role/storage_path`。
- `archive_task` 仅接受短期或长期根任务 ID；子任务随长期根任务归档状态展示。
- `expected_version` 必须同时包含最近读取的 revision 和 `sha256:` 版本令牌。
- 写工具成功响应统一返回 `task`（`TaskWriteResponse { task }`）；`document_version` 与 `storage_path` 内嵌在 `TaskDetail` 中。

### 10.3 错误映射

| BrainError 变体 | HTTP | Tool code |
|---|---:|---|
| `TaskNotFound` | 404 | `TASK_NOT_FOUND` |
| `TaskValidation` | 422 | `TASK_VALIDATION_ERROR` |
| `TaskVersionConflict` | 409 | `TASK_VERSION_CONFLICT` |
| `TaskDuplicateId` | 409 | `TASK_DUPLICATE_ID` |
| `TaskDocumentCorrupt` | 422 | `TASK_DOCUMENT_CORRUPT` |

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
- 乐观更新仅用于纯 UI 状态（展开、选中）；任务写入等待服务端成功后落实体，避免写入失败造成假成功。
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

- 列表查询只读 SQLite 摘要行，不装配完整文档。
- 单个长期任务详情只加载一个文档。
- 写入在单事务内整体替换文档行，无第二数据源需要增量维护。
- `list_tasks` 默认 50 条、最大 200 条。
- 任务树后端限制 5000 个节点/文档、20 层；超过限制返回明确错误，防止异常数据耗尽内存或栈。
- 树遍历使用迭代方式或受控深度，禁止对未校验输入做无界递归。
- Path lock 和详情缓存都设置容量或生命周期上限。

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
| 普通写入到界面确认 | P95 < 500ms |
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

### 16.2 服务测试

用 fake `TaskIndexStore` 和 fake `TaskClock`：

- 创建短期任务生成 `db:short:` 文档并记录正确 `storage_month`。
- 修改日期不改变 `storage_month`。
- 存量 `Tasks/...` 文档键的读写不依赖文件系统。
- 写入失败时整个操作失败，数据库无部分结果。
- stale revision 或版本令牌返回冲突且不写数据库。
- 每次写入轮换版本令牌。
- 根任务有活动后代时不能直接完成。
- 并行修改同一文档被路径锁串行化。
- 不同长期任务可并行写入。

### 16.3 Tool 与前端测试

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
tracing::warn!(path = %path, task_id = %task_id, "文档内发现重复任务 ID，写入被拒绝");
```

不得记录完整描述、关闭说明或进展正文，避免个人信息进入日志。

建议指标：

- `task_write_duration_ms`
- `task_query_duration_ms`
- `task_version_conflicts_total`

---

## 18. 实施顺序

### 阶段 A：领域与存储基础

1. 增加模型、枚举、校验和错误类型测试。
2. 实现树构建、移动和进度计算。
3. 添加 migration 009/010 和 SQLite 存储层（装配、替换、查询）。

交付标准：领域/store 单测通过。

### 阶段 B：服务与 Tool API

1. 实现 TaskService 创建、编辑、状态、子任务、进展和归档。
2. 加入 path lock 与文档版本控制。
3. 注册 10 个 Tool 定义与 handler。
4. 接入 AppContext。

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

1. 完成重复 ID、版本冲突和事务回滚流程。
2. 进行 10,000 个存储节点、500 节点长期任务的性能验证。
3. 检查内存、事件监听器和 observer 释放。
4. 执行格式化、lint、全部测试和端到端回归。
5. 更新用户文档。

---

## 19. 完成定义

- [ ] 需求文档 §10 的全部 MVP 验收项通过。
- [ ] SQLite 为唯一权威存储，全模块不产生任何 Vault 文件写入。
- [ ] 写入失败时事务整体回滚，不产生半写状态。
- [ ] stale 文档版本、重复 ID 均不会静默丢数据。
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

任何扩展都必须通过显式数据库迁移实现，不能在读取路径中静默猜测旧数据。
