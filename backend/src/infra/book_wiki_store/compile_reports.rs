//! Durable, paged analysis coverage; deliberately not a semantic completeness score.
use super::BookWikiStore;
use crate::error::BrainError;
use crate::models::book_wiki::{
    KnowledgeCompileFragment, KnowledgeCompileReport, SourceSpanSnapshot,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::HashSet;

fn invalid(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

fn require_running(conn: &Connection, id: &str) -> Result<String, BrainError> {
    let value: Option<(String, String)> = conn
        .query_row(
            "SELECT knowledge_base_id,status FROM knowledge_compile_reports WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match value {
        Some((base, status)) if status == "running" => Ok(base),
        _ => Err(invalid("该编译报告已结束或不存在，不能覆盖历史结果")),
    }
}

fn insert_fragments<'a>(
    conn: &Connection,
    id: &str,
    fragments: impl Iterator<Item = (usize, &'a SourceSpanSnapshot)>,
) -> Result<(), BrainError> {
    let mut statement = conn.prepare("INSERT INTO knowledge_compile_report_fragments
        (report_id,ordinal,batch,source_document_id,source_version_id,source_span_id,source_path,line_start,line_end,locator_json,status)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'unprocessed')")?;
    for (ordinal, (batch, span)) in fragments.enumerate() {
        statement.execute(params![
            id,
            ordinal as i64,
            batch as i64,
            span.source_document_id,
            span.source_version_id,
            span.id,
            span.source_path,
            span.line_start,
            span.line_end,
            span.locator.to_string()
        ])?;
    }
    Ok(())
}

impl BookWikiStore {
    pub(crate) fn start_compile_report(
        &self,
        base_id: &str,
        fingerprint: &str,
        spans: &[SourceSpanSnapshot],
    ) -> Result<String, BrainError> {
        self.get_active_base(base_id)?;
        if spans.is_empty() {
            return Err(invalid("编译覆盖报告不能以空来源开始"));
        }
        let id = format!("compile-report-{}", uuid::Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            // Scope and version validation happens before any foreign metadata is persisted.
            for span in spans {
                let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM source_spans ss JOIN source_versions sv ON sv.id=ss.source_version_id JOIN source_documents sd ON sd.id=sv.source_document_id WHERE ss.id=?1 AND ss.knowledge_base_id=?2 AND ss.source_version_id=?3 AND sv.source_document_id=?4 AND sd.current_version_id=sv.id AND sd.sync_status='current')",params![span.id,base_id,span.source_version_id,span.source_document_id],|row|row.get(0))?;
                if !valid { return Err(invalid("编译报告来源必须属于本书当前同步版本")); }
            }
            let current:i64=conn.query_row("SELECT COUNT(*) FROM source_documents WHERE knowledge_base_id=?1 AND sync_status='current'",[base_id],|row|row.get(0))?;
            conn.execute("INSERT INTO knowledge_compile_reports(id,knowledge_base_id,fingerprint,status,selected_sources,current_sources,selected_spans,created_at,updated_at) VALUES(?1,?2,?3,'running',?4,?5,?6,?7,?7)",params![id,base_id,fingerprint,spans.iter().map(|s|&s.source_document_id).collect::<HashSet<_>>().len() as i64,current,spans.len() as i64,now])?;
            insert_fragments(conn,&id,spans.iter().map(|span|(1,span)))?;
            Ok(())
        })?;
        Ok(id)
    }

    pub(crate) fn plan_compile_report(
        &self,
        id: &str,
        batches: &[Vec<SourceSpanSnapshot>],
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            require_running(conn,id)?;
            let begun:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_compile_report_fragments WHERE report_id=?1 AND status<>'unprocessed')",[id],|row|row.get(0))?;
            if begun { return Err(invalid("分析开始后不能重置覆盖计划")); }
            let planned_ids=batches.iter().flatten().map(|s|(s.id.as_str(),s.source_document_id.as_str(),s.source_version_id.as_str())).collect::<HashSet<_>>();
            let original_ids = {
                let mut statement=conn.prepare("SELECT DISTINCT source_span_id,source_document_id,source_version_id FROM knowledge_compile_report_fragments WHERE report_id=?1")?;
                let rows=statement.query_map([id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?;
                rows.collect::<Result<HashSet<_>,_>>()?
            };
            if planned_ids != original_ids.iter().map(|(span,doc,version)|(span.as_str(),doc.as_str(),version.as_str())).collect::<HashSet<_>>() { return Err(invalid("覆盖计划必须保留全部选定来源及其原版本")); }
            conn.execute("DELETE FROM knowledge_compile_report_fragments WHERE report_id=?1",[id])?;
            insert_fragments(conn,id,batches.iter().enumerate().flat_map(|(index,batch)|batch.iter().map(move |span|(index+1,span))))?;
            conn.execute("UPDATE knowledge_compile_reports SET planned=1,updated_at=?2 WHERE id=?1",params![id,Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub(crate) fn begin_compile_report_batch(
        &self,
        id: &str,
        batch: i64,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            require_running(conn,id)?;
            let count=conn.execute("UPDATE knowledge_compile_report_fragments SET status='analyzing' WHERE report_id=?1 AND batch=?2 AND status='unprocessed'",params![id,batch])?;
            if count==0 { return Err(invalid("该来源批次不存在或已处理")); }
            Ok(())
        })
    }

    pub(crate) fn complete_compile_report_batch(
        &self,
        id: &str,
        batch: i64,
        run_id: &str,
        candidates: &[Value],
        no_material_reason: Option<&str>,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            let base=require_running(conn,id)?;
            let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=?1 AND knowledge_base_id=?2)",params![run_id,base],|row|row.get(0))?;
            if !valid { return Err(invalid("批次运行不属于报告知识库")); }
            let rows = {
                let mut stmt=conn.prepare("SELECT ordinal,source_span_id FROM knowledge_compile_report_fragments WHERE report_id=?1 AND batch=?2 AND status='analyzing'")?;
                let values=stmt.query_map(params![id,batch],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?)))?;
                values.collect::<Result<Vec<_>,_>>()?
            };
            if rows.is_empty() { return Err(invalid("批次尚未开始或已经记录")); }
            if candidates.is_empty() && no_material_reason.is_none_or(|v|v.trim().is_empty()) { return Err(invalid("无实质结果必须保留具体原因")); }
            for (ordinal,span) in rows {
                let slugs=candidates.iter().filter(|candidate|candidate["citations"].as_array().is_some_and(|ids|ids.iter().any(|id|id.as_str()==Some(span.as_str())))).filter_map(|c|c["slug"].as_str()).collect::<Vec<_>>();
                conn.execute("UPDATE knowledge_compile_report_fragments SET status=?3,run_id=?4,candidate_slugs_json=?5,reason=?6 WHERE report_id=?1 AND ordinal=?2",params![id,ordinal,if candidates.is_empty(){"no_material"}else{"analyzed"},run_id,serde_json::json!(slugs).to_string(),no_material_reason])?;
            }
            conn.execute("UPDATE knowledge_compile_reports SET updated_at=?2 WHERE id=?1",params![id,Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub(crate) fn finish_compile_report(
        &self,
        id: &str,
        change_set_id: &str,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            let base=require_running(conn,id)?;
            let no_material:bool=conn.query_row("SELECT COALESCE(json_extract(classification_summary_json,'$.no_material'),0)>0 FROM knowledge_change_sets WHERE id=?1 AND knowledge_base_id=?2",params![change_set_id,base],|row|row.get(0))?;
            let unfinished:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_compile_report_fragments WHERE report_id=?1 AND status NOT IN ('analyzed','no_material'))",[id],|row|row.get(0))?;
            if unfinished { return Err(invalid("仍有来源未完成分析，不能标记编译报告完成")); }
            conn.execute("UPDATE knowledge_compile_reports SET status=?2,change_set_id=?3,updated_at=?4 WHERE id=?1",params![id,if no_material{"no_material"}else{"waiting_review"},change_set_id,Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub(crate) fn record_compile_report_topic(
        &self,
        base_id: &str,
        outcome: &Value,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            let id:Option<String>=conn.query_row("SELECT id FROM knowledge_compile_reports WHERE knowledge_base_id=?1 AND status='running'",[base_id],|row|row.get(0)).optional()?;
            let id=id.ok_or_else(||invalid("没有可记录主题结果的编译报告"))?;
            conn.execute("INSERT INTO knowledge_compile_report_topics(report_id,ordinal,outcome_json) SELECT ?1,COALESCE(MAX(ordinal)+1,0),?2 FROM knowledge_compile_report_topics WHERE report_id=?1",params![id,outcome.to_string()])?;
            Ok(())
        })
    }

    pub(crate) fn fail_compile_reports(
        &self,
        base_id: &str,
        error: &str,
        cancelled: bool,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            conn.execute("UPDATE knowledge_compile_report_fragments SET status='failed',reason=?2 WHERE status='analyzing' AND report_id IN (SELECT id FROM knowledge_compile_reports WHERE knowledge_base_id=?1 AND status='running')",params![base_id,error.chars().take(2000).collect::<String>()])?;
            conn.execute("UPDATE knowledge_compile_reports SET status=?2,error=?3,updated_at=?4 WHERE knowledge_base_id=?1 AND status='running'",params![base_id,if cancelled{"cancelled"}else{"failed"},error.chars().take(2000).collect::<String>(),Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    pub fn get_compile_report(
        &self,
        base_id: &str,
        id: Option<&str>,
        fragment_offset: usize,
        topic_offset: usize,
        limit: usize,
    ) -> Result<Option<KnowledgeCompileReport>, BrainError> {
        self.get_base(base_id)?;
        let limit = limit.clamp(1, 100);
        let fragment_sql_offset =
            i64::try_from(fragment_offset).map_err(|_| invalid("来源分页偏移超出范围"))?;
        let topic_sql_offset =
            i64::try_from(topic_offset).map_err(|_| invalid("主题分页偏移超出范围"))?;
        self.db.with_connection(|conn| {
            let found=conn.query_row("SELECT r.id, CASE WHEN r.status='waiting_review' AND c.status IS NOT NULL AND c.status<>'proposed' THEN c.status ELSE r.status END, r.created_at,r.updated_at,r.selected_sources,r.current_sources,r.selected_spans,r.planned,r.change_set_id,r.error
                FROM knowledge_compile_reports r LEFT JOIN knowledge_change_sets c ON c.id=r.change_set_id WHERE r.knowledge_base_id=?1 AND (?2 IS NULL OR r.id=?2) ORDER BY r.created_at DESC,r.id DESC LIMIT 1",params![base_id,id],|row|Ok(KnowledgeCompileReport{id:row.get(0)?,knowledge_base_id:base_id.into(),status:row.get(1)?,created_at:row.get(2)?,updated_at:row.get(3)?,selected_sources:row.get(4)?,current_sources:row.get(5)?,selected_spans:row.get(6)?,planned:row.get(7)?,change_set_id:row.get(8)?,error:row.get(9)?,fragment_total:0,analyzed_fragments:0,no_material_fragments:0,failed_fragments:0,unprocessed_fragments:0,fragment_offset,fragment_has_more:false,fragments:vec![],topic_total:0,topic_offset,topic_has_more:false,topics:vec![],previous_report_id:None,next_report_id:None})).optional()?;
            let Some(mut report)=found else { return if id.is_some(){Err(invalid("编译报告不存在或不属于本书"))}else{Ok(None)}; };
            report.previous_report_id=conn.query_row("SELECT id FROM knowledge_compile_reports WHERE knowledge_base_id=?1 AND (created_at,id)<(?2,?3) ORDER BY created_at DESC,id DESC LIMIT 1",params![base_id,report.created_at,report.id],|row|row.get(0)).optional()?;
            report.next_report_id=conn.query_row("SELECT id FROM knowledge_compile_reports WHERE knowledge_base_id=?1 AND (created_at,id)>(?2,?3) ORDER BY created_at,id LIMIT 1",params![base_id,report.created_at,report.id],|row|row.get(0)).optional()?;
            let (total,analyzed,no_material,failed,unprocessed):(i64,i64,i64,i64,i64)=conn.query_row("SELECT COUNT(*),COALESCE(SUM(status IN ('analyzed','no_material')),0),COALESCE(SUM(status='no_material'),0),COALESCE(SUM(status='failed'),0),COALESCE(SUM(status IN ('unprocessed','analyzing')),0) FROM knowledge_compile_report_fragments WHERE report_id=?1",[&report.id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)))?;
            report.fragment_total=total;report.analyzed_fragments=analyzed;report.no_material_fragments=no_material;report.failed_fragments=failed;report.unprocessed_fragments=unprocessed;
            let mut stmt=conn.prepare("SELECT ordinal,batch,source_document_id,source_version_id,source_span_id,source_path,line_start,line_end,locator_json,status,run_id,candidate_slugs_json,reason FROM knowledge_compile_report_fragments WHERE report_id=?1 ORDER BY ordinal LIMIT ?2 OFFSET ?3")?;
            let rows=stmt.query_map(params![report.id,limit as i64,fragment_sql_offset],|row|Ok(KnowledgeCompileFragment{ordinal:row.get(0)?,batch:row.get(1)?,source_document_id:row.get(2)?,source_version_id:row.get(3)?,source_span_id:row.get(4)?,source_path:row.get(5)?,line_start:row.get(6)?,line_end:row.get(7)?,locator:super::source_locator_from_row(row,8)?,status:row.get(9)?,run_id:row.get(10)?,candidate_slugs:serde_json::from_str(&row.get::<_,String>(11)?).map_err(|error|rusqlite::Error::FromSqlConversionFailure(11,rusqlite::types::Type::Text,Box::new(error)))?,reason:row.get(12)?}))?;
            report.fragments=rows.collect::<Result<Vec<_>,_>>()?;
            report.fragment_has_more=fragment_offset.saturating_add(report.fragments.len())<(total as usize);
            report.topic_total=conn.query_row("SELECT COUNT(*) FROM knowledge_compile_report_topics WHERE report_id=?1",[&report.id],|row|row.get(0))?;
            let mut stmt=conn.prepare("SELECT outcome_json FROM knowledge_compile_report_topics WHERE report_id=?1 ORDER BY ordinal LIMIT ?2 OFFSET ?3")?;
            let rows=stmt.query_map(params![report.id,limit as i64,topic_sql_offset],|row|row.get::<_,String>(0))?;
            for row in rows { report.topics.push(serde_json::from_str(&row?).map_err(|error|BrainError::Internal(format!("编译主题报告读取失败: {error}")))?); }
            report.topic_has_more=topic_offset.saturating_add(report.topics.len())<(report.topic_total as usize);
            Ok(Some(report))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{sample_book, sample_source, test_store};
    use serde_json::json;

    #[test]
    fn test_compile_report_no_material_and_topic_history_remain_honest() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("no-material-report", "/tmp/report")])
            .unwrap();
        let base = store.initialize_base("no-material-report").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("no-material", "Report", "span-no-material")],
            )
            .unwrap();
        let spans = store.list_current_source_spans(&base.id).unwrap();
        store.begin_semantic_compile(&base.id, 1).unwrap();
        let id = store.start_compile_report(&base.id, "fp", &spans).unwrap();
        store.plan_compile_report(&id, &[spans.clone()]).unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_ingest", &json!({}))
            .unwrap();
        assert!(store.finish_compile_report(&id, "missing").is_err());
        store.begin_compile_report_batch(&id, 1).unwrap();
        for index in 0..121 {
            store.record_compile_report_topic(&base.id,&json!({"slug":format!("topic-{index}"),"outcome":"excluded_by_archive","reason":"保留用户归档"})).unwrap();
        }
        store
            .complete_compile_report_batch(&id, 1, &run.id, &[], Some("纯导航"))
            .unwrap();
        let changes = store
            .create_no_material_change_set(
                &base.id,
                &run.id,
                "无新增",
                "纯导航",
                "report-no-material",
            )
            .unwrap();
        store.finish_compile_report(&id, &changes.id).unwrap();
        let report = store
            .get_compile_report(&base.id, Some(&id), 0, 100, 100)
            .unwrap()
            .unwrap();
        assert_eq!(report.status, "no_material");
        assert_eq!(report.topic_total, 121);
        assert_eq!(report.topics.len(), 21);
        assert!(!report.topic_has_more);
        assert_eq!(report.topics[0]["slug"], "topic-100");
        assert_eq!(report.change_set_id.as_deref(), Some(changes.id.as_str()));
        assert_eq!(report.selected_sources, 1);
        assert_eq!(report.current_sources, 1);
        assert!(store
            .record_compile_report_topic(&base.id, &json!({}))
            .is_err());
        let next = store
            .start_compile_report(&base.id, "next", &spans)
            .unwrap();
        store.begin_compile_report_batch(&next, 1).unwrap();
        store.recover_interrupted_tasks().unwrap();
        let interrupted = store
            .get_compile_report(&base.id, None, 0, 0, 50)
            .unwrap()
            .unwrap();
        assert_eq!(interrupted.id, next);
        assert_eq!(interrupted.status, "failed");
        assert_eq!(interrupted.failed_fragments, 1);
        let old = store
            .get_compile_report(&base.id, Some(&id), 0, 0, 50)
            .unwrap()
            .unwrap();
        assert_eq!(old.status, "no_material");
        assert_eq!(old.topic_total, 121);
        assert_eq!(old.next_report_id.as_deref(), Some(next.as_str()));
        assert_eq!(interrupted.previous_report_id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn test_compile_report_tracks_review_without_claiming_current_freshness() {
        use super::super::tests::semantic_candidate;
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[sample_book("review-report", "/tmp/review")])
            .unwrap();
        let base = store.initialize_base("review-report").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("review-report", "Review", "span-review")],
            )
            .unwrap();
        let spans = store.list_current_source_spans(&base.id).unwrap();
        store.begin_semantic_compile(&base.id, 1).unwrap();
        let id = store.start_compile_report(&base.id, "fp", &spans).unwrap();
        store.plan_compile_report(&id, &[spans.clone()]).unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_ingest", &json!({}))
            .unwrap();
        let candidate = semantic_candidate("span-review", "Review topic");
        store.begin_compile_report_batch(&id, 1).unwrap();
        store
            .complete_compile_report_batch(&id, 1, &run.id, &[candidate.clone()], None)
            .unwrap();
        let changes = store
            .create_semantic_change_set(
                &base.id,
                &run.id,
                "候选",
                "候选",
                "report-review",
                &[candidate],
            )
            .unwrap();
        store.finish_compile_report(&id, &changes.id).unwrap();
        assert_eq!(
            store
                .get_compile_report(&base.id, None, 0, 0, 50)
                .unwrap()
                .unwrap()
                .status,
            "waiting_review"
        );
        store.resolve_change_set(&changes.id, true, "确认").unwrap();
        assert_eq!(
            store
                .get_compile_report(&base.id, None, 0, 0, 50)
                .unwrap()
                .unwrap()
                .status,
            "applied"
        );
        store.sync_markdown_sources(&base.id, &[]).unwrap();
        let historical = store
            .get_compile_report(&base.id, Some(&id), 0, 0, 50)
            .unwrap()
            .unwrap();
        assert_eq!(historical.status, "applied");
        assert_eq!(historical.current_sources, 1); // Historical selection, not a current health claim.
        assert_eq!(
            historical.fragments[0].source_version_id,
            spans[0].source_version_id
        );
        assert!(!serde_json::to_string(&historical)
            .unwrap()
            .contains("content_md"));
    }

    #[test]
    fn test_compile_report_retains_partial_analysis_and_pages_all_fragments() {
        let (store, _dir) = test_store();
        store
            .save_reader_books(&[
                sample_book("report", "/tmp/report"),
                sample_book("foreign", "/tmp/foreign"),
            ])
            .unwrap();
        let base = store.initialize_base("report").unwrap();
        let foreign = store.initialize_base("foreign").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[sample_source("report", "Report", "span-report")],
            )
            .unwrap();
        let spans = store.list_current_source_spans(&base.id).unwrap();
        store.begin_semantic_compile(&base.id, 1).unwrap();
        let id = store
            .start_compile_report(&base.id, "fingerprint", &spans)
            .unwrap();
        assert!(store
            .start_compile_report(&foreign.id, "fp", &spans)
            .is_err());
        let mut foreign_version = spans[0].clone();
        foreign_version.source_version_id = "another-version".into();
        assert!(store
            .plan_compile_report(&id, &[vec![foreign_version]])
            .is_err());
        let batches = vec![
            vec![spans[0].clone(); 100],
            vec![spans[0].clone(); 10],
            vec![spans[0].clone()],
        ];
        store.plan_compile_report(&id, &batches).unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_ingest", &json!({}))
            .unwrap();
        store.begin_compile_report_batch(&id, 1).unwrap();
        store
            .complete_compile_report_batch(&id, 1, &run.id, &[], Some("纯导航，没有新增事实"))
            .unwrap();
        store.begin_compile_report_batch(&id, 2).unwrap();
        store
            .fail_compile_reports(&base.id, "第二批失败", false)
            .unwrap();
        let first = store
            .get_compile_report(&base.id, Some(&id), 0, 0, 100)
            .unwrap()
            .unwrap();
        assert_eq!(first.fragment_total, 111);
        assert_eq!(first.analyzed_fragments, 100);
        assert_eq!(first.no_material_fragments, 100);
        assert_eq!(first.failed_fragments, 10);
        assert_eq!(first.unprocessed_fragments, 1);
        assert_eq!(first.fragments.len(), 100);
        assert_eq!(
            first.fragments[0].reason.as_deref(),
            Some("纯导航，没有新增事实")
        );
        assert_eq!(first.status, "failed");
        assert!(first.fragment_has_more);
        let tail = store
            .get_compile_report(&base.id, Some(&id), 100, 0, 100)
            .unwrap()
            .unwrap();
        assert_eq!(tail.fragments.len(), 11);
        assert!(!tail.fragment_has_more);
        assert!(store
            .get_compile_report(&foreign.id, Some(&id), 0, 0, 50)
            .is_err());
        assert!(store
            .get_compile_report(&foreign.id, None, 0, 0, 50)
            .unwrap()
            .is_none());
        assert!(store.begin_compile_report_batch(&id, 3).is_err());
    }
}
