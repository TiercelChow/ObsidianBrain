//! Durable business deliverables and lease-fenced stage checkpoints, not CoT.

use super::{insert_agent_run_event, stable_id, BookWikiStore};
use crate::error::BrainError;
use crate::models::book_wiki::{
    ResearchBaselineSummary, ResearchBrief, ResearchEvidenceReference, ResearchFinding,
    ResearchPlan, ResearchSectionOutput, ResearchStageContent, ResearchStageSummary,
    ResearchSynthesisOutput, ResearchWorkspace,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// A stage completion must still own this exact attempt and claim. A stale
/// worker cannot save after lease recovery even if its model eventually returns.
#[derive(Clone, Debug)]
pub(crate) struct ResearchStageClaim {
    pub task_id: String,
    pub stage_key: String,
    pub claim_id: String,
    pub attempt: i64,
}

fn invalid(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}
fn conflict(message: &str) -> BrainError {
    BrainError::KnowledgeConflict(message.into())
}
fn encode(value: &impl Serialize) -> Result<String, BrainError> {
    serde_json::to_string(value)
        .map_err(|e| BrainError::Internal(format!("研究状态序列化失败: {e}")))
}
fn decode<T: DeserializeOwned>(text: &str) -> Result<T, BrainError> {
    serde_json::from_str(text).map_err(|e| BrainError::Internal(format!("研究状态损坏: {e}")))
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
fn bounded_list(values: &[String], count: usize, length: usize) -> bool {
    values.len() <= count && values.iter().all(|value| text(value, length))
}

fn pages_cover_scope(pages: &[Value], scope: Option<&str>) -> bool {
    let mut versions: HashMap<(String, u64), Vec<(u64, u64)>> = HashMap::new();
    for page in pages {
        let page_scope = match page.get("question_id") {
            Some(Value::Null) => None,
            Some(Value::String(id)) => Some(id.as_str()),
            _ => continue,
        };
        if page_scope != scope {
            continue;
        }
        let (Some(hash), Some(start), Some(length), Some(total)) = (
            page["manifest_hash"].as_str(),
            page["offset_chars"].as_u64(),
            page["returned_chars"].as_u64(),
            page["total_chars"].as_u64(),
        ) else {
            continue;
        };
        let Some(end) = start.checked_add(length) else {
            continue;
        };
        if hash.is_empty() || length == 0 || total == 0 || end > total {
            continue;
        }
        versions
            .entry((hash.to_owned(), total))
            .or_default()
            .push((start, end));
    }
    versions.into_iter().any(|((_, total), mut ranges)| {
        ranges.sort_unstable();
        let mut covered = 0;
        for (start, end) in ranges {
            if start > covered {
                break;
            }
            covered = covered.max(end);
        }
        covered >= total
    })
}

fn manifest_pages_cover(pages: &[Value], question_id: &str) -> bool {
    pages_cover_scope(pages, Some(question_id)) || pages_cover_scope(pages, None)
}

fn read_manifest_page_events(conn: &Connection, run: &str) -> Result<Vec<Value>, BrainError> {
    let mut stmt = conn.prepare(
        "SELECT payload_json FROM agent_run_events
         WHERE run_id=?1 AND event_type='run.research_manifest_page' ORDER BY sequence",
    )?;
    let pages = stmt
        .query_map([run], |row| row.get::<_, String>(0))?
        .map(|row| decode(&row?))
        .collect();
    pages
}

/// Keep the saved stage text intact; only suppress exact duplication introduced
/// by the report wrapper. Its summary remains available in stage metadata.
fn append_report_section(body: &mut String, title: &str, summary: &str, content: &str) {
    let content = content.trim_matches('\n');
    let (first_line, rest) = content.split_once('\n').unwrap_or((content, ""));
    let heading = first_line.trim();
    let level = heading.bytes().take_while(|byte| *byte == b'#').count();
    let repeats_title = (1..=6).contains(&level)
        && heading
            .as_bytes()
            .get(level)
            .is_some_and(u8::is_ascii_whitespace)
        && heading[level..].trim().trim_end_matches('#').trim() == title.trim();
    let content = if repeats_title && !rest.trim().is_empty() {
        rest.trim_start_matches('\n')
    } else {
        content
    };
    body.push_str(&format!("## {title}\n\n"));
    let summary = summary.trim();
    if !summary.is_empty() && !content.contains(summary) {
        body.push_str(summary);
        body.push_str("\n\n");
    }
    body.push_str(content);
    body.push_str("\n\n");
}

fn append_report_context(body: &mut String, plan: &ResearchPlan, after_executive_summary: bool) {
    if plan.constraints.is_empty() && plan.terminology.is_empty() {
        return;
    }
    if after_executive_summary {
        body.push_str("### 研究边界\n\n");
        if !plan.constraints.is_empty() {
            body.push_str(&format!(
                "- **范围与约束**：{}\n",
                plan.constraints.join("；")
            ));
        }
        if !plan.terminology.is_empty() {
            body.push_str(&format!(
                "- **术语口径**：{}\n",
                plan.terminology.join("；")
            ));
        }
        body.push('\n');
    } else {
        if !plan.constraints.is_empty() {
            body.push_str(&format!(
                "研究范围与约束：{}\n\n",
                plan.constraints.join("；")
            ));
        }
        if !plan.terminology.is_empty() {
            body.push_str(&format!("术语口径：{}\n\n", plan.terminology.join("；")));
        }
    }
}

pub(crate) fn validate_plan(plan: &ResearchPlan) -> Result<(), BrainError> {
    if !text(&plan.goal, 4000)
        || plan.report_title.as_ref().is_some_and(|title| {
            !text(title, 160) || title.contains(['\n', '\r']) || title.trim_start().starts_with('#')
        })
        || !matches!(plan.depth.as_str(), "brief" | "standard" | "deep")
        || !bounded_list(&plan.constraints, 32, 1000)
        || !bounded_list(&plan.acceptance, 32, 1000)
        || plan.acceptance.is_empty()
        || !bounded_list(&plan.terminology, 64, 1000)
        || plan.questions.is_empty()
        || plan.questions.len() > 24
    {
        return Err(invalid(
            "研究规划必须有目标、有效深度及1至24个明确子问题；文本/列表不能超过安全边界",
        ));
    }
    let mut ids = HashSet::new();
    for q in &plan.questions {
        if !text(&q.id, 80)
            || q.id == "_synthesis"
            || !q
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(&q.id)
            || !text(&q.title, 500)
            || !text(&q.question, 2000)
            || !bounded_list(&q.required_evidence, 16, 1000)
            || q.required_evidence.is_empty()
            || q.expected_output_tokens
                .is_some_and(|value| value == 0 || value > 262144)
            || !bounded_list(&q.target_entry_ids, 64, 200)
            || q.target_entry_ids.iter().collect::<HashSet<_>>().len() != q.target_entry_ids.len()
        {
            return Err(invalid(
                "研究子问题身份不能重复，必须有明确的问题、标题和合法证据要求",
            ));
        }
    }
    Ok(())
}

fn live_task(
    conn: &Connection,
    task: &str,
    allow_cancel: bool,
) -> Result<(String, i64), BrainError> {
    conn.query_row("SELECT t.knowledge_base_id,t.research_execution_epoch FROM knowledge_tasks t JOIN knowledge_bases b ON b.id=t.knowledge_base_id
        WHERE t.id=?1 AND t.status='running' AND (?2 OR t.cancel_requested=0) AND b.lifecycle='active'
        AND julianday(t.lease_expires_at)>julianday('now')",params![task,allow_cancel],|row|Ok((row.get(0)?,row.get(1)?))).optional()?.ok_or_else(||conflict("研究任务已取消、结束、暂停或租约已失效，拒绝迟到阶段写入"))
}

fn owned_stage(
    conn: &Connection,
    claim: &ResearchStageClaim,
    allow_cancel: bool,
) -> Result<(String, String), BrainError> {
    let (base, attempt) = live_task(conn, &claim.task_id, allow_cancel)?;
    if attempt != claim.attempt {
        return Err(conflict("该研究阶段属于旧任务尝试，不能覆盖新执行器"));
    }
    let kind=conn.query_row("SELECT kind FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2 AND claim_id=?3 AND claimed_attempt=?4 AND status='running'",params![claim.task_id,claim.stage_key,claim.claim_id,claim.attempt],|row|row.get(0)).optional()?.ok_or_else(||conflict("研究阶段已完成或被重新领取，不能覆盖检查点"))?;
    Ok((base, kind))
}

/// Shared artifact persistence stays compatible with legacy tasks, but a
/// staged research worker must still own the exact presentation attempt.
pub(super) fn validate_artifact_write(
    conn: &Connection,
    task: &str,
    run: &str,
) -> Result<(), BrainError> {
    let staged: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_research_workspaces WHERE task_id=?1)",
        [task],
        |row| row.get(0),
    )?;
    if !staged {
        return Ok(());
    }
    let raw: String = conn.query_row(
        "SELECT input_json FROM agent_runs WHERE id=?1",
        [run],
        |row| row.get(0),
    )?;
    let input: Value = decode(&raw)?;
    let claim = ResearchStageClaim {
        task_id: task.into(),
        stage_key: "presentation".into(),
        claim_id: input["research_claim_id"]
            .as_str()
            .unwrap_or_default()
            .into(),
        attempt: input["research_claim_attempt"].as_i64().unwrap_or(-1),
    };
    let (base, kind) = owned_stage(conn, &claim, false)?;
    if kind != "presentation" {
        return Err(conflict("成果写入未获得当前演示阶段授权"));
    }
    check_run(conn, &claim, run, true)?;
    let report = read_content(conn, task, "report")?;
    for reference in &report.evidence {
        if !current_reference(conn, &base, reference)? {
            return Err(conflict("生成期间报告依据变化，保留报告但不提交过期演示"));
        }
    }
    Ok(())
}

fn check_run(
    conn: &Connection,
    claim: &ResearchStageClaim,
    run: &str,
    completed: bool,
) -> Result<(), BrainError> {
    let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs r JOIN knowledge_tasks t ON t.id=?2 JOIN knowledge_research_stages s ON s.task_id=t.id AND s.stage_key=?3 WHERE r.id=?1 AND r.knowledge_base_id=t.knowledge_base_id
        AND r.task_type LIKE 'knowledge_task_%' AND r.status=?5
        AND (?5<>'completed' OR s.run_id=r.id)
        AND json_extract(r.input_json,'$.knowledge_task_id')=?2 AND json_extract(r.input_json,'$.research_stage_key')=?3
        AND json_extract(r.input_json,'$.research_claim_id')=?4)",params![run,claim.task_id,claim.stage_key,claim.claim_id,if completed {"completed"} else {"running"}],|row|row.get(0))?;
    if !valid {
        return Err(conflict("阶段运行身份、任务范围或完成状态不匹配"));
    }
    Ok(())
}

pub(super) fn validate_run_lease(conn: &Connection, run: &str) -> Result<(), BrainError> {
    let raw: String = conn.query_row(
        "SELECT input_json FROM agent_runs WHERE id=?1",
        [run],
        |row| row.get(0),
    )?;
    let input: Value = decode(&raw)?;
    let Some(key) = input["research_stage_key"].as_str() else {
        return Ok(());
    };
    let claim = ResearchStageClaim {
        task_id: input["knowledge_task_id"]
            .as_str()
            .unwrap_or_default()
            .into(),
        stage_key: key.into(),
        claim_id: input["research_claim_id"]
            .as_str()
            .unwrap_or_default()
            .into(),
        attempt: input["research_claim_attempt"].as_i64().unwrap_or(-1),
    };
    owned_stage(conn, &claim, false)?;
    check_run(conn, &claim, run, false)
}

/// Candidate creation is a write even though it still requires human review.
/// Fence it in the same transaction, not through a prior non-atomic check.
pub(super) fn validate_report_candidate_write(
    conn: &Connection,
    run: &str,
) -> Result<Option<String>, BrainError> {
    let raw: String = conn.query_row(
        "SELECT input_json FROM agent_runs WHERE id=?1",
        [run],
        |row| row.get(0),
    )?;
    let input: Value = decode(&raw)?;
    if input["research_stage_key"].as_str() != Some("report") {
        return Ok(None);
    }
    let task = input["knowledge_task_id"]
        .as_str()
        .ok_or_else(|| invalid("报告缺少研究任务身份"))?;
    let (base, epoch) = live_task(conn, task, false)?;
    if input["research_claim_attempt"].as_i64() != Some(epoch) {
        return Err(invalid("旧研究报告执行器不能向新尝试写入知识候选"));
    }
    let report = read_content(conn, task, "report")?;
    if report.stage.status != "completed" || report.content_run_id.as_deref() != Some(run) {
        return Err(invalid("当前完整报告尚未保存，不能创建回写候选"));
    }
    for reference in &report.evidence {
        if !current_reference(conn, &base, reference)? {
            return Err(invalid("研究报告依据已变化，不能提交过期候选"));
        }
    }
    Ok(Some(task.to_string()))
}

