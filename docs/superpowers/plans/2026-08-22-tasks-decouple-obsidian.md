# 任务中枢脱离 Obsidian 同步 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all Obsidian markdown storage and sync machinery from the Tasks module, making SQLite the sole authoritative store (per approved spec `docs/superpowers/specs/2026-08-22-tasks-decouple-obsidian-design.md`).

**Architecture:** Keep the 4-table document model (`task_documents` / `task_nodes` / `task_progress` / `task_audit_events`) and all business logic (state machine, cascades, progress tree, OCC versioning). Replace "render md → write via Obsidian REST → replay projection" with a single-transaction SQLite write. Tasks are ordered compilation-preserving: first add a DB read path (`load_document`), then delete the manual sync tool end-to-end, then cut the write path over to DB-direct, then drop the sync DB objects via migration, then update docs, then rebuild and roll out.

**Tech Stack:** Rust 2021 / Axum / rusqlite (bundled SQLite) / serde; Vue 3 + TypeScript + Pinia; rust-embed (release binary embeds `frontend/dist_new`).

## Global Constraints

- Backend gates per commit, run from `backend/`: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`（bin-only crate — use `cargo test`, never `cargo test --lib`）.
- Frontend gates per commit, run from `frontend/`: `npx vue-tsc -b`（typecheck）and `npm test`（node --test，24 个策略/单元测试）.
- Commit format: Conventional Commits（e.g. `refactor(tasks): ...`）. No commit touches unrelated files.
- 禁止生产代码 `.unwrap()` / `.expect()`（测试除外）.
- The user's live backend is the **installed release binary** `/usr/local/bin/obsidian-brain`; nothing in Tasks 1–6 changes it. Rollout happens only in Task 7.
- Never read or write anything under `/Users/tiercelchow/Documents/Obsidian/TiercelChow's Blog/Tasks/` during implementation (the running old binary still owns those files).
- New short tasks get one dedicated document per task with path key `db:short:{uuid}`; new long tasks use `db:long:{uuid}`. Existing document rows keep their `Tasks/...md` paths unchanged.
- API surface after this plan: exactly 10 task tools (the `sync_tasks` tool is removed).

---

### Task 1: `load_document` — read a full TaskDocument back from SQLite

Additive only: the index store gains the ability to reconstruct a `TaskDocument` (nodes + progress + audit) from its rows. Nothing existing changes behavior. This is the read half of the DB-direct write path used in Task 3.

**Files:**
- Modify: `backend/src/models/task.rs`（add `FromStr` for `TaskEventType`）
- Modify: `backend/src/infra/task_index_store.rs`（trait method + SQLite impl + row-mapping helpers + test）
- Test: `backend/src/infra/task_index_store.rs`（tests module）

**Interfaces:**
- Consumes: existing `TaskIndexStore` trait, existing parse helpers (`parse_uuid`, `parse_enum`, `parse_datetime`, `optional_row`, `conversion_error`), existing tables.
- Produces: `async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError>` on `TaskIndexStore`; `impl FromStr for TaskEventType`（variants map from their `as_str` strings）. Task 3's service rewrite calls `self.index.load_document(&meta.path)`.

- [ ] **Step 1: Write the failing test**

In `backend/src/infra/task_index_store.rs`, tests module（after `test_replace_and_query_document_projection`）add a richer fixture and a roundtrip test:

```rust
    fn roundtrip_document() -> TaskDocument {
        let root_id = Uuid::new_v4();
        let subtask_id = Uuid::new_v4();
        let now = Utc::now();
        let date = NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date");
        TaskDocument {
            schema: "tasks-long/v1".to_string(),
            document_kind: TaskDocumentKind::LongTask,
            storage_month: None,
            revision: 7,
            tasks: vec![
                TaskNode {
                    id: root_id,
                    root_id,
                    parent_id: None,
                    kind: TaskKind::Long,
                    role: TaskRole::Root,
                    title: "长期任务".to_string(),
                    description: "描述".to_string(),
                    start_date: date,
                    end_date: date,
                    importance: TaskImportance::High,
                    status: TaskStatus::InProgress,
                    position: 0,
                    closure_note: None,
                    closed_at: None,
                    created_at: now,
                    updated_at: now,
                    revision: 7,
                    archived_at: None,
                },
                TaskNode {
                    id: subtask_id,
                    root_id,
                    parent_id: Some(root_id),
                    kind: TaskKind::Long,
                    role: TaskRole::Subtask,
                    title: "子任务".to_string(),
                    description: String::new(),
                    start_date: date,
                    end_date: date,
                    importance: TaskImportance::Medium,
                    status: TaskStatus::Planned,
                    position: 0,
                    closure_note: None,
                    closed_at: None,
                    created_at: now,
                    updated_at: now,
                    revision: 7,
                    archived_at: None,
                },
            ],
            progress: vec![ProgressEntry {
                id: Uuid::new_v4(),
                root_id,
                task_id: subtask_id,
                recorded_at: now,
                note: "进展".to_string(),
                percent_after: Some(40),
                created_at: now,
            }],
            audit: vec![AuditEvent {
                id: Uuid::new_v4(),
                root_id,
                task_id: root_id,
                event_type: TaskEventType::StatusChanged,
                from_status: Some(TaskStatus::Planned),
                to_status: Some(TaskStatus::InProgress),
                note: None,
                occurred_at: now,
            }],
            extra: BTreeMap::new(),
            freeform_notes: String::new(),
        }
    }

    #[tokio::test]
    async fn test_load_document_reconstructs_full_document() {
        let (_dir, store) = store();
        let document = roundtrip_document();
        store
            .replace_document("Tasks/Long/roundtrip.md", "sha256:test", &document)
            .await
            .expect("replace");

        let loaded = store
            .load_document("Tasks/Long/roundtrip.md")
            .await
            .expect("load")
            .expect("document exists");

        assert_eq!(loaded.document_kind, TaskDocumentKind::LongTask);
        assert_eq!(loaded.storage_month, None);
        assert_eq!(loaded.revision, 7);
        assert_eq!(loaded.tasks.len(), 2);
        assert_eq!(loaded.tasks[0].title, "长期任务");
        assert_eq!(loaded.tasks[0].status, TaskStatus::InProgress);
        assert_eq!(loaded.tasks[1].id, document.tasks[1].id);
        assert_eq!(loaded.tasks[1].parent_id, Some(document.tasks[0].id));
        assert_eq!(loaded.progress.len(), 1);
        assert_eq!(loaded.progress[0].note, "进展");
        assert_eq!(loaded.progress[0].percent_after, Some(40));
        assert_eq!(loaded.audit.len(), 1);
        assert_eq!(loaded.audit[0].event_type, TaskEventType::StatusChanged);
        assert_eq!(loaded.audit[0].from_status, Some(TaskStatus::Planned));
        assert_eq!(loaded.audit[0].to_status, Some(TaskStatus::InProgress));
    }

    #[tokio::test]
    async fn test_load_document_missing_path_returns_none() {
        let (_dir, store) = store();
        let loaded = store.load_document("Tasks/Long/missing.md").await.expect("load");
        assert!(loaded.is_none());
    }
```

