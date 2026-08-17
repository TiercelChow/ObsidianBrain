//! Application service for personal task management.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

use chrono::{DateTime, Local, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::core::tasks::markdown_codec::TaskMarkdownCodec;
use crate::core::tasks::tree::{calculate_progress, descendant_ids, normalize_sibling_positions};
use crate::error::BrainError;
use crate::infra::task_document_store::TaskDocumentStore;
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
    documents: Arc<dyn TaskDocumentStore>,
    index: Arc<dyn TaskIndexStore>,
    codec: TaskMarkdownCodec,
    path_locks: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
    dirty_paths: Mutex<HashSet<String>>,
    clock: Arc<dyn TaskClock>,
}

impl TaskService {
    pub fn new(documents: Arc<dyn TaskDocumentStore>, index: Arc<dyn TaskIndexStore>) -> Self {
        Self::with_clock(documents, index, Arc::new(SystemTaskClock))
    }

    pub fn with_clock(
        documents: Arc<dyn TaskDocumentStore>,
        index: Arc<dyn TaskIndexStore>,
        clock: Arc<dyn TaskClock>,
    ) -> Self {
        Self {
            documents,
            index,
            codec: TaskMarkdownCodec,
            path_locks: Mutex::new(HashMap::new()),
            dirty_paths: Mutex::new(HashSet::new()),
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
        let (document, markdown) = self.load_path(&meta.path).await?;
        let version = version_for(&document, &markdown);
        if version.content_hash != meta.content_hash || version.revision != meta.revision {
            self.index
                .replace_document(&meta.path, &version.content_hash, &document)
                .await?;
        }
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

    pub async fn sync_tasks(&self, dry_run: bool) -> Result<TaskSyncResult, BrainError> {
        let mut result = TaskSyncResult::default();
        let mut paths = self.documents.list("Tasks/Short").await?;
        paths.extend(self.documents.list("Tasks/Long").await?);
        paths.sort();
        paths.dedup();
        let active_paths: HashSet<String> = paths.iter().cloned().collect();

        for path in paths {
            let markdown = match self.documents.read(&path).await {
                Ok(Some(markdown)) => markdown,
                Ok(None) => continue,
                Err(error) => {
                    result.errors.push(sync_error(&path, &error));
                    continue;
                }
            };
            let content_hash = content_hash(&markdown);
            let existing = self.index.document_meta(&path).await?;
            if existing
                .as_ref()
                .is_some_and(|meta| meta.content_hash == content_hash)
            {
                result.unchanged += 1;
                continue;
            }
            let document = match self.codec.parse(&path, &markdown) {
                Ok(document) => document,
                Err(error) => {
                    result.errors.push(sync_error(&path, &error));
                    continue;
                }
            };
            if existing.is_some() {
                result.updated += 1;
            } else {
                result.created += 1;
            }
            if !dry_run {
                if let Err(error) = self
                    .index
                    .replace_document(&path, &content_hash, &document)
                    .await
                {
                    result.errors.push(sync_error(&path, &error));
                    continue;
                }
                self.clear_dirty(&path);
            }
        }

        for stored_path in self.index.list_document_paths().await? {
            if !active_paths.contains(&stored_path) {
                result.removed += 1;
                if !dry_run {
                    self.index.remove_document(&stored_path).await?;
                }
            }
        }
        Ok(result)
    }

    async fn create_short_task(
        &self,
        request: TaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        let storage_month = self.clock.storage_month();
        let path = format!("Tasks/Short/{storage_month}.md");
        let path_lock = self.path_lock(&path)?;
        let _guard = path_lock.lock().await;
        let now = self.clock.now_utc();
        let mut document = match self.documents.read(&path).await? {
            Some(markdown) => self.codec.parse(&path, &markdown)?,
            None => TaskDocument {
                schema: "tasks-short/v1".to_string(),
                document_kind: TaskDocumentKind::ShortMonth,
                storage_month: Some(storage_month),
                revision: 0,
                tasks: Vec::new(),
                progress: Vec::new(),
                audit: Vec::new(),
                extra: BTreeMap::new(),
                freeform_notes: String::new(),
            },
        };
        let next_revision = document.revision + 1;
        let id = Uuid::new_v4();
        let position = document.tasks.len() as i32;
        document.tasks.push(TaskNode {
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
            position,
            closure_note: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
            revision: next_revision,
            archived_at: None,
        });
        document.revision = next_revision;
        self.persist_document(&path, document, id).await
    }

    async fn create_long_task(
        &self,
        request: TaskCreateRequest,
    ) -> Result<TaskWriteResponse, BrainError> {
        let id = Uuid::new_v4();
        let path = format!(
            "Tasks/Long/{}--{}.md",
            slugify(&request.title),
            &id.simple().to_string()[..8]
        );
        let path_lock = self.path_lock(&path)?;
        let _guard = path_lock.lock().await;
        if self.documents.read(&path).await?.is_some() {
            return Err(BrainError::TaskDuplicateId(id.to_string()));
        }
        let now = self.clock.now_utc();
        let document = TaskDocument {
            schema: "tasks-long/v1".to_string(),
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
            extra: BTreeMap::new(),
            freeform_notes: String::new(),
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
        let meta = self.meta_for_task(task_id).await?;
        let path_lock = self.path_lock(&meta.path)?;
        let _guard = path_lock.lock().await;
        let (mut document, markdown) = self.load_path(&meta.path).await?;
        let actual_version = version_for(&document, &markdown);
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
        let markdown = self.codec.render(&document)?;
        self.documents.write(path, &markdown).await?;
        let version = version_for(&document, &markdown);
        let mut warnings = Vec::new();
        if let Err(error) = self
            .index
            .replace_document(path, &version.content_hash, &document)
            .await
        {
            warnings.push(format!("index_out_of_sync: {error}"));
            let _ = self.index.enqueue_sync(path, &error.to_string()).await;
            self.mark_dirty(path);
        } else {
            self.clear_dirty(path);
        }
        tracing::info!(path = %path, revision = document.revision, "任务文档写入完成");
        Ok(TaskWriteResponse {
            task: detail_from_document(&document, path, version, focused_task_id)?,
            warnings,
        })
    }

    async fn meta_for_task(&self, task_id: Uuid) -> Result<TaskDocumentMeta, BrainError> {
        self.index
            .find_document_by_task(task_id)
            .await?
            .ok_or_else(|| BrainError::TaskNotFound(task_id.to_string()))
    }

    async fn load_path(&self, path: &str) -> Result<(TaskDocument, String), BrainError> {
        let markdown = self
            .documents
            .read(path)
            .await?
            .ok_or_else(|| BrainError::TaskNotFound(path.to_string()))?;
        let document = self.codec.parse(path, &markdown)?;
        Ok((document, markdown))
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

    fn mark_dirty(&self, path: &str) {
        if let Ok(mut dirty) = self.dirty_paths.lock() {
            dirty.insert(path.to_string());
        }
    }

    fn clear_dirty(&self, path: &str) {
        if let Ok(mut dirty) = self.dirty_paths.lock() {
            dirty.remove(path);
        }
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
        freeform_notes: document.freeform_notes.clone(),
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

fn version_for(document: &TaskDocument, markdown: &str) -> DocumentVersion {
    DocumentVersion {
        revision: document.revision,
        content_hash: content_hash(markdown),
    }
}

fn content_hash(content: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for character in title.trim().chars() {
        if character.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.extend(character.to_lowercase());
        } else if character.is_whitespace() || matches!(character, '-' | '_' | '—') {
            pending_dash = true;
        }
        if slug.chars().count() >= 48 {
            break;
        }
    }
    if slug.is_empty() {
        "task".to_string()
    } else {
        slug
    }
}

fn sync_error(path: &str, error: &BrainError) -> TaskSyncError {
    TaskSyncError {
        path: path.to_string(),
        code: error.error_code().to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::RwLock;

    use async_trait::async_trait;
    use tempfile::TempDir;

    use super::*;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::infra::task_index_store::SqliteTaskIndexStore;

    #[derive(Default)]
    struct MemoryDocuments {
        files: RwLock<HashMap<String, String>>,
    }

    #[async_trait]
    impl TaskDocumentStore for MemoryDocuments {
        async fn read(&self, path: &str) -> Result<Option<String>, BrainError> {
            Ok(self
                .files
                .read()
                .map_err(|error| BrainError::Internal(error.to_string()))?
                .get(path)
                .cloned())
        }

        async fn write(&self, path: &str, content: &str) -> Result<(), BrainError> {
            self.files
                .write()
                .map_err(|error| BrainError::Internal(error.to_string()))?
                .insert(path.to_string(), content.to_string());
            Ok(())
        }

        async fn list(&self, prefix: &str) -> Result<Vec<String>, BrainError> {
            Ok(self
                .files
                .read()
                .map_err(|error| BrainError::Internal(error.to_string()))?
                .keys()
                .filter(|path| path.starts_with(prefix))
                .cloned()
                .collect())
        }
    }

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

    fn service() -> (TempDir, Arc<MemoryDocuments>, TaskService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("tasks.db")).expect("sqlite"));
        let documents = Arc::new(MemoryDocuments::default());
        let index = Arc::new(SqliteTaskIndexStore::new(db));
        let service = TaskService::with_clock(documents.clone(), index, Arc::new(FixedClock));
        (dir, documents, service)
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
    async fn test_create_short_task_writes_creation_month_and_indexes() {
        let (_dir, documents, service) = service();
        let response = service
            .create_task(create_request(TaskKind::Short))
            .await
            .expect("create");
        assert_eq!(response.task.storage_path, "Tasks/Short/2026-08.md");
        assert!(documents
            .read("Tasks/Short/2026-08.md")
            .await
            .expect("read")
            .is_some());
        let listed = service
            .list_tasks(TaskQuery::default())
            .await
            .expect("list");
        assert_eq!(listed.tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_get_second_short_task_returns_requested_root_only() {
        let (_dir, _documents, service) = service();
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
        let (_dir, _documents, service) = service();
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
    async fn test_long_task_subtask_progress_and_cascade_completion() {
        let (_dir, _documents, service) = service();
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
        let (_dir, _documents, service) = service();
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
}
