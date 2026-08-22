//! Application service for personal task management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use chrono::{DateTime, Local, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::core::tasks::tree::{calculate_progress, descendant_ids, normalize_sibling_positions};
use crate::error::BrainError;
use crate::infra::task_index_store::{TaskDocumentMeta, TaskIndexStore};
use crate::models::task::*;

pub trait TaskClock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
    fn today_local(&self) -> NaiveDate;
    fn storage_month(&self) -> String;
}

#[derive(Debug, Default)]
pub struct SystemTaskClock;

impl TaskClock for SystemTaskClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn today_local(&self) -> NaiveDate {
        Local::now().date_naive()
    }

    fn storage_month(&self) -> String {
        Local::now().format("%Y-%m").to_string()
    }
}

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

    pub async fn create_task(
        &self,
        request: TaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        validate_create_request(&request)?;
        match request.kind {
            TaskKind::Short => self.create_short_task(request).await,
            TaskKind::Long => self.create_long_task(request).await,
        }
    }

    pub async fn list_tasks(&self, query: TaskQuery) -> Result<TaskListResponse, BrainError> {
        if query.limit > 200 {
            return Err(BrainError::TaskValidation("limit 不能超过 200".to_string()));
        }
        self.index
            .list_tasks(&query, self.clock.today_local())
            .await
    }

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

    pub async fn update_task(
        &self,
        request: TaskUpdateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.task_id,
            request.expected_version,
            move |document, now, next_revision| {
                let node = document
                    .node_mut(request.task_id)
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                if let Some(title) = request.patch.title {
                    node.title = title.trim().to_string();
                }
                if let Some(description) = request.patch.description {
                    node.description = description;
                }
                if let Some(start_date) = request.patch.start_date {
                    node.start_date = start_date;
                }
                if let Some(end_date) = request.patch.end_date {
                    node.end_date = end_date;
                }
                if let Some(importance) = request.patch.importance {
                    node.importance = importance;
                }
                node.updated_at = now;
                node.revision = next_revision;
                node.validate().map_err(BrainError::TaskValidation)
            },
        )
        .await
    }

    pub async fn set_task_status(
        &self,
        request: TaskStatusRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.task_id,
            request.expected_version,
            move |document, now, next_revision| {
                let current = document
                    .node(request.task_id)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                validate_status_for_kind(current.kind, request.status)?;

                let is_reopen = current.status.is_terminal() && !request.status.is_terminal();
                if current.kind == TaskKind::Short
                    && is_reopen
                    && request.status != TaskStatus::Open
                {
                    return Err(BrainError::TaskValidation(
                        "短期待办重新打开后必须为 open".to_string(),
                    ));
                }

                if current.kind == TaskKind::Long
                    && current.role == TaskRole::Root
                    && request.status == TaskStatus::Completed
                {
                    let descendants = descendant_ids(document, current.id);
                    let active: Vec<Uuid> = descendants
                        .into_iter()
                        .filter(|id| {
                            document
                                .node(*id)
                                .is_some_and(|node| !node.status.is_terminal())
                        })
                        .collect();
                    if !active.is_empty() && !request.cascade {
                        return Err(BrainError::TaskValidation(format!(
                            "仍有 {} 个活动子任务，请先处理或启用 cascade",
                            active.len()
                        )));
                    }
                    for id in active {
                        if let Some(node) = document.node_mut(id) {
                            let old_status = node.status;
                            node.status = TaskStatus::Completed;
                            node.closed_at = Some(now);
                            node.closure_note = Some("随父任务完成".to_string());
                            node.updated_at = now;
                            node.revision = next_revision;
                            document.audit.push(AuditEvent {
                                id: Uuid::new_v4(),
                                root_id: current.root_id,
                                task_id: id,
                                event_type: TaskEventType::CascadeCompleted,
                                from_status: Some(old_status),
                                to_status: Some(TaskStatus::Completed),
                                note: Some("随父任务完成".to_string()),
                                occurred_at: now,
                            });
                        }
                    }
                }

                if current.kind == TaskKind::Long
                    && current.role == TaskRole::Root
                    && request.status == TaskStatus::Cancelled
                    && request.cascade
                {
                    for id in descendant_ids(document, current.id) {
                        if let Some(node) = document.node_mut(id) {
                            if !node.status.is_terminal() {
                                let old_status = node.status;
                                node.status = TaskStatus::Cancelled;
                                node.closed_at = Some(now);
                                node.closure_note = Some("随父任务取消".to_string());
                                node.updated_at = now;
                                node.revision = next_revision;
                                document.audit.push(AuditEvent {
                                    id: Uuid::new_v4(),
                                    root_id: current.root_id,
                                    task_id: id,
                                    event_type: TaskEventType::StatusChanged,
                                    from_status: Some(old_status),
                                    to_status: Some(TaskStatus::Cancelled),
                                    note: Some("随父任务取消".to_string()),
                                    occurred_at: now,
                                });
                            }
                        }
                    }
                }

                let note = request
                    .closure_note
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let node = document
                    .node_mut(request.task_id)
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                node.status = request.status;
                node.updated_at = now;
                node.revision = next_revision;
                if request.status.is_terminal() {
                    node.closed_at = Some(now);
                    node.closure_note = note.clone();
                } else {
                    node.closed_at = None;
                    node.closure_note = None;
                }
                document.audit.push(AuditEvent {
                    id: Uuid::new_v4(),
                    root_id: current.root_id,
                    task_id: current.id,
                    event_type: if is_reopen {
                        TaskEventType::Reopened
                    } else {
                        TaskEventType::StatusChanged
                    },
                    from_status: Some(current.status),
                    to_status: Some(request.status),
                    note: note.or(current.closure_note),
                    occurred_at: now,
                });
                Ok(())
            },
        )
        .await
    }

    pub async fn add_subtask(
        &self,
        request: SubtaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.parent_id,
            request.expected_version,
            move |document, now, next_revision| {
                let parent = document
                    .node(request.parent_id)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskNotFound(request.parent_id.to_string()))?;
                if parent.kind != TaskKind::Long {
                    return Err(BrainError::TaskValidation(
                        "只有长期任务可以添加子任务".to_string(),
                    ));
                }
                let position = document
                    .tasks
                    .iter()
                    .filter(|node| node.parent_id == Some(parent.id))
                    .count() as i32;
                let node = TaskNode {
                    id: Uuid::new_v4(),
                    root_id: parent.root_id,
                    parent_id: Some(parent.id),
                    kind: TaskKind::Long,
                    role: TaskRole::Subtask,
                    title: request.title.trim().to_string(),
                    description: request.description,
                    start_date: request.start_date,
                    end_date: request.end_date,
                    importance: request.importance,
                    status: TaskStatus::Planned,
                    position,
                    closure_note: None,
                    closed_at: None,
                    created_at: now,
                    updated_at: now,
                    revision: next_revision,
                    archived_at: None,
                };
                node.validate().map_err(BrainError::TaskValidation)?;
                document.tasks.push(node);
                Ok(())
            },
        )
        .await
    }

    pub async fn move_subtask(
        &self,
        request: MoveSubtaskRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.task_id,
            request.expected_version,
            move |document, now, next_revision| {
                let moving = document
                    .node(request.task_id)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                if moving.role != TaskRole::Subtask {
                    return Err(BrainError::TaskValidation("长期根任务不能移动".to_string()));
                }
                let new_parent = document
                    .node(request.new_parent_id)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskNotFound(request.new_parent_id.to_string()))?;
                if new_parent.root_id != moving.root_id
                    || new_parent.kind != TaskKind::Long
                    || descendant_ids(document, moving.id).contains(&new_parent.id)
                    || new_parent.id == moving.id
                {
                    return Err(BrainError::TaskValidation(
                        "不能跨根任务移动，也不能移动到自身或后代下".to_string(),
                    ));
                }

                let target = request.position.max(0);
                for sibling in document
                    .tasks
                    .iter_mut()
                    .filter(|node| node.parent_id == Some(new_parent.id) && node.id != moving.id)
                {
                    if sibling.position >= target {
                        sibling.position += 1;
                    }
                }
                let node = document
                    .node_mut(moving.id)
                    .ok_or_else(|| BrainError::TaskNotFound(moving.id.to_string()))?;
                node.parent_id = Some(new_parent.id);
                node.position = target;
                node.updated_at = now;
                node.revision = next_revision;
                normalize_sibling_positions(document);
                document.audit.push(AuditEvent {
                    id: Uuid::new_v4(),
                    root_id: moving.root_id,
                    task_id: moving.id,
                    event_type: TaskEventType::Moved,
                    from_status: None,
                    to_status: None,
                    note: Some(format!("移动到 {}", new_parent.title)),
                    occurred_at: now,
                });
                Ok(())
            },
        )
        .await
    }

    pub async fn add_progress(
        &self,
        request: ProgressCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.task_id,
            request.expected_version,
            move |document, now, next_revision| {
                let node = document
                    .node(request.task_id)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                if node.kind != TaskKind::Long {
                    return Err(BrainError::TaskValidation(
                        "短期待办不支持进展记录".to_string(),
                    ));
                }
                let note = request.note.trim().to_string();
                if note.is_empty() || note.chars().count() > 10_000 {
                    return Err(BrainError::TaskValidation(
                        "进展说明必须为 1–10000 个字符".to_string(),
                    ));
                }
                if request.percent_after.is_some_and(|percent| percent > 100) {
                    return Err(BrainError::TaskValidation(
                        "进度百分比必须在 0–100 之间".to_string(),
                    ));
                }
                document.progress.push(ProgressEntry {
                    id: Uuid::new_v4(),
                    root_id: node.root_id,
                    task_id: node.id,
                    recorded_at: now,
                    note,
                    percent_after: request.percent_after,
                    created_at: now,
                });
                if let Some(node) = document.node_mut(request.task_id) {
                    node.updated_at = now;
                    node.revision = next_revision;
                }
                Ok(())
            },
        )
        .await
    }

    pub async fn archive_task(
        &self,
        request: ArchiveTaskRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        self.mutate_document(
            request.task_id,
            request.expected_version,
            move |document, now, next_revision| {
                let node = document
                    .node_mut(request.task_id)
                    .ok_or_else(|| BrainError::TaskNotFound(request.task_id.to_string()))?;
                if node.role != TaskRole::Root {
                    return Err(BrainError::TaskValidation("子任务不能单独归档".to_string()));
                }
                node.archived_at = request.archived.then_some(now);
                node.updated_at = now;
                node.revision = next_revision;
                let root_id = node.root_id;
                document.audit.push(AuditEvent {
                    id: Uuid::new_v4(),
                    root_id,
                    task_id: request.task_id,
                    event_type: if request.archived {
                        TaskEventType::Archived
                    } else {
                        TaskEventType::Unarchived
                    },
                    from_status: None,
                    to_status: None,
                    note: None,
                    occurred_at: now,
                });
                Ok(())
            },
        )
        .await
    }

    pub async fn calendar_tasks(
        &self,
        query: CalendarTaskQuery,
    ) -> Result<Vec<TaskSummary>, BrainError> {
        if query.end_date < query.start_date {
            return Err(BrainError::TaskValidation(
                "日历结束日期不能早于开始日期".to_string(),
            ));
        }
        if (query.end_date - query.start_date).num_days() > 365 {
            return Err(BrainError::TaskValidation(
                "日历查询范围不能超过 366 天".to_string(),
            ));
        }
        self.index
            .calendar_tasks(&query, self.clock.today_local())
            .await
    }

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

    async fn mutate_document<F>(
        &self,
        task_id: Uuid,
        expected_version: DocumentVersion,
        mutate: F,
    ) -> Result<TaskWriteResponse, BrainError>
    where
        F: FnOnce(&mut TaskDocument, DateTime<Utc>, i64) -> Result<(), BrainError>,
    {
        // 第一次查询只用于确定路径锁的 key；OCC 校验必须基于拿到锁之后的最新
        // 快照，否则锁外读到的 meta 可能已被并发写入越过，补丁会落在客户端从未
        // 见过的内容上（丢失更新）。
        let lock_path = self.meta_for_task(task_id).await?.path;
        let path_lock = self.path_lock(&lock_path)?;
        let _guard = path_lock.lock().await;
        let meta = self.meta_for_task(task_id).await?;
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

    async fn meta_for_task(&self, task_id: Uuid) -> Result<TaskDocumentMeta, BrainError> {
        self.index
            .find_document_by_task(task_id)
            .await?
            .ok_or_else(|| BrainError::TaskNotFound(task_id.to_string()))
    }

    fn path_lock(&self, path: &str) -> Result<Arc<AsyncMutex<()>>, BrainError> {
        let mut locks = self
            .path_locks
            .lock()
            .map_err(|error| BrainError::Internal(format!("任务路径锁已损坏: {error}")))?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(AsyncMutex::new(()));
        locks.insert(path.to_string(), Arc::downgrade(&lock));
        Ok(lock)
    }
}

