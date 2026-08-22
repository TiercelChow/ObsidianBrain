# 任务中枢脱离 Obsidian 同步 — 设计文档

- 日期：2026-08-22
- 状态：已批准（用户确认"彻底脱离 Obsidian"路线）
- 影响模块：Tasks（backend `core/tasks`、`infra/task_*`、tools；frontend Tasks 视图/Store/API）

## 1. 背景与决策

当前任务中枢以 Obsidian Markdown 为权威存储（`Tasks/Short/YYYY-MM.md`、`Tasks/Long/{slug}--{id8}.md`），SQLite 仅作可重建投影；每次增删改都经 Obsidian Local REST API 读写 md，另设手动 `sync_tasks` 全量重扫。

该链路存在实际拖累：双写分裂（`index_out_of_sync` 警告路径）、版本冲突处理、外部编辑检测、同步队列等可靠性承诺大多未实现（见记忆 task-management-reliability-gaps），而 md 生成区对用户价值有限。

**决策：任务数据彻底脱离 Obsidian，SQLite 成为唯一权威存储。** 删除 markdown 存储层与同步机制；vault 中现存的 Tasks/ md 文件不再被读写，去留由用户自行处置。产品其余部分（记忆引擎、笔记搜索）对 Obsidian REST API 的依赖不受影响。

## 2. 实现路线

**路线 A（采纳）：保留文档模型的"DB 直写"改造。**
`task_documents / task_nodes / task_progress / task_audit_events` 四张表结构保留，把"渲染 md → REST 写回 → 重放投影"替换为单事务直接写 SQLite。业务逻辑（状态机、级联、进度树 `tree.rs`）完全复用。

路线 B（连文档模型一起去掉、重设计纯任务表）被否决：查询层与 service 全量重写，无用户可见收益。

## 3. 删除 / 保留清单

### 删除

| 项 | 位置 |
|---|---|
| markdown 编解码器（YAML frontmatter 解析/渲染，419 行） | `backend/src/core/tasks/markdown_codec.rs` |
| 任务文档存储适配器（Obsidian REST，66 行） | `backend/src/infra/task_document_store.rs` |
| `sync_tasks` 工具：schema、handler、注册（11 工具 → 10） | `backend/src/tools/definitions.rs`、`tools/handlers/task_handlers.rs`、`tools/handlers/mod.rs` |
| `sync_tasks` 服务实现（~61 行）与 `sync_error` 辅助 | `backend/src/core/tasks/service.rs` |
| `TaskSyncResult` / `TaskSyncError` 模型 | `backend/src/models/task.rs` |
| 同步队列：表、`enqueue_sync`、成功写入时的清队列语句 | `backend/migrations/009_tasks.sql`（历史保留）、`backend/src/infra/task_index_store.rs` |
| 内存脏路径集合 `dirty_paths`、`mark_dirty` / `clear_dirty` | `backend/src/core/tasks/service.rs` |
| `index_out_of_sync` 警告路径（md 写成功但索引失败的双写分裂兜底） | `service.rs` `persist_document`、模型 `TaskWriteResponse.warnings` 相关 |
| `task_documents.sync_error` 列 | 迁移中 DROP |
| 前端"同步 Obsidian"按钮、空列表自动同步（`onMounted`）、`syncNow` | `frontend/src/views/Tasks.vue` |
| 前端 store 的 `sync` / `syncing` / `lastSync`（`lastSync` 本就是死状态） | `frontend/src/stores/tasks.ts` |
| 前端 API client `syncTasks` 与 `TaskSyncResult` 类型 | `frontend/src/api/tasks.ts` |
| 详情面板"位置"字段（显示 md 路径，已无意义） | `frontend/src/views/Tasks.vue` detail-facts |

### 保留

- 四张任务表结构与全部业务行为：状态机与级联、子任务树、进度计算、归档、审计事件。
- `DocumentVersion` OCC 机制与 `TASK_VERSION_CONFLICT` 语义、前端自动重载。
- `ObsidianClient` 及其配置（记忆引擎/笔记搜索仍在使用）。
- 工具协议其余 10 个任务工具的对外行为。

## 4. 数据与迁移

- **现有数据直接沿用**：SQLite 投影即最新状态（切换前会再执行一次最终同步兜底，见 §8）。
- **新迁移（仅做减法）**：`DROP TABLE task_sync_queue`；`ALTER TABLE task_documents DROP COLUMN sync_error`。`revision` 与 `content_hash` 列保留用于 OCC。
- `task_documents` 行退化为纯粹的分组与版本载体，不再对应物理文件；存量文档行原样保留。**新短期任务每个任务一行文档**（`storage_month` 仍记录所属月份），不再做按月归档合并；`path` 列保留为文档主键，新文档使用合成键（如 `db:short:{uuid}`、`db:long:{uuid}`），无文件系统语义。
- 注意：脱离后 SQLite 不再可由 vault 重建——这是本设计的固有取舍，需在模块文档中明示。

## 5. 后端设计

- `mutate_document` 流程改为：从 SQLite 读行 → 组装内存文档模型 → 应用原有变更闭包 → `revision += 1`、重新生成 `content_hash` → 复用 `replace_document`（单事务 delete+insert）写回。`replace_document` 中清除同步队列的语句随队列一并删除。
- `DocumentVersion` 结构不变：`revision` 为 DB 计数器；`content_hash` 为每次写入重新生成的不透明令牌（不再依赖 md 内容）。前端只透传，无感知。
- 写入原子性：md/索引双写分裂不再存在，事务内全成功或全回滚。
- `TaskService::new` 构造签名相应简化（去掉文档存储参数）；service 测试改用内存 SQLite（rusqlite in-memory），不再需要 fake 文档存储。

## 6. 前端设计

- Tasks 视图：删除同步按钮与相关状态样式；`onMounted` 不再自动 sync，直接 `loadList`。
- Store/API：删除 `sync` action 与 `syncTasks` client；`TaskWriteResponse.warnings` 类型随警告路径一并清理。
- detail-facts 由四列变三列（开始/结束/优先级）。

## 7. 文档与测试

- `docs/requirement/09-task-management.md`：存储原则改为"SQLite 唯一权威"；删除 §4.7 Obsidian 同步节及 sync 相关验收标准；工具表移除 `sync_tasks`。
- `docs/development/09-task-management.md`：删除同步队列、codec、增量扫描相关章节；存储层描述改为 DB 直写。
- `docs/development/02-tool-protocol.md`：工具清单更新（10 个任务工具）。
- 测试：service 测试改写为 DB 直连（现有用例语义保留：OCC 拒绝过期版本、级联、进度、归档审计）；删除 codec 测试；新增迁移测试；前端 store 删除 sync 相关内容。

## 8. 上线顺序

1. 旧二进制下最后执行一次 `sync_tasks`，兜住 Obsidian 侧可能的手工改动。
2. 部署新二进制：启动时自动执行新迁移。
3. 前端随 release 二进制内嵌（`vite build --outDir dist_new` → `cargo build --release` → sudo 安装重启）。

## 9. 明确不做（YAGNI）

- 不做 schema 重设计（路线 B）。
- 不保留任何形式的 md 导出/备份路径。
- 不做 SQLite 定期备份机制（属基础设施话题，另行立项）。
- 不清理 vault 中现存 Tasks/ 文件（用户自行处置）。
