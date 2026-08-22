//! Personal task management domain models.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Short,
    Long,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Long => "long",
        }
    }
}

impl FromStr for TaskKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "short" => Ok(Self::Short),
            "long" => Ok(Self::Long),
            _ => Err(format!("未知任务类型: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    Root,
    Subtask,
}

impl TaskRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Subtask => "subtask",
        }
    }
}

impl FromStr for TaskRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "root" => Ok(Self::Root),
            "subtask" => Ok(Self::Subtask),
            _ => Err(format!("未知任务角色: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskImportance {
    Low,
    Normal,
    High,
    Urgent,
}

impl TaskImportance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

impl FromStr for TaskImportance {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            _ => Err(format!("未知重要程度: {value}")),
        }
    }
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

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

impl FromStr for TaskStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "planned" => Ok(Self::Planned),
            "in_progress" => Ok(Self::InProgress),
            "blocked" => Ok(Self::Blocked),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("未知任务状态: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskNode {
    pub id: Uuid,
    pub root_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: TaskKind,
    pub role: TaskRole,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub importance: TaskImportance,
    pub status: TaskStatus,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub closure_note: Option<String>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revision: i64,
    #[serde(default)]
    pub archived_at: Option<DateTime<Utc>>,
}

impl TaskNode {
    pub fn validate(&self) -> Result<(), String> {
        let title = self.title.trim();
        if title.is_empty() || title.chars().count() > 200 {
            return Err("任务标题必须为 1–200 个字符".to_string());
        }
        if self.description.chars().count() > 10_000 {
            return Err("任务描述不能超过 10000 个字符".to_string());
        }
        if self
            .closure_note
            .as_deref()
            .is_some_and(|note| note.chars().count() > 10_000)
        {
            return Err("关闭说明不能超过 10000 个字符".to_string());
        }
        if self.end_date < self.start_date {
            return Err("结束日期不能早于开始日期".to_string());
        }
        if self.position < 0 {
            return Err("任务位置不能为负数".to_string());
        }
        if self.revision < 1 {
            return Err("任务 revision 必须大于等于 1".to_string());
        }
        match self.kind {
            TaskKind::Short => {
                if self.role != TaskRole::Root || self.parent_id.is_some() {
                    return Err("短期待办只能是根任务".to_string());
                }
                if !matches!(
                    self.status,
                    TaskStatus::Open | TaskStatus::Completed | TaskStatus::Cancelled
                ) {
                    return Err("短期待办状态必须为 open/completed/cancelled".to_string());
                }
            }
            TaskKind::Long => {
                if self.status == TaskStatus::Open {
                    return Err("长期任务不能使用 open 状态".to_string());
                }
            }
        }
        if self.status.is_terminal() != self.closed_at.is_some() {
            return Err("终态任务必须设置 closed_at，活动任务不能设置 closed_at".to_string());
        }
        if self.role == TaskRole::Subtask && self.archived_at.is_some() {
            return Err("子任务不能单独归档".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressEntry {
    pub id: Uuid,
    pub root_id: Uuid,
    pub task_id: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub note: String,
    #[serde(default)]
    pub percent_after: Option<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskEventType {
    StatusChanged,
    Reopened,
    Archived,
    Unarchived,
    CascadeCompleted,
    Moved,
}

impl TaskEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StatusChanged => "status_changed",
            Self::Reopened => "reopened",
            Self::Archived => "archived",
            Self::Unarchived => "unarchived",
            Self::CascadeCompleted => "cascade_completed",
            Self::Moved => "moved",
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEvent {
    pub id: Uuid,
    pub root_id: Uuid,
    pub task_id: Uuid,
    pub event_type: TaskEventType,
    #[serde(default)]
    pub from_status: Option<TaskStatus>,
    #[serde(default)]
    pub to_status: Option<TaskStatus>,
    #[serde(default)]
    pub note: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskDocumentKind {
    ShortMonth,
    LongTask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl TaskDocument {
    pub fn root_id(&self) -> Option<Uuid> {
        self.tasks
            .iter()
            .find(|node| node.role == TaskRole::Root)
            .map(|node| node.id)
    }

    pub fn node(&self, id: Uuid) -> Option<&TaskNode> {
        self.tasks.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: Uuid) -> Option<&mut TaskNode> {
        self.tasks.iter_mut().find(|node| node.id == id)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.revision < 1 {
            return Err("文档 revision 必须大于等于 1".to_string());
        }
        if self.tasks.is_empty() {
            return Err("任务文档不能为空".to_string());
        }
        if self.tasks.len() > 5_000 {
            return Err("单个任务文档不能超过 5000 个节点".to_string());
        }

        let mut by_id = HashMap::with_capacity(self.tasks.len());
        for node in &self.tasks {
            node.validate()?;
            if by_id.insert(node.id, node).is_some() {
                return Err(format!("任务 ID 重复: {}", node.id));
            }
        }

        match self.document_kind {
            TaskDocumentKind::ShortMonth => {
                if self
                    .storage_month
                    .as_deref()
                    .is_none_or(|month| month.len() != 7 || month.as_bytes().get(4) != Some(&b'-'))
                {
                    return Err("短期待办文档缺少合法 storage_month".to_string());
                }
                if !self.progress.is_empty() {
                    return Err("短期待办不支持进展记录".to_string());
                }
                for node in &self.tasks {
                    if node.kind != TaskKind::Short
                        || node.role != TaskRole::Root
                        || node.root_id != node.id
                    {
                        return Err("短期待办月份文件只能包含独立短期根任务".to_string());
                    }
                }
            }
            TaskDocumentKind::LongTask => {
                if self.storage_month.is_some() {
                    return Err("长期任务文档不能设置 storage_month".to_string());
                }
                let roots: Vec<&TaskNode> = self
                    .tasks
                    .iter()
                    .filter(|node| node.role == TaskRole::Root)
                    .collect();
                if roots.len() != 1 {
                    return Err("长期任务文档必须且只能包含一个根任务".to_string());
                }
                let root = roots[0];
                if root.kind != TaskKind::Long
                    || root.parent_id.is_some()
                    || root.root_id != root.id
                {
                    return Err("长期根任务身份字段不合法".to_string());
                }
                for node in &self.tasks {
                    if node.kind != TaskKind::Long || node.root_id != root.id {
                        return Err("长期任务节点必须属于同一个根任务".to_string());
                    }
                    if node.role == TaskRole::Subtask {
                        let parent_id = node
                            .parent_id
                            .ok_or_else(|| "子任务必须设置 parent_id".to_string())?;
                        if !by_id.contains_key(&parent_id) {
                            return Err(format!("子任务父节点不存在: {parent_id}"));
                        }
                    }
                }

                for node in self
                    .tasks
                    .iter()
                    .filter(|node| node.role == TaskRole::Subtask)
                {
                    let mut cursor = node;
                    let mut visited = HashSet::new();
                    let mut depth = 0usize;
                    while let Some(parent_id) = cursor.parent_id {
                        if !visited.insert(cursor.id) {
                            return Err(format!("任务树存在循环: {}", node.id));
                        }
                        depth += 1;
                        if depth > 20 {
                            return Err("任务树深度不能超过 20 层".to_string());
                        }
                        cursor = by_id
                            .get(&parent_id)
                            .copied()
                            .ok_or_else(|| format!("子任务父节点不存在: {parent_id}"))?;
                    }
                    if cursor.id != root.id {
                        return Err(format!("任务节点无法追溯到根任务: {}", node.id));
                    }
                }
            }
        }

        for entry in &self.progress {
            let note = entry.note.trim();
            if note.is_empty() || note.chars().count() > 10_000 {
                return Err("进展说明必须为 1–10000 个字符".to_string());
            }
            if entry.percent_after.is_some_and(|percent| percent > 100) {
                return Err("进展百分比必须在 0–100 之间".to_string());
            }
            let node = by_id
                .get(&entry.task_id)
                .ok_or_else(|| format!("进展所属任务不存在: {}", entry.task_id))?;
            if entry.root_id != node.root_id || node.kind != TaskKind::Long {
                return Err("进展记录必须属于同一长期任务".to_string());
            }
        }
        for event in &self.audit {
            let node = by_id
                .get(&event.task_id)
                .ok_or_else(|| format!("审计事件所属任务不存在: {}", event.task_id))?;
            if event.root_id != node.root_id {
                return Err("审计事件必须属于同一根任务".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentVersion {
    pub revision: i64,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreateRequest {
    pub kind: TaskKind,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    #[serde(default = "default_importance")]
    pub importance: TaskImportance,
}

fn default_importance() -> TaskImportance {
    TaskImportance::Normal
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub importance: Option<TaskImportance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdateRequest {
    pub task_id: Uuid,
    pub patch: TaskPatch,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusRequest {
    pub task_id: Uuid,
    pub status: TaskStatus,
    #[serde(default)]
    pub closure_note: Option<String>,
    #[serde(default)]
    pub cascade: bool,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskCreateRequest {
    pub parent_id: Uuid,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    #[serde(default = "default_importance")]
    pub importance: TaskImportance,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSubtaskRequest {
    pub task_id: Uuid,
    pub new_parent_id: Uuid,
    #[serde(default)]
    pub position: i32,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressCreateRequest {
    pub task_id: Uuid,
    pub note: String,
    #[serde(default)]
    pub percent_after: Option<u8>,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveTaskRequest {
    pub task_id: Uuid,
    pub archived: bool,
    pub expected_version: DocumentVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQuery {
    #[serde(default)]
    pub kinds: Vec<TaskKind>,
    #[serde(default)]
    pub statuses: Vec<TaskStatus>,
    #[serde(default)]
    pub importance: Vec<TaskImportance>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub query: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub include_subtasks: bool,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_sort() -> String {
    "priority".to_string()
}

fn default_limit() -> usize {
    50
}

impl Default for TaskQuery {
    fn default() -> Self {
        Self {
            kinds: Vec::new(),
            statuses: Vec::new(),
            importance: Vec::new(),
            start_date: None,
            end_date: None,
            query: None,
            include_archived: false,
            include_subtasks: false,
            sort: default_sort(),
            cursor: None,
            limit: default_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarTaskQuery {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    #[serde(default)]
    pub include_subtasks: bool,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub kinds: Vec<TaskKind>,
    #[serde(default)]
    pub statuses: Vec<TaskStatus>,
    #[serde(default)]
    pub importance: Vec<TaskImportance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDerivedState {
    pub overdue: bool,
    pub active_today: bool,
    pub due_today: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    #[serde(flatten)]
    pub node: TaskNode,
    pub storage_path: String,
    pub document_version: DocumentVersion,
    pub progress_percent: u8,
    pub completed_leaf_count: u32,
    pub effective_leaf_count: u32,
    pub derived: TaskDerivedState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWriteResponse {
    pub task: TaskDetail,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_node(kind: TaskKind, role: TaskRole) -> TaskNode {
        let id = Uuid::new_v4();
        TaskNode {
            id,
            root_id: id,
            parent_id: None,
            kind,
            role,
            title: "测试任务".to_string(),
            description: String::new(),
            start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2026, 8, 18).expect("valid date"),
            importance: TaskImportance::Normal,
            status: if kind == TaskKind::Short {
                TaskStatus::Open
            } else {
                TaskStatus::Planned
            },
            position: 0,
            closure_note: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            revision: 1,
            archived_at: None,
        }
    }

    #[test]
    fn test_validate_short_task_rejects_long_status() {
        let mut node = sample_node(TaskKind::Short, TaskRole::Root);
        node.status = TaskStatus::InProgress;
        assert!(node.validate().is_err());
    }

    #[test]
    fn test_validate_task_rejects_reversed_dates() {
        let mut node = sample_node(TaskKind::Long, TaskRole::Root);
        node.end_date = NaiveDate::from_ymd_opt(2026, 8, 16).expect("valid date");
        assert!(node.validate().is_err());
    }

    #[test]
    fn test_validate_long_document_rejects_cycle() {
        let root = sample_node(TaskKind::Long, TaskRole::Root);
        let mut first = sample_node(TaskKind::Long, TaskRole::Subtask);
        let mut second = sample_node(TaskKind::Long, TaskRole::Subtask);
        first.root_id = root.id;
        second.root_id = root.id;
        first.parent_id = Some(second.id);
        second.parent_id = Some(first.id);
        let document = TaskDocument {
            document_kind: TaskDocumentKind::LongTask,
            storage_month: None,
            revision: 1,
            tasks: vec![root, first, second],
            progress: vec![],
            audit: vec![],
        };
        assert!(document.validate().is_err());
    }
}
