use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::book_wiki::{
    AgentRun, AgentRunEvent, AgentRunInspection, AgentRunInspectionSnapshot, AgentTokenUsage,
    AgentUsageCaller, AgentUsagePoint, AgentUsageStats, AgentUsageTotals, BookKind,
    BookKnowledgeCard, ConfigDocument, KnowledgeArtifact, KnowledgeBaseSummary,
    KnowledgeBridgeEntry, KnowledgeChange, KnowledgeChangeSet, KnowledgeCitation,
    KnowledgeClaimSummary, KnowledgeConversationDetail, KnowledgeConversationSummary,
    KnowledgeEntryDetail, KnowledgeEntryPage, KnowledgeEntrySummary, KnowledgeEntryVersionSummary,
    KnowledgeGraphOverview, KnowledgeGraphPath, KnowledgeGraphRelation, KnowledgeGraphSnapshot,
    KnowledgeHealthIssue, KnowledgeHealthReport, KnowledgeMessage, KnowledgeRelationSummary,
    KnowledgeTask, ModelProviderProfile, ReaderBook, RuntimeProfile, SourceDocumentSummary,
    SourceSpanSnapshot, WikiSkill, WikiSkillBenchmarkCase, WikiSkillBenchmarkCaseResult,
    WikiSkillBenchmarkRun, WikiSkillDetail, WikiSkillEvaluationFinding, WikiSkillEvaluationRun,
    WikiSkillFile, WikiSkillOrigin, WikiSkillVersion,
};

const LEGACY_BOOKS_KEY: &str = "reader_books";
const BOOKS_MIGRATED_KEY: &str = "reader_books_table_migrated";
const KNOWLEDGE_FTS_CONTENT_VERSION_KEY: &str = "knowledge_fts_content_v2";

#[derive(Clone)]
pub struct BookWikiStore {
    db: Arc<SqliteStore>,
}

pub struct KnowledgeEntryEditProposal<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub content_md: &'a str,
    pub aliases: &'a [String],
    pub status: &'a str,
    pub expected_revision: i64,
}

pub struct KnowledgeEntryRevision {
    pub entry_id: String,
    pub expected_revision: i64,
}

pub struct KnowledgeEntryMergeProposal<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub content_md: &'a str,
    pub aliases: &'a [String],
    pub status: &'a str,
    pub expected_revision: i64,
    pub sources: &'a [KnowledgeEntryRevision],
}

pub struct KnowledgeEntrySplitPart {
    pub title: String,
    pub summary: String,
    pub content_md: String,
    pub aliases: Vec<String>,
}

pub struct KnowledgeEntrySplitProposal<'a> {
    pub expected_revision: i64,
    pub parts: &'a [KnowledgeEntrySplitPart],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedAgentCapability {
    pub token: String,
    pub run_id: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCapabilityGrant {
    pub run_id: String,
    pub knowledge_base_ids: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub expires_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalResearchAuthorization {
    pub run_id: String,
    pub knowledge_base_id: String,
    pub host: String,
    pub request_number: i64,
    pub request_limit: i64,
}

pub struct WikiSkillBenchmarkCompletion<'a> {
    pub baseline_agent_run_id: &'a str,
    pub candidate_agent_run_id: &'a str,
    pub baseline_score: f64,
    pub candidate_score: f64,
    pub metrics: &'a serde_json::Value,
    pub results: &'a [WikiSkillBenchmarkCaseResult],
}

#[derive(Clone, Debug)]
pub struct SourceSectionDraft {
    pub id: String,
    pub entry_id: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub content_md: String,
    pub line_start: i64,
    pub line_end: i64,
    pub content_hash: String,
}

#[derive(Clone, Debug)]
pub struct MarkdownSourceDraft {
    pub id: String,
    pub version_id: String,
    pub original_path: String,
    pub relative_path: String,
    pub title: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub sections: Vec<SourceSectionDraft>,
}

impl BookWikiStore {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    fn ensure_knowledge_fts_current(&self) -> Result<(), BrainError> {
        if self
            .db
            .get_state(KNOWLEDGE_FTS_CONTENT_VERSION_KEY)?
            .is_some()
        {
            return Ok(());
        }

        self.db.transaction(|conn| {
            let already_rebuilt = conn
                .query_row(
                    "SELECT value FROM app_state WHERE key = ?1",
                    params![KNOWLEDGE_FTS_CONTENT_VERSION_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if already_rebuilt.is_some() {
                return Ok(());
            }

            let entries = {
                let mut stmt = conn.prepare(
                    "SELECT id, knowledge_base_id, title, aliases_json, summary, content_md
                     FROM knowledge_entries",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            conn.execute("DELETE FROM knowledge_entries_fts", [])?;
            for (entry_id, base_id, title, aliases, summary, content_md) in entries {
                let cjk_terms = knowledge_fts_cjk_terms(&[
                    title.as_str(),
                    aliases.as_str(),
                    summary.as_str(),
                    content_md.as_str(),
                ]);
                conn.execute(
                    "INSERT INTO knowledge_entries_fts
                     (entry_id, knowledge_base_id, title, aliases, summary, content_md, tags,
                      cjk_terms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7)",
                    params![entry_id, base_id, title, aliases, summary, content_md, cjk_terms],
                )?;
            }

            let spans = {
                let mut stmt = conn.prepare(
                    "SELECT ss.id, ss.knowledge_base_id, sd.title, ss.heading, ss.content
                     FROM source_spans ss
                     JOIN source_versions sv ON sv.id = ss.source_version_id
                     JOIN source_documents sd ON sd.id = sv.source_document_id
                     WHERE sd.sync_status = 'current'
                       AND sd.current_version_id = ss.source_version_id",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            conn.execute("DELETE FROM source_spans_fts", [])?;
            for (span_id, base_id, source_title, heading, content) in spans {
                let heading_text = heading.as_deref().unwrap_or_default();
                let cjk_terms = knowledge_fts_cjk_terms(&[
                    source_title.as_str(),
                    heading_text,
                    content.as_str(),
                ]);
                conn.execute(
                    "INSERT INTO source_spans_fts
                     (span_id, knowledge_base_id, source_title, heading, content, cjk_terms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![span_id, base_id, source_title, heading, content, cjk_terms],
                )?;
            }
            conn.execute(
                "INSERT INTO app_state (key, value, updated_at)
                 VALUES (?1, '1', CURRENT_TIMESTAMP)
                 ON CONFLICT(key) DO UPDATE SET value = '1', updated_at = CURRENT_TIMESTAMP",
                params![KNOWLEDGE_FTS_CONTENT_VERSION_KEY],
            )?;
            Ok(())
        })
    }

    /// Force derived search indexes to be rebuilt after a database restore.
    pub fn rebuild_knowledge_search_indexes(&self) -> Result<(), BrainError> {
        self.db.with_connection(|conn| {
            conn.execute(
                "DELETE FROM app_state WHERE key = ?1",
                params![KNOWLEDGE_FTS_CONTENT_VERSION_KEY],
            )?;
            Ok(())
        })?;
        self.ensure_knowledge_fts_current()
    }

    pub fn ensure_reader_books_migrated(&self) -> Result<(), BrainError> {
        if self.db.get_state(BOOKS_MIGRATED_KEY)?.is_some() {
            return Ok(());
        }

        let legacy_books = self
            .db
            .get_state(LEGACY_BOOKS_KEY)?
            .and_then(|raw| serde_json::from_str::<Vec<ReaderBook>>(&raw).ok())
            .unwrap_or_default();

        self.db.transaction(|conn| {
            let already_migrated = conn
                .query_row(
                    "SELECT value FROM app_state WHERE key = ?1",
                    params![BOOKS_MIGRATED_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if already_migrated.is_some() {
                return Ok(());
            }

            for book in &legacy_books {
                let progress_json = book
                    .progress
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|error| {
                        BrainError::Internal(format!("阅读进度序列化失败: {error}"))
                    })?;
                conn.execute(
                    "INSERT OR IGNORE INTO reader_books
                     (id, path, kind, name, description, category, added_at, progress_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        book.id,
                        book.path,
                        book.kind.as_str(),
                        book.name,
                        book.description,
                        book.category,
                        book.added_at,
                        progress_json,
                    ],
                )?;
            }

            if let Some(raw) = conn
                .query_row(
                    "SELECT value FROM app_state WHERE key = ?1",
                    params![LEGACY_BOOKS_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                conn.execute(
                    "INSERT OR IGNORE INTO app_state (key, value, updated_at)
                     VALUES ('reader_books_legacy_backup', ?1, CURRENT_TIMESTAMP)",
                    params![raw],
                )?;
            }
            conn.execute(
                "INSERT INTO app_state (key, value, updated_at)
                 VALUES (?1, '1', CURRENT_TIMESTAMP)
                 ON CONFLICT(key) DO UPDATE SET value = '1', updated_at = CURRENT_TIMESTAMP",
                params![BOOKS_MIGRATED_KEY],
            )?;
            Ok(())
        })
    }

    pub fn list_reader_books(&self) -> Result<Vec<ReaderBook>, BrainError> {
        self.ensure_reader_books_migrated()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, kind, name, description, category, added_at, progress_json
                 FROM reader_books
                 WHERE shelf_state = 'active'
                 ORDER BY added_at, name COLLATE NOCASE",
            )?;
            let rows = stmt.query_map([], |row| {
                let kind: String = row.get(2)?;
                let progress_json: Option<String> = row.get(7)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    kind,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    progress_json,
                ))
            })?;

            rows.map(|row| {
                let (id, path, kind, name, description, category, added_at, progress_json) = row?;
                let kind =
                    BookKind::try_from(kind.as_str()).map_err(BrainError::KnowledgeValidation)?;
                let progress = progress_json
                    .map(|raw| serde_json::from_str(&raw))
                    .transpose()
                    .map_err(|error| BrainError::Internal(format!("阅读进度解析失败: {error}")))?;
                Ok(ReaderBook {
                    id,
                    path,
                    kind,
                    name,
                    description,
                    category,
                    added_at,
                    progress,
                })
            })
            .collect()
        })
    }

    pub fn save_reader_books(&self, books: &[ReaderBook]) -> Result<usize, BrainError> {
        self.ensure_reader_books_migrated()?;
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            conn.execute(
                "UPDATE reader_books
                 SET shelf_state = 'removed', removed_at = ?1, revision = revision + 1,
                     updated_at = ?1
                 WHERE shelf_state = 'active'",
                params![now],
            )?;

            for book in books {
                if book.id.trim().is_empty()
                    || book.path.trim().is_empty()
                    || book.name.trim().is_empty()
                {
                    return Err(BrainError::KnowledgeValidation(
                        "书籍 id、路径和名称不能为空".to_string(),
                    ));
                }
                let replaced_id = conn
                    .query_row(
                        "SELECT id FROM reader_books WHERE path = ?1 AND id != ?2",
                        params![book.path, book.id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if let Some(existing_id) = &replaced_id {
                    let retired_path = format!("{}#replaced:{existing_id}", book.path);
                    conn.execute(
                        "UPDATE reader_books SET path = ?2 WHERE id = ?1",
                        params![existing_id, retired_path],
                    )?;
                }
                let progress_json = book
                    .progress
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|error| {
                        BrainError::Internal(format!("阅读进度序列化失败: {error}"))
                    })?;
                conn.execute(
                    "INSERT INTO reader_books
                     (id, path, kind, name, description, category, added_at, progress_json,
                      shelf_state, removed_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'active', NULL, ?9, ?9)
                     ON CONFLICT(id) DO UPDATE SET
                        path = excluded.path,
                        kind = excluded.kind,
                        name = excluded.name,
                        description = excluded.description,
                        category = excluded.category,
                        added_at = excluded.added_at,
                        progress_json = excluded.progress_json,
                        shelf_state = 'active',
                        removed_at = NULL,
                        revision = reader_books.revision + 1,
                        updated_at = excluded.updated_at",
                    params![
                        book.id,
                        book.path,
                        book.kind.as_str(),
                        book.name,
                        book.description,
                        book.category,
                        book.added_at,
                        progress_json,
                        now,
                    ],
                )?;
                if let Some(existing_id) = replaced_id {
                    conn.execute(
                        "UPDATE knowledge_bases SET book_id = ?2, revision = revision + 1,
                         updated_at = ?3 WHERE book_id = ?1",
                        params![existing_id, book.id, now],
                    )?;
                    conn.execute(
                        "DELETE FROM reader_books WHERE id = ?1",
                        params![existing_id],
                    )?;
                }
            }
            Ok(books.len())
        })
    }

    pub fn list_book_cards(&self) -> Result<Vec<BookKnowledgeCard>, BrainError> {
        let books = self.list_reader_books()?;
        books
            .into_iter()
            .filter(|book| book.kind == BookKind::Folder)
            .map(|book| {
                let knowledge_base = self.get_base_by_book_id(&book.id)?;
                Ok(BookKnowledgeCard {
                    book,
                    knowledge_base,
                })
            })
            .collect()
    }

    pub fn initialize_base(&self, book_id: &str) -> Result<KnowledgeBaseSummary, BrainError> {
        self.ensure_reader_books_migrated()?;
        if let Some(existing) = self.get_base_by_book_id(book_id)? {
            return Ok(existing);
        }
        let book_kind = self.db.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT kind FROM reader_books WHERE id = ?1 AND shelf_state = 'active'",
                    params![book_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?)
        })?;
        let book_kind =
            book_kind.ok_or_else(|| BrainError::KnowledgeNotFound(book_id.to_string()))?;
        if book_kind != "folder" {
            return Err(BrainError::KnowledgeValidation(
                "书籍知识库仅支持 Markdown 文件夹；PDF 仍可在阅境轩中阅读".to_string(),
            ));
        }

        let base_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO knowledge_bases
                 (id, book_id, lifecycle, sync_state, health_state, created_at, updated_at)
                 VALUES (?1, ?2, 'active', 'outdated', 'healthy', ?3, ?3)",
                params![base_id, book_id, now],
            )?;

