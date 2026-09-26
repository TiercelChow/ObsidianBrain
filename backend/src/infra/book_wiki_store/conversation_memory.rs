//! Small, explicit conversational goals and constraints, never factual evidence.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::BookWikiStore;
use crate::error::BrainError;

/// Server-owned revision/run metadata cannot grant evidence or tool permissions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConversationMemory {
    pub objective: String,
    pub constraints: Vec<String>,
    pub unresolved_questions: Vec<String>,
    pub entity_ids: Vec<String>,
    pub revision: i64,
    pub last_run_id: Option<String>,
}

impl BookWikiStore {
    /// Missing memory for an existing conversation is deliberately empty. The
    /// planner may consult recent user history; old assistant text is not facts.
    pub fn get_conversation_memory(
        &self,
        base_id: &str,
        conversation_id: &str,
    ) -> Result<ConversationMemory, BrainError> {
        self.db.with_connection(|conn| {
            check_scope(conn, base_id, conversation_id)?;
            let mut memory = load_memory(conn, base_id, conversation_id)?.unwrap_or_default();
            let mut current_ids = Vec::with_capacity(memory.entity_ids.len());
            for id in &memory.entity_ids {
                if entity_is_current(conn, base_id, id)? {
                    current_ids.push(id.clone());
                }
            }
            // Source changes may invalidate a focus after the turn completed.
            // Filter it for planning without mutating the persisted revision.
            memory.entity_ids = current_ids;
            Ok(memory)
        })
    }

