//! Evidence expiry is a review requirement, not a claim that a fact is false.

use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::error::BrainError;
use crate::models::book_wiki::KnowledgeSourceImpact;

impl super::BookWikiStore {
    /// Explicit review only; analysis checkpoints and historical evidence remain
    /// untouched. Select current contributors of affected topics/dependency roots.
    pub(crate) fn list_source_review_spans(
        &self,
        base: &str,
    ) -> Result<Vec<crate::models::book_wiki::SourceSpanSnapshot>, BrainError> {
        self.get_active_base(base)?;
        let spans = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT ss.id,sd.id,ss.source_version_id,sd.relative_path,ss.heading,
                ss.line_start,ss.line_end,ss.content,
                COALESCE((SELECT locator_json FROM source_span_structures WHERE source_span_id=ss.id),'{}')
                FROM source_spans ss JOIN source_versions sv ON sv.id=ss.source_version_id
                JOIN source_documents sd ON sd.id=sv.source_document_id
                WHERE ss.knowledge_base_id=?1 AND sd.knowledge_base_id=?1
                AND sd.sync_status='current' AND sd.current_version_id=ss.source_version_id
                AND EXISTS(SELECT 1 FROM knowledge_source_impacts impact
                    JOIN knowledge_entries affected ON affected.id=impact.entry_id
                    JOIN knowledge_citations citation ON citation.knowledge_base_id=impact.knowledge_base_id
                    LEFT JOIN knowledge_claims claim ON claim.id=citation.claim_id
                    JOIN source_spans old_span ON old_span.id=citation.source_span_id
                    JOIN source_versions old_version ON old_version.id=old_span.source_version_id
                    WHERE impact.knowledge_base_id=?1 AND impact.resolved_at IS NULL
                    AND affected.knowledge_base_id=?1 AND affected.status<>'archived'
                    AND old_span.knowledge_base_id=?1
                    AND COALESCE(citation.entry_id,claim.entry_id) IN (impact.entry_id,impact.root_entry_id)
                    AND old_version.source_document_id=sd.id)
                ORDER BY sd.ordinal,ss.ordinal")?;
            let rows = stmt.query_map([base], |row| Ok(crate::models::book_wiki::SourceSpanSnapshot {
                id:row.get(0)?,source_document_id:row.get(1)?,source_version_id:row.get(2)?,
                source_path:row.get(3)?,heading:row.get(4)?,line_start:row.get(5)?,line_end:row.get(6)?,
                content:row.get(7)?,locator:super::source_locator_from_row(row,8)?,
            }))?;
            Ok(rows.collect::<Result<Vec<_>,_>>()?)
        })?;
        if spans.is_empty() {
            return Err(BrainError::KnowledgeValidation("没有可重新分析的当前依据：请先同步或补回缺失来源；不再需要的历史主题可提交归档审核".into()));
        }
        Ok(spans)
    }
}

pub(crate) fn backfill_source_impacts(conn: &Connection) -> Result<(), BrainError> {
    let bases = {
        let mut stmt = conn.prepare("SELECT id FROM knowledge_bases")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let now = Utc::now().to_rfc3339();
    for base in bases {
        refresh_source_impacts(conn, &base, &now)?;
        super::refresh_review_state(conn, &base, &now)?;
    }
    Ok(())
}

pub(super) fn current_span_ids(
    conn: &Connection,
    base: &str,
) -> Result<HashSet<String>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT ss.id FROM source_spans ss JOIN source_documents sd
        ON sd.current_version_id=ss.source_version_id WHERE ss.knowledge_base_id=?1
        AND sd.knowledge_base_id=?1 AND sd.sync_status='current'",
    )?;
    let rows = stmt.query_map([base], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<HashSet<_>, _>>()?)
}

