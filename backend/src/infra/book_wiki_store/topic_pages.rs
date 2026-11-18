use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    insert_agent_run_event, knowledge_fts_cjk_terms, knowledge_fts_query, stable_id, BookWikiStore,
};
use crate::error::BrainError;
use crate::models::wiki_topics::{
    TopicCompileRequest, TopicDraftCitation, TopicExcerpt, TopicLink, TopicLinkedPage, TopicPage,
    TopicPageSummary, TopicQuestion, TopicRevision, TopicRevisionSummary, TopicSection,
    TopicSectionMatch, TopicSourceSection, TopicWritebackKind, TopicWritebackRequest,
};

pub(crate) struct TopicCompileClaim {
    pub page_id: String,
    pub attempt: String,
    pub input_hash: String,
    pub parent_id: Option<String>,
}

const PAGE_COLUMNS: &str = "p.id, p.title, p.published_id, p.draft_id,
    EXISTS(SELECT 1 FROM wiki_topic_stale_revisions stale WHERE stale.revision_id = p.published_id),
    COALESCE(julianday(p.attempt_deadline) > julianday('now'), 0),
    CASE WHEN p.running_attempt IS NOT NULL AND julianday(p.attempt_deadline) <= julianday('now')
         THEN '主题编译已超时，可以重试' ELSE p.last_error END,
    p.last_request_json";

struct TopicPageRow {
    summary: TopicPageSummary,
    published_id: Option<String>,
    draft_id: Option<String>,
    last_request: Option<String>,
}

