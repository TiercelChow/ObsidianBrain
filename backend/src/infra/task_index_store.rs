//! Rebuildable SQLite projection for personal tasks.

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::{Type, Value as SqlValue};
use rusqlite::{params, params_from_iter, Row};
use uuid::Uuid;

use crate::core::tasks::tree::calculate_progress;
use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::task::{
    AuditEvent, CalendarTaskQuery, DocumentVersion, ProgressEntry, TaskDerivedState, TaskDocument,
    TaskDocumentKind, TaskEventType, TaskImportance, TaskKind, TaskListResponse, TaskNode,
    TaskQuery, TaskRole, TaskStatus, TaskSummary,
};

#[derive(Debug, Clone)]
pub struct TaskDocumentMeta {
    pub path: String,
    pub document_kind: TaskDocumentKind,
    pub root_id: Option<Uuid>,
    pub revision: i64,
    pub content_hash: String,
}

#[async_trait]
pub trait TaskIndexStore: Send + Sync {
    async fn replace_document(
        &self,
        path: &str,
        content_hash: &str,
        document: &TaskDocument,
    ) -> Result<(), BrainError>;
    async fn find_document_by_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<TaskDocumentMeta>, BrainError>;
    async fn load_document(&self, path: &str) -> Result<Option<TaskDocument>, BrainError>;
    async fn document_meta(&self, path: &str) -> Result<Option<TaskDocumentMeta>, BrainError>;
    async fn list_document_paths(&self) -> Result<Vec<String>, BrainError>;
    async fn remove_document(&self, path: &str) -> Result<(), BrainError>;
    async fn list_tasks(
        &self,
        query: &TaskQuery,
        today: NaiveDate,
    ) -> Result<TaskListResponse, BrainError>;
    async fn calendar_tasks(
        &self,
        query: &CalendarTaskQuery,
        today: NaiveDate,
    ) -> Result<Vec<TaskSummary>, BrainError>;
    async fn enqueue_sync(&self, path: &str, reason: &str) -> Result<(), BrainError>;
}

pub struct SqliteTaskIndexStore {
    db: Arc<SqliteStore>,
}