pub(super) fn refresh_source_impacts(
    conn: &Connection,
    base: &str,
    now: &str,
) -> Result<(), BrainError> {
    // UNION (not UNION ALL) makes dependency cycles finite. Only explicit
    // dependency edges propagate; a comparison/support link is not a dependency.
    conn.execute("WITH RECURSIVE affected(entry_id,source_document_id,previous_version_id,current_version_id,root_entry_id,reason) AS (
        SELECT DISTINCT ke.id,sd.id,sv.id,CASE WHEN sd.sync_status='current' THEN sd.current_version_id END,ke.id,
            CASE WHEN sd.sync_status<>'current' THEN 'source_missing'
                 WHEN sv.content_hash=current_version.content_hash THEN 'source_reindexed' ELSE 'source_changed' END
        FROM knowledge_citations kc
        LEFT JOIN knowledge_claims claim ON claim.id=kc.claim_id
        JOIN knowledge_entries ke ON ke.id=COALESCE(kc.entry_id,claim.entry_id)
        JOIN source_spans ss ON ss.id=kc.source_span_id
        JOIN source_versions sv ON sv.id=ss.source_version_id
        JOIN source_documents sd ON sd.id=sv.source_document_id
        LEFT JOIN source_versions current_version ON current_version.id=sd.current_version_id
        WHERE ke.knowledge_base_id=?1 AND ss.knowledge_base_id=?1 AND sd.knowledge_base_id=?1
            AND ke.entry_type<>'source_section' AND ke.status<>'archived'
            AND (sd.sync_status<>'current' OR sd.current_version_id IS NULL OR sd.current_version_id<>sv.id)
        UNION
        SELECT i.entry_id,i.source_document_id,i.previous_version_id,
            CASE WHEN sd.sync_status='current' THEN sd.current_version_id END,i.root_entry_id,
            CASE WHEN sd.sync_status<>'current' THEN 'source_missing'
                 WHEN sv.content_hash=cv.content_hash THEN 'source_reindexed' ELSE 'source_changed' END
        FROM knowledge_source_impacts i JOIN knowledge_entries ke ON ke.id=i.entry_id
        JOIN source_documents sd ON sd.id=i.source_document_id
        JOIN source_versions sv ON sv.id=i.previous_version_id
        LEFT JOIN source_versions cv ON cv.id=sd.current_version_id
        WHERE i.knowledge_base_id=?1 AND i.resolved_at IS NULL AND ke.status<>'archived'
        UNION
        SELECT kr.from_entry_id,a.source_document_id,a.previous_version_id,a.current_version_id,a.root_entry_id,a.reason
        FROM affected a JOIN knowledge_relations kr ON kr.to_entry_id=a.entry_id
        JOIN knowledge_entries ke ON ke.id=kr.from_entry_id
        WHERE kr.knowledge_base_id=?1 AND ke.knowledge_base_id=?1 AND ke.status<>'archived'
            AND ke.entry_type<>'source_section' AND kr.relation_type='依赖'
    ) INSERT INTO knowledge_source_impacts
        (knowledge_base_id,entry_id,source_document_id,previous_version_id,current_version_id,root_entry_id,reason,detected_at)
        SELECT ?1,entry_id,source_document_id,previous_version_id,current_version_id,root_entry_id,reason,?2 FROM affected WHERE 1
        ON CONFLICT(entry_id,source_document_id,previous_version_id,root_entry_id) DO UPDATE SET
            current_version_id=excluded.current_version_id,reason=excluded.reason,resolved_at=NULL", params![base,now])?;
    // Keep the before and after status revisions without changing any content,
    // aliases, manual edit policy, citations, or frozen run snapshots.
    conn.execute("INSERT OR IGNORE INTO knowledge_entry_versions
        (id,entry_id,revision,title,summary,content_md,aliases_json,status,confidence,changed_by_run_id,created_at)
        SELECT 'entry-version-'||id||'-'||revision,id,revision,title,summary,content_md,aliases_json,status,confidence,updated_by_run_id,updated_at
        FROM knowledge_entries WHERE knowledge_base_id=?1 AND status NOT IN ('stale','archived')
        AND EXISTS(SELECT 1 FROM knowledge_source_impacts i WHERE i.entry_id=knowledge_entries.id AND i.resolved_at IS NULL)", [base])?;
    conn.execute("UPDATE knowledge_entries SET status='stale',revision=revision+1,updated_at=?2
        WHERE knowledge_base_id=?1 AND status NOT IN ('stale','archived')
        AND EXISTS(SELECT 1 FROM knowledge_source_impacts i WHERE i.entry_id=knowledge_entries.id AND i.resolved_at IS NULL)", params![base,now])?;
    conn.execute("INSERT OR IGNORE INTO knowledge_entry_versions
        (id,entry_id,revision,title,summary,content_md,aliases_json,status,confidence,changed_by_run_id,created_at)
        SELECT 'entry-version-'||id||'-'||revision,id,revision,title,summary,content_md,aliases_json,status,confidence,updated_by_run_id,updated_at
        FROM knowledge_entries WHERE knowledge_base_id=?1 AND status='stale'
        AND EXISTS(SELECT 1 FROM knowledge_source_impacts i WHERE i.entry_id=knowledge_entries.id AND i.resolved_at IS NULL)", [base])?;
    conn.execute("UPDATE knowledge_claims SET verification_status='stale',revision=revision+1,updated_at=?2
        WHERE knowledge_base_id=?1 AND verification_status NOT IN ('stale','disputed','rejected')
        AND EXISTS(SELECT 1 FROM knowledge_citations kc JOIN source_spans ss ON ss.id=kc.source_span_id
            JOIN source_documents sd ON sd.id=(SELECT source_document_id FROM source_versions WHERE id=ss.source_version_id)
            WHERE kc.claim_id=knowledge_claims.id AND (sd.sync_status<>'current' OR sd.current_version_id IS NULL OR sd.current_version_id<>ss.source_version_id))",params![base,now])?;
    Ok(())
}

pub(super) fn resolve_reviewed_entry_impacts(
    conn: &Connection,
    entry: &str,
    now: &str,
) -> Result<(), BrainError> {
    conn.execute("UPDATE knowledge_source_impacts SET resolved_at=?2 WHERE entry_id=?1 AND resolved_at IS NULL
        AND EXISTS(SELECT 1 FROM knowledge_entries ke WHERE ke.id=?1 AND ke.status<>'stale')
        AND NOT EXISTS(SELECT 1 FROM knowledge_citations kc LEFT JOIN knowledge_claims claim ON claim.id=kc.claim_id
            JOIN source_spans ss ON ss.id=kc.source_span_id JOIN source_versions sv ON sv.id=ss.source_version_id
            JOIN source_documents sd ON sd.id=sv.source_document_id
            WHERE COALESCE(kc.entry_id,claim.entry_id)=?1 AND (sd.sync_status<>'current' OR sd.current_version_id IS NULL OR sd.current_version_id<>sv.id))
        AND NOT EXISTS(SELECT 1 FROM knowledge_relations kr JOIN knowledge_entries target ON target.id=kr.to_entry_id
            WHERE kr.from_entry_id=?1 AND kr.relation_type='依赖' AND target.status IN ('stale','archived'))",params![entry,now])?;
    Ok(())
}

pub(super) fn affected_entries(
    conn: &Connection,
    base: &str,
) -> Result<(i64, Vec<String>), BrainError> {
    let count = conn.query_row("SELECT COUNT(DISTINCT i.entry_id) FROM knowledge_source_impacts i
        JOIN knowledge_entries ke ON ke.id=i.entry_id WHERE i.knowledge_base_id=?1 AND i.resolved_at IS NULL AND ke.status<>'archived'",[base],|row|row.get(0))?;
    let mut stmt = conn.prepare("SELECT DISTINCT i.entry_id FROM knowledge_source_impacts i
        JOIN knowledge_entries ke ON ke.id=i.entry_id WHERE i.knowledge_base_id=?1 AND i.resolved_at IS NULL AND ke.status<>'archived' ORDER BY i.entry_id LIMIT 50")?;
    let rows = stmt.query_map([base], |row| row.get::<_, String>(0))?;
    Ok((count, rows.collect::<Result<Vec<_>, _>>()?))
}

pub(super) fn entry_impact_count(conn: &Connection, entry: &str) -> Result<i64, BrainError> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM knowledge_source_impacts WHERE entry_id=?1 AND resolved_at IS NULL",
        [entry],
        |row| row.get(0),
    )?)
}

pub(super) fn entry_impacts(
    conn: &Connection,
    entry: &str,
) -> Result<Vec<KnowledgeSourceImpact>, BrainError> {
    let mut stmt = conn.prepare("SELECT i.source_document_id,sd.relative_path,i.previous_version_id,i.current_version_id,i.reason,
        CASE WHEN i.root_entry_id<>i.entry_id THEN i.root_entry_id END,
        CASE WHEN i.root_entry_id<>i.entry_id THEN substr(root.title,1,240) END,i.detected_at
        FROM knowledge_source_impacts i JOIN source_documents sd ON sd.id=i.source_document_id
        JOIN knowledge_entries root ON root.id=i.root_entry_id
        WHERE i.entry_id=?1 AND i.resolved_at IS NULL ORDER BY i.source_document_id,i.root_entry_id LIMIT 20")?;
    let rows = stmt.query_map([entry], |row| {
        Ok(KnowledgeSourceImpact {
            source_document_id: row.get(0)?,
            source_path: row.get(1)?,
            previous_version_id: row.get(2)?,
            current_version_id: row.get(3)?,
            reason: row.get(4)?,
            affected_via_entry_id: row.get(5)?,
            affected_via_entry_title: row.get(6)?,
            detected_at: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{sample_book, sample_source, semantic_candidate, test_store};
    use super::super::{stable_id, BookWikiStore, MarkdownSourceDraft};
    use rusqlite::params;

    fn approve(store: &BookWikiStore, base: &str, slug: &str, span: &str) -> String {
        let run = store
            .start_agent_run(
                base,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let mut candidate = semantic_candidate(span, slug);
        candidate["slug"] = serde_json::json!(slug);
        candidate["status"] = serde_json::json!("verified");
        let set = store
            .create_semantic_change_set(base, &run.id, slug, "测试", &run.id, &[candidate])
            .unwrap();
        store.resolve_change_set(&set.id, true, "").unwrap();
        stable_id("entry", &format!("{base}:concept:{slug}"))
    }

    fn updated(mut source: MarkdownSourceDraft) -> MarkdownSourceDraft {
        source.version_id.push_str("-new");
        source.content_hash.push_str("-new");
        source.sections[0].id.push_str("-new");
        source.sections[0].content_hash.push_str("-new");
        source.sections[0].content_md = "更新后的条件及原文。".into();
        source
    }

    #[test]
    fn test_incremental_compile_revisits_all_contributing_current_sources_only() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[
                sample_book("revisit", "/tmp/revisit"),
                sample_book("revisit-foreign", "/tmp/revisit-foreign"),
            ])
            .unwrap();
        let base = store.initialize_base("revisit").unwrap();
        let foreign = store.initialize_base("revisit-foreign").unwrap();
        let changing = sample_source("changing", "changing", "changing-span");
        let stable = sample_source("stable", "stable", "stable-span");
        let unrelated = sample_source("unrelated", "unrelated", "unrelated-span");
        store
            .sync_markdown_sources(
                &base.id,
                &[changing.clone(), stable.clone(), unrelated.clone()],
            )
            .unwrap();
        store
            .sync_markdown_sources(
                &foreign.id,
                &[sample_source("foreign", "foreign", "foreign-span")],
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
        let mut candidate = semantic_candidate("changing-span", "多章主题");
        candidate["citations"] = serde_json::json!(["changing-span", "stable-span"]);
        candidate["claims"][0]["citations"] = serde_json::json!(["changing-span", "stable-span"]);
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "首次",
                "",
                "revisit-initial",
                &[candidate],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        store
            .record_compile_checkpoints(
                &base.id,
                &store.list_current_source_spans(&base.id).unwrap(),
                &initial.id,
                "revisit-contract",
            )
            .unwrap();
        let next = [updated(changing), stable, unrelated];
        store.sync_markdown_sources(&base.id, &next).unwrap();
        let spans = store
            .list_source_spans_pending_compile(&base.id, "revisit-contract")
            .unwrap();
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().any(|span| span.id == "changing-span-new"));
        assert!(spans.iter().any(|span| span.id == "stable-span"));
        assert!(!spans.iter().any(|span| span.id == "changing-span"
            || span.id == "unrelated-span"
            || span.id == "foreign-span"));
        let no_material = store
            .create_no_material_change_set(
                &base.id,
                &run.id,
                "无新增",
                "检查过",
                "revisit-analysis",
            )
            .unwrap();
        store
            .record_compile_checkpoints(&base.id, &spans, &no_material.id, "revisit-contract")
            .unwrap();
        // Analysis is recorded separately from approval: don't endlessly redo
        // the same input, but also don't pretend expired knowledge is healthy.
        assert!(store
            .list_source_spans_pending_compile(&base.id, "revisit-contract")
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .get_entry(&initial.changes[0].object_id)
                .unwrap()
                .entry
                .status,
            "stale"
        );
        assert_eq!(store.get_base(&base.id).unwrap().health_state, "warning");
        // A user may explicitly ask to re-analyse those current contributors
        // without erasing checkpoints or falsely resolving the stale topic.
        let retry = store.list_source_review_spans(&base.id).unwrap();
        assert_eq!(retry.len(), 2);
        assert!(retry.iter().any(|span| span.id == "changing-span-new"));
        assert!(retry.iter().any(|span| span.id == "stable-span"));
        assert!(store.list_source_review_spans(&foreign.id).is_err());
        assert!(store
            .list_source_spans_pending_compile(&base.id, "revisit-contract")
            .unwrap()
            .is_empty());
        let mut newest = next;
        newest[0] = updated(newest[0].clone());
        store.sync_markdown_sources(&base.id, &newest).unwrap();
        assert_eq!(
            store
                .list_source_spans_pending_compile(&base.id, "revisit-contract")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn test_source_change_marks_direct_and_dependency_cycle_stale_without_rewriting_body() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("impact", "/tmp/impact")])
            .unwrap();
        let base = store.initialize_base("impact").unwrap();
        let first = sample_source("impact-a", "chapter-a", "span-a");
        let second = sample_source("impact-b", "chapter-b", "span-b");
        store
            .sync_markdown_sources(&base.id, &[first.clone(), second.clone()])
            .unwrap();
        let direct = approve(&store, &base.id, "direct", "span-a");
        let dependent = approve(&store, &base.id, "dependent", "span-b");
        let unrelated = approve(&store, &base.id, "unrelated", "span-b");
        store.db.transaction(|conn| {
            conn.execute("UPDATE knowledge_entries SET edit_policy='human_protected' WHERE id=?1",[&direct])?;
            for (id, from, to, kind) in [("dep", &dependent, &direct, "依赖"),("cycle", &direct, &dependent, "依赖"),("comparison", &unrelated, &direct, "对比")] {
                conn.execute("INSERT INTO knowledge_relations(id,knowledge_base_id,from_entry_id,to_entry_id,relation_type,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",params![id,base.id,from,to,kind])?;
            }
            Ok(())
        }).unwrap();
        let original = store.get_entry(&direct).unwrap();
        let history_run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();
        store
            .record_visible_agent_evidence(
                &history_run.id,
                "entry",
                &direct,
                &original.revision.to_string(),
                &serde_json::json!({"content_md":original.content_md}),
            )
            .unwrap();
        let sources = [updated(first), second];
        store.sync_markdown_sources(&base.id, &sources).unwrap();
        let stale = store.get_entry(&direct).unwrap();
        assert_eq!(stale.entry.status, "stale");
        assert_eq!(stale.content_md, original.content_md);
        assert_eq!(stale.edit_policy, "human_protected");
        assert_eq!(stale.source_impact_count, 1);
        assert_eq!(stale.source_impacts[0].reason, "source_changed");
        assert_eq!(stale.revision, original.revision + 1);
        assert_eq!(stale.claims[0].verification_status, "stale");
        assert_eq!(store.get_entry(&dependent).unwrap().entry.status, "stale");
        assert_eq!(
            store.get_entry(&unrelated).unwrap().entry.status,
            "verified"
        );
        assert_eq!(store.list_qa_catalog(&base.id).unwrap().len(), 1);
        let workspace = store
            .list_entries_page_with_stale(&base.id, None, Some("concept"), 0, 20, true)
            .unwrap();
        assert_eq!(workspace.total, 3);
        assert_eq!(workspace.entries.len(), 3);
        let search = store
            .list_entries_page_with_stale(&base.id, Some("direct"), Some("concept"), 0, 20, true)
            .unwrap();
        assert_eq!(search.entries.len(), 1);
        assert_eq!(search.entries[0].status, "stale");
        assert_eq!(
            store
                .list_entries_page(&base.id, None, Some("concept"), 0, 20)
                .unwrap()
                .total,
            1
        );
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();
        assert!(store
            .record_visible_agent_evidence(
                &run.id,
                "entry",
                &direct,
                &stale.revision.to_string(),
                &serde_json::json!({"content_md":stale.content_md})
            )
            .is_err());
        assert_eq!(store.get_base(&base.id).unwrap().health_state, "warning");
        assert!(store
            .lint_knowledge_base(&base.id)
            .unwrap()
            .issues
            .iter()
            .any(|issue| issue.code == "expired-source-evidence"));
        store.sync_markdown_sources(&base.id, &sources).unwrap();
        assert_eq!(store.get_entry(&direct).unwrap().revision, stale.revision);
        assert_eq!(
            store
                .get_agent_run_citation(&history_run.id, 0)
                .unwrap()
                .content_md,
            original.content_md
        );
    }

    #[test]
    fn test_migration_backfills_old_expired_claim_only_evidence_without_losing_shelf() {
        let (store, dir) = test_store();
        store
            .save_reader_books(&[sample_book("old-impacts", "/tmp/old-impacts")])
            .unwrap();
        let base = store.initialize_base("old-impacts").unwrap();
        let source = sample_source("old-impacts", "old-chapter", "old-span");
        store
            .sync_markdown_sources(&base.id, std::slice::from_ref(&source))
            .unwrap();
        let id = approve(&store, &base.id, "old-topic", "old-span");
        let original = store.get_entry(&id).unwrap();
        store
            .sync_markdown_sources(&base.id, &[updated(source)])
            .unwrap();
        // Reconstruct only the pre-v48 derived status in this temporary DB.
        store.db.transaction(|conn| {
            conn.execute("DELETE FROM knowledge_citations WHERE entry_id=?1",[&id])?;
            conn.execute("UPDATE knowledge_entries SET status='verified',revision=1 WHERE id=?1",[&id])?;
            conn.execute("DELETE FROM knowledge_entry_versions WHERE entry_id=?1 AND revision>1",[&id])?;
            // Reconstruct a coherent pre-v48 migration ledger, including any
            // subsequently added migrations. MAX(version) drives upgrades.
            conn.execute_batch("DROP TABLE knowledge_source_impacts; DELETE FROM _migrations WHERE version>=48;")?;
            Ok(())
        }).unwrap();
        drop(store);
        let db = std::sync::Arc::new(
            crate::infra::sqlite_store::SqliteStore::new(&dir.path().join("book-wiki.db")).unwrap(),
        );
        let migrated = BookWikiStore::new(db);
        let detail = migrated.get_entry(&id).unwrap();
        assert_eq!(detail.entry.status, "stale");
        assert_eq!(detail.content_md, original.content_md);
        assert_eq!(detail.source_impact_count, 1);
        assert_eq!(migrated.list_reader_books().unwrap().len(), 1);
    }

    #[test]
    fn test_no_material_does_not_clear_expired_evidence_but_reviewed_refresh_does() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("impact-refresh", "/tmp/impact-refresh")])
            .unwrap();
        let base = store.initialize_base("impact-refresh").unwrap();
        let source = sample_source("impact-refresh", "chapter", "refresh-span");
        store
            .sync_markdown_sources(&base.id, std::slice::from_ref(&source))
            .unwrap();
        let id = approve(&store, &base.id, "refresh", "refresh-span");
        store
            .sync_markdown_sources(&base.id, &[updated(source)])
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let set = store
            .create_no_material_change_set(&base.id, &run.id, "无新增", "只是导航", &run.id)
            .unwrap();
        store
            .mark_semantic_compile_no_material(&base.id, &set.id, 1, 1)
            .unwrap();
        assert_eq!(store.get_base(&base.id).unwrap().health_state, "warning");
        assert_eq!(store.get_base(&base.id).unwrap().compile_state, "outdated");
        assert_eq!(store.get_entry(&id).unwrap().entry.status, "stale");
        approve(&store, &base.id, "refresh", "refresh-span-new");
        assert_eq!(store.get_entry(&id).unwrap().entry.status, "verified");
        assert_eq!(store.get_base(&base.id).unwrap().health_state, "healthy");
        assert_eq!(store.get_base(&base.id).unwrap().compile_state, "ready");
    }

    #[test]
    fn test_missing_source_does_not_mutate_other_book_or_delete_history() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[
                sample_book("missing-impact", "/tmp/missing-impact"),
                sample_book("foreign-impact", "/tmp/foreign-impact"),
            ])
            .unwrap();
        let base = store.initialize_base("missing-impact").unwrap();
        let foreign = store.initialize_base("foreign-impact").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("missing-impact", "chapter", "span")],
            )
            .unwrap();
        store
            .sync_markdown_sources(
                &foreign.id,
                &[sample_source(
                    "foreign-impact",
                    "foreign-chapter",
                    "foreign-span",
                )],
            )
            .unwrap();
        let id = approve(&store, &base.id, "missing", "span");
        let foreign_id = approve(&store, &foreign.id, "foreign", "foreign-span");
        store.sync_markdown_sources(&base.id, &[]).unwrap();
        let detail = store.get_entry(&id).unwrap();
        assert_eq!(detail.entry.status, "stale");
        assert!(!detail.content_md.is_empty());
        assert!(!detail.citations.is_empty());
        assert_eq!(detail.source_impacts[0].reason, "source_missing");
        assert!(detail.source_impacts[0].current_version_id.is_none());
        assert_eq!(
            store.get_entry(&foreign_id).unwrap().entry.status,
            "verified"
        );
        assert_eq!(store.get_base(&foreign.id).unwrap().health_state, "healthy");
    }
}