fn validate_create_request(request: &TaskCreateRequest) -> Result<(), BrainError> {
    if request.title.trim().is_empty() || request.title.trim().chars().count() > 200 {
        return Err(BrainError::TaskValidation(
            "标题必须为 1–200 个字符".to_string(),
        ));
    }
    if request.description.chars().count() > 10_000 {
        return Err(BrainError::TaskValidation(
            "描述不能超过 10000 个字符".to_string(),
        ));
    }
    if request.end_date < request.start_date {
        return Err(BrainError::TaskValidation(
            "结束日期不能早于开始日期".to_string(),
        ));
    }
    Ok(())
}

fn validate_status_for_kind(kind: TaskKind, status: TaskStatus) -> Result<(), BrainError> {
    let valid = match kind {
        TaskKind::Short => matches!(
            status,
            TaskStatus::Open | TaskStatus::Completed | TaskStatus::Cancelled
        ),
        TaskKind::Long => !matches!(status, TaskStatus::Open),
    };
    valid.then_some(()).ok_or_else(|| {
        BrainError::TaskValidation(format!(
            "{} 任务不能使用 {} 状态",
            kind.as_str(),
            status.as_str()
        ))
    })
}

fn detail_from_document(
    document: &TaskDocument,
    path: &str,
    version: DocumentVersion,
    focused_task_id: Uuid,
) -> Result<TaskDetail, BrainError> {
    let focused = document
        .node(focused_task_id)
        .cloned()
        .ok_or_else(|| BrainError::TaskNotFound(focused_task_id.to_string()))?;
    let root = if focused.kind == TaskKind::Short {
        focused.clone()
    } else {
        document
            .node(focused.root_id)
            .filter(|node| node.role == TaskRole::Root)
            .cloned()
            .ok_or_else(|| BrainError::TaskDocumentCorrupt {
                path: path.to_string(),
                detail: format!("任务 {} 缺少根任务", focused.id),
            })?
    };
    let metrics = calculate_progress(document, root.id);
    let tasks: Vec<TaskNode> = document
        .tasks
        .iter()
        .filter(|node| {
            if root.kind == TaskKind::Short {
                node.id == root.id
            } else {
                node.root_id == root.id
            }
        })
        .cloned()
        .collect();
    let mut progress: Vec<ProgressEntry> = document
        .progress
        .iter()
        .filter(|entry| entry.root_id == root.id)
        .cloned()
        .collect();
    progress.sort_by_key(|entry| std::cmp::Reverse(entry.recorded_at));
    Ok(TaskDetail {
        root,
        tasks,
        progress,
        audit: document
            .audit
            .iter()
            .filter(|event| event.root_id == focused.root_id)
            .cloned()
            .collect(),
        storage_path: path.to_string(),
        document_version: version,
        progress_percent: metrics.percent,
        completed_leaf_count: metrics.completed_leaf_count,
        effective_leaf_count: metrics.effective_leaf_count,
    })
}