impl BookWikiStore {
    pub fn list_topic_pages(&self, base_id: &str) -> Result<Vec<TopicPageSummary>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {PAGE_COLUMNS} FROM wiki_topic_pages p WHERE p.knowledge_base_id = ?1
                 ORDER BY p.title COLLATE NOCASE, p.id"
            ))?;
            let rows = stmt.query_map([base_id], |row| Ok(topic_page_row(row)?.summary))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn topic_page(&self, base_id: &str, page_id: &str) -> Result<TopicPage, BrainError> {
        self.find_topic_page(base_id, page_id)?
            .ok_or_else(|| BrainError::KnowledgeNotFound(page_id.into()))
    }

    pub(crate) fn find_topic_page(
        &self,
        base_id: &str,
        page_id: &str,
    ) -> Result<Option<TopicPage>, BrainError> {
        self.db.with_connection(|conn| {
            let Some(row) = conn.query_row(
                &format!("SELECT {PAGE_COLUMNS} FROM wiki_topic_pages p WHERE p.knowledge_base_id = ?1 AND p.id = ?2"),
                params![base_id, page_id],
                topic_page_row,
            ).optional()? else { return Ok(None); };
            let mut stmt = conn.prepare(
                "SELECT id, created_at, published_at FROM wiki_topic_revisions
                 WHERE page_id = ?1 ORDER BY created_at DESC, id LIMIT 25"
            )?;
            let revisions = stmt.query_map([page_id], |row| Ok(TopicRevisionSummary {
                id: row.get(0)?, created_at: row.get(1)?, published_at: row.get(2)?,
            }))?.collect::<Result<Vec<_>, _>>()?;
            let mut questions = conn.prepare(
                "SELECT id, question, created_at, resolved_at FROM wiki_topic_questions WHERE page_id = ?1
                 ORDER BY resolved_at IS NOT NULL, created_at DESC, id LIMIT 40"
            )?;
            let questions = questions.query_map([page_id], |row| Ok(TopicQuestion {
                id: row.get(0)?, question: row.get(1)?, created_at: row.get(2)?, resolved_at: row.get(3)?,
            }))?.collect::<Result<Vec<_>, _>>()?;
            Ok(Some(TopicPage {
                page: row.summary,
                published: row.published_id.map(|id| read_topic_revision(conn, base_id, page_id, &id)).transpose()?,
                draft: row.draft_id.map(|id| read_topic_revision(conn, base_id, page_id, &id)).transpose()?,
                last_request: row.last_request.map(|raw| serde_json::from_str(&raw)).transpose()
                    .map_err(|error| BrainError::Internal(format!("主题编译请求读取失败: {error}")))?,
                revisions,
                questions,
                backlinks: read_topic_backlinks(conn, base_id, page_id)?,
            }))
        })
    }

    pub fn topic_revision(
        &self,
        base_id: &str,
        page_id: &str,
        revision_id: &str,
    ) -> Result<TopicRevision, BrainError> {
        self.db
            .with_connection(|conn| read_topic_revision(conn, base_id, page_id, revision_id))
    }

    pub(crate) fn topic_revision_source_documents(
        &self,
        base_id: &str,
        revision_id: &str,
    ) -> Result<Vec<String>, BrainError> {
        self.db.with_connection(|conn| {
            let mut query = conn.prepare(
                "SELECT s.source_document_id FROM wiki_topic_revision_sources s
                 JOIN wiki_topic_revisions r ON r.id = s.revision_id
                 JOIN wiki_topic_pages p ON p.id = r.page_id
                 WHERE p.knowledge_base_id = ?1 AND s.revision_id = ?2",
            )?;
            let rows = query.query_map(params![base_id, revision_id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn list_topic_source_sections(
        &self,
        base_id: &str,
        document_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<TopicSourceSection>, BrainError> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id, s.heading, s.line_start, length(s.content)
                 FROM source_spans s JOIN source_documents d ON d.current_version_id = s.source_version_id
                 WHERE d.knowledge_base_id = ?1 AND d.id = ?2 AND d.sync_status = 'current'
                   AND s.knowledge_base_id = d.knowledge_base_id AND length(trim(s.content)) > 0
                 ORDER BY s.ordinal LIMIT ?3 OFFSET ?4",
            )?;
            let rows = stmt.query_map(params![base_id, document_id, limit.clamp(1, 100), offset], |row| {
                Ok(TopicSourceSection {
                    id: row.get(0)?, heading: row.get(1)?, line_start: row.get(2)?, total_chars: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub(crate) fn topic_excerpt(
        &self,
        base_id: &str,
        span_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Option<TopicExcerpt>, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT d.id, s.source_version_id, d.relative_path, s.heading, substr(s.content, ?3 + 1, ?4)
                 FROM source_spans s JOIN source_documents d ON d.current_version_id = s.source_version_id
                 WHERE s.id = ?2 AND d.knowledge_base_id = ?1 AND s.knowledge_base_id = ?1
                   AND d.sync_status = 'current' AND length(s.content) > ?3",
                params![base_id, span_id, offset, limit],
                |row| Ok(TopicExcerpt {
                    span_id: span_id.into(), document_id: row.get(0)?, version_id: row.get(1)?,
                    source_path: row.get(2)?, heading: row.get(3)?, offset, content: row.get(4)?,
                }),
            ).optional().map_err(Into::into)
        })
    }

    pub(crate) fn reader_selection_excerpt(
        &self,
        base_id: &str,
        source_path: &str,
        selection: &str,
    ) -> Result<Option<TopicExcerpt>, BrainError> {
        fn visible(character: char) -> bool {
            !character.is_whitespace() && !"*_`#[]()>~".contains(character)
        }
        let selected: String = selection.chars().filter(|&c| visible(c)).collect();
        if selected.is_empty() {
            return Ok(None);
        }
        let selected_chars = selected.chars().count();
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id, d.id, d.current_version_id, s.heading, s.content
                 FROM source_documents d JOIN source_spans s ON s.source_version_id = d.current_version_id
                 WHERE d.knowledge_base_id = ?1 AND d.relative_path = ?2 AND d.sync_status = 'current'
                   AND s.knowledge_base_id = d.knowledge_base_id
                 ORDER BY length(s.content), s.ordinal",
            )?;
            let mut rows = stmt.query(params![base_id, source_path])?;
            while let Some(row) = rows.next()? {
                let body: String = row.get(4)?;
                let indexed = body.chars().enumerate().filter(|&(_, c)| visible(c));
                let normalized: String = indexed.clone().map(|(_, c)| c).collect();
                if let Some(start) = normalized.find(&selected) {
                    let skip = normalized[..start].chars().count();
                    let mut positions = indexed.skip(skip);
                    if let (Some((offset, _)), Some((end, _))) =
                        (positions.clone().next(), positions.nth(selected_chars - 1))
                    {
                        return Ok(Some(TopicExcerpt {
                            span_id: row.get(0)?, document_id: row.get(1)?, version_id: row.get(2)?,
                            heading: row.get(3)?, source_path: source_path.to_string(), offset,
                            content: body.chars().skip(offset).take(end - offset + 1).collect(),
                        }));
                    }
                }
            }
            Ok(None)
        })
    }

    pub(crate) fn topic_evidence_is_current(
        &self,
        base_id: &str,
        revisions: &[String],
        excerpts: &[TopicExcerpt],
    ) -> Result<bool, BrainError> {
        self.db.with_connection(|conn| {
            let mut pages = conn.prepare(
                "SELECT EXISTS(SELECT 1 FROM wiki_topic_pages p
                 WHERE p.knowledge_base_id = ?1 AND p.published_id = ?2
                   AND NOT EXISTS(SELECT 1 FROM wiki_topic_stale_revisions WHERE revision_id = ?2))",
            )?;
            for revision in revisions {
                if !pages.query_row(params![base_id, revision], |row| row.get::<_, bool>(0))? {
                    return Ok(false);
                }
            }
            topic_source_versions_current(conn, base_id, excerpts)
        })
    }

    pub(crate) fn topic_revision_counts(
        &self,
        base_id: &str,
    ) -> Result<(usize, usize), BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*), COUNT(stale.revision_id) FROM wiki_topic_pages p
                 LEFT JOIN (SELECT DISTINCT revision_id FROM wiki_topic_stale_revisions) stale
                   ON stale.revision_id = p.published_id
                 WHERE p.knowledge_base_id = ?1 AND p.published_id IS NOT NULL",
                [base_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(Into::into)
        })
    }

    pub(crate) fn claim_topic_compile(
        &self,
        base_id: &str,
        request: &TopicCompileRequest,
        parent_id: Option<String>,
        input_hash: String,
        expected_draft: Option<Option<&str>>,
    ) -> Result<(String, Option<TopicCompileClaim>), BrainError> {
        let key = topic_key(&request.title);
        let page_id = request
            .page_id
            .clone()
            .unwrap_or_else(|| topic_id(base_id, &request.title));
        let raw = serde_json::to_string(request)
            .map_err(|error| BrainError::Internal(format!("主题编译请求保存失败: {error}")))?;
        let now = Utc::now();
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            if request.page_id.is_none() {
                conn.execute(
                    "INSERT INTO wiki_topic_pages(id, knowledge_base_id, topic_key, title, created_at)
                     VALUES(?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO NOTHING",
                    params![page_id, base_id, key, request.title, now.to_rfc3339()],
                )?;
            }
            let (stored_key, published_id, compiling, same_input, draft_id) = conn.query_row(
                "SELECT p.topic_key, p.published_id,
                        COALESCE(julianday(p.attempt_deadline) > julianday('now'), 0),
                        COALESCE(r.input_hash = ?3 AND NOT EXISTS (
                            SELECT 1 FROM wiki_topic_stale_revisions WHERE revision_id = r.id
                        ), 0), p.draft_id
                 FROM wiki_topic_pages p
                 LEFT JOIN wiki_topic_revisions r ON r.id = COALESCE(p.draft_id, p.published_id)
                 WHERE p.knowledge_base_id = ?1 AND p.id = ?2",
                params![base_id, page_id, input_hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, bool>(2)?, row.get::<_, bool>(3)?, row.get::<_, Option<String>>(4)?)),
            ).optional()?.ok_or_else(|| BrainError::KnowledgeNotFound(page_id.clone()))?;
            if stored_key != key || published_id != parent_id {
                return Err(BrainError::KnowledgeConflict("主题或已发布版本已变化，请刷新后重试".into()));
            }
            if expected_draft.is_some_and(|expected| expected != draft_id.as_deref()) {
                return Err(BrainError::KnowledgeConflict("主题草稿已变化，请先审核当前草稿".into()));
            }
            if compiling {
                return Err(BrainError::KnowledgeConflict("该主题正在编译".into()));
            }
            if same_input {
                return Ok((page_id, None));
            }
            let attempt = Uuid::new_v4().to_string();
            let deadline = (now + Duration::minutes(3)).to_rfc3339();
            conn.execute(
                "UPDATE wiki_topic_pages SET running_attempt = ?2, attempt_deadline = ?3,
                  last_request_json = ?4, last_error = NULL WHERE id = ?1",
                params![page_id, attempt, deadline, raw],
            )?;
            Ok((page_id.clone(), Some(TopicCompileClaim { page_id, attempt, input_hash, parent_id })))
        })
    }

    pub(crate) fn finish_topic_compile(
        &self,
        base_id: &str,
        claim: &TopicCompileClaim,
        run_id: &str,
        markdown: &str,
        excerpts: &[TopicExcerpt],
        links: &[TopicLink],
    ) -> Result<(), BrainError> {
        let revision_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let links_json = serde_json::to_string(links)
            .map_err(|error| BrainError::Internal(format!("主题链接保存失败: {error}")))?;
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            let claimed = conn.execute(
                "UPDATE wiki_topic_pages SET draft_id = ?4, running_attempt = NULL, attempt_deadline = NULL
                 WHERE id = ?1 AND knowledge_base_id = ?2 AND running_attempt = ?3
                   AND julianday(attempt_deadline) > julianday('now') AND published_id IS ?5",
                params![claim.page_id, base_id, claim.attempt, revision_id, claim.parent_id],
            )?;
            if claimed == 0 {
                return Err(BrainError::KnowledgeConflict("本次主题编译的版本或执行状态已变化".into()));
            }
            conn.execute(
                "INSERT INTO wiki_topic_revisions(id, page_id, parent_id, run_id, input_hash, markdown, created_at, links_json)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![revision_id, claim.page_id, claim.parent_id, run_id, claim.input_hash, markdown, now, links_json],
            )?;
            // Refuse a mixed revision even if one source was synced during context assembly.
            if !topic_source_versions_current(conn, base_id, excerpts)? {
                return Err(BrainError::KnowledgeConflict("编译期间原文已更新，请使用最新来源重试".into()));
            }
            for excerpt in excerpts {
                conn.execute(
                    "INSERT INTO wiki_topic_revision_sources(revision_id, source_document_id, source_version_id)
                     VALUES(?1, ?2, ?3) ON CONFLICT(revision_id, source_document_id) DO NOTHING",
                    params![revision_id, excerpt.document_id, excerpt.version_id],
                )?;
            }
            Ok(())
        })
    }

    pub(crate) fn fail_topic_compile(
        &self,
        claim: &TopicCompileClaim,
        error: &str,
    ) -> Result<(), BrainError> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE wiki_topic_pages SET running_attempt = NULL, attempt_deadline = NULL, last_error = ?3
                 WHERE id = ?1 AND running_attempt = ?2",
                params![claim.page_id, claim.attempt, error],
            )?;
            Ok(())
        })
    }

    pub fn save_topic_question(
        &self,
        base_id: &str,
        page_id: &str,
        run_id: &str,
    ) -> Result<TopicPage, BrainError> {
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            let question: String = conn.query_row(
                "SELECT trim(json_extract(r.input_json, '$.question'))
                 FROM agent_runs r JOIN wiki_topic_pages p ON p.knowledge_base_id = r.knowledge_base_id
                 WHERE r.id = ?1 AND p.id = ?2 AND p.knowledge_base_id = ?3 AND p.published_id IS NOT NULL
                   AND r.task_type = 'knowledge_qa' AND r.status = 'completed'
                   AND length(trim(json_extract(r.input_json, '$.question'))) BETWEEN 1 AND 2000",
                params![run_id, page_id, base_id], |row| row.get(0),
            ).optional()?.ok_or_else(|| BrainError::KnowledgeValidation("请选择本书已完成的问答和已发布的主题页".into()))?;
            conn.execute(
                "INSERT INTO wiki_topic_questions(id, page_id, run_id, question, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5) ON CONFLICT(page_id, question) DO NOTHING",
                params![Uuid::new_v4().to_string(), page_id, run_id, question, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })?;
        self.topic_page(base_id, page_id)
    }

    pub(crate) fn topic_writeback_origin(
        &self,
        base_id: &str,
        run_id: &str,
    ) -> Result<(TopicWritebackKind, String), BrainError> {
        self.db.with_connection(|conn| {
            ensure_active_base(conn, base_id)?;
            read_writeback_origin(conn, base_id, run_id)
        })
    }

    pub(crate) fn save_topic_writeback_draft(
        &self,
        base_id: &str,
        page_id: &str,
        request: &TopicWritebackRequest,
        markdown: &str,
        citations: &[TopicDraftCitation],
    ) -> Result<TopicPage, BrainError> {
        let now = Utc::now().to_rfc3339();
        let hash = stable_id(
            "topic-writeback",
            &json!([request.run_id, request.passages]).to_string(),
        );
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            let current = conn.query_row(
                &format!("SELECT {PAGE_COLUMNS} FROM wiki_topic_pages p WHERE p.id=?1 AND p.knowledge_base_id=?2"),
                params![page_id, base_id], topic_page_row,
            ).optional()?.ok_or_else(|| BrainError::KnowledgeNotFound(page_id.into()))?;
            let (kind, _) = read_writeback_origin(conn, base_id, &request.run_id)?;
            let known: Option<bool> = conn.query_row(
                "SELECT id IS COALESCE(?3, ?4) FROM wiki_topic_revisions
                 WHERE page_id=?1 AND input_hash=?2 ORDER BY (id IS COALESCE(?3, ?4)) DESC LIMIT 1",
                params![page_id, hash, current.draft_id, current.published_id], |row| row.get(0),
            ).optional()?;
            if let Some(known) = known {
                return if known {
                    Ok(())
                } else {
                    Err(BrainError::KnowledgeConflict("所选片段已整理过，可在历史版本中查看".into()))
                };
            }
            if current.published_id.as_deref() != Some(request.expected_revision_id.as_str()) {
                return Err(BrainError::KnowledgeConflict("已发布的主题版本已变化，请刷新后重试".into()));
            }
            if current.draft_id.is_some() || current.summary.compiling {
                return Err(BrainError::KnowledgeConflict("此主题已有编辑中的草稿，请先审核现有草稿".into()));
            }
            if current.summary.needs_review {
                return Err(BrainError::KnowledgeConflict("该主题的原文已更新，请先复核主题".into()));
            }
            let parent_run: String = conn.query_row(
                "SELECT run_id FROM wiki_topic_revisions WHERE id=?1",
                [&request.expected_revision_id], |row| row.get(0),
            )?;
            let ledger = draft_citation_snapshots(conn, base_id, &parent_run, &request.run_id, citations)?;
            let run = Uuid::new_v4().to_string();
            let revision = Uuid::new_v4().to_string();
            let input = json!({"source_run_id": request.run_id, "source_kind":kind, "parent_revision_id": request.expected_revision_id,
                "parent_run_id": parent_run, "topic_page_id": page_id, "passages": request.passages});
            let output = json!({"topic_revision_id": revision, "source_count": ledger.len()});
            conn.execute(
                "INSERT INTO agent_runs(id,knowledge_base_id,runtime,task_type,status,input_json,output_json,
                    started_at,finished_at,created_at) VALUES(?1,?2,'user','wiki_content_draft','completed',?3,?4,?5,?5,?5)",
                params![run, base_id, input.to_string(), output.to_string(), now],
            )?;
            conn.execute(
                "INSERT INTO wiki_topic_revisions(id,page_id,parent_id,run_id,input_hash,markdown,created_at,links_json)
                 SELECT ?1,page_id,id,?3,?4,?5,?6,links_json FROM wiki_topic_revisions WHERE id=?2",
                params![revision, request.expected_revision_id, run, hash, markdown, now],
            )?;
            conn.execute(
                "INSERT INTO wiki_topic_revision_sources(revision_id,source_document_id,source_version_id)
                 SELECT ?1,source_document_id,source_version_id FROM wiki_topic_revision_sources WHERE revision_id=?2",
                params![revision, request.expected_revision_id],
            )?;
            for (index, entry) in ledger {
                let raw = entry.snapshot.to_string();
                if raw.len() > 8 * 1024 * 1024 {
                    return Err(BrainError::KnowledgeValidation("整理后的引用快照超过存储上限".into()));
                }
                conn.execute(
                    "INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at)
                     VALUES(?1,?2,'source_span',?3,?4,?5,?6)",
                    params![run, index, entry.span_id, entry.version_id, raw, now],
                )?;
                conn.execute(
                    "INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at)
                     VALUES(?1,'source_span',?2,?3,?4,?5)",
                    params![run, entry.span_id, entry.version_id,
                        json!({"citation_index":index,"origin":"topic_content_draft",
                            "origins":entry.snapshot["topic_origins"]}).to_string(), now],
                )?;
                conn.execute(
                    "INSERT INTO wiki_topic_revision_sources(revision_id,source_document_id,source_version_id)
                     VALUES(?1,?2,?3) ON CONFLICT(revision_id,source_document_id) DO NOTHING",
                    params![revision, entry.document_id, entry.version_id],
                )?;
            }
            conn.execute(
                "UPDATE wiki_topic_pages SET draft_id=?2,running_attempt=NULL,attempt_deadline=NULL,last_error=NULL WHERE id=?1",
                params![page_id, revision],
            )?;
            insert_agent_run_event(conn, &run, "run.completed", Some("completed"), "已整理片段为主题草稿", &output.to_string(), &now)?;
            Ok(())
        })?;
        self.topic_page(base_id, page_id)
    }

    pub fn resolve_topic_question(
        &self,
        base_id: &str,
        page_id: &str,
        question_id: &str,
    ) -> Result<TopicPage, BrainError> {
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            let updated = conn.execute(
                "UPDATE wiki_topic_questions SET resolved_at=COALESCE(resolved_at, ?4)
                 WHERE id = ?3 AND page_id = ?2 AND page_id IN (
                   SELECT id FROM wiki_topic_pages WHERE knowledge_base_id = ?1
                 )",
                params![base_id, page_id, question_id, Utc::now().to_rfc3339()],
            )?;
            if updated == 0 {
                return Err(BrainError::KnowledgeNotFound(question_id.into()));
            }
            Ok(())
        })?;
        self.topic_page(base_id, page_id)
    }

    pub(crate) fn edit_topic_markdown(
        &self,
        base_id: &str,
        page_id: &str,
        draft_id: &str,
        markdown: &str,
        citations: &BTreeSet<usize>,
    ) -> Result<TopicPage, BrainError> {
        let citations = serde_json::to_string(citations)
            .map_err(|error| BrainError::Internal(format!("主题引用编号保存失败: {error}")))?;
        self.db.transaction(|conn| {
            ensure_active_base(conn, base_id)?;
            let id = Uuid::new_v4().to_string();
            let moved = conn.execute(
                "UPDATE wiki_topic_pages SET draft_id = ?4 WHERE id = ?2 AND knowledge_base_id = ?1
                   AND draft_id = ?3 AND COALESCE(julianday(attempt_deadline) <= julianday('now'), 1)",
                params![base_id, page_id, draft_id, id],
            )?;
            if moved == 0 { return Err(BrainError::KnowledgeConflict("草稿已变化或正在编译，请刷新后重试".into())); }
            conn.execute(
                "INSERT INTO wiki_topic_revisions(id, page_id, parent_id, run_id, input_hash, markdown, created_at, links_json)
                 SELECT ?2, page_id, id, run_id, input_hash, ?3, ?4, links_json FROM wiki_topic_revisions WHERE id = ?1",
                params![draft_id, id, markdown, Utc::now().to_rfc3339()],
            )?;
            conn.execute(
                "INSERT INTO wiki_topic_revision_sources(revision_id, source_document_id, source_version_id)
                 SELECT DISTINCT r.id, v.source_document_id, v.id FROM wiki_topic_revisions r
                 JOIN agent_run_citations c ON c.run_id = r.run_id AND c.kind = 'source_span'
                 JOIN source_versions v ON v.id = c.version_id
                 WHERE r.id = ?1 AND c.citation_index IN (SELECT value FROM json_each(?2))",
                params![id, citations],
            )?;
            Ok(())
        })?;
        self.topic_page(base_id, page_id)
    }

    pub(crate) fn publish_topic_sections(
        &self,
        base_id: &str,
        page_id: &str,
        revision_id: &str,
        sections: &[TopicSection],
    ) -> Result<TopicPage, BrainError> {
        self.db.transaction(|conn| {
            publish_topic_revision(conn, base_id, page_id, revision_id, sections)
        })?;
        self.topic_page(base_id, page_id)
    }

    pub(crate) fn search_topic_sections(
        &self,
        base_id: &str,
        query: &str,
        overview: bool,
    ) -> Result<Vec<TopicSectionMatch>, BrainError> {
        let fts = if overview {
            None
        } else {
            knowledge_fts_query(Some(query))
        };
        if !overview && fts.is_none() {
            return Ok(Vec::new());
        }
        let (constraint, order) = if overview {
            (
                "AND f.ordinal < 2",
                "f.ordinal, p.title COLLATE NOCASE, p.id",
            )
        } else {
            (
                "AND wiki_topic_sections_fts MATCH ?2",
                "bm25(wiki_topic_sections_fts, 0.0, 0.0, 9.0, 4.0, 1.0, 1.0), p.id, f.ordinal",
            )
        };
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT p.id, r.id, r.run_id, p.title, f.heading, f.markdown
                 FROM wiki_topic_sections_fts f JOIN wiki_topic_pages p ON p.id = f.page_id
                 JOIN wiki_topic_revisions r ON r.id = p.published_id
                 WHERE p.knowledge_base_id = ?1
                   AND NOT EXISTS(SELECT 1 FROM wiki_topic_stale_revisions stale WHERE stale.revision_id = r.id)
                   {constraint} ORDER BY {order} LIMIT 24"
            ))?;
            let arguments: &[&dyn rusqlite::ToSql] = match &fts {
                Some(query) => &[&base_id, query],
                None => &[&base_id],
            };
            let rows = stmt.query_map(arguments, |row| Ok(TopicSectionMatch {
                page_id: row.get(0)?, revision_id: row.get(1)?, run_id: row.get(2)?, title: row.get(3)?,
                section: TopicSection { heading: row.get(4)?, markdown: row.get(5)? },
            }))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }
}

