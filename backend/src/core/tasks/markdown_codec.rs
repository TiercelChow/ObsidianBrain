//! Markdown frontmatter codec for short-month and long-task documents.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::BrainError;
use crate::models::task::{
    AuditEvent, ProgressEntry, TaskDocument, TaskDocumentKind, TaskImportance, TaskNode, TaskRole,
    TaskStatus,
};

pub const GENERATED_START: &str = "<!-- obsidianbrain:generated:start -->";
pub const GENERATED_END: &str = "<!-- obsidianbrain:generated:end -->";
pub const NOTES_START: &str = "<!-- obsidianbrain:notes:start -->";
pub const NOTES_END: &str = "<!-- obsidianbrain:notes:end -->";

#[derive(Debug, Default, Clone)]
pub struct TaskMarkdownCodec;

impl TaskMarkdownCodec {
    pub fn parse(&self, path: &str, markdown: &str) -> Result<TaskDocument, BrainError> {
        let normalized = markdown.replace("\r\n", "\n");
        let (frontmatter, body) = split_frontmatter(path, &normalized)?;
        let probe: SchemaProbe = serde_yaml::from_str(frontmatter)
            .map_err(|error| corrupt(path, format!("YAML 解析失败: {error}")))?;

        let mut document = match probe.schema.as_str() {
            "tasks-short/v1" => {
                let value: ShortFrontmatter = serde_yaml::from_str(frontmatter)
                    .map_err(|error| corrupt(path, format!("短期待办 YAML 解析失败: {error}")))?;
                TaskDocument {
                    schema: value.schema,
                    document_kind: TaskDocumentKind::ShortMonth,
                    storage_month: Some(value.storage_month),
                    revision: value.revision,
                    tasks: value.tasks,
                    progress: Vec::new(),
                    audit: value.audit,
                    extra: value.extra,
                    freeform_notes: String::new(),
                }
            }
            "tasks-long/v1" => {
                let value: LongFrontmatter = serde_yaml::from_str(frontmatter)
                    .map_err(|error| corrupt(path, format!("长期任务 YAML 解析失败: {error}")))?;
                let mut tasks = Vec::with_capacity(value.subtasks.len() + 1);
                tasks.push(value.task);
                tasks.extend(value.subtasks);
                TaskDocument {
                    schema: value.schema,
                    document_kind: TaskDocumentKind::LongTask,
                    storage_month: None,
                    revision: value.revision,
                    tasks,
                    progress: value.progress,
                    audit: value.audit,
                    extra: value.extra,
                    freeform_notes: extract_notes(path, body)?,
                }
            }
            schema => {
                return Err(corrupt(path, format!("不支持的任务 schema: {schema}")));
            }
        };

        normalize_positions(&mut document.tasks);
        document
            .validate()
            .map_err(|detail| corrupt(path, detail))?;
        Ok(document)
    }

    pub fn render(&self, document: &TaskDocument) -> Result<String, BrainError> {
        document.validate().map_err(BrainError::TaskValidation)?;

        let yaml = match document.document_kind {
            TaskDocumentKind::ShortMonth => {
                let value = ShortFrontmatter {
                    schema: document.schema.clone(),
                    storage_month: document.storage_month.clone().ok_or_else(|| {
                        BrainError::TaskValidation("短期待办缺少 storage_month".to_string())
                    })?,
                    revision: document.revision,
                    tasks: document.tasks.clone(),
                    audit: document.audit.clone(),
                    extra: document.extra.clone(),
                };
                serde_yaml::to_string(&value)
            }
            TaskDocumentKind::LongTask => {
                let root = document
                    .tasks
                    .iter()
                    .find(|node| node.role == TaskRole::Root)
                    .cloned()
                    .ok_or_else(|| BrainError::TaskValidation("长期任务缺少根节点".to_string()))?;
                let value = LongFrontmatter {
                    schema: document.schema.clone(),
                    revision: document.revision,
                    task: root,
                    subtasks: document
                        .tasks
                        .iter()
                        .filter(|node| node.role == TaskRole::Subtask)
                        .cloned()
                        .collect(),
                    progress: document.progress.clone(),
                    audit: document.audit.clone(),
                    extra: document.extra.clone(),
                };
                serde_yaml::to_string(&value)
            }
        }
        .map_err(|error| BrainError::Internal(format!("任务 YAML 序列化失败: {error}")))?;

        let generated = match document.document_kind {
            TaskDocumentKind::ShortMonth => render_short_snapshot(document),
            TaskDocumentKind::LongTask => render_long_snapshot(document),
        };

        let mut markdown =
            format!("---\n{yaml}---\n\n{GENERATED_START}\n{generated}\n{GENERATED_END}\n");
        if document.document_kind == TaskDocumentKind::LongTask {
            markdown.push_str(&format!(
                "\n{NOTES_START}\n## 自由笔记\n\n{}\n{NOTES_END}\n",
                document.freeform_notes
            ));
        }
        Ok(markdown)
    }
}