Also add imports the tests need — in the tests module `use super::*;` already covers the store types; add to the file's top-level `use crate::models::task::{...}` list: `ProgressEntry`, `AuditEvent`, `TaskEventType` (keep alphabetical-ish order of the existing list).

- [ ] **Step 2: Run test to verify it fails**

Run (from `backend/`): `cargo test load_document`
Expected: FAIL to compile — `load_document` is not a member of trait `TaskIndexStore`, and `TaskEventType: FromStr` is not satisfied.

- [ ] **Step 3: Add `FromStr` for `TaskEventType`**

In `backend/src/models/task.rs`, next to the `TaskEventType` `as_str` impl, add:

```rust
impl FromStr for TaskEventType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "status_changed" => Ok(Self::StatusChanged),
            "reopened" => Ok(Self::Reopened),
            "archived" => Ok(Self::Archived),
            "unarchived" => Ok(Self::Unarchived),
            "cascade_completed" => Ok(Self::CascadeCompleted),
            "moved" => Ok(Self::Moved),
            other => Err(format!("未知事件类型: {other}")),
        }
    }
}
```

(`FromStr` is already imported in this file for the other enums.)

- [ ] **Step 4: Implement `load_document`**

In `backend/src/infra/task_index_store.rs`:

(a) Add to the `TaskIndexStore` trait (after `find_document_by_task`):

```rust
    async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError>;
```

(b) Add the impl in `impl TaskIndexStore for SqliteTaskIndexStore` (after `find_document_by_task`). Note `schema` / `extra` / `freeform_notes` are transitional fills — Task 3 removes those three fields:

```rust
    async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError> {
        self.db.with_connection(|conn| {
            let header = conn.query_row(
                "SELECT document_kind, storage_month, revision
                 FROM task_documents WHERE path = ?1",
                params![path],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            );
            let (kind, storage_month, revision) = match optional_row(header)? {
                Some(header) => header,
                None => return Ok(None),
            };
            let document_kind = match kind.as_str() {
                "short_month" => TaskDocumentKind::ShortMonth,
                "long_task" => TaskDocumentKind::LongTask,
                other => {
                    return Err(BrainError::TaskDocumentCorrupt {
                        path: path.to_string(),
                        detail: format!("未知文档类型: {other}"),
                    })
                }
            };

            let tasks = {
                let mut statement = conn.prepare(
                    "SELECT id, root_id, parent_id, kind, role, title, description, status,
                            importance, start_date, end_date, position, closure_note, closed_at,
                            created_at, updated_at, revision, archived_at
                     FROM task_nodes WHERE storage_path = ?1 ORDER BY position, id",
                )?;
                let rows = statement.query_map(params![path], node_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            let progress = {
                let mut statement = conn.prepare(
                    "SELECT id, root_id, task_id, recorded_at, note, percent_after, created_at
                     FROM task_progress WHERE storage_path = ?1 ORDER BY rowid",
                )?;
                let rows = statement.query_map(params![path], progress_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            let audit = {
                let mut statement = conn.prepare(
                    "SELECT id, root_id, task_id, event_type, from_status, to_status, note,
                            occurred_at
                     FROM task_audit_events WHERE storage_path = ?1 ORDER BY rowid",
                )?;
                let rows = statement.query_map(params![path], audit_from_row)?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            Ok(Some(TaskDocument {
                schema: match document_kind {
                    TaskDocumentKind::ShortMonth => "tasks-short/v1",
                    TaskDocumentKind::LongTask => "tasks-long/v1",
                }
                .to_string(),
                document_kind,
                storage_month,
                revision,
                tasks,
                progress,
                audit,
                extra: BTreeMap::new(),
                freeform_notes: String::new(),
            }))
        })
    }
```

(c) Add row-mapping helpers next to `summary_from_row`:

```rust
fn node_from_row(row: &Row<'_>) -> rusqlite::Result<TaskNode> {
    Ok(TaskNode {
        id: parse_uuid(row, 0)?,
        root_id: parse_uuid(row, 1)?,
        parent_id: parse_optional_uuid(row, 2)?,
        kind: parse_enum::<TaskKind>(row, 3)?,
        role: parse_enum::<TaskRole>(row, 4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        status: parse_enum::<TaskStatus>(row, 7)?,
        importance: parse_enum::<TaskImportance>(row, 8)?,
        start_date: parse_date(row, 9)?,
        end_date: parse_date(row, 10)?,
        position: row.get(11)?,
        closure_note: row.get(12)?,
        closed_at: parse_optional_datetime(row, 13)?,
        created_at: parse_datetime(row, 14)?,
        updated_at: parse_datetime(row, 15)?,
        revision: row.get(16)?,
        archived_at: parse_optional_datetime(row, 17)?,
    })
}

fn progress_from_row(row: &Row<'_>) -> rusqlite::Result<ProgressEntry> {
    Ok(ProgressEntry {
        id: parse_uuid(row, 0)?,
        root_id: parse_uuid(row, 1)?,
        task_id: parse_uuid(row, 2)?,
        recorded_at: parse_datetime(row, 3)?,
        note: row.get(4)?,
        percent_after: row.get(5)?,
        created_at: parse_datetime(row, 6)?,
    })
}

fn audit_from_row(row: &Row<'_>) -> rusqlite::Result<AuditEvent> {
    Ok(AuditEvent {
        id: parse_uuid(row, 0)?,
        root_id: parse_uuid(row, 1)?,
        task_id: parse_uuid(row, 2)?,
        event_type: parse_enum::<TaskEventType>(row, 3)?,
        from_status: parse_optional_status(row, 4)?,
        to_status: parse_optional_status(row, 5)?,
        note: row.get(6)?,
        occurred_at: parse_datetime(row, 7)?,
    })
}

fn parse_optional_status(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<TaskStatus>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| TaskStatus::from_str(&value).map_err(|error| conversion_error(index, error)))
        .transpose()
}
```