impl SqliteTaskIndexStore {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskIndexStore for SqliteTaskIndexStore {
    async fn replace_document(
        &self,
        path: &str,
        content_hash: &str,
        document: &TaskDocument,
    ) -> Result<(), BrainError> {
        document.validate().map_err(BrainError::TaskValidation)?;
        let root_id = (document.document_kind == TaskDocumentKind::LongTask)
            .then(|| document.root_id())
            .flatten()
            .map(|id| id.to_string());
        let document_kind = match document.document_kind {
            TaskDocumentKind::ShortMonth => "short_month",
            TaskDocumentKind::LongTask => "long_task",
        };
        let indexed_at = Utc::now().to_rfc3339();

        self.db.transaction(|conn| {
            conn.execute("DELETE FROM task_documents WHERE path = ?1", params![path])?;
            conn.execute(
                "INSERT INTO task_documents
                 (path, document_kind, root_id, storage_month, revision, content_hash, indexed_at, sync_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                params![
                    path,
                    document_kind,
                    root_id,
                    document.storage_month,
                    document.revision,
                    content_hash,
                    indexed_at,
                ],
            )?;

            for node in &document.tasks {
                let metrics = calculate_progress(document, node.id);
                let insert_result = conn.execute(
                    "INSERT INTO task_nodes
                     (id, root_id, parent_id, storage_path, kind, role, title, description,
                      status, importance, start_date, end_date, position, closure_note, closed_at,
                      created_at, updated_at, revision, archived_at, progress_percent,
                      completed_leaf_count, effective_leaf_count)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                             ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
                    params![
                        node.id.to_string(),
                        node.root_id.to_string(),
                        node.parent_id.map(|id| id.to_string()),
                        path,
                        node.kind.as_str(),
                        node.role.as_str(),
                        node.title,
                        node.description,
                        node.status.as_str(),
                        node.importance.as_str(),
                        node.start_date.to_string(),
                        node.end_date.to_string(),
                        node.position,
                        node.closure_note,
                        node.closed_at.map(|value| value.to_rfc3339()),
                        node.created_at.to_rfc3339(),
                        node.updated_at.to_rfc3339(),
                        node.revision,
                        node.archived_at.map(|value| value.to_rfc3339()),
                        metrics.percent,
                        metrics.completed_leaf_count,
                        metrics.effective_leaf_count,
                    ],
                );
                if let Err(error) = insert_result {
                    if matches!(
                        error,
                        rusqlite::Error::SqliteFailure(ref failure, _)
                            if failure.code == rusqlite::ErrorCode::ConstraintViolation
                    ) {
                        return Err(BrainError::TaskDuplicateId(node.id.to_string()));
                    }
                    return Err(error.into());
                }
            }

            for entry in &document.progress {
                conn.execute(
                    "INSERT INTO task_progress
                     (id, root_id, task_id, storage_path, recorded_at, note, percent_after, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        entry.id.to_string(),
                        entry.root_id.to_string(),
                        entry.task_id.to_string(),
                        path,
                        entry.recorded_at.to_rfc3339(),
                        entry.note,
                        entry.percent_after,
                        entry.created_at.to_rfc3339(),
                    ],
                )?;
            }

            for event in &document.audit {
                conn.execute(
                    "INSERT INTO task_audit_events
                     (id, root_id, task_id, storage_path, event_type, from_status, to_status, note, occurred_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        event.id.to_string(),
                        event.root_id.to_string(),
                        event.task_id.to_string(),
                        path,
                        event.event_type.as_str(),
                        event.from_status.map(TaskStatus::as_str),
                        event.to_status.map(TaskStatus::as_str),
                        event.note,
                        event.occurred_at.to_rfc3339(),
                    ],
                )?;
            }
            conn.execute("DELETE FROM task_sync_queue WHERE path = ?1", params![path])?;
            Ok(())
        })
    }

    async fn find_document_by_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<TaskDocumentMeta>, BrainError> {
        self.db.with_connection(|conn| {
            let result = conn.query_row(
                "SELECT d.path, d.document_kind, d.root_id, d.revision, d.content_hash
                 FROM task_nodes n JOIN task_documents d ON d.path = n.storage_path
                 WHERE n.id = ?1",
                params![task_id.to_string()],
                meta_from_row,
            );
            optional_row(result)
        })
    }

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
                     FROM task_nodes WHERE storage_path = ?1 ORDER BY rowid",
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

    async fn document_meta(&self, path: &str) -> Result<Option<TaskDocumentMeta>, BrainError> {
        self.db.with_connection(|conn| {
            let result = conn.query_row(
                "SELECT path, document_kind, root_id, revision, content_hash
                 FROM task_documents WHERE path = ?1",
                params![path],
                meta_from_row,
            );
            optional_row(result)
        })
    }

    async fn list_document_paths(&self) -> Result<Vec<String>, BrainError> {
        self.db.with_connection(|conn| {
            let mut statement = conn.prepare("SELECT path FROM task_documents ORDER BY path")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    async fn remove_document(&self, path: &str) -> Result<(), BrainError> {
        self.db.with_connection(|conn| {
            conn.execute("DELETE FROM task_documents WHERE path = ?1", params![path])?;
            Ok(())
        })
    }

    async fn list_tasks(
        &self,
        query: &TaskQuery,
        today: NaiveDate,
    ) -> Result<TaskListResponse, BrainError> {
        let (sql, params, limit, offset) = build_task_query(query, None);
        let mut tasks = self
            .db
            .with_connection(|conn| query_summaries(conn, &sql, &params, today))?;
        let has_more = tasks.len() > limit;
        if has_more {
            tasks.truncate(limit);
        }
        Ok(TaskListResponse {
            tasks,
            next_cursor: has_more.then(|| (offset + limit).to_string()),
        })
    }

    async fn calendar_tasks(
        &self,
        query: &CalendarTaskQuery,
        today: NaiveDate,
    ) -> Result<Vec<TaskSummary>, BrainError> {
        let task_query = TaskQuery {
            kinds: query.kinds.clone(),
            statuses: query.statuses.clone(),
            importance: query.importance.clone(),
            start_date: Some(query.start_date),
            end_date: Some(query.end_date),
            include_archived: query.include_archived,
            include_subtasks: query.include_subtasks,
            limit: 200,
            sort: "start_date".to_string(),
            ..Default::default()
        };
        let (sql, params, _, _) = build_task_query(&task_query, Some(200));
        let mut tasks = self
            .db
            .with_connection(|conn| query_summaries(conn, &sql, &params, today))?;
        tasks.truncate(200);
        Ok(tasks)
    }

    async fn enqueue_sync(&self, path: &str, reason: &str) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO task_sync_queue (path, reason, retry_count, created_at, updated_at)
                 VALUES (?1, ?2, 0, ?3, ?3)
                 ON CONFLICT(path) DO UPDATE SET
                   reason = excluded.reason,
                   retry_count = task_sync_queue.retry_count + 1,
                   updated_at = excluded.updated_at",
                params![path, reason, now],
            )?;
            Ok(())
        })
    }
}

fn build_task_query(
    query: &TaskQuery,
    forced_limit: Option<usize>,
) -> (String, Vec<SqlValue>, usize, usize) {
    let mut sql = String::from(
        "SELECT n.id, n.root_id, n.parent_id, n.kind, n.role, n.title, n.description,
                n.status, n.importance, n.start_date, n.end_date, n.position, n.closure_note,
                n.closed_at, n.created_at, n.updated_at, n.revision, n.archived_at,
                n.progress_percent, n.completed_leaf_count, n.effective_leaf_count,
                d.path, d.revision, d.content_hash
         FROM task_nodes n JOIN task_documents d ON d.path = n.storage_path WHERE 1=1",
    );
    let mut values = Vec::new();

    if !query.include_subtasks {
        sql.push_str(" AND n.role = 'root'");
    }
    if !query.include_archived {
        sql.push_str(
            " AND NOT EXISTS (
                SELECT 1 FROM task_nodes r
                WHERE r.id = n.root_id AND r.archived_at IS NOT NULL
              )",
        );
    }
    push_enum_filter(
        &mut sql,
        &mut values,
        "n.kind",
        query.kinds.iter().map(|value| value.as_str()),
    );
    push_enum_filter(
        &mut sql,
        &mut values,
        "n.status",
        query.statuses.iter().map(|value| value.as_str()),
    );
    push_enum_filter(
        &mut sql,
        &mut values,
        "n.importance",
        query.importance.iter().map(|value| value.as_str()),
    );