#[derive(Debug, Deserialize)]
struct SchemaProbe {
    #[serde(rename = "obsidianbrain_schema")]
    schema: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShortFrontmatter {
    #[serde(rename = "obsidianbrain_schema")]
    schema: String,
    storage_month: String,
    revision: i64,
    tasks: Vec<TaskNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    audit: Vec<AuditEvent>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LongFrontmatter {
    #[serde(rename = "obsidianbrain_schema")]
    schema: String,
    revision: i64,
    task: TaskNode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    subtasks: Vec<TaskNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    progress: Vec<ProgressEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    audit: Vec<AuditEvent>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

fn split_frontmatter<'a>(path: &str, markdown: &'a str) -> Result<(&'a str, &'a str), BrainError> {
    let rest = markdown
        .strip_prefix("---\n")
        .ok_or_else(|| corrupt(path, "缺少 YAML frontmatter".to_string()))?;
    let boundary = rest
        .find("\n---\n")
        .ok_or_else(|| corrupt(path, "frontmatter 未正确结束".to_string()))?;
    let frontmatter = &rest[..boundary];
    let body = &rest[boundary + 5..];
    Ok((frontmatter, body))
}

fn extract_notes(path: &str, body: &str) -> Result<String, BrainError> {
    let Some(start) = body.find(NOTES_START) else {
        return Ok(String::new());
    };
    let after_start = &body[start + NOTES_START.len()..];
    let end = after_start
        .find(NOTES_END)
        .ok_or_else(|| corrupt(path, "自由笔记区域缺少结束标记".to_string()))?;
    let section = after_start[..end].trim_matches('\n');
    let notes = section
        .strip_prefix("## 自由笔记")
        .unwrap_or(section)
        .trim_matches('\n');
    Ok(notes.to_string())
}

fn normalize_positions(tasks: &mut [TaskNode]) {
    tasks.sort_by_key(|node| (node.parent_id, node.position, node.created_at, node.id));
    let mut next: HashMap<Option<Uuid>, i32> = HashMap::new();
    for node in tasks {
        let position = next.entry(node.parent_id).or_insert(0);
        node.position = *position;
        *position += 1;
    }
}

fn render_short_snapshot(document: &TaskDocument) -> String {
    let month = document.storage_month.as_deref().unwrap_or("未知月份");
    let mut output = format!("# {month} 短期待办\n");
    for (title, predicate) in [
        ("待办", TaskStatus::Open),
        ("已完成", TaskStatus::Completed),
        ("已取消", TaskStatus::Cancelled),
    ] {
        let nodes: Vec<&TaskNode> = document
            .tasks
            .iter()
            .filter(|node| node.status == predicate)
            .collect();
        if nodes.is_empty() {
            continue;
        }
        output.push_str(&format!("\n## {title}\n\n"));
        for node in nodes {
            let checked = if node.status == TaskStatus::Completed {
                "x"
            } else {
                " "
            };
            output.push_str(&format!(
                "- [{checked}] **{}** `{}` · {} → {}\n",
                node.title,
                importance_label(node.importance),
                node.start_date,
                node.end_date
            ));
            if !node.description.is_empty() {
                output.push_str(&format!("  - {}\n", node.description));
            }
            if let Some(note) = &node.closure_note {
                output.push_str(&format!("  - 关闭说明：{note}\n"));
            }
        }
    }
    output.trim_end().to_string()
}

fn render_long_snapshot(document: &TaskDocument) -> String {
    let Some(root) = document
        .tasks
        .iter()
        .find(|node| node.role == TaskRole::Root)
    else {
        return "# 长期任务".to_string();
    };
    let mut output = format!(
        "# {}\n\n> [!info] {} · {} · {} → {}\n\n## 描述\n{}\n\n## 任务拆解\n",
        root.title,
        status_label(root.status),
        importance_label(root.importance),
        root.start_date,
        root.end_date,
        if root.description.is_empty() {
            "暂无描述"
        } else {
            &root.description
        }
    );
    let by_id: HashMap<Uuid, &TaskNode> =
        document.tasks.iter().map(|node| (node.id, node)).collect();
    let mut subtasks: Vec<&TaskNode> = document
        .tasks
        .iter()
        .filter(|node| node.role == TaskRole::Subtask)
        .collect();
    subtasks.sort_by_key(|node| (node.parent_id, node.position));
    if subtasks.is_empty() {
        output.push_str("\n- 尚未拆解\n");
    } else {
        for node in subtasks {
            let depth = node_depth(node, &by_id).min(20);
            let indent = "  ".repeat(depth.saturating_sub(1));
            let checked = if node.status == TaskStatus::Completed {
                "x"
            } else {
                " "
            };
            output.push_str(&format!("{indent}- [{checked}] {}\n", node.title));
        }
    }
    output.push_str("\n## 最近进展\n");
    let mut progress: Vec<&ProgressEntry> = document.progress.iter().collect();
    progress.sort_by_key(|entry| std::cmp::Reverse(entry.recorded_at));
    for entry in progress.into_iter().take(10) {
        let title = by_id
            .get(&entry.task_id)
            .map(|node| node.title.as_str())
            .unwrap_or("未知任务");
        output.push_str(&format!(
            "- {} · {} · {}\n",
            entry.recorded_at.format("%Y-%m-%d %H:%M"),
            title,
            entry.note
        ));
    }
    if document.progress.is_empty() {
        output.push_str("\n- 暂无进展\n");
    }
    output.trim_end().to_string()
}

fn node_depth(node: &TaskNode, by_id: &HashMap<Uuid, &TaskNode>) -> usize {
    let mut depth = 0usize;
    let mut cursor = node.parent_id;
    while let Some(parent_id) = cursor {
        depth += 1;
        cursor = by_id.get(&parent_id).and_then(|parent| parent.parent_id);
        if depth >= 20 {
            break;
        }
    }
    depth
}

fn importance_label(importance: TaskImportance) -> &'static str {
    match importance {
        TaskImportance::Low => "低",
        TaskImportance::Normal => "普通",
        TaskImportance::High => "重要",
        TaskImportance::Urgent => "紧急",
    }
}

fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "待办",
        TaskStatus::Planned => "计划中",
        TaskStatus::InProgress => "进行中",
        TaskStatus::Blocked => "受阻",
        TaskStatus::Completed => "已完成",
        TaskStatus::Cancelled => "已取消",
    }
}