fn read_writeback_origin(
    conn: &Connection,
    base_id: &str,
    run_id: &str,
) -> Result<(TopicWritebackKind, String), BrainError> {
    let question = conn
        .query_row(
            "SELECT trim(json_extract(input_json,'$.question'))
             FROM agent_runs WHERE id=?1 AND knowledge_base_id=?2
               AND task_type='knowledge_qa' AND status='completed'
               AND json_type(output_json,'$.answer')='text'
               AND length(trim(json_extract(input_json,'$.question')))>0",
            params![run_id, base_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(question) = question {
        return Ok((TopicWritebackKind::Answer, question));
    }
    let report = conn
        .query_row(
            "SELECT task.title FROM agent_runs r
             JOIN knowledge_research_stages stage
               ON stage.task_id=json_extract(r.input_json,'$.knowledge_task_id')
                AND stage.content_run_id=r.id AND stage.stage_key='report' AND stage.status='completed'
             JOIN knowledge_research_stages validation
               ON validation.task_id=stage.task_id AND validation.stage_key='validation'
                AND validation.status='completed' AND julianday(validation.updated_at)>=julianday(stage.updated_at)
             JOIN knowledge_tasks task ON task.id=stage.task_id AND task.knowledge_base_id=r.knowledge_base_id
             WHERE r.id=?1 AND r.knowledge_base_id=?2 AND r.status='completed'
               AND r.task_type='knowledge_task_report' AND json_extract(r.input_json,'$.research_stage_key')='report'
               AND stage.content_md=json_extract(r.output_json,'$.answer')",
            params![run_id, base_id],
            |row| row.get(0),
        )
        .optional()?;
    report
        .map(|title| (TopicWritebackKind::Report, title))
        .ok_or_else(|| {
            BrainError::KnowledgeValidation("请选择本书已完成的问答或通过校验的当前研究报告".into())
        })
}

struct DraftCitationSnapshot {
    span_id: String,
    version_id: String,
    document_id: String,
    snapshot: Value,
}

fn draft_citation_snapshots(
    conn: &Connection,
    base_id: &str,
    parent_run: &str,
    source_run: &str,
    citations: &[TopicDraftCitation],
) -> Result<BTreeMap<usize, DraftCitationSnapshot>, BrainError> {
    let mut ledger = BTreeMap::<usize, DraftCitationSnapshot>::new();
    let mut source = conn.prepare(
        "SELECT c.object_id,c.version_id,d.id,c.snapshot_json
         FROM agent_run_citations c JOIN agent_runs a ON a.id=c.run_id
         JOIN source_spans s ON s.id=c.object_id AND s.source_version_id=c.version_id AND s.knowledge_base_id=?3
         JOIN source_versions v ON v.id=s.source_version_id
         JOIN source_documents d ON d.id=v.source_document_id AND d.current_version_id=v.id
         WHERE c.run_id=?1 AND c.citation_index=?2 AND c.kind='source_span'
           AND a.knowledge_base_id=?3 AND a.status='completed'
           AND d.knowledge_base_id=?3 AND d.sync_status='current'",
    )?;
    for citation in citations {
        if citation.run_id != parent_run && citation.run_id != source_run {
            return Err(BrainError::Internal("整理草稿引用的运行身份无效".into()));
        }
        let (span_id, version_id, document_id, raw) = source
            .query_row(
                params![citation.run_id, citation.origin_index, base_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                BrainError::KnowledgeConflict(
                    "所选片段引用的原文已变化，请更新问答或研究结果后核对出处".into(),
                )
            })?;
        let mut snapshot: Value = serde_json::from_str(&raw)
            .map_err(|error| BrainError::Internal(format!("引用快照读取失败: {error}")))?;
        let segments = snapshot["segments"]
            .as_array()
            .filter(|segments| !segments.is_empty())
            .ok_or_else(|| BrainError::KnowledgeValidation("引用快照缺少实际原文范围".into()))?;
        let origin = json!({"run_id":citation.run_id, "citation_index":citation.origin_index});
        if let Some(existing) = ledger.get_mut(&citation.index) {
            if existing.span_id != span_id || existing.version_id != version_id {
                return Err(BrainError::Internal("整理草稿中的原文编号冲突".into()));
            }
            if let Some(current) = existing.snapshot["segments"].as_array_mut() {
                for segment in segments {
                    if !current.contains(segment) {
                        current.push(segment.clone());
                    }
                }
                current.sort_by_key(|segment| segment["offset_chars"].as_u64());
            }
            if let Some(origins) = existing.snapshot["topic_origins"].as_array_mut() {
                origins.push(origin);
            }
        } else {
            snapshot["topic_origins"] = json!([origin]);
            ledger.insert(
                citation.index,
                DraftCitationSnapshot {
                    span_id,
                    version_id,
                    document_id,
                    snapshot,
                },
            );
        }
    }
    Ok(ledger)
}

pub(super) fn publish_topic_revision(
    conn: &Connection,
    base_id: &str,
    page_id: &str,
    revision_id: &str,
    sections: &[TopicSection],
) -> Result<bool, BrainError> {
    ensure_active_base(conn, base_id)?;
    let (title, published_id, draft_id, compiling) = conn.query_row(
        "SELECT title, published_id, draft_id, COALESCE(julianday(attempt_deadline) > julianday('now'), 0)
         FROM wiki_topic_pages WHERE id = ?1 AND knowledge_base_id = ?2",
        params![page_id, base_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, bool>(3)?)),
    ).optional()?.ok_or_else(|| BrainError::KnowledgeNotFound(page_id.into()))?;
    if published_id.as_deref() == Some(revision_id) {
        return Ok(false);
    }
    if compiling || draft_id.as_deref() != Some(revision_id) {
        return Err(BrainError::KnowledgeConflict(
            "草稿已变化，请刷新主题页后再发布".into(),
        ));
    }
    let stale: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM wiki_topic_stale_revisions WHERE revision_id = ?1)",
        [revision_id],
        |row| row.get(0),
    )?;
    if stale {
        return Err(BrainError::KnowledgeConflict(
            "草稿引用的原文已更新，请重新编译该主题".into(),
        ));
    }
    conn.execute(
        "UPDATE wiki_topic_revisions SET published_at = ?2 WHERE id = ?1",
        params![revision_id, Utc::now().to_rfc3339()],
    )?;
    conn.execute("UPDATE wiki_topic_pages SET published_id = ?2, draft_id = NULL, last_error = NULL WHERE id = ?1", params![page_id, revision_id])?;
    conn.execute(
        "DELETE FROM wiki_topic_sections_fts WHERE page_id = ?1",
        [page_id],
    )?;
    let mut index = conn.prepare("INSERT INTO wiki_topic_sections_fts(page_id, ordinal, title, heading, markdown, cjk_terms) VALUES(?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (ordinal, section) in sections.iter().enumerate() {
        let terms = knowledge_fts_cjk_terms(&[&title, &section.heading, &section.markdown]);
        index.execute(params![
            page_id,
            ordinal,
            title,
            section.heading,
            section.markdown,
            terms
        ])?;
    }
    Ok(true)
}