(d) Add `use std::collections::BTreeMap;` at the top of the file (needed for the transitional `extra` fill). Add `ProgressEntry`, `AuditEvent`, and `TaskEventType` to the existing `use crate::models::task::{...}` import list.

- [ ] **Step 5: Run tests to verify they pass**

Run (from `backend/`): `cargo test load_document && cargo test task_index`
Expected: PASS (all index-store tests, including the pre-existing two).

- [ ] **Step 6: Gates + commit**

Run (from `backend/`): `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`

```bash
git add backend/src/models/task.rs backend/src/infra/task_index_store.rs
git commit -m "feat(tasks): reconstruct TaskDocument from SQLite projection rows"
```

---

### Task 2: Remove the manual `sync_tasks` surface end-to-end

Delete the user-visible sync feature: the tool API (11 tools → 10), the service method, the sync result models, and the frontend sync button / auto-sync / store action. Markdown storage itself still works in this commit (write path untouched) — the running binary is unaffected.

**Files:**
- Modify: `backend/src/core/tasks/service.rs`（delete `sync_tasks` + `sync_error` helper）
- Modify: `backend/src/tools/handlers/task_handlers.rs`（delete `SyncTasksArgs` + `SyncTasksHandler`, update test 11 → 10）
- Modify: `backend/src/tools/handlers/mod.rs`（remove registration）
- Modify: `backend/src/tools/definitions.rs`（delete `sync_tasks_schema`）
- Modify: `backend/src/models/task.rs`（delete `TaskSyncResult` + `TaskSyncError`）
- Modify: `frontend/src/api/tasks.ts`（delete `TaskSyncResult` + `syncTasks`）
- Modify: `frontend/src/stores/tasks.ts`（delete `syncing` / `lastSync` / `sync`）
- Modify: `frontend/src/views/Tasks.vue`（delete sync button, `syncNow`, auto-sync in `onMounted`）

**Interfaces:**
- Consumes: nothing new.
- Produces: task tool registry with exactly 10 handlers; `TaskService` without `sync_tasks`. Task 3 builds on the slimmed service.

- [ ] **Step 1: Backend — delete the service method**

In `backend/src/core/tasks/service.rs`, delete the whole `pub async fn sync_tasks(&self, dry_run: bool) -> Result<TaskSyncResult, BrainError> { ... }` block (starts `let mut result = TaskSyncResult::default();`, ends with the `for stored_path in self.index.list_document_paths()...` deletion pass) and the helper:

```rust
fn sync_error(path: &str, error: &BrainError) -> TaskSyncError {
    TaskSyncError {
        path: path.to_string(),
        code: error.error_code().to_string(),
        message: error.to_string(),
    }
}
```

Leave `documents`, `enqueue_sync` (still used by `persist_document`), `document_meta` / `list_document_paths` / `remove_document` (trait methods remain; no dead-code warnings for trait impls) untouched.

- [ ] **Step 2: Backend — delete the tool**

In `backend/src/tools/handlers/task_handlers.rs`, delete:

```rust
#[derive(Debug, Default, Deserialize)]
struct SyncTasksArgs {
    #[serde(default)]
    dry_run: bool,
}

task_handler!(
    SyncTasksHandler,
    "sync_tasks",
    "从 Obsidian Tasks 文件夹增量刷新或预检任务索引",
    sync_tasks_schema,
    |args, ctx| {
        let request: SyncTasksArgs = parse(args)?;
        json(ctx.task_service.sync_tasks(request.dry_run).await?)
    }
);
```

In the test `test_task_handlers_expose_task_module_and_schemas`: remove `Box::new(SyncTasksHandler),` from the vec and change `assert_eq!(handlers.len(), 11);` → `assert_eq!(handlers.len(), 10);`.

In `backend/src/tools/handlers/mod.rs`, remove the line:

```rust
    registry.register(Arc::new(SyncTasksHandler)).await;
```

In `backend/src/tools/definitions.rs`, delete:

```rust
pub fn sync_tasks_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "dry_run": { "type": "boolean", "default": false } },
        "additionalProperties": false
    })
}
```

In `backend/src/models/task.rs`, delete the `TaskSyncResult` and `TaskSyncError` structs.

- [ ] **Step 3: Backend gates**

Run (from `backend/`): `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Expected: PASS. If clippy flags now-unused imports in `service.rs` (e.g. `HashSet` is still used by `dirty_paths` — should NOT be flagged), remove exactly what the compiler names.

- [ ] **Step 4: Frontend — delete API client and store sync**

In `frontend/src/api/tasks.ts`, delete:

```ts
export interface TaskSyncResult {
  created: number
  updated: number
  unchanged: number
  removed: number
  errors: Array<{ path: string; code: string; message: string }>
}
```

and:

```ts
export function syncTasks(dryRun = false) {
  return taskCall<TaskSyncResult>('sync_tasks', { dry_run: dryRun })
}
```

In `frontend/src/stores/tasks.ts`:
- Remove `syncTasks as syncTasksApi,` and `type TaskSyncResult,` from the import list.
- Delete `const syncing = ref(false)` and `const lastSync = ref<TaskSyncResult | null>(null)`.
- Delete the whole `async function sync(dryRun = false) { ... }` action.
- In the `return { ... }` block, remove the `syncing,`, `lastSync,`, and `sync,` entries.

- [ ] **Step 5: Frontend — delete Tasks.vue sync UI**

In `frontend/src/views/Tasks.vue`:

(a) Delete the header button (lines 9–12):

```html
        <button class="glass-button secondary" type="button" :disabled="store.syncing" @click="syncNow">
          <el-icon :class="{ spinning: store.syncing }"><Refresh /></el-icon>
          {{ store.syncing ? '同步中' : '同步 Obsidian' }}
        </button>