    /// Replace explicit intent after a completed answer has entered this
    /// conversation. A stale planning snapshot must not overwrite newer goals.
    pub fn save_conversation_memory(
        &self,
        base_id: &str,
        conversation_id: &str,
        run_id: &str,
        expected_revision: i64,
        memory: &ConversationMemory,
    ) -> Result<ConversationMemory, BrainError> {
        if expected_revision < 0 || expected_revision == i64::MAX {
            return Err(validation("会话记忆 revision 无效"));
        }
        let mut normalized = normalize_content(memory)?;
        self.db.transaction(|conn| {
            check_scope(conn, base_id, conversation_id)?;
            let run_started_at: String = conn
                .query_row(
                    "SELECT started_at FROM agent_runs
                     WHERE id=?1 AND knowledge_base_id=?2 AND task_type='knowledge_qa'
                       AND status='completed'
                       AND COALESCE(json_extract(output_json, '$.complete'), 1) != 0",
                    params![run_id, base_id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| validation("会话记忆只能关联当前书籍已完整结束的问答运行"))?;
            let message = conn
                .query_row(
                    "SELECT ordinal, role, run_id FROM knowledge_messages
                     WHERE conversation_id=?1 ORDER BY ordinal DESC LIMIT 1",
                    [conversation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?;
            let ordinal = match message {
                Some((ordinal, role, Some(message_run)))
                    if role == "assistant" && message_run == run_id => ordinal,
                _ => return Err(validation("该问答不是当前会话的最新助手消息，不能更新记忆")),
            };
            let current = load_memory(conn, base_id, conversation_id)?.unwrap_or_default();
            if current.revision != expected_revision {
                return Err(validation("会话记忆版本冲突，请基于最新会话重新规划"));
            }
            if current.revision > 0 {
                let previous_ordinal: i64 = conn.query_row(
                    "SELECT last_message_ordinal FROM knowledge_conversation_memories
                     WHERE conversation_id=?1 AND knowledge_base_id=?2",
                    params![conversation_id, base_id],
                    |row| row.get(0),
                )?;
                if ordinal <= previous_ordinal {
                    return Err(validation("同一或更早的助手消息不能再次覆盖会话记忆"));
                }
                if let Some(previous_run_id) = current.last_run_id.as_deref() {
                    let previous_started_at: Option<String> = conn
                        .query_row("SELECT started_at FROM agent_runs WHERE id=?1", [previous_run_id], |row| row.get(0))
                        .optional()?;
                    if let Some(previous) = previous_started_at {
                        let parse_time = |value: &str| DateTime::parse_from_rfc3339(value)
                            .map_err(|error| BrainError::Internal(format!("会话运行时间损坏: {error}")));
                        if parse_time(&run_started_at)? < parse_time(&previous)? {
                            return Err(validation("迟到的旧问答不能覆盖较新的会话记忆"));
                        }
                    }
                }
            }
            for id in &normalized.entity_ids {
                if !entity_is_current(conn, base_id, id)? {
                    return Err(validation(format!("关注实体不存在、跨书或已经失效: {id}")));
                }
            }
            normalized.revision = expected_revision + 1;
            normalized.last_run_id = Some(run_id.to_string());
            let constraints = serialize_strings(&normalized.constraints)?;
            let questions = serialize_strings(&normalized.unresolved_questions)?;
            let entities = serialize_strings(&normalized.entity_ids)?;
            conn.execute(
                "INSERT INTO knowledge_conversation_memories
                 (conversation_id, knowledge_base_id, objective, constraints_json,
                  unresolved_questions_json, entity_ids_json, revision, last_run_id,
                  last_message_ordinal, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(conversation_id, knowledge_base_id) DO UPDATE SET
                    objective=excluded.objective, constraints_json=excluded.constraints_json,
                    unresolved_questions_json=excluded.unresolved_questions_json,
                    entity_ids_json=excluded.entity_ids_json, revision=excluded.revision,
                    last_run_id=excluded.last_run_id, last_message_ordinal=excluded.last_message_ordinal,
                    updated_at=excluded.updated_at",
                params![conversation_id, base_id, normalized.objective, constraints, questions,
                        entities, normalized.revision, run_id, ordinal, Utc::now().to_rfc3339()],
            )?;
            Ok(normalized)
        })
    }
}

fn check_scope(conn: &Connection, base_id: &str, conversation_id: &str) -> Result<(), BrainError> {
    let present: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_conversation_scopes
         WHERE conversation_id=?1 AND knowledge_base_id=?2)",
        params![conversation_id, base_id],
        |row| row.get(0),
    )?;
    if present {
        Ok(())
    } else {
        Err(validation("会话不存在或不属于当前知识库"))
    }
}

fn entity_is_current(conn: &Connection, base_id: &str, id: &str) -> Result<bool, BrainError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_entries
         WHERE id=?1 AND knowledge_base_id=?2 AND status IN ('draft','verified'))",
        params![id, base_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn load_memory(
    conn: &Connection,
    base_id: &str,
    conversation_id: &str,
) -> Result<Option<ConversationMemory>, BrainError> {
    let raw = conn
        .query_row(
            "SELECT objective, constraints_json, unresolved_questions_json, entity_ids_json,
                revision, last_run_id FROM knowledge_conversation_memories
         WHERE conversation_id=?1 AND knowledge_base_id=?2",
            params![conversation_id, base_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?;
    raw.map(
        |(objective, constraints, questions, entities, revision, last_run_id)| {
            let parse = |value: &str| {
                serde_json::from_str::<Vec<String>>(value)
                    .map_err(|error| BrainError::Internal(format!("会话记忆内容损坏: {error}")))
            };
            let memory = ConversationMemory {
                objective,
                constraints: parse(&constraints)?,
                unresolved_questions: parse(&questions)?,
                entity_ids: parse(&entities)?,
                revision,
                last_run_id,
            };
            normalize_content(&memory)
                .map_err(|error| BrainError::Internal(format!("会话记忆内容损坏: {error}")))?;
            Ok(memory)
        },
    )
    .transpose()
}

fn normalize_content(memory: &ConversationMemory) -> Result<ConversationMemory, BrainError> {
    let objective = memory.objective.trim();
    if objective.chars().count() > 800 || objective.contains('\0') {
        return Err(validation("会话目标超过 800 字符或包含非法字符"));
    }
    let normalize_list = |values: &[String], count_limit, length_limit, label: &str| {
        if values.len() > count_limit {
            return Err(validation(format!("{label}最多 {count_limit} 项")));
        }
        let mut seen = HashSet::new();
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            let value = value.trim();
            if value.is_empty() || value.chars().count() > length_limit || value.contains('\0') {
                return Err(validation(format!(
                    "{label}须为 1–{length_limit} 字符的有效文本"
                )));
            }
            if seen.insert(value.to_string()) {
                result.push(value.to_string());
            }
        }
        Ok(result)
    };
    Ok(ConversationMemory {
        objective: objective.to_string(),
        constraints: normalize_list(&memory.constraints, 16, 300, "用户约束")?,
        unresolved_questions: normalize_list(&memory.unresolved_questions, 12, 400, "未解决问题")?,
        entity_ids: normalize_list(&memory.entity_ids, 24, 128, "关注实体 ID")?,
        revision: memory.revision,
        last_run_id: memory.last_run_id.clone(),
    })
}

fn serialize_strings(values: &[String]) -> Result<String, BrainError> {
    serde_json::to_string(values)
        .map_err(|error| BrainError::Internal(format!("会话记忆序列化失败: {error}")))
}

fn validation(message: impl Into<String>) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::book_wiki_store::BookWikiStore;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::models::book_wiki::{BookKind, ReaderBook};
    use rusqlite::params;
    use serde_json::json;
    use std::sync::Arc;

    fn fixture() -> (BookWikiStore, tempfile::TempDir, String, String, String) {
        let directory = tempfile::tempdir().unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&directory.path().join("conversation-memory.db")).unwrap(),
        ));
        store
            .save_reader_books(&[ReaderBook {
                id: "memory-book".into(),
                path: "/tmp/memory-book".into(),
                kind: BookKind::Folder,
                name: "会话测试书".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("memory-book").unwrap();
        store.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_entries (id, knowledge_base_id, entry_type, slug, title,
                 summary, content_md, status, revision, created_at, updated_at)
                 VALUES ('memory-entity', ?1, 'concept', 'memory-concept', '关注概念',
                         '导航摘要', '可验证内容', 'verified', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                [&base.id],
            )?;
            Ok(())
        }).unwrap();
        let run_id = completed_qa(&store, &base.id);
        let conversation_id = store
            .save_conversation_exchange(
                &base.id,
                None,
                "比较机制，关注性能限制",
                "旧回答正文不应自动成为记忆或事实证据",
                &run_id,
                &[],
            )
            .unwrap();
        (store, directory, base.id, conversation_id, run_id)
    }