fn stage_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchStageSummary> {
    Ok(ResearchStageSummary {
        stage_key: row.get(0)?,
        title: row.get(1)?,
        kind: row.get(2)?,
        ordinal: row.get(3)?,
        status: row.get(4)?,
        revision: row.get(5)?,
        run_id: row.get(6)?,
        summary: row.get(7)?,
        content_characters: row.get(8)?,
        finding_count: row.get(9)?,
        error: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
const SUMMARY_COLUMNS:&str="stage_key,title,kind,ordinal,status,revision,run_id,summary,length(content_md),json_array_length(findings_json),error,updated_at";

fn body_indices(content: &str) -> Result<HashSet<usize>, BrainError> {
    let mut result = HashSet::new();
    for suffix in content.split("[S").skip(1) {
        let digits = suffix.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && suffix.as_bytes().get(digits) == Some(&b']') {
            let index = suffix[..digits]
                .parse::<usize>()
                .map_err(|_| invalid("章节引用编号超出范围"))?;
            if index == 0 {
                return Err(invalid("章节引用必须从S1开始"));
            }
            result.insert(index);
        }
    }
    Ok(result)
}

fn neutral_section_text(content: &str) -> Result<String, BrainError> {
    let mut result = String::with_capacity(content.len());
    let mut remainder = content;
    while let Some((before, after)) = remainder.split_once("[S") {
        result.push_str(before);
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after.as_bytes().get(digits) == Some(&b']') {
            let old = after[..digits]
                .parse::<usize>()
                .map_err(|_| invalid("旧章节引用编号溢出"))?;
            result.push_str(&format!("【旧章节引用{old}，非本轮证据】"));
            remainder = &after[digits + 1..];
        } else {
            result.push_str("[S");
            remainder = after;
        }
    }
    result.push_str(remainder);
    Ok(result)
}

fn current_reference(
    conn: &Connection,
    base: &str,
    reference: &ResearchEvidenceReference,
) -> Result<bool, BrainError> {
    let index =
        i64::try_from(reference.citation_index).map_err(|_| invalid("研究引用编号超出范围"))?;
    let raw:Option<(String,String,String,String)>=conn.query_row("SELECT kind,object_id,version_id,snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",params![reference.run_id,index],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    let Some((kind, object, version, raw)) = raw else {
        return Ok(false);
    };
    if kind != reference.kind
        || object != reference.object_id
        || version != reference.version_id
        || stable_id("research-evidence", &raw) != reference.snapshot_hash
    {
        return Ok(false);
    }
    let snapshot: Value = decode(&raw)?;
    if snapshot
        .pointer("/entry/knowledge_base_id")
        .and_then(Value::as_str)
        != Some(base)
        || !snapshot["segments"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["content"]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty())
            })
        })
    {
        return Ok(false);
    }
    let fresh:bool=match kind.as_str() {
        "entry"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_entries e WHERE e.id=?1 AND e.knowledge_base_id=?2 AND CAST(e.revision AS TEXT)=?3 AND e.status NOT IN ('stale','archived') AND NOT EXISTS(SELECT 1 FROM knowledge_source_impacts i WHERE i.entry_id=e.id AND i.resolved_at IS NULL))",params![object,base,version],|row|row.get(0))?,
        "source_span"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM source_spans s JOIN source_versions v ON v.id=s.source_version_id JOIN source_documents d ON d.id=v.source_document_id WHERE s.id=?1 AND s.knowledge_base_id=?2 AND s.source_version_id=?3 AND d.knowledge_base_id=?2 AND d.current_version_id=v.id AND d.sync_status='current')",params![object,base,version],|row|row.get(0))?,
        "external"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs r JOIN knowledge_tasks t ON t.id=json_extract(r.input_json,'$.knowledge_task_id') WHERE r.id=?1 AND r.knowledge_base_id=?2 AND t.knowledge_base_id=?2 AND t.external_research_enabled=1 AND json_extract(r.input_json,'$.external_research.enabled')=1)",params![reference.run_id,base],|row|row.get(0))?,
        _=>false,
    };
    if !fresh {
        return Ok(false);
    }
    for span in snapshot["source_span_ids"].as_array().into_iter().flatten() {
        let Some(span) = span.as_str() else {
            return Ok(false);
        };
        let current:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM source_spans s JOIN source_versions v ON v.id=s.source_version_id JOIN source_documents d ON d.id=v.source_document_id WHERE s.id=?1 AND s.knowledge_base_id=?2 AND d.knowledge_base_id=?2 AND d.current_version_id=v.id AND d.sync_status='current')",params![span,base],|row|row.get(0))?;
        if !current {
            return Ok(false);
        }
    }
    Ok(true)
}