```

(b) In the icon import, remove `Refresh` (keep `FolderChecked, Plus, Search`):

```ts
import { FolderChecked, Plus, Refresh, Search } from '@element-plus/icons-vue'
```

→

```ts
import { FolderChecked, Plus, Search } from '@element-plus/icons-vue'
```

(c) Delete the `syncNow` function:

```ts
async function syncNow() {
  try {
    const result = await store.sync()
    await refreshCurrent()
    ElMessage.success(`同步完成：${result.created} 个新增，${result.updated} 个更新`)
  } catch { /* shown by store */ }
}
```

(d) Replace `onMounted` — drop the empty-list auto-sync branch:

```ts
onMounted(async () => {
  const tasks = await store.loadTasks(taskFilters()).catch(() => [])
  if (tasks.length === 0) {
    await store.sync().catch(() => undefined)
    await loadList()
  }
  if (viewMode.value === 'calendar') await loadCalendar()
  if (typeof route.query.task === 'string') await openTask(route.query.task)
})
```

→

```ts
onMounted(async () => {
  await store.loadTasks(taskFilters()).catch(() => [])
  if (viewMode.value === 'calendar') await loadCalendar()
  if (typeof route.query.task === 'string') await openTask(route.query.task)
})
```

(e) Delete the now-unreferenced `.spinning` CSS rule (line ~744):

```css
.spinning { display: inline-block; animation: spin .8s linear infinite; }
```

Keep the `.spinning` mention inside the reduced-motion rule at line ~941 (that selector list also targets other animations; removing just `.spinning` from it is optional — simplest is to leave that line as-is since the class simply no longer occurs in the DOM).

- [ ] **Step 6: Frontend gates**

Run (from `frontend/`): `npx vue-tsc -b && npm test`
Expected: typecheck clean, 24 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add backend/src frontend/src
git commit -m "refactor(tasks): remove manual Obsidian sync tool and UI"
```

---

### Task 3: Storage cutover — SQLite-direct write path, delete markdown layer