    fn completed_qa(store: &BookWikiStore, base_id: &str) -> String {
        let run = store
            .start_agent_run(base_id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        store
            .complete_agent_run(&run.id, &json!({"answer":"完整回答"}))
            .unwrap();
        run.id
    }

    fn memory() -> ConversationMemory {
        ConversationMemory {
            objective: "比较机制".into(),
            constraints: vec!["只依据当前书籍，中文说明".into()],
            unresolved_questions: vec!["性能差异的适用条件是什么？".into()],
            entity_ids: vec!["memory-entity".into()],
            ..ConversationMemory::default()
        }
    }

    #[test]
    fn test_get_legacy_conversation_memory_is_empty_not_previous_answer() {
        let (store, _directory, base_id, conversation_id, _run_id) = fixture();
        let loaded = store
            .get_conversation_memory(&base_id, &conversation_id)
            .unwrap();
        assert_eq!(loaded, ConversationMemory::default());
        assert_eq!(
            store
                .get_conversation(&conversation_id)
                .unwrap()
                .messages
                .len(),
            2
        );
        assert_eq!(store.list_reader_books().unwrap().len(), 1);
    }

    #[test]
    fn test_save_conversation_memory_round_trips_and_preserves_old_messages() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        let saved = store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &memory())
            .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.last_run_id.as_deref(), Some(run_id.as_str()));
        assert_eq!(saved.objective, "比较机制");
        assert_eq!(
            store
                .get_conversation_memory(&base_id, &conversation_id)
                .unwrap(),
            saved
        );
        assert_eq!(
            store
                .get_conversation(&conversation_id)
                .unwrap()
                .messages
                .len(),
            2
        );
    }

    #[test]
    fn test_conversation_memory_rejects_cross_book_and_foreign_entities() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        assert!(store
            .get_conversation_memory("foreign-base", &conversation_id)
            .is_err());
        assert!(store
            .save_conversation_memory("foreign-base", &conversation_id, &run_id, 0, &memory())
            .is_err());
        let mut update = memory();
        update.entity_ids = vec!["unknown-or-foreign-entity".into()];
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .is_err());
        assert_eq!(
            store
                .get_conversation_memory(&base_id, &conversation_id)
                .unwrap()
                .revision,
            0
        );
    }

    #[test]
    fn test_conversation_memory_rejects_real_foreign_book_run_and_entity() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        let mut books = store.list_reader_books().unwrap();
        books.push(ReaderBook {
            id: "foreign-memory-book".into(),
            path: "/tmp/foreign-memory-book".into(),
            kind: BookKind::Folder,
            name: "另一本书".into(),
            description: String::new(),
            category: String::new(),
            added_at: 2,
            progress: None,
        });
        store.save_reader_books(&books).unwrap();
        let foreign = store.initialize_base("foreign-memory-book").unwrap();
        let foreign_run = completed_qa(&store, &foreign.id);
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &foreign_run, 0, &memory())
            .is_err());
        store.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_entries (id, knowledge_base_id, entry_type, slug, title,
                 status, created_at, updated_at)
                 VALUES ('foreign-memory-entity', ?1, 'concept', 'foreign-concept', '另一本书的概念',
                         'verified', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                [&foreign.id],
            )?;
            Ok(())
        }).unwrap();
        let update = ConversationMemory {
            entity_ids: vec!["foreign-memory-entity".into()],
            ..memory()
        };
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .is_err());
    }

    #[test]
    fn test_conversation_memory_concurrent_revision_zero_writers_only_one_succeeds() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                let base_id = base_id.clone();
                let conversation_id = conversation_id.clone();
                let run_id = run_id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store.save_conversation_memory(
                        &base_id,
                        &conversation_id,
                        &run_id,
                        0,
                        &memory(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(Result::is_ok)
            .count();
        assert_eq!(successes, 1);
        assert_eq!(
            store
                .get_conversation_memory(&base_id, &conversation_id)
                .unwrap()
                .revision,
            1
        );
    }

    #[test]
    fn test_memory_normalizes_content_and_owns_revision_and_run_metadata() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        let update = ConversationMemory {
            objective: " 比较机制 ".into(),
            constraints: vec![" 只依据当前书籍 ".into(), "只依据当前书籍".into()],
            revision: 999,
            last_run_id: Some("untrusted-model-run-id".into()),
            ..ConversationMemory::default()
        };
        let saved = store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .unwrap();
        assert_eq!(saved.objective, "比较机制");
        assert_eq!(saved.constraints, vec!["只依据当前书籍"]);
        assert_eq!(saved.revision, 1);
        assert_eq!(saved.last_run_id.as_deref(), Some(run_id.as_str()));
        assert!(serde_json::from_value::<ConversationMemory>(
            json!({"previous_answer":"不允许增加事实字段"})
        )
        .is_err());
    }

    #[test]
    fn test_memory_migration_preserves_existing_books_and_conversation() {
        let (store, _directory, base_id, conversation_id, _run_id) = fixture();
        let before_books = serde_json::to_value(store.list_reader_books().unwrap()).unwrap();
        let before_conversation =
            serde_json::to_value(store.get_conversation(&conversation_id).unwrap()).unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute_batch("DROP TABLE knowledge_conversation_memories;")?;
                conn.execute_batch(include_str!(
                    "../../../migrations/044_conversation_memory.sql"
                ))?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(store.list_reader_books().unwrap()).unwrap(),
            before_books
        );
        assert_eq!(
            serde_json::to_value(store.get_conversation(&conversation_id).unwrap()).unwrap(),
            before_conversation
        );
        assert_eq!(
            store
                .get_conversation_memory(&base_id, &conversation_id)
                .unwrap(),
            ConversationMemory::default()
        );
    }

    #[test]
    fn test_conversation_memory_rejects_unfinished_non_qa_and_unlinked_runs() {
        let (store, _directory, base_id, conversation_id, _run_id) = fixture();
        let running = store
            .start_agent_run(&base_id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &running.id, 0, &memory())
            .is_err());
        let research = store
            .start_agent_run(
                &base_id,
                "deepseek_harness",
                "knowledge_task_research",
                &json!({}),
            )
            .unwrap();
        store
            .complete_agent_run(&research.id, &json!({"answer":"报告"}))
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &research.id, 0, &memory())
            .is_err());
        let unlinked = completed_qa(&store, &base_id);
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &unlinked, 0, &memory())
            .is_err());
    }

    #[test]
    fn test_memory_rejects_incomplete_output_and_a_new_pending_user_message() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE agent_runs SET output_json=?2 WHERE id=?1",
                    params![
                        run_id,
                        json!({"partial_answer":"不完整", "complete":false}).to_string()
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &memory())
            .is_err());
        store.db.with_connection(|conn| {
            conn.execute("UPDATE agent_runs SET output_json=?2 WHERE id=?1",
                params![run_id, json!({"answer":"完整"}).to_string()])?;
            conn.execute("INSERT INTO knowledge_messages (id, conversation_id, ordinal, role, content, created_at)
                VALUES ('pending-user', ?1, 2, 'user', '新的请求', CURRENT_TIMESTAMP)", [&conversation_id])?;
            Ok(())
        }).unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &memory())
            .is_err());
    }

    #[test]
    fn test_conversation_memory_optimistic_lock_and_late_answer_protection() {
        let (store, _directory, base_id, conversation_id, first_run) = fixture();
        store
            .save_conversation_memory(&base_id, &conversation_id, &first_run, 0, &memory())
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &first_run, 0, &memory())
            .is_err());
        let second_run = completed_qa(&store, &base_id);
        store
            .save_conversation_exchange(
                &base_id,
                Some(&conversation_id),
                "改为分析部署",
                "新回答",
                &second_run,
                &[],
            )
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &first_run, 1, &memory())
            .is_err());
        let reset = ConversationMemory {
            objective: "分析部署".into(),
            ..ConversationMemory::default()
        };
        let latest = store
            .save_conversation_memory(&base_id, &conversation_id, &second_run, 1, &reset)
            .unwrap();
        assert_eq!(latest.revision, 2);
        assert!(latest.constraints.is_empty());
        assert!(latest.entity_ids.is_empty());
        // A late previous Run may append another message but may not regress memory.
        store
            .save_conversation_exchange(
                &base_id,
                Some(&conversation_id),
                "旧请求迟到",
                "迟到旧答",
                &first_run,
                &[],
            )
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &first_run, 2, &memory())
            .is_err());
        assert_eq!(
            store
                .get_conversation_memory(&base_id, &conversation_id)
                .unwrap(),
            latest
        );
    }

    #[test]
    fn test_conversation_memory_validates_limits_and_entity_status() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        let mut update = memory();
        update.objective = "大".repeat(801);
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .is_err());
        update = memory();
        update.constraints = (0..17).map(|index| format!("约束{index}")).collect();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .is_err());
        update = memory();
        update.unresolved_questions = vec!["问".repeat(401)];
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &update)
            .is_err());
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET status='stale' WHERE id=?1",
                    params!["memory-entity"],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &memory())
            .is_err());
    }

    #[test]
    fn test_get_memory_drops_entities_archived_after_save_without_mutating_revision() {
        let (store, _directory, base_id, conversation_id, run_id) = fixture();
        store
            .save_conversation_memory(&base_id, &conversation_id, &run_id, 0, &memory())
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET status='archived' WHERE id='memory-entity'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let loaded = store
            .get_conversation_memory(&base_id, &conversation_id)
            .unwrap();
        assert_eq!(loaded.revision, 1);
        assert!(loaded.entity_ids.is_empty());
        assert_eq!(loaded.objective, "比较机制");
    }
}
