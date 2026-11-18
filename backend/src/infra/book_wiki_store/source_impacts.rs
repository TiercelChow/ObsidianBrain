//! Evidence expiry is a review requirement, not a claim that a fact is false.

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::error::BrainError;
use crate::models::book_wiki::KnowledgeSourceImpact;

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
