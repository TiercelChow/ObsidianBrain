use std::collections::HashMap;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::{record_markdown_sync, BookWikiStore};
use super::{topic_catalog::record_topic_book_build, topic_pages::publish_topic_revision};
use crate::error::BrainError;
use crate::models::wiki_topics::{
    TopicBookBuild, TopicBookBuildSummary, TopicBookDocument, TopicBookPlan, TopicBookSection,
    TopicPublication,
};

pub(crate) struct TopicBookBuildClaim {
    pub attempt_id: String,
    pub snapshot: Option<String>,
    pub previous: Option<TopicBookBuild>,
}

impl BookWikiStore {
    pub(crate) fn topic_book_build_running(&self, base_id: &str) -> Result<bool, BrainError> {
        self.db.with_connection(|conn| conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM wiki_topic_book_builds WHERE knowledge_base_id = ?1 AND status = 'running')", [base_id], |row| row.get(0),
        ).map_err(Into::into))
    }

    pub(crate) fn claim_topic_book_build(
        &self,
        base_id: &str,
    ) -> Result<Option<TopicBookBuildClaim>, BrainError> {
        self.db.transaction(|conn| {
            let active: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_bases WHERE id = ?1 AND lifecycle = 'active')",
                [base_id], |row| row.get(0),
            )?;
            if !active { return Err(BrainError::KnowledgeConflict("知识库当前不可编译".into())); }
            let previous = read_build(conn, base_id)?;
            if previous.as_ref().is_some_and(|(build, _)| build.status == "running") {
                return Ok(None);
            }
            let (previous, snapshot) = previous.map_or((None, None), |(build, hash)| (Some(build), hash));
            let attempt_id = Uuid::new_v4().to_string();
            let state = TopicBookBuild {
                id: attempt_id.clone(), status: "running".into(), phase: "syncing".into(),
                error: None, stop_requested: false,
                ..previous.clone().unwrap_or_default()
            };
            conn.execute(
                "INSERT INTO wiki_topic_book_builds(knowledge_base_id, attempt_id, status, snapshot_hash, state_json, updated_at)
                 VALUES(?1, ?2, 'running', ?3, ?4, ?5) ON CONFLICT(knowledge_base_id) DO UPDATE
                 SET attempt_id = excluded.attempt_id, status = excluded.status, stop_requested = 0, snapshot_hash = excluded.snapshot_hash,
                     state_json = excluded.state_json, updated_at = excluded.updated_at",
                params![base_id, attempt_id, snapshot, serde_json::to_string(&state).map_err(build_json_error)?, Utc::now().to_rfc3339()],
            )?;
            Ok(Some(TopicBookBuildClaim { attempt_id, snapshot, previous }))
        })
    }

    pub fn topic_book_build(&self, base_id: &str) -> Result<Option<TopicBookBuild>, BrainError> {
        self.get_base(base_id)?;
        self.current_topic_book_build(base_id)
    }

    pub fn request_topic_book_stop(
        &self,
        base_id: &str,
        attempt: &str,
    ) -> Result<TopicBookBuild, BrainError> {
        self.db.transaction(|conn| {
            conn.execute(
                "UPDATE wiki_topic_book_builds SET stop_requested = 1 WHERE knowledge_base_id = ?1 AND attempt_id = ?2 AND status = 'running'",
                params![base_id, attempt],
            )?;
            read_build(conn, base_id)?.map(|(build, _)| build).filter(|build| build.id == attempt)
                .ok_or_else(|| BrainError::KnowledgeNotFound("没有全库编译记录".into()))
        })
    }

    pub(crate) fn topic_book_stop_requested(
        &self,
        base_id: &str,
        attempt: &str,
    ) -> Result<bool, BrainError> {
        self.db.with_connection(|conn| conn.query_row(
            "SELECT stop_requested FROM wiki_topic_book_builds WHERE knowledge_base_id = ?1 AND attempt_id = ?2 AND status = 'running'",
            params![base_id, attempt], |row| row.get(0),
        ).optional()?.ok_or_else(|| BrainError::KnowledgeConflict("全库编译已结束或已被替代".into())))
    }

    pub(super) fn topic_book_build_summary(
        &self,
        base_id: &str,
    ) -> Result<Option<TopicBookBuildSummary>, BrainError> {
        Ok(self
            .current_topic_book_build(base_id)?
            .map(|build| TopicBookBuildSummary {
                status: build.status,
                phase: build.phase,
                outdated: build.outdated,
                source_count: build.sources.len(),
                ready_pages: build
                    .pages
                    .iter()
                    .filter(|page| page.revision_id.is_some())
                    .count(),
                total_pages: build.pages.len(),
            }))
    }

    fn current_topic_book_build(
        &self,
        base_id: &str,
    ) -> Result<Option<TopicBookBuild>, BrainError> {
        self.db.with_connection(|conn| {
            let Some((mut build, snapshot)) = read_build(conn, base_id)? else {
                return Ok(None);
            };
            if let Some(snapshot) = snapshot {
                let versions = read_book_versions(conn, base_id)?;
                build.outdated |= snapshot != versions;
            }
            Ok(Some(build))
        })
    }

    pub(crate) fn save_topic_book_build(
        &self,
        base_id: &str,
        build: &TopicBookBuild,
        snapshot: Option<&str>,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            let now = Utc::now().to_rfc3339();
            let updated = conn.execute(
                "UPDATE wiki_topic_book_builds SET status = ?3, state_json = ?4, snapshot_hash = ?5, updated_at = ?6
                 WHERE knowledge_base_id = ?1 AND attempt_id = ?2 AND status = 'running'",
                params![base_id, build.id, build.status, serde_json::to_string(build).map_err(build_json_error)?, snapshot, now],
            )?;
            if updated == 0 { return Err(BrainError::KnowledgeConflict("本次全库编译已结束或被替代".into())); }
            if build.status != "running" {
                record_topic_book_build(conn, base_id, build, &now)?;
            }
            Ok(())
        })
    }

    pub(crate) fn publish_topic_build_sections(
        &self,
        base_id: &str,
        attempt_id: &str,
        pages: &[TopicPublication],
    ) -> Result<usize, BrainError> {
        self.db.transaction(|conn| {
            let (build, snapshot) = read_build(conn, base_id)?
                .filter(|(build, _)| build.id == attempt_id && build.phase == "finished")
                .ok_or_else(|| {
                    BrainError::KnowledgeConflict("请刷新并选择已结束的编译批次".into())
                })?;
            if build.outdated
                || snapshot.as_deref() != Some(read_book_versions(conn, base_id)?.as_str())
            {
                return Err(BrainError::KnowledgeConflict(
                    "原文已变化，请更新 Wiki 后再发布".into(),
                ));
            }
            let mut count = 0;
            for page in pages {
                if !build.pages.iter().any(|record| {
                    record.page_id == page.page_id
                        && record.focus.is_some()
                        && record.error.is_none()
                        && record.revision_id.as_ref() == Some(&page.revision_id)
                }) {
                    return Err(BrainError::KnowledgeConflict(
                        "草稿不再属于本次编译，请刷新后重试".into(),
                    ));
                }
                count += usize::from(publish_topic_revision(
                    conn,
                    base_id,
                    &page.page_id,
                    &page.revision_id,
                    &page.sections,
                )?);
            }
            Ok(count)
        })
    }

    pub(crate) fn topic_book_inventory(
        &self,
        base_id: &str,
    ) -> Result<(String, Vec<TopicBookDocument>), BrainError> {
        self.db.with_connection(|conn| {
            let versions = read_book_versions(conn, base_id)?;
            let mut documents = conn.prepare(
                "SELECT id, relative_path, current_version_id FROM source_documents
                 WHERE knowledge_base_id = ?1 AND sync_status = 'current' AND current_version_id IS NOT NULL
                 ORDER BY ordinal, relative_path, id",
            )?;
            let mut sections = conn.prepare(
                "SELECT id, heading, length(content)
                 FROM source_spans WHERE source_version_id = ?1 AND knowledge_base_id = ?2
                   AND length(trim(content, char(10) || char(13) || char(9) || ' ')) > 0 ORDER BY ordinal",
            )?;
            let mut rows = documents.query([base_id])?;
            let mut inventory = Vec::new();
            while let Some(row) = rows.next()? {
                let version: String = row.get(2)?;
                let excerpts = sections.query_map(params![version, base_id], |section| Ok(TopicBookSection {
                    id: section.get(0)?, heading: section.get(1)?, total_chars: section.get(2)?,
                }))?.collect::<Result<Vec<_>, _>>()?;
                inventory.push(TopicBookDocument { id: row.get(0)?, path: row.get(1)?, version, sections: excerpts });
            }
            Ok((versions, inventory))
        })
    }

    pub(crate) fn topic_book_snapshot_matches(
        &self,
        base_id: &str,
        expected: &str,
    ) -> Result<bool, BrainError> {
        self.db
            .with_connection(|conn| Ok(read_book_versions(conn, base_id)? == expected))
    }

    pub(crate) fn reuse_indexed_topic_snapshot(
        &self,
        base_id: &str,
        scanned: &HashMap<&str, &str>,
    ) -> Result<Option<usize>, BrainError> {
        self.db.transaction(|conn| {
            let current = read_book_source_versions(conn, base_id)?;
            if current.len() != scanned.len()
                || current.iter().any(|(id, version)| {
                    version.as_deref() != scanned.get(id.as_str()).copied() || version.is_none()
                })
            {
                return Ok(None);
            }
            let sections = conn.query_row(
                "SELECT COUNT(*) FROM source_documents d
                 JOIN source_spans s ON s.source_version_id = d.current_version_id
                 WHERE d.knowledge_base_id = ?1 AND d.sync_status = 'current'",
                [base_id],
                |row| row.get(0),
            )?;
            record_markdown_sync(conn, base_id, !scanned.is_empty(), &Utc::now().to_rfc3339())?;
            Ok(Some(sections))
        })
    }

    pub(crate) fn topic_book_source_text(
        &self,
        base_id: &str,
        version_id: &str,
    ) -> Result<String, BrainError> {
        self.db.with_connection(|conn| {
            let mut query = conn.prepare(
                "SELECT content FROM source_spans
                 WHERE knowledge_base_id = ?1 AND source_version_id = ?2 ORDER BY ordinal",
            )?;
            let rows =
                query.query_map(params![base_id, version_id], |row| row.get::<_, String>(0))?;
            let sections = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(sections.join("\n\n"))
        })
    }

    pub(crate) fn read_topic_book_plan(
        &self,
        base_id: &str,
        hash: &str,
    ) -> Result<Option<TopicBookPlan>, BrainError> {
        self.db.with_connection(|conn| {
            let plan = conn.query_row(
                "SELECT plan_json FROM wiki_topic_book_plans WHERE knowledge_base_id = ?1 AND input_hash = ?2",
                params![base_id, hash], |row| row.get::<_, String>(0),
            ).optional()?;
            plan.map(|plan| serde_json::from_str(&plan).map_err(build_json_error)).transpose()
        })
    }

    pub(crate) fn save_topic_book_plan(
        &self,
        base_id: &str,
        hash: &str,
        plan: &TopicBookPlan,
    ) -> Result<(), BrainError> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO wiki_topic_book_plans(knowledge_base_id, input_hash, plan_json, created_at)
                 VALUES(?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
                params![base_id, hash, serde_json::to_string(plan).map_err(build_json_error)?, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }
}

