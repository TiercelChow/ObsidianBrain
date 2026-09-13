use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::book_wiki::{
    AgentRun, BookKind, BookKnowledgeCard, ConfigDocument, KnowledgeBaseSummary, KnowledgeCitation,
    KnowledgeConversationDetail, KnowledgeConversationSummary, KnowledgeEntryDetail,
    KnowledgeEntrySummary, KnowledgeMessage, KnowledgeTask, ReaderBook, RuntimeProfile,
    RuntimeProviderConfig,
};

const LEGACY_BOOKS_KEY: &str = "reader_books";
const BOOKS_MIGRATED_KEY: &str = "reader_books_table_migrated";

#[derive(Clone)]
pub struct BookWikiStore {
    db: Arc<SqliteStore>,
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
        let book_exists = self.db.with_connection(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM reader_books WHERE id = ?1 AND shelf_state = 'active'",
                    params![book_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
        })?;
        if !book_exists {
            return Err(BrainError::KnowledgeNotFound(book_id.to_string()));
        }
        if let Some(existing) = self.get_base_by_book_id(book_id)? {
            return Ok(existing);
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
                        kb.lifecycle, kb.sync_state, kb.health_state, kb.last_error,
                        kb.last_synced_at, kb.last_scanned_at,
                        (SELECT COUNT(*) FROM source_documents sd
                          WHERE sd.knowledge_base_id = kb.id AND sd.sync_status = 'current'),
                        (SELECT COUNT(*) FROM knowledge_entries ke
                          WHERE ke.knowledge_base_id = kb.id AND ke.status != 'archived'),
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

    pub fn replace_markdown_sources(
        &self,
        base_id: &str,
        sources: &[MarkdownSourceDraft],
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
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
                "DELETE FROM knowledge_entries
                 WHERE knowledge_base_id = ?1 AND entry_type = 'source_section'",
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
                conn.execute(
                    "DELETE FROM source_spans WHERE source_version_id = ?1",
                    params![source.version_id],
                )?;

                for (ordinal, section) in source.sections.iter().enumerate() {
                    conn.execute(
                        "INSERT INTO source_spans
                         (id, knowledge_base_id, source_version_id, ordinal, heading, anchor,
                          line_start, line_end, content, content_hash, token_estimate)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
                            revision = knowledge_entries.revision + 1,
                            updated_at = excluded.updated_at",
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
                    conn.execute(
                        "INSERT INTO knowledge_entries_fts
                         (entry_id, knowledge_base_id, title, summary, content_md)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            section.entry_id,
                            base_id,
                            section.title,
                            section.summary,
                            section.content_md,
                        ],
                    )?;
                    conn.execute(
                        "INSERT INTO knowledge_citations
                         (id, knowledge_base_id, entry_id, source_span_id, quote_text, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            format!("citation-{}", section.id),
                            base_id,
                            section.entry_id,
                            section.id,
                            section.summary,
                            now,
                        ],
                    )?;
                }
            }

            let health = if sources.is_empty() {
                "warning"
            } else {
                "healthy"
            };
            conn.execute(
                "UPDATE knowledge_bases
                 SET sync_state = 'clean', health_state = ?2, last_error = NULL,
                     last_synced_at = ?3, last_scanned_at = ?3,
                     revision = revision + 1, updated_at = ?3
                 WHERE id = ?1",
                params![base_id, health, now],
            )?;
            Ok(())
        })
    }

    pub fn register_pdf_source(
        &self,
        base_id: &str,
        path: &str,
        title: &str,
        hash: &str,
        size: i64,
        modified_at: Option<&str>,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        let source_id = stable_id("source", &format!("{base_id}:{path}"));
        let version_id = stable_id("version", &format!("{source_id}:{hash}"));
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO source_documents
                 (id, knowledge_base_id, source_type, original_path, relative_path, title,
                  mime_type, current_version_id, sync_status, extraction_status, created_at, updated_at)
                 VALUES (?1, ?2, 'pdf', ?3, ?3, ?4, 'application/pdf', ?5,
                         'current', 'pending_harness', ?6, ?6)
                 ON CONFLICT(knowledge_base_id, original_path) DO UPDATE SET
                    title = excluded.title,
                    current_version_id = excluded.current_version_id,
                    sync_status = 'current',
                    extraction_status = 'pending_harness',
                    updated_at = excluded.updated_at",
                params![source_id, base_id, path, title, version_id, now],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO source_versions
                 (id, source_document_id, content_hash, size_bytes, modified_at,
                  extraction_version, extraction_status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending-harness', 'pending_harness', ?6)",
                params![version_id, source_id, hash, size, modified_at, now],
            )?;
            conn.execute(
                "UPDATE knowledge_bases
                 SET sync_state = 'outdated', health_state = 'warning',
                     last_error = NULL, last_scanned_at = ?2,
                     revision = revision + 1, updated_at = ?2
                 WHERE id = ?1",
                params![base_id, now],
            )?;
            Ok(())
        })
    }

    pub fn list_entries(
        &self,
        base_id: &str,
        query: Option<&str>,
        entry_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
        let limit = limit.clamp(1, 200) as i64;
        let patterns = knowledge_query_patterns(query);
        let entry_type = entry_type.filter(|value| !value.trim().is_empty());
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                        ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
                 FROM knowledge_entries ke
                 LEFT JOIN source_documents sd ON sd.id = ke.origin_document_id
                 WHERE ke.knowledge_base_id = ?1
                   AND ke.status != 'archived'
                   AND (?2 IS NULL OR ke.entry_type = ?2)
                   AND (ke.title LIKE ?3 OR ke.summary LIKE ?3 OR ke.content_md LIKE ?3)
                 ORDER BY sd.ordinal, ke.title COLLATE NOCASE
                 LIMIT ?4",
            )?;
            let mut entries = Vec::new();
            let mut seen = HashSet::new();
            for pattern in patterns {
                let remaining = limit - entries.len() as i64;
                if remaining <= 0 {
                    break;
                }
                let rows =
                    stmt.query_map(params![base_id, entry_type, pattern, remaining], |row| {
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
            Ok(entries)
        })
    }

    pub fn get_entry(&self, entry_id: &str) -> Result<KnowledgeEntryDetail, BrainError> {
        self.db.with_connection(|conn| {
            let (entry, content_md) = conn
                .query_row(
                    "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                            ke.summary, ke.status, ke.confidence, sd.relative_path,
                            ke.updated_at, ke.content_md
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
                 ORDER BY sd.ordinal, ss.ordinal",
            )?;
            let citations = stmt
                .query_map(params![entry_id], |row| {
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
            Ok(KnowledgeEntryDetail {
                entry,
                content_md,
                citations,
            })
        })
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
                conn.execute(
                    "INSERT INTO knowledge_message_citations (message_id, ordinal, entry_id)
                     VALUES (?1, ?2, ?3)",
                    params![assistant_message_id, ordinal as i64, entry.id],
                )?;
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
                        kt.task_type, kt.status, kt.result_summary, kt.created_at, kt.updated_at
                 FROM knowledge_tasks kt
                 JOIN knowledge_bases kb ON kb.id = kt.knowledge_base_id
                 JOIN reader_books b ON b.id = kb.book_id
                 WHERE (?1 IS NULL OR kt.knowledge_base_id = ?1)
                 ORDER BY kt.updated_at DESC",
            )?;
            let rows = stmt.query_map(params![base_id], |row| {
                Ok(KnowledgeTask {
                    id: row.get(0)?,
                    knowledge_base_id: row.get(1)?,
                    book_name: row.get(2)?,
                    title: row.get(3)?,
                    description: row.get(4)?,
                    task_type: row.get(5)?,
                    status: row.get(6)?,
                    result_summary: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?;
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
        self.get_base(base_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO knowledge_tasks
                 (id, knowledge_base_id, title, description, task_type, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?6)",
                params![id, base_id, title, description, task_type, now],
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
                        kt.task_type, kt.status, kt.result_summary, kt.created_at, kt.updated_at
                 FROM knowledge_tasks kt
                 JOIN knowledge_bases kb ON kb.id = kt.knowledge_base_id
                 JOIN reader_books b ON b.id = kb.book_id
                 WHERE kt.id = ?1",
                params![task_id],
                |row| {
                    Ok(KnowledgeTask {
                        id: row.get(0)?,
                        knowledge_base_id: row.get(1)?,
                        book_name: row.get(2)?,
                        title: row.get(3)?,
                        description: row.get(4)?,
                        task_type: row.get(5)?,
                        status: row.get(6)?,
                        result_summary: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| BrainError::KnowledgeNotFound(task_id.to_string()))
        })
    }

    pub fn start_task_execution(&self, task_id: &str) -> Result<KnowledgeTask, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE knowledge_tasks
                 SET status = 'running', result_summary = '', updated_at = ?2
                 WHERE id = ?1
                   AND (
                       status IN ('draft', 'failed', 'completed')
                       OR (
                           status = 'running'
                           AND julianday(updated_at) < julianday(?2, '-10 minutes')
                       )
                   )",
                params![task_id, now],
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
                "SELECT id, name, runtime, executable, model, enabled, config_json, revision, updated_at
                 FROM agent_runtime_profiles ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?;
            rows.map(|row| {
                let (id, name, runtime, executable, model, enabled, config_json, revision, updated_at) =
                    row?;
                Ok(RuntimeProfile {
                    id,
                    name,
                    runtime,
                    executable,
                    model,
                    provider_config: parse_runtime_provider_config(&config_json)?,
                    enabled: enabled != 0,
                    revision,
                    updated_at,
                })
            })
            .collect()
        })
    }

    pub fn save_runtime_profile(
        &self,
        profile_id: &str,
        executable: &str,
        model: &str,
        provider_config: Option<&RuntimeProviderConfig>,
        enabled: bool,
        expected_revision: i64,
    ) -> Result<RuntimeProfile, BrainError> {
        if executable.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "运行时可执行文件不能为空".to_string(),
            ));
        }
        let provider_config = provider_config
            .map(|provider| validate_runtime_provider_config(provider, model))
            .transpose()?;
        let config_json = serde_json::to_string(&serde_json::json!({
            "provider": provider_config,
        }))
        .map_err(|error| BrainError::Internal(format!("运行时供应商配置序列化失败: {error}")))?;
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE agent_runtime_profiles
                 SET executable = ?2, model = ?3, config_json = ?4, enabled = ?5,
                     revision = revision + 1, updated_at = ?6
                 WHERE id = ?1 AND revision = ?7",
                params![
                    profile_id,
                    executable.trim(),
                    model.trim(),
                    config_json,
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
        self.get_base(base_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_json = serde_json::to_string(input)
            .map_err(|error| BrainError::Internal(format!("Agent 输入序列化失败: {error}")))?;
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO agent_runs
                 (id, knowledge_base_id, runtime, task_type, status, input_json,
                  started_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?6)",
                params![id, base_id, runtime, task_type, input_json, now],
            )?;
            Ok(())
        })?;
        self.get_agent_run(&id)
    }

    pub fn complete_agent_run(
        &self,
        run_id: &str,
        output: &serde_json::Value,
    ) -> Result<AgentRun, BrainError> {
        let output_json = serde_json::to_string(output)
            .map_err(|error| BrainError::Internal(format!("Agent 输出序列化失败: {error}")))?;
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE agent_runs
                 SET status = 'completed', output_json = ?2, error = NULL, finished_at = ?3
                 WHERE id = ?1 AND status = 'running'",
                params![run_id, output_json, now],
            )?)
        })?;
        if updated == 0 {
            return Err(BrainError::KnowledgeValidation(
                "Agent 运行不存在或已结束".to_string(),
            ));
        }
        self.get_agent_run(run_id)
    }

    pub fn fail_agent_run(&self, run_id: &str, error: &str) -> Result<AgentRun, BrainError> {
        let now = Utc::now().to_rfc3339();
        let updated = self.db.with_connection(|conn| {
            Ok(conn.execute(
                "UPDATE agent_runs
                 SET status = 'failed', error = ?2, finished_at = ?3
                 WHERE id = ?1 AND status = 'running'",
                params![run_id, error, now],
            )?)
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
}

fn load_message_evidence(
    conn: &rusqlite::Connection,
    message_id: &str,
) -> Result<Vec<KnowledgeEntrySummary>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT ke.id, ke.knowledge_base_id, ke.entry_type, ke.slug, ke.title,
                ke.summary, ke.status, ke.confidence, sd.relative_path, ke.updated_at
         FROM knowledge_message_citations kmc
         JOIN knowledge_entries ke ON ke.id = kmc.entry_id
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

fn conversation_title(question: &str) -> String {
    let title = question.trim().chars().take(42).collect::<String>();
    if question.trim().chars().count() > 42 {
        format!("{title}…")
    } else {
        title
    }
}

fn parse_runtime_provider_config(
    config_json: &str,
) -> Result<Option<RuntimeProviderConfig>, BrainError> {
    let value: serde_json::Value = serde_json::from_str(config_json)
        .map_err(|error| BrainError::Internal(format!("运行时供应商配置解析失败: {error}")))?;
    let Some(provider) = value.get("provider").filter(|provider| !provider.is_null()) else {
        return Ok(None);
    };
    serde_json::from_value(provider.clone())
        .map(Some)
        .map_err(|error| BrainError::Internal(format!("运行时供应商配置解析失败: {error}")))
}

fn validate_runtime_provider_config(
    provider: &RuntimeProviderConfig,
    model: &str,
) -> Result<RuntimeProviderConfig, BrainError> {
    let provider_id = provider.provider_id.trim();
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
    let display_name = provider.display_name.trim();
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
    let api_protocol = provider.api_protocol.trim();
    if !matches!(
        api_protocol,
        "openai-completions" | "openai-responses" | "anthropic-messages"
    ) {
        return Err(BrainError::KnowledgeValidation(
            "不支持的模型 API 协议".to_string(),
        ));
    }
    let base_url = provider.base_url.trim().trim_end_matches('/');
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
    let api_key_env = provider.api_key_env.trim();
    let mut env_characters = api_key_env.chars();
    let valid_env = env_characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && env_characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
    if !valid_env || api_key_env.len() > 128 {
        return Err(BrainError::KnowledgeValidation(
            "API Key 环境变量名只能包含字母、数字和下划线，且不能以数字开头".to_string(),
        ));
    }

    Ok(RuntimeProviderConfig {
        provider_id: provider_id.to_string(),
        display_name: display_name.to_string(),
        api_protocol: api_protocol.to_string(),
        base_url: base_url.to_string(),
        api_key_env: api_key_env.to_string(),
    })
}

fn map_base_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeBaseSummary> {
    Ok(KnowledgeBaseSummary {
        id: row.get(0)?,
        book_id: row.get(1)?,
        book_name: row.get(2)?,
        book_path: row.get(3)?,
        book_kind: row.get(4)?,
        book_description: row.get(5)?,
        book_category: row.get(6)?,
        lifecycle: row.get(7)?,
        sync_state: row.get(8)?,
        health_state: row.get(9)?,
        last_error: row.get(10)?,
        last_synced_at: row.get(11)?,
        last_scanned_at: row.get(12)?,
        source_count: row.get(13)?,
        entry_count: row.get(14)?,
        claim_count: row.get(15)?,
        task_count: row.get(16)?,
    })
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

pub fn stable_id(prefix: &str, value: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.as_bytes());
    format!("{prefix}-{}", &hex::encode(digest)[..24])
}

fn knowledge_query_patterns(query: Option<&str>) -> Vec<String> {
    let Some(raw) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return vec!["%".to_string()];
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
        .map(|candidate| format!("%{candidate}%"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::book_wiki::{BookProgress, RuntimeProviderConfig};

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
    fn test_replace_markdown_sources_indexes_entries_and_citations() {
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

        store.replace_markdown_sources(&base.id, &[source]).unwrap();

        let entries = store
            .list_entries(&base.id, Some("正文"), None, 20)
            .unwrap();
        assert_eq!(entries.len(), 1);
        let detail = store.get_entry(&entries[0].id).unwrap();
        assert_eq!(detail.citations.len(), 1);
        assert_eq!(detail.citations[0].source_path, "intro.md");
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
        store.replace_markdown_sources(&base.id, &[source]).unwrap();
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
        store.replace_markdown_sources(&base.id, &[source]).unwrap();

        let entries = store
            .list_entries(&base.id, Some("找出与架构相关的章节"), None, 20)
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "核心架构");
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
    fn test_runtime_provider_config_round_trips_without_storing_secret() {
        let (store, _dir) = test_store();
        let profile = store
            .list_runtime_profiles()
            .unwrap()
            .into_iter()
            .find(|profile| profile.id == "runtime-deepseek-harness")
            .unwrap();
        let provider = RuntimeProviderConfig {
            provider_id: "aliyun-bailian".to_string(),
            display_name: "阿里云百炼".to_string(),
            api_protocol: "openai-completions".to_string(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            api_key_env: "CUSTOM_LLM_API_KEY".to_string(),
        };

        let saved = store
            .save_runtime_profile(
                &profile.id,
                &profile.executable,
                "glm-5.2",
                Some(&provider),
                true,
                profile.revision,
            )
            .unwrap();

        assert_eq!(saved.provider_config.as_ref(), Some(&provider));
        assert_eq!(saved.model, "glm-5.2");
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
        assert!(raw_config.contains("CUSTOM_LLM_API_KEY"));
        assert!(!raw_config.contains("sk-"));
    }

    #[test]
    fn test_runtime_provider_config_rejects_invalid_environment_variable() {
        let (store, _dir) = test_store();
        let profile = store.list_runtime_profiles().unwrap().remove(0);
        let provider = RuntimeProviderConfig {
            provider_id: "aliyun-bailian".to_string(),
            display_name: "阿里云百炼".to_string(),
            api_protocol: "openai-completions".to_string(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            api_key_env: "CUSTOM-LLM-KEY".to_string(),
        };

        let error = store
            .save_runtime_profile(
                &profile.id,
                &profile.executable,
                "glm-5.2",
                Some(&provider),
                true,
                profile.revision,
            )
            .unwrap_err();

        assert!(error.to_string().contains("环境变量"));
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
}