            for (name, content) in default_book_config_documents() {
                conn.execute(
                    "INSERT INTO knowledge_config_documents
                     (id, knowledge_base_id, scope, name, content_md, created_at, updated_at)
                     VALUES (?1, ?2, 'book', ?3, ?4, ?5, ?5)",
                    params![
                        uuid::Uuid::new_v4().to_string(),
                        base_id,
                        name,
                        content,
                        now,
                    ],
                )?;
            }
            Ok(())
        })?;
        self.get_base(&base_id)
    }

    pub fn get_base(&self, base_id: &str) -> Result<KnowledgeBaseSummary, BrainError> {
        self.query_base("kb.id = ?1", base_id)?
            .ok_or_else(|| BrainError::KnowledgeNotFound(base_id.to_string()))
    }

    pub fn get_active_base(&self, base_id: &str) -> Result<KnowledgeBaseSummary, BrainError> {
        let base = self.get_base(base_id)?;
        if base.lifecycle != "active" {
            return Err(BrainError::KnowledgeValidation(
                match base.lifecycle.as_str() {
                    "paused" => "知识库已暂停；恢复后才能同步、问答或执行任务".to_string(),
                    "archived" => "知识库已归档；恢复后才能同步、问答或执行任务".to_string(),
                    _ => "知识库当前不可执行操作".to_string(),
                },
            ));
        }
        Ok(base)
    }

    pub fn get_syncable_base(&self, base_id: &str) -> Result<KnowledgeBaseSummary, BrainError> {
        let base = self.get_active_base(base_id)?;
        if !base.source_available {
            return Err(BrainError::KnowledgeValidation(
                "原书目录已失效；历史知识仍可浏览，请先在阅境轩重新添加正确目录".to_string(),
            ));
        }
        Ok(base)
    }

    pub fn set_base_lifecycle(
        &self,
        base_id: &str,
        lifecycle: &str,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        if !matches!(lifecycle, "active" | "paused" | "archived") {
            return Err(BrainError::KnowledgeValidation(
                "知识库生命周期只能是 active、paused 或 archived".to_string(),
            ));
        }
        let base = self.get_base(base_id)?;
        if base.lifecycle == lifecycle {
            return Ok(base);
        }
        if lifecycle != "active" {
            if base.compile_state == "compiling" {
                return Err(BrainError::KnowledgeValidation(
                    "智能编译仍在执行或等待审核，请先完成或停止编译".to_string(),
                ));
            }
            let active_tasks = self.db.with_connection(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM knowledge_tasks
                     WHERE knowledge_base_id = ?1 AND status IN ('queued', 'running')",
                    params![base_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })?;
            if active_tasks > 0 {
                return Err(BrainError::KnowledgeValidation(
                    "知识库仍有排队或运行中的任务，请先取消任务再暂停或归档".to_string(),
                ));
            }
        }
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_bases
                 SET lifecycle = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![base_id, lifecycle, now],
            )?;
            Ok(())
        })?;
        self.get_base(base_id)
    }

    pub fn delete_base(&self, base_id: &str, confirmation: &str) -> Result<(), BrainError> {
        let base = self.get_base(base_id)?;
        if confirmation != base.book_name && confirmation != base.id {
            return Err(BrainError::KnowledgeValidation(
                "删除确认内容必须与书名或知识库 ID 完全一致".to_string(),
            ));
        }
        if base.compile_state == "compiling" {
            return Err(BrainError::KnowledgeValidation(
                "智能编译仍在执行或等待审核，不能删除知识库".to_string(),
            ));
        }
        let active_tasks = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM knowledge_tasks
                 WHERE knowledge_base_id = ?1 AND status IN ('queued', 'running')",
                params![base_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(Into::into)
        })?;
        if active_tasks > 0 {
            return Err(BrainError::KnowledgeValidation(
                "知识库仍有排队或运行中的任务，不能删除".to_string(),
            ));
        }
        self.db
            .create_managed_backup("before-knowledge-base-delete", self.db.backup_retention())?;
        self.db.transaction(|conn| {
            conn.execute(
                "DELETE FROM knowledge_entries_fts WHERE knowledge_base_id = ?1",
                params![base_id],
            )?;
            conn.execute(
                "DELETE FROM source_spans_fts WHERE knowledge_base_id = ?1",
                params![base_id],
            )?;
            let deleted = conn.execute(
                "DELETE FROM knowledge_bases WHERE id = ?1",
                params![base_id],
            )?;
            if deleted == 0 {
                return Err(BrainError::KnowledgeNotFound(base_id.to_string()));
            }
            Ok(())
        })
    }

    pub fn get_base_by_book_id(
        &self,
        book_id: &str,
    ) -> Result<Option<KnowledgeBaseSummary>, BrainError> {
        self.query_base("kb.book_id = ?1", book_id)
    }

    fn query_base(
        &self,
        predicate: &str,
        value: &str,
    ) -> Result<Option<KnowledgeBaseSummary>, BrainError> {
        self.db.with_connection(|conn| {
            let sql = format!(
                "SELECT kb.id, kb.book_id, b.name, b.path, b.kind, b.description, b.category,
                        kb.lifecycle, kb.sync_state, kb.compile_mode, kb.compile_state,
                        kb.compile_error, kb.health_state, kb.last_error,
                        kb.last_synced_at, kb.last_scanned_at, kb.last_compiled_at,
                        kb.compile_processed_sources, kb.compile_total_sources,
                        kb.compile_phase, kb.compile_message,
                        kb.compile_current_batch, kb.compile_total_batches,
                        kb.compile_active_run_id, kb.compile_change_set_id,
                        kb.compile_started_at, kb.compile_heartbeat_at,
                        kb.compile_cancel_requested,
                        kb.pending_review_count,
                        (SELECT COUNT(*) FROM source_documents sd
                          WHERE sd.knowledge_base_id = kb.id AND sd.sync_status = 'current'),
                        (SELECT COUNT(*) FROM knowledge_entries ke
                          WHERE ke.knowledge_base_id = kb.id
                            AND ke.status NOT IN ('archived', 'stale')),
                        (SELECT COUNT(*) FROM knowledge_claims kc
                          WHERE kc.knowledge_base_id = kb.id),
                        (SELECT COUNT(*) FROM knowledge_tasks kt
                          WHERE kt.knowledge_base_id = kb.id AND kt.status != 'cancelled')
                 FROM knowledge_bases kb
                 JOIN reader_books b ON b.id = kb.book_id
                 WHERE {predicate}"
            );
            conn.query_row(&sql, params![value], map_base_summary)
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn set_sync_state(
        &self,
        base_id: &str,
        sync_state: &str,
        health_state: &str,
        last_error: Option<&str>,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            let updated = conn.execute(
                "UPDATE knowledge_bases
                 SET sync_state = ?2, health_state = ?3, last_error = ?4,
                     last_scanned_at = ?5, revision = revision + 1, updated_at = ?5
                 WHERE id = ?1",
                params![base_id, sync_state, health_state, last_error, now],
            )?;
            if updated == 0 {
                return Err(BrainError::KnowledgeNotFound(base_id.to_string()));
            }
            Ok(())
        })
    }

    pub fn sync_markdown_sources(
        &self,
        base_id: &str,
        sources: &[MarkdownSourceDraft],
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let existing_versions = {
                let mut stmt = conn.prepare(
                    "SELECT original_path, current_version_id
                     FROM source_documents
                     WHERE knowledge_base_id = ?1
                       AND source_type = 'markdown'
                       AND sync_status = 'current'",
                )?;
                let versions = stmt.query_map(params![base_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                })?
                .collect::<Result<BTreeMap<_, _>, _>>()?;
                versions
            };
            let incoming_versions = sources
                .iter()
                .map(|source| {
                    (
                        source.original_path.clone(),
                        Some(source.version_id.clone()),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let sources_changed = existing_versions != incoming_versions;

            conn.execute(
                "UPDATE source_documents SET sync_status = 'missing', updated_at = ?2
                 WHERE knowledge_base_id = ?1 AND source_type = 'markdown'",
                params![base_id, now],
            )?;
            conn.execute(
                "DELETE FROM knowledge_entries_fts
                 WHERE entry_id IN (
                    SELECT id FROM knowledge_entries
                    WHERE knowledge_base_id = ?1 AND entry_type = 'source_section'
                 )",
                params![base_id],
            )?;
            conn.execute(
                "DELETE FROM source_spans_fts WHERE knowledge_base_id = ?1",
                params![base_id],
            )?;

            for source in sources {
                conn.execute(
                    "INSERT INTO source_documents
                     (id, knowledge_base_id, source_type, original_path, relative_path, title,
                      mime_type, ordinal, current_version_id, sync_status, extraction_status,
                      created_at, updated_at)
                     VALUES (?1, ?2, 'markdown', ?3, ?4, ?5, 'text/markdown', ?6, ?7,
                             'current', 'ready', ?8, ?8)
                     ON CONFLICT(knowledge_base_id, original_path) DO UPDATE SET
                        relative_path = excluded.relative_path,
                        title = excluded.title,
                        ordinal = excluded.ordinal,
                        current_version_id = excluded.current_version_id,
                        sync_status = 'current',
                        extraction_status = 'ready',
                        updated_at = excluded.updated_at",
                    params![
                        source.id,
                        base_id,
                        source.original_path,
                        source.relative_path,
                        source.title,
                        source.ordinal,
                        source.version_id,
                        now,
                    ],
                )?;
                conn.execute(
                    "INSERT OR IGNORE INTO source_versions
                     (id, source_document_id, content_hash, size_bytes, modified_at,
                      extraction_version, extraction_status, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'markdown-v1', 'ready', ?6)",
                    params![
                        source.version_id,
                        source.id,
                        source.content_hash,
                        source.size_bytes,
                        source.modified_at,
                        now,
                    ],
                )?;
                for (ordinal, section) in source.sections.iter().enumerate() {
                    conn.execute(
                        "INSERT INTO source_spans
                         (id, knowledge_base_id, source_version_id, ordinal, heading, anchor,
                          line_start, line_end, content, content_hash, token_estimate)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                         ON CONFLICT(source_version_id, ordinal) DO UPDATE SET
                            heading = excluded.heading,
                            anchor = excluded.anchor,
                            line_start = excluded.line_start,
                            line_end = excluded.line_end,
                            content = excluded.content,
                            content_hash = excluded.content_hash,
                            token_estimate = excluded.token_estimate",
                        params![
                            section.id,
                            base_id,
                            source.version_id,
                            ordinal as i64,
                            section.title,
                            section.slug,
                            section.line_start,
                            section.line_end,
                            section.content_md,
                            section.content_hash,
                            (section.content_md.chars().count() / 3) as i64,
                        ],
                    )?;
                    let span_id = conn.query_row(
                        "SELECT id FROM source_spans
                         WHERE source_version_id = ?1 AND ordinal = ?2",
                        params![source.version_id, ordinal as i64],
                        |row| row.get::<_, String>(0),
                    )?;
                    conn.execute(
                        "INSERT INTO knowledge_entries
                         (id, knowledge_base_id, origin_document_id, entry_type, slug, title,
                          summary, content_md, status, confidence, created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'source_section', ?4, ?5, ?6, ?7,
                                 'verified', 1.0, ?8, ?8)
                         ON CONFLICT(knowledge_base_id, entry_type, slug) DO UPDATE SET
                            origin_document_id = excluded.origin_document_id,
                            title = excluded.title,
                            summary = excluded.summary,
                            content_md = excluded.content_md,
                            status = 'verified',
                            confidence = 1.0,
                            revision = CASE
                                WHEN knowledge_entries.title != excluded.title
                                  OR knowledge_entries.summary != excluded.summary
                                  OR knowledge_entries.content_md != excluded.content_md
                                THEN knowledge_entries.revision + 1
                                ELSE knowledge_entries.revision
                            END,
                            updated_at = CASE
                                WHEN knowledge_entries.title != excluded.title
                                  OR knowledge_entries.summary != excluded.summary
                                  OR knowledge_entries.content_md != excluded.content_md
                                  OR knowledge_entries.status != 'verified'
                                THEN excluded.updated_at
                                ELSE knowledge_entries.updated_at
                            END",
                        params![
                            section.entry_id,
                            base_id,
                            source.id,
                            section.slug,
                            section.title,
                            section.summary,
                            section.content_md,
                            now,
                        ],
                    )?;
                    let entry_id = conn.query_row(
                        "SELECT id FROM knowledge_entries
                         WHERE knowledge_base_id = ?1 AND entry_type = 'source_section' AND slug = ?2",
                        params![base_id, section.slug],
                        |row| row.get::<_, String>(0),
                    )?;
                    conn.execute(
                        "DELETE FROM knowledge_entries_fts WHERE entry_id = ?1",
                        params![entry_id],
                    )?;
                    conn.execute(
                        "INSERT INTO knowledge_entries_fts
                         (entry_id, knowledge_base_id, title, aliases, summary, content_md, tags,
                          cjk_terms)
                         VALUES (?1, ?2, ?3, '', ?4, ?5, '', ?6)",
                        params![
                            entry_id,
                            base_id,
                            section.title,
                            section.summary,
                            section.content_md,
                            knowledge_fts_cjk_terms(&[
                                section.title.as_str(),
                                section.summary.as_str(),
                                section.content_md.as_str(),
                            ]),
                        ],
                    )?;
                    conn.execute(
                        "INSERT INTO source_spans_fts
                         (span_id, knowledge_base_id, source_title, heading, content, cjk_terms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            span_id,
                            base_id,
                            source.title,
                            section.title,
                            section.content_md,
                            knowledge_fts_cjk_terms(&[
                                source.title.as_str(),
                                section.title.as_str(),
                                section.content_md.as_str(),
                            ]),
                        ],
                    )?;
                    conn.execute(
                        "INSERT OR IGNORE INTO knowledge_citations
                         (id, knowledge_base_id, entry_id, source_span_id, quote_text, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            format!("citation-{span_id}"),
                            base_id,
                            entry_id,
                            span_id,
                            section.summary,
                            now,
                        ],
                    )?;
                }
            }

            conn.execute(
                "UPDATE knowledge_entries AS ke
                 SET status = 'stale', updated_at = ?2
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.entry_type = 'source_section'
                   AND NOT EXISTS (
                       SELECT 1
                       FROM knowledge_citations kc
                       JOIN source_spans ss ON ss.id = kc.source_span_id
                       JOIN source_versions sv ON sv.id = ss.source_version_id
                       JOIN source_documents sd ON sd.id = sv.source_document_id
                       WHERE kc.entry_id = ke.id
                         AND sd.sync_status = 'current'
                         AND sd.current_version_id = ss.source_version_id
                   )",
                params![base_id, now],
            )?;

            let health = if sources.is_empty() {
                "warning"
            } else {
                "healthy"
            };
            conn.execute(
                "UPDATE knowledge_bases
                 SET sync_state = 'clean', health_state = ?2, last_error = NULL,
                     compile_state = CASE
                         WHEN ?4 = 1 AND compile_mode = 'smart' THEN 'outdated'
                         ELSE compile_state
                     END,
                     compile_error = CASE WHEN ?4 = 1 THEN NULL ELSE compile_error END,
                     last_synced_at = ?3, last_scanned_at = ?3,
                     revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![base_id, health, now, i64::from(sources_changed)],
            )?;
            Ok(())
        })
    }

    pub fn list_current_source_documents(
        &self,
        base_id: &str,
    ) -> Result<Vec<SourceDocumentSummary>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT sd.id, sd.knowledge_base_id, sd.relative_path, sd.title,
                        sd.current_version_id, sd.sync_status,
                        (SELECT COUNT(*) FROM source_spans ss
                         WHERE ss.source_version_id = sd.current_version_id),
                        sd.updated_at
                 FROM source_documents sd
                 WHERE sd.knowledge_base_id = ?1 AND sd.sync_status = 'current'
                   AND sd.current_version_id IS NOT NULL
                 ORDER BY sd.ordinal, sd.relative_path COLLATE NOCASE",
            )?;
            let rows = stmt.query_map(params![base_id], |row| {
                Ok(SourceDocumentSummary {
                    id: row.get(0)?,
                    knowledge_base_id: row.get(1)?,
                    relative_path: row.get(2)?,
                    title: row.get(3)?,
                    current_version_id: row.get(4)?,
                    sync_status: row.get(5)?,
                    span_count: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn get_current_source_span(
        &self,
        base_id: &str,
        span_id: &str,
    ) -> Result<SourceSpanSnapshot, BrainError> {
        self.get_base(base_id)?;
        self.db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT ss.id, sd.id, ss.source_version_id, sd.relative_path, ss.heading,
                            ss.line_start, ss.line_end, ss.content
                     FROM source_spans ss
                     JOIN source_versions sv ON sv.id = ss.source_version_id
                     JOIN source_documents sd ON sd.id = sv.source_document_id
                     WHERE ss.id = ?2 AND ss.knowledge_base_id = ?1
                       AND sd.knowledge_base_id = ?1 AND sd.sync_status = 'current'
                       AND sd.current_version_id = ss.source_version_id",
                    params![base_id, span_id],
                    |row| {
                        Ok(SourceSpanSnapshot {
                            id: row.get(0)?,
                            source_document_id: row.get(1)?,
                            source_version_id: row.get(2)?,
                            source_path: row.get(3)?,
                            heading: row.get(4)?,
                            line_start: row.get(5)?,
                            line_end: row.get(6)?,
                            content: row.get(7)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
            })?
            .ok_or_else(|| BrainError::KnowledgeNotFound(span_id.to_string()))
    }

    pub fn list_current_source_spans(
        &self,
        base_id: &str,
    ) -> Result<Vec<SourceSpanSnapshot>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT ss.id, sd.id, ss.source_version_id, sd.relative_path, ss.heading,
                        ss.line_start, ss.line_end, ss.content
                 FROM source_spans ss
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 WHERE ss.knowledge_base_id = ?1
                   AND sd.sync_status = 'current'
                   AND sd.current_version_id = ss.source_version_id
                 ORDER BY sd.ordinal, ss.ordinal",
            )?;
            let rows = stmt.query_map(params![base_id], |row| {
                Ok(SourceSpanSnapshot {
                    id: row.get(0)?,
                    source_document_id: row.get(1)?,
                    source_version_id: row.get(2)?,
                    source_path: row.get(3)?,
                    heading: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    content: row.get(7)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn list_source_spans_pending_compile(
        &self,
        base_id: &str,
        compile_fingerprint: &str,
    ) -> Result<Vec<SourceSpanSnapshot>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT ss.id, sd.id, ss.source_version_id, sd.relative_path, ss.heading,
                        ss.line_start, ss.line_end, ss.content
                 FROM source_spans ss
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 LEFT JOIN knowledge_compile_checkpoints checkpoint
                   ON checkpoint.knowledge_base_id = ss.knowledge_base_id
                  AND checkpoint.source_document_id = sd.id
                 WHERE ss.knowledge_base_id = ?1
                   AND sd.sync_status = 'current'
                   AND sd.current_version_id = ss.source_version_id
                   AND (checkpoint.source_version_id IS NULL
                        OR checkpoint.source_version_id != ss.source_version_id
                        OR checkpoint.compile_fingerprint != ?2)
                 ORDER BY sd.ordinal, ss.ordinal",
            )?;
            let rows = stmt.query_map(params![base_id, compile_fingerprint], |row| {
                Ok(SourceSpanSnapshot {
                    id: row.get(0)?,
                    source_document_id: row.get(1)?,
                    source_version_id: row.get(2)?,
                    source_path: row.get(3)?,
                    heading: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    content: row.get(7)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn search_source_spans(
        &self,
        base_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SourceSpanSnapshot>, BrainError> {
        self.get_base(base_id)?;
        self.ensure_knowledge_fts_current()?;
        let limit = limit.clamp(1, 200) as i64;
        let fts_query = knowledge_fts_query(Some(query));
        let patterns = if fts_query.is_none() {
            knowledge_query_patterns(Some(query))
        } else {
            Vec::new()
        };
        self.db.with_connection(|conn| {
            let mut spans = Vec::new();
            let mut seen = HashSet::new();
            if let Some(fts_query) = fts_query.as_deref() {
                let mut stmt = conn.prepare(
                    "SELECT ss.id, sd.id, ss.source_version_id, sd.relative_path, ss.heading,
                            ss.line_start, ss.line_end, ss.content
                     FROM source_spans_fts
                     JOIN source_spans ss ON ss.id = source_spans_fts.span_id
                     JOIN source_versions sv ON sv.id = ss.source_version_id
                     JOIN source_documents sd ON sd.id = sv.source_document_id
                     WHERE source_spans_fts MATCH ?2
                       AND source_spans_fts.knowledge_base_id = ?1
                       AND ss.knowledge_base_id = ?1
                       AND sd.sync_status = 'current'
                       AND sd.current_version_id = ss.source_version_id
                     ORDER BY bm25(source_spans_fts, 0.0, 0.0, 4.0, 12.0, 1.0, 2.0),
                              sd.ordinal, ss.ordinal
                     LIMIT ?3",
                )?;
                let rows = stmt.query_map(params![base_id, fts_query, limit], |row| {
                    Ok(SourceSpanSnapshot {
                        id: row.get(0)?,
                        source_document_id: row.get(1)?,
                        source_version_id: row.get(2)?,
                        source_path: row.get(3)?,
                        heading: row.get(4)?,
                        line_start: row.get(5)?,
                        line_end: row.get(6)?,
                        content: row.get(7)?,
                    })
                })?;
                for row in rows {
                    let span = row?;
                    if seen.insert(span.id.clone()) {
                        spans.push(span);
                    }
                }
            }

            let mut fallback_stmt = conn.prepare(
                "SELECT ss.id, sd.id, ss.source_version_id, sd.relative_path, ss.heading,
                        ss.line_start, ss.line_end, ss.content
                 FROM source_spans ss
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 WHERE ss.knowledge_base_id = ?1
                   AND sd.sync_status = 'current'
                   AND sd.current_version_id = ss.source_version_id
                   AND (sd.title LIKE ?2 OR ss.heading LIKE ?2 OR ss.content LIKE ?2)
                 ORDER BY CASE WHEN ss.heading LIKE ?2 THEN 0
                               WHEN sd.title LIKE ?2 THEN 1 ELSE 2 END,
                          sd.ordinal, ss.ordinal
                 LIMIT ?3",
            )?;
            for pattern in patterns {
                let remaining = limit - spans.len() as i64;
                if remaining <= 0 {
                    break;
                }
                let rows =
                    fallback_stmt.query_map(params![base_id, pattern, remaining], |row| {
                        Ok(SourceSpanSnapshot {
                            id: row.get(0)?,
                            source_document_id: row.get(1)?,
                            source_version_id: row.get(2)?,
                            source_path: row.get(3)?,
                            heading: row.get(4)?,
                            line_start: row.get(5)?,
                            line_end: row.get(6)?,
                            content: row.get(7)?,
                        })
                    })?;
                for row in rows {
                    let span = row?;
                    if seen.insert(span.id.clone()) {
                        spans.push(span);
                    }
                }
            }
            Ok(spans)
        })
    }

    pub fn record_compile_checkpoints(
        &self,
        base_id: &str,
        spans: &[SourceSpanSnapshot],
        change_set_id: &str,
        compile_fingerprint: &str,
    ) -> Result<(), BrainError> {
        let versions = spans
            .iter()
            .map(|span| {
                (
                    span.source_document_id.clone(),
                    span.source_version_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if versions.is_empty() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            for (source_document_id, source_version_id) in &versions {
                let is_current = conn.query_row(
                    "SELECT COUNT(*) = 1 FROM source_documents
                     WHERE id = ?1 AND knowledge_base_id = ?2
                       AND sync_status = 'current' AND current_version_id = ?3",
                    params![source_document_id, base_id, source_version_id],
                    |row| row.get::<_, bool>(0),
                )?;
                if !is_current {
                    return Err(BrainError::KnowledgeValidation(format!(
                        "来源 {source_document_id} 已变化，请重新运行智能编译"
                    )));
                }
                conn.execute(
                    "INSERT INTO knowledge_compile_checkpoints
                     (knowledge_base_id, source_document_id, source_version_id,
                      last_change_set_id, compiled_at, compile_fingerprint)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(knowledge_base_id, source_document_id) DO UPDATE SET
                        source_version_id = excluded.source_version_id,
                        last_change_set_id = excluded.last_change_set_id,
                        compiled_at = excluded.compiled_at,
                        compile_fingerprint = excluded.compile_fingerprint",
                    params![
                        base_id,
                        source_document_id,
                        source_version_id,
                        change_set_id,
                        now,
                        compile_fingerprint,
                    ],
                )?;
            }
            Ok(())
        })
    }

    pub fn list_semantic_entries(
        &self,
        base_id: &str,
        limit: usize,
    ) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
        let limit = limit.clamp(1, 500) as i64;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                        ke.summary, ke.status, ke.confidence, NULL, ke.updated_at
                 FROM knowledge_entries ke
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.entry_type <> 'source_section'
                   AND ke.status NOT IN ('archived', 'stale')
                 ORDER BY ke.updated_at DESC, ke.title COLLATE NOCASE
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![base_id, limit], |row| {
                Ok(KnowledgeEntrySummary {
                    id: row.get(0)?,
                    knowledge_base_id: row.get(1)?,
                    entry_type: row.get(2)?,
                    slug: row.get(3)?,
                    title: row.get(4)?,
                    summary: row.get(5)?,
                    status: row.get(6)?,
                    confidence: row.get(7)?,
                    source_path: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn set_compile_state(
        &self,
        base_id: &str,
        state: &str,
        processed_sources: i64,
        total_sources: i64,
        error: Option<&str>,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        if !matches!(
            state,
            "not_started" | "outdated" | "compiling" | "ready" | "failed"
        ) {
            return Err(BrainError::KnowledgeValidation(
                "未知的 Wiki 编译状态".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_bases
                 SET compile_mode = 'smart', compile_state = ?2, compile_error = ?3,
                     compile_processed_sources = ?4, compile_total_sources = ?5,
                     last_compiled_at = CASE WHEN ?2 IN ('ready', 'failed') THEN ?6
                                             ELSE last_compiled_at END,
                     revision = revision + 1, updated_at = ?6
                 WHERE id = ?1",
                params![base_id, state, error, processed_sources, total_sources, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeNotFound(base_id.to_string()));
        }
        self.get_base(base_id)
    }

    pub fn begin_semantic_compile(
        &self,
        base_id: &str,
        total_sources: i64,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_bases
                 SET compile_mode = 'smart', compile_state = 'compiling', compile_error = NULL,
                     compile_processed_sources = 0, compile_total_sources = ?2,
                     compile_phase = 'queued', compile_message = '智能编译已进入后台队列',
                     compile_current_batch = 0, compile_total_batches = 0,
                     compile_active_run_id = NULL, compile_change_set_id = NULL,
                     compile_started_at = ?3, compile_heartbeat_at = ?3,
                     compile_cancel_requested = 0,
                     revision = revision + 1, updated_at = ?3
                 WHERE id = ?1 AND compile_state != 'compiling'",
                params![base_id, total_sources, now],
            )?)
        })?;
        if updated == 0 {
            self.get_base(base_id)?;
            return Err(BrainError::KnowledgeValidation(
                "智能编译已经在后台执行，请勿重复启动".to_string(),
            ));
        }
        self.get_base(base_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_compile_activity(
        &self,
        base_id: &str,
        phase: &str,
        message: &str,
        current_batch: i64,
        total_batches: i64,
        active_run_id: Option<&str>,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_bases
                 SET compile_phase = ?2, compile_message = ?3,
                     compile_current_batch = ?4, compile_total_batches = ?5,
                     compile_active_run_id = ?6, compile_heartbeat_at = ?7,
                     updated_at = ?7
                 WHERE id = ?1 AND compile_state = 'compiling'",
                params![
                    base_id,
                    phase,
                    message,
                    current_batch,
                    total_batches,
                    active_run_id,
                    now
                ],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "智能编译已经结束或知识库不存在".to_string(),
            ));
        }
        Ok(())
    }

    pub fn heartbeat_semantic_compile(&self, base_id: &str) -> Result<bool, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_bases SET compile_heartbeat_at = ?2
                 WHERE id = ?1 AND compile_state = 'compiling'",
                params![base_id, now],
            )? == 1)
        })
    }

    pub fn request_semantic_compile_cancel(
        &self,
        base_id: &str,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_bases
                 SET compile_cancel_requested = 1, compile_phase = 'cancelling',
                     compile_message = '正在停止当前模型调用', updated_at = ?2
                 WHERE id = ?1 AND compile_state = 'compiling'
                   AND compile_phase != 'waiting_review'",
                params![base_id, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "当前没有可以取消的智能编译".to_string(),
            ));
        }
        self.get_base(base_id)
    }

    pub fn is_semantic_compile_cancel_requested(&self, base_id: &str) -> Result<bool, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT compile_cancel_requested FROM knowledge_bases WHERE id = ?1",
                params![base_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(base_id.to_string()))
        })
    }

    pub fn finish_semantic_compile_failure(
        &self,
        base_id: &str,
        error: &str,
        cancelled: bool,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_bases
                 SET compile_state = 'failed', compile_error = ?2,
                     compile_phase = ?3, compile_message = ?2,
                     compile_active_run_id = NULL, compile_cancel_requested = 0,
                     compile_heartbeat_at = ?4, last_compiled_at = ?4,
                     revision = revision + 1, updated_at = ?4
                 WHERE id = ?1",
                params![
                    base_id,
                    error,
                    if cancelled { "cancelled" } else { "failed" },
                    now
                ],
            )?;
            Ok(())
        })?;
        self.get_base(base_id)
    }

    pub fn mark_semantic_compile_waiting_review(
        &self,
        base_id: &str,
        change_set_id: &str,
        processed_sources: i64,
        total_sources: i64,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_bases
                 SET compile_state = 'compiling', compile_error = NULL,
                     compile_processed_sources = ?3, compile_total_sources = ?4,
                     compile_phase = 'waiting_review',
                     compile_message = '智能编译完成，等待审核知识变更',
                     compile_active_run_id = NULL, compile_change_set_id = ?2,
                     compile_cancel_requested = 0, compile_heartbeat_at = ?5,
                     revision = revision + 1, updated_at = ?5
                 WHERE id = ?1",
                params![
                    base_id,
                    change_set_id,
                    processed_sources,
                    total_sources,
                    now
                ],
            )?;
            Ok(())
        })?;
        self.get_base(base_id)
    }

    pub fn list_entries(
        &self,
        base_id: &str,
        query: Option<&str>,
        entry_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
        self.ensure_knowledge_fts_current()?;
        let limit = limit.clamp(1, 10_000) as i64;
        let fts_query = knowledge_fts_query(query);
        let patterns = if fts_query.is_none() {
            knowledge_query_patterns(query)
        } else {
            Vec::new()
        };
        let entry_type = entry_type.filter(|value| !value.trim().is_empty());
        self.db.with_connection(|conn| {
            let mut entries = Vec::new();
            let mut seen = HashSet::new();
            if let Some(fts_query) = fts_query.as_deref() {
                let mut stmt = conn.prepare(
                    "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                            ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
                     FROM knowledge_entries_fts
                     JOIN knowledge_entries ke ON ke.id = knowledge_entries_fts.entry_id
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE knowledge_entries_fts MATCH ?3
                       AND knowledge_entries_fts.knowledge_base_id = ?1
                       AND ke.knowledge_base_id = ?1
                       AND ke.status NOT IN ('archived', 'stale')
                       AND (?2 IS NULL OR ke.entry_type = ?2)
                     ORDER BY CASE WHEN ke.entry_type = 'source_section' THEN 1 ELSE 0 END,
                              bm25(knowledge_entries_fts, 0.0, 0.0, 12.0, 8.0, 4.0, 1.0,
                                   1.0, 2.0),
                              sd.ordinal, ke.title COLLATE NOCASE
                     LIMIT ?4",
                )?;
                let rows =
                    stmt.query_map(params![base_id, entry_type, fts_query, limit], |row| {
                        Ok(KnowledgeEntrySummary {
                            id: row.get(0)?,
                            knowledge_base_id: row.get(1)?,
                            entry_type: row.get(2)?,
                            slug: row.get(3)?,
                            title: row.get(4)?,
                            summary: row.get(5)?,
                            status: row.get(6)?,
                            confidence: row.get(7)?,
                            source_path: row.get(8)?,
                            updated_at: row.get(9)?,
                        })
                    })?;
                for row in rows {
                    let entry = row?;
                    if seen.insert(entry.id.clone()) {
                        entries.push(entry);
                    }
                }
            }

            let mut fallback_stmt = conn.prepare(
                "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                        ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
                 FROM knowledge_entries ke
                 LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.status NOT IN ('archived', 'stale')
                   AND (?2 IS NULL OR ke.entry_type = ?2)
                   AND (ke.title LIKE ?3 OR ke.summary LIKE ?3 OR ke.content_md LIKE ?3)
                 ORDER BY CASE WHEN ke.entry_type = 'source_section' THEN 1 ELSE 0 END,
                          sd.ordinal, ke.title COLLATE NOCASE
                 LIMIT ?4",
            )?;
            for pattern in patterns {
                let remaining = limit - entries.len() as i64;
                if remaining <= 0 {
                    break;
                }
                let rows = fallback_stmt.query_map(
                    params![base_id, entry_type, pattern, remaining],
                    |row| {
                        Ok(KnowledgeEntrySummary {
                            id: row.get(0)?,
                            knowledge_base_id: row.get(1)?,
                            entry_type: row.get(2)?,
                            slug: row.get(3)?,
                            title: row.get(4)?,
                            summary: row.get(5)?,
                            status: row.get(6)?,
                            confidence: row.get(7)?,
                            source_path: row.get(8)?,
                            updated_at: row.get(9)?,
                        })
                    },
                )?;
                for row in rows {
                    let entry = row?;
                    if seen.insert(entry.id.clone()) {
                        entries.push(entry);
                    }
                }
            }
            entries.sort_by_key(|entry| entry.entry_type == "source_section");
            Ok(entries)
        })
    }

    pub fn list_entries_page(
        &self,
        base_id: &str,
        query: Option<&str>,
        entry_type: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<KnowledgeEntryPage, BrainError> {
        self.get_base(base_id)?;
        self.ensure_knowledge_fts_current()?;
        let limit = limit.clamp(1, 100) as i64;
        let offset = offset.min(100_000) as i64;
        let entry_type = entry_type.filter(|value| !value.trim().is_empty());
        let fts_query = knowledge_fts_query(query);
        self.db.with_connection(|conn| {
            let (entries, total) = if let Some(fts_query) = fts_query {
                let total = conn.query_row(
                    "SELECT COUNT(*)
                     FROM knowledge_entries_fts
                     JOIN knowledge_entries ke ON ke.id = knowledge_entries_fts.entry_id
                     WHERE knowledge_entries_fts MATCH ?3
                       AND knowledge_entries_fts.knowledge_base_id = ?1
                       AND ke.knowledge_base_id = ?1
                       AND ke.status NOT IN ('archived', 'stale')
                       AND (?2 IS NULL OR ke.entry_type = ?2)",
                    params![base_id, entry_type, fts_query],
                    |row| row.get::<_, i64>(0),
                )?;
                let mut statement = conn.prepare(
                    "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                            ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
                     FROM knowledge_entries_fts
                     JOIN knowledge_entries ke ON ke.id = knowledge_entries_fts.entry_id
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE knowledge_entries_fts MATCH ?3
                       AND knowledge_entries_fts.knowledge_base_id = ?1
                       AND ke.knowledge_base_id = ?1
                       AND ke.status NOT IN ('archived', 'stale')
                       AND (?2 IS NULL OR ke.entry_type = ?2)
                     ORDER BY CASE WHEN ke.entry_type = 'source_section' THEN 1 ELSE 0 END,
                              bm25(knowledge_entries_fts, 0.0, 0.0, 12.0, 8.0, 4.0, 1.0,
                                   1.0, 2.0),
                              sd.ordinal, ke.title COLLATE NOCASE
                     LIMIT ?4 OFFSET ?5",
                )?;
                let rows = statement.query_map(
                    params![base_id, entry_type, fts_query, limit, offset],
                    map_entry_summary,
                )?;
                (rows.collect::<Result<Vec<_>, _>>()?, total)
            } else if query.is_some_and(|value| !value.trim().is_empty()) {
                (Vec::new(), 0)
            } else {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM knowledge_entries ke
                     WHERE ke.knowledge_base_id = ?1
                       AND ke.status NOT IN ('archived', 'stale')
                       AND (?2 IS NULL OR ke.entry_type = ?2)",
                    params![base_id, entry_type],
                    |row| row.get::<_, i64>(0),
                )?;
                let mut statement = conn.prepare(
                    "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                            ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
                     FROM knowledge_entries ke
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE ke.knowledge_base_id = ?1
                       AND ke.status NOT IN ('archived', 'stale')
                       AND (?2 IS NULL OR ke.entry_type = ?2)
                     ORDER BY CASE WHEN ke.entry_type = 'source_section' THEN 1 ELSE 0 END,
                              sd.ordinal, ke.title COLLATE NOCASE
                     LIMIT ?3 OFFSET ?4",
                )?;
                let rows = statement.query_map(
                    params![base_id, entry_type, limit, offset],
                    map_entry_summary,
                )?;
                (rows.collect::<Result<Vec<_>, _>>()?, total)
            };
            Ok(KnowledgeEntryPage {
                has_more: offset + (entries.len() as i64) < total,
                entries,
                offset,
                limit,
                total,
            })
        })
    }

    pub fn lint_knowledge_base(&self, base_id: &str) -> Result<KnowledgeHealthReport, BrainError> {
        let base = self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let semantic_entry_count = conn.query_row(
                "SELECT COUNT(*) FROM knowledge_entries
                 WHERE knowledge_base_id = ?1 AND entry_type != 'source_section'
                   AND status NOT IN ('archived', 'stale')",
                params![base_id],
                |row| row.get::<_, i64>(0),
            )?;
            let source_span_count = conn.query_row(
                "SELECT COUNT(*) FROM source_spans ss
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 WHERE ss.knowledge_base_id = ?1 AND sd.sync_status = 'current'
                   AND sd.current_version_id = ss.source_version_id",
                params![base_id],
                |row| row.get::<_, i64>(0),
            )?;
            let mut issues = Vec::new();
            if source_span_count == 0 {
                issues.push(KnowledgeHealthIssue {
                    code: "no-source-spans".to_string(),
                    severity: "error".to_string(),
                    title: "没有可引用的来源片段".to_string(),
                    detail: "请先同步 Markdown 来源，再运行智能编译。".to_string(),
                    object_ids: Vec::new(),
                });
            }
            if semantic_entry_count == 0 {
                issues.push(KnowledgeHealthIssue {
                    code: "no-semantic-entries".to_string(),
                    severity: "warning".to_string(),
                    title: "尚未形成主题知识".to_string(),
                    detail: "当前仍可使用章节索引；运行智能编译并审核候选后会生成主题页面。"
                        .to_string(),
                    object_ids: Vec::new(),
                });
            }

            let mut stmt = conn.prepare(
                "SELECT ke.id, ke.title FROM knowledge_entries ke
                 WHERE ke.knowledge_base_id = ?1 AND ke.entry_type != 'source_section'
                   AND ke.status NOT IN ('archived', 'stale')
                   AND NOT EXISTS (
                       SELECT 1 FROM knowledge_citations kc WHERE kc.entry_id = ke.id
                   ) LIMIT 50",
            )?;
            let missing_entry_citations = stmt
                .query_map(params![base_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            if !missing_entry_citations.is_empty() {
                issues.push(KnowledgeHealthIssue {
                    code: "entries-without-citations".to_string(),
                    severity: "error".to_string(),
                    title: format!("{} 个主题缺少来源", missing_entry_citations.len()),
                    detail: missing_entry_citations
                        .iter()
                        .take(5)
                        .map(|(_, title)| title.as_str())
                        .collect::<Vec<_>>()
                        .join("、"),
                    object_ids: missing_entry_citations
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect(),
                });
            }

            let mut stmt = conn.prepare(
                "SELECT kc.id, kc.claim_text FROM knowledge_claims kc
                 WHERE kc.knowledge_base_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM knowledge_citations citation WHERE citation.claim_id = kc.id
                   ) LIMIT 50",
            )?;
            let unsupported_claims = stmt
                .query_map(params![base_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            if !unsupported_claims.is_empty() {
                issues.push(KnowledgeHealthIssue {
                    code: "claims-without-citations".to_string(),
                    severity: "warning".to_string(),
                    title: format!("{} 条论断缺少直接引用", unsupported_claims.len()),
                    detail: unsupported_claims
                        .iter()
                        .take(3)
                        .map(|(_, claim)| claim.as_str())
                        .collect::<Vec<_>>()
                        .join("；"),
                    object_ids: unsupported_claims.into_iter().map(|(id, _)| id).collect(),
                });
            }

            let mut stmt = conn.prepare(
                "SELECT lower(trim(title)), GROUP_CONCAT(id), COUNT(*)
                 FROM knowledge_entries
                 WHERE knowledge_base_id = ?1 AND entry_type != 'source_section'
                   AND status NOT IN ('archived', 'stale')
                 GROUP BY lower(trim(title)) HAVING COUNT(*) > 1 LIMIT 20",
            )?;
            let duplicates = stmt
                .query_map(params![base_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (title, ids, count) in duplicates {
                issues.push(KnowledgeHealthIssue {
                    code: "duplicate-title".to_string(),
                    severity: "warning".to_string(),
                    title: format!("可能重复的主题：{title}"),
                    detail: format!("检测到 {count} 个同名主题，请审核是否需要合并。"),
                    object_ids: ids.split(',').map(str::to_string).collect(),
                });
            }
            let state = if issues.iter().any(|issue| issue.severity == "error") {
                "error"
            } else if issues.is_empty() {
                "healthy"
            } else {
                "warning"
            };
            Ok(KnowledgeHealthReport {
                knowledge_base_id: base_id.to_string(),
                state: state.to_string(),
                semantic_entry_count,
                source_span_count,
                pending_review_count: base.pending_review_count,
                issues,
                generated_at: Utc::now().to_rfc3339(),
            })
        })
    }

    pub fn get_entry(&self, entry_id: &str) -> Result<KnowledgeEntryDetail, BrainError> {
        self.db.with_connection(|conn| {
            let (entry, content_md, aliases_json, edit_policy, revision) = conn
                .query_row(
                    "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                            ke.summary, ke.status, ke.confidence, sd.relative_path,
                            ke.updated_at, ke.content_md, ke.aliases_json, ke.edit_policy,
                            ke.revision
                     FROM knowledge_entries ke
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE ke.id = ?1",
                    params![entry_id],
                    |row| {
                        Ok((
                            KnowledgeEntrySummary {
                                id: row.get(0)?,
                                knowledge_base_id: row.get(1)?,
                                entry_type: row.get(2)?,
                                slug: row.get(3)?,
                                title: row.get(4)?,
                                summary: row.get(5)?,
                                status: row.get(6)?,
                                confidence: row.get(7)?,
                                source_path: row.get(8)?,
                                updated_at: row.get(9)?,
                            },
                            row.get::<_, String>(10)?,
                            row.get::<_, String>(11)?,
                            row.get::<_, String>(12)?,
                            row.get::<_, i64>(13)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(entry_id.to_string()))?;

            let mut stmt = conn.prepare(
                "SELECT kc.id, sd.relative_path, ss.heading, ss.line_start, ss.line_end,
                        kc.quote_text
                 FROM knowledge_citations kc
                 JOIN source_spans ss ON ss.id = kc.source_span_id
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 WHERE kc.entry_id = ?1
                   AND (?2 != 'source_section' OR sd.current_version_id = ss.source_version_id)
                 ORDER BY sd.ordinal, ss.ordinal",
            )?;
            let citations = stmt
                .query_map(params![entry_id, entry.entry_type], |row| {
                    Ok(KnowledgeCitation {
                        id: row.get(0)?,
                        source_path: row.get(1)?,
                        heading: row.get(2)?,
                        line_start: row.get(3)?,
                        line_end: row.get(4)?,
                        quote_text: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let aliases = parse_string_list(&aliases_json, "知识实体别名")?;
            let mut claim_stmt = conn.prepare(
                "SELECT kc.id, kc.predicate, kc.object_text, kc.claim_text, kc.confidence,
                        kc.verification_status,
                        (SELECT COUNT(*) FROM knowledge_citations citation
                         WHERE citation.claim_id = kc.id)
                 FROM knowledge_claims kc WHERE kc.entry_id = ?1 ORDER BY kc.created_at",
            )?;
            let claims = claim_stmt
                .query_map(params![entry_id], |row| {
                    Ok(KnowledgeClaimSummary {
                        id: row.get(0)?,
                        predicate: row.get(1)?,
                        object_text: row.get(2)?,
                        claim_text: row.get(3)?,
                        confidence: row.get(4)?,
                        verification_status: row.get(5)?,
                        citation_count: row.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut relation_stmt = conn.prepare(
                "SELECT kr.id, 'outgoing', kr.relation_type, target.id, target.title,
                        kr.strength, COALESCE(kr.evidence, '')
                 FROM knowledge_relations kr
                 JOIN knowledge_entries target ON target.id = kr.to_entry_id
                 WHERE kr.from_entry_id = ?1
                 UNION ALL
                 SELECT kr.id, 'incoming', kr.relation_type, source.id, source.title,
                        kr.strength, COALESCE(kr.evidence, '')
                 FROM knowledge_relations kr
                 JOIN knowledge_entries source ON source.id = kr.from_entry_id
                 WHERE kr.to_entry_id = ?1
                 ORDER BY 3, 5",
            )?;
            let relations = relation_stmt
                .query_map(params![entry_id], |row| {
                    Ok(KnowledgeRelationSummary {
                        id: row.get(0)?,
                        direction: row.get(1)?,
                        relation_type: row.get(2)?,
                        related_entry_id: row.get(3)?,
                        related_entry_title: row.get(4)?,
                        strength: row.get(5)?,
                        evidence: row.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut version_stmt = conn.prepare(
                "SELECT revision, title, summary, status, created_at
                 FROM knowledge_entry_versions WHERE entry_id = ?1
                 ORDER BY revision DESC LIMIT 20",
            )?;
            let versions = version_stmt
                .query_map(params![entry_id], |row| {
                    Ok(KnowledgeEntryVersionSummary {
                        revision: row.get(0)?,
                        title: row.get(1)?,
                        summary: row.get(2)?,
                        status: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KnowledgeEntryDetail {
                entry,
                content_md,
                aliases,
                edit_policy,
                revision,
                citations,
                claims,
                relations,
                versions,
            })
        })
    }

    pub fn get_graph_overview(
        &self,
        base_id: &str,
        limit: usize,
    ) -> Result<KnowledgeGraphOverview, BrainError> {
        self.get_base(base_id)?;
        let limit = limit.clamp(1, 100) as i64;
        self.db.with_connection(|conn| {
            let relation_count = conn.query_row(
                "SELECT COUNT(*) FROM knowledge_relations kr
                 JOIN knowledge_entries source ON source.id = kr.from_entry_id
                 JOIN knowledge_entries target ON target.id = kr.to_entry_id
                 WHERE kr.knowledge_base_id = ?1
                   AND source.status NOT IN ('archived', 'stale')
                   AND target.status NOT IN ('archived', 'stale')",
                params![base_id],
                |row| row.get::<_, i64>(0),
            )?;
            let entry_columns = "ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                                 ke.summary, ke.status, ke.confidence, sd.relative_path,
                                 ke.updated_at";
            let orphan_sql = format!(
                "SELECT {entry_columns}
                 FROM knowledge_entries ke
                 LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.entry_type != 'source_section'
                   AND ke.status NOT IN ('archived', 'stale')
                   AND NOT EXISTS (
                       SELECT 1 FROM knowledge_relations kr
                       JOIN knowledge_entries source ON source.id = kr.from_entry_id
                       JOIN knowledge_entries target ON target.id = kr.to_entry_id
                       WHERE kr.knowledge_base_id = ?1
                         AND (kr.from_entry_id = ke.id OR kr.to_entry_id = ke.id)
                         AND source.status NOT IN ('archived', 'stale')
                         AND target.status NOT IN ('archived', 'stale')
                   )
                 ORDER BY ke.updated_at DESC, ke.title COLLATE NOCASE LIMIT ?2"
            );
            let mut orphan_stmt = conn.prepare(&orphan_sql)?;
            let orphan_entries = orphan_stmt
                .query_map(params![base_id, limit], map_entry_summary)?
                .collect::<Result<Vec<_>, _>>()?;

            let bridge_sql = format!(
                "SELECT {entry_columns},
                        (SELECT COUNT(*) FROM knowledge_relations kr
                         JOIN knowledge_entries source ON source.id = kr.from_entry_id
                         JOIN knowledge_entries target ON target.id = kr.to_entry_id
                         WHERE kr.knowledge_base_id = ?1
                           AND (kr.from_entry_id = ke.id OR kr.to_entry_id = ke.id)
                           AND source.status NOT IN ('archived', 'stale')
                           AND target.status NOT IN ('archived', 'stale')) AS degree
                 FROM knowledge_entries ke
                 LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.entry_type != 'source_section'
                   AND ke.status NOT IN ('archived', 'stale')
                   AND degree > 0
                 ORDER BY degree DESC, ke.title COLLATE NOCASE LIMIT ?2"
            );
            let mut bridge_stmt = conn.prepare(&bridge_sql)?;
            let bridge_entries = bridge_stmt
                .query_map(params![base_id, limit], |row| {
                    Ok(KnowledgeBridgeEntry {
                        entry: map_entry_summary(row)?,
                        degree: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KnowledgeGraphOverview {
                relation_count,
                orphan_entries,
                bridge_entries,
            })
        })
    }

    pub fn get_graph_snapshot(
        &self,
        base_id: &str,
        limit: usize,
    ) -> Result<KnowledgeGraphSnapshot, BrainError> {
        self.get_base(base_id)?;
        let limit = limit.clamp(10, 300) as i64;
        self.db.with_connection(|conn| {
            let total_entries = conn.query_row(
                "SELECT COUNT(*) FROM knowledge_entries
                 WHERE knowledge_base_id = ?1 AND entry_type != 'source_section'
                   AND status NOT IN ('archived', 'stale')",
                params![base_id],
                |row| row.get::<_, i64>(0),
            )?;
            let mut stmt = conn.prepare(
                "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                        ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at,
                        COUNT(kr.id) AS degree
                 FROM knowledge_entries ke
                 LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                 LEFT JOIN knowledge_relations kr
                   ON kr.knowledge_base_id = ke.knowledge_base_id
                  AND (kr.from_entry_id = ke.id OR kr.to_entry_id = ke.id)
                  AND EXISTS (
                      SELECT 1 FROM knowledge_entries source
                      WHERE source.id = kr.from_entry_id
                        AND source.status NOT IN ('archived', 'stale')
                  )
                  AND EXISTS (
                      SELECT 1 FROM knowledge_entries target
                      WHERE target.id = kr.to_entry_id
                        AND target.status NOT IN ('archived', 'stale')
                  )
                 WHERE ke.knowledge_base_id = ?1 AND ke.entry_type != 'source_section'
                   AND ke.status NOT IN ('archived', 'stale')
                 GROUP BY ke.id
                 ORDER BY degree DESC, ke.updated_at DESC, ke.title COLLATE NOCASE
                 LIMIT ?2",
            )?;
            let entries = stmt
                .query_map(params![base_id, limit], map_entry_summary)?
                .collect::<Result<Vec<_>, _>>()?;
            if entries.is_empty() {
                return Ok(KnowledgeGraphSnapshot {
                    entries,
                    relations: Vec::new(),
                    total_entries,
                    truncated: false,
                });
            }
            let entry_ids = entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>();
            let placeholders = std::iter::repeat_n("?", entry_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT kr.id, kr.from_entry_id, kr.to_entry_id, kr.relation_type,
                        kr.strength, COALESCE(kr.evidence, '')
                 FROM knowledge_relations kr
                 JOIN knowledge_entries source ON source.id = kr.from_entry_id
                 JOIN knowledge_entries target ON target.id = kr.to_entry_id
                 WHERE kr.knowledge_base_id = ?
                   AND source.status NOT IN ('archived', 'stale')
                   AND target.status NOT IN ('archived', 'stale')
                   AND kr.from_entry_id IN ({placeholders})
                   AND kr.to_entry_id IN ({placeholders})
                 ORDER BY kr.relation_type, kr.id"
            );
            let parameters = std::iter::once(base_id)
                .chain(entry_ids.iter().map(String::as_str))
                .chain(entry_ids.iter().map(String::as_str));
            let mut relation_stmt = conn.prepare(&sql)?;
            let relations = relation_stmt
                .query_map(rusqlite::params_from_iter(parameters), |row| {
                    Ok(KnowledgeGraphRelation {
                        id: row.get(0)?,
                        from_entry_id: row.get(1)?,
                        to_entry_id: row.get(2)?,
                        relation_type: row.get(3)?,
                        strength: row.get(4)?,
                        evidence: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(KnowledgeGraphSnapshot {
                truncated: total_entries > entries.len() as i64,
                entries,
                relations,
                total_entries,
            })
        })
    }

    pub fn find_graph_path(
        &self,
        base_id: &str,
        from_entry_id: &str,
        to_entry_id: &str,
        max_depth: usize,
    ) -> Result<KnowledgeGraphPath, BrainError> {
        self.get_base(base_id)?;
        let max_depth = max_depth.clamp(1, 8);
        self.db.with_connection(|conn| {
            let from = load_entry_summary_in_base(conn, base_id, from_entry_id)?;
            let to = load_entry_summary_in_base(conn, base_id, to_entry_id)?;
            if from.id == to.id {
                return Ok(KnowledgeGraphPath {
                    entries: vec![from],
                });
            }
            let mut stmt = conn.prepare(
                "SELECT kr.from_entry_id, kr.to_entry_id FROM knowledge_relations kr
                 JOIN knowledge_entries source ON source.id = kr.from_entry_id
                 JOIN knowledge_entries target ON target.id = kr.to_entry_id
                 WHERE kr.knowledge_base_id = ?1
                   AND source.status NOT IN ('archived', 'stale')
                   AND target.status NOT IN ('archived', 'stale')",
            )?;
            let edges = stmt
                .query_map(params![base_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut adjacency = HashMap::<String, Vec<String>>::new();
            for (left, right) in edges {
                adjacency
                    .entry(left.clone())
                    .or_default()
                    .push(right.clone());
                adjacency.entry(right).or_default().push(left);
            }
            let mut queue = VecDeque::from([(from.id.clone(), 0usize)]);
            let mut previous = HashMap::<String, String>::new();
            let mut visited = HashSet::from([from.id.clone()]);
            while let Some((current, depth)) = queue.pop_front() {
                if depth >= max_depth {
                    continue;
                }
                for next in adjacency.get(&current).into_iter().flatten() {
                    if !visited.insert(next.clone()) {
                        continue;
                    }
                    previous.insert(next.clone(), current.clone());
                    if next == &to.id {
                        let mut ids = vec![to.id.clone()];
                        let mut cursor = to.id.clone();
                        while let Some(parent) = previous.get(&cursor) {
                            ids.push(parent.clone());
                            if parent == &from.id {
                                break;
                            }
                            cursor = parent.clone();
                        }
                        ids.reverse();
                        let entries = ids
                            .iter()
                            .map(|id| load_entry_summary_in_base(conn, base_id, id))
                            .collect::<Result<Vec<_>, _>>()?;
                        return Ok(KnowledgeGraphPath { entries });
                    }
                    queue.push_back((next.clone(), depth + 1));
                }
            }
            Ok(KnowledgeGraphPath {
                entries: Vec::new(),
            })
        })
    }

    pub fn current_citation_span_ids(
        &self,
        base_id: &str,
        entry_ids: &[String],
    ) -> Result<Vec<String>, BrainError> {
        self.get_base(base_id)?;
        if entry_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT kc.source_span_id
                 FROM knowledge_citations kc
                 JOIN source_spans ss ON ss.id = kc.source_span_id
                 JOIN source_versions sv ON sv.id = ss.source_version_id
                 JOIN source_documents sd ON sd.id = sv.source_document_id
                 WHERE kc.knowledge_base_id = ?1 AND kc.entry_id = ?2
                   AND sd.knowledge_base_id = ?1 AND sd.sync_status = 'current'
                   AND sd.current_version_id = ss.source_version_id
                 ORDER BY sd.ordinal, ss.ordinal",
            )?;
            let mut result = Vec::new();
            let mut seen = HashSet::new();
            for entry_id in entry_ids {
                let rows =
                    stmt.query_map(params![base_id, entry_id], |row| row.get::<_, String>(0))?;
                for row in rows {
                    let span_id = row?;
                    if seen.insert(span_id.clone()) {
                        result.push(span_id);
                    }
                }
            }
            Ok(result)
        })
    }

    fn entry_candidate_snapshot(
        &self,
        detail: &KnowledgeEntryDetail,
    ) -> Result<serde_json::Value, BrainError> {
        let entry_id = &detail.entry.id;
        let (citations, claims, relations) = self.db.with_connection(|conn| {
            let mut citation_stmt = conn.prepare(
                "SELECT source_span_id FROM knowledge_citations
                 WHERE entry_id = ?1 ORDER BY created_at, id",
            )?;
            let citations = citation_stmt
                .query_map(params![entry_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            let mut claim_stmt = conn.prepare(
                "SELECT predicate, object_text, claim_text, confidence
                 FROM knowledge_claims WHERE entry_id = ?1 ORDER BY created_at, id",
            )?;
            let claims = claim_stmt
                .query_map(params![entry_id], |row| {
                    Ok(serde_json::json!({
                        "predicate": row.get::<_, String>(0)?,
                        "object_text": row.get::<_, Option<String>>(1)?,
                        "claim_text": row.get::<_, String>(2)?,
                        "confidence": row.get::<_, Option<f64>>(3)?,
                        "citations": citations.clone(),
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut relation_stmt = conn.prepare(
                "SELECT target.slug, kr.relation_type, kr.strength, COALESCE(kr.evidence, '')
                 FROM knowledge_relations kr
                 JOIN knowledge_entries target ON target.id = kr.to_entry_id
                 WHERE kr.from_entry_id = ?1 ORDER BY kr.relation_type, target.title",
            )?;
            let relations = relation_stmt
                .query_map(params![entry_id], |row| {
                    Ok(serde_json::json!({
                        "to_slug": row.get::<_, String>(0)?,
                        "relation_type": row.get::<_, String>(1)?,
                        "strength": row.get::<_, Option<f64>>(2)?,
                        "evidence": row.get::<_, String>(3)?,
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok((citations, claims, relations))
        })?;
        Ok(serde_json::json!({
            "entry_type": detail.entry.entry_type,
            "slug": detail.entry.slug,
            "title": detail.entry.title,
            "summary": detail.entry.summary,
            "content_md": detail.content_md,
            "aliases": detail.aliases,
            "status": detail.entry.status,
            "edit_policy": detail.edit_policy,
            "confidence": detail.entry.confidence,
            "citations": citations,
            "claims": claims,
            "relations": relations,
        }))
    }

    pub fn propose_entry_edit(
        &self,
        entry_id: &str,
        proposal: &KnowledgeEntryEditProposal<'_>,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        let detail = self.get_entry(entry_id)?;
        if detail.entry.entry_type == "source_section" {
            return Err(BrainError::KnowledgeValidation(
                "来源章节来自原始 Markdown，不能在 Wiki 中直接编辑".to_string(),
            ));
        }
        let base = self.get_base(&detail.entry.knowledge_base_id)?;
        if base.lifecycle != "active" {
            return Err(BrainError::KnowledgeValidation(
                "知识库已暂停或归档，只能浏览，不能提交实体变更".to_string(),
            ));
        }
        if detail.revision != proposal.expected_revision {
            return Err(BrainError::KnowledgeValidation(
                "知识实体已变化，请刷新后重试".to_string(),
            ));
        }
        let mut candidate = self.entry_candidate_snapshot(&detail)?;
        candidate["title"] = serde_json::Value::String(proposal.title.trim().to_string());
        candidate["summary"] = serde_json::Value::String(proposal.summary.trim().to_string());
        candidate["content_md"] = serde_json::Value::String(proposal.content_md.trim().to_string());
        candidate["aliases"] = serde_json::to_value(proposal.aliases)
            .map_err(|error| BrainError::Internal(format!("实体别名序列化失败: {error}")))?;
        candidate["status"] = serde_json::Value::String(proposal.status.to_string());
        candidate["edit_policy"] = serde_json::Value::String("human_protected".to_string());
        let input = serde_json::json!({
            "entry_id": entry_id,
            "expected_revision": proposal.expected_revision,
            "source": "wiki_workspace",
        });
        let run = self.start_agent_run(
            &detail.entry.knowledge_base_id,
            "user",
            "knowledge_manual_edit",
            &input,
        )?;
        let result = self.create_semantic_change_set(
            &detail.entry.knowledge_base_id,
            &run.id,
            &format!("人工编辑：{}", detail.entry.title),
            "用户编辑会先形成可比较、可撤销的高风险审核候选",
            &format!("manual-edit:{entry_id}:{}", proposal.expected_revision),
            &[candidate],
        );
        match result {
            Ok(change_set) => {
                self.complete_agent_run(
                    &run.id,
                    &serde_json::json!({ "change_set_id": change_set.id }),
                )?;
                Ok(change_set)
            }
            Err(error) => {
                let _ = self.fail_agent_run(&run.id, &error.to_string());
                Err(error)
            }
        }
    }

    pub fn propose_entry_merge(
        &self,
        target_entry_id: &str,
        proposal: &KnowledgeEntryMergeProposal<'_>,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        let target = self.get_entry(target_entry_id)?;
        self.get_active_base(&target.entry.knowledge_base_id)?;
        if target.entry.entry_type == "source_section"
            || matches!(target.entry.status.as_str(), "archived" | "stale")
        {
            return Err(BrainError::KnowledgeValidation(
                "来源章节或已归档实体不能作为合并目标".to_string(),
            ));
        }
        if target.revision != proposal.expected_revision {
            return Err(BrainError::KnowledgeValidation(
                "合并目标已变化，请刷新后重试".to_string(),
            ));
        }
        if !matches!(proposal.status, "draft" | "verified") {
            return Err(BrainError::KnowledgeValidation(
                "合并后的实体状态只能是 draft 或 verified".to_string(),
            ));
        }
        if proposal.sources.is_empty() || proposal.sources.len() > 20 {
            return Err(BrainError::KnowledgeValidation(
                "一次合并需要选择 1 至 20 个来源实体".to_string(),
            ));
        }
        let mut seen_ids = HashSet::from([target_entry_id.to_string()]);
        let mut source_details = Vec::with_capacity(proposal.sources.len());
        for source in proposal.sources {
            if !seen_ids.insert(source.entry_id.clone()) {
                return Err(BrainError::KnowledgeValidation(
                    "合并来源不能重复或包含目标实体".to_string(),
                ));
            }
            let detail = self.get_entry(&source.entry_id)?;
            if detail.entry.knowledge_base_id != target.entry.knowledge_base_id {
                return Err(BrainError::KnowledgeValidation(
                    "只能合并同一本书中的实体".to_string(),
                ));
            }
            if detail.entry.entry_type == "source_section"
                || matches!(detail.entry.status.as_str(), "archived" | "stale")
            {
                return Err(BrainError::KnowledgeValidation(
                    "来源章节或已归档实体不能参与合并".to_string(),
                ));
            }
            if detail.revision != source.expected_revision {
                return Err(BrainError::KnowledgeValidation(format!(
                    "合并来源“{}”已变化，请刷新后重试",
                    detail.entry.title
                )));
            }
            source_details.push(detail);
        }

        let mut candidates = Vec::with_capacity(source_details.len() + 1);
        let mut target_candidate = self.entry_candidate_snapshot(&target)?;
        let mut citations = json_string_list(&target_candidate, "citations")?;
        let mut citation_set = citations.iter().cloned().collect::<HashSet<_>>();
        let mut claims = target_candidate["claims"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut claim_set = claims
            .iter()
            .filter_map(|claim| claim.get("claim_text").and_then(serde_json::Value::as_str))
            .map(str::to_string)
            .collect::<HashSet<_>>();
        let mut relations = target_candidate["relations"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut relation_set = relations
            .iter()
            .map(|relation| {
                format!(
                    "{}:{}",
                    relation
                        .get("to_slug")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    relation
                        .get("relation_type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                )
            })
            .collect::<HashSet<_>>();
        let merged_slugs = source_details
            .iter()
            .map(|detail| detail.entry.slug.clone())
            .chain(std::iter::once(target.entry.slug.clone()))
            .collect::<HashSet<_>>();
        let mut aliases = proposal.aliases.to_vec();
        for detail in &source_details {
            let source_candidate = self.entry_candidate_snapshot(detail)?;
            for citation in json_string_list(&source_candidate, "citations")? {
                if citation_set.insert(citation.clone()) {
                    citations.push(citation);
                }
            }
            for claim in source_candidate["claims"].as_array().into_iter().flatten() {
                let key = claim
                    .get("claim_text")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if claim_set.insert(key.to_string()) {
                    claims.push(claim.clone());
                }
            }
            for relation in source_candidate["relations"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let to_slug = relation
                    .get("to_slug")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let relation_type = relation
                    .get("relation_type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let key = format!("{to_slug}:{relation_type}");
                if !merged_slugs.contains(to_slug) && relation_set.insert(key) {
                    relations.push(relation.clone());
                }
            }
            aliases.push(detail.entry.title.clone());
            aliases.extend(detail.aliases.clone());
        }
        aliases.retain(|alias| !alias.trim().is_empty());
        aliases.sort();
        aliases.dedup();
        aliases.truncate(30);
        relations.retain(|relation| {
            relation
                .get("to_slug")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|slug| !merged_slugs.contains(slug))
        });
        target_candidate["title"] = serde_json::Value::String(proposal.title.trim().to_string());
        target_candidate["summary"] =
            serde_json::Value::String(proposal.summary.trim().to_string());
        target_candidate["content_md"] =
            serde_json::Value::String(proposal.content_md.trim().to_string());
        target_candidate["aliases"] = serde_json::to_value(aliases)
            .map_err(|error| BrainError::Internal(format!("合并别名序列化失败: {error}")))?;
        target_candidate["status"] = serde_json::Value::String(proposal.status.to_string());
        target_candidate["edit_policy"] = serde_json::Value::String("human_protected".to_string());
        target_candidate["citations"] = serde_json::to_value(citations)
            .map_err(|error| BrainError::Internal(format!("合并引用序列化失败: {error}")))?;
        target_candidate["claims"] = serde_json::Value::Array(claims);
        target_candidate["relations"] = serde_json::Value::Array(relations);
        target_candidate["_operation"] = serde_json::Value::String("merge".to_string());
        target_candidate["merge_source_ids"] = serde_json::to_value(
            source_details
                .iter()
                .map(|detail| detail.entry.id.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| BrainError::Internal(format!("合并来源序列化失败: {error}")))?;
        candidates.push(target_candidate);

        for detail in &source_details {
            let mut candidate = self.entry_candidate_snapshot(detail)?;
            candidate["status"] = serde_json::Value::String("archived".to_string());
            candidate["edit_policy"] = serde_json::Value::String("human_protected".to_string());
            candidate["relations"] = serde_json::json!([]);
            candidate["_operation"] = serde_json::Value::String("merge".to_string());
            candidate["merge_target_id"] = serde_json::Value::String(target_entry_id.to_string());
            candidates.push(candidate);
        }
        let revision_key = proposal
            .sources
            .iter()
            .map(|source| format!("{}:{}", source.entry_id, source.expected_revision))
            .collect::<Vec<_>>()
            .join(",");
        let input = serde_json::json!({
            "target_entry_id": target_entry_id,
            "source_entry_ids": proposal.sources.iter().map(|source| &source.entry_id).collect::<Vec<_>>(),
            "source": "wiki_workspace",
        });
        let run = self.start_agent_run(
            &target.entry.knowledge_base_id,
            "user",
            "knowledge_manual_merge",
            &input,
        )?;
        let result = self.create_semantic_change_set(
            &target.entry.knowledge_base_id,
            &run.id,
            &format!("合并实体：{}", target.entry.title),
            "合并会更新目标、归档来源并迁移关系，批准前不会修改正式知识",
            &format!(
                "manual-merge:{target_entry_id}:{}:{revision_key}",
                proposal.expected_revision
            ),
            &candidates,
        );
        complete_manual_change_run(self, &run.id, result)
    }

    pub fn propose_entry_split(
        &self,
        entry_id: &str,
        proposal: &KnowledgeEntrySplitProposal<'_>,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        let original = self.get_entry(entry_id)?;
        self.get_active_base(&original.entry.knowledge_base_id)?;
        if original.entry.entry_type == "source_section"
            || matches!(original.entry.status.as_str(), "archived" | "stale")
        {
            return Err(BrainError::KnowledgeValidation(
                "来源章节或已归档实体不能拆分".to_string(),
            ));
        }
        if original.revision != proposal.expected_revision {
            return Err(BrainError::KnowledgeValidation(
                "待拆分实体已变化，请刷新后重试".to_string(),
            ));
        }
        if !(2..=12).contains(&proposal.parts.len()) {
            return Err(BrainError::KnowledgeValidation(
                "一次拆分需要提供 2 至 12 个新实体".to_string(),
            ));
        }
        let source_candidate = self.entry_candidate_snapshot(&original)?;
        let citations = json_string_list(&source_candidate, "citations")?;
        let mut candidates = Vec::with_capacity(proposal.parts.len() + 1);
        let mut archived = source_candidate;
        archived["status"] = serde_json::Value::String("archived".to_string());
        archived["edit_policy"] = serde_json::Value::String("human_protected".to_string());
        archived["_operation"] = serde_json::Value::String("split".to_string());
        candidates.push(archived);
        let mut identity_set = HashSet::new();
        for (index, part) in proposal.parts.iter().enumerate() {
            let identity = format!(
                "{}:{}:{}",
                part.title.trim(),
                part.summary.trim(),
                part.content_md.trim()
            );
            if !identity_set.insert(identity.clone()) {
                return Err(BrainError::KnowledgeValidation(
                    "拆分后的实体内容不能完全重复".to_string(),
                ));
            }
            candidates.push(serde_json::json!({
                "entry_type": original.entry.entry_type,
                "slug": format!("{}-{}", original.entry.slug, stable_id("part", &format!("{entry_id}:{index}:{identity}"))),
                "title": part.title.trim(),
                "summary": part.summary.trim(),
                "content_md": part.content_md.trim(),
                "aliases": part.aliases,
                "status": "draft",
                "edit_policy": "human_protected",
                "confidence": original.entry.confidence,
                "citations": citations,
                "claims": [],
                "relations": [],
                "_operation": "split",
                "split_from_entry_id": entry_id,
            }));
        }
        let input = serde_json::json!({
            "entry_id": entry_id,
            "part_count": proposal.parts.len(),
            "source": "wiki_workspace",
        });
        let run = self.start_agent_run(
            &original.entry.knowledge_base_id,
            "user",
            "knowledge_manual_split",
            &input,
        )?;
        let result = self.create_semantic_change_set(
            &original.entry.knowledge_base_id,
            &run.id,
            &format!("拆分实体：{}", original.entry.title),
            "拆分会归档原实体并建立带原始引用的新实体，批准前不会修改正式知识",
            &format!("manual-split:{entry_id}:{}", proposal.expected_revision),
            &candidates,
        );
        complete_manual_change_run(self, &run.id, result)
    }

    pub fn propose_reader_selection(
        &self,
        base_id: &str,
        source_path: &str,
        selection: &str,
        title: Option<&str>,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        self.get_syncable_base(base_id)?;
        let source_path = source_path.trim().trim_start_matches('/');
        let selection = selection.trim();
        if source_path.is_empty() || selection.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "来源路径和选中文本不能为空".to_string(),
            ));
        }
        if selection.chars().count() > 5_000 {
            return Err(BrainError::KnowledgeValidation(
                "一次加入知识候选的选中文本不能超过 5000 个字符".to_string(),
            ));
        }
        let normalized_selection = normalize_reader_selection(selection);
        if normalized_selection.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "选中文本没有可保存的正文内容".to_string(),
            ));
        }
        let span_id = self
            .db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT ss.id, ss.content FROM source_spans ss
                     JOIN source_versions sv ON sv.id = ss.source_version_id
                     JOIN source_documents sd ON sd.id = sv.source_document_id
                     WHERE ss.knowledge_base_id = ?1
                       AND sd.knowledge_base_id = ?1
                       AND sd.relative_path = ?2
                       AND sd.sync_status = 'current'
                       AND sd.current_version_id = ss.source_version_id
                     ORDER BY length(ss.content) ASC, ss.ordinal",
                )?;
                let rows = stmt.query_map(params![base_id, source_path], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (id, content) = row?;
                    let normalized_content = normalize_reader_selection(&content);
                    if normalized_content.contains(&normalized_selection)
                        || normalized_selection.contains(&normalized_content)
                    {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            })?
            .ok_or_else(|| {
                BrainError::KnowledgeValidation(
                    "未能在当前同步版本中定位这段文字；请先同步知识库，或缩短选区后重试"
                        .to_string(),
                )
            })?;
        let default_title = selection
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("阅境轩摘录")
            .trim()
            .chars()
            .take(48)
            .collect::<String>();
        let title = title
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&default_title);
        let digest = stable_id("selection", &format!("{source_path}:{selection}"));
        let candidate = serde_json::json!({
            "entry_type": "synthesis",
            "slug": digest,
            "title": title,
            "summary": selection.chars().take(220).collect::<String>(),
            "content_md": selection,
            "aliases": [],
            "status": "draft",
            "edit_policy": "human_protected",
            "confidence": 1.0,
            "citations": [span_id],
            "claims": [],
            "relations": [],
        });
        let input = serde_json::json!({
            "source_path": source_path,
            "selection_length": selection.chars().count(),
            "source": "reader_selection",
        });
        let run = self.start_agent_run(base_id, "user", "knowledge_reader_selection", &input)?;
        let result = self.create_semantic_change_set(
            base_id,
            &run.id,
            &format!("阅境轩摘录：{title}"),
            "用户从已同步的 Markdown 原文中明确选择内容并提交为知识候选",
            &format!("reader-selection:{base_id}:{digest}"),
            &[candidate],
        );
        match result {
            Ok(change_set) => {
                self.complete_agent_run(
                    &run.id,
                    &serde_json::json!({ "change_set_id": change_set.id }),
                )?;
                Ok(change_set)
            }
            Err(error) => {
                let _ = self.fail_agent_run(&run.id, &error.to_string());
                Err(error)
            }
        }
    }

    pub fn create_semantic_change_set(
        &self,
        base_id: &str,
        run_id: &str,
        title: &str,
        reason: &str,
        idempotency_key: &str,
        candidates: &[serde_json::Value],
    ) -> Result<KnowledgeChangeSet, BrainError> {
        self.get_active_base(base_id)?;
        self.get_agent_run(run_id)?;
        if candidates.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "语义编译没有产生可审核的知识候选".to_string(),
            ));
        }
        let current_span_ids = self
            .list_current_source_spans(base_id)?
            .into_iter()
            .map(|span| span.id)
            .collect::<HashSet<_>>();
        for candidate in candidates {
            validate_semantic_candidate(candidate, &current_span_ids)?;
        }

        let resolved_idempotency_key = match self.find_change_set_by_idempotency(idempotency_key)? {
            Some(existing) if matches!(existing.status.as_str(), "rejected" | "conflicted") => {
                format!("{idempotency_key}:retry:{run_id}")
            }
            Some(existing) => return Ok(existing),
            None => idempotency_key.to_string(),
        };
        let change_set_id = stable_id("change-set", &resolved_idempotency_key);
        let review_id = stable_id("review", &change_set_id);
        let now = Utc::now().to_rfc3339();
        let mut risk_level = "low";
        let mut classification_counts = [0_i64; 3];
        let mut audit_entry_citations = 0_i64;
        let mut audit_claim_citations = 0_i64;
        let mut audit_issues = Vec::<String>::new();
        let mut impact_totals = [0_i64; 4];
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO knowledge_change_sets
                 (id, knowledge_base_id, agent_run_id, title, reason, risk_level, status,
                  idempotency_key, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'low', 'proposed', ?6, ?7)",
                params![
                    change_set_id,
                    base_id,
                    run_id,
                    title.trim(),
                    reason.trim(),
                    resolved_idempotency_key,
                    now,
                ],
            )?;
            for (ordinal, candidate) in candidates.iter().enumerate() {
                let slug = required_json_string(candidate, "slug")?;
                let entry_type = required_json_string(candidate, "entry_type")?;
                let requested_operation = candidate
                    .get("_operation")
                    .and_then(serde_json::Value::as_str);
                if requested_operation.is_some_and(|operation| {
                    !matches!(operation, "merge" | "split" | "archive" | "restore")
                }) {
                    return Err(BrainError::KnowledgeValidation(
                        "知识候选包含未知的结构化操作".to_string(),
                    ));
                }
                if candidate
                    .get("edit_policy")
                    .and_then(serde_json::Value::as_str)
                    == Some("human_protected")
                {
                    risk_level = "high";
                }
                let object_id = stable_id("entry", &format!("{base_id}:{entry_type}:{slug}"));
                let existing = conn
                    .query_row(
                        "SELECT revision, title, summary, content_md, aliases_json, status,
                                confidence, edit_policy
                         FROM knowledge_entries WHERE id = ?1 AND knowledge_base_id = ?2",
                        params![object_id, base_id],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                                row.get::<_, Option<f64>>(6)?,
                                row.get::<_, String>(7)?,
                            ))
                        },
                    )
                    .optional()?;
                let (default_operation, expected_revision, before) = match existing {
                    Some((
                        revision,
                        title,
                        summary,
                        content_md,
                        aliases,
                        status,
                        confidence,
                        policy,
                    )) => {
                        if policy == "human_protected" {
                            risk_level = "high";
                        } else if risk_level != "high" {
                            risk_level = "medium";
                        }
                        (
                            "update",
                            Some(revision),
                            Some(serde_json::json!({
                                "title": title,
                                "summary": summary,
                                "content_md": content_md,
                                "aliases": serde_json::from_str::<serde_json::Value>(&aliases)
                                    .unwrap_or_else(|_| serde_json::json!([])),
                                "status": status,
                                "confidence": confidence,
                                "edit_policy": policy,
                            })),
                        )
                    }
                    None => ("create", None, None),
                };
                let requested_classification = candidate
                    .get("_classification")
                    .and_then(serde_json::Value::as_str);
                let classification = if expected_revision.is_none() {
                    "new"
                } else if requested_classification == Some("disputed") {
                    risk_level = "high";
                    "disputed"
                } else {
                    "update"
                };
                match classification {
                    "new" => classification_counts[0] += 1,
                    "update" => classification_counts[1] += 1,
                    "disputed" => classification_counts[2] += 1,
                    _ => unreachable!(),
                }
                let (citation_audit, entry_citations, claim_citations, issues) =
                    semantic_candidate_citation_audit(candidate)?;
                audit_entry_citations += entry_citations;
                audit_claim_citations += claim_citations;
                audit_issues.extend(issues);
                let impact =
                    knowledge_change_impact(conn, &object_id, expected_revision.is_none())?;
                impact_totals[0] += impact["entries"].as_i64().unwrap_or(0);
                impact_totals[1] += impact["claims"].as_i64().unwrap_or(0);
                impact_totals[2] += impact["relations"].as_i64().unwrap_or(0);
                impact_totals[3] += impact["citations"].as_i64().unwrap_or(0);
                let mut after = candidate.clone();
                if let Some(object) = after.as_object_mut() {
                    object.remove("_operation");
                    object.remove("_classification");
                }
                let operation = requested_operation.unwrap_or(default_operation);
                after["id"] = serde_json::Value::String(object_id.clone());
                conn.execute(
                    "INSERT INTO knowledge_changes
                     (id, change_set_id, ordinal, operation, object_type, object_id,
                      expected_revision, classification, citation_audit_json, impact_json,
                      before_json, after_json)
                     VALUES (?1, ?2, ?3, ?4, 'entry', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        stable_id("change", &format!("{change_set_id}:{ordinal}")),
                        change_set_id,
                        ordinal as i64,
                        operation,
                        object_id,
                        expected_revision,
                        classification,
                        citation_audit.to_string(),
                        impact.to_string(),
                        before.map(|value| value.to_string()),
                        after.to_string(),
                    ],
                )?;
            }
            let classification_summary = serde_json::json!({
                "new": classification_counts[0],
                "update": classification_counts[1],
                "disputed": classification_counts[2],
                "no_material": 0,
            });
            let citation_audit = serde_json::json!({
                "passed": audit_issues.is_empty(),
                "entry_citations": audit_entry_citations,
                "claim_citations": audit_claim_citations,
                "issues": audit_issues,
            });
            let impact_summary = serde_json::json!({
                "entries": impact_totals[0],
                "claims": impact_totals[1],
                "relations": impact_totals[2],
                "citations": impact_totals[3],
            });
            conn.execute(
                "UPDATE knowledge_change_sets
                 SET risk_level = ?2, classification_summary_json = ?3,
                     citation_audit_json = ?4, impact_summary_json = ?5
                 WHERE id = ?1",
                params![
                    change_set_id,
                    risk_level,
                    classification_summary.to_string(),
                    citation_audit.to_string(),
                    impact_summary.to_string(),
                ],
            )?;
            conn.execute(
                "INSERT INTO knowledge_reviews
                 (id, knowledge_base_id, change_set_id, status, created_at)
                 VALUES (?1, ?2, ?3, 'pending', ?4)",
                params![review_id, base_id, change_set_id, now],
            )?;
            conn.execute(
                "UPDATE knowledge_bases
                 SET pending_review_count = (
                        SELECT COUNT(*) FROM knowledge_change_sets
                        WHERE knowledge_base_id = ?1 AND status = 'proposed'
                     ),
                     health_state = 'needs_review', updated_at = ?2
                 WHERE id = ?1",
                params![base_id, now],
            )?;
            Ok(())
        })?;
        self.get_change_set(&change_set_id)
    }

    pub fn create_no_material_change_set(
        &self,
        base_id: &str,
        run_id: &str,
        title: &str,
        reason: &str,
        idempotency_key: &str,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        self.get_active_base(base_id)?;
        self.get_agent_run(run_id)?;
        if let Some(existing) = self.find_change_set_by_idempotency(idempotency_key)? {
            return Ok(existing);
        }
        let change_set_id = stable_id("change-set", idempotency_key);
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_change_sets
                 (id, knowledge_base_id, agent_run_id, title, reason, risk_level, status,
                  idempotency_key, classification_summary_json, citation_audit_json,
                  impact_summary_json, created_at, resolved_at, resolved_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'low', 'applied', ?6,
                         '{\"new\":0,\"update\":0,\"disputed\":0,\"no_material\":1}',
                         '{\"passed\":true,\"entry_citations\":0,\"claim_citations\":0,\"issues\":[]}',
                         '{\"entries\":0,\"claims\":0,\"relations\":0,\"citations\":0}',
                         ?7, ?7, 'agent')",
                params![
                    change_set_id,
                    base_id,
                    run_id,
                    title.trim(),
                    reason.trim(),
                    idempotency_key,
                    now,
                ],
            )?;
            Ok(())
        })?;
        self.get_change_set(&change_set_id)
    }

    pub fn mark_semantic_compile_no_material(
        &self,
        base_id: &str,
        change_set_id: &str,
        processed_sources: i64,
        total_sources: i64,
    ) -> Result<KnowledgeBaseSummary, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_bases
                 SET compile_mode = 'smart', compile_state = 'ready', compile_error = NULL,
                     compile_processed_sources = ?3, compile_total_sources = ?4,
                     compile_phase = 'completed',
                     compile_message = '智能编译完成，没有发现需要审核的实质变化',
                     compile_active_run_id = NULL, compile_change_set_id = ?2,
                     compile_cancel_requested = 0, compile_heartbeat_at = ?5,
                     last_compiled_at = ?5, revision = revision + 1, updated_at = ?5
                 WHERE id = ?1",
                params![
                    base_id,
                    change_set_id,
                    processed_sources,
                    total_sources,
                    now
                ],
            )?;
            Ok(())
        })?;
        self.get_base(base_id)
    }

    pub fn list_change_sets(
        &self,
        base_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<KnowledgeChangeSet>, BrainError> {
        self.get_base(base_id)?;
        let ids = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id FROM knowledge_change_sets
                 WHERE knowledge_base_id = ?1 AND (?2 IS NULL OR status = ?2)
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![base_id, status], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })?;
        ids.into_iter().map(|id| self.get_change_set(&id)).collect()
    }

    pub fn get_change_set(&self, change_set_id: &str) -> Result<KnowledgeChangeSet, BrainError> {
        self.db.with_connection(|conn| {
            let raw = conn
                .query_row(
                    "SELECT id, knowledge_base_id, agent_run_id, title, reason, risk_level,
                            status, created_at, resolved_at, resolved_by,
                            classification_summary_json, citation_audit_json, impact_summary_json
                     FROM knowledge_change_sets WHERE id = ?1",
                    params![change_set_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, Option<String>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, String>(10)?,
                            row.get::<_, String>(11)?,
                            row.get::<_, String>(12)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(change_set_id.to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT id, ordinal, operation, object_type, object_id, expected_revision,
                        classification, citation_audit_json, impact_json, before_json, after_json
                 FROM knowledge_changes WHERE change_set_id = ?1 ORDER BY ordinal",
            )?;
            let changes = stmt
                .query_map(params![change_set_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                })?
                .map(|row| {
                    let row = row?;
                    Ok(KnowledgeChange {
                        id: row.0,
                        ordinal: row.1,
                        operation: row.2,
                        object_type: row.3,
                        object_id: row.4,
                        expected_revision: row.5,
                        classification: row.6,
                        citation_audit: serde_json::from_str(&row.7).map_err(|error| {
                            BrainError::Internal(format!("变更引用审计解析失败: {error}"))
                        })?,
                        impact: serde_json::from_str(&row.8).map_err(|error| {
                            BrainError::Internal(format!("变更影响范围解析失败: {error}"))
                        })?,
                        before: row
                            .9
                            .map(|value| serde_json::from_str(&value))
                            .transpose()
                            .map_err(|error| {
                                BrainError::Internal(format!("变更前快照解析失败: {error}"))
                            })?,
                        after: serde_json::from_str(&row.10).map_err(|error| {
                            BrainError::Internal(format!("变更后快照解析失败: {error}"))
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, BrainError>>()?;
            Ok(KnowledgeChangeSet {
                id: raw.0,
                knowledge_base_id: raw.1,
                agent_run_id: raw.2,
                title: raw.3,
                reason: raw.4,
                risk_level: raw.5,
                status: raw.6,
                created_at: raw.7,
                resolved_at: raw.8,
                resolved_by: raw.9,
                classification_summary: serde_json::from_str(&raw.10).map_err(|error| {
                    BrainError::Internal(format!("变更分类汇总解析失败: {error}"))
                })?,
                citation_audit: serde_json::from_str(&raw.11).map_err(|error| {
                    BrainError::Internal(format!("变更集引用审计解析失败: {error}"))
                })?,
                impact_summary: serde_json::from_str(&raw.12).map_err(|error| {
                    BrainError::Internal(format!("变更集影响范围解析失败: {error}"))
                })?,
                changes,
            })
        })
    }

    fn find_change_set_by_idempotency(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<KnowledgeChangeSet>, BrainError> {
        let id = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id FROM knowledge_change_sets WHERE idempotency_key = ?1",
                params![idempotency_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        id.map(|id| self.get_change_set(&id)).transpose()
    }

    pub fn resolve_change_set(
        &self,
        change_set_id: &str,
        approve: bool,
        note: &str,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        let change_set = self.get_change_set(change_set_id)?;
        self.get_active_base(&change_set.knowledge_base_id)?;
        if change_set.status != "proposed" {
            return Err(BrainError::KnowledgeValidation(
                "该变更集已经处理".to_string(),
            ));
        }
        let related_task_id = change_set
            .agent_run_id
            .as_deref()
            .map(|run_id| self.get_agent_run(run_id))
            .transpose()?
            .and_then(|run| {
                run.input
                    .get("knowledge_task_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            });
        if !approve {
            let now = Utc::now().to_rfc3339();
            self.db.transaction(|conn| {
                conn.execute(
                    "UPDATE knowledge_change_sets
                     SET status = 'rejected', resolved_at = ?2, resolved_by = 'user'
                     WHERE id = ?1 AND status = 'proposed'",
                    params![change_set_id, now],
                )?;
                conn.execute(
                    "UPDATE knowledge_reviews
                     SET status = 'rejected', note = ?2, resolved_at = ?3
                     WHERE change_set_id = ?1",
                    params![change_set_id, note.trim(), now],
                )?;
                conn.execute(
                    "DELETE FROM knowledge_compile_checkpoints
                     WHERE last_change_set_id = ?1",
                    params![change_set_id],
                )?;
                conn.execute(
                    "UPDATE knowledge_bases
                     SET compile_state = 'outdated', compile_error = NULL,
                         compile_phase = 'rejected',
                         compile_message = '知识变更已驳回，可以重新运行智能编译',
                         compile_active_run_id = NULL, compile_change_set_id = NULL,
                         compile_cancel_requested = 0, updated_at = ?2
                     WHERE id = ?1 AND compile_change_set_id = ?3",
                    params![change_set.knowledge_base_id, now, change_set_id],
                )?;
                if let Some(task_id) = related_task_id.as_deref() {
                    conn.execute(
                        "UPDATE knowledge_tasks
                         SET knowledge_change_state = 'rejected', updated_at = ?2 WHERE id = ?1",
                        params![task_id, now],
                    )?;
                }
                refresh_review_state(conn, &change_set.knowledge_base_id, &now)?;
                Ok(())
            })?;
            return self.get_change_set(change_set_id);
        }

        let current_span_ids = self
            .list_current_source_spans(&change_set.knowledge_base_id)?
            .into_iter()
            .map(|span| span.id)
            .collect::<HashSet<_>>();
        for change in &change_set.changes {
            validate_semantic_candidate(&change.after, &current_span_ids)?;
        }
        if change_set.changes.len() >= 50 {
            self.db
                .create_managed_backup("before-large-change-set", self.db.backup_retention())?;
        }
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            for change in &change_set.changes {
                validate_change_revision(conn, &change_set.knowledge_base_id, change)?;
            }
            for change in &change_set.changes {
                apply_entry_change(conn, &change_set, change, &now)?;
            }
            for change in &change_set.changes {
                apply_entry_relations(conn, &change_set.knowledge_base_id, change, &now)?;
            }
            apply_merge_redirects(conn, &change_set, &now)?;
            conn.execute(
                "UPDATE knowledge_change_sets
                 SET status = 'applied', resolved_at = ?2, resolved_by = 'user'
                 WHERE id = ?1 AND status = 'proposed'",
                params![change_set_id, now],
            )?;
            conn.execute(
                "UPDATE knowledge_reviews
                 SET status = 'approved', note = ?2, resolved_at = ?3
                 WHERE change_set_id = ?1",
                params![change_set_id, note.trim(), now],
            )?;
            conn.execute(
                "UPDATE knowledge_bases
                 SET compile_mode = 'smart', compile_state = 'ready', compile_error = NULL,
                     compile_phase = 'completed', compile_message = '智能 Wiki 已审核并应用',
                     compile_active_run_id = NULL, compile_change_set_id = NULL,
                     compile_cancel_requested = 0,
                     last_compiled_at = ?2, updated_at = ?2
                 WHERE id = ?1",
                params![change_set.knowledge_base_id, now],
            )?;
            if let Some(task_id) = related_task_id.as_deref() {
                conn.execute(
                    "UPDATE knowledge_tasks
                     SET knowledge_change_state = 'applied', updated_at = ?2 WHERE id = ?1",
                    params![task_id, now],
                )?;
            }
            refresh_review_state(conn, &change_set.knowledge_base_id, &now)?;
            Ok(())
        })?;
        self.get_change_set(change_set_id)
    }

    pub fn list_conversations(
        &self,
        base_id: &str,
        limit: usize,
    ) -> Result<Vec<KnowledgeConversationSummary>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT kc.id, kcs.knowledge_base_id, kc.title,
                        (SELECT COUNT(*) FROM knowledge_messages km
                         WHERE km.conversation_id = kc.id),
                        COALESCE((SELECT km.content FROM knowledge_messages km
                                  WHERE km.conversation_id = kc.id
                                  ORDER BY km.ordinal DESC LIMIT 1), ''),
                        kc.created_at, kc.updated_at
                 FROM knowledge_conversations kc
                 JOIN knowledge_conversation_scopes kcs ON kcs.conversation_id = kc.id
                 WHERE kcs.knowledge_base_id = ?1
                 ORDER BY kc.updated_at DESC
                 LIMIT ?2",
            )?;
            let conversations =
                stmt.query_map(params![base_id, limit.clamp(1, 100) as i64], |row| {
                    Ok(KnowledgeConversationSummary {
                        id: row.get(0)?,
                        knowledge_base_id: row.get(1)?,
                        title: row.get(2)?,
                        message_count: row.get(3)?,
                        preview: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                })?;
            conversations
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<KnowledgeConversationDetail, BrainError> {
        self.db.with_connection(|conn| {
            let conversation = conn
                .query_row(
                    "SELECT kc.id, kcs.knowledge_base_id, kc.title,
                            (SELECT COUNT(*) FROM knowledge_messages km
                             WHERE km.conversation_id = kc.id),
                            COALESCE((SELECT km.content FROM knowledge_messages km
                                      WHERE km.conversation_id = kc.id
                                      ORDER BY km.ordinal DESC LIMIT 1), ''),
                            kc.created_at, kc.updated_at
                     FROM knowledge_conversations kc
                     JOIN knowledge_conversation_scopes kcs ON kcs.conversation_id = kc.id
                     WHERE kc.id = ?1 AND kcs.ordinal = 0",
                    params![conversation_id],
                    |row| {
                        Ok(KnowledgeConversationSummary {
                            id: row.get(0)?,
                            knowledge_base_id: row.get(1)?,
                            title: row.get(2)?,
                            message_count: row.get(3)?,
                            preview: row.get(4)?,
                            created_at: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(conversation_id.to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT id, role, content, run_id, created_at
                 FROM knowledge_messages
                 WHERE conversation_id = ?1
                 ORDER BY ordinal",
            )?;
            let rows = stmt
                .query_map(params![conversation_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut messages = Vec::with_capacity(rows.len());
            for (id, role, content, run_id, created_at) in rows {
                messages.push(KnowledgeMessage {
                    evidence: load_message_evidence(conn, &id)?,
                    id,
                    role,
                    content,
                    run_id,
                    created_at,
                });
            }
            Ok(KnowledgeConversationDetail {
                conversation,
                messages,
            })
        })
    }

    pub fn save_conversation_exchange(
        &self,
        base_id: &str,
        conversation_id: Option<&str>,
        question: &str,
        answer: &str,
        run_id: &str,
        evidence: &[KnowledgeEntrySummary],
    ) -> Result<String, BrainError> {
        self.get_base(base_id)?;
        if evidence
            .iter()
            .any(|entry| entry.knowledge_base_id != base_id)
        {
            return Err(BrainError::KnowledgeValidation(
                "消息引用不能跨越当前书籍知识边界".to_string(),
            ));
        }
        let id = conversation_id
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let user_message_id = uuid::Uuid::new_v4().to_string();
        let assistant_message_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let title = conversation_title(question);
        self.db.transaction(|conn| {
            if conversation_id.is_some() {
                let in_scope = conn
                    .query_row(
                        "SELECT 1 FROM knowledge_conversation_scopes
                         WHERE conversation_id = ?1 AND knowledge_base_id = ?2",
                        params![id, base_id],
                        |_| Ok(()),
                    )
                    .optional()?;
                if in_scope.is_none() {
                    return Err(BrainError::KnowledgeValidation(
                        "会话不存在或不属于当前知识库".to_string(),
                    ));
                }
            } else {
                conn.execute(
                    "INSERT INTO knowledge_conversations (id, title, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3)",
                    params![id, title, now],
                )?;
                conn.execute(
                    "INSERT INTO knowledge_conversation_scopes
                     (conversation_id, knowledge_base_id, ordinal) VALUES (?1, ?2, 0)",
                    params![id, base_id],
                )?;
            }
            let next_ordinal = conn.query_row(
                "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM knowledge_messages
                 WHERE conversation_id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )?;
            conn.execute(
                "INSERT INTO knowledge_messages
                 (id, conversation_id, ordinal, role, content, run_id, created_at)
                 VALUES (?1, ?2, ?3, 'user', ?4, NULL, ?5)",
                params![user_message_id, id, next_ordinal, question, now],
            )?;
            conn.execute(
                "INSERT INTO knowledge_messages
                 (id, conversation_id, ordinal, role, content, run_id, created_at)
                 VALUES (?1, ?2, ?3, 'assistant', ?4, ?5, ?6)",
                params![
                    assistant_message_id,
                    id,
                    next_ordinal + 1,
                    answer,
                    run_id,
                    now
                ],
            )?;
            for (ordinal, entry) in evidence.iter().enumerate() {
                let inserted = conn.execute(
                    "INSERT INTO knowledge_message_citations
                     (message_id, ordinal, entry_id, entry_revision,
                      knowledge_base_id_snapshot, entry_type_snapshot, slug_snapshot,
                      title_snapshot, summary_snapshot, status_snapshot, confidence_snapshot,
                      source_path_snapshot, updated_at_snapshot)
                     SELECT ?1, ?2, ke.id, ke.revision, ke.knowledge_base_id, ke.entry_type,
                            ke.slug, ke.title, ke.summary, ke.status, ke.confidence,
                            sd.relative_path, ke.updated_at
                     FROM knowledge_entries ke
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE ke.id = ?3 AND ke.knowledge_base_id = ?4",
                    params![assistant_message_id, ordinal as i64, entry.id, base_id],
                )?;
                if inserted == 0 {
                    return Err(BrainError::KnowledgeValidation(format!(
                        "消息引用的知识条目不存在: {}",
                        entry.id
                    )));
                }
            }
            conn.execute(
                "UPDATE knowledge_conversations SET updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
            Ok(())
        })?;
        Ok(id)
    }

    pub fn list_tasks(&self, base_id: Option<&str>) -> Result<Vec<KnowledgeTask>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT kt.id, kt.knowledge_base_id, b.name, kt.title, kt.description,
                        kt.task_type, kt.status, kt.result_summary, kt.deliverable_type,
                        kt.artifact_state, kt.knowledge_change_state, kt.cancel_requested,
                        kt.external_research_enabled, kt.external_domains_json,
                        kt.external_request_limit, kt.external_requests_used,
                        kt.created_at, kt.updated_at
                 FROM knowledge_tasks kt
                 JOIN knowledge_bases kb ON kb.id = kt.knowledge_base_id
                 JOIN reader_books b ON b.id = kb.book_id
                 WHERE (?1 IS NULL OR kt.knowledge_base_id = ?1)
                 ORDER BY kt.updated_at DESC",
            )?;
            let rows = stmt.query_map(params![base_id], map_knowledge_task)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn create_task(
        &self,
        base_id: &str,
        title: &str,
        description: &str,
        task_type: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        self.create_task_with_deliverable(base_id, title, description, task_type, "report")
    }

    pub fn create_task_with_deliverable(
        &self,
        base_id: &str,
        title: &str,
        description: &str,
        task_type: &str,
        deliverable_type: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        self.create_task_with_options(
            base_id,
            title,
            description,
            task_type,
            deliverable_type,
            false,
            &[],
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_task_with_options(
        &self,
        base_id: &str,
        title: &str,
        description: &str,
        task_type: &str,
        deliverable_type: &str,
        external_research_enabled: bool,
        external_domains: &[String],
        external_request_limit: i64,
    ) -> Result<KnowledgeTask, BrainError> {
        let title = title.trim();
        let description = description.trim();
        if title.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "研究任务标题不能为空".to_string(),
            ));
        }
        if title.chars().count() > 200 {
            return Err(BrainError::KnowledgeValidation(
                "研究任务标题不能超过 200 个字符".to_string(),
            ));
        }
        if description.chars().count() > 4_000 {
            return Err(BrainError::KnowledgeValidation(
                "研究任务说明不能超过 4000 个字符".to_string(),
            ));
        }
        if !matches!(task_type, "research" | "refresh" | "review") {
            return Err(BrainError::KnowledgeValidation(
                "未知的研究任务类型".to_string(),
            ));
        }
        if !matches!(deliverable_type, "report" | "presentation") {
            return Err(BrainError::KnowledgeValidation(
                "未知的研究任务交付物类型".to_string(),
            ));
        }
        let external_domains = normalize_external_domains(external_domains)?;
        if external_research_enabled && task_type != "research" {
            return Err(BrainError::KnowledgeValidation(
                "只有专题研究任务可以授权外部资料检索".to_string(),
            ));
        }
        if external_research_enabled
            && (external_domains.is_empty() || !(1..=50).contains(&external_request_limit))
        {
            return Err(BrainError::KnowledgeValidation(
                "外部研究必须授权 1 至 20 个域名，并设置 1 至 50 次请求额度".to_string(),
            ));
        }
        let external_request_limit = if external_research_enabled {
            external_request_limit
        } else {
            0
        };
        let external_domains = if external_research_enabled {
            external_domains
        } else {
            Vec::new()
        };
        let external_domains_json = serde_json::to_string(&external_domains)
            .map_err(|error| BrainError::Internal(format!("外部研究域名序列化失败: {error}")))?;
        self.get_active_base(base_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_tasks
                 (id, knowledge_base_id, title, description, task_type, status,
                  deliverable_type, artifact_state, external_research_enabled,
                  external_domains_json, external_request_limit, external_requests_used,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6,
                         CASE WHEN ?6 = 'presentation' THEN 'pending' ELSE 'not_requested' END,
                         ?7, ?8, ?9, 0, ?10, ?10)",
                params![
                    id,
                    base_id,
                    title,
                    description,
                    task_type,
                    deliverable_type,
                    external_research_enabled,
                    external_domains_json,
                    external_request_limit,
                    now,
                ],
            )?;
            Ok(())
        })?;
        self.list_tasks(Some(base_id))?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or(BrainError::KnowledgeNotFound(id))
    }

    pub fn get_task(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT kt.id, kt.knowledge_base_id, b.name, kt.title, kt.description,
                        kt.task_type, kt.status, kt.result_summary, kt.deliverable_type,
                        kt.artifact_state, kt.knowledge_change_state, kt.cancel_requested,
                        kt.external_research_enabled, kt.external_domains_json,
                        kt.external_request_limit, kt.external_requests_used,
                        kt.created_at, kt.updated_at
                 FROM knowledge_tasks kt
                 JOIN knowledge_bases kb ON kb.id = kt.knowledge_base_id
                 JOIN reader_books b ON b.id = kb.book_id
                 WHERE kt.id = ?1",
                params![task_id],
                map_knowledge_task,
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(task_id.to_string()))
        })
    }

    pub fn start_task_execution(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        let current = self.get_task(task_id)?;
        self.get_active_base(&current.knowledge_base_id)?;
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        let lease_expires_at = (now + chrono::Duration::seconds(45)).to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'running', result_summary = '', cancel_requested = 0,
                     attempt_count = attempt_count + 1, lease_expires_at = ?3,
                     last_heartbeat_at = ?2, next_attempt_at = NULL,
                     artifact_state = CASE
                         WHEN deliverable_type = 'presentation' THEN 'pending'
                         ELSE artifact_state END,
                     updated_at = ?2
                 WHERE id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM knowledge_tasks running
                       WHERE running.knowledge_base_id = knowledge_tasks.knowledge_base_id
                         AND running.status = 'running' AND running.id <> ?1
                   )
                   AND (
                       status IN ('draft', 'failed', 'completed', 'cancelled')
                       OR (
                           status = 'running'
                           AND julianday(updated_at) < julianday(?2, '-10 minutes')
                       )
                   )",
                params![task_id, now_text, lease_expires_at],
            )?)
        })?;
        if updated == 0 {
            self.get_task(task_id)?;
            return Err(BrainError::KnowledgeValidation(
                "任务正在执行或当前状态不允许重新运行".to_string(),
            ));
        }
        self.get_task(task_id)
    }

    pub fn queue_task_execution(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        let current = self.get_task(task_id)?;
        self.get_active_base(&current.knowledge_base_id)?;
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'queued', result_summary = '', cancel_requested = 0,
                     attempt_count = 0, next_attempt_at = ?2,
                     lease_expires_at = NULL, last_heartbeat_at = NULL,
                     artifact_state = CASE
                         WHEN deliverable_type = 'presentation' THEN 'pending'
                         ELSE artifact_state END,
                     updated_at = ?2
                 WHERE id = ?1 AND status IN ('draft', 'failed', 'completed', 'cancelled')",
                params![task_id, now],
            )?)
        })?;
        if updated == 0 {
            self.get_task(task_id)?;
            return Err(BrainError::KnowledgeValidation(
                "任务正在排队或执行中".to_string(),
            ));
        }
        self.get_task(task_id)
    }

    pub fn recover_interrupted_tasks(&self) -> Result<usize, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            let recovered = conn.execute(
                "UPDATE knowledge_tasks
                 SET status = CASE WHEN cancel_requested = 1 THEN 'cancelled' ELSE 'queued' END,
                     result_summary = CASE WHEN cancel_requested = 1
                         THEN '任务在服务停止前收到取消请求' ELSE result_summary END,
                     next_attempt_at = CASE WHEN cancel_requested = 1 THEN NULL ELSE ?1 END,
                     lease_expires_at = NULL, last_heartbeat_at = NULL, updated_at = ?1
                 WHERE status = 'running'",
                params![now],
            )?;
            conn.execute(
                "UPDATE agent_runs
                 SET status = 'failed', error = '服务重启中断了本次运行', finished_at = ?1
                 WHERE status = 'running'",
                params![now],
            )?;
            conn.execute(
                "UPDATE knowledge_bases
                 SET compile_state = 'failed', compile_error = '服务重启中断了智能编译，请重新开始',
                     compile_phase = 'failed', compile_message = '服务重启中断了智能编译，请重新开始',
                     compile_active_run_id = NULL, compile_cancel_requested = 0,
                     compile_heartbeat_at = ?1,
                     updated_at = ?1
                 WHERE compile_state = 'compiling' AND compile_phase != 'waiting_review'",
                params![now],
            )?;
            conn.execute(
                "UPDATE skill_benchmark_runs
                 SET status = 'failed',
                     error = '服务重启中断了真实模型基准，请重新运行',
                     completed_at = ?1
                 WHERE status IN ('queued', 'running')",
                params![now],
            )?;
            Ok(recovered)
        })
    }

    pub fn claim_next_queued_task(&self) -> Result<Option<KnowledgeTask>, BrainError> {
        let task_id = self.db.transaction(|conn| {
            let task_id = conn
                .query_row(
                    "SELECT kt.id FROM knowledge_tasks kt
                     JOIN knowledge_bases kb ON kb.id = kt.knowledge_base_id
                     WHERE kt.status = 'queued' AND kb.lifecycle = 'active'
                       AND (kt.next_attempt_at IS NULL OR julianday(kt.next_attempt_at) <= julianday('now'))
                       AND NOT EXISTS (
                           SELECT 1 FROM knowledge_tasks running
                           WHERE running.knowledge_base_id = kt.knowledge_base_id
                             AND running.status = 'running'
                       )
                     ORDER BY kt.updated_at, kt.created_at LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let Some(task_id) = task_id else {
                return Ok(None);
            };
            let now = Utc::now();
            let now_text = now.to_rfc3339();
            let lease_expires_at = (now + chrono::Duration::seconds(45)).to_rfc3339();
            let updated = conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'running', attempt_count = attempt_count + 1,
                     lease_expires_at = ?3, last_heartbeat_at = ?2, updated_at = ?2
                 WHERE id = ?1 AND status = 'queued'",
                params![task_id, now_text, lease_expires_at],
            )?;
            Ok((updated == 1).then_some(task_id))
        })?;
        task_id.map(|task_id| self.get_task(&task_id)).transpose()
    }

    pub fn renew_task_lease(&self, task_id: &str) -> Result<bool, BrainError> {
        let now = Utc::now();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET lease_expires_at = ?2, last_heartbeat_at = ?3, updated_at = ?3
                 WHERE id = ?1 AND status = 'running'",
                params![
                    task_id,
                    (now + chrono::Duration::seconds(45)).to_rfc3339(),
                    now.to_rfc3339()
                ],
            )?)
        })?;
        Ok(updated == 1)
    }

    pub fn recover_expired_task_leases(&self) -> Result<usize, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = CASE WHEN cancel_requested = 1 THEN 'cancelled' ELSE 'queued' END,
                     result_summary = CASE WHEN cancel_requested = 1
                         THEN '任务在租约过期前收到取消请求'
                         ELSE '上一次执行租约已过期，任务已重新排队' END,
                     next_attempt_at = CASE WHEN cancel_requested = 1 THEN NULL ELSE ?1 END,
                     lease_expires_at = NULL, last_heartbeat_at = NULL, updated_at = ?1
                 WHERE status = 'running'
                   AND lease_expires_at IS NOT NULL
                   AND julianday(lease_expires_at) <= julianday(?1)",
                params![now],
            )?)
        })
    }

    pub fn retry_or_fail_task_execution(
        &self,
        task_id: &str,
        error: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now();
        let state = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT attempt_count, max_attempts FROM knowledge_tasks
                 WHERE id = ?1 AND status = 'running'",
                params![task_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(Into::into)
        })?;
        let Some((attempt_count, max_attempts)) = state else {
            return Err(BrainError::KnowledgeValidation(
                "任务不存在或已经结束".to_string(),
            ));
        };
        if attempt_count < max_attempts {
            let delay_seconds = 5_i64.saturating_mul(2_i64.pow((attempt_count - 1).max(0) as u32));
            let next_attempt_at =
                (now + chrono::Duration::seconds(delay_seconds.min(300))).to_rfc3339();
            self.db.with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks
                     SET status = 'queued', result_summary = ?2, next_attempt_at = ?3,
                         lease_expires_at = NULL, last_heartbeat_at = NULL, updated_at = ?4
                     WHERE id = ?1 AND status = 'running'",
                    params![task_id, error.trim(), next_attempt_at, now.to_rfc3339()],
                )?;
                Ok(())
            })?;
        } else {
            self.fail_task_execution(task_id, error)?;
        }
        self.get_task(task_id)
    }

    pub fn complete_task_execution(
        &self,
        task_id: &str,
        result_summary: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'completed', result_summary = ?2, updated_at = ?3
                     , lease_expires_at = NULL, last_heartbeat_at = NULL, next_attempt_at = NULL
                 WHERE id = ?1 AND status = 'running'",
                params![task_id, result_summary.trim(), now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "任务不存在或已经结束".to_string(),
            ));
        }
        self.get_task(task_id)
    }

    pub fn fail_task_execution(
        &self,
        task_id: &str,
        error: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'failed', result_summary = ?2, updated_at = ?3
                     , lease_expires_at = NULL, last_heartbeat_at = NULL, next_attempt_at = NULL
                 WHERE id = ?1 AND status = 'running'",
                params![task_id, error.trim(), now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "任务不存在或已经结束".to_string(),
            ));
        }
        self.get_task(task_id)
    }

    pub fn request_task_cancel(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET cancel_requested = 1,
                     status = CASE WHEN status IN ('draft', 'queued') THEN 'cancelled' ELSE status END,
                     updated_at = ?2
                 WHERE id = ?1 AND status IN ('draft', 'queued', 'running')",
                params![task_id, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "任务不存在或已经结束".to_string(),
            ));
        }
        self.get_task(task_id)
    }

    pub fn cancel_task_execution(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'cancelled', artifact_state = CASE
                         WHEN deliverable_type = 'presentation' THEN 'failed'
                         ELSE artifact_state END,
                     lease_expires_at = NULL, last_heartbeat_at = NULL,
                     next_attempt_at = NULL, updated_at = ?2
                 WHERE id = ?1 AND status = 'running'",
                params![task_id, now],
            )?;
            Ok(())
        })?;
        self.get_task(task_id)
    }

    pub fn set_task_artifact_state(
        &self,
        task_id: &str,
        state: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        if !matches!(state, "not_requested" | "pending" | "ready" | "failed") {
            return Err(BrainError::KnowledgeValidation(
                "未知的成果文件状态".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_tasks SET artifact_state = ?2, updated_at = ?3 WHERE id = ?1",
                params![task_id, state, now],
            )?;
            Ok(())
        })?;
        self.get_task(task_id)
    }

    pub fn set_task_knowledge_change_state(
        &self,
        task_id: &str,
        state: &str,
    ) -> Result<KnowledgeTask, BrainError> {
        if !matches!(state, "none" | "proposed" | "applied" | "rejected") {
            return Err(BrainError::KnowledgeValidation(
                "未知的任务知识变更状态".to_string(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE knowledge_tasks
                 SET knowledge_change_state = ?2, updated_at = ?3 WHERE id = ?1",
                params![task_id, state, now],
            )?;
            Ok(())
        })?;
        self.get_task(task_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_artifact(
        &self,
        base_id: &str,
        task_id: &str,
        run_id: &str,
        skill_id: Option<&str>,
        title: &str,
        relative_path: &str,
        content_hash: &str,
        size_bytes: i64,
        validation_state: &str,
        validation_message: &str,
        validation_details: &Value,
        evidence: &[KnowledgeEntrySummary],
    ) -> Result<KnowledgeArtifact, BrainError> {
        if relative_path.starts_with('/')
            || relative_path
                .split('/')
                .any(|part| part == ".." || part.is_empty())
        {
            return Err(BrainError::KnowledgeValidation(
                "成果文件路径必须是安全的相对路径".to_string(),
            ));
        }
        let task = self.get_task(task_id)?;
        if task.knowledge_base_id != base_id {
            return Err(BrainError::KnowledgeValidation(
                "成果文件与任务不属于同一知识库".to_string(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let validation_details_json = serde_json::to_string(validation_details)
            .map_err(|error| BrainError::Internal(format!("成果校验详情序列化失败: {error}")))?;
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO knowledge_artifacts
                 (id, knowledge_base_id, knowledge_task_id, agent_run_id, skill_id,
                  artifact_type, title, relative_path, mime_type, content_hash, size_bytes,
                  validation_state, validation_message, validation_details_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pptx', ?6, ?7,
                         'application/vnd.openxmlformats-officedocument.presentationml.presentation',
                         ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    id,
                    base_id,
                    task_id,
                    run_id,
                    skill_id,
                    title.trim(),
                    relative_path,
                    content_hash,
                    size_bytes,
                    validation_state,
                    validation_message,
                    validation_details_json,
                    now,
                ],
            )?;
            for (ordinal, entry) in evidence.iter().enumerate() {
                conn.execute(
                    "INSERT INTO knowledge_artifact_citations
                     (artifact_id, ordinal, entry_id, entry_revision, title_snapshot,
                      source_path_snapshot)
                     SELECT ?1, ?2, ke.id, ke.revision, ke.title, sd.relative_path
                     FROM knowledge_entries ke
                     LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                     WHERE ke.id = ?3 AND ke.knowledge_base_id = ?4",
                    params![id, ordinal as i64, entry.id, base_id],
                )?;
            }
            conn.execute(
                "UPDATE knowledge_tasks SET artifact_state = 'ready', updated_at = ?2 WHERE id = ?1",
                params![task_id, now],
            )?;
            Ok(())
        })?;
        self.get_artifact(&id)
    }

    pub fn list_task_artifacts(&self, task_id: &str) -> Result<Vec<KnowledgeArtifact>, BrainError> {
        self.get_task(task_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, knowledge_base_id, knowledge_task_id, agent_run_id, skill_id,
                        artifact_type, title, relative_path, mime_type, content_hash, size_bytes,
                        validation_state, validation_message, validation_details_json, created_at
                 FROM knowledge_artifacts WHERE knowledge_task_id = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![task_id], map_artifact)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn get_artifact(&self, artifact_id: &str) -> Result<KnowledgeArtifact, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, knowledge_base_id, knowledge_task_id, agent_run_id, skill_id,
                        artifact_type, title, relative_path, mime_type, content_hash, size_bytes,
                        validation_state, validation_message, validation_details_json, created_at
                 FROM knowledge_artifacts WHERE id = ?1",
                params![artifact_id],
                map_artifact,
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(artifact_id.to_string()))
        })
    }

    pub fn list_wiki_skills(&self, base_id: Option<&str>) -> Result<Vec<WikiSkill>, BrainError> {
        if let Some(base_id) = base_id {
            self.get_base(base_id)?;
        }
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id, s.slug, s.name, s.description, s.source_type, s.status,
                        s.permissions_json, s.requirements_json, sv.revision, sf.content_text,
                        COALESCE(kbsb.enabled, 0), COALESCE(kbsb.usage_scope, 'both'),
                        s.updated_at
                 FROM skills s
                 JOIN skill_versions sv ON sv.id = s.current_version_id
                 JOIN skill_files sf ON sf.skill_version_id = sv.id
                                    AND sf.relative_path = 'SKILL.md'
                 LEFT JOIN knowledge_base_skill_bindings kbsb
                        ON kbsb.skill_id = s.id AND kbsb.knowledge_base_id = ?1
                 ORDER BY CASE s.source_type WHEN 'builtin' THEN 0 ELSE 1 END, s.name",
            )?;
            let rows = stmt.query_map(params![base_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, bool>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            })?;
            rows.map(|row| {
                let row = row?;
                Ok(WikiSkill {
                    id: row.0,
                    slug: row.1,
                    name: row.2,
                    description: row.3,
                    source_type: row.4,
                    status: row.5,
                    permissions: parse_string_list(&row.6, "Skill 权限")?,
                    requirements: parse_string_list(&row.7, "Skill 依赖")?,
                    revision: row.8,
                    instructions: extract_skill_instructions(&row.9),
                    enabled: row.10,
                    usage_scope: row.11,
                    updated_at: row.12,
                })
            })
            .collect::<Result<Vec<_>, BrainError>>()
        })
    }

    pub fn get_wiki_skill_detail(
        &self,
        skill_id: &str,
        base_id: Option<&str>,
    ) -> Result<WikiSkillDetail, BrainError> {
        let skill = self
            .list_wiki_skills(base_id)?
            .into_iter()
            .find(|skill| skill.id == skill_id)
            .ok_or_else(|| BrainError::KnowledgeNotFound(skill_id.to_string()))?;
        let (current_version_id, versions) = self.db.with_connection(|conn| {
            let current_version_id = conn
                .query_row(
                    "SELECT current_version_id FROM skills WHERE id = ?1",
                    params![skill_id],
                    |row| row.get::<_, Option<String>>(0),
                )?
                .ok_or_else(|| {
                    BrainError::Internal("Skill 当前版本不存在，数据库状态已损坏".to_string())
                })?;
            let version_rows = {
                let mut stmt = conn.prepare(
                    "SELECT id, revision, content_hash, release_state, parent_version_id,
                            changelog, created_at
                     FROM skill_versions WHERE skill_id = ?1 ORDER BY revision DESC",
                )?;
                let rows = stmt.query_map(params![skill_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let mut versions = Vec::with_capacity(version_rows.len());
            for (
                id,
                revision,
                content_hash,
                release_state,
                parent_version_id,
                changelog,
                created_at,
            ) in version_rows
            {
                let files = {
                    let mut stmt = conn.prepare(
                        "SELECT relative_path, media_type, content_text, content_hash, size_bytes
                         FROM skill_files WHERE skill_version_id = ?1 ORDER BY relative_path",
                    )?;
                    let rows = stmt.query_map(params![id], |row| {
                        Ok(WikiSkillFile {
                            relative_path: row.get(0)?,
                            media_type: row.get(1)?,
                            content_text: row.get(2)?,
                            content_hash: row.get(3)?,
                            size_bytes: row.get(4)?,
                        })
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                let origin = conn
                    .query_row(
                        "SELECT repository_url, source_path, source_ref, license_spdx,
                                attribution, adaptation_notes, reviewed_at
                         FROM skill_version_origins WHERE skill_version_id = ?1",
                        params![id],
                        |row| {
                            Ok(WikiSkillOrigin {
                                repository_url: row.get(0)?,
                                source_path: row.get(1)?,
                                source_ref: row.get(2)?,
                                license_spdx: row.get(3)?,
                                attribution: row.get(4)?,
                                adaptation_notes: row.get(5)?,
                                reviewed_at: row.get(6)?,
                            })
                        },
                    )
                    .optional()?;
                let latest_evaluation = latest_skill_evaluation(conn, &id)?;
                let latest_benchmark = latest_skill_benchmark(conn, &id)?;
                versions.push(WikiSkillVersion {
                    id,
                    revision,
                    content_hash,
                    release_state,
                    parent_version_id,
                    changelog,
                    created_at,
                    files,
                    origin,
                    latest_evaluation,
                    latest_benchmark,
                });
            }
            Ok((current_version_id, versions))
        })?;
        Ok(WikiSkillDetail {
            skill,
            current_version_id,
            versions,
        })
    }

    pub fn evaluate_wiki_skill_version(
        &self,
        skill_id: &str,
        version_id: &str,
        suite_slug: &str,
    ) -> Result<WikiSkillEvaluationRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (content, current_version_id, skill_slug) = conn
                .query_row(
                    "SELECT sf.content_text, s.current_version_id, s.slug
                     FROM skill_versions sv
                     JOIN skills s ON s.id = sv.skill_id
                     JOIN skill_files sf ON sf.skill_version_id = sv.id
                                        AND sf.relative_path = 'SKILL.md'
                     WHERE sv.id = ?1 AND sv.skill_id = ?2",
                    params![version_id, skill_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(version_id.to_string()))?;
            let current_version_id = current_version_id.ok_or_else(|| {
                BrainError::Internal("Skill 当前版本不存在，数据库状态已损坏".to_string())
            })?;
            let expected_suite = match skill_slug.as_str() {
                "book-ingest" => Some("semantic-ingest"),
                "book-query" => Some("grounded-query"),
                "book-research" => Some("evidence-research"),
                "book-presentation" => Some("evidence-presentation"),
                _ => None,
            };
            if expected_suite.is_some_and(|expected| expected != suite_slug) {
                return Err(BrainError::KnowledgeValidation(format!(
                    "Skill {skill_slug} 必须使用固定评测集 {}",
                    expected_suite.unwrap_or_default()
                )));
            }
            let (suite_id, suite_name, pass_score) = conn
                .query_row(
                    "SELECT id, name, pass_score FROM skill_evaluation_suites WHERE slug = ?1",
                    params![suite_slug],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, f64>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation(format!(
                        "未知的 Skill 固定评测集: {suite_slug}"
                    ))
                })?;
            let cases = load_skill_evaluation_cases(conn, &suite_id)?;
            let (score, findings) = score_skill_instructions(&content, &cases);
            let baseline_score = if current_version_id == version_id {
                None
            } else {
                let baseline_content = conn.query_row(
                    "SELECT content_text FROM skill_files
                     WHERE skill_version_id = ?1 AND relative_path = 'SKILL.md'",
                    params![current_version_id],
                    |row| row.get::<_, String>(0),
                )?;
                Some(score_skill_instructions(&baseline_content, &cases).0)
            };
            let passed = score + f64::EPSILON >= pass_score;
            let findings_json = serde_json::to_string(&findings).map_err(|error| {
                BrainError::Internal(format!("Skill 评测结果序列化失败: {error}"))
            })?;
            let run_id = stable_id(
                "skill-evaluation",
                &format!("{version_id}:{suite_id}:{now}:{score}"),
            );
            conn.execute(
                "INSERT INTO skill_evaluation_runs
                 (id, skill_version_id, suite_id, baseline_version_id, score, baseline_score,
                  passed, findings_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    run_id,
                    version_id,
                    suite_id,
                    if current_version_id == version_id {
                        None
                    } else {
                        Some(current_version_id)
                    },
                    score,
                    baseline_score,
                    passed,
                    findings_json,
                    now,
                ],
            )?;
            Ok(WikiSkillEvaluationRun {
                id: run_id,
                skill_version_id: version_id.to_string(),
                suite_id,
                suite_name,
                score,
                baseline_score,
                passed,
                findings,
                created_at: now,
            })
        })
    }

    pub fn prepare_wiki_skill_benchmark(
        &self,
        skill_id: &str,
        version_id: &str,
        suite_slug: &str,
        base_id: &str,
        runtime_profile_id: &str,
        model: &str,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        self.get_active_base(base_id)?;
        let now = Utc::now().to_rfc3339();
        let run_id = uuid::Uuid::new_v4().to_string();
        self.db.transaction(|conn| {
            let (release_state, current_version_id, skill_slug) = conn
                .query_row(
                    "SELECT sv.release_state, s.current_version_id, s.slug
                     FROM skill_versions sv JOIN skills s ON s.id = sv.skill_id
                     WHERE sv.id = ?1 AND sv.skill_id = ?2",
                    params![version_id, skill_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(version_id.to_string()))?;
            if release_state != "candidate" {
                return Err(BrainError::KnowledgeValidation(
                    "只有候选 Skill 版本可以运行真实模型基准".to_string(),
                ));
            }
            let structurally_passed = conn
                .query_row(
                    "SELECT passed FROM skill_evaluation_runs
                     WHERE skill_version_id = ?1 ORDER BY created_at DESC LIMIT 1",
                    params![version_id],
                    |row| row.get::<_, bool>(0),
                )
                .optional()?
                .unwrap_or(false);
            if !structurally_passed {
                return Err(BrainError::KnowledgeValidation(
                    "候选版本必须先通过固定评测才能运行真实模型基准".to_string(),
                ));
            }
            let baseline_version_id = current_version_id.ok_or_else(|| {
                BrainError::Internal("Skill 当前版本不存在，数据库状态已损坏".to_string())
            })?;
            if baseline_version_id == version_id {
                return Err(BrainError::KnowledgeValidation(
                    "候选版本不能与当前基线版本相同".to_string(),
                ));
            }
            let expected_suite = expected_skill_suite(&skill_slug).ok_or_else(|| {
                BrainError::KnowledgeValidation("自定义 Skill 暂不支持内置真实模型基准".to_string())
            })?;
            if expected_suite != suite_slug {
                return Err(BrainError::KnowledgeValidation(format!(
                    "Skill {skill_slug} 必须使用固定基准集 {expected_suite}"
                )));
            }
            let (suite_id, total_cases) = conn
                .query_row(
                    "SELECT ses.id, COUNT(sbc.id)
                     FROM skill_evaluation_suites ses
                     LEFT JOIN skill_benchmark_cases sbc ON sbc.suite_id = ses.id
                     WHERE ses.slug = ?1 GROUP BY ses.id",
                    params![suite_slug],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation(format!("未知的 Skill 基准集: {suite_slug}"))
                })?;
            if total_cases == 0 {
                return Err(BrainError::KnowledgeValidation(
                    "真实模型基准集为空，无法运行".to_string(),
                ));
            }
            let in_progress = conn.query_row(
                "SELECT COUNT(*) FROM skill_benchmark_runs
                 WHERE skill_version_id = ?1 AND status IN ('queued', 'running')",
                params![version_id],
                |row| row.get::<_, i64>(0),
            )?;
            if in_progress > 0 {
                return Err(BrainError::KnowledgeValidation(
                    "该候选版本已有正在运行的真实模型基准".to_string(),
                ));
            }
            conn.execute(
                "INSERT INTO skill_benchmark_runs
                 (id, skill_id, skill_version_id, baseline_version_id, suite_id,
                  knowledge_base_id, runtime_profile_id, model, status, total_cases,
                  completed_cases, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, 0, ?10)",
                params![
                    run_id,
                    skill_id,
                    version_id,
                    baseline_version_id,
                    suite_id,
                    base_id,
                    runtime_profile_id,
                    model,
                    total_cases,
                    now,
                ],
            )?;
            Ok(())
        })?;
        self.get_wiki_skill_benchmark(&run_id)
    }

    pub fn start_wiki_skill_benchmark(
        &self,
        run_id: &str,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE skill_benchmark_runs SET status = 'running', started_at = ?2, error = NULL
                 WHERE id = ?1 AND status = 'queued'",
                params![run_id, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "真实模型基准不在等待执行状态".to_string(),
            ));
        }
        self.get_wiki_skill_benchmark(run_id)
    }

    pub fn mark_wiki_skill_benchmark_candidate_complete(
        &self,
        run_id: &str,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE skill_benchmark_runs SET completed_cases = total_cases
                 WHERE id = ?1 AND status = 'running'",
                params![run_id],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "真实模型基准不在执行状态".to_string(),
            ));
        }
        self.get_wiki_skill_benchmark(run_id)
    }

    pub fn load_wiki_skill_benchmark_cases(
        &self,
        suite_id: &str,
    ) -> Result<Vec<WikiSkillBenchmarkCase>, BrainError> {
        self.db
            .with_connection(|conn| load_skill_benchmark_cases(conn, suite_id))
    }

    pub fn wiki_skill_version_instructions(&self, version_id: &str) -> Result<String, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT content_text FROM skill_files
                 WHERE skill_version_id = ?1 AND relative_path = 'SKILL.md'",
                params![version_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(version_id.to_string()))
        })
    }

    pub fn complete_wiki_skill_benchmark(
        &self,
        run_id: &str,
        completion: WikiSkillBenchmarkCompletion<'_>,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let score_delta = completion.candidate_score - completion.baseline_score;
        self.db.transaction(|conn| {
            let (status, pass_score, total_cases) = conn
                .query_row(
                    "SELECT sbr.status, ses.pass_score, sbr.total_cases
                     FROM skill_benchmark_runs sbr
                     JOIN skill_evaluation_suites ses ON ses.id = sbr.suite_id
                     WHERE sbr.id = ?1",
                    params![run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(run_id.to_string()))?;
            if status != "running" {
                return Err(BrainError::KnowledgeValidation(
                    "只能完成正在运行的真实模型基准".to_string(),
                ));
            }
            let candidate_results = completion
                .results
                .iter()
                .filter(|result| result.variant == "candidate")
                .count() as i64;
            let baseline_results = completion
                .results
                .iter()
                .filter(|result| result.variant == "baseline")
                .count() as i64;
            let candidate_passed_cases = completion
                .results
                .iter()
                .filter(|result| result.variant == "candidate" && result.passed)
                .count() as i64;
            if candidate_results != total_cases || baseline_results != total_cases {
                return Err(BrainError::KnowledgeValidation(
                    "真实模型基准结果数量与固定样例不一致".to_string(),
                ));
            }
            for result in completion.results {
                let citations_json = serde_json::to_string(&result.citations).map_err(|error| {
                    BrainError::Internal(format!("Skill 基准引用序列化失败: {error}"))
                })?;
                let metrics_json = serde_json::to_string(&result.metrics).map_err(|error| {
                    BrainError::Internal(format!("Skill 基准指标序列化失败: {error}"))
                })?;
                let agent_run_id = if result.variant == "candidate" {
                    completion.candidate_agent_run_id
                } else {
                    completion.baseline_agent_run_id
                };
                conn.execute(
                    "INSERT INTO skill_benchmark_case_results
                     (run_id, case_id, variant, agent_run_id, response_text, citations_json,
                      metrics_json, score, passed, error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        run_id,
                        result.case_id,
                        result.variant,
                        agent_run_id,
                        result.response_text,
                        citations_json,
                        metrics_json,
                        result.score,
                        result.passed,
                        result.error,
                    ],
                )?;
            }
            let passed = completion.candidate_score + f64::EPSILON >= pass_score
                && score_delta + f64::EPSILON >= -0.03
                && candidate_passed_cases as f64 / total_cases as f64 + f64::EPSILON >= 0.8;
            let metrics_json = serde_json::to_string(completion.metrics).map_err(|error| {
                BrainError::Internal(format!("Skill 基准汇总序列化失败: {error}"))
            })?;
            conn.execute(
                "UPDATE skill_benchmark_runs
                 SET status = 'completed', completed_cases = total_cases,
                     candidate_score = ?2, baseline_score = ?3, score_delta = ?4,
                     passed = ?5, metrics_json = ?6, completed_at = ?7, error = NULL
                 WHERE id = ?1",
                params![
                    run_id,
                    completion.candidate_score.clamp(0.0, 1.0),
                    completion.baseline_score.clamp(0.0, 1.0),
                    score_delta,
                    i64::from(passed),
                    metrics_json,
                    now,
                ],
            )?;
            Ok(())
        })?;
        self.get_wiki_skill_benchmark(run_id)
    }

    pub fn fail_wiki_skill_benchmark(
        &self,
        run_id: &str,
        error: &str,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE skill_benchmark_runs
                 SET status = 'failed', error = ?2, completed_at = ?3
                 WHERE id = ?1 AND status IN ('queued', 'running')",
                params![run_id, error, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "真实模型基准已经结束".to_string(),
            ));
        }
        self.get_wiki_skill_benchmark(run_id)
    }

    pub fn get_wiki_skill_benchmark(
        &self,
        run_id: &str,
    ) -> Result<WikiSkillBenchmarkRun, BrainError> {
        self.db.with_connection(|conn| {
            load_skill_benchmark_run(conn, run_id)?
                .ok_or_else(|| BrainError::KnowledgeNotFound(format!("Skill 基准运行 {run_id}")))
        })
    }

    pub fn publish_wiki_skill_version(
        &self,
        skill_id: &str,
        version_id: &str,
    ) -> Result<WikiSkillDetail, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (release_state, current_version_id) = conn
                .query_row(
                    "SELECT sv.release_state, s.current_version_id
                     FROM skill_versions sv JOIN skills s ON s.id = sv.skill_id
                     WHERE sv.id = ?1 AND sv.skill_id = ?2",
                    params![version_id, skill_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(version_id.to_string()))?;
            if release_state != "candidate" {
                return Err(BrainError::KnowledgeValidation(
                    "只有候选版本可以执行发布".to_string(),
                ));
            }
            let passed = conn
                .query_row(
                    "SELECT passed FROM skill_evaluation_runs
                     WHERE skill_version_id = ?1 ORDER BY created_at DESC LIMIT 1",
                    params![version_id],
                    |row| row.get::<_, bool>(0),
                )
                .optional()?
                .unwrap_or(false);
            if !passed {
                return Err(BrainError::KnowledgeValidation(
                    "候选版本必须先通过固定评测才能发布".to_string(),
                ));
            }
            let benchmark = conn
                .query_row(
                    "SELECT status = 'completed' AND passed = 1, baseline_version_id
                     FROM skill_benchmark_runs
                     WHERE skill_version_id = ?1 ORDER BY created_at DESC LIMIT 1",
                    params![version_id],
                    |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((benchmark_passed, baseline_version_id)) = benchmark else {
                return Err(BrainError::KnowledgeValidation(
                    "候选版本必须先通过真实模型基准才能发布".to_string(),
                ));
            };
            if !benchmark_passed {
                return Err(BrainError::KnowledgeValidation(
                    "候选版本必须先通过真实模型基准才能发布".to_string(),
                ));
            }
            if current_version_id.as_deref() != Some(baseline_version_id.as_str()) {
                return Err(BrainError::KnowledgeValidation(
                    "当前 Skill 版本已变化，请重新运行真实模型基准".to_string(),
                ));
            }
            conn.execute(
                "UPDATE skill_versions SET release_state = 'published' WHERE id = ?1",
                params![version_id],
            )?;
            conn.execute(
                "UPDATE skills SET current_version_id = ?2, updated_at = ?3 WHERE id = ?1",
                params![skill_id, version_id, now],
            )?;
            conn.execute(
                "DELETE FROM skill_versions WHERE skill_id = ?1 AND id <> ?2",
                params![skill_id, version_id],
            )?;
            Ok(())
        })?;
        self.get_wiki_skill_detail(skill_id, None)
    }

    pub fn rollback_wiki_skill_version(
        &self,
        skill_id: &str,
        version_id: &str,
    ) -> Result<WikiSkillDetail, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (release_state, current_version_id) = conn
                .query_row(
                    "SELECT sv.release_state, s.current_version_id
                     FROM skill_versions sv JOIN skills s ON s.id = sv.skill_id
                     WHERE sv.id = ?1 AND sv.skill_id = ?2",
                    params![version_id, skill_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(version_id.to_string()))?;
            if release_state != "published" {
                return Err(BrainError::KnowledgeValidation(
                    "只能回滚到已通过发布流程的历史版本".to_string(),
                ));
            }
            if current_version_id.as_deref() == Some(version_id) {
                return Err(BrainError::KnowledgeValidation(
                    "目标版本已经是当前版本".to_string(),
                ));
            }
            conn.execute(
                "UPDATE skills SET current_version_id = ?2, updated_at = ?3 WHERE id = ?1",
                params![skill_id, version_id, now],
            )?;
            conn.execute(
                "DELETE FROM skill_versions WHERE skill_id = ?1 AND id <> ?2",
                params![skill_id, version_id],
            )?;
            Ok(())
        })?;
        self.get_wiki_skill_detail(skill_id, None)
    }

    pub fn enabled_wiki_skills(
        &self,
        base_id: &str,
        usage_scope: &str,
    ) -> Result<Vec<WikiSkill>, BrainError> {
        validate_skill_scope(usage_scope)?;
        Ok(self
            .list_wiki_skills(Some(base_id))?
            .into_iter()
            .filter(|skill| {
                skill.enabled
                    && skill.status == "ready"
                    && (skill.usage_scope == "all"
                        || skill.usage_scope == usage_scope
                        || (skill.usage_scope == "both"
                            && matches!(usage_scope, "qa" | "research")))
            })
            .collect())
    }

    pub fn save_custom_wiki_skill(
        &self,
        skill_id: Option<&str>,
        slug: &str,
        name: &str,
        description: &str,
        instructions: &str,
        expected_revision: Option<i64>,
    ) -> Result<WikiSkill, BrainError> {
        let slug = validate_skill_slug(slug)?;
        let name = name.trim();
        let description = description.trim();
        let instructions = instructions.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(BrainError::KnowledgeValidation(
                "Skill 名称不能为空且不能超过 100 个字符".to_string(),
            ));
        }
        if description.chars().count() > 500 {
            return Err(BrainError::KnowledgeValidation(
                "Skill 描述不能超过 500 个字符".to_string(),
            ));
        }
        if instructions.is_empty() || instructions.chars().count() > 12_000 {
            return Err(BrainError::KnowledgeValidation(
                "Skill 指令不能为空且不能超过 12000 个字符".to_string(),
            ));
        }

        let id = skill_id
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let existing = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT s.source_type, sv.revision, sv.content_hash
                 FROM skills s
                 JOIN skill_versions sv ON sv.id = s.current_version_id
                 WHERE s.id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(Into::into)
        })?;
        let (revision, existing_hash) = match existing {
            Some((source_type, revision, content_hash)) => {
                if source_type != "custom" {
                    return Err(BrainError::KnowledgeValidation(
                        "内置 Skill 不允许直接修改".to_string(),
                    ));
                }
                if expected_revision != Some(revision) {
                    return Err(BrainError::KnowledgeValidation(
                        "Skill 已被其他操作修改，请刷新后重试".to_string(),
                    ));
                }
                (revision + 1, Some(content_hash))
            }
            None if skill_id.is_some() => {
                return Err(BrainError::KnowledgeNotFound(id));
            }
            None => (1, None),
        };
        let content = build_skill_document(&slug, name, description, instructions)?;
        let content_hash = stable_id("skill-content", &content);
        if existing_hash.as_deref() == Some(content_hash.as_str()) {
            return self
                .list_wiki_skills(None)?
                .into_iter()
                .find(|skill| skill.id == id)
                .ok_or(BrainError::KnowledgeNotFound(id));
        }
        let version_id = stable_id("skill-version", &format!("{id}:{revision}:{content_hash}"));
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let duplicate: Option<String> = conn
                .query_row(
                    "SELECT id FROM skills WHERE slug = ?1 AND id <> ?2",
                    params![slug, id],
                    |row| row.get(0),
                )
                .optional()?;
            if duplicate.is_some() {
                return Err(BrainError::KnowledgeValidation(
                    "Skill 标识已存在".to_string(),
                ));
            }
            if revision == 1 {
                conn.execute(
                    "INSERT INTO skills
                     (id, slug, name, description, source_type, status, permissions_json,
                      requirements_json, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'custom', 'ready', '[\"knowledge.read\"]',
                             '[]', ?5, ?5)",
                    params![id, slug, name, description, now],
                )?;
            }
            let parent_version_id = if revision > 1 {
                conn.query_row(
                    "SELECT current_version_id FROM skills WHERE id = ?1",
                    params![id],
                    |row| row.get::<_, Option<String>>(0),
                )?
            } else {
                None
            };
            conn.execute(
                "INSERT INTO skill_versions
                 (id, skill_id, revision, content_hash, release_state, parent_version_id,
                  changelog, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    version_id,
                    id,
                    revision,
                    content_hash,
                    "published",
                    parent_version_id,
                    if revision == 1 {
                        "初始版本"
                    } else {
                        "用户编辑产生的候选版本"
                    },
                    now
                ],
            )?;
            conn.execute(
                "INSERT INTO skill_files
                 (skill_version_id, relative_path, media_type, content_text, content_hash,
                  size_bytes)
                 VALUES (?1, 'SKILL.md', 'text/markdown', ?2, ?3, ?4)",
                params![version_id, content, content_hash, content.len() as i64,],
            )?;
            conn.execute(
                "UPDATE skills
                 SET slug = ?2, name = ?3, description = ?4, status = 'ready',
                     current_version_id = ?5, updated_at = ?6
                 WHERE id = ?1",
                params![id, slug, name, description, version_id, now],
            )?;
            conn.execute(
                "DELETE FROM skill_versions WHERE skill_id = ?1 AND id <> ?2",
                params![id, version_id],
            )?;
            Ok(())
        })?;
        self.list_wiki_skills(None)?
            .into_iter()
            .find(|skill| skill.id == id)
            .ok_or(BrainError::KnowledgeNotFound(id))
    }

    pub fn import_custom_wiki_skill(
        &self,
        slug: &str,
        name: &str,
        description: &str,
        files: &[(String, String)],
    ) -> Result<WikiSkill, BrainError> {
        let slug = validate_skill_slug(slug)?;
        let name = name.trim();
        let description = description.trim();
        if name.is_empty() || name.chars().count() > 100 || description.chars().count() > 500 {
            return Err(BrainError::KnowledgeValidation(
                "导入的 Skill 名称或描述超出限制".to_string(),
            ));
        }
        if files.is_empty() || !files.iter().any(|(path, _)| path == "SKILL.md") {
            return Err(BrainError::KnowledgeValidation(
                "导入包根目录必须包含 SKILL.md".to_string(),
            ));
        }
        let mut normalized = files.to_vec();
        normalized.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, content) in &normalized {
            if path.is_empty()
                || path.starts_with('/')
                || path.split('/').any(|part| part.is_empty() || part == "..")
                || content.len() > 512 * 1024
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "Skill 包含不安全或过大的资源: {path}"
                )));
            }
        }
        let fingerprint = normalized
            .iter()
            .map(|(path, content)| format!("{path}\0{content}"))
            .collect::<Vec<_>>()
            .join("\0");
        let content_hash = stable_id("skill-package", &fingerprint);
        if let Some(existing_id) = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT s.id FROM skills s
                 JOIN skill_versions sv ON sv.id = s.current_version_id
                 WHERE sv.content_hash = ?1",
                params![content_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })? {
            return self
                .list_wiki_skills(None)?
                .into_iter()
                .find(|skill| skill.id == existing_id)
                .ok_or(BrainError::KnowledgeNotFound(existing_id));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let version_id = stable_id("skill-version", &format!("{id}:1:{content_hash}"));
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let duplicate = conn.query_row(
                "SELECT COUNT(*) FROM skills WHERE slug = ?1",
                params![slug],
                |row| row.get::<_, i64>(0),
            )?;
            if duplicate > 0 {
                return Err(BrainError::KnowledgeValidation(
                    "Skill 标识已存在；请在 SKILL.md 中更换 name 或 slug".to_string(),
                ));
            }
            conn.execute(
                "INSERT INTO skills
                 (id, slug, name, description, source_type, status, permissions_json,
                  requirements_json, current_version_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'custom', 'ready', '[\"knowledge.read\"]',
                         '[]', ?5, ?6, ?6)",
                params![id, slug, name, description, version_id, now],
            )?;
            conn.execute(
                "INSERT INTO skill_versions (id, skill_id, revision, content_hash, created_at)
                 VALUES (?1, ?2, 1, ?3, ?4)",
                params![version_id, id, content_hash, now],
            )?;
            for (path, content) in &normalized {
                let file_hash = stable_id("skill-file", content);
                let media_type = if path.ends_with(".md") {
                    "text/markdown"
                } else {
                    "text/plain"
                };
                conn.execute(
                    "INSERT INTO skill_files
                     (skill_version_id, relative_path, media_type, content_text, content_hash,
                      size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        version_id,
                        path,
                        media_type,
                        content,
                        file_hash,
                        content.len() as i64
                    ],
                )?;
            }
            Ok(())
        })?;
        self.list_wiki_skills(None)?
            .into_iter()
            .find(|skill| skill.id == id)
            .ok_or(BrainError::KnowledgeNotFound(id))
    }

    pub fn set_wiki_skill_binding(
        &self,
        base_id: &str,
        skill_id: &str,
        enabled: bool,
        usage_scope: &str,
    ) -> Result<WikiSkill, BrainError> {
        self.get_base(base_id)?;
        validate_skill_scope(usage_scope)?;
        let status = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT status FROM skills WHERE id = ?1",
                params![skill_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        match status.as_deref() {
            None => return Err(BrainError::KnowledgeNotFound(skill_id.to_string())),
            Some("ready") => {}
            Some(_) if enabled => {
                return Err(BrainError::KnowledgeValidation(
                    "当前 Skill 未通过校验，不能启用".to_string(),
                ));
            }
            Some(_) => {}
        }
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_base_skill_bindings
                 (knowledge_base_id, skill_id, usage_scope, enabled, revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5)
                 ON CONFLICT(knowledge_base_id, skill_id) DO UPDATE SET
                    usage_scope = excluded.usage_scope,
                    enabled = excluded.enabled,
                    revision = knowledge_base_skill_bindings.revision + 1,
                    updated_at = excluded.updated_at",
                params![base_id, skill_id, usage_scope, i64::from(enabled), now],
            )?;
            Ok(())
        })?;
        self.list_wiki_skills(Some(base_id))?
            .into_iter()
            .find(|skill| skill.id == skill_id)
            .ok_or_else(|| BrainError::KnowledgeNotFound(skill_id.to_string()))
    }

    pub fn list_config_documents(
        &self,
        base_id: Option<&str>,
    ) -> Result<Vec<ConfigDocument>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, knowledge_base_id, scope, name, content_md, revision, updated_at
                 FROM knowledge_config_documents
                 WHERE (scope = 'global' AND knowledge_base_id IS NULL)
                    OR (?1 IS NOT NULL AND knowledge_base_id = ?1)
                 ORDER BY scope, name",
            )?;
            let rows = stmt.query_map(params![base_id], |row| {
                Ok(ConfigDocument {
                    id: row.get(0)?,
                    knowledge_base_id: row.get(1)?,
                    scope: row.get(2)?,
                    name: row.get(3)?,
                    content_md: row.get(4)?,
                    revision: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn save_config_document(
        &self,
        document_id: &str,
        content_md: &str,
        expected_revision: i64,
    ) -> Result<ConfigDocument, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_config_documents
                 SET content_md = ?2, revision = revision + 1, updated_at = ?3
                 WHERE id = ?1 AND revision = ?4",
                params![document_id, content_md, now, expected_revision],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "配置已被其他操作修改，请刷新后重试".to_string(),
            ));
        }
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, knowledge_base_id, scope, name, content_md, revision, updated_at
                 FROM knowledge_config_documents WHERE id = ?1",
                params![document_id],
                |row| {
                    Ok(ConfigDocument {
                        id: row.get(0)?,
                        knowledge_base_id: row.get(1)?,
                        scope: row.get(2)?,
                        name: row.get(3)?,
                        content_md: row.get(4)?,
                        revision: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .map_err(Into::into)
        })
    }

    pub fn list_runtime_profiles(&self) -> Result<Vec<RuntimeProfile>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT r.id, r.name, r.runtime, r.executable,
                        COALESCE(p.model, r.model), r.provider_id, r.enabled, r.revision, r.updated_at,
                        p.display_name, p.api_protocol, p.base_url, p.model,
                        p.credential_source, p.api_key_env, p.api_key_configured,
                        p.enabled, p.revision, p.updated_at
                 FROM agent_runtime_profiles r
                 LEFT JOIN llm_provider_profiles p ON p.id = r.provider_id
                 WHERE r.runtime = 'deepseek_harness'
                 ORDER BY r.name",
            )?;
            let rows = stmt.query_map([], |row| {
                let provider_id = row.get::<_, Option<String>>(5)?;
                let provider_config = match (
                    provider_id.as_ref(),
                    row.get::<_, Option<String>>(9)?,
                ) {
                    (Some(provider_id), Some(display_name)) => Some(ModelProviderProfile {
                        provider_id: provider_id.clone(),
                        display_name,
                        api_protocol: row.get(10)?,
                        base_url: row.get(11)?,
                        model: row.get(12)?,
                        credential_source: row.get(13)?,
                        api_key_env: row.get(14)?,
                        api_key_configured: row.get::<_, i64>(15)? != 0,
                        enabled: row.get::<_, i64>(16)? != 0,
                        revision: row.get(17)?,
                        updated_at: row.get(18)?,
                    }),
                    _ => None,
                };
                Ok(RuntimeProfile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    runtime: row.get(2)?,
                    executable: row.get(3)?,
                    model: row.get(4)?,
                    provider_id,
                    enabled: row.get::<_, i64>(6)? != 0,
                    revision: row.get(7)?,
                    updated_at: row.get(8)?,
                    provider_config,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn list_model_provider_profiles(&self) -> Result<Vec<ModelProviderProfile>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, display_name, api_protocol, base_url, model, credential_source,
                        api_key_env, api_key_configured, enabled, revision, updated_at
                 FROM llm_provider_profiles
                 ORDER BY enabled DESC, updated_at DESC, display_name COLLATE NOCASE",
            )?;
            let rows = stmt.query_map([], map_model_provider_profile)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn get_model_provider_profile(
        &self,
        provider_id: &str,
    ) -> Result<ModelProviderProfile, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, display_name, api_protocol, base_url, model, credential_source,
                        api_key_env, api_key_configured, enabled, revision, updated_at
                 FROM llm_provider_profiles WHERE id = ?1",
                params![provider_id],
                map_model_provider_profile,
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(provider_id.to_string()))
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_model_provider_profile(
        &self,
        provider_id: &str,
        display_name: &str,
        api_protocol: &str,
        base_url: &str,
        model: &str,
        credential_source: &str,
        api_key_env: &str,
        api_key_configured: bool,
        enabled: bool,
        expected_revision: i64,
    ) -> Result<ModelProviderProfile, BrainError> {
        let provider = validate_model_provider_profile(
            provider_id,
            display_name,
            api_protocol,
            base_url,
            model,
            credential_source,
            api_key_env,
        )?;
        let now = Utc::now().to_rfc3339();
        if expected_revision == 0 {
            self.db.with_connection(|conn| {
                conn.execute(
                    "INSERT INTO llm_provider_profiles
                     (id, display_name, api_protocol, base_url, model, credential_source,
                      api_key_env, api_key_configured, enabled, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                    params![
                        provider.provider_id,
                        provider.display_name,
                        provider.api_protocol,
                        provider.base_url,
                        provider.model,
                        provider.credential_source,
                        provider.api_key_env,
                        i64::from(api_key_configured),
                        i64::from(enabled),
                        now,
                    ],
                )?;
                Ok(())
            })?;
        } else {
            let updated = self.db.with_connection(|conn| {
                Ok(conn.execute(
                    "UPDATE llm_provider_profiles
                     SET display_name = ?2, api_protocol = ?3, base_url = ?4, model = ?5,
                         credential_source = ?6, api_key_env = ?7, api_key_configured = ?8,
                         enabled = ?9, revision = revision + 1, updated_at = ?10
                     WHERE id = ?1 AND revision = ?11",
                    params![
                        provider.provider_id,
                        provider.display_name,
                        provider.api_protocol,
                        provider.base_url,
                        provider.model,
                        provider.credential_source,
                        provider.api_key_env,
                        i64::from(api_key_configured),
                        i64::from(enabled),
                        now,
                        expected_revision,
                    ],
                )?)
            })?;
            if updated == 0 {
                return Err(BrainError::KnowledgeValidation(
                    "模型供应商配置已变化，请刷新后重试".to_string(),
                ));
            }
        }
        self.get_model_provider_profile(provider_id)
    }

    pub fn delete_model_provider_profile(
        &self,
        provider_id: &str,
        expected_revision: i64,
    ) -> Result<(), BrainError> {
        let deleted = self.db.with_connection(|conn| {
            let active_runtime: Option<String> = conn
                .query_row(
                    "SELECT name FROM agent_runtime_profiles WHERE provider_id = ?1 LIMIT 1",
                    params![provider_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(runtime_name) = active_runtime {
                return Err(BrainError::KnowledgeValidation(format!(
                    "模型供应商正在被 {runtime_name} 使用，请先切换 Runtime 配置"
                )));
            }
            Ok(conn.execute(
                "DELETE FROM llm_provider_profiles WHERE id = ?1 AND revision = ?2",
                params![provider_id, expected_revision],
            )?)
        })?;
        if deleted == 0 {
            return Err(BrainError::KnowledgeValidation(
                "模型供应商正在被 Runtime 使用、已被删除或版本已变化".to_string(),
            ));
        }
        Ok(())
    }

    pub fn save_runtime_profile(
        &self,
        profile_id: &str,
        executable: &str,
        model: &str,
        provider_id: Option<&str>,
        enabled: bool,
        expected_revision: i64,
    ) -> Result<RuntimeProfile, BrainError> {
        if profile_id != "runtime-deepseek-harness" {
            return Err(BrainError::KnowledgeValidation(
                "书籍知识库仅支持 DeepSeek Harness 运行时".to_string(),
            ));
        }
        if executable.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "运行时可执行文件不能为空".to_string(),
            ));
        }
        let provider_id = provider_id.map(str::trim).filter(|value| !value.is_empty());
        if let Some(provider_id) = provider_id {
            let provider = self.get_model_provider_profile(provider_id)?;
            if !provider.enabled {
                return Err(BrainError::KnowledgeValidation(
                    "不能选择已停用的模型供应商".to_string(),
                ));
            }
        }
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE agent_runtime_profiles
                 SET executable = ?2, model = ?3, provider_id = ?4, config_json = '{}', enabled = ?5,
                     revision = revision + 1, updated_at = ?6
                 WHERE id = ?1 AND revision = ?7",
                params![
                    profile_id,
                    executable.trim(),
                    model.trim(),
                    provider_id,
                    i64::from(enabled),
                    now,
                    expected_revision,
                ],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "运行配置已变化，请刷新后重试".to_string(),
            ));
        }
        self.list_runtime_profiles()?
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| BrainError::KnowledgeNotFound(profile_id.to_string()))
    }

    pub fn start_agent_run(
        &self,
        base_id: &str,
        runtime: &str,
        task_type: &str,
        input: &serde_json::Value,
    ) -> Result<AgentRun, BrainError> {
        self.get_active_base(base_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_json = serde_json::to_string(input)
            .map_err(|error| BrainError::Internal(format!("Agent 输入序列化失败: {error}")))?;
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO agent_runs
                 (id, knowledge_base_id, runtime, task_type, status, input_json,
                  started_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?6)",
                params![id, base_id, runtime, task_type, input_json, now],
            )?;
            insert_agent_run_event(
                conn,
                &id,
                "run.started",
                Some("starting"),
                "Agent 运行已启动",
                "{}",
                &now,
            )?;
            Ok(())
        })?;
        self.get_agent_run(&id)
    }

    pub fn issue_agent_run_capability(
        &self,
        run_id: &str,
        knowledge_base_ids: &[String],
        allowed_tools: &[String],
        ttl_seconds: i64,
    ) -> Result<IssuedAgentCapability, BrainError> {
        let run = self.get_agent_run(run_id)?;
        if run.status != "running" {
            return Err(BrainError::KnowledgeValidation(
                "只能为正在运行的 Agent 签发能力令牌".to_string(),
            ));
        }
        if knowledge_base_ids.is_empty() || allowed_tools.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "Agent 能力令牌必须包含知识库范围和工具权限".to_string(),
            ));
        }
        let mut scopes = knowledge_base_ids.to_vec();
        scopes.sort();
        scopes.dedup();
        if let Some(run_base_id) = run.knowledge_base_id.as_deref() {
            if !scopes.iter().any(|base_id| base_id == run_base_id) {
                return Err(BrainError::KnowledgeValidation(
                    "Agent 能力范围必须包含运行所属知识库".to_string(),
                ));
            }
        }
        for base_id in &scopes {
            self.get_base(base_id)?;
        }
        let mut tools = allowed_tools
            .iter()
            .map(|tool| tool.trim().to_string())
            .filter(|tool| !tool.is_empty())
            .collect::<Vec<_>>();
        tools.sort();
        tools.dedup();
        if tools.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "Agent 能力令牌必须包含工具权限".to_string(),
            ));
        }

        use sha2::{Digest, Sha256};
        let token = format!(
            "obw_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = (now + chrono::Duration::seconds(ttl_seconds.max(0))).to_rfc3339();
        let created_at = now.to_rfc3339();
        let tools_json = serde_json::to_string(&tools)
            .map_err(|error| BrainError::Internal(format!("Agent 工具权限序列化失败: {error}")))?;
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO agent_run_capabilities
                 (id, run_id, token_hash, allowed_tools_json, expires_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, run_id, token_hash, tools_json, expires_at, created_at],
            )?;
            for base_id in &scopes {
                conn.execute(
                    "INSERT INTO agent_run_capability_scopes
                     (capability_id, knowledge_base_id) VALUES (?1, ?2)",
                    params![id, base_id],
                )?;
            }
            Ok(())
        })?;
        Ok(IssuedAgentCapability {
            token,
            run_id: run_id.to_string(),
            expires_at,
        })
    }

    pub fn validate_agent_run_capability(
        &self,
        token: &str,
        tool: &str,
    ) -> Result<AgentCapabilityGrant, BrainError> {
        let grant = self.validate_agent_run_token(token)?;
        if !grant.allowed_tools.iter().any(|allowed| allowed == tool) {
            return Err(BrainError::KnowledgeValidation(format!(
                "Agent 能力令牌没有 {tool} 工具权限"
            )));
        }
        Ok(grant)
    }

    pub fn validate_agent_run_token(
        &self,
        token: &str,
    ) -> Result<AgentCapabilityGrant, BrainError> {
        use sha2::{Digest, Sha256};
        if !token.starts_with("obw_") || token.len() < 32 {
            return Err(BrainError::KnowledgeValidation(
                "Agent 能力令牌无效".to_string(),
            ));
        }
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let capability = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT arc.id, arc.run_id, arc.allowed_tools_json, arc.expires_at,
                        arc.revoked_at, ar.status
                 FROM agent_run_capabilities arc
                 JOIN agent_runs ar ON ar.id = arc.run_id
                 WHERE arc.token_hash = ?1",
                params![token_hash],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(Into::into)
        })?;
        let (capability_id, run_id, tools_json, expires_at, revoked_at, run_status) = capability
            .ok_or_else(|| BrainError::KnowledgeValidation("Agent 能力令牌无效".to_string()))?;
        if revoked_at.is_some() || run_status != "running" {
            return Err(BrainError::KnowledgeValidation(
                "Agent 能力令牌无效或已撤销".to_string(),
            ));
        }
        let expiry = chrono::DateTime::parse_from_rfc3339(&expires_at)
            .map_err(|error| BrainError::Internal(format!("能力令牌过期时间损坏: {error}")))?
            .with_timezone(&Utc);
        if expiry <= Utc::now() {
            return Err(BrainError::KnowledgeValidation(
                "Agent 能力令牌已过期".to_string(),
            ));
        }
        let allowed_tools = parse_string_list(&tools_json, "Agent 工具权限")?;
        let knowledge_base_ids = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT knowledge_base_id FROM agent_run_capability_scopes
                 WHERE capability_id = ?1 ORDER BY knowledge_base_id",
            )?;
            let rows = stmt.query_map(params![capability_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })?;
        Ok(AgentCapabilityGrant {
            run_id,
            knowledge_base_ids,
            allowed_tools,
            expires_at,
        })
    }

    pub fn authorize_external_research_request(
        &self,
        run_id: &str,
        host: &str,
    ) -> Result<ExternalResearchAuthorization, BrainError> {
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        if host.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "外部资料地址缺少域名".to_string(),
            ));
        }
        self.db.transaction(|conn| {
            let (base_id, input_json, run_status) = conn
                .query_row(
                    "SELECT knowledge_base_id, input_json, status FROM agent_runs WHERE id = ?1",
                    params![run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(run_id.to_string()))?;
            if run_status != "running" {
                return Err(BrainError::KnowledgeValidation(
                    "Agent 运行已经结束，外部研究授权已失效".to_string(),
                ));
            }
            let input: serde_json::Value = serde_json::from_str(&input_json)
                .map_err(|error| BrainError::Internal(format!("Agent 运行输入损坏: {error}")))?;
            let task_id = input
                .get("knowledge_task_id")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation(
                        "只有用户创建的研究任务可以访问外部资料".to_string(),
                    )
                })?;
            let (task_base_id, enabled, domains_json, request_limit, requests_used) = conn
                .query_row(
                    "SELECT knowledge_base_id, external_research_enabled,
                            external_domains_json, external_request_limit, external_requests_used
                     FROM knowledge_tasks WHERE id = ?1",
                    params![task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, bool>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| BrainError::KnowledgeNotFound(task_id.to_string()))?;
            if !enabled || task_base_id != base_id {
                return Err(BrainError::KnowledgeValidation(
                    "该研究任务未授权访问外部资料".to_string(),
                ));
            }
            let domains = parse_string_list(&domains_json, "外部研究域名")?;
            if !domains
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "域名 {host} 不在本任务的外部研究授权范围内"
                )));
            }
            if requests_used >= request_limit {
                return Err(BrainError::KnowledgeValidation(
                    "该研究任务的外部请求额度已用完".to_string(),
                ));
            }
            let updated = conn.execute(
                "UPDATE knowledge_tasks
                 SET external_requests_used = external_requests_used + 1, updated_at = ?2
                 WHERE id = ?1 AND external_research_enabled = 1
                   AND external_requests_used < external_request_limit",
                params![task_id, Utc::now().to_rfc3339()],
            )?;
            if updated != 1 {
                return Err(BrainError::KnowledgeValidation(
                    "该研究任务的外部请求额度已用完".to_string(),
                ));
            }
            Ok(ExternalResearchAuthorization {
                run_id: run_id.to_string(),
                knowledge_base_id: base_id,
                host,
                request_number: requests_used + 1,
                request_limit,
            })
        })
    }

    pub fn revoke_agent_run_capabilities(&self, run_id: &str) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_run_capabilities SET revoked_at = ?2
                 WHERE run_id = ?1 AND revoked_at IS NULL",
                params![run_id, now],
            )?;
            Ok(())
        })
    }

    pub fn append_agent_run_event(
        &self,
        run_id: &str,
        event_type: &str,
        phase: Option<&str>,
        message: &str,
        payload: &serde_json::Value,
    ) -> Result<AgentRunEvent, BrainError> {
        validate_agent_event_type(event_type)?;
        let payload_json = serde_json::to_string(payload)
            .map_err(|error| BrainError::Internal(format!("Agent 事件序列化失败: {error}")))?;
        let now = Utc::now().to_rfc3339();
        let sequence = self.db.transaction(|conn| {
            insert_agent_run_event(
                conn,
                run_id,
                event_type,
                phase,
                message,
                &payload_json,
                &now,
            )
        })?;
        self.list_agent_run_events(run_id)?
            .into_iter()
            .find(|event| event.sequence == sequence)
            .ok_or_else(|| BrainError::Internal("Agent 事件写入后无法读取".to_string()))
    }

    pub fn list_agent_run_events(&self, run_id: &str) -> Result<Vec<AgentRunEvent>, BrainError> {
        self.get_agent_run(run_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT run_id, sequence, event_type, phase, message, payload_json, created_at
                 FROM agent_run_events WHERE run_id = ?1 ORDER BY sequence",
            )?;
            let rows = stmt.query_map(params![run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            rows.map(|row| {
                let row = row?;
                Ok(AgentRunEvent {
                    run_id: row.0,
                    sequence: row.1,
                    event_type: row.2,
                    phase: row.3,
                    message: row.4,
                    payload: serde_json::from_str(&row.5).map_err(|error| {
                        BrainError::Internal(format!("Agent 事件载荷解析失败: {error}"))
                    })?,
                    created_at: row.6,
                })
            })
            .collect::<Result<Vec<_>, BrainError>>()
        })
    }

    pub fn save_agent_run_inspection(
        &self,
        run_id: &str,
        prompt_text: &str,
        skill_snapshots: &serde_json::Value,
        config_snapshots: &serde_json::Value,
        tool_names: &[String],
        evidence_refs: &serde_json::Value,
    ) -> Result<(), BrainError> {
        self.get_agent_run(run_id)?;
        let skill_snapshots_json = serde_json::to_string(skill_snapshots)
            .map_err(|error| BrainError::Internal(format!("Skill 运行快照序列化失败: {error}")))?;
        let config_snapshots_json = serde_json::to_string(config_snapshots)
            .map_err(|error| BrainError::Internal(format!("配置运行快照序列化失败: {error}")))?;
        let tool_names_json = serde_json::to_string(tool_names)
            .map_err(|error| BrainError::Internal(format!("工具白名单序列化失败: {error}")))?;
        let evidence_refs_json = serde_json::to_string(evidence_refs)
            .map_err(|error| BrainError::Internal(format!("证据引用序列化失败: {error}")))?;
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO agent_run_inspections
                 (run_id, prompt_text, prompt_hash, prompt_characters, skill_snapshots_json,
                  config_snapshots_json, tool_names_json, evidence_refs_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    run_id,
                    prompt_text,
                    stable_id("prompt", prompt_text),
                    prompt_text.chars().count() as i64,
                    skill_snapshots_json,
                    config_snapshots_json,
                    tool_names_json,
                    evidence_refs_json,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_agent_run_inspection(&self, run_id: &str) -> Result<AgentRunInspection, BrainError> {
        let run = self.get_agent_run(run_id)?;
        let events = self.list_agent_run_events(run_id)?;
        let snapshot = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT run_id, prompt_text, prompt_hash, prompt_characters,
                        skill_snapshots_json, config_snapshots_json, tool_names_json,
                        evidence_refs_json, created_at
                 FROM agent_run_inspections WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(Into::into)
        })?;
        let snapshot = snapshot
            .map(|row| -> Result<AgentRunInspectionSnapshot, BrainError> {
                Ok(AgentRunInspectionSnapshot {
                    run_id: row.0,
                    prompt_text: row.1,
                    prompt_hash: row.2,
                    prompt_characters: row.3,
                    skill_snapshots: serde_json::from_str(&row.4).map_err(|error| {
                        BrainError::Internal(format!("Skill 运行快照解析失败: {error}"))
                    })?,
                    config_snapshots: serde_json::from_str(&row.5).map_err(|error| {
                        BrainError::Internal(format!("配置运行快照解析失败: {error}"))
                    })?,
                    tool_names: parse_string_list(&row.6, "Agent 工具白名单")?,
                    evidence_refs: serde_json::from_str(&row.7).map_err(|error| {
                        BrainError::Internal(format!("证据引用解析失败: {error}"))
                    })?,
                    created_at: row.8,
                })
            })
            .transpose()?;
        Ok(AgentRunInspection {
            run,
            events,
            snapshot,
        })
    }

    pub fn complete_agent_run(
        &self,
        run_id: &str,
        output: &serde_json::Value,
    ) -> Result<AgentRun, BrainError> {
        self.complete_agent_run_with_usage(run_id, output, &AgentTokenUsage::default())
    }

    pub fn complete_agent_run_with_usage(
        &self,
        run_id: &str,
        output: &serde_json::Value,
        usage: &AgentTokenUsage,
    ) -> Result<AgentRun, BrainError> {
        if !matches!(
            usage.usage_source.as_str(),
            "unavailable" | "estimated" | "measured"
        ) {
            return Err(BrainError::KnowledgeValidation(
                "未知的 Token 用量来源".to_string(),
            ));
        }
        if [
            usage.input_tokens,
            usage.output_tokens,
            usage.reasoning_tokens,
            usage.cache_read_tokens,
            usage.cache_write_tokens,
        ]
        .iter()
        .any(|value| *value < 0)
        {
            return Err(BrainError::KnowledgeValidation(
                "Token 用量不能为负数".to_string(),
            ));
        }
        let output_json = serde_json::to_string(output)
            .map_err(|error| BrainError::Internal(format!("Agent 输出序列化失败: {error}")))?;
        let now = Utc::now().to_rfc3339();
        let updated = self.db.transaction(|conn| {
            let updated = conn.execute(
                "UPDATE agent_runs
                 SET status = 'completed', output_json = ?2, error = NULL, finished_at = ?3,
                     input_tokens = ?4, output_tokens = ?5, reasoning_tokens = ?6,
                     cache_read_tokens = ?7, cache_write_tokens = ?8, usage_source = ?9
                 WHERE id = ?1 AND status = 'running'",
                params![
                    run_id,
                    output_json,
                    now,
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.reasoning_tokens,
                    usage.cache_read_tokens,
                    usage.cache_write_tokens,
                    usage.usage_source,
                ],
            )?;
            if updated > 0 {
                insert_agent_run_event(
                    conn,
                    run_id,
                    "run.completed",
                    Some("completed"),
                    "Agent 运行已完成",
                    "{}",
                    &now,
                )?;
                conn.execute(
                    "UPDATE agent_run_capabilities SET revoked_at = ?2
                     WHERE run_id = ?1 AND revoked_at IS NULL",
                    params![run_id, now],
                )?;
            }
            Ok(updated)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "Agent 运行不存在或已结束".to_string(),
            ));
        }
        self.get_agent_run(run_id)
    }

    pub fn get_agent_usage_stats(
        &self,
        start_date: &str,
        end_date: &str,
        caller: Option<&str>,
    ) -> Result<AgentUsageStats, BrainError> {
        let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d").map_err(|_| {
            BrainError::KnowledgeValidation("start_date 必须为 YYYY-MM-DD".to_string())
        })?;
        let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d").map_err(|_| {
            BrainError::KnowledgeValidation("end_date 必须为 YYYY-MM-DD".to_string())
        })?;
        if start > end {
            return Err(BrainError::KnowledgeValidation(
                "Token 统计开始日期不能晚于结束日期".to_string(),
            ));
        }
        if let Some(value) = caller {
            if !matches!(value, "knowledge_qa" | "knowledge_task") {
                return Err(BrainError::KnowledgeValidation(
                    "caller 仅支持 knowledge_qa 或 knowledge_task".to_string(),
                ));
            }
        }

        let rows = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT date(finished_at, 'localtime'),
                        CASE
                            WHEN task_type = 'knowledge_qa' THEN 'knowledge_qa'
                            WHEN task_type LIKE 'knowledge_task_%' THEN 'knowledge_task'
                            ELSE task_type
                        END,
                        input_tokens, output_tokens, reasoning_tokens,
                        cache_read_tokens, cache_write_tokens, usage_source
                 FROM agent_runs
                 WHERE status = 'completed'
                   AND date(finished_at, 'localtime') BETWEEN ?1 AND ?2
                   AND (?3 IS NULL OR
                        CASE
                            WHEN task_type = 'knowledge_qa' THEN 'knowledge_qa'
                            WHEN task_type LIKE 'knowledge_task_%' THEN 'knowledge_task'
                            ELSE task_type
                        END = ?3)
                 ORDER BY finished_at ASC",
            )?;
            let mapped = stmt.query_map(params![start_date, end_date, caller], |row| {
                Ok(AgentUsageRow {
                    date: row.get(0)?,
                    caller: row.get(1)?,
                    input_tokens: row.get(2)?,
                    output_tokens: row.get(3)?,
                    reasoning_tokens: row.get(4)?,
                    cache_read_tokens: row.get(5)?,
                    cache_write_tokens: row.get(6)?,
                    usage_source: row.get(7)?,
                })
            })?;
            mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })?;

        Ok(aggregate_agent_usage(start_date, end_date, caller, rows))
    }

    pub fn fail_agent_run(&self, run_id: &str, error: &str) -> Result<AgentRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let payload = serde_json::to_string(&serde_json::json!({ "error": error })).map_err(
            |serialize_error| {
                BrainError::Internal(format!("Agent 失败事件序列化失败: {serialize_error}"))
            },
        )?;
        let updated = self.db.transaction(|conn| {
            let updated = conn.execute(
                "UPDATE agent_runs
                 SET status = 'failed', error = ?2, finished_at = ?3
                 WHERE id = ?1 AND status = 'running'",
                params![run_id, error, now],
            )?;
            if updated > 0 {
                insert_agent_run_event(
                    conn,
                    run_id,
                    "run.failed",
                    Some("failed"),
                    "Agent 运行失败",
                    &payload,
                    &now,
                )?;
                conn.execute(
                    "UPDATE agent_run_capabilities SET revoked_at = ?2
                     WHERE run_id = ?1 AND revoked_at IS NULL",
                    params![run_id, now],
                )?;
            }
            Ok(updated)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "Agent 运行不存在或已结束".to_string(),
            ));
        }
        self.get_agent_run(run_id)
    }

    pub fn cancel_agent_run(&self, run_id: &str) -> Result<AgentRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.transaction(|conn| {
            let updated = conn.execute(
                "UPDATE agent_runs
                 SET status = 'cancelled', error = NULL, finished_at = ?2
                 WHERE id = ?1 AND status = 'running'",
                params![run_id, now],
            )?;
            if updated > 0 {
                insert_agent_run_event(
                    conn,
                    run_id,
                    "run.cancelled",
                    Some("cancelled"),
                    "Agent 运行已取消",
                    "{}",
                    &now,
                )?;
                conn.execute(
                    "UPDATE agent_run_capabilities SET revoked_at = ?2
                     WHERE run_id = ?1 AND revoked_at IS NULL",
                    params![run_id, now],
                )?;
            }
            Ok(updated)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "Agent 运行不存在或已结束".to_string(),
            ));
        }
        self.get_agent_run(run_id)
    }

    pub fn get_agent_run(&self, run_id: &str) -> Result<AgentRun, BrainError> {
        let raw = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, knowledge_base_id, runtime, task_type, status, input_json,
                        output_json, error, started_at, finished_at, created_at
                 FROM agent_runs WHERE id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()
            .map_err(Into::into)
        })?;
        let raw = raw.ok_or_else(|| BrainError::KnowledgeNotFound(run_id.to_string()))?;
        Ok(AgentRun {
            id: raw.0,
            knowledge_base_id: raw.1,
            runtime: raw.2,
            task_type: raw.3,
            status: raw.4,
            input: serde_json::from_str(&raw.5)
                .map_err(|error| BrainError::Internal(format!("Agent 输入解析失败: {error}")))?,
            output: raw
                .6
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|error| BrainError::Internal(format!("Agent 输出解析失败: {error}")))?,
            error: raw.7,
            started_at: raw.8,
            finished_at: raw.9,
            created_at: raw.10,
        })
    }

    pub fn get_agent_run_observation(&self, run_id: &str) -> Result<serde_json::Value, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT input_tokens, output_tokens, reasoning_tokens,
                        cache_read_tokens, cache_write_tokens, usage_source,
                        CAST(ROUND((julianday(finished_at) - julianday(started_at)) * 86400000) AS INTEGER)
                 FROM agent_runs WHERE id = ?1 AND status = 'completed'",
                params![run_id],
                |row| {
                    Ok(serde_json::json!({
                        "input_tokens": row.get::<_, i64>(0)?,
                        "output_tokens": row.get::<_, i64>(1)?,
                        "reasoning_tokens": row.get::<_, i64>(2)?,
                        "cache_read_tokens": row.get::<_, i64>(3)?,
                        "cache_write_tokens": row.get::<_, i64>(4)?,
                        "usage_source": row.get::<_, String>(5)?,
                        "duration_ms": row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    }))
                },
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(format!("已完成 Agent Run {run_id}")))
        })
    }

    pub fn get_latest_completed_task_run(
        &self,
        task_id: &str,
    ) -> Result<Option<AgentRun>, BrainError> {
        let run_id = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id
                 FROM agent_runs
                 WHERE status = 'completed'
                   AND task_type LIKE 'knowledge_task_%'
                   AND task_type != 'knowledge_task_presentation_plan'
                   AND json_extract(input_json, '$.knowledge_task_id') = ?1
                 ORDER BY finished_at DESC, created_at DESC
                 LIMIT 1",
                params![task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        run_id.map(|run_id| self.get_agent_run(&run_id)).transpose()
    }

    pub fn get_latest_task_run(&self, task_id: &str) -> Result<Option<AgentRun>, BrainError> {
        self.get_task(task_id)?;
        let run_id = self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id
                 FROM agent_runs
                 WHERE task_type LIKE 'knowledge_task_%'
                   AND json_extract(input_json, '$.knowledge_task_id') = ?1
                 ORDER BY created_at DESC
                 LIMIT 1",
                params![task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        run_id.map(|run_id| self.get_agent_run(&run_id)).transpose()
    }
}

#[derive(Clone, Debug)]
struct AgentUsageRow {
    date: String,
    caller: String,
    input_tokens: i64,
    output_tokens: i64,
    reasoning_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    usage_source: String,
}

fn aggregate_agent_usage(
    start_date: &str,
    end_date: &str,
    caller: Option<&str>,
    rows: Vec<AgentUsageRow>,
) -> AgentUsageStats {
    let mut totals = AgentUsageTotals::default();
    let mut daily = BTreeMap::<String, AgentUsageTotals>::new();
    let mut by_caller = BTreeMap::<String, AgentUsageTotals>::new();
    let mut has_measured = false;
    let mut has_estimated = false;

    for row in rows {
        has_measured |= row.usage_source == "measured";
        has_estimated |= row.usage_source == "estimated";
        accumulate_usage(&mut totals, &row);
        accumulate_usage(daily.entry(row.date.clone()).or_default(), &row);
        accumulate_usage(by_caller.entry(row.caller.clone()).or_default(), &row);
    }

    let usage_source = match (has_measured, has_estimated) {
        (true, true) => "mixed",
        (true, false) => "measured",
        (false, true) => "estimated",
        (false, false) => "unavailable",
    }
    .to_string();

    AgentUsageStats {
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        caller: caller.map(str::to_string),
        usage_source,
        totals,
        daily: daily
            .into_iter()
            .map(|(date, totals)| AgentUsagePoint { date, totals })
            .collect(),
        by_caller: by_caller
            .into_iter()
            .map(|(caller, totals)| AgentUsageCaller { caller, totals })
            .collect(),
    }
}

fn accumulate_usage(total: &mut AgentUsageTotals, row: &AgentUsageRow) {
    if row.usage_source == "unavailable" {
        total.unreported_runs += 1;
        return;
    }
    total.runs += 1;
    total.input_tokens += row.input_tokens;
    total.output_tokens += row.output_tokens;
    total.reasoning_tokens += row.reasoning_tokens;
    total.cache_read_tokens += row.cache_read_tokens;
    total.cache_write_tokens += row.cache_write_tokens;
    total.total_tokens += row.input_tokens + row.output_tokens;
}

fn load_message_evidence(
    conn: &rusqlite::Connection,
    message_id: &str,
) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT kmc.entry_id,
                COALESCE(kmc.knowledge_base_id_snapshot, ke.knowledge_base_id),
                COALESCE(kmc.entry_type_snapshot, ke.entry_type),
                COALESCE(kmc.slug_snapshot, ke.slug),
                COALESCE(kmc.title_snapshot, ke.title),
                COALESCE(kmc.summary_snapshot, ke.summary),
                COALESCE(kmc.status_snapshot, ke.status),
                COALESCE(kmc.confidence_snapshot, ke.confidence),
                COALESCE(kmc.source_path_snapshot, sd.relative_path),
                COALESCE(kmc.updated_at_snapshot, ke.updated_at)
         FROM knowledge_message_citations kmc
         LEFT JOIN knowledge_entries ke ON ke.id = kmc.entry_id
         LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
         WHERE kmc.message_id = ?1
         ORDER BY kmc.ordinal",
    )?;
    let entries = stmt.query_map(params![message_id], |row| {
        Ok(KnowledgeEntrySummary {
            id: row.get(0)?,
            knowledge_base_id: row.get(1)?,
            entry_type: row.get(2)?,
            slug: row.get(3)?,
            title: row.get(4)?,
            summary: row.get(5)?,
            status: row.get(6)?,
            confidence: row.get(7)?,
            source_path: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;
    entries.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn map_entry_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeEntrySummary> {
    Ok(KnowledgeEntrySummary {
        id: row.get(0)?,
        knowledge_base_id: row.get(1)?,
        entry_type: row.get(2)?,
        slug: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        status: row.get(6)?,
        confidence: row.get(7)?,
        source_path: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn load_entry_summary_in_base(
    conn: &rusqlite::Connection,
    base_id: &str,
    entry_id: &str,
) -> Result<KnowledgeEntrySummary, BrainError> {
    conn.query_row(
        "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
         FROM knowledge_entries ke
         LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
         WHERE ke.id = ?1 AND ke.knowledge_base_id = ?2",
        params![entry_id, base_id],
        map_entry_summary,
    )
    .optional()?
    .ok_or_else(|| BrainError::KnowledgeNotFound(entry_id.to_string()))
}

fn conversation_title(question: &str) -> String {
    let title = question.trim().chars().take(42).collect::<String>();
    if question.trim().chars().count() > 42 {
        format!("{title}…")
    } else {
        title
    }
}

fn map_model_provider_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelProviderProfile> {
    Ok(ModelProviderProfile {
        provider_id: row.get(0)?,
        display_name: row.get(1)?,
        api_protocol: row.get(2)?,
        base_url: row.get(3)?,
        model: row.get(4)?,
        credential_source: row.get(5)?,
        api_key_env: row.get(6)?,
        api_key_configured: row.get::<_, i64>(7)? != 0,
        enabled: row.get::<_, i64>(8)? != 0,
        revision: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn validate_model_provider_profile(
    provider_id: &str,
    display_name: &str,
    api_protocol: &str,
    base_url: &str,
    model: &str,
    credential_source: &str,
    api_key_env: &str,
) -> Result<ModelProviderProfile, BrainError> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty()
        || provider_id.len() > 64
        || !provider_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(BrainError::KnowledgeValidation(
            "供应商 ID 只能包含字母、数字、点、短横线和下划线".to_string(),
        ));
    }
    let display_name = display_name.trim();
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return Err(BrainError::KnowledgeValidation(
            "供应商名称不能为空且不能超过 100 个字符".to_string(),
        ));
    }
    if model.trim().is_empty() || model.chars().count() > 200 {
        return Err(BrainError::KnowledgeValidation(
            "自定义供应商必须填写有效的模型 ID".to_string(),
        ));
    }
    let api_protocol = api_protocol.trim();
    if !matches!(
        api_protocol,
        "openai-completions" | "openai-responses" | "anthropic-messages"
    ) {
        return Err(BrainError::KnowledgeValidation(
            "不支持的模型 API 协议".to_string(),
        ));
    }
    let base_url = base_url.trim().trim_end_matches('/');
    let parsed_url = reqwest::Url::parse(base_url)
        .map_err(|_| BrainError::KnowledgeValidation("模型 API Base URL 格式不正确".to_string()))?;
    if !matches!(parsed_url.scheme(), "http" | "https")
        || parsed_url.host_str().is_none()
        || !parsed_url.username().is_empty()
        || parsed_url.password().is_some()
    {
        return Err(BrainError::KnowledgeValidation(
            "模型 API Base URL 必须是无内嵌凭据的 HTTP(S) 地址".to_string(),
        ));
    }
    let credential_source = credential_source.trim();
    if !matches!(credential_source, "keychain" | "environment") {
        return Err(BrainError::KnowledgeValidation(
            "凭据来源必须是系统凭据库或环境变量".to_string(),
        ));
    }
    let api_key_env = api_key_env.trim();
    if credential_source == "environment" {
        let mut env_characters = api_key_env.chars();
        let valid_env = env_characters
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            && env_characters
                .all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !valid_env || api_key_env.len() > 128 {
            return Err(BrainError::KnowledgeValidation(
                "API Key 环境变量名只能包含字母、数字和下划线，且不能以数字开头".to_string(),
            ));
        }
    }

    Ok(ModelProviderProfile {
        provider_id: provider_id.to_string(),
        display_name: display_name.to_string(),
        api_protocol: api_protocol.to_string(),
        base_url: base_url.to_string(),
        model: model.trim().to_string(),
        credential_source: credential_source.to_string(),
        api_key_env: api_key_env.to_string(),
        api_key_configured: false,
        enabled: true,
        revision: 0,
        updated_at: String::new(),
    })
}

fn map_base_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeBaseSummary> {
    let book_path = row.get::<_, String>(3)?;
    Ok(KnowledgeBaseSummary {
        id: row.get(0)?,
        book_id: row.get(1)?,
        book_name: row.get(2)?,
        source_available: Path::new(&book_path).is_dir(),
        book_path,
        book_kind: row.get(4)?,
        book_description: row.get(5)?,
        book_category: row.get(6)?,
        lifecycle: row.get(7)?,
        sync_state: row.get(8)?,
        compile_mode: row.get(9)?,
        compile_state: row.get(10)?,
        compile_error: row.get(11)?,
        health_state: row.get(12)?,
        last_error: row.get(13)?,
        last_synced_at: row.get(14)?,
        last_scanned_at: row.get(15)?,
        last_compiled_at: row.get(16)?,
        compile_processed_sources: row.get(17)?,
        compile_total_sources: row.get(18)?,
        compile_phase: row.get(19)?,
        compile_message: row.get(20)?,
        compile_current_batch: row.get(21)?,
        compile_total_batches: row.get(22)?,
        compile_active_run_id: row.get(23)?,
        compile_change_set_id: row.get(24)?,
        compile_started_at: row.get(25)?,
        compile_heartbeat_at: row.get(26)?,
        compile_cancel_requested: row.get(27)?,
        pending_review_count: row.get(28)?,
        source_count: row.get(29)?,
        entry_count: row.get(30)?,
        claim_count: row.get(31)?,
        task_count: row.get(32)?,
    })
}

fn map_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeArtifact> {
    let validation_details_raw = row.get::<_, String>(13)?;
    let validation_details = serde_json::from_str(&validation_details_raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(KnowledgeArtifact {
        id: row.get(0)?,
        knowledge_base_id: row.get(1)?,
        knowledge_task_id: row.get(2)?,
        agent_run_id: row.get(3)?,
        skill_id: row.get(4)?,
        artifact_type: row.get(5)?,
        title: row.get(6)?,
        relative_path: row.get(7)?,
        mime_type: row.get(8)?,
        content_hash: row.get(9)?,
        size_bytes: row.get(10)?,
        validation_state: row.get(11)?,
        validation_message: row.get(12)?,
        validation_details,
        created_at: row.get(14)?,
    })
}

fn map_knowledge_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeTask> {
    let raw_domains = row.get::<_, String>(13)?;
    let external_domains = serde_json::from_str(&raw_domains).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(KnowledgeTask {
        id: row.get(0)?,
        knowledge_base_id: row.get(1)?,
        book_name: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        task_type: row.get(5)?,
        status: row.get(6)?,
        result_summary: row.get(7)?,
        deliverable_type: row.get(8)?,
        artifact_state: row.get(9)?,
        knowledge_change_state: row.get(10)?,
        cancel_requested: row.get(11)?,
        external_research_enabled: row.get(12)?,
        external_domains,
        external_request_limit: row.get(14)?,
        external_requests_used: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn normalize_external_domains(domains: &[String]) -> Result<Vec<String>, BrainError> {
    if domains.len() > 20 {
        return Err(BrainError::KnowledgeValidation(
            "外部研究最多授权 20 个域名".to_string(),
        ));
    }
    let mut normalized = domains
        .iter()
        .map(|domain| domain.trim().trim_end_matches('.').to_ascii_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    let valid = normalized.iter().all(|domain| {
        domain.len() <= 253
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && domain.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
            })
    });
    if !valid {
        return Err(BrainError::KnowledgeValidation(
            "外部研究域名格式无效；请只填写 example.com 这样的域名".to_string(),
        ));
    }
    Ok(normalized)
}

fn default_book_config_documents() -> [(&'static str, &'static str); 3] {
    [
        (
            "knowledge.md",
            "# 知识建模\n\n优先保留可追溯事实，区分原文内容、推断与待核实观点。\n",
        ),
        (
            "questions.md",
            "# 问答规则\n\n回答必须引用当前书籍知识库中的来源；证据不足时明确说明。\n",
        ),
        (
            "tasks.md",
            "# 研究任务\n\n先制定步骤，再检索来源；所有写入以变更集形式提交。\n",
        ),
    ]
}

fn parse_string_list(raw: &str, label: &str) -> Result<Vec<String>, BrainError> {
    serde_json::from_str(raw)
        .map_err(|error| BrainError::Internal(format!("{label}解析失败: {error}")))
}

#[derive(Clone)]
struct SkillEvaluationCase {
    id: String,
    name: String,
    required_concepts: Vec<String>,
    forbidden_concepts: Vec<String>,
    weight: f64,
}

fn load_skill_evaluation_cases(
    conn: &rusqlite::Connection,
    suite_id: &str,
) -> Result<Vec<SkillEvaluationCase>, BrainError> {
    let mut statement = conn.prepare(
        "SELECT id, name, required_concepts_json, forbidden_concepts_json, weight
         FROM skill_evaluation_cases WHERE suite_id = ?1 ORDER BY ordinal",
    )?;
    let cases = statement
        .query_map(params![suite_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?
        .map(|row| {
            let (id, name, required, forbidden, weight) = row?;
            Ok(SkillEvaluationCase {
                id,
                name,
                required_concepts: parse_string_list(&required, "Skill 评测必需概念")?,
                forbidden_concepts: parse_string_list(&forbidden, "Skill 评测禁用概念")?,
                weight,
            })
        })
        .collect();
    cases
}

fn score_skill_instructions(
    content: &str,
    cases: &[SkillEvaluationCase],
) -> (f64, Vec<WikiSkillEvaluationFinding>) {
    let normalized = content.to_lowercase();
    let mut earned = 0.0;
    let total = cases.iter().map(|case| case.weight).sum::<f64>();
    let findings = cases
        .iter()
        .map(|case| {
            let missing_concepts = case
                .required_concepts
                .iter()
                .filter(|concept| !normalized.contains(&concept.to_lowercase()))
                .cloned()
                .collect::<Vec<_>>();
            let forbidden_concepts = case
                .forbidden_concepts
                .iter()
                .filter(|concept| normalized.contains(&concept.to_lowercase()))
                .cloned()
                .collect::<Vec<_>>();
            let passed = missing_concepts.is_empty() && forbidden_concepts.is_empty();
            if passed {
                earned += case.weight;
            }
            WikiSkillEvaluationFinding {
                case_id: case.id.clone(),
                name: case.name.clone(),
                passed,
                missing_concepts,
                forbidden_concepts,
                weight: case.weight,
            }
        })
        .collect::<Vec<_>>();
    let score = if total <= f64::EPSILON {
        0.0
    } else {
        (earned / total).clamp(0.0, 1.0)
    };
    (score, findings)
}

fn latest_skill_evaluation(
    conn: &rusqlite::Connection,
    version_id: &str,
) -> Result<Option<WikiSkillEvaluationRun>, BrainError> {
    let row = conn
        .query_row(
            "SELECT ser.id, ser.skill_version_id, ser.suite_id, ses.name, ser.score,
                    ser.baseline_score, ser.passed, ser.findings_json, ser.created_at
             FROM skill_evaluation_runs ser
             JOIN skill_evaluation_suites ses ON ses.id = ser.suite_id
             WHERE ser.skill_version_id = ?1 ORDER BY ser.created_at DESC LIMIT 1",
            params![version_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| {
        Ok(WikiSkillEvaluationRun {
            id: row.0,
            skill_version_id: row.1,
            suite_id: row.2,
            suite_name: row.3,
            score: row.4,
            baseline_score: row.5,
            passed: row.6,
            findings: serde_json::from_str(&row.7).map_err(|error| {
                BrainError::Internal(format!("Skill 评测结果解析失败: {error}"))
            })?,
            created_at: row.8,
        })
    })
    .transpose()
}

fn expected_skill_suite(skill_slug: &str) -> Option<&'static str> {
    match skill_slug {
        "book-ingest" => Some("semantic-ingest"),
        "book-query" => Some("grounded-query"),
        "book-research" => Some("evidence-research"),
        "book-presentation" => Some("evidence-presentation"),
        _ => None,
    }
}

fn load_skill_benchmark_cases(
    conn: &rusqlite::Connection,
    suite_id: &str,
) -> Result<Vec<WikiSkillBenchmarkCase>, BrainError> {
    let mut statement = conn.prepare(
        "SELECT id, suite_id, ordinal, name, scenario_type, fixture_json,
                expectations_json, weight
         FROM skill_benchmark_cases WHERE suite_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![suite_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
        ))
    })?;
    rows.map(|row| {
        let row = row?;
        Ok(WikiSkillBenchmarkCase {
            id: row.0,
            suite_id: row.1,
            ordinal: row.2,
            name: row.3,
            scenario_type: row.4,
            fixture: serde_json::from_str(&row.5).map_err(|error| {
                BrainError::Internal(format!("Skill 基准输入解析失败: {error}"))
            })?,
            expectations: serde_json::from_str(&row.6).map_err(|error| {
                BrainError::Internal(format!("Skill 基准期望解析失败: {error}"))
            })?,
            weight: row.7,
        })
    })
    .collect()
}

fn load_skill_benchmark_run(
    conn: &rusqlite::Connection,
    run_id: &str,
) -> Result<Option<WikiSkillBenchmarkRun>, BrainError> {
    let row = conn
        .query_row(
            "SELECT sbr.id, sbr.skill_id, sbr.skill_version_id, sbr.baseline_version_id,
                    sbr.suite_id, ses.name, sbr.knowledge_base_id, sbr.runtime_profile_id,
                    sbr.model, sbr.status, sbr.total_cases, sbr.completed_cases,
                    sbr.candidate_score, sbr.baseline_score, sbr.score_delta, sbr.passed,
                    sbr.metrics_json, sbr.error, sbr.started_at, sbr.completed_at, sbr.created_at
             FROM skill_benchmark_runs sbr
             JOIN skill_evaluation_suites ses ON ses.id = sbr.suite_id
             WHERE sbr.id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, Option<f64>>(12)?,
                    row.get::<_, Option<f64>>(13)?,
                    row.get::<_, Option<f64>>(14)?,
                    row.get::<_, bool>(15)?,
                    row.get::<_, String>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                    row.get::<_, Option<String>>(19)?,
                    row.get::<_, String>(20)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let metrics = serde_json::from_str(&row.16)
        .map_err(|error| BrainError::Internal(format!("Skill 基准汇总解析失败: {error}")))?;
    let mut statement = conn.prepare(
        "SELECT sbcr.case_id, sbc.name, sbcr.variant, sbcr.agent_run_id,
                sbcr.response_text, sbcr.citations_json, sbcr.metrics_json,
                sbcr.score, sbcr.passed, sbcr.error
         FROM skill_benchmark_case_results sbcr
         JOIN skill_benchmark_cases sbc ON sbc.id = sbcr.case_id
         WHERE sbcr.run_id = ?1
         ORDER BY sbcr.variant, sbc.ordinal",
    )?;
    let result_rows = statement.query_map(params![run_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
            row.get::<_, bool>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;
    let results = result_rows
        .map(|result| {
            let result = result?;
            Ok(WikiSkillBenchmarkCaseResult {
                case_id: result.0,
                name: result.1,
                variant: result.2,
                agent_run_id: result.3,
                response_text: result.4,
                citations: serde_json::from_str(&result.5).map_err(|error| {
                    BrainError::Internal(format!("Skill 基准引用解析失败: {error}"))
                })?,
                metrics: serde_json::from_str(&result.6).map_err(|error| {
                    BrainError::Internal(format!("Skill 基准指标解析失败: {error}"))
                })?,
                score: result.7,
                passed: result.8,
                error: result.9,
            })
        })
        .collect::<Result<Vec<_>, BrainError>>()?;
    Ok(Some(WikiSkillBenchmarkRun {
        id: row.0,
        skill_id: row.1,
        skill_version_id: row.2,
        baseline_version_id: row.3,
        suite_id: row.4,
        suite_name: row.5,
        knowledge_base_id: row.6,
        runtime_profile_id: row.7,
        model: row.8,
        status: row.9,
        total_cases: row.10,
        completed_cases: row.11,
        candidate_score: row.12,
        baseline_score: row.13,
        score_delta: row.14,
        passed: row.15,
        metrics,
        error: row.17,
        results,
        started_at: row.18,
        completed_at: row.19,
        created_at: row.20,
    }))
}

fn latest_skill_benchmark(
    conn: &rusqlite::Connection,
    version_id: &str,
) -> Result<Option<WikiSkillBenchmarkRun>, BrainError> {
    let run_id = conn
        .query_row(
            "SELECT id FROM skill_benchmark_runs
             WHERE skill_version_id = ?1 ORDER BY created_at DESC LIMIT 1",
            params![version_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    run_id
        .map(|run_id| load_skill_benchmark_run(conn, &run_id))
        .transpose()
        .map(Option::flatten)
}

fn validate_skill_scope(value: &str) -> Result<(), BrainError> {
    if matches!(value, "qa" | "research" | "both" | "ingest" | "all") {
        Ok(())
    } else {
        Err(BrainError::KnowledgeValidation(
            "Skill 使用范围仅支持 qa、research、both、ingest 或 all".to_string(),
        ))
    }
}

fn validate_skill_slug(value: &str) -> Result<String, BrainError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        });
    if valid {
        Ok(value.to_string())
    } else {
        Err(BrainError::KnowledgeValidation(
            "Skill 标识只能包含小写字母、数字和短横线，且不能以短横线开头或结尾".to_string(),
        ))
    }
}

fn build_skill_document(
    slug: &str,
    name: &str,
    description: &str,
    instructions: &str,
) -> Result<String, BrainError> {
    let quoted_name = serde_json::to_string(name)
        .map_err(|error| BrainError::Internal(format!("Skill 名称序列化失败: {error}")))?;
    let quoted_description = serde_json::to_string(description)
        .map_err(|error| BrainError::Internal(format!("Skill 描述序列化失败: {error}")))?;
    Ok(format!(
        "---\nname: {slug}\ndisplay_name: {quoted_name}\ndescription: {quoted_description}\n---\n\n# {name}\n\n{instructions}\n"
    ))
}

fn extract_skill_instructions(content: &str) -> String {
    let mut body = content.trim();
    if let Some(frontmatter) = body.strip_prefix("---\n") {
        if let Some((_, remainder)) = frontmatter.split_once("\n---\n") {
            body = remainder.trim();
        }
    }
    if let Some(remainder) = body.strip_prefix("# ") {
        body = remainder
            .split_once('\n')
            .map(|(_, instructions)| instructions.trim())
            .unwrap_or("");
    }
    body.to_string()
}

fn validate_agent_event_type(value: &str) -> Result<(), BrainError> {
    if matches!(
        value,
        "run.started"
            | "run.phase_changed"
            | "run.progress"
            | "run.text_delta"
            | "run.usage"
            | "run.tool_started"
            | "run.tool_finished"
            | "run.external_source_read"
            | "run.review_required"
            | "run.completed"
            | "run.failed"
            | "run.cancelled"
    ) {
        Ok(())
    } else {
        Err(BrainError::KnowledgeValidation(
            "未知的 Agent 运行事件类型".to_string(),
        ))
    }
}

fn insert_agent_run_event(
    conn: &rusqlite::Connection,
    run_id: &str,
    event_type: &str,
    phase: Option<&str>,
    message: &str,
    payload_json: &str,
    created_at: &str,
) -> Result<i64, BrainError> {
    let sequence = conn.query_row(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM agent_run_events WHERE run_id = ?1",
        params![run_id],
        |row| row.get::<_, i64>(0),
    )?;
    conn.execute(
        "INSERT INTO agent_run_events
         (run_id, sequence, event_type, phase, message, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            run_id,
            sequence,
            event_type,
            phase,
            message.trim(),
            payload_json,
            created_at,
        ],
    )?;
    Ok(sequence)
}

fn required_json_string<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, BrainError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("知识候选缺少有效字段 {key}")))
}

fn json_string_list(value: &serde_json::Value, key: &str) -> Result<Vec<String>, BrainError> {
    let Some(values) = value.get(key) else {
        return Ok(Vec::new());
    };
    let values = values
        .as_array()
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("知识候选字段 {key} 必须是数组")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation(format!(
                        "知识候选字段 {key} 只能包含非空字符串"
                    ))
                })
        })
        .collect()
}

fn validate_semantic_candidate(
    candidate: &serde_json::Value,
    current_span_ids: &HashSet<String>,
) -> Result<(), BrainError> {
    let entry_type = required_json_string(candidate, "entry_type")?;
    if !matches!(
        entry_type,
        "entity"
            | "concept"
            | "method"
            | "event"
            | "comparison"
            | "synthesis"
            | "question"
            | "answer"
            | "overview"
    ) {
        return Err(BrainError::KnowledgeValidation(format!(
            "不支持的语义条目类型: {entry_type}"
        )));
    }
    let slug = required_json_string(candidate, "slug")?;
    if slug.chars().count() > 120 || slug.contains(['/', '\\']) || slug.contains("..") {
        return Err(BrainError::KnowledgeValidation(
            "知识候选 slug 格式不安全".to_string(),
        ));
    }
    if let Some(classification) = candidate
        .get("_classification")
        .and_then(serde_json::Value::as_str)
    {
        if !matches!(classification, "new" | "update" | "disputed") {
            return Err(BrainError::KnowledgeValidation(
                "知识候选分类只能是 new、update 或 disputed".to_string(),
            ));
        }
    }
    for key in ["title", "summary", "content_md"] {
        let value = required_json_string(candidate, key)?;
        let limit = if key == "content_md" { 30_000 } else { 1_000 };
        if value.chars().count() > limit {
            return Err(BrainError::KnowledgeValidation(format!(
                "知识候选字段 {key} 超过长度限制"
            )));
        }
    }
    let citations = json_string_list(candidate, "citations")?;
    if citations.is_empty() {
        return Err(BrainError::KnowledgeValidation(
            "每个语义知识候选至少需要一个当前来源引用".to_string(),
        ));
    }
    if let Some(invalid) = citations
        .iter()
        .find(|citation| !current_span_ids.contains(*citation))
    {
        return Err(BrainError::KnowledgeValidation(format!(
            "知识候选引用不属于当前书籍版本: {invalid}"
        )));
    }
    let aliases = json_string_list(candidate, "aliases")?;
    if aliases.len() > 30 || aliases.iter().any(|alias| alias.chars().count() > 100) {
        return Err(BrainError::KnowledgeValidation(
            "知识候选别名数量或长度超过限制".to_string(),
        ));
    }
    if let Some(status) = candidate.get("status").and_then(serde_json::Value::as_str) {
        if !matches!(status, "draft" | "verified" | "archived") {
            return Err(BrainError::KnowledgeValidation(
                "知识候选状态只能是 draft、verified 或 archived".to_string(),
            ));
        }
    }
    if let Some(policy) = candidate
        .get("edit_policy")
        .and_then(serde_json::Value::as_str)
    {
        if !matches!(policy, "agent_managed" | "human_protected") {
            return Err(BrainError::KnowledgeValidation(
                "未知的知识候选编辑策略".to_string(),
            ));
        }
    }
    if let Some(claims) = candidate.get("claims") {
        let claims = claims.as_array().ok_or_else(|| {
            BrainError::KnowledgeValidation("知识候选 claims 必须是数组".to_string())
        })?;
        if claims.len() > 60 {
            return Err(BrainError::KnowledgeValidation(
                "单个条目的论断数量不能超过 60".to_string(),
            ));
        }
        for claim in claims {
            required_json_string(claim, "claim_text")?;
            let claim_citations = json_string_list(claim, "citations")?;
            if let Some(invalid) = claim_citations
                .iter()
                .find(|citation| !current_span_ids.contains(*citation))
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "论断引用不属于当前书籍版本: {invalid}"
                )));
            }
        }
    }
    Ok(())
}

fn semantic_candidate_citation_audit(
    candidate: &serde_json::Value,
) -> Result<(serde_json::Value, i64, i64, Vec<String>), BrainError> {
    let entry_citations = json_string_list(candidate, "citations")?;
    let mut explicit_claim_citations = 0_i64;
    let mut inherited_claims = 0_i64;
    let mut issues = Vec::new();
    if let Some(claims) = candidate
        .get("claims")
        .and_then(serde_json::Value::as_array)
    {
        for (index, claim) in claims.iter().enumerate() {
            let citations = json_string_list(claim, "citations")?;
            if citations.is_empty() {
                inherited_claims += 1;
                if entry_citations.is_empty() {
                    issues.push(format!("第 {} 条论断没有可继承的引用", index + 1));
                }
            } else {
                explicit_claim_citations += citations.len() as i64;
            }
        }
    }
    let effective_claim_citations =
        explicit_claim_citations + inherited_claims.saturating_mul(entry_citations.len() as i64);
    let audit = serde_json::json!({
        "passed": issues.is_empty(),
        "entry_citations": entry_citations.len(),
        "explicit_claim_citations": explicit_claim_citations,
        "inherited_claims": inherited_claims,
        "effective_claim_citations": effective_claim_citations,
        "issues": issues.clone(),
    });
    Ok((
        audit,
        entry_citations.len() as i64,
        effective_claim_citations,
        issues,
    ))
}

fn knowledge_change_impact(
    conn: &rusqlite::Connection,
    entry_id: &str,
    is_new: bool,
) -> Result<serde_json::Value, BrainError> {
    if is_new {
        return Ok(serde_json::json!({
            "entries": 1,
            "claims": 0,
            "relations": 0,
            "citations": 0,
            "relation_entry_ids": [],
        }));
    }
    let claims = conn.query_row(
        "SELECT COUNT(*) FROM knowledge_claims WHERE entry_id = ?1",
        params![entry_id],
        |row| row.get::<_, i64>(0),
    )?;
    let relations = conn.query_row(
        "SELECT COUNT(*) FROM knowledge_relations
         WHERE from_entry_id = ?1 OR to_entry_id = ?1",
        params![entry_id],
        |row| row.get::<_, i64>(0),
    )?;
    let citations = conn.query_row(
        "SELECT COUNT(*) FROM knowledge_citations kc
         WHERE kc.entry_id = ?1 OR kc.claim_id IN
               (SELECT id FROM knowledge_claims WHERE entry_id = ?1)",
        params![entry_id],
        |row| row.get::<_, i64>(0),
    )?;
    let relation_entry_ids = {
        let mut statement = conn.prepare(
            "SELECT DISTINCT CASE WHEN from_entry_id = ?1 THEN to_entry_id ELSE from_entry_id END
             FROM knowledge_relations WHERE from_entry_id = ?1 OR to_entry_id = ?1
             ORDER BY 1 LIMIT 50",
        )?;
        let rows = statement.query_map(params![entry_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(serde_json::json!({
        "entries": 1,
        "claims": claims,
        "relations": relations,
        "citations": citations,
        "relation_entry_ids": relation_entry_ids,
    }))
}

fn validate_change_revision(
    conn: &rusqlite::Connection,
    base_id: &str,
    change: &KnowledgeChange,
) -> Result<(), BrainError> {
    let current = conn
        .query_row(
            "SELECT revision FROM knowledge_entries
             WHERE id = ?1 AND knowledge_base_id = ?2",
            params![change.object_id, base_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    match (change.expected_revision, current) {
        (None, None) => Ok(()),
        (Some(expected), Some(current)) if expected == current => Ok(()),
        _ => Err(BrainError::KnowledgeValidation(format!(
            "知识条目 {} 已变化，变更集需要重新生成",
            change.object_id
        ))),
    }
}

fn apply_entry_change(
    conn: &rusqlite::Connection,
    change_set: &KnowledgeChangeSet,
    change: &KnowledgeChange,
    now: &str,
) -> Result<(), BrainError> {
    let candidate = &change.after;
    let entry_type = required_json_string(candidate, "entry_type")?;
    let slug = required_json_string(candidate, "slug")?;
    let title = required_json_string(candidate, "title")?;
    let summary = required_json_string(candidate, "summary")?;
    let content_md = required_json_string(candidate, "content_md")?;
    let aliases = serde_json::to_string(&json_string_list(candidate, "aliases")?)
        .map_err(|error| BrainError::Internal(format!("知识别名序列化失败: {error}")))?;
    let confidence = candidate
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .map(|value| value.clamp(0.0, 1.0));
    let status = candidate
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("draft");
    let edit_policy = candidate
        .get("edit_policy")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("agent_managed");

    if change.expected_revision.is_none() {
        conn.execute(
            "INSERT INTO knowledge_entries
             (id, knowledge_base_id, entry_type, slug, title, summary, content_md, status,
              confidence, aliases_json, edit_policy, created_by_run_id, updated_by_run_id,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13, ?13)",
            params![
                change.object_id,
                change_set.knowledge_base_id,
                entry_type,
                slug,
                title,
                summary,
                content_md,
                status,
                confidence,
                aliases,
                edit_policy,
                change_set.agent_run_id,
                now,
            ],
        )?;
    } else {
        conn.execute(
            "INSERT OR IGNORE INTO knowledge_entry_versions
             (id, entry_id, revision, title, summary, content_md, aliases_json, status,
              confidence, changed_by_run_id, created_at)
             SELECT 'entry-version-' || id || '-' || revision, id, revision, title, summary,
                    content_md, aliases_json, status, confidence, updated_by_run_id, updated_at
             FROM knowledge_entries WHERE id = ?1",
            params![change.object_id],
        )?;
        conn.execute(
            "UPDATE knowledge_entries
             SET entry_type = ?2, slug = ?3, title = ?4, summary = ?5, content_md = ?6,
                 aliases_json = ?7, confidence = ?8, status = ?9, edit_policy = ?10,
                 revision = revision + 1, updated_by_run_id = ?11, updated_at = ?12
             WHERE id = ?1",
            params![
                change.object_id,
                entry_type,
                slug,
                title,
                summary,
                content_md,
                aliases,
                confidence,
                status,
                edit_policy,
                change_set.agent_run_id,
                now,
            ],
        )?;
    }
    conn.execute(
        "DELETE FROM knowledge_entries_fts WHERE entry_id = ?1",
        params![change.object_id],
    )?;
    conn.execute(
        "INSERT INTO knowledge_entries_fts
         (entry_id, knowledge_base_id, title, aliases, summary, content_md, tags, cjk_terms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', ?7)",
        params![
            change.object_id,
            change_set.knowledge_base_id,
            title,
            aliases,
            summary,
            content_md,
            knowledge_fts_cjk_terms(&[title, aliases.as_str(), summary, content_md,]),
        ],
    )?;

    conn.execute(
        "DELETE FROM knowledge_claims WHERE entry_id = ?1",
        params![change.object_id],
    )?;
    conn.execute(
        "DELETE FROM knowledge_citations WHERE entry_id = ?1",
        params![change.object_id],
    )?;
    let entry_citations = json_string_list(candidate, "citations")?;
    for span_id in &entry_citations {
        conn.execute(
            "INSERT INTO knowledge_citations
             (id, knowledge_base_id, entry_id, source_span_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                stable_id("citation", &format!("{}:{span_id}", change.object_id)),
                change_set.knowledge_base_id,
                change.object_id,
                span_id,
                now,
            ],
        )?;
    }
    if let Some(claims) = candidate
        .get("claims")
        .and_then(serde_json::Value::as_array)
    {
        for claim in claims {
            let claim_text = required_json_string(claim, "claim_text")?;
            let claim_id = stable_id("claim", &format!("{}:{claim_text}", change.object_id));
            let predicate = claim
                .get("predicate")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("states");
            let object_text = claim.get("object_text").and_then(serde_json::Value::as_str);
            let claim_confidence = claim
                .get("confidence")
                .and_then(serde_json::Value::as_f64)
                .map(|value| value.clamp(0.0, 1.0));
            let verification_status = if change.classification == "disputed" {
                "disputed"
            } else {
                "unverified"
            };
            conn.execute(
                "INSERT INTO knowledge_claims
                 (id, knowledge_base_id, entry_id, predicate, object_text, claim_text,
                  confidence, verification_status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                params![
                    claim_id,
                    change_set.knowledge_base_id,
                    change.object_id,
                    predicate,
                    object_text,
                    claim_text,
                    claim_confidence,
                    verification_status,
                    now,
                ],
            )?;
            let claim_citations = json_string_list(claim, "citations")?;
            let claim_citations = if claim_citations.is_empty() {
                &entry_citations
            } else {
                &claim_citations
            };
            for span_id in claim_citations {
                conn.execute(
                    "INSERT INTO knowledge_citations
                     (id, knowledge_base_id, claim_id, source_span_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        stable_id("claim-citation", &format!("{claim_id}:{span_id}")),
                        change_set.knowledge_base_id,
                        claim_id,
                        span_id,
                        now,
                    ],
                )?;
            }
        }
    }
    let revision: i64 = conn.query_row(
        "SELECT revision FROM knowledge_entries WHERE id = ?1",
        params![change.object_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO knowledge_entry_versions
         (id, entry_id, revision, title, summary, content_md, aliases_json, status,
          confidence, changed_by_run_id, change_set_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            stable_id(
                "entry-version",
                &format!("{}:{revision}:{}", change.object_id, change_set.id),
            ),
            change.object_id,
            revision,
            title,
            summary,
            content_md,
            aliases,
            status,
            confidence,
            change_set.agent_run_id,
            change_set.id,
            now,
        ],
    )?;
    Ok(())
}

fn apply_entry_relations(
    conn: &rusqlite::Connection,
    base_id: &str,
    change: &KnowledgeChange,
    now: &str,
) -> Result<(), BrainError> {
    conn.execute(
        "DELETE FROM knowledge_relations WHERE from_entry_id = ?1",
        params![change.object_id],
    )?;
    if change
        .after
        .get("status")
        .and_then(serde_json::Value::as_str)
        == Some("archived")
    {
        return Ok(());
    }
    let Some(relations) = change
        .after
        .get("relations")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for relation in relations {
        let to_slug = required_json_string(relation, "to_slug")?;
        let relation_type = required_json_string(relation, "relation_type")?;
        let to_entry_id = conn
            .query_row(
                "SELECT id FROM knowledge_entries
                 WHERE knowledge_base_id = ?1 AND slug = ?2
                   AND status NOT IN ('archived', 'stale')
                 ORDER BY updated_at DESC LIMIT 1",
                params![base_id, to_slug],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                BrainError::KnowledgeValidation(format!("关系目标尚不存在，无法应用: {to_slug}"))
            })?;
        let strength = relation
            .get("strength")
            .and_then(serde_json::Value::as_f64)
            .map(|value| value.clamp(0.0, 1.0));
        let evidence = relation
            .get("evidence")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        conn.execute(
            "INSERT INTO knowledge_relations
             (id, knowledge_base_id, from_entry_id, to_entry_id, relation_type,
              strength, evidence, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                stable_id(
                    "relation",
                    &format!("{}:{to_entry_id}:{relation_type}", change.object_id),
                ),
                base_id,
                change.object_id,
                to_entry_id,
                relation_type,
                strength,
                evidence,
                now,
            ],
        )?;
    }
    Ok(())
}

fn apply_merge_redirects(
    conn: &rusqlite::Connection,
    change_set: &KnowledgeChangeSet,
    now: &str,
) -> Result<(), BrainError> {
    for change in &change_set.changes {
        let Some(source_ids) = change
            .after
            .get("merge_source_ids")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let source_ids = source_ids
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<HashSet<_>>();
        for source_id in &source_ids {
            let mut stmt = conn.prepare(
                "SELECT from_entry_id, relation_type, strength, COALESCE(evidence, '')
                 FROM knowledge_relations
                 WHERE knowledge_base_id = ?1 AND to_entry_id = ?2",
            )?;
            let incoming = stmt
                .query_map(params![change_set.knowledge_base_id, source_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (from_entry_id, relation_type, strength, evidence) in incoming {
                if from_entry_id == change.object_id || source_ids.contains(from_entry_id.as_str())
                {
                    continue;
                }
                conn.execute(
                    "INSERT OR IGNORE INTO knowledge_relations
                     (id, knowledge_base_id, from_entry_id, to_entry_id, relation_type,
                      strength, evidence, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        stable_id(
                            "relation",
                            &format!("{from_entry_id}:{}:{relation_type}", change.object_id),
                        ),
                        change_set.knowledge_base_id,
                        from_entry_id,
                        change.object_id,
                        relation_type,
                        strength,
                        evidence,
                        now,
                    ],
                )?;
            }
        }
        for source_id in source_ids {
            conn.execute(
                "DELETE FROM knowledge_relations
                 WHERE knowledge_base_id = ?1
                   AND (from_entry_id = ?2 OR to_entry_id = ?2)",
                params![change_set.knowledge_base_id, source_id],
            )?;
        }
    }
    Ok(())
}

fn complete_manual_change_run(
    store: &BookWikiStore,
    run_id: &str,
    result: Result<KnowledgeChangeSet, BrainError>,
) -> Result<KnowledgeChangeSet, BrainError> {
    match result {
        Ok(change_set) => {
            store.complete_agent_run(
                run_id,
                &serde_json::json!({ "change_set_id": change_set.id }),
            )?;
            Ok(change_set)
        }
        Err(error) => {
            let _ = store.fail_agent_run(run_id, &error.to_string());
            Err(error)
        }
    }
}

fn refresh_review_state(
    conn: &rusqlite::Connection,
    base_id: &str,
    now: &str,
) -> Result<(), BrainError> {
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM knowledge_change_sets
         WHERE knowledge_base_id = ?1 AND status = 'proposed'",
        params![base_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "UPDATE knowledge_bases
         SET pending_review_count = ?2,
             health_state = CASE WHEN ?2 > 0 THEN 'needs_review' ELSE 'healthy' END,
             updated_at = ?3
         WHERE id = ?1",
        params![base_id, pending, now],
    )?;
    Ok(())
}

pub fn stable_id(prefix: &str, value: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.as_bytes());
    format!("{prefix}-{}", &hex::encode(digest)[..24])
}

fn knowledge_query_candidates(query: Option<&str>) -> Vec<String> {
    let Some(raw) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };

    let mut candidates = vec![raw.to_string()];
    let mut simplified = raw.to_string();
    for stop_word in [
        "这本书",
        "本书",
        "书中",
        "书内",
        "请",
        "帮我",
        "找出",
        "找到",
        "列出",
        "有哪些内容",
        "有哪些",
        "有什么",
        "是什么",
        "内容",
        "提到了",
        "提到",
        "相关的",
        "相关",
        "章节",
        "？",
        "?",
        "。",
        "，",
        ",",
        "的",
    ] {
        simplified = simplified.replace(stop_word, "");
    }
    let simplified = simplified.trim();
    if simplified.chars().count() >= 2 {
        candidates.push(simplified.to_string());
        let characters = simplified.chars().collect::<Vec<_>>();
        if characters.len() > 2 {
            candidates.extend(
                characters
                    .windows(2)
                    .map(|window| window.iter().collect::<String>()),
            );
        }
    }

    candidates.extend(
        raw.split(|character: char| !character.is_alphanumeric() && character != '_')
            .filter(|term| term.chars().count() >= 2)
            .map(str::to_string),
    );

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.clone()))
        .take(8)
        .collect()
}

fn knowledge_query_patterns(query: Option<&str>) -> Vec<String> {
    let candidates = knowledge_query_candidates(query);
    if candidates.is_empty() {
        return vec!["%".to_string()];
    }
    candidates
        .into_iter()
        .map(|candidate| format!("%{candidate}%"))
        .collect()
}

fn knowledge_fts_query(query: Option<&str>) -> Option<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for candidate in knowledge_query_candidates(query) {
        for term in knowledge_search_terms(&candidate) {
            if seen.insert(term.clone()) {
                terms.push(format!("\"{}\"", term.replace('"', "\"\"")));
            }
        }
    }
    (!terms.is_empty()).then(|| terms.into_iter().take(32).collect::<Vec<_>>().join(" OR "))
}

fn knowledge_fts_cjk_terms(values: &[&str]) -> String {
    values
        .iter()
        .flat_map(|value| cjk_terms(value, false))
        .collect::<Vec<_>>()
        .join(" ")
}

fn knowledge_search_terms(value: &str) -> Vec<String> {
    let mut terms = cjk_terms(value, true);
    let mut ascii = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            ascii.push(character.to_ascii_lowercase());
        } else if !ascii.is_empty() {
            if ascii.chars().count() >= 2 {
                terms.push(std::mem::take(&mut ascii));
            } else {
                ascii.clear();
            }
        }
    }
    if ascii.chars().count() >= 2 {
        terms.push(ascii);
    }
    terms
}

fn cjk_terms(value: &str, include_full_run: bool) -> Vec<String> {
    fn flush(run: &mut Vec<char>, terms: &mut Vec<String>, include_full_run: bool) {
        if run.is_empty() {
            return;
        }
        if include_full_run {
            terms.push(run.iter().collect());
        }
        if run.len() == 1 {
            terms.push(run[0].to_string());
        } else {
            terms.extend(
                run.windows(2)
                    .map(|window| window.iter().collect::<String>()),
            );
        }
        run.clear();
    }

    let mut terms = Vec::new();
    let mut run = Vec::new();
    for character in value.chars() {
        if is_cjk(character) {
            run.push(character);
        } else {
            flush(&mut run, &mut terms, include_full_run);
        }
    }
    flush(&mut run, &mut terms, include_full_run);
    terms
}

fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

fn normalize_reader_selection(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(
                    character,
                    '*' | '_' | '`' | '#' | '[' | ']' | '(' | ')' | '>' | '~'
                )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::book_wiki::BookProgress;

    fn test_store() -> (BookWikiStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(SqliteStore::new(&dir.path().join("book-wiki.db")).expect("db"));
        (BookWikiStore::new(db), dir)
    }

    fn sample_book(id: &str, path: &str) -> ReaderBook {
        ReaderBook {
            id: id.to_string(),
            path: path.to_string(),
            kind: BookKind::Folder,
            name: "测试书".to_string(),
            description: "描述".to_string(),
            category: "技术".to_string(),
            added_at: 123,
            progress: Some(BookProgress {
                last_file: Some("intro.md".to_string()),
                position: 0.25,
                page_count: None,
                updated_at: 456,
            }),
        }
    }

    fn sample_source(id: &str, entry_id: &str, span_id: &str) -> MarkdownSourceDraft {
        MarkdownSourceDraft {
            id: format!("source-{id}"),
            version_id: format!("version-{id}"),
            original_path: format!("/tmp/{id}.md"),
            relative_path: format!("{id}.md"),
            title: "测试来源".to_string(),
            ordinal: 0,
            content_hash: format!("document-{id}"),
            size_bytes: 24,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: span_id.to_string(),
                entry_id: entry_id.to_string(),
                slug: format!("source-{id}"),
                title: "来源章节".to_string(),
                summary: "可追溯摘要".to_string(),
                content_md: "可追溯的来源正文。".to_string(),
                line_start: 1,
                line_end: 2,
                content_hash: format!("section-{id}"),
            }],
        }
    }

    fn semantic_candidate(span_id: &str, title: &str) -> serde_json::Value {
        serde_json::json!({
            "entry_type": "concept",
            "slug": "semantic-concept",
            "title": title,
            "summary": "跨来源归纳摘要",
            "content_md": "这是经过审核后才能写入的主题知识。",
            "aliases": ["Semantic Concept"],
            "confidence": 0.88,
            "citations": [span_id],
            "claims": [{
                "claim_text": "主题知识必须保留来源。",
                "predicate": "requires",
                "object_text": "来源引用",
                "confidence": 0.9,
                "citations": [span_id]
            }],
            "relations": []
        })
    }

    fn insert_builtin_skill_candidate(store: &BookWikiStore, skill_id: &str) -> String {
        store
            .db
            .with_connection(|conn| {
                let (current_version_id, revision, content): (String, i64, String) = conn
                    .query_row(
                        "SELECT s.current_version_id, v.revision, f.content_text
                         FROM skills s
                         JOIN skill_versions v ON v.id = s.current_version_id
                         JOIN skill_files f ON f.skill_version_id = v.id
                                           AND f.relative_path = 'SKILL.md'
                         WHERE s.id = ?1",
                        params![skill_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )?;
                let candidate_content = format!(
                    "{content}\n\n候选测试补充：新增、更新、争议、无实质变化；引用、来源与论断；冲突不得强行消解；证据不足时先拆解再综合；研究计划区分书内、外部与待验证内容。\n"
                );
                let content_hash = stable_id("test-skill-candidate", &candidate_content);
                let version_id = stable_id(
                    "test-skill-version",
                    &format!("{skill_id}:{}:{content_hash}", revision + 1),
                );
                conn.execute(
                    "INSERT INTO skill_versions
                        (id, skill_id, revision, content_hash, release_state,
                         parent_version_id, changelog, created_at)
                     VALUES (?1, ?2, ?3, ?4, 'candidate', ?5,
                             '测试候选版本', CURRENT_TIMESTAMP)",
                    params![
                        version_id,
                        skill_id,
                        revision + 1,
                        content_hash,
                        current_version_id
                    ],
                )?;
                conn.execute(
                    "INSERT INTO skill_files
                        (skill_version_id, relative_path, media_type, content_text,
                         content_hash, size_bytes)
                     VALUES (?1, 'SKILL.md', 'text/markdown', ?2, ?3, ?4)",
                    params![
                        version_id,
                        candidate_content,
                        content_hash,
                        candidate_content.len() as i64
                    ],
                )?;
                Ok(version_id)
            })
            .unwrap()
    }

    #[test]
    fn test_reader_books_round_trip_uses_normalized_table() {
        let (store, _dir) = test_store();
        let book = sample_book("book-1", "/tmp/book-1");

        assert_eq!(
            store
                .save_reader_books(std::slice::from_ref(&book))
                .unwrap(),
            1
        );
        assert_eq!(store.list_reader_books().unwrap(), vec![book]);
    }

    #[test]
    fn test_reader_books_missing_from_save_is_soft_removed() {
        let (store, _dir) = test_store();
        let book = sample_book("book-1", "/tmp/book-1");
        store.save_reader_books(&[book]).unwrap();

        store.save_reader_books(&[]).unwrap();

        assert!(store.list_reader_books().unwrap().is_empty());
    }

    #[test]
    fn test_reader_book_readded_with_new_id_keeps_existing_knowledge_base() {
        let (store, _dir) = test_store();
        let path = "/tmp/book-1";
        store
            .save_reader_books(&[sample_book("book-old", path)])
            .unwrap();
        let original_base = store.initialize_base("book-old").unwrap();
        store.save_reader_books(&[]).unwrap();

        store
            .save_reader_books(&[sample_book("book-new", path)])
            .unwrap();

        let books = store.list_reader_books().unwrap();
        assert_eq!(books[0].id, "book-new");
        let rebound_base = store.get_base_by_book_id("book-new").unwrap().unwrap();
        assert_eq!(rebound_base.id, original_base.id);
    }

    #[test]
    fn test_initialize_base_is_idempotent_and_creates_config_documents() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-1", "/tmp/book-1")])
            .unwrap();

        let first = store.initialize_base("book-1").unwrap();
        let second = store.initialize_base("book-1").unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(
            store.list_config_documents(Some(&first.id)).unwrap().len(),
            3
        );
    }

    #[test]
    fn test_sync_markdown_sources_indexes_entries_and_citations() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-1", "/tmp/book-1")])
            .unwrap();
        let base = store.initialize_base("book-1").unwrap();
        let source = MarkdownSourceDraft {
            id: "source-1".to_string(),
            version_id: "version-1".to_string(),
            original_path: "/tmp/book-1/intro.md".to_string(),
            relative_path: "intro.md".to_string(),
            title: "介绍".to_string(),
            ordinal: 0,
            content_hash: "hash".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-1".to_string(),
                entry_id: "entry-1".to_string(),
                slug: "intro--hello".to_string(),
                title: "你好".to_string(),
                summary: "这是摘要".to_string(),
                content_md: "这是正文".to_string(),
                line_start: 1,
                line_end: 1,
                content_hash: "section-hash".to_string(),
            }],
        };

        store.sync_markdown_sources(&base.id, &[source]).unwrap();

        let entries = store
            .list_entries(&base.id, Some("正文"), None, 20)
            .unwrap();
        assert_eq!(entries.len(), 1);
        let detail = store.get_entry(&entries[0].id).unwrap();
        assert_eq!(detail.citations.len(), 1);
        assert_eq!(detail.citations[0].source_path, "intro.md");
    }

    #[test]
    fn test_source_update_preserves_old_spans_and_conversation_evidence_snapshot() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-history", "/tmp/book-history")])
            .unwrap();
        let base = store.initialize_base("book-history").unwrap();
        let first = MarkdownSourceDraft {
            id: "source-history".to_string(),
            version_id: "version-history-1".to_string(),
            original_path: "/tmp/book-history/intro.md".to_string(),
            relative_path: "intro.md".to_string(),
            title: "介绍".to_string(),
            ordinal: 0,
            content_hash: "document-hash-1".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-history-1".to_string(),
                entry_id: "entry-history".to_string(),
                slug: "intro--history".to_string(),
                title: "历史章节".to_string(),
                summary: "旧摘要".to_string(),
                content_md: "旧正文".to_string(),
                line_start: 1,
                line_end: 2,
                content_hash: "section-hash-1".to_string(),
            }],
        };
        store
            .sync_markdown_sources(&base.id, std::slice::from_ref(&first))
            .unwrap();
        let old_evidence = store.list_entries(&base.id, None, None, 10).unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();
        let conversation_id = store
            .save_conversation_exchange(
                &base.id,
                None,
                "旧内容是什么？",
                "旧内容。[S1]",
                &run.id,
                &old_evidence,
            )
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_bases
                     SET compile_mode = 'smart', compile_state = 'ready'
                     WHERE id = ?1",
                    params![base.id],
                )?;
                Ok(())
            })
            .unwrap();

        let mut second = first;
        second.version_id = "version-history-2".to_string();
        second.content_hash = "document-hash-2".to_string();
        second.sections[0].id = "span-history-2".to_string();
        second.sections[0].summary = "新摘要".to_string();
        second.sections[0].content_md = "新正文".to_string();
        second.sections[0].content_hash = "section-hash-2".to_string();
        store.sync_markdown_sources(&base.id, &[second]).unwrap();

        let current = store.get_entry("entry-history").unwrap();
        assert_eq!(current.entry.summary, "新摘要");
        assert_eq!(current.citations.len(), 1);
        assert_eq!(current.citations[0].quote_text.as_deref(), Some("新摘要"));
        let conversation = store.get_conversation(&conversation_id).unwrap();
        assert_eq!(conversation.messages[1].evidence[0].summary, "旧摘要");
        assert_eq!(store.get_base(&base.id).unwrap().compile_state, "outdated");
        let span_count = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM source_spans WHERE knowledge_base_id = ?1",
                    params![base.id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(span_count, 2);
    }

    #[test]
    fn test_repeated_source_sync_is_idempotent() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-repeat", "/tmp/book-repeat")])
            .unwrap();
        let base = store.initialize_base("book-repeat").unwrap();
        let source = MarkdownSourceDraft {
            id: "source-repeat".to_string(),
            version_id: "version-repeat".to_string(),
            original_path: "/tmp/book-repeat/intro.md".to_string(),
            relative_path: "intro.md".to_string(),
            title: "介绍".to_string(),
            ordinal: 0,
            content_hash: "document-repeat".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-repeat".to_string(),
                entry_id: "entry-repeat".to_string(),
                slug: "intro--repeat".to_string(),
                title: "重复同步".to_string(),
                summary: "摘要".to_string(),
                content_md: "正文".to_string(),
                line_start: 1,
                line_end: 2,
                content_hash: "section-repeat".to_string(),
            }],
        };

        store
            .sync_markdown_sources(&base.id, std::slice::from_ref(&source))
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_bases
                     SET compile_mode = 'smart', compile_state = 'ready'
                     WHERE id = ?1",
                    params![base.id],
                )?;
                Ok(())
            })
            .unwrap();
        store.sync_markdown_sources(&base.id, &[source]).unwrap();

        let detail = store.get_entry("entry-repeat").unwrap();
        assert_eq!(detail.citations.len(), 1);
        let revision = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT revision FROM knowledge_entries WHERE id = 'entry-repeat'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(revision, 1);
        assert_eq!(store.get_base(&base.id).unwrap().compile_state, "ready");
    }

    #[test]
    fn test_missing_source_is_hidden_from_current_index_but_remains_addressable() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-missing", "/tmp/book-missing")])
            .unwrap();
        let base = store.initialize_base("book-missing").unwrap();
        let source = MarkdownSourceDraft {
            id: "source-missing".to_string(),
            version_id: "version-missing".to_string(),
            original_path: "/tmp/book-missing/removed.md".to_string(),
            relative_path: "removed.md".to_string(),
            title: "即将删除".to_string(),
            ordinal: 0,
            content_hash: "document-missing".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-missing".to_string(),
                entry_id: "entry-missing".to_string(),
                slug: "removed--missing".to_string(),
                title: "已删除章节".to_string(),
                summary: "保留历史".to_string(),
                content_md: "历史正文".to_string(),
                line_start: 1,
                line_end: 2,
                content_hash: "section-missing".to_string(),
            }],
        };
        store.sync_markdown_sources(&base.id, &[source]).unwrap();

        store.sync_markdown_sources(&base.id, &[]).unwrap();

        assert!(store
            .list_entries(&base.id, Some("历史正文"), None, 10)
            .unwrap()
            .is_empty());
        let historic = store.get_entry("entry-missing").unwrap();
        assert_eq!(historic.entry.status, "stale");
        assert_eq!(historic.content_md, "历史正文");
        assert_eq!(store.get_base(&base.id).unwrap().entry_count, 0);
    }

    #[test]
    fn test_knowledge_conversation_round_trips_messages_and_evidence() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-1", "/tmp/book-1")])
            .unwrap();
        let base = store.initialize_base("book-1").unwrap();
        let source = MarkdownSourceDraft {
            id: "conversation-source".to_string(),
            version_id: "conversation-version".to_string(),
            original_path: "/tmp/book-1/history.md".to_string(),
            relative_path: "history.md".to_string(),
            title: "历史".to_string(),
            ordinal: 0,
            content_hash: "history-hash".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "conversation-span".to_string(),
                entry_id: "conversation-entry".to_string(),
                slug: "history".to_string(),
                title: "历史章节".to_string(),
                summary: "来源摘要".to_string(),
                content_md: "来源正文".to_string(),
                line_start: 1,
                line_end: 3,
                content_hash: "history-section-hash".to_string(),
            }],
        };
        store.sync_markdown_sources(&base.id, &[source]).unwrap();
        let evidence = store.list_entries(&base.id, None, None, 10).unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({ "question": "什么是历史？" }),
            )
            .unwrap();

        let conversation_id = store
            .save_conversation_exchange(
                &base.id,
                None,
                "什么是历史？",
                "历史是可追溯的。[S1]",
                &run.id,
                &evidence,
            )
            .unwrap();
        store
            .save_conversation_exchange(
                &base.id,
                Some(&conversation_id),
                "再说明一下",
                "这是第二轮回答。[S1]",
                &run.id,
                &evidence,
            )
            .unwrap();

        let summaries = store.list_conversations(&base.id, 20).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].message_count, 4);
        assert_eq!(summaries[0].title, "什么是历史？");
        let detail = store.get_conversation(&conversation_id).unwrap();
        assert_eq!(detail.messages.len(), 4);
        assert_eq!(detail.messages[0].role, "user");
        assert_eq!(detail.messages[1].evidence, evidence);
        assert_eq!(detail.messages[3].content, "这是第二轮回答。[S1]");
    }

    #[test]
    fn test_list_entries_simplifies_natural_language_question() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-1", "/tmp/book-1")])
            .unwrap();
        let base = store.initialize_base("book-1").unwrap();
        let source = MarkdownSourceDraft {
            id: "source-architecture".to_string(),
            version_id: "version-architecture".to_string(),
            original_path: "/tmp/book-1/architecture.md".to_string(),
            relative_path: "architecture.md".to_string(),
            title: "系统架构".to_string(),
            ordinal: 0,
            content_hash: "architecture-hash".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-architecture".to_string(),
                entry_id: "entry-architecture".to_string(),
                slug: "architecture".to_string(),
                title: "核心架构".to_string(),
                summary: "介绍系统的核心架构".to_string(),
                content_md: "这一章描述系统架构。".to_string(),
                line_start: 1,
                line_end: 1,
                content_hash: "section-architecture-hash".to_string(),
            }],
        };
        store.sync_markdown_sources(&base.id, &[source]).unwrap();

        let entries = store
            .list_entries(&base.id, Some("找出与架构相关的章节"), None, 20)
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "核心架构");
    }

    #[test]
    fn test_list_entries_fts_prioritizes_title_match_before_body_match() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-fts", "/tmp/book-fts")])
            .unwrap();
        let base = store.initialize_base("book-fts").unwrap();
        let body_match = MarkdownSourceDraft {
            id: "source-body-match".to_string(),
            version_id: "version-body-match".to_string(),
            original_path: "/tmp/book-fts/body.md".to_string(),
            relative_path: "body.md".to_string(),
            title: "正文命中".to_string(),
            ordinal: 0,
            content_hash: "body-document-hash".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-body-match".to_string(),
                entry_id: "entry-body-match".to_string(),
                slug: "body-match".to_string(),
                title: "其他主题".to_string(),
                summary: "普通摘要".to_string(),
                content_md: "分层架构".to_string(),
                line_start: 1,
                line_end: 1,
                content_hash: "body-section-hash".to_string(),
            }],
        };
        let title_match = MarkdownSourceDraft {
            id: "source-title-match".to_string(),
            version_id: "version-title-match".to_string(),
            original_path: "/tmp/book-fts/title.md".to_string(),
            relative_path: "title.md".to_string(),
            title: "标题命中".to_string(),
            ordinal: 1,
            content_hash: "title-document-hash".to_string(),
            size_bytes: 20,
            modified_at: None,
            sections: vec![SourceSectionDraft {
                id: "span-title-match".to_string(),
                entry_id: "entry-title-match".to_string(),
                slug: "title-match".to_string(),
                title: "分层架构".to_string(),
                summary: "普通摘要".to_string(),
                content_md: "其他正文".to_string(),
                line_start: 1,
                line_end: 1,
                content_hash: "title-section-hash".to_string(),
            }],
        };
        store
            .sync_markdown_sources(&base.id, &[body_match, title_match])
            .unwrap();

        let entries = store
            .list_entries(&base.id, Some("分层架构"), None, 20)
            .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "entry-title-match");
        assert_eq!(entries[1].id, "entry-body-match");
    }

    #[test]
    fn test_search_source_spans_fts_prioritizes_heading_and_isolates_books() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[
                sample_book("book-source-fts", "/tmp/book-source-fts"),
                sample_book("book-source-other", "/tmp/book-source-other"),
            ])
            .unwrap();
        let base = store.initialize_base("book-source-fts").unwrap();
        let other_base = store.initialize_base("book-source-other").unwrap();
        let mut body_match = sample_source("source-body", "entry-source-body", "span-source-body");
        body_match.ordinal = 0;
        body_match.sections[0].title = "其他主题".to_string();
        body_match.sections[0].content_md = "分层架构".to_string();
        let mut heading_match = sample_source(
            "source-heading",
            "entry-source-heading",
            "span-source-heading",
        );
        heading_match.ordinal = 1;
        heading_match.sections[0].title = "分层架构".to_string();
        heading_match.sections[0].content_md = "其他正文".to_string();
        let mut foreign = sample_source("source-foreign", "entry-foreign", "span-foreign");
        foreign.sections[0].title = "分层架构".to_string();
        store
            .sync_markdown_sources(&base.id, &[body_match, heading_match])
            .unwrap();
        store
            .sync_markdown_sources(&other_base.id, &[foreign])
            .unwrap();

        let spans = store.search_source_spans(&base.id, "分层架构", 20).unwrap();

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].id, "span-source-heading");
        assert_eq!(spans[1].id, "span-source-body");
        assert!(spans.iter().all(|span| span.id != "span-foreign"));
    }

    #[test]
    fn test_agent_run_records_completed_output() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-agent", "/tmp/book-agent")])
            .unwrap();
        let base = store.initialize_base("book-agent").unwrap();

        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({"question": "核心主题是什么？"}),
            )
            .unwrap();
        assert_eq!(run.status, "running");

        let completed = store
            .complete_agent_run(&run.id, &serde_json::json!({"answer": "测试回答"}))
            .unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.output.unwrap()["answer"], "测试回答");
        assert!(completed.finished_at.is_some());
        let events = store.list_agent_run_events(&run.id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "run.started");
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].event_type, "run.completed");
        assert_eq!(events[1].sequence, 2);
    }

    #[test]
    fn test_agent_run_capability_is_hashed_scoped_expiring_and_revocable() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-capability", "/tmp/book-capability")])
            .unwrap();
        let base = store.initialize_base("book-capability").unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();
        let issued = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &["book_get_context".to_string()],
                300,
            )
            .unwrap();

        let grant = store
            .validate_agent_run_capability(&issued.token, "book_get_context")
            .unwrap();
        assert_eq!(grant.run_id, run.id);
        assert_eq!(grant.knowledge_base_ids, vec![base.id.clone()]);
        assert_eq!(grant.allowed_tools, vec!["book_get_context"]);
        let stored_token: String = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT token_hash FROM agent_run_capabilities WHERE run_id = ?1",
                    params![run.id],
                    |row| row.get(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_ne!(stored_token, issued.token);
        assert!(store
            .validate_agent_run_capability(&issued.token, "knowledge_get_entry")
            .unwrap_err()
            .to_string()
            .contains("工具权限"));
        assert!(store
            .validate_agent_run_capability("invalid-token", "book_get_context")
            .unwrap_err()
            .to_string()
            .contains("能力令牌"));

        store.revoke_agent_run_capabilities(&run.id).unwrap();
        assert!(store
            .validate_agent_run_capability(&issued.token, "book_get_context")
            .unwrap_err()
            .to_string()
            .contains("能力令牌"));

        let expired = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &["book_get_context".to_string()],
                0,
            )
            .unwrap();
        assert!(store
            .validate_agent_run_capability(&expired.token, "book_get_context")
            .unwrap_err()
            .to_string()
            .contains("过期"));
    }

    #[test]
    fn test_custom_wiki_skill_keeps_latest_revision_and_book_scope_binding() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-skills", "/tmp/book-skills")])
            .unwrap();
        let base = store.initialize_base("book-skills").unwrap();

        let created = store
            .save_custom_wiki_skill(
                None,
                "argument-map",
                "论证图谱",
                "按论点和证据组织答案",
                "先列出主张，再列出支持与反对证据。",
                None,
            )
            .unwrap();
        assert_eq!(created.revision, 1);
        assert!(created.instructions.starts_with("先列出主张"));
        assert!(!created.enabled);

        let bound = store
            .set_wiki_skill_binding(&base.id, &created.id, true, "research")
            .unwrap();
        assert!(bound.enabled);
        assert_eq!(bound.usage_scope, "research");
        assert!(store
            .enabled_wiki_skills(&base.id, "qa")
            .unwrap()
            .is_empty());
        assert_eq!(
            store.enabled_wiki_skills(&base.id, "research").unwrap()[0].id,
            created.id
        );

        let ingest = store
            .set_wiki_skill_binding(&base.id, &created.id, true, "ingest")
            .unwrap();
        assert_eq!(ingest.usage_scope, "ingest");
        assert!(store
            .enabled_wiki_skills(&base.id, "research")
            .unwrap()
            .is_empty());
        assert_eq!(
            store.enabled_wiki_skills(&base.id, "ingest").unwrap()[0].id,
            created.id
        );

        store
            .set_wiki_skill_binding(&base.id, &created.id, true, "all")
            .unwrap();
        assert_eq!(store.enabled_wiki_skills(&base.id, "qa").unwrap().len(), 1);
        assert_eq!(
            store.enabled_wiki_skills(&base.id, "ingest").unwrap().len(),
            1
        );

        let updated = store
            .save_custom_wiki_skill(
                Some(&created.id),
                "argument-map",
                "论证图谱",
                "按论点和证据组织答案",
                "先列出主张、支持证据、反对证据，再说明证据缺口。",
                Some(created.revision),
            )
            .unwrap();
        assert_eq!(updated.revision, 2);
        assert!(updated.instructions.contains("证据缺口"));
        assert_eq!(
            store
                .get_wiki_skill_detail(&updated.id, None)
                .unwrap()
                .versions
                .len(),
            1
        );
        let unchanged = store
            .save_custom_wiki_skill(
                Some(&updated.id),
                &updated.slug,
                &updated.name,
                &updated.description,
                &updated.instructions,
                Some(updated.revision),
            )
            .unwrap();
        assert_eq!(unchanged.revision, 2);
    }

    #[test]
    fn test_custom_wiki_skill_rejects_unsafe_slug_and_builtin_edits() {
        let (store, _dir) = test_store();
        let invalid = store
            .save_custom_wiki_skill(None, "../escape", "无效", "", "指令", None)
            .unwrap_err();
        assert!(invalid.to_string().contains("Skill 标识"));

        let builtin = store
            .list_wiki_skills(None)
            .unwrap()
            .into_iter()
            .find(|skill| skill.source_type == "builtin")
            .unwrap();
        let error = store
            .save_custom_wiki_skill(
                Some(&builtin.id),
                &builtin.slug,
                &builtin.name,
                &builtin.description,
                "尝试修改",
                Some(builtin.revision),
            )
            .unwrap_err();
        assert!(error.to_string().contains("内置 Skill"));
    }

    #[test]
    fn test_imported_skill_package_is_idempotent_and_persists_resources() {
        let (store, _dir) = test_store();
        let files = vec![
            (
                "SKILL.md".to_string(),
                "---\nname: source-review\ndescription: 核对来源\n---\n先读取证据，再回答。"
                    .to_string(),
            ),
            (
                "references/checklist.md".to_string(),
                "# 核对清单\n- 引用必须属于当前书籍".to_string(),
            ),
        ];

        let imported = store
            .import_custom_wiki_skill("source-review", "来源核对", "核对来源", &files)
            .unwrap();
        let repeated = store
            .import_custom_wiki_skill("source-review", "来源核对", "核对来源", &files)
            .unwrap();

        assert_eq!(imported.id, repeated.id);
        assert_eq!(imported.revision, 1);
        let file_count = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM skill_files sf
                     JOIN skill_versions sv ON sv.id = sf.skill_version_id
                     WHERE sv.skill_id = ?1",
                    params![imported.id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(file_count, 2);
    }

    #[test]
    fn test_wiki_skill_detail_keeps_only_latest_custom_version() {
        let (store, _dir) = test_store();
        let files = vec![
            (
                "SKILL.md".to_string(),
                "---\nname: source-review\ndescription: 核对来源\n---\n先读取证据，再回答。"
                    .to_string(),
            ),
            (
                "references/checklist.md".to_string(),
                "# 核对清单\n- 引用必须属于当前书籍".to_string(),
            ),
        ];
        let imported = store
            .import_custom_wiki_skill("source-review", "来源核对", "核对来源", &files)
            .unwrap();
        store
            .save_custom_wiki_skill(
                Some(&imported.id),
                &imported.slug,
                &imported.name,
                &imported.description,
                "先读取证据，再检查反例，最后回答。",
                Some(imported.revision),
            )
            .unwrap();

        let detail = store.get_wiki_skill_detail(&imported.id, None).unwrap();
        assert_eq!(detail.skill.id, imported.id);
        assert_eq!(detail.versions.len(), 1);
        assert_eq!(detail.versions[0].revision, 2);
        assert_eq!(detail.versions[0].files.len(), 1);
        assert!(detail.versions[0].files[0]
            .content_text
            .contains("检查反例"));
    }

    #[test]
    fn test_builtin_ingest_v3_is_reviewable_and_active() {
        let (store, _dir) = test_store();
        let detail = store
            .get_wiki_skill_detail("skill-book-ingest", None)
            .unwrap();
        assert_eq!(detail.current_version_id, "skill-version-book-ingest-v3");
        let published = detail
            .versions
            .iter()
            .find(|version| version.revision == 3)
            .unwrap();
        assert_eq!(detail.versions.len(), 1);
        assert_eq!(published.release_state, "published");
        assert!(published.parent_version_id.is_none());
        let skill_file = published
            .files
            .iter()
            .find(|file| file.relative_path == "SKILL.md")
            .unwrap();
        assert_eq!(
            skill_file.content_text,
            include_str!("../../skills/book-ingest/SKILL.md")
        );
    }

    #[test]
    fn test_builtin_skill_candidate_requires_evaluation_then_keeps_only_published_version() {
        let (store, _dir) = test_store();
        let baseline_version_id = "skill-version-book-ingest-v3";
        let candidate_id = insert_builtin_skill_candidate(&store, "skill-book-ingest");
        store
            .save_reader_books(&[sample_book(
                "book-skill-benchmark",
                "/tmp/book-skill-benchmark",
            )])
            .unwrap();
        let base = store.initialize_base("book-skill-benchmark").unwrap();
        let detail = store
            .get_wiki_skill_detail("skill-book-ingest", None)
            .unwrap();
        let candidate = detail
            .versions
            .iter()
            .find(|version| version.id == candidate_id)
            .unwrap();
        assert_eq!(candidate.revision, 4);
        assert_eq!(
            candidate.parent_version_id.as_deref(),
            Some(baseline_version_id)
        );
        assert!(store
            .publish_wiki_skill_version("skill-book-ingest", &candidate.id)
            .unwrap_err()
            .to_string()
            .contains("固定评测"));
        assert!(store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .unwrap_err()
            .to_string()
            .contains("固定评测"));
        assert!(store
            .evaluate_wiki_skill_version("skill-book-ingest", &candidate.id, "grounded-query")
            .unwrap_err()
            .to_string()
            .contains("必须使用固定评测集"));

        let evaluation = store
            .evaluate_wiki_skill_version("skill-book-ingest", &candidate.id, "semantic-ingest")
            .unwrap();
        assert!(evaluation.passed);
        assert_eq!(evaluation.score, 1.0);
        assert!(evaluation.baseline_score.is_some_and(|score| score < 1.0));
        assert!(store
            .publish_wiki_skill_version("skill-book-ingest", &candidate.id)
            .unwrap_err()
            .to_string()
            .contains("真实模型基准"));

        let benchmark = store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .unwrap();
        assert_eq!(benchmark.status, "queued");
        let benchmark = store.start_wiki_skill_benchmark(&benchmark.id).unwrap();
        assert_eq!(benchmark.status, "running");
        let benchmark = store
            .mark_wiki_skill_benchmark_candidate_complete(&benchmark.id)
            .unwrap();
        assert_eq!(benchmark.completed_cases, benchmark.total_cases);
        let cases = store
            .load_wiki_skill_benchmark_cases(&benchmark.suite_id)
            .unwrap();
        assert_eq!(cases.len(), 12);
        let baseline_run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "skill_benchmark",
                &serde_json::json!({ "variant": "baseline" }),
            )
            .unwrap();
        let candidate_run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "skill_benchmark",
                &serde_json::json!({ "variant": "candidate" }),
            )
            .unwrap();
        let results = ["baseline", "candidate"]
            .into_iter()
            .flat_map(|variant| {
                cases.iter().map(move |case| WikiSkillBenchmarkCaseResult {
                    case_id: case.id.clone(),
                    name: case.name.clone(),
                    variant: variant.to_string(),
                    agent_run_id: None,
                    response_text: "通过".to_string(),
                    citations: Vec::new(),
                    metrics: serde_json::json!({ "score": 1.0 }),
                    score: 1.0,
                    passed: true,
                    error: None,
                })
            })
            .collect::<Vec<_>>();
        let metrics = serde_json::json!({ "score_delta": 0.1 });
        let incomplete_results = results
            .iter()
            .cloned()
            .map(|mut result| {
                if result.variant == "candidate" {
                    result.passed = false;
                }
                result
            })
            .collect::<Vec<_>>();
        let incomplete = store
            .complete_wiki_skill_benchmark(
                &benchmark.id,
                WikiSkillBenchmarkCompletion {
                    baseline_agent_run_id: &baseline_run.id,
                    candidate_agent_run_id: &candidate_run.id,
                    baseline_score: 0.8,
                    candidate_score: 0.9,
                    metrics: &metrics,
                    results: &incomplete_results,
                },
            )
            .unwrap();
        assert!(!incomplete.passed, "high score cannot bypass failed cases");
        let benchmark = store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .unwrap();
        let benchmark = store.start_wiki_skill_benchmark(&benchmark.id).unwrap();
        let benchmark = store
            .complete_wiki_skill_benchmark(
                &benchmark.id,
                WikiSkillBenchmarkCompletion {
                    baseline_agent_run_id: &baseline_run.id,
                    candidate_agent_run_id: &candidate_run.id,
                    baseline_score: 0.8,
                    candidate_score: 0.9,
                    metrics: &metrics,
                    results: &results,
                },
            )
            .unwrap();
        assert_eq!(benchmark.status, "completed");
        assert!(benchmark.passed);
        assert_eq!(benchmark.results.len(), 24);

        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE skills SET current_version_id = ?2 WHERE id = ?1",
                    params!["skill-book-ingest", candidate.id],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store
            .publish_wiki_skill_version("skill-book-ingest", &candidate.id)
            .unwrap_err()
            .to_string()
            .contains("当前 Skill 版本已变化"));
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE skills SET current_version_id = 'skill-version-book-ingest-v3'
                     WHERE id = 'skill-book-ingest'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let published = store
            .publish_wiki_skill_version("skill-book-ingest", &candidate.id)
            .unwrap();
        assert_eq!(published.current_version_id, candidate.id);
        assert_eq!(published.skill.revision, 4);
        assert_eq!(published.versions.len(), 1);
        assert!(published
            .versions
            .iter()
            .find(|version| version.id == candidate.id)
            .unwrap()
            .latest_evaluation
            .is_some());

        assert!(matches!(
            store.rollback_wiki_skill_version("skill-book-ingest", baseline_version_id),
            Err(BrainError::KnowledgeNotFound(_))
        ));
    }

    #[test]
    fn test_interrupted_skill_benchmarks_fail_and_can_be_restarted() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book(
                "book-benchmark-restart",
                "/tmp/book-benchmark-restart",
            )])
            .unwrap();
        let base = store.initialize_base("book-benchmark-restart").unwrap();
        let candidate_id = insert_builtin_skill_candidate(&store, "skill-book-ingest");
        let candidate = store
            .get_wiki_skill_detail("skill-book-ingest", None)
            .unwrap()
            .versions
            .into_iter()
            .find(|version| version.id == candidate_id)
            .unwrap();
        store
            .evaluate_wiki_skill_version("skill-book-ingest", &candidate.id, "semantic-ingest")
            .unwrap();

        let queued = store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .unwrap();
        store.recover_interrupted_tasks().unwrap();
        let queued = store.get_wiki_skill_benchmark(&queued.id).unwrap();
        assert_eq!(queued.status, "failed");
        assert!(queued.error.unwrap().contains("服务重启"));

        let running = store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .unwrap();
        store.start_wiki_skill_benchmark(&running.id).unwrap();
        store.recover_interrupted_tasks().unwrap();
        let running = store.get_wiki_skill_benchmark(&running.id).unwrap();
        assert_eq!(running.status, "failed");
        assert!(running.error.unwrap().contains("服务重启"));
        assert!(store
            .prepare_wiki_skill_benchmark(
                "skill-book-ingest",
                &candidate.id,
                "semantic-ingest",
                &base.id,
                "runtime-deepseek-harness",
                "test-model",
            )
            .is_ok());
    }

    #[test]
    fn test_agent_run_inspection_persists_effective_prompt_and_snapshots() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-inspection", "/tmp/book-inspection")])
            .unwrap();
        let base = store.initialize_base("book-inspection").unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({ "question": "什么是证据链？" }),
            )
            .unwrap();
        store
            .save_agent_run_inspection(
                &run.id,
                "系统规则\n<question>什么是证据链？</question>",
                &serde_json::json!([{
                    "id": "skill-book-query",
                    "slug": "book-query",
                    "revision": 1
                }]),
                &serde_json::json!([{
                    "document_type": "purpose",
                    "revision": 1
                }]),
                &["knowledge_search_entries".to_string()],
                &serde_json::json!({ "entry_ids": ["entry-1"] }),
            )
            .unwrap();
        assert!(store
            .save_agent_run_inspection(
                &run.id,
                "后续内容不得覆盖原始快照",
                &serde_json::json!([]),
                &serde_json::json!([]),
                &[],
                &serde_json::json!({}),
            )
            .is_err());

        let inspection = store.get_agent_run_inspection(&run.id).unwrap();
        assert_eq!(inspection.run.id, run.id);
        let snapshot = inspection.snapshot.unwrap();
        assert_eq!(snapshot.prompt_characters, 33);
        assert!(snapshot.prompt_text.contains("证据链"));
        assert_eq!(snapshot.tool_names, vec!["knowledge_search_entries"]);
        assert_eq!(snapshot.skill_snapshots[0]["revision"], 1);
    }

    #[test]
    fn test_task_queue_supports_claim_cancel_retry_and_restart_recovery() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-queue", "/tmp/book-queue")])
            .unwrap();
        let base = store.initialize_base("book-queue").unwrap();
        let task = store
            .create_task(&base.id, "后台研究", "验证耐久队列", "research")
            .unwrap();

        assert_eq!(
            store.queue_task_execution(&task.id).unwrap().status,
            "queued"
        );
        assert_eq!(store.claim_next_queued_task().unwrap().unwrap().id, task.id);
        assert_eq!(
            store.request_task_cancel(&task.id).unwrap().status,
            "running"
        );
        assert_eq!(
            store.cancel_task_execution(&task.id).unwrap().status,
            "cancelled"
        );

        store.queue_task_execution(&task.id).unwrap();
        store.claim_next_queued_task().unwrap();
        assert_eq!(store.recover_interrupted_tasks().unwrap(), 1);
        assert_eq!(store.get_task(&task.id).unwrap().status, "queued");
        assert_eq!(
            store.request_task_cancel(&task.id).unwrap().status,
            "cancelled"
        );
    }

    #[test]
    fn test_semantic_changes_validate_current_citations_and_revision_atomically() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-change", "/tmp/book-change")])
            .unwrap();
        let base = store.initialize_base("book-change").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source(
                    "change",
                    "source-entry-change",
                    "span-change",
                )],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_compile",
                &serde_json::json!({}),
            )
            .unwrap();

        let invalid = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "非法引用",
                "测试",
                "invalid-citation",
                &[semantic_candidate("span-from-another-book", "无效主题")],
            )
            .unwrap_err();
        assert!(invalid.to_string().contains("不属于当前书籍版本"));

        let first = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "首次写入",
                "测试",
                "first-change",
                &[semantic_candidate("span-change", "语义主题")],
            )
            .unwrap();
        assert_eq!(first.changes[0].classification, "new");
        assert_eq!(first.classification_summary["new"], 1);
        assert_eq!(first.citation_audit["passed"], true);
        assert_eq!(first.impact_summary["entries"], 1);
        store.resolve_change_set(&first.id, true, "确认").unwrap();
        let second = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "增量更新",
                "测试",
                "second-change",
                &[semantic_candidate("span-change", "待更新标题")],
            )
            .unwrap();
        assert_eq!(second.changes[0].classification, "update");
        assert_eq!(second.classification_summary["update"], 1);
        assert_eq!(second.changes[0].impact["claims"], 1);
        assert_eq!(second.changes[0].impact["citations"], 2);
        let entry_id = second.changes[0].object_id.clone();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET revision = revision + 1 WHERE id = ?1",
                    params![entry_id],
                )?;
                Ok(())
            })
            .unwrap();

        let conflict = store
            .resolve_change_set(&second.id, true, "确认")
            .unwrap_err();
        assert!(conflict.to_string().contains("需要重新生成"));
        assert_eq!(store.get_change_set(&second.id).unwrap().status, "proposed");
        assert_eq!(store.get_entry(&entry_id).unwrap().entry.title, "语义主题");
    }

    #[test]
    fn test_no_material_compile_record_finishes_without_pending_review() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-no-material", "/tmp/book-no-material")])
            .unwrap();
        let base = store.initialize_base("book-no-material").unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let change_set = store
            .create_no_material_change_set(
                &base.id,
                &run.id,
                "无实质变化",
                "固定检查完成",
                "no-material-compile",
            )
            .unwrap();
        assert_eq!(change_set.status, "applied");
        assert!(change_set.changes.is_empty());
        assert_eq!(change_set.classification_summary["no_material"], 1);

        let ready = store
            .mark_semantic_compile_no_material(&base.id, &change_set.id, 1, 1)
            .unwrap();
        assert_eq!(ready.compile_state, "ready");
        assert_eq!(ready.compile_phase, "completed");
        assert_eq!(ready.pending_review_count, 0);
    }

    #[test]
    fn test_disputed_semantic_update_is_high_risk_and_marks_claims_disputed() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-disputed", "/tmp/book-disputed")])
            .unwrap();
        let base = store.initialize_base("book-disputed").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source(
                    "disputed",
                    "source-entry-disputed",
                    "span-disputed",
                )],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let first = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "初始知识",
                "测试",
                "disputed-first",
                &[semantic_candidate("span-disputed", "原结论")],
            )
            .unwrap();
        store.resolve_change_set(&first.id, true, "确认").unwrap();

        let mut disputed = semantic_candidate("span-disputed", "存在冲突的结论");
        disputed["_classification"] = serde_json::Value::String("disputed".to_string());
        let change_set = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "争议知识",
                "来源无法消解",
                "disputed-second",
                &[disputed],
            )
            .unwrap();
        assert_eq!(change_set.risk_level, "high");
        assert_eq!(change_set.changes[0].classification, "disputed");
        store
            .resolve_change_set(&change_set.id, true, "保留争议")
            .unwrap();
        let verification_status = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT verification_status FROM knowledge_claims
                     WHERE entry_id = ?1 LIMIT 1",
                    params![change_set.changes[0].object_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(verification_status, "disputed");
    }

    #[test]
    fn test_rejecting_semantic_change_set_reopens_source_compile_checkpoint() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-reject", "/tmp/book-reject")])
            .unwrap();
        let base = store.initialize_base("book-reject").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source(
                    "reject",
                    "source-entry-reject",
                    "span-reject",
                )],
            )
            .unwrap();
        let spans = store
            .list_source_spans_pending_compile(&base.id, "compile-v1")
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_compile",
                &serde_json::json!({}),
            )
            .unwrap();
        let change_set = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "待驳回变更",
                "测试检查点恢复",
                "rejected-checkpoint",
                &[semantic_candidate("span-reject", "待驳回主题")],
            )
            .unwrap();
        store
            .record_compile_checkpoints(&base.id, &spans, &change_set.id, "compile-v1")
            .unwrap();
        assert!(store
            .list_source_spans_pending_compile(&base.id, "compile-v1")
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .list_source_spans_pending_compile(&base.id, "compile-v2")
                .unwrap()
                .len(),
            1,
            "changing the effective compile context must invalidate the checkpoint"
        );

        store
            .resolve_change_set(&change_set.id, false, "需要重新分析")
            .unwrap();

        assert_eq!(
            store
                .list_source_spans_pending_compile(&base.id, "compile-v1")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn test_lint_reports_missing_semantic_layer_without_modifying_sources() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-lint", "/tmp/book-lint")])
            .unwrap();
        let base = store.initialize_base("book-lint").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("lint", "source-entry-lint", "span-lint")],
            )
            .unwrap();

        let report = store.lint_knowledge_base(&base.id).unwrap();

        assert_eq!(report.source_span_count, 1);
        assert_eq!(report.semantic_entry_count, 0);
        assert_eq!(report.state, "warning");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "no-semantic-entries"));
        assert_eq!(
            store.list_entries(&base.id, None, None, 10).unwrap().len(),
            1
        );
    }

    #[test]
    fn test_agent_usage_stats_filter_completed_runs_by_caller_and_date() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-usage", "/tmp/book-usage")])
            .unwrap();
        let base = store.initialize_base("book-usage").unwrap();

        let qa_run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({"question": "核心主题"}),
            )
            .unwrap();
        store
            .complete_agent_run_with_usage(
                &qa_run.id,
                &serde_json::json!({"answer": "回答"}),
                &AgentTokenUsage::estimated(120, 48),
            )
            .unwrap();

        let task_run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_task_research",
                &serde_json::json!({"knowledge_task_id": "task-1"}),
            )
            .unwrap();
        store
            .complete_agent_run_with_usage(
                &task_run.id,
                &serde_json::json!({"answer": "研究结果"}),
                &AgentTokenUsage::estimated(200, 90),
            )
            .unwrap();

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let all = store.get_agent_usage_stats(&today, &today, None).unwrap();
        assert_eq!(all.totals.runs, 2);
        assert_eq!(all.totals.input_tokens, 320);
        assert_eq!(all.totals.output_tokens, 138);
        assert_eq!(all.totals.total_tokens, 458);
        assert_eq!(all.by_caller.len(), 2);

        let qa = store
            .get_agent_usage_stats(&today, &today, Some("knowledge_qa"))
            .unwrap();
        assert_eq!(qa.totals.runs, 1);
        assert_eq!(qa.totals.total_tokens, 168);
        assert_eq!(qa.by_caller[0].caller, "knowledge_qa");
        assert_eq!(qa.usage_source, "estimated");
    }

    #[test]
    fn test_agent_usage_stats_rejects_reversed_date_range() {
        let (store, _dir) = test_store();
        let error = store
            .get_agent_usage_stats("2026-09-13", "2026-09-01", None)
            .unwrap_err();

        assert!(error.to_string().contains("开始日期"));
    }

    #[test]
    fn test_agent_run_records_failure_message() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-agent", "/tmp/book-agent-fail")])
            .unwrap();
        let base = store.initialize_base("book-agent").unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();

        let failed = store.fail_agent_run(&run.id, "认证失败").unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.error.as_deref(), Some("认证失败"));
        let events = store.list_agent_run_events(&run.id).unwrap();
        assert_eq!(events[1].event_type, "run.failed");
        assert_eq!(events[1].payload["error"], "认证失败");
    }

    #[test]
    fn test_cancel_agent_run_marks_terminal_event_and_revokes_capability() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-cancel-run", "/tmp/book-cancel-run")])
            .unwrap();
        let base = store.initialize_base("book-cancel-run").unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_task_research",
                &serde_json::json!({}),
            )
            .unwrap();
        let capability = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &["book_get_context".to_string()],
                300,
            )
            .unwrap();

        let cancelled = store.cancel_agent_run(&run.id).unwrap();

        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(
            store
                .list_agent_run_events(&run.id)
                .unwrap()
                .last()
                .unwrap()
                .event_type,
            "run.cancelled"
        );
        assert!(store
            .validate_agent_run_token(&capability.token)
            .unwrap_err()
            .to_string()
            .contains("无效"));
    }

    #[test]
    fn test_knowledge_task_execution_lifecycle_persists_result() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-task", "/tmp/book-task")])
            .unwrap();
        let base = store.initialize_base("book-task").unwrap();
        let task = store
            .create_task(&base.id, "梳理核心观点", "输出证据化摘要", "research")
            .unwrap();

        let running = store.start_task_execution(&task.id).unwrap();
        assert_eq!(running.status, "running");

        let completed = store
            .complete_task_execution(&task.id, "核心观点已经完成梳理。[S1]")
            .unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.result_summary, "核心观点已经完成梳理。[S1]");
        assert_eq!(store.get_task(&task.id).unwrap(), completed);
    }

    #[test]
    fn test_external_research_requires_task_grant_domain_scope_and_quota() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-external", "/tmp/book-external")])
            .unwrap();
        let base = store.initialize_base("book-external").unwrap();
        let task = store
            .create_task_with_options(
                &base.id,
                "核验外部资料",
                "只访问授权官网",
                "research",
                "report",
                true,
                &["Docs.Example.com".to_string()],
                1,
            )
            .unwrap();
        assert!(task.external_research_enabled);
        assert_eq!(task.external_domains, vec!["docs.example.com"]);
        assert_eq!(task.external_request_limit, 1);
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_task_research",
                &serde_json::json!({"knowledge_task_id": task.id}),
            )
            .unwrap();

        let wrong_domain = store
            .authorize_external_research_request(&run.id, "other.example.com")
            .unwrap_err();
        assert!(wrong_domain.to_string().contains("授权范围"));

        let granted = store
            .authorize_external_research_request(&run.id, "api.docs.example.com")
            .unwrap();
        assert_eq!(granted.request_number, 1);
        assert_eq!(granted.request_limit, 1);

        let exhausted = store
            .authorize_external_research_request(&run.id, "docs.example.com")
            .unwrap_err();
        assert!(exhausted.to_string().contains("额度"));
    }

    #[test]
    fn test_external_research_defaults_to_denied() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-no-external", "/tmp/book-no-external")])
            .unwrap();
        let base = store.initialize_base("book-no-external").unwrap();
        let task = store
            .create_task(&base.id, "仅书内研究", "", "research")
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_task_research",
                &serde_json::json!({"knowledge_task_id": task.id}),
            )
            .unwrap();

        let error = store
            .authorize_external_research_request(&run.id, "example.com")
            .unwrap_err();
        assert!(error.to_string().contains("未授权"));
    }

    #[test]
    fn test_compile_activity_is_claimed_once_and_recovered_when_stale() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-compile-job", "/tmp/book-compile-job")])
            .unwrap();
        let base = store.initialize_base("book-compile-job").unwrap();

        let started = store.begin_semantic_compile(&base.id, 12).unwrap();
        assert_eq!(started.compile_state, "compiling");
        assert_eq!(started.compile_phase, "queued");
        assert_eq!(started.compile_total_sources, 12);
        assert!(store.begin_semantic_compile(&base.id, 12).is_err());

        store
            .update_compile_activity(
                &base.id,
                "thinking",
                "模型正在分析第 1/3 批",
                1,
                3,
                Some("run-1"),
            )
            .unwrap();
        let active = store.get_base(&base.id).unwrap();
        assert_eq!(active.compile_phase, "thinking");
        assert_eq!(active.compile_current_batch, 1);
        assert_eq!(active.compile_total_batches, 3);
        assert_eq!(active.compile_active_run_id.as_deref(), Some("run-1"));

        store.recover_interrupted_tasks().unwrap();
        let recovered = store.get_base(&base.id).unwrap();
        assert_eq!(recovered.compile_state, "failed");
        assert_eq!(recovered.compile_phase, "failed");
        assert!(recovered.compile_message.contains("服务重启"));
        assert!(recovered.compile_active_run_id.is_none());

        store.begin_semantic_compile(&base.id, 12).unwrap();
        store
            .mark_semantic_compile_waiting_review(&base.id, "change-set-1", 12, 12)
            .unwrap();
        store.recover_interrupted_tasks().unwrap();
        let review = store.get_base(&base.id).unwrap();
        assert_eq!(review.compile_state, "compiling");
        assert_eq!(review.compile_phase, "waiting_review");
        assert_eq!(
            review.compile_change_set_id.as_deref(),
            Some("change-set-1")
        );
    }

    #[test]
    fn test_knowledge_task_can_retry_after_failure() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-retry", "/tmp/book-retry")])
            .unwrap();
        let base = store.initialize_base("book-retry").unwrap();
        let task = store
            .create_task(&base.id, "核验事实", "", "review")
            .unwrap();

        store.start_task_execution(&task.id).unwrap();
        let failed = store
            .fail_task_execution(&task.id, "执行失败：凭据未配置")
            .unwrap();
        assert_eq!(failed.status, "failed");
        assert!(failed.result_summary.contains("凭据未配置"));

        let retried = store.start_task_execution(&task.id).unwrap();
        assert_eq!(retried.status, "running");
        assert!(retried.result_summary.is_empty());
    }

    #[test]
    fn test_create_knowledge_task_rejects_oversized_prompt_fields() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-limits", "/tmp/book-limits")])
            .unwrap();
        let base = store.initialize_base("book-limits").unwrap();

        let error = store
            .create_task(&base.id, &"标题".repeat(101), "", "research")
            .unwrap_err();

        assert!(error.to_string().contains("200"));
    }

    #[test]
    fn test_model_provider_profiles_support_multiple_configs_and_runtime_selection() {
        let (store, _dir) = test_store();
        let profile = store
            .list_runtime_profiles()
            .unwrap()
            .into_iter()
            .find(|profile| profile.id == "runtime-deepseek-harness")
            .unwrap();
        let aliyun = store
            .save_model_provider_profile(
                "aliyun-bailian",
                "阿里云百炼",
                "openai-completions",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "glm-5.2",
                "keychain",
                "",
                true,
                true,
                0,
            )
            .unwrap();
        let second = store
            .save_model_provider_profile(
                "openrouter",
                "OpenRouter",
                "openai-completions",
                "https://openrouter.ai/api/v1",
                "openai/gpt-5-mini",
                "environment",
                "OPENROUTER_API_KEY",
                false,
                true,
                0,
            )
            .unwrap();

        let saved = store
            .save_runtime_profile(
                &profile.id,
                &profile.executable,
                "",
                Some(&aliyun.provider_id),
                true,
                profile.revision,
            )
            .unwrap();

        assert_eq!(saved.provider_config.as_ref(), Some(&aliyun));
        assert_eq!(saved.model, "glm-5.2");
        assert_eq!(store.list_model_provider_profiles().unwrap().len(), 2);
        assert_eq!(second.api_key_env, "OPENROUTER_API_KEY");
        let raw_config = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT config_json FROM agent_runtime_profiles WHERE id = ?1",
                    params![profile.id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(raw_config, "{}");
        let raw_provider: String = store
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT base_url || '|' || model || '|' || api_key_env
                     FROM llm_provider_profiles WHERE id = 'aliyun-bailian'",
                    [],
                    |row| row.get(0),
                )
                .map_err(Into::into)
            })
            .unwrap();
        assert!(!raw_provider.contains("sk-"));
    }

    #[test]
    fn test_model_provider_profile_rejects_invalid_environment_variable() {
        let (store, _dir) = test_store();
        let error = store
            .save_model_provider_profile(
                "aliyun-bailian",
                "阿里云百炼",
                "openai-completions",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "glm-5.2",
                "environment",
                "CUSTOM-LLM-KEY",
                false,
                true,
                0,
            )
            .unwrap_err();

        assert!(error.to_string().contains("环境变量"));
    }

    #[test]
    fn test_base_lifecycle_blocks_new_work_and_can_resume() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-lifecycle", "/tmp/book-lifecycle")])
            .unwrap();
        let base = store.initialize_base("book-lifecycle").unwrap();

        let paused = store.set_base_lifecycle(&base.id, "paused").unwrap();
        assert_eq!(paused.lifecycle, "paused");
        assert!(store
            .create_task(&base.id, "不应创建", "", "research")
            .unwrap_err()
            .to_string()
            .contains("暂停"));
        assert!(store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({})
            )
            .unwrap_err()
            .to_string()
            .contains("暂停"));

        assert_eq!(
            store
                .set_base_lifecycle(&base.id, "active")
                .unwrap()
                .lifecycle,
            "active"
        );
        assert!(store
            .create_task(&base.id, "恢复后可创建", "", "research")
            .is_ok());
    }

    #[test]
    fn test_entry_page_reports_total_and_stable_offsets() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-page", "/tmp/book-page")])
            .unwrap();
        let base = store.initialize_base("book-page").unwrap();
        let sources = (0..3)
            .map(|index| {
                sample_source(
                    &format!("page-{index}"),
                    &format!("entry-page-{index}"),
                    &format!("span-page-{index}"),
                )
            })
            .collect::<Vec<_>>();
        store.sync_markdown_sources(&base.id, &sources).unwrap();

        let first = store.list_entries_page(&base.id, None, None, 0, 1).unwrap();
        let second = store.list_entries_page(&base.id, None, None, 1, 1).unwrap();
        assert_eq!(first.total, 3);
        assert!(first.has_more);
        assert_eq!(first.entries.len(), 1);
        assert_eq!(second.offset, 1);
        assert_ne!(first.entries[0].id, second.entries[0].id);
    }

    #[test]
    fn test_manual_entry_edit_is_reviewed_and_human_protected() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-edit", "/tmp/book-edit")])
            .unwrap();
        let base = store.initialize_base("book-edit").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("edit", "entry-source-edit", "span-edit")],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "初始主题",
                "测试",
                "test-edit-initial",
                &[semantic_candidate("span-edit", "初始标题")],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        let entry_id = stable_id("entry", &format!("{}:concept:semantic-concept", base.id));
        let original = store.get_entry(&entry_id).unwrap();

        let edit = store
            .propose_entry_edit(
                &entry_id,
                &KnowledgeEntryEditProposal {
                    title: "人工标题",
                    summary: "人工摘要",
                    content_md: "## 人工正文",
                    aliases: &["人工别名".to_string()],
                    status: "verified",
                    expected_revision: original.revision,
                },
            )
            .unwrap();
        assert_eq!(edit.status, "proposed");
        assert_eq!(edit.risk_level, "high");
        assert_eq!(store.get_entry(&entry_id).unwrap().entry.title, "初始标题");

        store
            .resolve_change_set(&edit.id, true, "人工确认")
            .unwrap();
        let updated = store.get_entry(&entry_id).unwrap();
        assert_eq!(updated.entry.title, "人工标题");
        assert_eq!(updated.entry.status, "verified");
        assert_eq!(updated.edit_policy, "human_protected");
        assert!(updated
            .versions
            .iter()
            .any(|version| version.title == "初始标题"));
        assert_eq!(updated.claims.len(), 1);
    }

    #[test]
    fn test_manual_merge_archives_sources_and_redirects_relations_atomically() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-merge", "/tmp/book-merge")])
            .unwrap();
        let base = store.initialize_base("book-merge").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("merge", "source-entry-merge", "span-merge")],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let candidate = |slug: &str, title: &str, relations: serde_json::Value| {
            serde_json::json!({
                "entry_type": "concept", "slug": slug, "title": title,
                "summary": format!("{title}摘要"), "content_md": format!("{title}正文"),
                "aliases": [], "confidence": 0.8, "citations": ["span-merge"],
                "claims": [], "relations": relations,
            })
        };
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "合并准备",
                "测试",
                "test-merge-initial",
                &[
                    candidate("alpha", "甲", serde_json::json!([])),
                    candidate(
                        "beta",
                        "乙",
                        serde_json::json!([{
                            "to_slug": "gamma", "relation_type": "explains",
                            "strength": 0.8, "evidence": "乙解释丙"
                        }]),
                    ),
                    candidate(
                        "gamma",
                        "丙",
                        serde_json::json!([{
                            "to_slug": "beta", "relation_type": "supports",
                            "strength": 0.7, "evidence": "丙支持乙"
                        }]),
                    ),
                ],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        let alpha_id = stable_id("entry", &format!("{}:concept:alpha", base.id));
        let beta_id = stable_id("entry", &format!("{}:concept:beta", base.id));
        let gamma_id = stable_id("entry", &format!("{}:concept:gamma", base.id));
        let alpha = store.get_entry(&alpha_id).unwrap();
        let beta = store.get_entry(&beta_id).unwrap();

        let merge = store
            .propose_entry_merge(
                &alpha_id,
                &KnowledgeEntryMergeProposal {
                    title: "甲乙合并",
                    summary: "合并摘要",
                    content_md: "甲与乙的完整内容",
                    aliases: &["甲".to_string(), "乙".to_string()],
                    status: "verified",
                    expected_revision: alpha.revision,
                    sources: &[KnowledgeEntryRevision {
                        entry_id: beta_id.clone(),
                        expected_revision: beta.revision,
                    }],
                },
            )
            .unwrap();
        assert!(merge
            .changes
            .iter()
            .all(|change| change.operation == "merge"));
        assert_eq!(store.get_entry(&beta_id).unwrap().entry.status, "draft");

        store
            .resolve_change_set(&merge.id, true, "确认合并")
            .unwrap();
        let merged = store.get_entry(&alpha_id).unwrap();
        assert_eq!(merged.entry.title, "甲乙合并");
        assert_eq!(merged.entry.status, "verified");
        assert_eq!(store.get_entry(&beta_id).unwrap().entry.status, "archived");
        let path = store
            .find_graph_path(&base.id, &gamma_id, &alpha_id, 3)
            .unwrap();
        assert_eq!(path.entries.len(), 2);
        assert!(store
            .list_entries(&base.id, Some("乙正文"), None, 20)
            .unwrap()
            .iter()
            .all(|entry| entry.id != beta_id));
    }

    #[test]
    fn test_manual_split_archives_original_and_creates_cited_parts_after_review() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-split", "/tmp/book-split")])
            .unwrap();
        let base = store.initialize_base("book-split").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("split", "source-entry-split", "span-split")],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "拆分准备",
                "测试",
                "test-split-initial",
                &[semantic_candidate("span-split", "复合主题")],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        let original_id = stable_id("entry", &format!("{}:concept:semantic-concept", base.id));
        let original = store.get_entry(&original_id).unwrap();

        let split = store
            .propose_entry_split(
                &original_id,
                &KnowledgeEntrySplitProposal {
                    expected_revision: original.revision,
                    parts: &[
                        KnowledgeEntrySplitPart {
                            title: "主题甲".to_string(),
                            summary: "甲摘要".to_string(),
                            content_md: "甲正文".to_string(),
                            aliases: Vec::new(),
                        },
                        KnowledgeEntrySplitPart {
                            title: "主题乙".to_string(),
                            summary: "乙摘要".to_string(),
                            content_md: "乙正文".to_string(),
                            aliases: vec!["乙别名".to_string()],
                        },
                    ],
                },
            )
            .unwrap();
        assert_eq!(split.changes.len(), 3);
        assert!(split
            .changes
            .iter()
            .all(|change| change.operation == "split"));
        assert_eq!(store.get_entry(&original_id).unwrap().entry.status, "draft");

        store
            .resolve_change_set(&split.id, true, "确认拆分")
            .unwrap();
        assert_eq!(
            store.get_entry(&original_id).unwrap().entry.status,
            "archived"
        );
        let parts = store
            .list_entries(&base.id, Some("主题"), Some("concept"), 20)
            .unwrap();
        assert_eq!(parts.len(), 2);
        for part in parts {
            assert_eq!(store.get_entry(&part.id).unwrap().citations.len(), 1);
        }
    }

    #[test]
    fn test_reader_selection_creates_cited_review_candidate() {
        let (store, dir) = test_store();
        let book_path = dir.path().join("selection-book");
        std::fs::create_dir_all(&book_path).unwrap();
        store
            .save_reader_books(&[sample_book("book-selection", book_path.to_str().unwrap())])
            .unwrap();
        let base = store.initialize_base("book-selection").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source(
                    "selection",
                    "entry-source-selection",
                    "span-selection",
                )],
            )
            .unwrap();

        let change_set = store
            .propose_reader_selection(
                &base.id,
                "selection.md",
                "可追溯的来源正文",
                Some("阅读摘录"),
            )
            .unwrap();
        assert_eq!(change_set.status, "proposed");
        assert_eq!(change_set.risk_level, "high");
        assert_eq!(
            change_set.changes[0].after["citations"][0],
            "span-selection"
        );
    }

    #[test]
    fn test_graph_overview_and_path_are_scoped_to_one_book() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-graph", "/tmp/book-graph")])
            .unwrap();
        let base = store.initialize_base("book-graph").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("graph", "entry-source-graph", "span-graph")],
            )
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let candidate = |slug: &str, title: &str, relations: serde_json::Value| {
            serde_json::json!({
                "entry_type": "concept", "slug": slug, "title": title,
                "summary": format!("{title}摘要"), "content_md": format!("{title}正文"),
                "aliases": [], "confidence": 0.8, "citations": ["span-graph"],
                "claims": [], "relations": relations,
            })
        };
        let changes = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "图谱",
                "测试",
                "test-graph",
                &[
                    candidate(
                        "alpha",
                        "甲",
                        serde_json::json!([{
                        "to_slug": "beta", "relation_type": "explains",
                            "strength": 0.9, "evidence": "甲解释乙"
                        }]),
                    ),
                    candidate("beta", "乙", serde_json::json!([])),
                    candidate("gamma", "丙", serde_json::json!([])),
                ],
            )
            .unwrap();
        store.resolve_change_set(&changes.id, true, "").unwrap();

        let overview = store.get_graph_overview(&base.id, 20).unwrap();
        assert_eq!(overview.relation_count, 1);
        assert_eq!(overview.bridge_entries.len(), 2);
        assert_eq!(overview.orphan_entries.len(), 1);
        assert_eq!(overview.orphan_entries[0].title, "丙");
        let alpha = stable_id("entry", &format!("{}:concept:alpha", base.id));
        let beta = stable_id("entry", &format!("{}:concept:beta", base.id));
        let path = store.find_graph_path(&base.id, &alpha, &beta, 5).unwrap();
        assert_eq!(
            path.entries
                .iter()
                .map(|entry| entry.title.as_str())
                .collect::<Vec<_>>(),
            vec!["甲", "乙"]
        );
        let snapshot = store.get_graph_snapshot(&base.id, 120).unwrap();
        assert_eq!(snapshot.entries.len(), 3);
        assert_eq!(snapshot.relations.len(), 1);
        assert_eq!(snapshot.relations[0].from_entry_id, alpha);
        assert_eq!(snapshot.relations[0].to_entry_id, beta);
        assert!(!snapshot.truncated);
    }

    #[test]
    fn test_stale_running_knowledge_task_can_recover_after_interruption() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-stale", "/tmp/book-stale")])
            .unwrap();
        let base = store.initialize_base("book-stale").unwrap();
        let task = store
            .create_task(&base.id, "恢复任务", "", "research")
            .unwrap();
        store.start_task_execution(&task.id).unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET updated_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
                    params![task.id],
                )?;
                Ok(())
            })
            .unwrap();

        assert_eq!(
            store.start_task_execution(&task.id).unwrap().status,
            "running"
        );
    }

    #[test]
    fn test_task_leases_serialize_execution_per_knowledge_base() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-lease", "/tmp/book-lease")])
            .unwrap();
        let base = store.initialize_base("book-lease").unwrap();
        let first = store
            .create_task(&base.id, "任务一", "", "research")
            .unwrap();
        let second = store
            .create_task(&base.id, "任务二", "", "research")
            .unwrap();
        store.queue_task_execution(&first.id).unwrap();
        store.queue_task_execution(&second.id).unwrap();

        let claimed = store.claim_next_queued_task().unwrap().unwrap();
        assert_eq!(claimed.id, first.id);
        assert!(store.claim_next_queued_task().unwrap().is_none());
        assert!(store.renew_task_lease(&first.id).unwrap());
        store.complete_task_execution(&first.id, "完成").unwrap();
        assert_eq!(
            store.claim_next_queued_task().unwrap().unwrap().id,
            second.id
        );
    }

    #[test]
    fn test_failed_task_retries_with_backoff_then_exhausts_attempts() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-backoff", "/tmp/book-backoff")])
            .unwrap();
        let base = store.initialize_base("book-backoff").unwrap();
        let task = store
            .create_task(&base.id, "重试任务", "", "research")
            .unwrap();
        store.queue_task_execution(&task.id).unwrap();
        store.claim_next_queued_task().unwrap().unwrap();

        let retry = store
            .retry_or_fail_task_execution(&task.id, "临时错误")
            .unwrap();
        assert_eq!(retry.status, "queued");
        assert!(store.claim_next_queued_task().unwrap().is_none());

        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET next_attempt_at = '2020-01-01T00:00:00Z',
                         attempt_count = max_attempts - 1 WHERE id = ?1",
                    params![task.id],
                )?;
                Ok(())
            })
            .unwrap();
        store.claim_next_queued_task().unwrap().unwrap();
        assert_eq!(
            store
                .retry_or_fail_task_execution(&task.id, "最终错误")
                .unwrap()
                .status,
            "failed"
        );
    }

    #[test]
    fn test_expired_task_lease_is_requeued() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-expired", "/tmp/book-expired")])
            .unwrap();
        let base = store.initialize_base("book-expired").unwrap();
        let task = store
            .create_task(&base.id, "过期任务", "", "research")
            .unwrap();
        store.start_task_execution(&task.id).unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET lease_expires_at = '2020-01-01T00:00:00Z'
                     WHERE id = ?1",
                    params![task.id],
                )?;
                Ok(())
            })
            .unwrap();

        assert_eq!(store.recover_expired_task_leases().unwrap(), 1);
        assert_eq!(store.get_task(&task.id).unwrap().status, "queued");
    }

    #[test]
    fn test_entry_page_uses_database_pagination_beyond_first_two_hundred_rows() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-page", "/tmp/book-page")])
            .unwrap();
        let base = store.initialize_base("book-page").unwrap();
        store
            .db
            .transaction(|conn| {
                for index in 0..250 {
                    conn.execute(
                        "INSERT INTO knowledge_entries
                            (id, knowledge_base_id, entry_type, slug, title, summary, content_md,
                             status, created_at, updated_at)
                         VALUES (?1, ?2, 'concept', ?3, ?4, '', '', 'verified',
                                 CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                        params![
                            format!("page-entry-{index:03}"),
                            base.id,
                            format!("page-entry-{index:03}"),
                            format!("分页实体 {index:03}")
                        ],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        let page = store
            .list_entries_page(&base.id, None, Some("concept"), 200, 25)
            .unwrap();
        assert_eq!(page.entries.len(), 25);
        assert_eq!(page.offset, 200);
        assert_eq!(page.total, 250);
        assert!(page.has_more);
    }

    #[test]
    #[ignore = "100k-fragment scale regression; run explicitly before release"]
    fn test_100k_fragment_fts_query_uses_virtual_index_without_full_scan() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("book-scale", "/tmp/book-scale")])
            .unwrap();
        let base = store.initialize_base("book-scale").unwrap();
        store
            .db
            .transaction(|conn| {
                conn.execute(
                    "INSERT INTO source_documents
                        (id, knowledge_base_id, source_type, original_path, relative_path, title,
                         mime_type, current_version_id, created_at, updated_at)
                     VALUES ('scale-source', ?1, 'markdown', '/tmp/book-scale/all.md', 'all.md',
                             '规模测试', 'text/markdown', 'scale-version', CURRENT_TIMESTAMP,
                             CURRENT_TIMESTAMP)",
                    params![base.id],
                )?;
                conn.execute(
                    "INSERT INTO source_versions
                        (id, source_document_id, content_hash, size_bytes, extraction_version,
                         extraction_status, created_at)
                     VALUES ('scale-version', 'scale-source', 'scale-hash', 1, 'markdown-v1',
                             'ready', CURRENT_TIMESTAMP)",
                    [],
                )?;
                let mut span_statement = conn.prepare(
                    "INSERT INTO source_spans
                        (id, knowledge_base_id, source_version_id, ordinal, heading, content,
                         content_hash, token_estimate)
                     VALUES (?1, ?2, 'scale-version', ?3, ?4, ?5, ?6, 3)",
                )?;
                let mut fts_statement = conn.prepare(
                    "INSERT INTO source_spans_fts
                        (span_id, knowledge_base_id, source_title, heading, content, cjk_terms)
                     VALUES (?1, ?2, '规模测试', ?3, ?4, ?5)",
                )?;
                for index in 0..100_000 {
                    let id = format!("scale-span-{index}");
                    let is_target = index == 99_999;
                    let heading = if is_target { "Scalability needle" } else { "Fragment" };
                    let content = if is_target { "unique scalability needle" } else { "ordinary fragment" };
                    span_statement.execute(params![id, base.id, index, heading, content, id])?;
                    fts_statement.execute(params![id, base.id, heading, content, if is_target { "scalability needle" } else { "fragment" }])?;
                }
                conn.execute(
                    "INSERT INTO app_state (key, value, updated_at) VALUES (?1, '1', CURRENT_TIMESTAMP)
                     ON CONFLICT(key) DO UPDATE SET value = '1'",
                    params![KNOWLEDGE_FTS_CONTENT_VERSION_KEY],
                )?;
                Ok(())
            })
            .unwrap();

        let matches = store
            .search_source_spans(&base.id, "scalability needle", 5)
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "scale-span-99999");
        let plan = store
            .db
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    "EXPLAIN QUERY PLAN SELECT span_id FROM source_spans_fts
                 WHERE source_spans_fts MATCH 'scalability' AND knowledge_base_id = ?1 LIMIT 5",
                )?;
                let rows = statement.query_map(params![base.id], |row| row.get::<_, String>(3))?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            })
            .unwrap();
        assert!(plan
            .iter()
            .any(|detail| detail.contains("VIRTUAL TABLE INDEX")));
    }
}