    if let Some(start) = query.start_date {
        sql.push_str(" AND n.end_date >= ?");
        values.push(SqlValue::Text(start.to_string()));
    }
    if let Some(end) = query.end_date {
        sql.push_str(" AND n.start_date <= ?");
        values.push(SqlValue::Text(end.to_string()));
    }
    if let Some(search) = query
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = format!("%{}%", escape_like(search));
        sql.push_str(
            " AND (
                n.title LIKE ? ESCAPE '\\' OR n.description LIKE ? ESCAPE '\\'
                OR n.closure_note LIKE ? ESCAPE '\\'
                OR EXISTS (
                    SELECT 1 FROM task_progress p
                    WHERE p.task_id = n.id AND p.note LIKE ? ESCAPE '\\'
                )
              )",
        );
        values.extend((0..4).map(|_| SqlValue::Text(pattern.clone())));
    }

    match query.sort.as_str() {
        "start_date" => sql.push_str(" ORDER BY n.start_date ASC, n.id ASC"),
        "updated_at" => sql.push_str(" ORDER BY n.updated_at DESC, n.id ASC"),
        "created_at" => sql.push_str(" ORDER BY n.created_at DESC, n.id ASC"),
        "importance" => sql.push_str(
            " ORDER BY CASE n.importance
                WHEN 'urgent' THEN 4 WHEN 'high' THEN 3 WHEN 'normal' THEN 2 ELSE 1 END DESC,
              n.end_date ASC, n.id ASC",
        ),
        _ => sql.push_str(
            " ORDER BY CASE n.importance
                WHEN 'urgent' THEN 4 WHEN 'high' THEN 3 WHEN 'normal' THEN 2 ELSE 1 END DESC,
              n.end_date ASC, n.id ASC",
        ),
    }

    let limit = forced_limit.unwrap_or(query.limit).clamp(1, 200);
    let offset = query
        .cursor
        .as_deref()
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    sql.push_str(" LIMIT ? OFFSET ?");
    values.push(SqlValue::Integer((limit + 1) as i64));
    values.push(SqlValue::Integer(offset as i64));
    (sql, values, limit, offset)
}

fn push_enum_filter<'a>(
    sql: &mut String,
    values: &mut Vec<SqlValue>,
    column: &str,
    items: impl Iterator<Item = &'a str>,
) {
    let collected: Vec<&str> = items.collect();
    if collected.is_empty() {
        return;
    }
    let placeholders = std::iter::repeat_n("?", collected.len())
        .collect::<Vec<_>>()
        .join(", ");
    sql.push_str(&format!(" AND {column} IN ({placeholders})"));
    values.extend(
        collected
            .into_iter()
            .map(|value| SqlValue::Text(value.to_string())),
    );
}