fn section_evidence(
    conn: &Connection,
    base: &str,
    run: &str,
    output: &ResearchSectionOutput,
) -> Result<Vec<ResearchEvidenceReference>, BrainError> {
    if !text(&output.summary, 2000)
        || !text(&output.content_md, 120_000)
        || output.findings.is_empty()
        || output.findings.len() > 64
    {
        return Err(invalid(
            "章节必须有完整正文、概述与证据矩阵，不能通过截尾或空成果推进阶段",
        ));
    }
    let mut indices = body_indices(&output.content_md)?;
    indices.extend(body_indices(&output.summary)?);
    for finding in &output.findings {
        if !text(&finding.finding, 4000)
            || !matches!(
                finding.status.as_str(),
                "supported" | "partial" | "missing" | "conflict"
            )
            || !bounded_list(&finding.limitations, 16, 2000)
            || finding.citation_indices.len() > 64
            || finding.citation_indices.contains(&0)
            || finding
                .citation_indices
                .iter()
                .collect::<HashSet<_>>()
                .len()
                != finding.citation_indices.len()
            || (matches!(finding.status.as_str(), "supported" | "conflict")
                && finding.citation_indices.is_empty())
            || (matches!(finding.status.as_str(), "partial" | "missing")
                && finding.limitations.is_empty())
        {
            return Err(invalid(
                "研究发现的状态、真实依据或缺口说明不合法；自报支持必须有已读引用",
            ));
        }
        indices.extend(finding.citation_indices.iter().copied());
        indices.extend(body_indices(&finding.finding)?);
        for limitation in &finding.limitations {
            indices.extend(body_indices(limitation)?);
        }
    }
    let mut indices = indices.into_iter().collect::<Vec<_>>();
    indices.sort_unstable();
    let mut references = Vec::with_capacity(indices.len());
    for index in indices {
        let ordinal = i64::try_from(index).map_err(|_| invalid("研究引用编号超出范围"))?;
        let (kind,object,version,raw):(String,String,String,String)=conn.query_row("SELECT kind,object_id,version_id,snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",params![run,ordinal],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?.ok_or_else(||invalid("研究章节引用了本次未实际读取的证据"))?;
        let reference = ResearchEvidenceReference {
            run_id: run.into(),
            citation_index: index,
            kind,
            object_id: object,
            version_id: version,
            snapshot_hash: stable_id("research-evidence", &raw),
        };
        if !current_reference(conn, base, &reference)? {
            return Err(conflict(
                "阶段保存前来源已变化或超出书籍权限；保留旧成果，不认证为当前章节",
            ));
        }
        references.push(reference);
    }
    Ok(references)
}

fn read_content(
    conn: &Connection,
    task: &str,
    key: &str,
) -> Result<ResearchStageContent, BrainError> {
    let stage=conn.query_row(&format!("SELECT {SUMMARY_COLUMNS} FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2"),params![task,key],stage_summary).optional()?.ok_or_else(||invalid("研究阶段不存在"))?;
    let (body,findings,evidence,content_run_id):(String,String,String,Option<String>)=conn.query_row("SELECT content_md,findings_json,evidence_json,content_run_id FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,key],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
    Ok(ResearchStageContent {
        stage,
        content_run_id,
        content_md: body,
        findings: decode(&findings)?,
        evidence: decode(&evidence)?,
    })
}

fn preserve(conn: &Connection, task: &str, key: &str, now: &str) -> Result<(), BrainError> {
    let content = read_content(conn, task, key)?;
    if !content.content_md.is_empty()
        || content.stage.status == "completed"
        || content.stage.run_id.is_some()
    {
        conn.execute("INSERT OR IGNORE INTO knowledge_research_stage_versions(task_id,stage_key,revision,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![task,key,content.stage.revision,encode(&content)?,now])?;
    }
    Ok(())
}

fn capture_baseline(conn: &Connection, base: &str, entry: &str) -> Result<Value, BrainError> {
    let raw:String=conn.query_row("SELECT json_object('entry_id',id,'revision',revision,'title',title,'summary',summary,'status',status,'content_md',content_md) FROM knowledge_entries WHERE id=?1 AND knowledge_base_id=?2 AND entry_type<>'source_section' AND status<>'archived'",params![entry,base],|row|row.get(0)).optional()?.ok_or_else(||invalid("研究基线必须是本书真实的编译条目，不能引用跨书、未知、已归档或章节兜底对象"))?;
    let mut snapshot: Value = decode(&raw)?;
    let mut claims=conn.prepare("SELECT json_object('id',id,'predicate',predicate,'object_text',object_text,'claim_text',claim_text,'verification_status',verification_status,'revision',revision) FROM knowledge_claims WHERE entry_id=?1 AND knowledge_base_id=?2 ORDER BY created_at,id")?;
    let claims = claims
        .query_map(params![entry, base], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(|raw| decode::<Value>(raw))
        .collect::<Result<Vec<_>, _>>()?;
    let mut citations=conn.prepare("SELECT json_object('claim_id',c.claim_id,'source_span_id',c.source_span_id,'source_version_id',s.source_version_id,'source_path',d.relative_path,'heading',s.heading,'line_start',s.line_start,'line_end',s.line_end,'quote_text',c.quote_text,'baseline_source_content',s.content) FROM knowledge_citations c JOIN source_spans s ON s.id=c.source_span_id JOIN source_versions v ON v.id=s.source_version_id JOIN source_documents d ON d.id=v.source_document_id WHERE c.knowledge_base_id=?2 AND (c.entry_id=?1 OR c.claim_id IN (SELECT id FROM knowledge_claims WHERE entry_id=?1)) ORDER BY c.id")?;
    snapshot["claims"] = json!(claims);
    snapshot["source_basis"] = json!(citations
        .query_map(params![entry, base], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(|raw| decode::<Value>(raw))
        .collect::<Result<Vec<_>, _>>()?);
    if encode(&snapshot)?.len() > 16 * 1024 * 1024 {
        return Err(invalid("单个核验基线超过存储安全边界；不截尾伪装完整基线"));
    }
    Ok(snapshot)
}

fn baseline_summaries(
    conn: &Connection,
    task: &str,
) -> Result<Vec<ResearchBaselineSummary>, BrainError> {
    let mut stmt=conn.prepare("SELECT question_id,entry_id,json_extract(snapshot_json,'$.revision'),json_extract(snapshot_json,'$.title'),json_extract(snapshot_json,'$.status'),length(json_extract(snapshot_json,'$.content_md')),json_array_length(snapshot_json,'$.claims'),captured_at FROM knowledge_research_baselines WHERE task_id=?1 ORDER BY question_id,entry_id")?;
    let rows = stmt.query_map([task], |row| {
        Ok(ResearchBaselineSummary {
            question_id: row.get(0)?,
            entry_id: row.get(1)?,
            revision: row.get(2)?,
            title: row.get(3)?,
            status: row.get(4)?,
            content_characters: row.get(5)?,
            claim_count: row.get(6)?,
            captured_at: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn check_baseline_findings(
    conn: &Connection,
    claim: &ResearchStageClaim,
    output: &ResearchSectionOutput,
) -> Result<(), BrainError> {
    let question = claim
        .stage_key
        .strip_prefix("section:")
        .ok_or_else(|| invalid("章节缺少问题身份"))?;
    let task_type: String = conn.query_row(
        "SELECT task_type FROM knowledge_tasks WHERE id=?1",
        [&claim.task_id],
        |row| row.get(0),
    )?;
    let baselines = baseline_summaries(conn, &claim.task_id)?
        .into_iter()
        .filter(|b| b.question_id == question)
        .collect::<Vec<_>>();
    let mut covered = HashSet::new();
    for finding in &output.findings {
        if let Some(entry) = &finding.baseline_entry_id {
            if !baselines.iter().any(|b| b.entry_id == *entry) {
                return Err(invalid("研究发现引用了未冻结的核验对象"));
            }
            if let Some(id) = &finding.baseline_claim_id {
                let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_baselines b,json_each(b.snapshot_json,'$.claims') c WHERE b.task_id=?1 AND b.question_id=?2 AND b.entry_id=?3 AND json_extract(c.value,'$.id')=?4)",params![claim.task_id,question,entry,id],|row|row.get(0))?;
                if !exists {
                    return Err(invalid("研究发现的基线主张不存在，不能编造核验身份"));
                }
            }
            covered.insert(entry);
        } else if finding.baseline_claim_id.is_some() {
            return Err(invalid("基线主张必须同时指定所属条目"));
        }
    }
    if matches!(task_type.as_str(), "review" | "refresh") {
        if baselines.is_empty()
            && output
                .findings
                .iter()
                .any(|f| matches!(f.status.as_str(), "supported" | "conflict"))
        {
            return Err(invalid(
                "未找到真实核验/刷新基线，只能报告证据缺口，不能宣布完成核验或版本变化",
            ));
        }
        if baselines.iter().any(|b| !covered.contains(&b.entry_id)) {
            return Err(invalid(
                "每个选定的基线条目必须有对应发现，不得只写泛泛研究结论",
            ));
        }
    }
    Ok(())
}

/// Replace citation labels in one pass. Sequential string replacement corrupts
/// collisions such as S1 -> S2 followed by S2 -> S3.
fn relabel(value: &str, mapping: &HashMap<usize, usize>) -> Result<String, BrainError> {
    let mut result = String::with_capacity(value.len());
    let mut remainder = value;
    while let Some((before, after)) = remainder.split_once("[S") {
        result.push_str(before);
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after.as_bytes().get(digits) == Some(&b']') {
            let old = after[..digits]
                .parse::<usize>()
                .map_err(|_| invalid("引用编号溢出"))?;
            let new = mapping
                .get(&old)
                .ok_or_else(|| invalid("章节正文引用未进入实际证据矩阵"))?;
            result.push_str(&format!("[S{new}]"));
            remainder = &after[digits + 1..];
        } else {
            result.push_str("[S");
            remainder = after;
        }
    }
    result.push_str(remainder);
    Ok(result)
}

impl BookWikiStore {
    pub(crate) fn research_integration_manifest(&self, task: &str) -> Result<Value, BrainError> {
        self.db.with_connection(|conn| {
            let raw: String = conn.query_row("SELECT plan_json FROM knowledge_research_workspaces WHERE task_id=?1", [task], |row| row.get(0))?;
            let plan: ResearchPlan = decode(&raw)?;
            let mut sections = Vec::new();
            for question in plan.questions {
                let (status,revision,summary,findings,evidence):(String,i64,String,String,String)=conn.query_row("SELECT status,revision,summary,findings_json,evidence_json FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,format!("section:{}",question.id)],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)))?;
                if status!="completed" { return Err(invalid("主题尚未完成，不能进行跨章节综合")) }
                let findings: Vec<ResearchFinding> = decode(&findings)?;
                sections.push(json!({"question_id":question.id,"title":question.title,"revision":revision,"summary":neutral_section_text(&summary)?,"findings":findings.iter().map(|finding| Ok(json!({"finding":neutral_section_text(&finding.finding)?,"status":finding.status,"limitations":finding.limitations.iter().map(|text|neutral_section_text(text)).collect::<Result<Vec<_>,_>>()?,"saved_reference_indices":finding.citation_indices}))).collect::<Result<Vec<Value>,BrainError>>()?,"saved_reference_objects":decode::<Value>(&evidence)?}));
            }
            Ok(json!({"sections":sections,"notice":"章节成果是待综合输入；旧章节编号不是本轮已读依据。按需分页查看完整章节，并读取当前实体或原文后再使用本轮S编号。"}))
        })
    }

    pub(crate) fn read_research_manifest_page(
        &self,
        task: &str,
        question_id: Option<&str>,
        offset: usize,
        max_chars: usize,
    ) -> Result<Value, BrainError> {
        let manifest = self.research_integration_manifest(task)?;
        let scoped = if let Some(question_id) = question_id {
            let section = manifest["sections"]
                .as_array()
                .and_then(|sections| {
                    sections
                        .iter()
                        .find(|section| section["question_id"] == question_id)
                })
                .ok_or_else(|| invalid("只能读取当前任务已规划的综合章节矩阵"))?;
            json!({"section":section,"notice":manifest["notice"]})
        } else {
            manifest
        };
        let raw = scoped.to_string();
        let total = raw.chars().count();
        let offset = offset.min(total);
        let length = max_chars.clamp(1, 12_000);
        Ok(json!({
            "content_json": raw.chars().skip(offset).take(length).collect::<String>(),
            "offset_chars": offset,
            "total_chars": total,
            "has_more": offset.saturating_add(length) < total,
            "manifest_hash": stable_id("research-integration-manifest", &raw),
            "question_id": question_id,
            "notice": "这是保存章节的综合输入而非本轮原始证据；分页可能切开 JSON 字段，需用连续 offset_chars 读取完整内容，旧编号不能作为本轮引用。",
        }))
    }

    pub(crate) fn read_research_section_page(
        &self,
        task: &str,
        question: &str,
        offset: usize,
        max_chars: usize,
    ) -> Result<Value, BrainError> {
        self.db.with_connection(|conn| {
            let raw: String = conn.query_row("SELECT plan_json FROM knowledge_research_workspaces WHERE task_id=?1", [task], |row| row.get(0))?;
            let plan: ResearchPlan = decode(&raw)?;
            if !plan.questions.iter().any(|value|value.id==question) { return Err(invalid("只能读取当前任务已规划的研究章节")) }
            let (status,revision,body,evidence):(String,i64,String,String)=conn.query_row("SELECT status,revision,content_md,evidence_json FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,format!("section:{question}")],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
            if status!="completed" { return Err(invalid("章节未完整保存，不能作为综合输入")) }
            // Offset refers to the consistently neutralized business text, not source pages.
            let body=neutral_section_text(&body)?;
            let total=body.chars().count();
            let offset=offset.min(total);
            let length=max_chars.clamp(1,12000);
            Ok(json!({"question_id":question,"revision":revision,"content_md":body.chars().skip(offset).take(length).collect::<String>(),"offset_chars":offset,"total_chars":total,"has_more":offset.saturating_add(length)<total,"saved_reference_objects":decode::<Value>(&evidence)?,"notice":"这是保存的章节成果，不是本轮原始证据；旧章节引用已中性化，需要读取当前实体或原文获得本轮S编号。"}))
        })
    }

    pub(crate) fn save_research_synthesis(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        output: &ResearchSynthesisOutput,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,kind)=owned_stage(conn,claim,false)?;
            if kind!="synthesis" { return Err(conflict("综合成果只能保存在交叉核验阶段")) }
            check_run(conn,claim,run,true)?;
            let raw:String=conn.query_row("SELECT plan_json FROM knowledge_research_workspaces WHERE task_id=?1",[&claim.task_id],|row|row.get(0))?;
            let plan:ResearchPlan=decode(&raw)?;
            let run_input:String=conn.query_row("SELECT input_json FROM agent_runs WHERE id=?1",[run],|row|row.get(0))?;
            let projected=decode::<Value>(&run_input)?["research_manifest_projected"].as_bool().unwrap_or(false);
            let manifest_pages=if projected {read_manifest_page_events(conn,run)?} else {Vec::new()};
            let mut seen=HashSet::new();
            if output.section_checks.len()!=plan.questions.len() { return Err(invalid("交叉核验必须逐项记录所有主题的对照结果，不能省略章节")) }
            let mut body=output.content_md.clone();
            body.push_str("\n\n### 跨章节对照记录\n\n以下为模型的公开对照判断，不是独立事实证明；原始章节保留，条件不同不得抹平。\n\n");
            for check in &output.section_checks {
                if !seen.insert(&check.question_id) || !matches!(check.assessment.as_str(),"consistent"|"qualified"|"conflict"|"insufficient") || !text(&check.note,4000) { return Err(invalid("交叉核验主题、判定或说明不符合合同")) }
                let question=plan.questions.iter().find(|q|q.id==check.question_id).ok_or_else(||invalid("交叉核验引用了未规划主题"))?;
                let current:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2 AND status='completed' AND revision=?3)",params![claim.task_id,format!("section:{}",check.question_id),check.revision],|row|row.get(0))?;
                if !current { return Err(conflict("交叉核验对应的章节版本已变化，不能写入过期综合")) }
                if projected && check.assessment!="insufficient" && !manifest_pages_cover(&manifest_pages,&check.question_id) { return Err(invalid("综合输入为容量投影，当前主题的完整发现矩阵未读完；不能标记口径一致、条件成立或冲突。请分页读完矩阵，或诚实标记依据不足")) }
                let label=match check.assessment.as_str(){"consistent"=>"口径一致（模型自报）","qualified"=>"条件成立","conflict"=>"存在冲突",_=>"依据不足"};
                body.push_str(&format!("- **{}** · {}：{}\n",question.title,label,check.note));
            }
            let section=ResearchSectionOutput{summary:output.summary.clone(),content_md:body,findings:output.findings.clone()};
            if section.findings.iter().any(|finding|finding.baseline_entry_id.is_some() || finding.baseline_claim_id.is_some()) { return Err(invalid("综合阶段不得冒充具体对象的历史核验")) }
            let references=section_evidence(conn,&base,run,&section)?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='completed',summary=?3,content_md=?4,findings_json=?5,evidence_json=?6,content_run_id=?7,error=NULL,revision=revision+1,updated_at=?8 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,section.summary,section.content_md,encode(&section.findings)?,encode(&references)?,run,now])?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            Ok(())
        })
    }
    pub(crate) fn ensure_research_workspace(&self, task: &str) -> Result<(), BrainError> {
        let request = self.get_task(task)?;
        let original = json!({"title":request.title,"description":request.description,"task_type":request.task_type,"deliverable_type":request.deliverable_type,"external_research_enabled":request.external_research_enabled,"external_domains":request.external_domains,"external_request_limit":request.external_request_limit});
        let raw = encode(&original)?;
        let hash = stable_id("research-request", &raw);
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,_)=live_task(conn,task,false)?;
            let old:Option<String>=conn.query_row("SELECT original_request_hash FROM knowledge_research_workspaces WHERE task_id=?1",[task],|row|row.get(0)).optional()?;
            if old.as_ref().is_some_and(|old|old!=&hash) {return Err(conflict("研究目标已变化，不能复用旧目标的阶段；请建立新任务保留原成果"))}
            conn.execute("INSERT OR IGNORE INTO knowledge_research_workspaces(task_id,knowledge_base_id,original_request_json,original_request_hash,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",params![task,base,raw,hash,now])?;
            conn.execute("INSERT OR IGNORE INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,updated_at) VALUES(?1,'plan','目标与研究规划','plan',0,'pending',?2)",params![task,now])?;
            Ok(())
        })
    }

    pub fn get_research_workspace(
        &self,
        task: &str,
    ) -> Result<Option<ResearchWorkspace>, BrainError> {
        self.get_task(task)?;
        self.db.with_connection(|conn| {
            let row:Option<(String,String,Option<String>,String,String)>=conn.query_row("SELECT knowledge_base_id,original_request_json,plan_json,created_at,updated_at FROM knowledge_research_workspaces WHERE task_id=?1",[task],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional()?;
            let Some((base,request,plan,created,updated))=row else {return Ok(None)};
            let mut stmt=conn.prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM knowledge_research_stages WHERE task_id=?1 ORDER BY ordinal,stage_key"))?;
            let stages=stmt.query_map([task],stage_summary)?.collect::<Result<Vec<_>,_>>()?;
            Ok(Some(ResearchWorkspace{task_id:task.into(),knowledge_base_id:base,original_request:decode(&request)?,plan:plan.as_deref().map(decode).transpose()?,stages,baselines:baseline_summaries(conn,task)?,created_at:created,updated_at:updated}))
        })
    }

    pub fn get_research_stage_content(
        &self,
        task: &str,
        key: &str,
        revision: Option<i64>,
    ) -> Result<ResearchStageContent, BrainError> {
        self.get_task(task)?;
        self.db.with_connection(|conn| {
            let current:i64=conn.query_row("SELECT revision FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,key],|row|row.get(0)).optional()?.ok_or_else(||invalid("研究阶段不存在"))?;
            if revision.is_none() || revision==Some(current) {return read_content(conn,task,key)}
            let raw:String=conn.query_row("SELECT snapshot_json FROM knowledge_research_stage_versions WHERE task_id=?1 AND stage_key=?2 AND revision=?3",params![task,key,revision],|row|row.get(0)).optional()?.ok_or_else(||invalid("该研究阶段历史版本不存在"))?;
            decode(&raw)
        })
    }

    /// The latest attempted model Run may no longer be the current stage's
    /// run_id after a preflight-only retry. Read it from the immutable Run
    /// ledger so a later resume still honors its observed capacity and output
    /// request, including databases created before failed stages were archived.
    pub(crate) fn latest_research_stage_run(
        &self,
        task: &str,
        stage_key: &str,
    ) -> Result<Option<String>, BrainError> {
        self.get_task(task)?;
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id FROM agent_runs
                 WHERE json_extract(input_json, '$.knowledge_task_id') = ?1
                   AND json_extract(input_json, '$.research_stage_key') = ?2
                   AND task_type LIKE 'knowledge_task_%'
                 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                params![task, stage_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })
    }

    /// Historical input has no S number and is never proof of current truth.
    pub fn read_research_baseline_page(
        &self,
        task: &str,
        question: &str,
        entry: &str,
        offset: usize,
        max_chars: usize,
        claims_offset: usize,
    ) -> Result<Value, BrainError> {
        self.get_task(task)?;
        self.db.with_connection(|conn| {
            let (raw,captured):(String,String)=conn.query_row("SELECT snapshot_json,captured_at FROM knowledge_research_baselines WHERE task_id=?1 AND question_id=?2 AND entry_id=?3",params![task,question,entry],|row|Ok((row.get(0)?,row.get(1)?))).optional()?.ok_or_else(||invalid("本研究问题没有该冻结基线"))?;
            let mut snapshot:Value=decode(&raw)?;
            let body=snapshot["content_md"].as_str().unwrap_or_default();
            let total=body.chars().count();
            let offset=offset.min(total);
            let limit=max_chars.clamp(1,12000);
            let page=body.chars().skip(offset).take(limit).collect::<String>();
            let claims=snapshot["claims"].as_array().cloned().unwrap_or_default();
            // Source basis is paged with claims: no implicit first-50 truncation.
            // It remains retrievable even when the original source is changed.
            let basis=snapshot["source_basis"].as_array().cloned().unwrap_or_default();
            snapshot["content_md"]=json!(page);
            snapshot["claims"]=json!(claims.iter().skip(claims_offset).take(16).collect::<Vec<_>>());
            snapshot["source_basis"]=json!(basis.iter().skip(claims_offset).take(16).map(|v| {let mut v=v.clone();if let Some(m)=v.as_object_mut() {m.remove("baseline_source_content");} v}).collect::<Vec<_>>());
            snapshot["claims_offset"]=json!(claims_offset);
            snapshot["claim_count"]=json!(claims.len());
            snapshot["source_basis_count"]=json!(basis.len());
            snapshot["metadata_has_more"]=json!(claims_offset.saturating_add(16)<claims.len().max(basis.len()));
            snapshot["offset_chars"]=json!(offset);
            snapshot["total_chars"]=json!(total);
            snapshot["has_more"]=json!(offset.saturating_add(limit)<total);
            snapshot["captured_at"]=json!(captured);
            snapshot["historical_baseline"]=json!(true);
            snapshot["evidence_notice"]=json!("这是待核验的历史输入，不是当前已读证据，没有S编号。请读取当前编译内容/原文后引用；旧版与当前版不得混淆。");
            Ok(snapshot)
        })
    }

    pub fn read_research_baseline_source_page(
        &self,
        task: &str,
        question: &str,
        entry: &str,
        index: usize,
        offset: usize,
        max_chars: usize,
    ) -> Result<Value, BrainError> {
        self.get_task(task)?;
        self.db.with_connection(|conn| {
            let raw:Option<String>=conn.query_row("SELECT json_extract(snapshot_json,?4) FROM knowledge_research_baselines WHERE task_id=?1 AND question_id=?2 AND entry_id=?3",params![task,question,entry,format!("$.source_basis[{index}]")],|row|row.get(0)).optional()?.flatten();
            let mut source:Value=decode(&raw.ok_or_else(||invalid("冻结基线中不存在该旧版来源"))?)?;
            let content=source["baseline_source_content"].as_str().unwrap_or_default();
            let total=content.chars().count();
            let offset=offset.min(total);
            let page=content.chars().skip(offset).take(max_chars.clamp(1,12000)).collect::<String>();
            source["returned_chars"]=json!(page.chars().count());
            source["baseline_source_content"]=json!(page);
            source["offset_chars"]=json!(offset);
            source["total_chars"]=json!(total);
            source["has_more"]=json!(offset.saturating_add(max_chars.clamp(1,12000))<total);
            source["historical_baseline"]=json!(true);
            Ok(source)
        })
    }

    pub(crate) fn research_report_outline(&self, run: &str) -> Result<Value, BrainError> {
        self.db.with_connection(|conn| {
            let raw:Option<String>=conn.query_row("SELECT json_extract(output_json,'$.section_ranges') FROM agent_runs WHERE id=?1",[run],|row|row.get(0)).optional()?.flatten();
            raw.as_deref().map(decode).transpose().map(|value|value.unwrap_or(Value::Null))
        })
    }

    pub(crate) fn claim_research_stage(
        &self,
        task: &str,
        key: &str,
    ) -> Result<ResearchStageClaim, BrainError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (_,attempt)=live_task(conn,task,false)?;
            let row:Option<(String,Option<i64>)>=conn.query_row("SELECT status,claimed_attempt FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,key],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
            let Some((status,old_attempt))=row else {return Err(invalid("研究阶段未规划，不能领取未知阶段"))};
            if status=="completed" || (status=="running" && old_attempt==Some(attempt)) {return Err(conflict("该阶段已完成或正在当前尝试中执行，不应重复研究"))}
            let kind:String=conn.query_row("SELECT kind FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",params![task,key],|row|row.get(0))?;
            let ready:bool=match kind.as_str() {
                "plan"=>true,
                "section"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='plan' AND status='completed')",[task],|row|row.get(0))?,
                "synthesis"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND kind='section') AND NOT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND kind='section' AND status<>'completed')",[task],|row|row.get(0))?,
                "report"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='synthesis' AND status='completed') AND NOT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND kind='section' AND status<>'completed')",[task],|row|row.get(0))?,
                "validation"=>conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='report' AND status='completed')",[task],|row|row.get(0))?,
                "presentation"=>conn.query_row("SELECT COUNT(*)=2 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key IN ('report','validation') AND status='completed'",[task],|row|row.get(0))?,
                _=>false,
            };
            if !ready {return Err(invalid("前置研究阶段尚未完整保存，不能提前宣布报告或演示交付"))}
            preserve(conn,task,key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='running',claim_id=?3,claimed_attempt=?4,run_id=NULL,error=NULL,revision=revision+1,updated_at=?5 WHERE task_id=?1 AND stage_key=?2",params![task,key,id,attempt,now])?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![task,now])?;
            Ok(ResearchStageClaim{task_id:task.into(),stage_key:key.into(),claim_id:id,attempt})
        })
    }

    pub(crate) fn attach_research_stage_run(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
    ) -> Result<(), BrainError> {
        self.db.transaction(|conn| {
            owned_stage(conn, claim, false)?;
            check_run(conn, claim, run, false)?;
            let attached: Option<String> = conn.query_row(
                "SELECT run_id FROM knowledge_research_stages WHERE task_id=?1 AND stage_key=?2",
                params![claim.task_id, claim.stage_key],
                |row| row.get(0),
            )?;
            if attached.as_deref().is_some_and(|id| id != run) {
                return Err(conflict("本阶段已关联其他运行，不能替换来源身份"));
            }
            conn.execute(
                "UPDATE knowledge_research_stages SET run_id=?3 WHERE task_id=?1 AND stage_key=?2",
                params![claim.task_id, claim.stage_key, run],
            )?;
            Ok(())
        })
    }

    pub(crate) fn save_research_plan(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        plan: &ResearchPlan,
    ) -> Result<(), BrainError> {
        validate_plan(plan)?;
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,kind)=owned_stage(conn,claim,false)?;
            if kind!="plan" {return Err(conflict("只有规划阶段能保存研究目标"))}
            check_run(conn,claim,run,true)?;
            let has_plan:bool=conn.query_row("SELECT plan_json IS NOT NULL FROM knowledge_research_workspaces WHERE task_id=?1",[&claim.task_id],|row|row.get(0))?;
            if has_plan {return Err(conflict("规划已持久化，不得改写已完成阶段的目标身份"))}
            for question in &plan.questions {
                for entry in &question.target_entry_ids {
                    let snapshot=capture_baseline(conn,&base,entry)?;
                    conn.execute("INSERT INTO knowledge_research_baselines(task_id,question_id,entry_id,snapshot_json,captured_at) VALUES(?1,?2,?3,?4,?5)",params![claim.task_id,question.id,entry,encode(&snapshot)?,now])?;
                }
            }
            conn.execute("UPDATE knowledge_research_workspaces SET plan_json=?2,updated_at=?3 WHERE task_id=?1",params![claim.task_id,encode(plan)?,now])?;
            for (ordinal,question) in plan.questions.iter().enumerate() {
                conn.execute("INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,updated_at) VALUES(?1,?2,?3,'section',?4,'pending',?5)",params![claim.task_id,format!("section:{}",question.id),question.title,ordinal as i64+1,now])?;
            }
            for (offset,key,title) in [(1,"synthesis","综合结论与交叉核验"),(2,"report","完整报告"),(3,"validation","引用与结构校验"),(4,"presentation","演示交付")] {
                if key=="presentation" {
                    let requested:bool=conn.query_row("SELECT deliverable_type='presentation' FROM knowledge_tasks WHERE id=?1",[&claim.task_id],|row|row.get(0))?;
                    if !requested {continue}
                }
                conn.execute("INSERT INTO knowledge_research_stages(task_id,stage_key,title,kind,ordinal,status,updated_at) VALUES(?1,?2,?3,?2,?4,'pending',?5)",params![claim.task_id,key,title,plan.questions.len() as i64+offset,now])?;
            }
            conn.execute("UPDATE knowledge_research_stages SET status='completed',run_id=?3,content_run_id=?3,summary=?4,content_md=?5,revision=revision+1,updated_at=?6 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,run,plan.goal,encode(plan)?,now])?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            Ok(())
        })
    }

    pub(crate) fn fail_research_stage(
        &self,
        claim: &ResearchStageClaim,
        error: &str,
        cancelled: bool,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            owned_stage(conn,claim,cancelled)?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status=?3,error=?4,revision=revision+1,updated_at=?5 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,if cancelled {"cancelled"} else {"failed"},error.chars().take(2000).collect::<String>(),now])?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            Ok(())
        })
    }

    pub(crate) fn save_research_section(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        output: &ResearchSectionOutput,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,kind)=owned_stage(conn,claim,false)?;
            if kind!="section" {return Err(conflict("章节成果不能替代其他业务阶段的交付物"))}
            check_run(conn,claim,run,true)?;
            check_baseline_findings(conn,claim,output)?;
            let references=section_evidence(conn,&base,run,output)?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='completed',summary=?3,content_md=?4,findings_json=?5,evidence_json=?6,run_id=?7,content_run_id=?7,error=NULL,revision=revision+1,updated_at=?8 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,output.summary,output.content_md,encode(&output.findings)?,encode(&references)?,run,now])?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            Ok(())
        })
    }

    /// Deterministic assembly of validated chapters. This is an application
    /// run, not a new LLM read or fictitious bill. Imported snapshots explicitly
    /// name the original Run in which their exact text was actually exposed.
    pub(crate) fn assemble_research_report(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
    ) -> Result<ResearchStageContent, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,kind)=owned_stage(conn,claim,false)?;
            if kind!="report" {return Err(invalid("只能在报告阶段组装完整章节"))}
            check_run(conn,claim,run,false)?;
            let plan_raw:String=conn.query_row("SELECT plan_json FROM knowledge_research_workspaces WHERE task_id=?1",[&claim.task_id],|row|row.get(0))?;
            let plan:ResearchPlan=decode(&plan_raw)?;
            let brief_raw:String=conn.query_row("SELECT brief_json FROM knowledge_tasks WHERE id=?1",[&claim.task_id],|row|row.get(0))?;
            let brief:ResearchBrief=decode(&brief_raw)?;
            let mut body=format!("# {}\n\n",plan.report_title.as_deref().unwrap_or(&plan.goal));
            let front_loaded=brief.confirmed && matches!(brief.purpose.as_str(),"decision"|"reference");
            if !front_loaded {append_report_context(&mut body,&plan,false)}
            let mut findings=Vec::<ResearchFinding>::new();
            let mut references=Vec::<ResearchEvidenceReference>::new();
            let mut identities=HashMap::new();
            let mut section_ranges=Vec::new();
            let mut sections=plan.questions.iter().map(|q|(format!("section:{}",q.id),q.id.clone(),q.title.clone())).collect::<Vec<_>>();
            let synthesis_title=match (brief.confirmed,brief.purpose.as_str()) {
                (true,"decision")=>"执行摘要与判断",
                (true,"reference")=>"要点速览",
                _=>"综合结论与交叉核验",
            };
            let synthesis=("synthesis".into(),"_synthesis".into(),synthesis_title.into());
            if front_loaded {
                sections.insert(0,synthesis);
            } else {
                sections.push(synthesis);
            }
            for (key,question_id,title) in sections {
                let section=read_content(conn,&claim.task_id,&key)?;
                if section.stage.status!="completed" {return Err(invalid("研究章节尚未完成，不能把局部报告伪装成完整交付"))}
                let mut mapping=HashMap::new();
                for origin in &section.evidence {
                    if !current_reference(conn,&base,origin)? {return Err(invalid("章节依据发生变化，必须复核受影响章节后再组装报告"))}
                    let raw:String=conn.query_row("SELECT snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",params![origin.run_id,origin.citation_index],|row|row.get(0))?;
                    let mut snapshot:Value=decode(&raw)?;
                    let identity=(origin.kind.clone(),origin.object_id.clone(),origin.version_id.clone());
                    let old=identities.get(&identity).copied();
                    let index=old.unwrap_or(references.len()+1);
                    let provenance=json!({"mode":"assembled_from_actual_read","run_id":origin.run_id,"citation_index":origin.citation_index,"stage_key":section.stage.stage_key,"revision":section.stage.revision});
                    if old.is_some() {
                        let existing:String=conn.query_row("SELECT snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",params![run,index],|row|row.get(0))?;
                        let mut existing:Value=decode(&existing)?;
                        let segments=existing["segments"].as_array_mut().ok_or_else(||invalid("证据快照缺少已读范围"))?;
                        for segment in snapshot["segments"].as_array().into_iter().flatten() {
                            if !segments.contains(segment) {segments.push(segment.clone())}
                        }
                        existing["research_origins"].as_array_mut().ok_or_else(||invalid("组合依据缺少来源身份"))?.push(provenance);
                        snapshot=existing;
                    } else {snapshot["research_origins"]=json!([provenance])}
                    let raw=encode(&snapshot)?;
                    mapping.insert(origin.citation_index,index);
                    conn.execute("INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(run_id,citation_index) DO UPDATE SET snapshot_json=excluded.snapshot_json",params![run,index,origin.kind,origin.object_id,origin.version_id,raw,now])?;
                    conn.execute("INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![run,origin.kind,origin.object_id,origin.version_id,encode(&json!({"citation_index":index,"origin":"research_assembly","origin_run_id":origin.run_id,"origin_citation_index":origin.citation_index}))?,now])?;
                    if old.is_some() {references[index-1].snapshot_hash=stable_id("research-evidence",&raw)} else {
                        identities.insert(identity,index);
                        references.push(ResearchEvidenceReference{run_id:run.into(),citation_index:index,kind:origin.kind.clone(),object_id:origin.object_id.clone(),version_id:origin.version_id.clone(),snapshot_hash:stable_id("research-evidence",&raw)});
                    }
                }
                let byte_start=body.len();
                let section_summary=relabel(&section.stage.summary,&mapping)?;
                append_report_section(&mut body,&title,if brief.confirmed {""} else {&section_summary},&relabel(&section.content_md,&mapping)?);
                if front_loaded && key=="synthesis" {append_report_context(&mut body,&plan,true)}
                let finding_start=findings.len();
                for mut finding in section.findings {
                    finding.finding=relabel(&finding.finding,&mapping)?;
                    finding.limitations=finding.limitations.iter().map(|value|relabel(value,&mapping)).collect::<Result<Vec<_>,_>>()?;
                    finding.citation_indices=finding.citation_indices.iter().map(|index|mapping.get(index).copied().ok_or_else(||invalid("研究发现引用缺失"))).collect::<Result<Vec<_>,_>>()?;
                    findings.push(finding);
                }
                section_ranges.push(json!({"question_id":question_id,"title":title,"summary":section_summary,"byte_start":byte_start,"byte_end":body.len(),"finding_start":finding_start,"finding_end":findings.len()}));
            }
            if body.len()>16*1024*1024 {return Err(invalid("完整报告超出存储安全边界，不能截尾伪装完成"))}
            let missing=findings.iter().filter(|finding|finding.status!="supported").count();
            let summary=format!("{} 个主题完整保存，{} 项发现，{} 项部分支持、缺口或争议；这是成果覆盖统计，不是事实正确性证明",plan.questions.len(),findings.len(),missing);
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='completed',summary=?3,content_md=?4,findings_json=?5,evidence_json=?6,error=NULL,content_run_id=?8,revision=revision+1,updated_at=?7 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,summary,body,encode(&findings)?,encode(&references)?,now,run])?;
            conn.execute("UPDATE agent_runs SET status='completed',output_json=?2,finished_at=?3,usage_source='unavailable' WHERE id=?1 AND status='running'",params![run,encode(&json!({"answer":body,"assembly_only":true,"section_ranges":section_ranges,"billing_usage":"not_applicable"}))?,now])?;
            insert_agent_run_event(conn,run,"run.completed",Some("completed"),&summary,&encode(&json!({"origin":"application_assembly","chapters":plan.questions.len(),"evidence":references.len(),"billing_usage":"not_applicable"}))?,&now)?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            read_content(conn,&claim.task_id,&claim.stage_key)
        })
    }

    /// Structural/current-evidence checks only. This does not pretend to prove
    /// the natural-language conclusions or independent source coverage.
    pub(crate) fn check_research_report(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,kind)=owned_stage(conn,claim,false)?;
            if kind!="validation" {return Err(invalid("核验交付必须在核验阶段执行"))}
            check_run(conn,claim,run,false)?;
            let report=read_content(conn,&claim.task_id,"report")?;
            if report.stage.status!="completed" {return Err(invalid("没有完整报告可供核验"))}
            for reference in &report.evidence {
                if !current_reference(conn,&base,reference)? {return Err(invalid("报告依据已变化，请复核对应章节"))}
            }
            let visible=report.evidence.iter().map(|reference|reference.citation_index).collect::<HashSet<_>>();
            if !body_indices(&report.content_md)?.is_subset(&visible) {return Err(invalid("报告存在未实际读取的引用"))}
            let gaps=report.findings.iter().filter(|finding|finding.status!="supported").count();
            let summary=format!("结构与当前引用校验通过；{} 项部分支持、缺口或争议仍需读者判断。此校验不是事实正确性证明。",gaps);
            let content=format!("## 交付校验\n\n- 研究章节已完整保存，未用部分章节伪装全书结论。\n- 引用来自章节内的实际读取记录，版本与当前知识库匹配。\n- 章节引用已统一编号，原始 Run 与读取范围可追溯。\n- 已记录 {} 项证据缺口或分歧；模型自报支持不等于独立事实核验。\n\n{}",gaps,summary);
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='completed',summary=?3,content_md=?4,error=NULL,content_run_id=?6,revision=revision+1,updated_at=?5 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,summary,content,now,run])?;
            conn.execute("UPDATE agent_runs SET status='completed',output_json=?2,finished_at=?3 WHERE id=?1 AND status='running'",params![run,encode(&json!({"answer":content,"validation_scope":"structure_and_current_citation_versions_not_entailment"}))?,now])?;
            insert_agent_run_event(conn,run,"run.completed",Some("completed"),&summary,"{\"application_validation\":true}",&now)?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            Ok(())
        })
    }

    pub(crate) fn save_research_presentation(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (_,kind)=owned_stage(conn,claim,false)?;
            if kind!="presentation" {return Err(invalid("演示成果不能替代研究章节"))}
            check_run(conn,claim,run,true)?;
            let artifact:Option<String>=conn.query_row("SELECT title FROM knowledge_artifacts WHERE knowledge_task_id=?1 AND agent_run_id=?2 AND validation_state='valid' ORDER BY created_at DESC LIMIT 1",params![claim.task_id,run],|row|row.get(0)).optional()?;
            let artifact=artifact.ok_or_else(||invalid("没有已通过文件校验的演示成果，不能宣布交付完成"))?;
            let plan:Option<String>=conn.query_row("SELECT json_extract(output_json,'$.answer') FROM agent_runs WHERE id=?1",[run],|row|row.get(0))?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_stages SET status='completed',summary=?3,content_md=?4,error=NULL,content_run_id=?6,revision=revision+1,updated_at=?5 WHERE task_id=?1 AND stage_key=?2",params![claim.task_id,claim.stage_key,format!("{artifact} 已生成，报告与演示分别保留"),plan.unwrap_or_default(),now,run])?;
            preserve(conn,&claim.task_id,&claim.stage_key,&now)?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![claim.task_id,now])?;
            Ok(())
        })
    }

    pub(crate) fn research_attempt(&self, task: &str) -> Result<i64, BrainError> {
        self.db
            .with_connection(|conn| Ok(live_task(conn, task, false)?.1))
    }

    pub(crate) fn complete_research_task(
        &self,
        task: &str,
        attempt: i64,
        report: &str,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,current_attempt)=live_task(conn,task,false)?;
            if current_attempt!=attempt {return Err(invalid("旧研究执行器不能完成新的任务尝试"))}
            let incomplete:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND status<>'completed')",[task],|row|row.get(0))?;
            if incomplete {return Err(invalid("仍有未完成的研究阶段，不能宣布整体交付"))}
            let (saved,evidence):(String,String)=conn.query_row("SELECT content_md,evidence_json FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='report' AND status='completed'",[task],|row|Ok((row.get(0)?,row.get(1)?)))?;
            if saved!=report {return Err(invalid("任务摘要与保存的完整报告不一致"))}
            for reference in decode::<Vec<ResearchEvidenceReference>>(&evidence)? {
                if !current_reference(conn,&base,&reference)? {return Err(invalid("交付完成前报告依据已变化，历史成果保留，但必须复核当前版本后才能完成任务"))}
            }
            conn.execute("UPDATE knowledge_tasks SET status='completed',result_summary=?2,lease_expires_at=NULL,last_heartbeat_at=NULL,next_attempt_at=NULL,artifact_state=CASE WHEN deliverable_type='presentation' THEN 'ready' ELSE artifact_state END,updated_at=?3 WHERE id=?1",params![task,report,now])?;
            Ok(())
        })
    }

    pub(crate) fn renew_research_task_lease(
        &self,
        task: &str,
        attempt: i64,
    ) -> Result<bool, BrainError> {
        let now = Utc::now();
        self.db.with_connection(|conn| Ok(conn.execute("UPDATE knowledge_tasks SET last_heartbeat_at=?3,lease_expires_at=?4 WHERE id=?1 AND status='running' AND research_execution_epoch=?2 AND julianday(lease_expires_at)>julianday('now')",params![task,attempt,now.to_rfc3339(),(now+chrono::Duration::seconds(45)).to_rfc3339()])?==1))
    }

    pub(crate) fn fail_research_task(
        &self,
        task: &str,
        attempt: i64,
        error: &str,
        retryable: bool,
    ) -> Result<(), BrainError> {
        let now = Utc::now();
        self.db.transaction(|conn| {
            if live_task(conn,task,true)?.1!=attempt {return Err(invalid("旧研究执行器不能改变新尝试的失败状态"))}
            let (cancelled,max_attempts,presentation,retry_count):(bool,i64,bool,i64)=conn.query_row("SELECT cancel_requested,max_attempts,deliverable_type='presentation',attempt_count FROM knowledge_tasks WHERE id=?1",[task],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
            let report:Option<String>=conn.query_row("SELECT content_md FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='report' AND content_md<>''",[task],|row|row.get(0)).optional()?;
            let reason=error.chars().take(2000).collect::<String>();
            let summary=if let Some(report)=report {format!("> [!warning] 后续交付未完成，完整研究报告和已保存阶段仍保留。\n> 原因：{reason}\n\n{report}")} else {format!("执行未完成：{reason}；已保存阶段保留，重试从未完成阶段恢复。")};
            let retry=retryable && !cancelled && retry_count<max_attempts && !["(research_output_hard_limit)","(research_input_hard_limit)","(research_output_retry_exhausted)"].iter().any(|marker|error.contains(marker));
            let status=if cancelled {"cancelled"} else if retry {"queued"} else {"failed"};
            let delay=5_i64.saturating_mul(2_i64.saturating_pow((retry_count-1).clamp(0,8) as u32)).min(300);
            let next=retry.then(||(now+chrono::Duration::seconds(delay)).to_rfc3339());
            conn.execute("UPDATE knowledge_tasks SET status=?2,result_summary=?3,lease_expires_at=NULL,last_heartbeat_at=NULL,next_attempt_at=?4,artifact_state=CASE WHEN ?5 THEN 'failed' ELSE artifact_state END,updated_at=?6 WHERE id=?1",params![task,status,summary,next,presentation,now.to_rfc3339()])?;
            Ok(())
        })
    }

    /// Current retry invalidates affected delivery status, never old text or
    /// frozen citations. Independent sections and the immutable plan survive.
    pub(crate) fn invalidate_changed_research_evidence(
        &self,
        task: &str,
    ) -> Result<usize, BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let (base,_)=live_task(conn,task,false)?;
            let mut stmt=conn.prepare("SELECT stage_key,evidence_json FROM knowledge_research_stages WHERE task_id=?1 AND kind IN ('section','synthesis') AND status='completed'")?;
            let keys=stmt.query_map([task],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            let mut invalidated=Vec::new();
            for (key,evidence) in keys {
                let evidence:Vec<ResearchEvidenceReference>=decode(&evidence)?;
                for reference in &evidence {
                    if !current_reference(conn,&base,reference)? {invalidated.push(key); break}
                }
            }
            if invalidated.is_empty() {return Ok(0)}
            let count=invalidated.len();
            let mut stmt=conn.prepare("SELECT stage_key FROM knowledge_research_stages WHERE task_id=?1 AND kind IN ('synthesis','report','validation','presentation') AND status='completed'")?;
            invalidated.extend(stmt.query_map([task],|row|row.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?);
            invalidated.sort_unstable();
            invalidated.dedup();
            for key in invalidated {
                preserve(conn,task,&key,&now)?;
                conn.execute("UPDATE knowledge_research_stages SET status='stale',error='已读依据的当前版本或可用性发生变化；历史成果仍保留',revision=revision+1,updated_at=?3 WHERE task_id=?1 AND stage_key=?2",params![task,key,now])?;
            }
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",params![task,now])?;
            Ok(count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::book_wiki_store::tests::{sample_book, sample_source, test_store};
    use crate::models::book_wiki::{
        ResearchBrief, ResearchFinding, ResearchPlan, ResearchQuestion, ResearchSectionOutput,
    };
    use serde_json::json;

    #[test]
    fn test_manifest_page_coverage_requires_every_character_from_one_version() {
        let pages = vec![
            json!({"question_id":"mechanism","offset_chars":0,"returned_chars":5,"total_chars":10,"manifest_hash":"v1"}),
            json!({"question_id":"mechanism","offset_chars":5,"returned_chars":5,"total_chars":10,"manifest_hash":"v1"}),
        ];
        assert!(manifest_pages_cover(&pages, "mechanism"));
        assert!(!manifest_pages_cover(&pages[..1], "mechanism"));
        assert!(!manifest_pages_cover(
            &[
                pages[0].clone(),
                json!({"question_id":"mechanism","offset_chars":5,"returned_chars":5,"total_chars":10,"manifest_hash":"v2"}),
            ],
            "mechanism"
        ));
        assert!(!manifest_pages_cover(&pages, "boundary"));
        assert!(!manifest_pages_cover(
            &[json!({"offset_chars":0,"returned_chars":10,"total_chars":10,"manifest_hash":"all"}),],
            "boundary"
        ));
        let all = vec![
            json!({"question_id":null,"offset_chars":0,"returned_chars":4,"total_chars":10,"manifest_hash":"all"}),
            json!({"question_id":null,"offset_chars":4,"returned_chars":6,"total_chars":10,"manifest_hash":"all"}),
        ];
        assert!(manifest_pages_cover(&all, "boundary"));
    }

    #[test]
    fn test_projected_manifest_needs_full_read_before_affirmative_synthesis_check() {
        for (with_reads, assessment, succeeds) in [
            (false, "consistent", false),
            (true, "consistent", true),
            (false, "insufficient", true),
        ] {
            let (store, _dir, base, task) = fixture();
            setup_plan(&store, &base, &task);
            complete_section_only(&store, &base, &task, "section:mechanism");
            complete_section_only(&store, &base, &task, "section:boundary");
            let claim = store.claim_research_stage(&task, "synthesis").unwrap();
            let run = store
                .start_agent_run(
                    &base,
                    "deepseek_harness",
                    "knowledge_task_research",
                    &json!({"knowledge_task_id":task,"research_stage_key":"synthesis","research_claim_id":claim.claim_id,"research_claim_attempt":claim.attempt,"research_manifest_projected":true}),
                )
                .unwrap();
            store.attach_research_stage_run(&claim, &run.id).unwrap();
            let manifest = store.research_integration_manifest(&task).unwrap();
            let checks = manifest["sections"]
                .as_array()
                .unwrap()
                .iter()
                .map(|section| crate::models::book_wiki::ResearchSectionCheck {
                    question_id: section["question_id"].as_str().unwrap().into(),
                    revision: section["revision"].as_i64().unwrap(),
                    assessment: assessment.into(),
                    note: if assessment == "insufficient" {
                        "完整矩阵尚未读取，暂不能核验一致性"
                    } else {
                        "已核对保存章节的完整发现矩阵"
                    }
                    .into(),
                })
                .collect();
            if with_reads {
                for question in ["mechanism", "boundary"] {
                    let page = store
                        .read_research_manifest_page(&task, Some(question), 0, 12_000)
                        .unwrap();
                    assert_eq!(page["has_more"], false);
                    store.append_agent_run_event(&run.id,"run.research_manifest_page",Some("synthesis"),"已读取完整综合矩阵页",&json!({"question_id":page["question_id"],"offset_chars":page["offset_chars"],"returned_chars":page["content_json"].as_str().unwrap().chars().count(),"total_chars":page["total_chars"],"manifest_hash":page["manifest_hash"]})).unwrap();
                }
            }
            store
                .complete_agent_run(&run.id, &json!({"answer":"有界综合"}))
                .unwrap();
            let output = ResearchSynthesisOutput {
                summary: "跨章节综合".into(),
                content_md: "结论需保持条件限定。".into(),
                findings: vec![ResearchFinding {
                    finding: "仍需核验外部适用性".into(),
                    status: "partial".into(),
                    citation_indices: vec![],
                    limitations: vec!["不把同源材料视为独立证明".into()],
                    baseline_entry_id: None,
                    baseline_claim_id: None,
                }],
                section_checks: checks,
            };
            let result = store.save_research_synthesis(&claim, &run.id, &output);
            if succeeds {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(result.unwrap_err().to_string().contains("完整发现矩阵未读"));
            }
        }
    }

    fn fixture() -> (BookWikiStore, tempfile::TempDir, String, String) {
        let (store, dir) = test_store();
        store
            .save_reader_books(&[sample_book("research", "/tmp/research")])
            .unwrap();
        let base = store.initialize_base("research").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[
                    sample_source("doc", "section", "span"),
                    sample_source("doc2", "section2", "span2"),
                ],
            )
            .unwrap();
        let task = store
            .create_task(&base.id, "比较机制与边界", "保留公式，检查反例", "research")
            .unwrap();
        store.start_task_execution(&task.id).unwrap();
        (store, dir, base.id, task.id)
    }

    #[test]
    fn test_exhausted_phase_output_stops_queue_retry_but_allows_explicit_resume() {
        let (store, _dir, _base, task) = fixture();
        let epoch = store.research_attempt(&task).unwrap();
        store
            .fail_research_task(
                &task,
                epoch,
                "(research_output_retry_exhausted) 当前阶段扩容已耗尽",
                false,
            )
            .unwrap();
        assert_eq!(store.get_task(&task).unwrap().status, "failed");
        assert_eq!(store.queue_task_execution(&task).unwrap().status, "queued");
    }

    #[test]
    fn test_research_retry_requires_explicit_transient_disposition() {
        for (retryable, expected) in [(false, "failed"), (true, "queued")] {
            let (store, _dir, _base, task) = fixture();
            let epoch = store.research_attempt(&task).unwrap();
            store
                .fail_research_task(&task, epoch, "模型调用失败", retryable)
                .unwrap();
            assert_eq!(store.get_task(&task).unwrap().status, expected);
        }
    }
    fn plan() -> ResearchPlan {
        ResearchPlan {
            goal: "比较机制与边界".into(),
            report_title: Some("机制与边界研究".into()),
            constraints: vec!["保留公式".into()],
            acceptance: vec!["给出条件差异及证据不足".into()],
            depth: "deep".into(),
            terminology: vec!["统一使用分层架构".into()],
            questions: vec![
                ResearchQuestion {
                    id: "mechanism".into(),
                    title: "机制与公式".into(),
                    question: "机制和完整公式是什么？".into(),
                    required_evidence: vec!["完整公式与变量含义".into()],
                    expected_output_tokens: None,
                    target_entry_ids: vec![],
                },
                ResearchQuestion {
                    id: "boundary".into(),
                    title: "边界与反例".into(),
                    question: "适用条件和反例是什么？".into(),
                    required_evidence: vec!["条件与反例".into()],
                    expected_output_tokens: None,
                    target_entry_ids: vec![],
                },
            ],
        }
    }
    fn attach(store: &BookWikiStore, base: &str, task: &str, claim: &ResearchStageClaim) -> String {
        let run=store.start_agent_run(base,"deepseek_harness","knowledge_task_research",&json!({"knowledge_task_id":task,"research_stage_key":claim.stage_key,"research_claim_id":claim.claim_id,"research_claim_attempt":claim.attempt})).unwrap();
        store.attach_research_stage_run(claim, &run.id).unwrap();
        run.id
    }
    fn setup_plan(store: &BookWikiStore, base: &str, task: &str) {
        store.ensure_research_workspace(task).unwrap();
        let claim = store.claim_research_stage(task, "plan").unwrap();
        let run = attach(store, base, task, &claim);
        store
            .complete_agent_run(&run, &json!({"answer":"规划"}))
            .unwrap();
        store.save_research_plan(&claim, &run, &plan()).unwrap();
    }
    fn output() -> ResearchSectionOutput {
        ResearchSectionOutput {
            summary: "当前章节只支持机制，边界仍需更多证据".into(),
            content_md: format!(
                "## 机制\n{}\n$$d_k=d_v=128$$\nLATE_CONCLUSION 保留条件与反例。[S1]",
                "正文条件。".repeat(2200)
            ),
            findings: vec![ResearchFinding {
                finding: "原文支持机制，但不提供普遍适用保证".into(),
                status: "partial".into(),
                citation_indices: vec![1],
                limitations: vec!["缺少跨版本实验".into()],
                baseline_entry_id: None,
                baseline_claim_id: None,
            }],
        }
    }

    #[test]
    fn test_report_section_omits_only_exact_repeated_heading_and_summary() {
        let mut body = String::new();
        append_report_section(
            &mut body,
            "适用边界",
            "只有条件成立才适用。[S1]",
            "## 适用边界\n\n只有条件成立才适用。[S1]\n\n完整推导与反例。[S1]",
        );
        assert_eq!(body.matches("## 适用边界").count(), 1);
        assert_eq!(body.matches("只有条件成立才适用。[S1]").count(), 1);
        assert!(body.contains("完整推导与反例。[S1]"));

        let mut distinct = String::new();
        append_report_section(
            &mut distinct,
            "适用边界",
            "简明摘要仍有新增限定。[S1]",
            "### 反例\n\n证据只覆盖某一版本。[S1]",
        );
        assert!(distinct.contains("简明摘要仍有新增限定。[S1]"));
        assert!(distinct.contains("### 反例"));
    }

    fn complete_section_only(store: &BookWikiStore, base: &str, task: &str, key: &str) {
        let claim = store.claim_research_stage(task, key).unwrap();
        let run = attach(store, base, task, &claim);
        let (span, version) = if key == "section:boundary" {
            ("span2", "version-doc2")
        } else {
            ("span", "version-doc")
        };
        store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                span,
                version,
                &json!({"content_md":"可追溯的来源正文。","offset_chars":0}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"结论 [S1]"}))
            .unwrap();
        store
            .save_research_section(&claim, &run, &output())
            .unwrap();
    }

    fn complete_section(store: &BookWikiStore, base: &str, task: &str, key: &str) {
        complete_section_only(store, base, task, key);
        complete_synthesis_if_ready(store, base, task);
    }

    fn complete_synthesis_if_ready(store: &BookWikiStore, base: &str, task: &str) {
        let workspace = store.get_research_workspace(task).unwrap().unwrap();
        if workspace
            .stages
            .iter()
            .filter(|stage| stage.kind == "section")
            .all(|stage| stage.status == "completed")
            && !workspace
                .stages
                .iter()
                .any(|stage| stage.kind == "synthesis" && stage.status == "completed")
        {
            let claim = store.claim_research_stage(task, "synthesis").unwrap();
            let run = attach(store, base, task, &claim);
            let manifest = store.research_integration_manifest(task).unwrap();
            let checks = manifest["sections"]
                .as_array()
                .unwrap()
                .iter()
                .map(|section| crate::models::book_wiki::ResearchSectionCheck {
                    question_id: section["question_id"].as_str().unwrap().into(),
                    revision: section["revision"].as_i64().unwrap(),
                    assessment: "insufficient".into(),
                    note: "反例仍需复核".into(),
                })
                .collect();
            store
                .complete_agent_run(&run, &json!({"answer":"有界综合"}))
                .unwrap();
            store
                .save_research_synthesis(
                    &claim,
                    &run,
                    &crate::models::book_wiki::ResearchSynthesisOutput {
                        summary: "保留条件和缺口".into(),
                        content_md: "跨章节综合仍受证据边界限制".into(),
                        findings: vec![crate::models::book_wiki::ResearchFinding {
                            finding: "反例尚不足".into(),
                            status: "partial".into(),
                            citation_indices: vec![],
                            limitations: vec!["不能证明普遍适用".into()],
                            baseline_entry_id: None,
                            baseline_claim_id: None,
                        }],
                        section_checks: checks,
                    },
                )
                .unwrap();
        }
    }

    #[test]
    fn test_report_assembly_relabels_actual_section_evidence_and_preserves_tail_and_provenance() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        assert!(store.claim_research_stage(&task, "report").is_err());
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &run).unwrap();
        assert!(report.content_md.starts_with("# 机制与边界研究\n\n"));
        assert!(
            report.content_md.find("## 机制与公式").unwrap()
                < report.content_md.find("## 综合结论与交叉核验").unwrap()
        );
        assert_eq!(report.content_md.matches("LATE_CONCLUSION").count(), 2);
        assert!(report.content_md.contains("$$d_k=d_v=128$$"));
        assert!(report.content_md.contains("[S1]"));
        assert!(report.content_md.contains("[S2]"));
        assert_eq!(report.evidence.len(), 2);
        assert_eq!(report.findings[0].citation_indices, vec![1]);
        assert_eq!(report.findings[1].citation_indices, vec![2]);
        let snapshot = store.get_agent_run_citation(&run, 1).unwrap();
        assert_eq!(snapshot.content_md, "可追溯的来源正文。");
        assert_eq!(store.get_agent_run(&run).unwrap().status, "completed");
        assert!(store.assemble_research_report(&claim, &run).is_err());
        let provenance: Value = store.db.with_connection(|conn| {
            let raw: String = conn.query_row("SELECT snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=2", [&run], |row| row.get(0))?;
            Ok(serde_json::from_str(&raw).unwrap())
        }).unwrap();
        assert_eq!(
            provenance["research_origins"][0]["mode"],
            "assembled_from_actual_read"
        );
        assert_ne!(provenance["research_origins"][0]["run_id"], run);
    }

    #[test]
    fn test_decision_report_opens_with_synthesis_without_losing_section_ranges() {
        let (store, _dir, base, task) = fixture();
        let brief = ResearchBrief {
            confirmed: true,
            purpose: "decision".into(),
            ..ResearchBrief::default()
        };
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET brief_json=?2 WHERE id=?1",
                    params![task, encode(&brief)?],
                )?;
                Ok(())
            })
            .unwrap();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &run).unwrap();
        let synthesis = report.content_md.find("## 执行摘要与判断").unwrap();
        let section = report.content_md.find("## 机制与公式").unwrap();
        assert!(report
            .content_md
            .starts_with("# 机制与边界研究\n\n## 执行摘要与判断"));
        assert!(synthesis < section);
        let boundaries = report.content_md.find("### 研究边界").unwrap();
        assert!(boundaries > synthesis && boundaries < section);
        assert!(report.content_md.contains("- **范围与约束**：保留公式"));
        assert!(
            !report.content_md.contains("保留条件和缺口"),
            "阶段摘要留在元数据，不重复插入已确认偏好的正文"
        );
        assert!(!report
            .content_md
            .contains("当前章节只支持机制，边界仍需更多证据"));
        assert!(report.content_md.contains("LATE_CONCLUSION"));
        let run = store.get_agent_run(&run).unwrap();
        let ranges = run.output.unwrap()["section_ranges"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(ranges[0]["question_id"], "_synthesis");
        assert_eq!(ranges[0]["summary"], "保留条件和缺口");
        assert_eq!(ranges.len(), 3);
        for (index, range) in ranges.iter().enumerate() {
            let start = range["byte_start"].as_u64().unwrap() as usize;
            let end = range["byte_end"].as_u64().unwrap() as usize;
            assert!(report.content_md[start..end].starts_with("## "));
            if index + 1 < ranges.len() {
                assert_eq!(
                    end,
                    ranges[index + 1]["byte_start"].as_u64().unwrap() as usize
                );
            }
        }
    }

    #[test]
    fn test_reference_report_opens_with_key_points_but_teaching_report_concludes_last() {
        for (purpose, heading, front) in [
            ("reference", "## 要点速览", true),
            ("teach", "## 综合结论与交叉核验", false),
        ] {
            let (store, _dir, base, task) = fixture();
            let brief = ResearchBrief {
                confirmed: true,
                purpose: purpose.into(),
                ..ResearchBrief::default()
            };
            store
                .db
                .with_connection(|conn| {
                    conn.execute(
                        "UPDATE knowledge_tasks SET brief_json=?2 WHERE id=?1",
                        params![task, encode(&brief)?],
                    )?;
                    Ok(())
                })
                .unwrap();
            setup_plan(&store, &base, &task);
            complete_section(&store, &base, &task, "section:mechanism");
            complete_section(&store, &base, &task, "section:boundary");
            let claim = store.claim_research_stage(&task, "report").unwrap();
            let run = attach(&store, &base, &task, &claim);
            let report = store.assemble_research_report(&claim, &run).unwrap();
            let synthesis = report.content_md.find(heading).unwrap();
            let section = report.content_md.find("## 机制与公式").unwrap();
            assert_eq!(synthesis < section, front, "{purpose}");
            assert!(
                !report.content_md.contains("保留条件和缺口"),
                "{purpose} 的阶段摘要不重复插入正文"
            );
            if front {
                assert!(report
                    .content_md
                    .starts_with(&format!("# 机制与边界研究\n\n{heading}")));
                assert!(report.content_md.find("### 研究边界").unwrap() < section);
            } else {
                assert!(report.content_md.find("研究范围与约束").unwrap() < section);
            }
        }
    }

    #[test]
    fn test_research_plan_accepts_legacy_missing_title_and_rejects_multiline_title() {
        let mut value = serde_json::to_value(plan()).unwrap();
        value.as_object_mut().unwrap().remove("report_title");
        let legacy: ResearchPlan = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.report_title, None);
        assert!(validate_plan(&legacy).is_ok());
        let mut invalid = plan();
        invalid.report_title = Some("标题\n伪造章节".into());
        assert!(validate_plan(&invalid).is_err());
    }

    #[test]
    fn test_cross_review_requires_every_current_chapter_and_never_inherits_old_citations() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        assert!(store.claim_research_stage(&task, "synthesis").is_err());
        complete_section_only(&store, &base, &task, "section:mechanism");
        complete_section_only(&store, &base, &task, "section:boundary");
        assert!(store.claim_research_stage(&task, "report").is_err());
        let claim = store.claim_research_stage(&task, "synthesis").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let manifest = store.research_integration_manifest(&task).unwrap();
        assert_eq!(manifest["sections"].as_array().unwrap().len(), 2);
        assert!(!manifest.to_string().contains("[S1]"));
        let manifest_page = store
            .read_research_manifest_page(&task, Some("mechanism"), 0, 12)
            .unwrap();
        assert_eq!(manifest_page["has_more"], true);
        assert_eq!(manifest_page["offset_chars"], 0);
        let manifest_end = store
            .read_research_manifest_page(
                &task,
                Some("mechanism"),
                manifest_page["total_chars"].as_u64().unwrap() as usize,
                12,
            )
            .unwrap();
        assert_eq!(manifest_end["has_more"], false);
        assert_eq!(
            manifest_end["manifest_hash"],
            manifest_page["manifest_hash"]
        );
        assert!(store
            .read_research_manifest_page(&task, Some("not-planned"), 0, 12)
            .is_err());
        let page = store
            .read_research_section_page(&task, "mechanism", 0, 12000)
            .unwrap();
        assert!(!page["content_md"].as_str().unwrap().contains("[S1]"));
        assert!(page.get("citation").is_none());
        assert!(store
            .read_research_section_page(&task, "not-planned", 0, 12000)
            .is_err());
        let capability = store
            .issue_agent_run_capability(
                &run,
                std::slice::from_ref(&base),
                &[
                    "knowledge_get_research_section".into(),
                    "knowledge_get_research_manifest".into(),
                ],
                300,
            )
            .unwrap();
        let tool = crate::core::agent_tool_gateway::call_agent_knowledge_tool;
        let read = tool(
            &store,
            &capability.token,
            "knowledge_get_research_section",
            json!({"question_id":"mechanism","max_chars":10}),
        )
        .unwrap();
        assert_eq!(read["has_more"], true);
        let full_matrix_page = tool(
            &store,
            &capability.token,
            "knowledge_get_research_manifest",
            json!({"question_id":"mechanism","max_chars":10}),
        )
        .unwrap();
        assert_eq!(full_matrix_page["has_more"], true);
        assert!(tool(
            &store,
            &capability.token,
            "knowledge_get_research_manifest",
            json!({"task_id":"spoofed"})
        )
        .is_err());
        let section_run = store
            .start_agent_run(
                &base,
                "deepseek_harness",
                "knowledge_task_research",
                &json!({"knowledge_task_id":task,"research_stage_key":"section:mechanism"}),
            )
            .unwrap();
        let section_capability = store
            .issue_agent_run_capability(
                &section_run.id,
                std::slice::from_ref(&base),
                &["knowledge_get_research_manifest".into()],
                300,
            )
            .unwrap();
        assert!(tool(
            &store,
            &section_capability.token,
            "knowledge_get_research_manifest",
            json!({"question_id":"mechanism"})
        )
        .is_err());
        assert!(tool(
            &store,
            &capability.token,
            "knowledge_get_research_section",
            json!({"question_id":"mechanism","task_id":"spoofed"})
        )
        .is_err());
        assert!(tool(
            &store,
            &capability.token,
            "knowledge_get_research_section",
            json!({"question_id":"other-task-section"})
        )
        .is_err());
        assert!(store.list_agent_run_citations(&run).unwrap().is_empty());
        let mut checks = manifest["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|section| crate::models::book_wiki::ResearchSectionCheck {
                question_id: section["question_id"].as_str().unwrap().into(),
                revision: section["revision"].as_i64().unwrap(),
                assessment: "insufficient".into(),
                note: "条件与反例仍缺依据；只作跨章节对照，不声称事实已证明".into(),
            })
            .collect::<Vec<_>>();
        let mut integrated = crate::models::book_wiki::ResearchSynthesisOutput {
            summary: "综合判定保留缺口".into(),
            content_md: "## 综合判断\n不同主题的条件分别保留；反例仍不足。".into(),
            findings: vec![crate::models::book_wiki::ResearchFinding {
                finding: "跨主题结论仍受条件与反例限制".into(),
                status: "partial".into(),
                citation_indices: vec![],
                limitations: vec!["缺少独立反例依据".into()],
                baseline_entry_id: None,
                baseline_claim_id: None,
            }],
            section_checks: checks.clone(),
        };
        store
            .complete_agent_run(&run, &json!({"answer":"综合判断"}))
            .unwrap();
        integrated.section_checks.pop();
        assert!(store
            .save_research_synthesis(&claim, &run, &integrated)
            .is_err());
        checks[0].revision += 1;
        integrated.section_checks = checks.clone();
        assert!(store
            .save_research_synthesis(&claim, &run, &integrated)
            .is_err());
        checks[0].revision -= 1;
        integrated.section_checks = checks;
        integrated.content_md.push_str("[S1]");
        assert!(store
            .save_research_synthesis(&claim, &run, &integrated)
            .is_err());
        integrated.content_md = "## 综合判断\n不同主题的条件分别保留；反例仍不足。".into();
        store
            .save_research_synthesis(&claim, &run, &integrated)
            .unwrap();
        let saved = store
            .get_research_stage_content(&task, "synthesis", None)
            .unwrap();
        assert_eq!(saved.stage.status, "completed");
        assert!(saved.content_md.contains("跨章节对照，不声称事实已证明"));
        assert!(saved.evidence.is_empty());
    }

    #[test]
    fn test_source_read_only_during_synthesis_invalidates_review_and_report_not_saved_chapters() {
        let (store, _dir, base, task) = fixture();
        store
            .sync_markdown_sources(
                &base,
                &[
                    sample_source("doc", "section", "span"),
                    sample_source("doc2", "section2", "span2"),
                    sample_source("review-doc", "review-section", "review-span"),
                ],
            )
            .unwrap();
        setup_plan(&store, &base, &task);
        complete_section_only(&store, &base, &task, "section:mechanism");
        complete_section_only(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "synthesis").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "review-span",
                "version-review-doc",
                &json!({"content_md":"可追溯的来源正文。"}),
            )
            .unwrap();
        let manifest = store.research_integration_manifest(&task).unwrap();
        let checks = manifest["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|section| crate::models::book_wiki::ResearchSectionCheck {
                question_id: section["question_id"].as_str().unwrap().into(),
                revision: section["revision"].as_i64().unwrap(),
                assessment: "qualified".into(),
                note: "结论受适用条件限制 [S1]".into(),
            })
            .collect();
        store
            .complete_agent_run(&run, &json!({"answer":"有界结论 [S1]"}))
            .unwrap();
        store
            .save_research_synthesis(
                &claim,
                &run,
                &crate::models::book_wiki::ResearchSynthesisOutput {
                    summary: "综合核对当前条件".into(),
                    content_md: "综合结论有条件 [S1]".into(),
                    findings: vec![ResearchFinding {
                        finding: "有界结论".into(),
                        status: "supported".into(),
                        citation_indices: vec![1],
                        limitations: vec![],
                        baseline_entry_id: None,
                        baseline_claim_id: None,
                    }],
                    section_checks: checks,
                },
            )
            .unwrap();
        let report = store.claim_research_stage(&task, "report").unwrap();
        let report_run = attach(&store, &base, &task, &report);
        let saved = store
            .assemble_research_report(&report, &report_run)
            .unwrap();
        assert_eq!(saved.evidence.len(), 3);
        let validation = store.claim_research_stage(&task, "validation").unwrap();
        let validation_run = attach(&store, &base, &task, &validation);
        store
            .check_research_report(&validation, &validation_run)
            .unwrap();
        store
            .sync_markdown_sources(
                &base,
                &[
                    sample_source("doc", "section", "span"),
                    sample_source("doc2", "section2", "span2"),
                ],
            )
            .unwrap();
        let attempt = store.research_attempt(&task).unwrap();
        assert!(
            store
                .complete_research_task(&task, attempt, &saved.content_md)
                .is_err(),
            "source changes after validation must not certify a current complete deliverable"
        );
        assert_eq!(store.get_task(&task).unwrap().status, "running");
        assert_eq!(
            store.invalidate_changed_research_evidence(&task).unwrap(),
            1
        );
        let workspace = store.get_research_workspace(&task).unwrap().unwrap();
        assert!(workspace
            .stages
            .iter()
            .filter(|stage| stage.kind == "section")
            .all(|stage| stage.status == "completed"));
        for key in ["synthesis", "report"] {
            let current = store.get_research_stage_content(&task, key, None).unwrap();
            assert_eq!(current.stage.status, "stale");
            assert!(!current.content_md.is_empty());
            let historical = store
                .get_research_stage_content(&task, key, Some(current.stage.revision - 1))
                .unwrap();
            assert_eq!(historical.stage.status, "completed");
            assert_eq!(historical.content_md, current.content_md);
        }
        assert!(store.claim_research_stage(&task, "report").is_err());
        assert_eq!(
            store.invalidate_changed_research_evidence(&task).unwrap(),
            0
        );
    }

    #[test]
    fn test_report_assembly_current_version_check_is_atomic_and_keeps_completed_sections() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store.sync_markdown_sources(&base, &[]).unwrap();
        assert!(store.assemble_research_report(&claim, &run).is_err());
        assert!(store.list_agent_run_citations(&run).unwrap().is_empty());
        assert_eq!(store.get_agent_run(&run).unwrap().status, "running");
        assert_eq!(
            store
                .get_research_stage_content(&task, "section:boundary", None)
                .unwrap()
                .stage
                .status,
            "completed"
        );
    }

    #[test]
    fn test_research_report_deduplicates_same_object_but_retains_every_actual_read_origin() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        for (key, offset, text) in [
            ("section:mechanism", 0, "可追溯"),
            ("section:boundary", 4, "来源正文。"),
        ] {
            let claim = store.claim_research_stage(&task, key).unwrap();
            let run = attach(&store, &base, &task, &claim);
            store
                .record_visible_agent_evidence(
                    &run,
                    "source_span",
                    "span",
                    "version-doc",
                    &json!({"content_md":text,"offset_chars":offset}),
                )
                .unwrap();
            store
                .complete_agent_run(&run, &json!({"answer":"结论 [S1]"}))
                .unwrap();
            store
                .save_research_section(&claim, &run, &output())
                .unwrap();
        }
        complete_synthesis_if_ready(&store, &base, &task);
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &run).unwrap();
        assert_eq!(report.evidence.len(), 1);
        assert_eq!(report.findings[1].citation_indices, vec![1]);
        let snapshot = store.get_agent_run_citation(&run, 0).unwrap();
        assert_eq!(snapshot.read_ranges.len(), 2);
        assert!(snapshot.content_md.contains("可追溯"));
        assert!(snapshot.content_md.contains("来源正文。"));
        let origins: Value = store
            .db
            .with_connection(|conn| {
                let raw: String = conn.query_row(
                    "SELECT snapshot_json FROM agent_run_citations WHERE run_id=?1",
                    [&run],
                    |row| row.get(0),
                )?;
                Ok(serde_json::from_str(&raw).unwrap())
            })
            .unwrap();
        assert_eq!(origins["research_origins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_changed_source_keeps_independent_section_and_old_report_provenance_during_retry() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let boundary = store
            .get_research_stage_content(&task, "section:boundary", None)
            .unwrap();
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let report_run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &report_run).unwrap();
        store
            .sync_markdown_sources(&base, &[sample_source("doc2", "section2", "span2")])
            .unwrap();
        assert_eq!(
            store.invalidate_changed_research_evidence(&task).unwrap(),
            1
        );
        assert_eq!(
            store
                .get_research_stage_content(&task, "section:boundary", None)
                .unwrap(),
            boundary
        );
        let old = store
            .get_research_stage_content(&task, "report", None)
            .unwrap();
        assert_eq!(old.stage.status, "stale");
        assert_eq!(old.content_run_id, Some(report_run.clone()));
        assert_eq!(old.content_md, report.content_md);
        // Simulate already revalidated changed chapter in this temporary DB;
        // report assembly failure must retain the previous content's identity.
        store.db.with_connection(|conn| {conn.execute("UPDATE knowledge_research_stages SET status='completed' WHERE task_id=?1 AND stage_key='section:mechanism'",[&task])?;Ok(())}).unwrap();
        assert!(store.claim_research_stage(&task, "report").is_err());
        complete_synthesis_if_ready(&store, &base, &task);
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let active_run = attach(&store, &base, &task, &claim);
        let retry = store
            .get_research_stage_content(&task, "report", None)
            .unwrap();
        assert_eq!(retry.stage.run_id, Some(active_run));
        assert_eq!(retry.content_run_id, Some(report_run));
        assert_eq!(retry.content_md, report.content_md);
        assert_eq!(retry.evidence, report.evidence);
    }

    #[test]
    fn test_stale_source_at_section_save_is_a_state_conflict_not_model_error() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        let claim = store
            .claim_research_stage(&task, "section:mechanism")
            .unwrap();
        let run = attach(&store, &base, &task, &claim);
        store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "span",
                "version-doc",
                &json!({"content_md":"可追溯的来源正文。","offset_chars":0}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"结论 [S1]"}))
            .unwrap();
        store
            .sync_markdown_sources(&base, &[sample_source("doc2", "section2", "span2")])
            .unwrap();

        let error = store
            .save_research_section(&claim, &run, &output())
            .unwrap_err();
        assert!(matches!(error, BrainError::KnowledgeConflict(_)));
        let stage = store
            .get_research_stage_content(&task, "section:mechanism", None)
            .unwrap();
        assert_eq!(stage.stage.status, "running");
        assert!(stage.content_md.is_empty());
    }

    #[test]
    fn test_changed_section_revision_at_synthesis_save_is_a_state_conflict() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        complete_section_only(&store, &base, &task, "section:mechanism");
        complete_section_only(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "synthesis").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let checks = store.research_integration_manifest(&task).unwrap()["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|section| crate::models::book_wiki::ResearchSectionCheck {
                question_id: section["question_id"].as_str().unwrap().into(),
                revision: section["revision"].as_i64().unwrap(),
                assessment: "insufficient".into(),
                note: "仍有证据缺口".into(),
            })
            .collect();
        store
            .complete_agent_run(&run, &json!({"answer":"有界综合"}))
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_research_stages SET revision=revision+1 WHERE task_id=?1 AND stage_key='section:mechanism'",
                    [&task],
                )?;
                Ok(())
            })
            .unwrap();

        let error = store
            .save_research_synthesis(
                &claim,
                &run,
                &ResearchSynthesisOutput {
                    summary: "证据仍有缺口".into(),
                    content_md: "当前不能得出无条件结论。".into(),
                    findings: vec![ResearchFinding {
                        finding: "尚需核验差异".into(),
                        status: "partial".into(),
                        citation_indices: vec![],
                        limitations: vec!["缺少当前版本核验".into()],
                        baseline_entry_id: None,
                        baseline_claim_id: None,
                    }],
                    section_checks: checks,
                },
            )
            .unwrap_err();
        assert!(matches!(error, BrainError::KnowledgeConflict(_)));
        assert_eq!(
            store
                .get_research_stage_content(&task, "synthesis", None)
                .unwrap()
                .stage
                .status,
            "running"
        );
    }

    #[test]
    fn test_research_failure_keeps_complete_report_and_late_worker_cannot_renew_or_finish() {
        let (store, _dir, base, task) = fixture();
        let attempt = store.research_attempt(&task).unwrap();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &run).unwrap();
        assert!(store
            .complete_research_task(&task, attempt, &report.content_md)
            .is_err());
        let claim = store.claim_research_stage(&task, "validation").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store.check_research_report(&claim, &run).unwrap();
        store
            .fail_research_task(&task, attempt, "模拟后续交付失败", true)
            .unwrap();
        let failed = store.get_task(&task).unwrap();
        assert_eq!(failed.status, "queued");
        assert!(failed.result_summary.ends_with(&report.content_md));
        store.request_task_cancel(&task).unwrap();
        store.start_task_execution(&task).unwrap();
        assert!(!store.renew_research_task_lease(&task, attempt).unwrap());
        assert!(store
            .fail_research_task(&task, attempt, "旧执行器失败", true)
            .is_err());
        assert!(store
            .complete_research_task(&task, attempt, &report.content_md)
            .is_err());
        let new_attempt = store.research_attempt(&task).unwrap();
        assert!(store.renew_research_task_lease(&task, new_attempt).unwrap());
        store
            .complete_research_task(&task, new_attempt, &report.content_md)
            .unwrap();
        assert_eq!(store.get_task(&task).unwrap().status, "completed");
    }

    #[test]
    fn test_artifact_write_cannot_escape_stage_attempt_or_revive_expired_lease() {
        let (store, _dir, base, task) = fixture();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET deliverable_type='presentation' WHERE id=?1",
                    [&task],
                )?;
                Ok(())
            })
            .unwrap();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store.assemble_research_report(&claim, &run).unwrap();
        let claim = store.claim_research_stage(&task, "validation").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store.check_research_report(&claim, &run).unwrap();
        let claim = store.claim_research_stage(&task, "presentation").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store
            .complete_agent_run(&run, &json!({"answer":"演示规划"}))
            .unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET lease_expires_at='2000-01-01' WHERE id=?1",
                    [&task],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(!store
            .renew_research_task_lease(&task, claim.attempt)
            .unwrap());
        assert!(store
            .save_artifact(
                &base,
                &task,
                &run,
                None,
                "迟到文件",
                "safe/file.pptx",
                "hash",
                1,
                "valid",
                "校验通过",
                &json!({}),
                &[]
            )
            .is_err());
        assert!(store.list_task_artifacts(&task).unwrap().is_empty());
        assert!(store.save_research_presentation(&claim, &run).is_err());
    }

    #[test]
    fn test_expired_research_tool_capability_is_rejected_before_payload_or_budget_consumption() {
        let (store, _dir, base, task) = fixture();
        store.ensure_research_workspace(&task).unwrap();
        let claim = store.claim_research_stage(&task, "plan").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let capability = store
            .issue_agent_run_capability(
                &run,
                std::slice::from_ref(&base),
                &["book_get_context".into()],
                300,
            )
            .unwrap();
        assert!(store
            .validate_agent_run_capability(&capability.token, "book_get_context")
            .is_ok());
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET lease_expires_at='2000-01-01' WHERE id=?1",
                    [&task],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(store
            .validate_agent_run_capability(&capability.token, "book_get_context")
            .is_err());
        assert!(store
            .consume_agent_tool_call(&capability.token, "book_get_context")
            .is_err());
        let count: i64 = store
            .db
            .with_connection(|conn| {
                Ok(conn.query_row(
                    "SELECT used_calls FROM agent_run_capabilities WHERE run_id=?1",
                    [&run],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_review_plan_freezes_real_claims_and_body_and_rejects_cross_base_baselines() {
        let (store, _dir, base, task) = fixture();
        store.ensure_research_workspace(&task).unwrap();
        let entry = store.list_entries(&base, None, None, 10).unwrap().remove(0);
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_entries SET entry_type='concept' WHERE id=?1",
                    [&entry.id],
                )?;
                conn.execute(
                    "UPDATE knowledge_tasks SET task_type='review' WHERE id=?1",
                    [&task],
                )?;
                for index in 0..30 {
                    conn.execute("INSERT INTO knowledge_claims(id,knowledge_base_id,entry_id,predicate,claim_text,created_at,updated_at) VALUES(?1,?2,?3,'supports',?4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",params![format!("baseline-claim-{index:02}"),base,entry.id,format!("旧版具体主张{index:02}")])?;
                }
                Ok(())
            })
            .unwrap();
        let mut scope = plan();
        scope.questions[0].target_entry_ids = vec![entry.id.clone()];
        let claim = store.claim_research_stage(&task, "plan").unwrap();
        let run = attach(&store, &base, &task, &claim);
        store
            .complete_agent_run(&run, &json!({"answer":"plan"}))
            .unwrap();
        let mut invalid = scope.clone();
        invalid.questions[0].target_entry_ids = vec!["entry-from-another-base".into()];
        assert!(store.save_research_plan(&claim, &run, &invalid).is_err());
        assert!(store
            .get_research_workspace(&task)
            .unwrap()
            .unwrap()
            .plan
            .is_none());
        store.save_research_plan(&claim, &run, &scope).unwrap();
        let baseline = store
            .read_research_baseline_page(&task, "mechanism", &entry.id, 0, 12000, 0)
            .unwrap();
        assert!(baseline["content_md"].as_str().unwrap().contains("正文"));
        assert_eq!(baseline["historical_baseline"], true);
        assert!(baseline.get("citation").is_none());
        assert_eq!(baseline["claim_count"], 30);
        assert_eq!(baseline["claims"].as_array().unwrap().len(), 16);
        assert_eq!(baseline["metadata_has_more"], true);
        let tail = store
            .read_research_baseline_page(&task, "mechanism", &entry.id, 0, 12000, 16)
            .unwrap();
        assert_eq!(tail["claims"][13]["claim_text"], "旧版具体主张29");
        assert_eq!(tail["metadata_has_more"], false);
        let source = store
            .read_research_baseline_source_page(&task, "mechanism", &entry.id, 0, 0, 12000)
            .unwrap();
        assert!(source["baseline_source_content"]
            .as_str()
            .unwrap()
            .contains("正文"));
        store.db.with_connection(|conn| {
            conn.execute("UPDATE knowledge_entries SET content_md='新版本正文',revision=revision+1 WHERE id=?1",[&entry.id])?;
            Ok(())
        }).unwrap();
        assert_eq!(
            baseline,
            store
                .read_research_baseline_page(&task, "mechanism", &entry.id, 0, 12000, 0)
                .unwrap()
        );
        assert!(store
            .read_research_baseline_page(&task, "boundary", &entry.id, 0, 12000, 0)
            .is_err());
        assert_eq!(
            store
                .get_research_workspace(&task)
                .unwrap()
                .unwrap()
                .baselines
                .len(),
            1
        );
        let section = store
            .claim_research_stage(&task, "section:mechanism")
            .unwrap();
        let run = attach(&store, &base, &task, &section);
        let token = store
            .issue_agent_run_capability(
                &run,
                std::slice::from_ref(&base),
                &["knowledge_get_research_baseline".into()],
                300,
            )
            .unwrap();
        let tool = crate::core::agent_tool_gateway::call_agent_knowledge_tool;
        assert_eq!(
            tool(
                &store,
                &token.token,
                "knowledge_get_research_baseline",
                json!({"entry_id":entry.id,"metadata_offset":16,"source_basis_index":0})
            )
            .unwrap()["claim_count"],
            30
        );
        assert!(tool(
            &store,
            &token.token,
            "knowledge_get_research_baseline",
            json!({"entry_id":"cross-book"})
        )
        .is_err());
        assert!(tool(
            &store,
            &token.token,
            "knowledge_get_research_baseline",
            json!({"entry_id":entry.id,"task_id":"spoofed"})
        )
        .is_err());
        assert!(store.list_agent_run_citations(&run).unwrap().is_empty());
        store
            .record_visible_agent_evidence(
                &run,
                "entry",
                &entry.id,
                &store.get_entry(&entry.id).unwrap().revision.to_string(),
                &json!({"content_md":"新版本正文"}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"checked [S1]"}))
            .unwrap();
        let mut finding = output();
        assert!(store
            .save_research_section(&section, &run, &finding)
            .is_err());
        finding.findings[0].baseline_entry_id = Some(entry.id);
        finding.findings[0].baseline_claim_id = Some("fake-claim".into());
        assert!(store
            .save_research_section(&section, &run, &finding)
            .is_err());
        finding.findings[0].baseline_claim_id = Some("baseline-claim-29".into());
        store
            .save_research_section(&section, &run, &finding)
            .unwrap();
    }

    #[test]
    fn test_report_candidate_creation_is_atomic_and_old_epoch_cannot_write_after_requeue() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        complete_section(&store, &base, &task, "section:mechanism");
        complete_section(&store, &base, &task, "section:boundary");
        let claim = store.claim_research_stage(&task, "report").unwrap();
        let run = attach(&store, &base, &task, &claim);
        let report = store.assemble_research_report(&claim, &run).unwrap();
        let candidate = json!({"entry_type":"synthesis","slug":"research-result","title":"研究结果","summary":"摘要","content_md":report.content_md,"aliases":[],"confidence":0.7,"citations":["span","span2"],"claims":[],"relations":[]});
        store
            .create_semantic_change_set(
                &base,
                &run,
                "研究结果",
                "待人工审核",
                "current-report",
                std::slice::from_ref(&candidate),
            )
            .unwrap();
        assert_eq!(
            store.get_task(&task).unwrap().knowledge_change_state,
            "proposed"
        );
        store.request_task_cancel(&task).unwrap();
        store.cancel_task_execution(&task).unwrap();
        store.start_task_execution(&task).unwrap();
        assert!(store
            .create_semantic_change_set(
                &base,
                &run,
                "迟到结果",
                "旧尝试",
                "late-report",
                &[candidate]
            )
            .is_err());
        assert_eq!(store.list_change_sets(&base, None).unwrap().len(), 1);
    }

    #[test]
    fn test_explicit_requeue_does_not_reuse_previous_worker_identity_when_retry_count_resets() {
        let (store, _dir, base, task) = fixture();
        store.ensure_research_workspace(&task).unwrap();
        let old = store.claim_research_stage(&task, "plan").unwrap();
        let run = attach(&store, &base, &task, &old);
        let capability = store
            .issue_agent_run_capability(
                &run,
                std::slice::from_ref(&base),
                &["book_get_context".into()],
                300,
            )
            .unwrap();
        store.request_task_cancel(&task).unwrap();
        store.cancel_task_execution(&task).unwrap();
        store.queue_task_execution(&task).unwrap();
        store
            .db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE knowledge_tasks SET next_attempt_at=NULL WHERE id=?1",
                    [&task],
                )?;
                Ok(())
            })
            .unwrap();
        store.claim_next_queued_task().unwrap().unwrap();
        assert!(store.research_attempt(&task).unwrap() > old.attempt);
        assert!(!store.renew_research_task_lease(&task, old.attempt).unwrap());
        assert!(store
            .validate_agent_run_capability(&capability.token, "book_get_context")
            .is_err());
        let new = store.claim_research_stage(&task, "plan").unwrap();
        assert_ne!(new.claim_id, old.claim_id);
        assert!(store
            .fail_research_stage(&old, "迟到旧回答", false)
            .is_err());
        store
            .fail_research_stage(&new, "新的暂时失败", false)
            .unwrap();
        store
            .fail_research_task(&task, new.attempt, "新一次失败", true)
            .unwrap();
        assert_eq!(store.get_task(&task).unwrap().status, "queued");
    }

    #[test]
    fn test_stage_failure_resume_preserves_completed_sections_and_exact_evidence() {
        let (store, _dir, base, task) = fixture();
        assert!(store.get_research_workspace(&task).unwrap().is_none());
        setup_plan(&store, &base, &task);
        let first = store
            .claim_research_stage(&task, "section:mechanism")
            .unwrap();
        let run = attach(&store, &base, &task, &first);
        store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "span",
                "version-doc",
                &json!({"content_md":"可追溯的来源正文。"}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"研究完成"}))
            .unwrap();
        store
            .save_research_section(&first, &run, &output())
            .unwrap();
        let saved = store
            .get_research_stage_content(&task, "section:mechanism", None)
            .unwrap();
        assert!(saved.content_md.contains("LATE_CONCLUSION"));
        assert_eq!(saved.evidence[0].run_id, run);
        assert_eq!(saved.evidence[0].citation_index, 1);
        assert!(store
            .claim_research_stage(&task, "section:mechanism")
            .is_err());

        let second = store
            .claim_research_stage(&task, "section:boundary")
            .unwrap();
        store
            .fail_research_stage(&second, "暂时的模型错误", false)
            .unwrap();
        store.fail_task_execution(&task, "阶段二失败").unwrap();
        store.start_task_execution(&task).unwrap();
        store.ensure_research_workspace(&task).unwrap();
        let resumed = store
            .claim_research_stage(&task, "section:boundary")
            .unwrap();
        assert_ne!(resumed.claim_id, second.claim_id);
        assert!(store
            .fail_research_stage(&second, "迟到写入", false)
            .is_err());
        assert_eq!(
            store
                .get_research_stage_content(&task, "section:mechanism", None)
                .unwrap()
                .content_md,
            saved.content_md
        );
        let workspace = store.get_research_workspace(&task).unwrap().unwrap();
        assert_eq!(workspace.plan.unwrap(), plan());
        assert_eq!(
            workspace
                .stages
                .iter()
                .filter(|s| s.status == "completed")
                .count(),
            2
        );
    }

    #[test]
    fn test_stale_source_invalidates_only_affected_deliverables_without_erasing_history() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        let first = store
            .claim_research_stage(&task, "section:mechanism")
            .unwrap();
        let run = attach(&store, &base, &task, &first);
        store
            .record_visible_agent_evidence(
                &run,
                "source_span",
                "span",
                "version-doc",
                &json!({"content_md":"可追溯的来源正文。"}),
            )
            .unwrap();
        store
            .complete_agent_run(&run, &json!({"answer":"研究完成"}))
            .unwrap();
        store
            .save_research_section(&first, &run, &output())
            .unwrap();
        let saved = store
            .get_research_stage_content(&task, "section:mechanism", None)
            .unwrap();
        store.sync_markdown_sources(&base, &[]).unwrap();
        assert_eq!(
            store.invalidate_changed_research_evidence(&task).unwrap(),
            1
        );
        let current = store
            .get_research_stage_content(&task, "section:mechanism", None)
            .unwrap();
        assert_eq!(current.stage.status, "stale");
        assert_eq!(current.content_md, saved.content_md);
        assert_eq!(
            store
                .get_research_stage_content(&task, "section:mechanism", Some(saved.stage.revision))
                .unwrap()
                .content_md,
            saved.content_md
        );
        assert_eq!(
            store
                .get_research_workspace(&task)
                .unwrap()
                .unwrap()
                .stages
                .iter()
                .find(|s| s.stage_key == "plan")
                .unwrap()
                .status,
            "completed"
        );
        assert!(store
            .get_research_stage_content(&task, "section:mechanism", Some(999))
            .is_err());
    }

    #[test]
    fn test_stage_rejects_unread_cross_run_late_lease_cancel_and_invalid_plan() {
        let (store, _dir, base, task) = fixture();
        setup_plan(&store, &base, &task);
        let first = store
            .claim_research_stage(&task, "section:mechanism")
            .unwrap();
        let run = attach(&store, &base, &task, &first);
        store
            .complete_agent_run(&run, &json!({"answer":"未读取任何正文"}))
            .unwrap();
        assert!(store
            .save_research_section(&first, &run, &output())
            .is_err());
        let other = store
            .start_agent_run(&base, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        assert!(store.attach_research_stage_run(&first, &other.id).is_err());
        let mut invalid = plan();
        invalid.questions[1].id = "mechanism".into();
        assert!(validate_plan(&invalid).is_err());
        assert!(store.claim_research_stage(&task, "unknown").is_err());
        store.db.with_connection(|conn| {conn.execute("UPDATE knowledge_tasks SET lease_expires_at='2000-01-01T00:00:00Z' WHERE id=?1",[&task])?; Ok(())}).unwrap();
        assert!(store
            .fail_research_stage(&first, "过期执行器", false)
            .is_err());
        store.db.with_connection(|conn| {conn.execute("UPDATE knowledge_tasks SET lease_expires_at=datetime('now','+1 hour'),cancel_requested=1 WHERE id=?1",[&task])?; Ok(())}).unwrap();
        assert!(store
            .save_research_section(&first, &run, &output())
            .is_err());
        assert!(store
            .claim_research_stage(&task, "section:boundary")
            .is_err());
    }
}
