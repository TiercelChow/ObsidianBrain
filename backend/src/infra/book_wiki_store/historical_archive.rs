//! Status-only human archives never revalidate, rewrite or relabel old facts.
use super::*;

fn invalid(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

fn payload_hash(value: &Value) -> String {
    stable_id("historical-archive-payload", &value.to_string())
}

impl BookWikiStore {
    /// Creates a reviewed, metadata-only human operation, not a knowledge update.
    pub fn propose_entry_archive(
        &self,
        entry_id: &str,
        expected_revision: i64,
    ) -> Result<KnowledgeChangeSet, BrainError> {
        let detail = self.get_entry(entry_id)?;
        self.get_active_base(&detail.entry.knowledge_base_id)?;
        if detail.entry.entry_type == "source_section" || detail.entry.status == "archived" {
            return Err(invalid("来源章节或已归档实体不能提交历史归档"));
        }
        if detail.revision != expected_revision {
            return Err(invalid("知识实体已变化，请刷新后重试"));
        }
        let mut before = self.entry_candidate_snapshot(&detail)?;
        before["id"] = serde_json::json!(entry_id);
        let mut after = before.clone();
        after["status"] = serde_json::json!("archived");
        after["edit_policy"] = serde_json::json!("human_protected");
        let base = &detail.entry.knowledge_base_id;
        let run = self.start_agent_run(base,"user","knowledge_manual_archive",&serde_json::json!({"entry_id":entry_id,"expected_revision":expected_revision,"status_only":true}))?;
        let set = format!("historical-archive-{}", uuid::Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        let result = self.db.transaction(|conn| {
            let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_entries e JOIN knowledge_bases b ON b.id=e.knowledge_base_id WHERE e.id=?1 AND e.knowledge_base_id=?2 AND e.revision=?3 AND e.status<>'archived' AND e.entry_type<>'source_section' AND b.lifecycle='active')",params![entry_id,base,expected_revision],|row|row.get(0))?;
            if !valid { return Err(invalid("知识实体或知识库状态已变化，请刷新后重试")); }
            let pending:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_historical_archive_authorizations a JOIN knowledge_change_sets s ON s.id=a.change_set_id WHERE a.entry_id=?1 AND s.status='proposed')",[entry_id],|row|row.get(0))?;
            if pending { return Err(invalid("该实体已有待处理的归档审核，请先批准或驳回")); }
            let impact=knowledge_change_impact(conn,entry_id,false)?;
            let audit=serde_json::json!({"passed":true,"historical_archive":true,"entry_citations":0,"claim_citations":0,"explicit_claim_citations":0,"inherited_claims":0,"effective_claim_citations":0,"issues":[]});
            conn.execute("INSERT INTO knowledge_change_sets(id,knowledge_base_id,agent_run_id,title,reason,risk_level,status,idempotency_key,classification_summary_json,citation_audit_json,impact_summary_json,created_at) VALUES(?1,?2,?3,?4,?5,'high','proposed',?1,?6,?7,?8,?9)",params![set,base,run.id,format!("历史归档：{}",detail.entry.title),"仅归档并保留历史正文、论断、关系和引用；不校验为当前事实、不清除来源影响，不自动恢复主题",serde_json::json!({"new":0,"update":1,"disputed":0,"no_material":0}).to_string(),audit.to_string(),impact.to_string(),now])?;
            conn.execute("INSERT INTO knowledge_changes(id,change_set_id,ordinal,operation,object_type,object_id,expected_revision,classification,citation_audit_json,impact_json,before_json,after_json) VALUES(?1,?2,0,'archive','entry',?3,?4,'update',?5,?6,?7,?8)",params![stable_id("change",&set),set,entry_id,expected_revision,audit.to_string(),impact.to_string(),before.to_string(),after.to_string()])?;
            conn.execute("INSERT INTO knowledge_historical_archive_authorizations(change_set_id,knowledge_base_id,entry_id,expected_revision,before_hash,after_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![set,base,entry_id,expected_revision,payload_hash(&before),payload_hash(&after),now])?;
            conn.execute("INSERT INTO knowledge_reviews(id,knowledge_base_id,change_set_id,status,created_at) VALUES(?1,?2,?3,'pending',?4)",params![stable_id("review",&set),base,set,now])?;
            refresh_review_state(conn,base,&now)?;
            Ok(())
        });
        if let Err(error) = result {
            let _ = self.fail_agent_run(&run.id, &error.to_string());
            return Err(error);
        }
        self.complete_agent_run(
            &run.id,
            &serde_json::json!({"change_set_id":set,"historical_archive":true}),
        )?;
        self.get_change_set(&set)
    }
}

/// Only the dedicated server operation can mint an authorization. Caller-owned
/// candidate/audit flags are never authority to bypass current-source checks.
pub(super) fn authorized(
    conn: &rusqlite::Connection,
    set: &KnowledgeChangeSet,
) -> Result<bool, BrainError> {
    let auth=conn.query_row("SELECT knowledge_base_id,entry_id,expected_revision,before_hash,after_hash FROM knowledge_historical_archive_authorizations WHERE change_set_id=?1",[&set.id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?))).optional()?;
    let Some((base, entry, revision, before_hash, after_hash)) = auth else {
        return Ok(false);
    };
    let change = set
        .changes
        .first()
        .ok_or_else(|| invalid("历史归档授权缺少实体变更"))?;
    let before = change
        .before
        .as_ref()
        .ok_or_else(|| invalid("历史归档必须有完整原始快照"))?;
    let mut expected = before.clone();
    expected["status"] = serde_json::json!("archived");
    expected["edit_policy"] = serde_json::json!("human_protected");
    if set.changes.len() != 1
        || set.knowledge_base_id != base
        || change.object_id != entry
        || change.object_type != "entry"
        || change.operation != "archive"
        || change.expected_revision != Some(revision)
        || payload_hash(before) != before_hash
        || payload_hash(&change.after) != after_hash
        || change.after != expected
    {
        return Err(invalid(
            "历史归档授权与变更不匹配；不能修改正文或绕过当前依据校验",
        ));
    }
    let active: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_bases WHERE id=?1 AND lifecycle='active')",
        [base],
        |row| row.get(0),
    )?;
    if !active {
        return Err(invalid("知识库已暂停或归档，不能应用历史归档"));
    }
    Ok(true)
}