fn query_summaries(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[SqlValue],
    today: NaiveDate,
) -> Result<Vec<TaskSummary>, BrainError> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        summary_from_row(row, today)
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn summary_from_row(row: &Row<'_>, today: NaiveDate) -> rusqlite::Result<TaskSummary> {
    let id = parse_uuid(row, 0)?;
    let root_id = parse_uuid(row, 1)?;
    let parent_id = parse_optional_uuid(row, 2)?;
    let kind = parse_enum::<TaskKind>(row, 3)?;
    let role = parse_enum::<TaskRole>(row, 4)?;
    let status = parse_enum::<TaskStatus>(row, 7)?;
    let start_date = parse_date(row, 9)?;
    let end_date = parse_date(row, 10)?;
    let node = TaskNode {
        id,
        root_id,
        parent_id,
        kind,
        role,
        title: row.get(5)?,
        description: row.get(6)?,
        status,
        importance: parse_enum::<TaskImportance>(row, 8)?,
        start_date,
        end_date,
        position: row.get(11)?,
        closure_note: row.get(12)?,
        closed_at: parse_optional_datetime(row, 13)?,
        created_at: parse_datetime(row, 14)?,
        updated_at: parse_datetime(row, 15)?,
        revision: row.get(16)?,
        archived_at: parse_optional_datetime(row, 17)?,
    };
    Ok(TaskSummary {
        node,
        storage_path: row.get(21)?,
        document_version: DocumentVersion {
            revision: row.get(22)?,
            content_hash: row.get(23)?,
        },
        progress_percent: row.get(18)?,
        completed_leaf_count: row.get(19)?,
        effective_leaf_count: row.get(20)?,
        derived: TaskDerivedState {
            overdue: !status.is_terminal() && end_date < today,
            active_today: !status.is_terminal() && start_date <= today && today <= end_date,
            due_today: end_date == today,
        },
    })
}

fn meta_from_row(row: &Row<'_>) -> rusqlite::Result<TaskDocumentMeta> {
    let kind: String = row.get(1)?;
    let document_kind = match kind.as_str() {
        "short_month" => TaskDocumentKind::ShortMonth,
        "long_task" => TaskDocumentKind::LongTask,
        _ => return Err(conversion_error(1, format!("未知文档类型: {kind}"))),
    };
    Ok(TaskDocumentMeta {
        path: row.get(0)?,
        document_kind,
        root_id: parse_optional_uuid(row, 2)?,
        revision: row.get(3)?,
        content_hash: row.get(4)?,
    })
}

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

fn optional_row<T>(result: rusqlite::Result<T>) -> Result<Option<T>, BrainError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn parse_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let value: String = row.get(index)?;
    Uuid::parse_str(&value).map_err(|error| conversion_error(index, error))
}

fn parse_optional_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Uuid>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| Uuid::parse_str(&value).map_err(|error| conversion_error(index, error)))
        .transpose()
}

fn parse_date(row: &Row<'_>, index: usize) -> rusqlite::Result<NaiveDate> {
    let value: String = row.get(index)?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| conversion_error(index, error))
}

fn parse_datetime(row: &Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let value: String = row.get(index)?;
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| conversion_error(index, error))
}

fn parse_optional_datetime(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| conversion_error(index, error))
        })
        .transpose()
}

fn parse_enum<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: FromStr<Err = String>,
{
    let value: String = row.get(index)?;
    T::from_str(&value).map_err(|error| conversion_error(index, error))
}

fn conversion_error(index: usize, error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;

    fn sample_document() -> TaskDocument {
        let id = Uuid::new_v4();
        let now = Utc::now();
        TaskDocument {
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
                title: "索引任务".to_string(),
                description: "用于查询测试".to_string(),
                start_date: NaiveDate::from_ymd_opt(2026, 8, 17).expect("valid date"),
                end_date: NaiveDate::from_ymd_opt(2026, 8, 31).expect("valid date"),
                importance: TaskImportance::High,
                status: TaskStatus::InProgress,
                position: 0,
                closure_note: None,
                closed_at: None,
                created_at: now,
                updated_at: now,
                revision: 1,
                archived_at: None,
            }],
            progress: vec![],
            audit: vec![],
            extra: BTreeMap::new(),
            freeform_notes: String::new(),
        }
    }

    fn store() -> (TempDir, SqliteTaskIndexStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("tasks.db")).expect("sqlite"));
        (dir, SqliteTaskIndexStore::new(db))
    }

    #[tokio::test]
    async fn test_replace_and_query_document_projection() {
        let (_dir, store) = store();
        let document = sample_document();
        store
            .replace_document("Tasks/Long/test.md", "sha256:test", &document)
            .await
            .expect("replace");
        let result = store
            .list_tasks(
                &TaskQuery {
                    query: Some("索引".to_string()),
                    ..Default::default()
                },
                NaiveDate::from_ymd_opt(2026, 8, 20).expect("valid date"),
            )
            .await
            .expect("query");
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.tasks[0].node.title, "索引任务");
        assert!(result.tasks[0].derived.active_today);
    }

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
                    importance: TaskImportance::Normal,
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
        let loaded = store
            .load_document("Tasks/Long/missing.md")
            .await
            .expect("load");
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_remove_document_cascades_projection() {
        let (_dir, store) = store();
        let document = sample_document();
        store
            .replace_document("Tasks/Long/test.md", "sha256:test", &document)
            .await
            .expect("replace");
        store
            .remove_document("Tasks/Long/test.md")
            .await
            .expect("remove");
        let result = store
            .list_tasks(&TaskQuery::default(), Utc::now().date_naive())
            .await
            .expect("query");
        assert!(result.tasks.is_empty());
    }
}
