use super::*;
use serde_json::json;

pub(crate) struct PresentationArtifactRevision<'a> {
    pub publication_id: &'a str,
    pub report_run_id: &'a str,
    pub page: usize,
    pub relative_path: &'a str,
    pub hash: &'a str,
    pub size: i64,
    pub validation_message: &'a str,
    pub validation_details: &'a Value,
    pub spec: &'a Value,
}

fn task_report_is_current(
    conn: &Connection,
    task_id: &str,
    report_run_id: &str,
) -> Result<(), BrainError> {
    let base_id = conn
        .query_row(
            "SELECT t.knowledge_base_id FROM knowledge_tasks t
             JOIN knowledge_bases b ON b.id=t.knowledge_base_id
             WHERE t.id=?1 AND t.deliverable_type='presentation'
               AND t.status IN ('completed','failed','cancelled') AND b.lifecycle='active'",
            [task_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            BrainError::KnowledgeConflict("研究任务正在更新或该知识库不可编辑".into())
        })?;
    let (current, _) = research::saved_report_for_presentation(conn, task_id, &base_id)?;
    if current != report_run_id {
        return Err(BrainError::KnowledgeConflict(
            "研究报告已更新，请从当前报告生成演示".into(),
        ));
    }
    Ok(())
}

fn require_editable_parent(
    conn: &Connection,
    parent: &KnowledgeArtifact,
    task_id: &str,
    report_run_id: &str,
) -> Result<(), BrainError> {
    task_report_is_current(conn, task_id, report_run_id)?;
    let current: bool = conn.query_row(
        "SELECT EXISTS (
           SELECT 1 FROM knowledge_artifacts a
           JOIN agent_runs r ON r.id=a.agent_run_id AND r.knowledge_base_id=a.knowledge_base_id
           WHERE a.id=?1 AND a.knowledge_task_id=?2 AND a.content_hash=?3
             AND a.artifact_type='pptx' AND a.validation_state='valid'
             AND r.status='completed' AND json_extract(r.input_json,'$.research_run_id')=?4
         )",
        params![parent.id, task_id, parent.content_hash, report_run_id],
        |row| row.get(0),
    )?;
    if !current {
        return Err(BrainError::KnowledgeConflict(
            "该演示版本已变化，请重新打开大纲".into(),
        ));
    }
    let replaced: bool = conn.query_row(
        "SELECT EXISTS (
           SELECT 1 FROM knowledge_artifacts a
           JOIN agent_runs r ON r.id=a.agent_run_id
           WHERE a.knowledge_task_id=?1 AND r.task_type='knowledge_task_presentation_page_edit'
             AND json_extract(r.input_json,'$.parent_artifact_id')=?2
         )",
        params![task_id, parent.id],
        |row| row.get(0),
    )?;
    if replaced {
        return Err(BrainError::KnowledgeConflict(
            "这份演示已有更新版本，请从更新后的成品继续编辑".into(),
        ));
    }
    Ok(())
}

impl BookWikiStore {
    pub(crate) fn require_current_presentation_report(
        &self,
        task_id: &str,
        report_run_id: &str,
    ) -> Result<(), BrainError> {
        self.db
            .with_connection(|conn| task_report_is_current(conn, task_id, report_run_id))
    }

    pub(crate) fn require_editable_presentation_artifact(
        &self,
        parent: &KnowledgeArtifact,
        task_id: &str,
        report_run_id: &str,
    ) -> Result<(), BrainError> {
        self.db
            .with_connection(|conn| require_editable_parent(conn, parent, task_id, report_run_id))
    }

    pub(crate) fn save_presentation_page_revision(
        &self,
        parent: &KnowledgeArtifact,
        task_id: &str,
        revision: PresentationArtifactRevision<'_>,
    ) -> Result<KnowledgeArtifact, BrainError> {
        let now = Utc::now().to_rfc3339();
        let run_id = uuid::Uuid::new_v4().to_string();
        let input = json!({"knowledge_task_id":task_id,"research_run_id":revision.report_run_id,
            "parent_artifact_id":parent.id,"page":revision.page});
        let output = json!({"presentation_spec":revision.spec,"billing_usage":"not_applicable"});
        self.db.transaction(|conn| {
            require_editable_parent(conn, parent, task_id, revision.report_run_id)?;
            conn.execute(
                "INSERT INTO agent_runs
                 (id,knowledge_base_id,runtime,task_type,status,input_json,output_json,started_at,finished_at,created_at,usage_source)
                 VALUES (?1,?2,'application','knowledge_task_presentation_page_edit','completed',?3,?4,?5,?5,?5,'unavailable')",
                params![run_id, parent.knowledge_base_id, input.to_string(), output.to_string(), now],
            )?;
            conn.execute(
                "INSERT INTO knowledge_artifacts
                 (id,knowledge_base_id,knowledge_task_id,agent_run_id,skill_id,artifact_type,title,
                  relative_path,mime_type,content_hash,size_bytes,validation_state,validation_message,
                  validation_details_json,created_at)
                 SELECT ?2, a.knowledge_base_id, a.knowledge_task_id, ?3, a.skill_id, a.artifact_type,
                        t.title || ' · 演示文稿 · 第 ' || ?4 || ' 页修订', ?5, a.mime_type, ?6, ?7,
                        'valid', ?8, ?9, ?10
                 FROM knowledge_artifacts a JOIN knowledge_tasks t ON t.id=a.knowledge_task_id
                 WHERE a.id=?1",
                params![parent.id, revision.publication_id, run_id, revision.page, revision.relative_path,
                        revision.hash, revision.size, revision.validation_message,
                        revision.validation_details.to_string(), now],
            )?;
            conn.execute(
                "INSERT INTO knowledge_artifact_citations
                 (artifact_id,ordinal,entry_id,entry_revision,title_snapshot,source_path_snapshot)
                 SELECT ?2,ordinal,entry_id,entry_revision,title_snapshot,source_path_snapshot
                 FROM knowledge_artifact_citations WHERE artifact_id=?1",
                params![parent.id,revision.publication_id],
            )?;
            insert_agent_run_event(
                conn,
                &run_id,
                "run.completed",
                Some("completed"),
                &format!("已保存第 {} 页的手动修订", revision.page),
                &json!({"artifact_id": revision.publication_id, "parent_artifact_id": parent.id}).to_string(),
                &now,
            )?;
            conn.query_row(
                "SELECT id,knowledge_base_id,knowledge_task_id,agent_run_id,skill_id,artifact_type,
                        title,relative_path,mime_type,content_hash,size_bytes,validation_state,
                        validation_message,validation_details_json,created_at
                 FROM knowledge_artifacts WHERE id=?1",
                [revision.publication_id],
                map_artifact,
            ).map_err(Into::into)
        })
    }
}