The big one. Slim the document model (drop `schema` / `extra` / `freeform_notes`), rewrite `TaskService` to load-and-persist via `TaskIndexStore` only, delete `markdown_codec.rs` and `task_document_store.rs`, slim the index-store trait, rewire `main.rs`, and drop the `serde_yaml` dependency. TDD order: rewrite the service tests first (they fail to compile against the new constructor — that's the red), then implement.

**Files:**
- Modify: `backend/src/models/task.rs`（slim `TaskDocument`, `TaskDetail`, `TaskWriteResponse`, `validate`）
- Modify: `backend/src/core/tasks/service.rs`（DB-direct rewrite; delete `load_path`, `slugify`, `version_for`, `content_hash`, dirty-set; rewrite tests）
- Modify: `backend/src/core/tasks/mod.rs`（remove `pub mod markdown_codec;`）
- Delete: `backend/src/core/tasks/markdown_codec.rs`
- Delete: `backend/src/infra/task_document_store.rs`
- Modify: `backend/src/infra/mod.rs`（remove `pub mod task_document_store;`）
- Modify: `backend/src/infra/task_index_store.rs`（remove `document_meta` / `list_document_paths` / `remove_document` / `enqueue_sync`; slim `TaskDocumentMeta`; remove queue statement from `replace_document`; update tests）
- Modify: `backend/src/main.rs`（2 wiring sites + import）
- Modify: `backend/Cargo.toml`（remove `serde_yaml`）
- Test: `backend/src/core/tasks/service.rs`, `backend/src/infra/task_index_store.rs`

**Interfaces:**
- Consumes: `load_document` from Task 1.
- Produces: `TaskService::new(index: Arc<dyn TaskIndexStore>)` and `TaskService::with_clock(index: Arc<dyn TaskIndexStore>, clock: Arc<dyn TaskClock>)`; `TaskDocument { document_kind, storage_month, revision, tasks, progress, audit }`; `TaskWriteResponse { task }`; `TaskIndexStore` with methods `replace_document` / `find_document_by_task` / `load_document` / `list_tasks` / `calendar_tasks`; `TaskDocumentMeta { path, revision, content_hash }`. Task 4 (frontend) relies on responses no longer carrying `warnings` / `freeform_notes`; Task 5 relies on nothing writing `task_sync_queue` / `sync_error`.

- [ ] **Step 1: Rewrite the service tests (red — compile failure)**

In `backend/src/core/tasks/service.rs` tests module:

(a) Delete `MemoryDocuments` and its `TaskDocumentStore` impl entirely. Delete the `use async_trait::async_trait;`, `use std::collections::HashMap;`, `use std::sync::RwLock;` imports if now unused (RwLock/HashMap/async_trait were only for the fake).

(b) Replace the `service()` helper:

```rust
    fn service() -> (TempDir, TaskService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("tasks.db")).expect("sqlite"));
        let index = Arc::new(SqliteTaskIndexStore::new(db));
        let service = TaskService::with_clock(index, Arc::new(FixedClock));
        (dir, service)
    }
```

(c) Replace the first test (short-task creation) — short tasks now get a dedicated document:

```rust
    #[tokio::test]
    async fn test_create_short_task_persists_dedicated_document() {
        let (_dir, service) = service();
        let response = service
            .create_task(create_request(TaskKind::Short))
            .await
            .expect("create");
        assert!(response.task.storage_path.starts_with("db:short:"));
        assert_eq!(response.task.document_version.revision, 1);
        let listed = service
            .list_tasks(TaskQuery::default())
            .await
            .expect("list");
        assert_eq!(listed.tasks.len(), 1);
    }
```

(d) In every other test, update the `service()` destructure from `let (_dir, _documents, service) = service();` / `let (_dir, documents, service) = service();` to `let (_dir, service) = service();` and delete any `documents.read(...)` assertions (only the first test had one).

(e) Add a new test for the regenerated version token:

```rust
    #[tokio::test]
    async fn test_each_write_rotates_version_token() {
        let (_dir, service) = service();
        let created = service
            .create_task(create_request(TaskKind::Long))
            .await
            .expect("create");
        let task_id = created.task.root.id;
        let updated = service
            .update_task(TaskUpdateRequest {
                task_id,
                patch: TaskPatch {
                    title: Some("新标题".to_string()),
                    ..Default::default()
                },
                expected_version: created.task.document_version.clone(),
            })
            .await
            .expect("update");
        assert_eq!(updated.task.document_version.revision, 2);
        assert_ne!(
            updated.task.document_version.content_hash,
            created.task.document_version.content_hash
        );
        // 旧版本再次提交必须冲突
        let stale = service
            .update_task(TaskUpdateRequest {
                task_id,
                patch: TaskPatch {
                    title: Some("旧标题".to_string()),
                    ..Default::default()
                },
                expected_version: created.task.document_version.clone(),
            })
            .await;
        assert!(matches!(stale, Err(BrainError::TaskVersionConflict(_))));
    }
```

(`TaskPatch` is `#[derive(Default)]` with `title / description / start_date / end_date / importance` — all optional; `TaskUpdateRequest` is `{ task_id, patch, expected_version }`. Verified against `backend/src/models/task.rs:471-486`.)

- [ ] **Step 2: Run tests to verify they fail**

Run (from `backend/`): `cargo test --test '' 2>&1 | head -30` — or simply `cargo build`
Expected: FAIL to compile — `TaskService::with_clock` takes 2 args not 3, `MemoryDocuments` references a deleted trait, etc. This is the red state.

- [ ] **Step 3: Slim the models**

In `backend/src/models/task.rs`:

(a) `TaskDocument` becomes:

```rust
pub struct TaskDocument {
    pub document_kind: TaskDocumentKind,
    pub storage_month: Option<String>,
    pub revision: i64,
    pub tasks: Vec<TaskNode>,
    #[serde(default)]
    pub progress: Vec<ProgressEntry>,
    #[serde(default)]
    pub audit: Vec<AuditEvent>,
}
```

(delete `schema`, `extra`, `freeform_notes` fields)

(b) In `validate()`, delete the two schema checks:

```rust
                if self.schema != "tasks-short/v1" {
                    return Err(format!("短期待办 schema 不受支持: {}", self.schema));
                }
```

```rust
                if self.schema != "tasks-long/v1" {
                    return Err(format!("长期任务 schema 不受支持: {}", self.schema));
                }
```

(keep every other branch — `storage_month` format check, progress-empty check, root-shape checks, long-task tree checks)

(c) `TaskDetail`: delete `pub freeform_notes: String,`.

(d) `TaskWriteResponse` becomes:

```rust
pub struct TaskWriteResponse {
    pub task: TaskDetail,
}
```

(e) Delete any `use serde_yaml`-related path usages — after (a) there are none (the `extra` field was the only one).

- [ ] **Step 4: Delete the markdown layer**

```bash
git rm backend/src/core/tasks/markdown_codec.rs backend/src/infra/task_document_store.rs
```

In `backend/src/core/tasks/mod.rs` remove `pub mod markdown_codec;`.
In `backend/src/infra/mod.rs` remove `pub mod task_document_store;`.
In `backend/Cargo.toml` remove `serde_yaml = "0.9"`.

- [ ] **Step 5: Slim the index store trait**

In `backend/src/infra/task_index_store.rs`:

(a) `TaskDocumentMeta` becomes:

```rust
#[derive(Debug, Clone)]
pub struct TaskDocumentMeta {
    pub path: String,
    pub revision: i64,
    pub content_hash: String,
}
```

(b) Trait `TaskIndexStore` keeps exactly: `replace_document`, `find_document_by_task`, `load_document`, `list_tasks`, `calendar_tasks`. Delete `document_meta`, `list_document_paths`, `remove_document`, `enqueue_sync` from the trait AND their `SqliteTaskIndexStore` impls (the whole `enqueue_sync` impl block including its `INSERT INTO task_sync_queue ...` SQL).

(c) `meta_from_row` becomes:

```rust
fn meta_from_row(row: &Row<'_>) -> rusqlite::Result<TaskDocumentMeta> {
    Ok(TaskDocumentMeta {
        path: row.get(0)?,
        revision: row.get(1)?,
        content_hash: row.get(2)?,
    })
}
```

and `find_document_by_task`'s SQL becomes:

```sql
                "SELECT d.path, d.revision, d.content_hash
                 FROM task_nodes n JOIN task_documents d ON d.path = n.storage_path
                 WHERE n.id = ?1"
```

(d) In `replace_document`: remove `sync_error` from the INSERT column list and value list:

```sql
                "INSERT INTO task_documents
                 (path, document_kind, root_id, storage_month, revision, content_hash, indexed_at, sync_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
```

→

```sql
                "INSERT INTO task_documents
                 (path, document_kind, root_id, storage_month, revision, content_hash, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
```

and delete the trailing queue statement:

```rust
            conn.execute("DELETE FROM task_sync_queue WHERE path = ?1", params![path])?;
```

(The `sync_error` column still exists until Task 5's migration; it is nullable so omitting it inserts NULL.)

(e) In `load_document` (from Task 1), remove the three transitional fills — the constructor tail becomes:

```rust
            Ok(Some(TaskDocument {
                document_kind,
                storage_month,
                revision,
                tasks,
                progress,
                audit,
            }))
```

Delete the now-unused `use std::collections::BTreeMap;` added in Task 1.

(f) Tests: in `sample_document()` remove the `schema:` / `extra:` / `freeform_notes:` fields; same for `roundtrip_document()` from Task 1. Delete `test_remove_document_cascades_projection` entirely (its subject method is gone; cascade is still exercised by `replace_document`'s DELETE+INSERT path).

- [ ] **Step 6: Rewrite the service**

In `backend/src/core/tasks/service.rs`:

(a) Imports: delete `use crate::core::tasks::markdown_codec::TaskMarkdownCodec;`, `use crate::infra::task_document_store::TaskDocumentStore;`, and `use std::collections::{BTreeMap, HashMap, HashSet};` → keep only what remains used (`HashMap` for path_locks; `BTreeMap`/`HashSet` go — `dirty_paths` is deleted).

(b) Struct + constructors:

```rust
pub struct TaskService {
    index: Arc<dyn TaskIndexStore>,
    path_locks: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
    clock: Arc<dyn TaskClock>,
}

impl TaskService {
    pub fn new(index: Arc<dyn TaskIndexStore>) -> Self {
        Self::with_clock(index, Arc::new(SystemTaskClock))
    }

    pub fn with_clock(index: Arc<dyn TaskIndexStore>, clock: Arc<dyn TaskClock>) -> Self {
        Self {
            index,
            path_locks: Mutex::new(HashMap::new()),
            clock,
        }
    }
```

(c) `get_task` (no markdown, no self-heal — the DB is authoritative):

```rust
    pub async fn get_task(&self, task_id: Uuid) -> Result<TaskDetail, BrainError> {
        let meta = self.meta_for_task(task_id).await?;
        let document = self
            .index
            .load_document(&meta.path)
            .await?
            .ok_or_else(|| BrainError::TaskNotFound(task_id.to_string()))?;
        let version = DocumentVersion {
            revision: meta.revision,
            content_hash: meta.content_hash,
        };
        detail_from_document(&document, &meta.path, version, task_id)
    }
```

(d) `create_short_task` — one document per task, no file read, no path lock (UUID key is unique by construction):

```rust
    async fn create_short_task(
        &self,
        request: TaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        let storage_month = self.clock.storage_month();
        let id = Uuid::new_v4();
        let path = format!("db:short:{id}");
        let now = self.clock.now_utc();
        let document = TaskDocument {
            document_kind: TaskDocumentKind::ShortMonth,
            storage_month: Some(storage_month),
            revision: 1,
            tasks: vec![TaskNode {
                id,
                root_id: id,
                parent_id: None,
                kind: TaskKind::Short,
                role: TaskRole::Root,
                title: request.title.trim().to_string(),
                description: request.description,
                start_date: request.start_date,
                end_date: request.end_date,
                importance: request.importance,
                status: TaskStatus::Open,
                position: 0,
                closure_note: None,
                closed_at: None,
                created_at: now,
                updated_at: now,
                revision: 1,
                archived_at: None,
            }],
            progress: Vec::new(),
            audit: Vec::new(),
        };
        self.persist_document(&path, document, id).await
    }
```

(e) `create_long_task` — same shape with `db:long:{id}`, `TaskDocumentKind::LongTask`, `storage_month: None`, `TaskStatus::Planned`; delete the `slugify` path construction, the duplicate-check `documents.read` call, and the path lock:

```rust
    async fn create_long_task(
        &self,
        request: TaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        let id = Uuid::new_v4();
        let path = format!("db:long:{id}");
        let now = self.clock.now_utc();
        let document = TaskDocument {
            document_kind: TaskDocumentKind::LongTask,
            storage_month: None,
            revision: 1,
            tasks: vec![TaskNode {
                id,
                root_id: id,
                parent_id: None,
                kind: TaskKind::Long,
                role: TaskRole::Root,
                title: request.title.trim().to_string(),
                description: request.description,
                start_date: request.start_date,
                end_date: request.end_date,
                importance: request.importance,
                status: TaskStatus::Planned,
                position: 0,
                closure_note: None,
                closed_at: None,
                created_at: now,
                updated_at: now,
                revision: 1,
                archived_at: None,
            }],
            progress: Vec::new(),
            audit: Vec::new(),
        };
        self.persist_document(&path, document, id).await
    }
```

(f) `mutate_document` — load from DB instead of markdown:

```rust
    async fn mutate_document<F>(
        &self,
        task_id: Uuid,
        expected_version: DocumentVersion,
        mutate: F,
    ) -> Result<TaskWriteResponse, BrainError>
    where
        F: FnOnce(&mut TaskDocument, DateTime<Utc>, i64) -> Result<(), BrainError>,
    {
        let meta = self.meta_for_task(task_id).await?;
        let path_lock = self.path_lock(&meta.path)?;
        let _guard = path_lock.lock().await;
        let mut document = self
            .index
            .load_document(&meta.path)
            .await?
            .ok_or_else(|| BrainError::TaskNotFound(task_id.to_string()))?;
        let actual_version = DocumentVersion {
            revision: meta.revision,
            content_hash: meta.content_hash.clone(),
        };
        verify_version(&expected_version, &actual_version)?;
        let next_revision = document.revision + 1;
        mutate(&mut document, self.clock.now_utc(), next_revision)?;
        document.revision = next_revision;
        self.persist_document(&meta.path, document, task_id).await
    }
```

(g) `persist_document` — single transaction, no warnings path:

```rust
    async fn persist_document(
        &self,
        path: &str,
        document: TaskDocument,
        focused_task_id: Uuid,
    ) -> Result<TaskWriteResponse, BrainError> {
        document.validate().map_err(BrainError::TaskValidation)?;
        let version = DocumentVersion {
            revision: document.revision,
            content_hash: version_token(path, document.revision),
        };
        self.index
            .replace_document(path, &version.content_hash, &document)
            .await?;
        tracing::info!(path = %path, revision = document.revision, "任务文档写入完成");
        Ok(TaskWriteResponse {
            task: detail_from_document(&document, path, version, focused_task_id)?,
        })
    }
```

(h) Delete: `load_path`, `mark_dirty`, `clear_dirty` methods; `version_for`, `content_hash`, `slugify`, and the `sync_error` free functions (sync_error already went in Task 2). Add:

```rust
fn version_token(path: &str, revision: i64) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(format!("{path}:{revision}").as_bytes()))
    )
}
```

(`sha2::Digest` / `Sha256` imports stay; `hex::encode` is a crate-level path already used by the old `content_hash`, so no import changes are needed for it.)

(i) `detail_from_document`: delete the `freeform_notes: document.freeform_notes.clone(),` line from the `TaskDetail` construction.

- [ ] **Step 7: Rewire main.rs**

In `backend/src/main.rs`:

Delete line 29: `use crate::infra::task_document_store::ObsidianTaskDocumentStore;`

Replace both construction sites:

```rust
    let task_service = Arc::new(TaskService::new(
        Arc::new(ObsidianTaskDocumentStore::new(obsidian.clone())),
        Arc::new(SqliteTaskIndexStore::new(db.clone())),
    ));
```

→

```rust
    let task_service = Arc::new(TaskService::new(Arc::new(SqliteTaskIndexStore::new(
        db.clone(),
    ))));
```

(and the identical second site using `obsidian_provider`).

- [ ] **Step 8: Run tests to verify they pass**

Run (from `backend/`): `cargo test`
Expected: all service + index-store + handler tests PASS, including the rotated-token and stale-version tests.

- [ ] **Step 9: Gates + commit**

Run (from `backend/`): `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
Also verify no stragglers: `grep -rn "serde_yaml\|markdown_codec\|task_document_store\|TaskDocumentStore\|enqueue_sync\|dirty_paths\|slugify\|version_for" backend/src backend/Cargo.toml` → expect zero hits.

```bash
git add backend/src backend/Cargo.toml backend/Cargo.lock
git commit -m "refactor(tasks): cut task storage over to SQLite as sole authority"
```

---

### Task 4: Frontend response-shape cleanup

The backend no longer returns `warnings` / `freeform_notes`; remove the dead type fields and the now-meaningless "位置" fact row.

**Files:**
- Modify: `frontend/src/api/tasks.ts`
- Modify: `frontend/src/views/Tasks.vue`

**Interfaces:**
- Consumes: Task 3's response shapes (`TaskWriteResponse` without `warnings`, `TaskDetail` without `freeform_notes`).
- Produces: frontend types matching the backend exactly; detail-facts grid with 3 columns.

- [ ] **Step 1: api/tasks.ts**

Delete `freeform_notes: string` from `TaskDetail` and `warnings: string[]` from `TaskWriteResponse`:

```ts
export interface TaskWriteResponse {
  task: TaskDetail
  warnings: string[]
}
```

→

```ts
export interface TaskWriteResponse {
  task: TaskDetail
}
```

- [ ] **Step 2: Tasks.vue — remove 位置 row, 4-col → 3-col**

Delete from `detail-facts`:

```html
            <div><span>位置</span><strong>{{ detail.storage_path }}</strong></div>
```

Change the CSS:

```css
.detail-facts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin: 22px 0; }
```

→

```css
.detail-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 22px 0; }
```

(Leave the mobile `.detail-facts` rules untouched.)

- [ ] **Step 3: Gates + commit**

Run (from `frontend/`): `npx vue-tsc -b && npm test`
Expected: clean. (`storage_path` remains in the API types — the backend still returns it; only the display is gone.)

```bash
git add frontend/src
git commit -m "refactor(tasks-ui): drop sync-era detail fields and location fact"
```

---

### Task 5: Migration 010 — drop sync DB objects

**Files:**
- Create: `backend/migrations/010_remove_task_sync.sql`
- Modify: `backend/src/infra/sqlite_store.rs`（MIGRATIONS entry + migration count test + new test）
- Test: `backend/src/infra/sqlite_store.rs`

**Interfaces:**
- Consumes: Task 3 left no writer of `task_sync_queue` or `task_documents.sync_error`.
- Produces: schema at version 10: no `task_sync_queue` table, no `sync_error` column.

- [ ] **Step 1: Write the failing test**

In `backend/src/infra/sqlite_store.rs` tests module, add:

```rust
    #[test]
    fn test_migration_010_removes_task_sync_objects() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();

        let queue_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='task_sync_queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!queue_exists);

        let sync_error_columns: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('task_documents') WHERE name='sync_error'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sync_error_columns, 0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run (from `backend/`): `cargo test migration_010`
Expected: FAIL — `task_sync_queue` still exists (only 9 migrations applied).

- [ ] **Step 3: Add the migration**

Create `backend/migrations/010_remove_task_sync.sql`:

```sql
-- 任务中枢脱离 Obsidian 同步：删除同步队列与文档同步错误标记（设计见
-- docs/superpowers/specs/2026-08-22-tasks-decouple-obsidian-design.md）
DROP TABLE IF EXISTS task_sync_queue;

ALTER TABLE task_documents DROP COLUMN sync_error;
```

In `backend/src/infra/sqlite_store.rs`, append to the `MIGRATIONS` const (follow the existing entry format):

```rust
    Migration {
        version: 10,
        description: "remove task sync queue and sync_error marker",
        sql: include_str!("../../migrations/010_remove_task_sync.sql"),
    },
```

Update `test_new_creates_db_and_runs_migrations`: `assert_eq!(count, 9);` → `assert_eq!(count, 10);`

- [ ] **Step 4: Run tests to verify they pass**

Run (from `backend/`): `cargo test sqlite_store`
Expected: PASS (including idempotency test — opening the same DB twice must not re-apply or fail).

- [ ] **Step 5: Gates + commit**

Run (from `backend/`): `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`

```bash
git add backend/migrations/010_remove_task_sync.sql backend/src/infra/sqlite_store.rs
git commit -m "refactor(tasks): drop task sync queue table and sync_error column"
```

---

### Task 6: Update design docs

**Files:**
- Modify: `docs/requirement/09-task-management.md`
- Modify: `docs/development/09-task-management.md`
- Modify: `docs/development/02-tool-protocol.md`

**Interfaces:**
- Consumes: final code state from Tasks 1–5.
- Produces: docs describing SQLite as sole task authority, 10 task tools.

- [ ] **Step 1: requirement/09-task-management.md**

- Storage principle (§ around line 60, "Obsidian 持久化、SQLite 索引和手动同步"）→ rewrite to: 任务数据以 SQLite 为唯一权威存储（`task_documents` 等四张表），不再读写任何 Markdown 文件；SQLite 不可由 vault 重建，备份属基础设施话题另行立项。
- Delete the §4.7 Obsidian 同步 section entirely (around lines 361–366) and renumber subsequent sections if the doc numbers them sequentially.
- Frontmatter/文件权威性表述（around lines 391–393）→ rewrite: 数据权威在 SQLite；`task_documents.path` 仅为文档主键（存量行为 `Tasks/...` 路径，新文档为 `db:short:{uuid}` / `db:long:{uuid}` 合成键），无文件系统语义。
- Tool table（around line 575）: remove the `sync_tasks` row; remove the sync-related "版本豁免" note (around line 577) if it refers to sync being exempt from OCC.
- Delete the `index_out_of_sync` degradation behavior description (around line 655).
- Delete the sync-related acceptance criterion (around line 719).
- Search the file for `同步` / `sync` / `Tasks/Short` / `Tasks/Long` and adjust every remaining statement to the new reality (e.g. 位置/文件路径 references).

- [ ] **Step 2: development/09-task-management.md**

- Planned storage layout（around line 64: 同步队列/脏路径/启动扫描）→ delete those items; storage layer is `SqliteTaskIndexStore` alone.
- Module layout（around line 87）→ remove `markdown_codec` / `task_document_store` / `sync` references; actual files are `core/tasks/{service,tree}.rs` + `infra/task_index_store.rs`.
- Queue table schema（around lines 492–498）→ delete.
- Hash-skip incremental scan note（around line 512）→ delete.
- §9.1 / §9.2 sync procedures and §9.4 external-edit warning → delete sections.
- §16.3 sync tests → delete.
- Metrics（around lines 952–964）→ delete sync metrics.
- Storage-layer description sections: `mutate_document` flow becomes "load rows → assemble document → apply closure → revision+1 → new version token → single-transaction `replace_document`"; `TaskDocument` field list drops `schema`/`extra`/`freeform_notes`; version token = `sha256(path:revision)` regenerated per write; write failures roll back atomically (no `index_out_of_sync` path).
- `TaskService::new(index)` constructor signature in any wiring snippet.
- Search for `同步` / `sync` / `markdown` / `frontmatter` / `slugify` and clean every hit.

- [ ] **Step 3: development/02-tool-protocol.md**

Update the task tools section: remove `sync_tasks`, state 10 task tools. Search the file for `sync_tasks` and fix every occurrence (tool table, counts, examples).

- [ ] **Step 4: Verify no stale references + commit**

Run: `grep -rn "sync_tasks\|task_sync_queue\|index_out_of_sync\|markdown_codec" docs/` → expect zero hits (historical mentions inside `docs/superpowers/` specs/plans are fine — do not touch those).

```bash
git add docs/requirement/09-task-management.md docs/development/09-task-management.md docs/development/02-tool-protocol.md
git commit -m "docs(tasks): document SQLite as the sole task authority"
```

---

### Task 7: Full verification, release build, and rollout

No new code. Prove the whole thing, then ship.

**Files:** none (build + live verification).

**Interfaces:**
- Consumes: everything above.
- Produces: updated installed release binary serving the new frontend; user's DB migrated to schema v10.

- [ ] **Step 1: Final full gates**

From `backend/`: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
From `frontend/`: `npm test` and `npx vue-tsc -b`
Expected: all green.

- [ ] **Step 2: Final sync against the OLD binary (one-time, before deploy)**

With the still-running old binary on :9876, capture any drift from hand-edited vault files into SQLite so nothing is lost at cutover. The local API needs **no auth header**; request shape is `{"tool": ..., "arguments": ...}` (verified live):

```bash
curl -s -X POST http://127.0.0.1:9876/v1/tools/call \
  -H 'Content-Type: application/json' \
  -d '{"tool": "sync_tasks", "arguments": {"dry_run": false}}'
```

Response is an envelope `{"tool", "status", "result"}` — note the `result.created` / `result.updated` counts.

- [ ] **Step 3: Build frontend into the release binary**

```bash
cd frontend && npx vite build --outDir dist_new
cd ../backend && cargo build --release
```

- [ ] **Step 4: Install + restart (needs the user / sudo)**

Tell the user to run:

```bash
sudo cp backend/target/release/obsidian-brain /usr/local/bin/obsidian-brain
# then restart the service the way they usually do (obsidian-brain start)
```

(The migration to schema v10 runs automatically on first startup.)

- [ ] **Step 5: Live smoke test against the new binary**

```bash
# 1. migration applied + schema clean
sqlite3 ~/.obsidian-brain/brain.db "SELECT version FROM _migrations ORDER BY version DESC LIMIT 1"   # → 10
sqlite3 ~/.obsidian-brain/brain.db "SELECT COUNT(*) FROM sqlite_master WHERE name='task_sync_queue'" # → 0

# 2. existing tasks still listed (count should match pre-cutover state)
curl -s -X POST http://127.0.0.1:9876/v1/tools/call -H 'Content-Type: application/json' \
  -d '{"tool": "list_tasks", "arguments": {}}'

# 3. create → update → version conflict still enforced
#    (create_task short; update_task with returned document_version → ok;
#     replay the same version → envelope status "error", code TASK_VERSION_CONFLICT)
```

Also load the Tasks page in a browser: list renders, detail opens (no 位置 row), create/update/progress flows work, no sync button.

- [ ] **Step 6: Update project memory**

In `~/.claude/projects/-Users-tiercelchow-Documents-WorkSpace-MyProjects-ObsidianBrain/memory/`:
- Update `backend-run-via-debug-binary.md`: the "no delete-progress tool API → edit md + sync_tasks" workaround is obsolete — SQLite is now the sole authority; removing progress entries is no longer possible via vault edits.
- Update `task-management-reliability-gaps.md`: the 5 unimplemented reliability items (startup scan, queue drain, external-edit warning, dup-id sync_error, index fallback) are moot — the mechanisms they described were deleted. Rewrite the memory to say so (or replace its content with a note that decoupling landed 2026-08-22 and the gaps no longer apply).
- Update `MEMORY.md` index lines accordingly.

- [ ] **Step 7: Rollout commit (if any files remain uncommitted)**

Everything code-wise should already be committed in Tasks 1–6. Verify `git status` is clean apart from known-dirty `frontend/package.json` / `package-lock.json` (pre-existing; leave them). Done.
