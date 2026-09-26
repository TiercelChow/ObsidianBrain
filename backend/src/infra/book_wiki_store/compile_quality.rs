//! Compilation identity metadata. Existing source bodies remain read-only.

use super::{parse_string_list, BookWikiStore, CompileCatalogEntry};
use crate::error::BrainError;
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;

pub(crate) struct CompileEntrySnapshot {
    pub entry_id: String,
    pub revision: i64,
    pub fresh: bool,
    pub status: String,
    pub candidate: serde_json::Value,
}

impl BookWikiStore {
    pub(crate) fn current_compile_span_ids(
        &self,
        base_id: &str,
    ) -> Result<HashSet<String>, BrainError> {
        self.get_base(base_id)?;
        self.db
            .with_connection(|conn| super::source_impacts::current_span_ids(conn, base_id))
    }

    pub(crate) fn compile_entry_snapshot(
        &self,
        base_id: &str,
        kind: &str,
        slug: &str,
    ) -> Result<Option<CompileEntrySnapshot>, BrainError> {
        let id = self.db.with_connection(|conn| {
            conn.query_row("SELECT id FROM knowledge_entries WHERE knowledge_base_id=?1 AND entry_type=?2 AND slug=?3", params![base_id, kind, slug], |row| row.get::<_, String>(0)).optional().map_err(Into::into)
        })?;
        let Some(id) = id else { return Ok(None) };
        let detail = self.get_entry(&id)?;
        let fresh = !matches!(detail.entry.status.as_str(), "stale" | "archived")
            && detail.source_impact_count == 0;
        Ok(Some(CompileEntrySnapshot {
            entry_id: id,
            revision: detail.revision,
            fresh,
            status: detail.entry.status.clone(),
            candidate: self.entry_candidate_snapshot(&detail)?,
        }))
    }
    pub fn list_compile_catalog(
        &self,
        base_id: &str,
    ) -> Result<Vec<CompileCatalogEntry>, BrainError> {
        self.get_base(base_id)?;
        self.db.with_connection(|conn| {
            // Include stale identities for recompile alignment, but not stale
            // facts. No chapter body is loaded, and there is no first-500 gate.
            let mut stmt = conn.prepare(
                "SELECT id,entry_type,slug,title,summary,aliases_json,status,revision
                FROM knowledge_entries WHERE knowledge_base_id=?1 AND entry_type<>'source_section'
                    AND status<>'archived' ORDER BY id",
            )?;
            let rows = stmt.query_map([base_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })?;
            rows.map(|row| {
                let (id, entry_type, slug, title, summary, aliases, status, revision) = row?;
                Ok(CompileCatalogEntry {
                    id,
                    entry_type,
                    slug,
                    title,
                    summary,
                    aliases: parse_string_list(&aliases, "编译实体别名")?,
                    status,
                    revision,
                })
            })
            .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{sample_book, test_store};
    use rusqlite::params;

    #[test]
    fn test_compile_snapshot_preserves_individual_claim_sources() {
        use super::super::tests::{sample_source, semantic_candidate};
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("snapshot", "/tmp/snapshot")])
            .unwrap();
        let base = store.initialize_base("snapshot").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[
                    sample_source("one", "one", "s1"),
                    sample_source("two", "two", "s2"),
                ],
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
        let mut candidate = semantic_candidate("s1", "不同条件的主题");
        candidate["citations"] = serde_json::json!(["s1", "s2"]);
        candidate["claims"] = serde_json::json!([
            {"claim_text":"条件一成立", "predicate":"states", "object_text":"条件一", "citations":["s1"]},
            {"claim_text":"条件二成立", "predicate":"states", "object_text":"条件二", "citations":["s2"]}
        ]);
        let set = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "首次",
                "",
                "snapshot-test",
                &[candidate],
            )
            .unwrap();
        store.resolve_change_set(&set.id, true, "").unwrap();
        let detail = store.get_entry(&set.changes[0].object_id).unwrap();
        let snapshot = store.entry_candidate_snapshot(&detail).unwrap();
        for claim in snapshot["claims"].as_array().unwrap() {
            let source = if claim["object_text"] == "条件一" {
                "s1"
            } else {
                "s2"
            };
            assert_eq!(claim["citations"], serde_json::json!([source]));
        }
        store.db.with_connection(|conn| {
            conn.execute("UPDATE knowledge_claims SET verification_status=CASE WHEN object_text='条件一' THEN 'rejected' ELSE 'disputed' END WHERE entry_id=?1",[&detail.entry.id])?;
            Ok(())
        }).unwrap();
        let protected_snapshot = store
            .entry_candidate_snapshot(&store.get_entry(&detail.entry.id).unwrap())
            .unwrap();
        let rewrite = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "润色更新",
                "",
                "snapshot-rewrite",
                &[protected_snapshot],
            )
            .unwrap();
        store.resolve_change_set(&rewrite.id, true, "").unwrap();
        for claim in store.get_entry(&detail.entry.id).unwrap().claims {
            assert_eq!(
                claim.verification_status,
                if claim.object_text.as_deref() == Some("条件一") {
                    "rejected"
                } else {
                    "disputed"
                }
            );
        }
        let mut duplicate = store
            .entry_candidate_snapshot(&store.get_entry(&detail.entry.id).unwrap())
            .unwrap();
        let same_claim = duplicate["claims"][0].clone();
        duplicate["claims"].as_array_mut().unwrap().push(same_claim);
        let error = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "重复论断",
                "",
                "snapshot-duplicate",
                &[duplicate],
            )
            .unwrap_err();
        assert!(error.to_string().contains("重复"));
        assert_eq!(store.list_change_sets(&base.id, None).unwrap().len(), 2);
    }

    #[test]
    fn test_compile_expected_revision_rejects_newer_content_before_creating_proposal() {
        use super::super::tests::{sample_source, semantic_candidate};
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("revision", "/tmp/revision")])
            .unwrap();
        let base = store.initialize_base("revision").unwrap();
        store
            .sync_markdown_sources(&base.id, &[sample_source("revision", "chapter", "s1")])
            .unwrap();
        let run = store
            .start_agent_run(
                &base.id,
                "deepseek_harness",
                "knowledge_ingest",
                &serde_json::json!({}),
            )
            .unwrap();
        let candidate = semantic_candidate("s1", "旧内容");
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "首次",
                "",
                "revision-initial",
                &[candidate.clone()],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        let mut newer = candidate.clone();
        newer["content_md"] = serde_json::json!("用户刚刚修改的完整内容");
        let update = store
            .create_semantic_change_set(&base.id, &run.id, "更新", "", "revision-new", &[newer])
            .unwrap();
        store.resolve_change_set(&update.id, true, "").unwrap();
        let mut delayed = candidate;
        delayed["_expected_revision"] = serde_json::json!(1);
        let error = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "迟到编译",
                "",
                "revision-delayed",
                &[delayed],
            )
            .unwrap_err();
        assert!(error.to_string().contains("版本已变化"));
        assert_eq!(store.list_change_sets(&base.id, None).unwrap().len(), 2);
        assert_eq!(
            store
                .get_entry(&initial.changes[0].object_id)
                .unwrap()
                .content_md,
            "用户刚刚修改的完整内容"
        );
    }

    #[test]
    fn test_compile_update_cannot_remove_manual_protection() {
        use super::super::tests::{sample_source, semantic_candidate};
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("protected", "/tmp/protected")])
            .unwrap();
        let base = store.initialize_base("protected").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("protected", "chapter", "protected-span")],
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
        let candidate = semantic_candidate("protected-span", "已有知识");
        let initial = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "首次",
                "",
                "protect-initial",
                &[candidate.clone()],
            )
            .unwrap();
        store.resolve_change_set(&initial.id, true, "").unwrap();
        let entry_id = &initial.changes[0].object_id;
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET edit_policy='human_protected' WHERE id=?1",
                    [entry_id],
                )?;
                Ok(())
            })
            .unwrap();
        // Neither an omitted policy nor an explicit agent-managed policy may
        // make approval of model content implicitly release a manual lock.
        for (index, policy) in [None, Some("agent_managed")].into_iter().enumerate() {
            let mut next = candidate.clone();
            next["content_md"] = serde_json::json!(format!("新增知识 {index}"));
            if let Some(policy) = policy {
                next["edit_policy"] = serde_json::json!(policy);
            }
            let set = store
                .create_semantic_change_set(
                    &base.id,
                    &run.id,
                    "更新",
                    "",
                    &format!("protect-update-{index}"),
                    &[next],
                )
                .unwrap();
            assert_eq!(set.risk_level, "high");
            assert_eq!(set.changes[0].after["edit_policy"], "human_protected");
            assert_eq!(
                store.get_entry(entry_id).unwrap().content_md,
                if index == 0 {
                    candidate["content_md"].as_str().unwrap().to_string()
                } else {
                    "新增知识 0".to_string()
                }
            );
            store.resolve_change_set(&set.id, true, "").unwrap();
            assert_eq!(
                store.get_entry(entry_id).unwrap().edit_policy,
                "human_protected"
            );
        }
    }

    #[test]
    fn test_compile_catalog_includes_late_and_stale_identity_but_not_other_books() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[
                sample_book("catalog-quality", "/tmp/catalog-quality"),
                sample_book("foreign-quality", "/tmp/foreign-quality"),
            ])
            .unwrap();
        let base = store.initialize_base("catalog-quality").unwrap();
        let foreign = store.initialize_base("foreign-quality").unwrap();
        store.db.transaction(|conn| {
            let mut stmt=conn.prepare("INSERT INTO knowledge_entries
                (id,knowledge_base_id,entry_type,slug,title,summary,content_md,status,aliases_json,created_at,updated_at)
                VALUES(?1,?2,'concept',?1,'主题','概述','完整正文',?3,'[\"PagedAttention\"]',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)")?;
            for index in 0..1100 {
                stmt.execute(params![format!("catalog-{index:04}"),base.id,if index==1088 {"stale"}else{"verified"}])?;
            }
            stmt.execute(params!["foreign-only",foreign.id,"verified"])?;
            stmt.execute(params!["archived-only",base.id,"archived"])?;
            Ok(())
        }).unwrap();
        let rows = store.list_compile_catalog(&base.id).unwrap();
        assert_eq!(rows.len(), 1100);
        let late = rows.iter().find(|row| row.id == "catalog-1088").unwrap();
        assert_eq!(late.status, "stale");
        assert_eq!(late.aliases, vec!["PagedAttention"]);
        assert!(!rows
            .iter()
            .any(|row| row.id == "foreign-only" || row.id == "archived-only"));
    }
}
