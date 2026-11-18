use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use super::{stable_id, BookWikiStore};
use crate::error::BrainError;
use crate::models::wiki_review::{TopicReviewPage, TopicReviewReport};

impl BookWikiStore {
    pub(crate) fn topic_review_pages(
        &self,
        base_id: &str,
    ) -> Result<Vec<TopicReviewPage>, BrainError> {
        self.db.with_connection(|conn| {
            let mut query = conn.prepare(
                "SELECT p.id,p.title,r.id,r.run_id,r.markdown,
                     EXISTS(SELECT 1 FROM wiki_topic_stale_revisions s WHERE s.revision_id=r.id)
                 FROM wiki_topic_pages p JOIN wiki_topic_revisions r ON r.id=p.published_id
                 WHERE p.knowledge_base_id=?1 ORDER BY p.title,p.id",
            )?;
            let rows = query.query_map([base_id], |row| {
                Ok(TopicReviewPage {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    revision: row.get(2)?,
                    run_id: row.get(3)?,
                    markdown: row.get(4)?,
                    stale: row.get(5)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub(crate) fn latest_topic_review(
        &self,
        base_id: &str,
        snapshot: &str,
    ) -> Result<Option<TopicReviewReport>, BrainError> {
        self.db.with_connection(|conn| {
            let raw: Option<String> = conn.query_row(
                "SELECT report_json FROM wiki_topic_reviews WHERE knowledge_base_id=?1 AND snapshot_hash=?2",
                params![base_id, snapshot], |row| row.get(0),
            ).optional()?;
            raw.map(|raw| serde_json::from_str(&raw).map_err(|error| BrainError::Internal(error.to_string())))
                .transpose()
        })
    }

    pub(crate) fn save_topic_review(
        &self,
        base_id: &str,
        snapshot: &str,
        report: &TopicReviewReport,
    ) -> Result<(), BrainError> {
        let raw = serde_json::to_string(report)
            .map_err(|error| BrainError::Internal(error.to_string()))?;
        self.db.transaction(|conn| {
            if review_snapshot(conn, base_id)? != snapshot {
                return Err(BrainError::KnowledgeConflict("体检期间主题或原文已变化，请重新检查".into()));
            }
            conn.execute(
                "INSERT INTO wiki_topic_reviews(knowledge_base_id,snapshot_hash,report_json) VALUES(?1,?2,?3)
                 ON CONFLICT(knowledge_base_id) DO UPDATE SET snapshot_hash=excluded.snapshot_hash, report_json=excluded.report_json",
                params![base_id,snapshot,raw],
            )?;
            Ok(())
        })
    }
}

pub(crate) fn topic_review_snapshot(pages: &[TopicReviewPage]) -> String {
    stable_id(
        "topic-review",
        &json!(pages
            .iter()
            .map(|page| (&page.id, &page.revision, page.stale))
            .collect::<Vec<_>>())
        .to_string(),
    )
}

fn review_snapshot(conn: &Connection, base_id: &str) -> Result<String, BrainError> {
    let mut query = conn.prepare(
        "SELECT p.id,r.id,EXISTS(SELECT 1 FROM wiki_topic_stale_revisions s WHERE s.revision_id=r.id)
         FROM wiki_topic_pages p JOIN wiki_topic_revisions r ON r.id=p.published_id
         WHERE p.knowledge_base_id=?1 ORDER BY p.title,p.id",
    )?;
    let rows = query.query_map([base_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, bool>(2)?,
        ))
    })?;
    Ok(stable_id(
        "topic-review",
        &json!(rows.collect::<Result<Vec<_>, _>>()?).to_string(),
    ))
}