fn verify_version(expected: &DocumentVersion, actual: &DocumentVersion) -> Result<(), BrainError> {
    if expected == actual {
        return Ok(());
    }
    Err(BrainError::TaskVersionConflict(format!(
        "expected revision {} / {}, actual revision {} / {}",
        expected.revision, expected.content_hash, actual.revision, actual.content_hash
    )))
}

fn version_token(path: &str, revision: i64) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(format!("{path}:{revision}").as_bytes()))
    )
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use tempfile::TempDir;

    use super::*;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::infra::task_index_store::SqliteTaskIndexStore;

    struct FixedClock;

    impl TaskClock for FixedClock {
        fn now_utc(&self) -> DateTime<Utc> {
            DateTime::parse_from_rfc3339("2026-08-17T02:00:00Z")
                .expect("valid datetime")
                .with_timezone(&Utc)
        }

        fn today_local(&self) -> NaiveDate {
            NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date")
        }

        fn storage_month(&self) -> String {
            "2026-08".to_string()
        }
    }

    fn service() -> (TempDir, TaskService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("tasks.db")).expect("sqlite"));
        let index = Arc::new(SqliteTaskIndexStore::new(db));
        let service = TaskService::with_clock(index, Arc::new(FixedClock));
        (dir, service)
    }

    fn create_request(kind: TaskKind) -> TaskCreateRequest {
        TaskCreateRequest {
            kind,
            title: "测试任务".to_string(),
            description: "描述".to_string(),
            start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2026, 8, 20).expect("valid date"),
            importance: TaskImportance::High,
        }
    }

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

    #[tokio::test]
    async fn test_get_second_short_task_returns_requested_root_only() {
        let (_dir, service) = service();
        let first = service
            .create_task(create_request(TaskKind::Short))
            .await
            .expect("create first");
        let mut second_request = create_request(TaskKind::Short);
        second_request.title = "第二个任务".to_string();
        let second = service
            .create_task(second_request)
            .await
            .expect("create second");

        let detail = service
            .get_task(second.task.root.id)
            .await
            .expect("get second");
        assert_ne!(detail.root.id, first.task.root.id);
        assert_eq!(detail.root.id, second.task.root.id);
        assert_eq!(detail.root.title, "第二个任务");
        assert_eq!(detail.tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_update_rejects_stale_content_hash() {
        let (_dir, service) = service();
        let created = service
            .create_task(create_request(TaskKind::Long))
            .await
            .expect("create");
        let mut version = created.task.document_version.clone();
        version.content_hash = "sha256:stale".to_string();
        let error = service
            .update_task(TaskUpdateRequest {
                task_id: created.task.root.id,
                patch: TaskPatch {
                    title: Some("新标题".to_string()),
                    ..Default::default()
                },
                expected_version: version,
            })
            .await
            .expect_err("must conflict");
        assert!(matches!(error, BrainError::TaskVersionConflict(_)));
    }

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

    #[tokio::test]
    async fn test_long_task_subtask_progress_and_cascade_completion() {
        let (_dir, service) = service();
        let created = service
            .create_task(create_request(TaskKind::Long))
            .await
            .expect("create");
        let root_id = created.task.root.id;
        let with_child = service
            .add_subtask(SubtaskCreateRequest {
                parent_id: root_id,
                title: "第一步".to_string(),
                description: String::new(),
                start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("date"),
                end_date: NaiveDate::from_ymd_opt(2026, 8, 18).expect("date"),
                importance: TaskImportance::Normal,
                expected_version: created.task.document_version,
            })
            .await
            .expect("add child");
        let child_id = with_child
            .task
            .tasks
            .iter()
            .find(|node| node.role == TaskRole::Subtask)
            .expect("child")
            .id;
        let progressed = service
            .add_progress(ProgressCreateRequest {
                task_id: child_id,
                note: "已完成一半".to_string(),
                percent_after: Some(50),
                expected_version: with_child.task.document_version,
            })
            .await
            .expect("progress");
        assert_eq!(progressed.task.progress_percent, 50);

        let blocked = service
            .set_task_status(TaskStatusRequest {
                task_id: root_id,
                status: TaskStatus::Completed,
                closure_note: Some("完成".to_string()),
                cascade: false,
                expected_version: progressed.task.document_version.clone(),
            })
            .await
            .expect_err("active child must block completion");
        assert!(matches!(blocked, BrainError::TaskValidation(_)));

        let completed = service
            .set_task_status(TaskStatusRequest {
                task_id: root_id,
                status: TaskStatus::Completed,
                closure_note: Some("完成".to_string()),
                cascade: true,
                expected_version: progressed.task.document_version,
            })
            .await
            .expect("cascade completion");
        assert_eq!(completed.task.root.status, TaskStatus::Completed);
        assert_eq!(
            completed
                .task
                .tasks
                .iter()
                .find(|node| node.id == child_id)
                .expect("child")
                .status,
            TaskStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_short_task_close_and_reopen_preserves_audit_note() {
        let (_dir, service) = service();
        let created = service
            .create_task(create_request(TaskKind::Short))
            .await
            .expect("create");
        let task_id = created.task.root.id;
        let closed = service
            .set_task_status(TaskStatusRequest {
                task_id,
                status: TaskStatus::Completed,
                closure_note: Some("已经处理完".to_string()),
                cascade: false,
                expected_version: created.task.document_version,
            })
            .await
            .expect("close");
        assert_eq!(closed.task.root.closure_note.as_deref(), Some("已经处理完"));
        let reopened = service
            .set_task_status(TaskStatusRequest {
                task_id,
                status: TaskStatus::Open,
                closure_note: None,
                cascade: false,
                expected_version: closed.task.document_version,
            })
            .await
            .expect("reopen");
        assert_eq!(reopened.task.root.status, TaskStatus::Open);
        assert!(reopened.task.root.closure_note.is_none());
        assert!(reopened
            .task
            .audit
            .iter()
            .any(|event| event.note.as_deref() == Some("已经处理完")));
    }

    /// 装饰真实索引存储：第一次 `find_document_by_task` 返回注入的过期 meta，
    /// 之后全部委托给真实存储。用于确定性地复现"请求 A 在拿到路径锁之前读到
    /// 旧 meta，请求 B 的写入随后落库，A 加锁后加载到的已是 B 写过的新文档"
    /// 这一并发交错（真实存储里已经是 rev N+1 的文档与 meta）。
    struct StaleFirstLookupStore {
        inner: Arc<SqliteTaskIndexStore>,
        stale_meta: Mutex<Option<TaskDocumentMeta>>,
    }

    #[async_trait]
    impl TaskIndexStore for StaleFirstLookupStore {
        async fn replace_document(
            &self,
            path: &str,
            content_hash: &str,
            document: &TaskDocument,
        ) -> Result<(), BrainError> {
            self.inner
                .replace_document(path, content_hash, document)
                .await
        }

        async fn find_document_by_task(
            &self,
            task_id: Uuid,
        ) -> Result<Option<TaskDocumentMeta>, BrainError> {
            if let Some(stale) = self.stale_meta.lock().expect("stale meta lock").take() {
                return Ok(Some(stale));
            }
            self.inner.find_document_by_task(task_id).await
        }

        async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError> {
            self.inner.load_document(path).await
        }

        async fn list_tasks(
            &self,
            query: &TaskQuery,
            today: NaiveDate,
        ) -> Result<TaskListResponse, BrainError> {
            self.inner.list_tasks(query, today).await
        }

        async fn calendar_tasks(
            &self,
            query: &CalendarTaskQuery,
            today: NaiveDate,
        ) -> Result<Vec<TaskSummary>, BrainError> {
            self.inner.calendar_tasks(query, today).await
        }
    }

    #[tokio::test]
    async fn test_update_conflicts_when_meta_written_after_prelock_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("tasks.db")).expect("sqlite"));
        let real = Arc::new(SqliteTaskIndexStore::new(db));
        let writer = TaskService::with_clock(real.clone(), Arc::new(FixedClock));
        let created = writer
            .create_task(create_request(TaskKind::Long))
            .await
            .expect("create");
        let task_id = created.task.root.id;

        // 请求 B 先完成一次写入：文档从 rev 1 真实推进到 rev 2。
        let bumped = writer
            .update_task(TaskUpdateRequest {
                task_id,
                patch: TaskPatch {
                    title: Some("请求 B 的标题".to_string()),
                    ..Default::default()
                },
                expected_version: created.task.document_version.clone(),
            })
            .await
            .expect("request B writes rev 2");
        assert_eq!(bumped.task.document_version.revision, 2);

        // 请求 A 仍持有创建时的 rev-1 版本；装饰器把 A 的第一次 meta 查询
        // （发生在拿到路径锁之前）固定为过期的 rev-1 meta，模拟 B 的写入
        // 恰好落在 A 读取 meta 与 A 加锁之间。
        let stale_meta = TaskDocumentMeta {
            path: created.task.storage_path.clone(),
            revision: created.task.document_version.revision,
            content_hash: created.task.document_version.content_hash.clone(),
        };
        let victim = TaskService::with_clock(
            Arc::new(StaleFirstLookupStore {
                inner: real,
                stale_meta: Mutex::new(Some(stale_meta)),
            }),
            Arc::new(FixedClock),
        );
        let error = victim
            .update_task(TaskUpdateRequest {
                task_id,
                patch: TaskPatch {
                    title: Some("请求 A 的标题".to_string()),
                    ..Default::default()
                },
                expected_version: created.task.document_version,
            })
            .await
            .expect_err("stale pre-lock snapshot must conflict");
        assert!(matches!(error, BrainError::TaskVersionConflict(_)));
    }
}