fn corrupt(path: &str, detail: String) -> BrainError {
    BrainError::TaskDocumentCorrupt {
        path: path.to_string(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::{NaiveDate, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::models::task::{
        TaskDocumentKind, TaskImportance, TaskKind, TaskNode, TaskRole, TaskStatus,
    };

    fn long_document() -> TaskDocument {
        let id = Uuid::new_v4();
        let now = Utc::now();
        TaskDocument {
            schema: "tasks-long/v1".to_string(),
            document_kind: TaskDocumentKind::LongTask,
            storage_month: None,
            revision: 3,
            tasks: vec![TaskNode {
                id,
                root_id: id,
                parent_id: None,
                kind: TaskKind::Long,
                role: TaskRole::Root,
                title: "发布个人网站".to_string(),
                description: "完成内容与部署".to_string(),
                start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date"),
                end_date: NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid date"),
                importance: TaskImportance::High,
                status: TaskStatus::InProgress,
                position: 0,
                closure_note: None,
                closed_at: None,
                created_at: now,
                updated_at: now,
                revision: 3,
                archived_at: None,
            }],
            progress: vec![],
            audit: vec![],
            extra: BTreeMap::new(),
            freeform_notes: "这里是不会被覆盖的笔记。".to_string(),
        }
    }

    #[test]
    fn test_long_document_roundtrip_preserves_notes() {
        let codec = TaskMarkdownCodec;
        let original = long_document();
        let markdown = codec.render(&original).expect("render succeeds");
        let parsed = codec
            .parse("Tasks/Long/test.md", &markdown)
            .expect("parse succeeds");
        assert_eq!(parsed.tasks, original.tasks);
        assert_eq!(parsed.freeform_notes, original.freeform_notes);
        assert!(markdown.contains(GENERATED_START));
        assert!(markdown.contains(NOTES_START));
    }

    #[test]
    fn test_parse_rejects_missing_frontmatter() {
        let codec = TaskMarkdownCodec;
        assert!(codec.parse("Tasks/Long/test.md", "# no yaml").is_err());
    }
}