fn read_book_versions(conn: &Connection, base_id: &str) -> Result<String, BrainError> {
    let versions = read_book_source_versions(conn, base_id)?;
    Ok(super::stable_id(
        "topic-book",
        &serde_json::to_string(&versions).map_err(build_json_error)?,
    ))
}

fn read_book_source_versions(
    conn: &Connection,
    base_id: &str,
) -> Result<Vec<(String, Option<String>)>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT id, current_version_id FROM source_documents WHERE knowledge_base_id = ?1 AND sync_status = 'current' ORDER BY id",
    )?;
    let rows = stmt.query_map([base_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn read_build(
    conn: &Connection,
    base_id: &str,
) -> Result<Option<(TopicBookBuild, Option<String>)>, BrainError> {
    let row = conn.query_row(
        "SELECT state_json, snapshot_hash, stop_requested FROM wiki_topic_book_builds WHERE knowledge_base_id = ?1",
        [base_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, bool>(2)?)),
    ).optional()?;
    row.map(|(state, hash, stop_requested)| {
        let mut build: TopicBookBuild = serde_json::from_str(&state).map_err(build_json_error)?;
        build.stop_requested = stop_requested;
        Ok((build, hash))
    })
    .transpose()
}

pub(super) fn mark_interrupted(conn: &Connection, now: &str) -> Result<(), BrainError> {
    conn.execute(
        "INSERT INTO wiki_topic_build_log(attempt_id, knowledge_base_id, status, source_count,
                                          linked_sources, written_pages, planned_pages, recorded_at)
         SELECT attempt_id, knowledge_base_id, 'partial', COALESCE(json_array_length(state_json, '$.sources'),0),
                (SELECT COUNT(*) FROM json_each(state_json, '$.sources') WHERE json_extract(value, '$.status') = 'linked'),
                (SELECT COUNT(*) FROM json_each(state_json, '$.pages') WHERE json_extract(value, '$.revision_id') IS NOT NULL),
                COALESCE(json_array_length(state_json, '$.pages'),0), ?1
         FROM wiki_topic_book_builds WHERE status='running' ON CONFLICT(attempt_id) DO NOTHING",
        [now],
    )?;
    conn.execute(
        "UPDATE wiki_topic_book_builds SET status = 'partial', updated_at = ?1,
         state_json = json_set(state_json, '$.status', 'partial', '$.phase', 'interrupted',
                  '$.error', '服务重启中断了全库编译，可以继续') WHERE status = 'running'",
        [now],
    )?;
    Ok(())
}

fn build_json_error(error: serde_json::Error) -> BrainError {
    BrainError::Internal(format!("全库编译记录解析失败: {error}"))
}