fn topic_page_row(row: &Row<'_>) -> rusqlite::Result<TopicPageRow> {
    let published_id: Option<String> = row.get(2)?;
    let draft_id: Option<String> = row.get(3)?;
    Ok(TopicPageRow {
        summary: TopicPageSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            has_publication: published_id.is_some(),
            has_draft: draft_id.is_some(),
            draft_revision_id: draft_id.clone(),
            needs_review: row.get(4)?,
            compiling: row.get(5)?,
            last_error: row.get(6)?,
        },
        published_id,
        draft_id,
        last_request: row.get(7)?,
    })
}

fn read_topic_revision(
    conn: &Connection,
    base_id: &str,
    page_id: &str,
    id: &str,
) -> Result<TopicRevision, BrainError> {
    let mut revision = conn
        .query_row(
            "SELECT r.id, r.markdown, r.run_id, r.created_at,
                (SELECT COUNT(*) FROM agent_run_citations c WHERE c.run_id = r.run_id),
                r.parent_id
         FROM wiki_topic_revisions r JOIN wiki_topic_pages p ON p.id = r.page_id
         WHERE p.knowledge_base_id = ?1 AND r.page_id = ?2 AND r.id = ?3",
            params![base_id, page_id, id],
            |row| {
                Ok(TopicRevision {
                    id: row.get(0)?,
                    markdown: row.get(1)?,
                    run_id: row.get(2)?,
                    created_at: row.get(3)?,
                    evidence_count: row.get(4)?,
                    parent_id: row.get(5)?,
                    links: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| BrainError::KnowledgeNotFound(id.into()))?;
    revision.links = read_topic_links(conn, base_id, id)?;
    Ok(revision)
}

pub(crate) fn read_topic_links(
    conn: &Connection,
    base_id: &str,
    revision_id: &str,
) -> Result<Vec<TopicLinkedPage>, BrainError> {
    let mut query = conn.prepare(
        "SELECT json_extract(rel.value, '$.page_id'),
                COALESCE(target.title, json_extract(rel.value, '$.title')),
                json_extract(rel.value, '$.reason'), target.published_id IS NOT NULL
         FROM wiki_topic_revisions AS revision
         JOIN wiki_topic_pages AS page ON page.id=revision.page_id AND page.knowledge_base_id=?1
         CROSS JOIN json_each(revision.links_json) AS rel
         LEFT JOIN wiki_topic_pages AS target ON target.id=json_extract(rel.value, '$.page_id')
              AND target.knowledge_base_id=page.knowledge_base_id
         WHERE revision.id=?2 ORDER BY rel.key",
    )?;
    let rows = query.query_map(params![base_id, revision_id], |row| {
        Ok(TopicLinkedPage {
            link: TopicLink {
                page_id: row.get(0)?,
                title: row.get(1)?,
                reason: row.get(2)?,
            },
            published: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn read_topic_backlinks(
    conn: &Connection,
    base_id: &str,
    page_id: &str,
) -> Result<Vec<TopicLink>, BrainError> {
    let mut query = conn.prepare(
        "SELECT page.id, page.title, json_extract(rel.value, '$.reason')
         FROM wiki_topic_pages AS page
         JOIN wiki_topic_revisions AS revision ON revision.id=page.published_id
         CROSS JOIN json_each(revision.links_json) AS rel
         WHERE page.knowledge_base_id=?1 AND json_extract(rel.value, '$.page_id')=?2
         ORDER BY page.title, page.id",
    )?;
    let rows = query.query_map(params![base_id, page_id], |row| {
        Ok(TopicLink {
            page_id: row.get(0)?,
            title: row.get(1)?,
            reason: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn ensure_active_base(conn: &Connection, base_id: &str) -> Result<(), BrainError> {
    let active: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_bases WHERE id = ?1 AND lifecycle = 'active')",
        [base_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(BrainError::KnowledgeConflict(
            "知识库已暂停、归档或删除".into(),
        ));
    }
    Ok(())
}

fn topic_source_versions_current(
    conn: &Connection,
    base_id: &str,
    excerpts: &[TopicExcerpt],
) -> Result<bool, BrainError> {
    let mut checked = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT EXISTS(SELECT 1 FROM source_documents WHERE id = ?1 AND knowledge_base_id = ?2
           AND current_version_id = ?3 AND sync_status = 'current')",
    )?;
    for excerpt in excerpts {
        if let Some(previous) = checked.insert(&excerpt.document_id, &excerpt.version_id) {
            if previous != &excerpt.version_id {
                return Ok(false);
            }
        } else if !stmt.query_row(
            params![excerpt.document_id, base_id, excerpt.version_id],
            |row| row.get::<_, bool>(0),
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn topic_id(base_id: &str, title: &str) -> String {
    stable_id("topic", &format!("{base_id}:{}", topic_key(title)))
}

fn topic_key(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