pub(super) fn apply(
    conn: &rusqlite::Connection,
    set: &KnowledgeChangeSet,
    now: &str,
) -> Result<(), BrainError> {
    let change = set
        .changes
        .first()
        .ok_or_else(|| invalid("历史归档缺少实体变更"))?;
    conn.execute("INSERT OR IGNORE INTO knowledge_entry_versions(id,entry_id,revision,title,summary,content_md,aliases_json,status,confidence,changed_by_run_id,created_at) SELECT 'entry-version-'||id||'-'||revision,id,revision,title,summary,content_md,aliases_json,status,confidence,updated_by_run_id,updated_at FROM knowledge_entries WHERE id=?1",[&change.object_id])?;
    let changed=conn.execute("UPDATE knowledge_entries SET status='archived',edit_policy='human_protected',revision=revision+1,updated_by_run_id=?2,updated_at=?3 WHERE id=?1 AND knowledge_base_id=?4 AND revision=?5",params![change.object_id,set.agent_run_id,now,set.knowledge_base_id,change.expected_revision])?;
    if changed != 1 {
        return Err(invalid("归档基线已变化，未修改历史内容"));
    }
    conn.execute("INSERT INTO knowledge_entry_versions(id,entry_id,revision,title,summary,content_md,aliases_json,status,confidence,changed_by_run_id,created_at) SELECT 'entry-version-'||id||'-'||revision,id,revision,title,summary,content_md,aliases_json,status,confidence,updated_by_run_id,updated_at FROM knowledge_entries WHERE id=?1",[&change.object_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{sample_book, sample_source, semantic_candidate, test_store};
    use super::*;

    fn setup() -> (BookWikiStore, tempfile::TempDir, String, String) {
        let (store, dir) = test_store();
        store
            .save_reader_books(&[sample_book("archive", "/tmp/archive")])
            .unwrap();
        let base = store.initialize_base("archive").unwrap();
        store
            .sync_markdown_sources(&base.id, &[sample_source("doc", "section", "span")])
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
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "初次",
                "",
                "archive-initial",
                &[semantic_candidate("span", "主题")],
            )
            .unwrap();
        store.resolve_change_set(&set.id, true, "").unwrap();
        store.sync_markdown_sources(&base.id, &[]).unwrap();
        (store, dir, base.id, set.changes[0].object_id.clone())
    }

    #[test]
    fn test_historical_archive_keeps_body_claims_citations_relations_and_old_versions() {
        let (store, _dir, base, entry) = setup();
        store.db.with_connection(|conn| {
            conn.execute("UPDATE knowledge_claims SET verification_status='disputed' WHERE entry_id=?1",[&entry])?;
            conn.execute("INSERT INTO knowledge_relations(id,knowledge_base_id,from_entry_id,to_entry_id,relation_type,evidence,created_at,updated_at) VALUES('kept-relation',?1,?2,'section','对比','历史关系依据',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",params![base,entry])?;
            Ok(())
        }).unwrap();
        let before = store.get_entry(&entry).unwrap();
        assert_eq!(before.entry.status, "stale");
        let snapshot = store.entry_candidate_snapshot(&before).unwrap();
        let proposal = store
            .propose_entry_archive(&entry, before.revision)
            .unwrap();
        assert_eq!(proposal.risk_level, "high");
        assert_eq!(proposal.changes[0].operation, "archive");
        assert_eq!(store.get_entry(&entry).unwrap().entry.status, "stale");
        assert!(store
            .propose_entry_archive(&entry, before.revision)
            .is_err());
        store
            .resolve_change_set(&proposal.id, true, "保留历史，不再使用")
            .unwrap();
        let after = store.get_entry(&entry).unwrap();
        assert_eq!(after.entry.status, "archived");
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.content_md, before.content_md);
        assert_eq!(after.relations.len(), before.relations.len());
        assert_eq!(after.relations[0].id, before.relations[0].id);
        assert!(after
            .versions
            .iter()
            .any(|version| version.revision == before.revision));
        assert_eq!(after.claims[0].id, before.claims[0].id);
        assert_eq!(
            after.claims[0].verification_status,
            before.claims[0].verification_status
        );
        let mut expected = snapshot;
        expected["status"] = serde_json::json!("archived");
        expected["edit_policy"] = serde_json::json!("human_protected");
        assert_eq!(store.entry_candidate_snapshot(&after).unwrap(), expected);
        assert!(store.list_qa_catalog(&base).unwrap().is_empty());
        assert!(
            after.source_impact_count > 0,
            "historical archive does not certify stale evidence"
        );
        assert!(store.propose_entry_archive(&entry, after.revision).is_err());
        assert!(store.resolve_change_set(&proposal.id, true, "").is_err());
    }

    #[test]
    fn test_archive_rejects_forged_payload_late_revision_and_paused_book() {
        let (store, _dir, base, entry) = setup();
        let before = store.get_entry(&entry).unwrap();
        assert!(store.propose_entry_archive("section", 1).is_err());
        assert!(store.propose_entry_archive("no-such-entry", 1).is_err());
        let proposal = store
            .propose_entry_archive(&entry, before.revision)
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                let mut payload = proposal.changes[0].after.clone();
                payload["content_md"] = serde_json::json!("伪造的新事实");
                conn.execute(
                    "UPDATE knowledge_changes SET after_json=?2 WHERE change_set_id=?1",
                    params![proposal.id, payload.to_string()],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store.resolve_change_set(&proposal.id, true, "").is_err());
        assert_eq!(
            store.get_entry(&entry).unwrap().content_md,
            before.content_md
        );
        store.resolve_change_set(&proposal.id, false, "").unwrap();
        let late = store
            .propose_entry_archive(&entry, before.revision)
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET revision=revision+1 WHERE id=?1",
                    [&entry],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store.resolve_change_set(&late.id, true, "").is_err());
        store.resolve_change_set(&late.id, false, "").unwrap();
        store.set_base_lifecycle(&base, "paused").unwrap();
        assert!(store
            .propose_entry_archive(&entry, before.revision + 1)
            .is_err());
    }

    #[test]
    fn test_agent_archive_audit_flag_cannot_bypass_current_source_validation() {
        let (store, _dir, base, entry) = setup();
        let before = store.get_entry(&entry).unwrap();
        let mut payload = store.entry_candidate_snapshot(&before).unwrap();
        payload["status"] = serde_json::json!("archived");
        payload["_operation"] = serde_json::json!("archive");
        payload["historical_archive"] = serde_json::json!(true);
        let run = store
            .start_agent_run(
                &base,
                "deepseek_harness",
                "knowledge_qa",
                &serde_json::json!({}),
            )
            .unwrap();
        assert!(store
            .create_semantic_change_set(&base, &run.id, "伪造归档", "", "forged", &[payload])
            .is_err());
        assert_eq!(store.get_entry(&entry).unwrap().entry.status, "stale");
    }
}
