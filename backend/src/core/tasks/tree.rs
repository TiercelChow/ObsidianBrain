//! Tree traversal and progress aggregation for long tasks.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::models::task::{TaskDocument, TaskRole, TaskStatus};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProgressMetrics {
    pub percent: u8,
    pub completed_leaf_count: u32,
    pub effective_leaf_count: u32,
}

pub fn calculate_progress(document: &TaskDocument, task_id: Uuid) -> ProgressMetrics {
    let children = children_map(document);
    let mut memo = HashMap::new();
    calculate_node(document, task_id, &children, &mut memo)
}

pub fn descendant_ids(document: &TaskDocument, task_id: Uuid) -> HashSet<Uuid> {
    let children = children_map(document);
    let mut descendants = HashSet::new();
    let mut stack = children.get(&task_id).cloned().unwrap_or_default();
    while let Some(id) = stack.pop() {
        if descendants.insert(id) {
            if let Some(nested) = children.get(&id) {
                stack.extend(nested.iter().copied());
            }
        }
    }
    descendants
}

pub fn normalize_sibling_positions(document: &mut TaskDocument) {
    let mut siblings: HashMap<Option<Uuid>, Vec<usize>> = HashMap::new();
    for (index, node) in document.tasks.iter().enumerate() {
        siblings.entry(node.parent_id).or_default().push(index);
    }
    for indices in siblings.values_mut() {
        indices.sort_by_key(|index| {
            let node = &document.tasks[*index];
            (node.position, node.created_at, node.id)
        });
        for (position, index) in indices.iter().enumerate() {
            document.tasks[*index].position = position as i32;
        }
    }
}

fn calculate_node(
    document: &TaskDocument,
    task_id: Uuid,
    children: &HashMap<Uuid, Vec<Uuid>>,
    memo: &mut HashMap<Uuid, ProgressMetrics>,
) -> ProgressMetrics {
    if let Some(metrics) = memo.get(&task_id) {
        return *metrics;
    }
    let Some(node) = document.node(task_id) else {
        return ProgressMetrics::default();
    };
    if node.status == TaskStatus::Cancelled {
        return ProgressMetrics::default();
    }

    let child_ids = children.get(&task_id).cloned().unwrap_or_default();
    let mut child_metrics = Vec::with_capacity(child_ids.len());
    for child_id in child_ids {
        let metrics = calculate_node(document, child_id, children, memo);
        if metrics.effective_leaf_count > 0 {
            child_metrics.push(metrics);
        }
    }

    let (completed_leaf_count, effective_leaf_count) = if child_metrics.is_empty() {
        (u32::from(node.status == TaskStatus::Completed), 1)
    } else {
        child_metrics.iter().fold((0, 0), |acc, metrics| {
            (
                acc.0 + metrics.completed_leaf_count,
                acc.1 + metrics.effective_leaf_count,
            )
        })
    };

    let explicit = document
        .progress
        .iter()
        .filter(|entry| entry.task_id == task_id && entry.percent_after.is_some())
        .max_by_key(|entry| (entry.recorded_at, entry.created_at, entry.id))
        .and_then(|entry| entry.percent_after);

    let percent = if node.status == TaskStatus::Completed {
        100
    } else if let Some(percent) = explicit {
        percent
    } else if child_metrics.is_empty() {
        0
    } else {
        let total: u32 = child_metrics
            .iter()
            .map(|metrics| u32::from(metrics.percent))
            .sum();
        ((total as f32 / child_metrics.len() as f32).round() as u8).min(100)
    };

    let result = ProgressMetrics {
        percent,
        completed_leaf_count,
        effective_leaf_count,
    };
    memo.insert(task_id, result);
    result
}

fn children_map(document: &TaskDocument) -> HashMap<Uuid, Vec<Uuid>> {
    let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for node in document
        .tasks
        .iter()
        .filter(|node| node.role == TaskRole::Subtask)
    {
        if let Some(parent_id) = node.parent_id {
            children.entry(parent_id).or_default().push(node.id);
        }
    }
    children
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::{NaiveDate, Utc};

    use super::*;
    use crate::models::task::{
        ProgressEntry, TaskDocumentKind, TaskImportance, TaskKind, TaskNode,
    };

    fn node(root_id: Uuid, parent_id: Option<Uuid>, status: TaskStatus, position: i32) -> TaskNode {
        let id = Uuid::new_v4();
        TaskNode {
            id,
            root_id,
            parent_id,
            kind: TaskKind::Long,
            role: if parent_id.is_some() {
                TaskRole::Subtask
            } else {
                TaskRole::Root
            },
            title: format!("任务 {position}"),
            description: String::new(),
            start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date"),
            end_date: NaiveDate::from_ymd_opt(2026, 8, 31).expect("valid date"),
            importance: TaskImportance::Normal,
            status,
            position,
            closure_note: None,
            closed_at: status.is_terminal().then(Utc::now),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            revision: 1,
            archived_at: None,
        }
    }

    fn document() -> TaskDocument {
        let mut root = node(Uuid::nil(), None, TaskStatus::InProgress, 0);
        root.root_id = root.id;
        let mut first = node(root.id, Some(root.id), TaskStatus::Completed, 0);
        first.closed_at = Some(Utc::now());
        let second = node(root.id, Some(root.id), TaskStatus::InProgress, 1);
        TaskDocument {
            schema: "tasks-long/v1".to_string(),
            document_kind: TaskDocumentKind::LongTask,
            storage_month: None,
            revision: 1,
            tasks: vec![root, first, second],
            progress: vec![],
            audit: vec![],
            extra: BTreeMap::new(),
            freeform_notes: String::new(),
        }
    }

    #[test]
    fn test_calculate_progress_averages_children_and_counts_leaves() {
        let document = document();
        let root_id = document.root_id().expect("root exists");
        let metrics = calculate_progress(&document, root_id);
        assert_eq!(metrics.percent, 50);
        assert_eq!(metrics.completed_leaf_count, 1);
        assert_eq!(metrics.effective_leaf_count, 2);
    }

    #[test]
    fn test_explicit_progress_overrides_child_average() {
        let mut document = document();
        let root_id = document.root_id().expect("root exists");
        document.progress.push(ProgressEntry {
            id: Uuid::new_v4(),
            root_id,
            task_id: root_id,
            recorded_at: Utc::now(),
            note: "整体评估".to_string(),
            percent_after: Some(35),
            created_at: Utc::now(),
        });
        assert_eq!(calculate_progress(&document, root_id).percent, 35);
    }

    #[test]
    fn test_cancelled_child_is_excluded_from_progress() {
        let mut document = document();
        document.tasks[2].status = TaskStatus::Cancelled;
        document.tasks[2].closed_at = Some(Utc::now());
        let metrics = calculate_progress(&document, document.tasks[0].id);
        assert_eq!(metrics.percent, 100);
        assert_eq!(metrics.effective_leaf_count, 1);
    }
}
