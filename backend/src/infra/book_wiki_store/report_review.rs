use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReportReviewEdit {
    pub start: usize,
    pub end: usize,
    pub expected: String,
    pub replacement: String,
    pub source_index: usize,
}

pub(crate) struct ReportReviewApproval {
    pub report_run_id: String,
    pub runs: Vec<String>,
    pub reviewed_blocks: usize,
    pub excerpts_limited: bool,
    pub edits: Vec<ReportReviewEdit>,
}

pub(crate) fn apply_report_review_edits(
    markdown: &str,
    edits: &[ReportReviewEdit],
) -> Result<String, BrainError> {
    let mut result = String::with_capacity(markdown.len());
    let mut offset = 0;
    for edit in edits {
        if offset > edit.start
            || edit.start >= edit.end
            || markdown.get(edit.start..edit.end) != Some(edit.expected.as_str())
        {
            return Err(invalid("报告审稿的修改位置重叠或原文段落已改变"));
        }
        result.push_str(&markdown[offset..edit.start]);
        result.push_str(&edit.replacement);
        offset = edit.end;
    }
    result.push_str(&markdown[offset..]);
    let mut available = body_indices(markdown)?;
    available.extend(edits.iter().map(|edit| edit.source_index));
    if !report_markup(markdown).eq(report_markup(&result))
        || !body_indices(&result)?.is_subset(&available)
    {
        return Err(invalid("报告审稿的局部修改改变了章节结构或正式引用"));
    }
    Ok(result)
}

fn report_markup(markdown: &str) -> impl Iterator<Item = Event<'_>> {
    Parser::new_ext(markdown, Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES).filter(|event| {
        !matches!(
            event,
            Event::Text(_)
                | Event::Start(Tag::Emphasis | Tag::Strong)
                | Event::End(pulldown_cmark::TagEnd::Emphasis | pulldown_cmark::TagEnd::Strong)
        )
    })
}

fn current_review_report(
    conn: &Connection,
    claim: &ResearchStageClaim,
    validation_run: &str,
    expected_report: Option<&str>,
) -> Result<(String, ResearchStageContent), BrainError> {
    let (base, kind) = owned_stage(conn, claim, false)?;
    if kind != "validation" {
        return Err(invalid("报告审稿必须在交付校验阶段执行"));
    }
    check_run(conn, claim, validation_run, false)?;
    let report = read_content(conn, &claim.task_id, "report")?;
    if report.stage.status != "completed"
        || expected_report
            .is_some_and(|expected| report.content_run_id.as_deref() != Some(expected))
    {
        return Err(conflict("报告已更新，拒绝在旧稿上继续审稿"));
    }
    for reference in &report.evidence {
        if !current_reference(conn, &base, reference)? {
            return Err(conflict("报告的原文依据已变化，请复核对应章节"));
        }
    }
    let visible = report
        .evidence
        .iter()
        .map(|reference| reference.citation_index)
        .collect::<HashSet<_>>();
    if !body_indices(&report.content_md)?.is_subset(&visible) {
        return Err(invalid("报告存在未实际读取的引用"));
    }
    Ok((base, report))
}

impl BookWikiStore {
    pub(crate) fn ensure_report_review_is_current(
        &self,
        claim: &ResearchStageClaim,
        validation_run: &str,
        expected_report: &str,
    ) -> Result<(), BrainError> {
        self.db
            .with_connection(|conn| {
                current_review_report(conn, claim, validation_run, Some(expected_report))
            })
            .map(|_| ())
    }
}

pub(super) fn report_for_validation(
    conn: &Connection,
    claim: &ResearchStageClaim,
    validation_run: &str,
    review: Option<&ReportReviewApproval>,
    now: &str,
) -> Result<ResearchStageContent, BrainError> {
    let (base, mut report) = current_review_report(
        conn,
        claim,
        validation_run,
        review.map(|review| review.report_run_id.as_str()),
    )?;
    let Some(review) = review else {
        return Ok(report);
    };
    if review.reviewed_blocks == 0 || review.runs.is_empty() {
        return Err(invalid("缺少可回看的报告审稿记录"));
    }
    let mut confirmed_patch = false;
    let mut seen = HashSet::new();
    for run in &review.runs {
        let round: Option<i64> = conn
            .query_row(
                "SELECT json_extract(input_json,'$.report_review_round') FROM agent_runs
                 WHERE id=?1 AND status='completed' AND task_type='knowledge_task_report_review'
                   AND knowledge_base_id=?2 AND json_extract(input_json,'$.knowledge_task_id')=?3
                   AND json_extract(input_json,'$.validation_run_id')=?4
                   AND json_extract(input_json,'$.reviewed_report_run_id')=?5",
                params![
                    run,
                    base,
                    claim.task_id,
                    validation_run,
                    review.report_run_id
                ],
                |row| row.get(0),
            )
            .optional()?;
        if !matches!(round, Some(1..=3)) || !seen.insert(run) {
            return Err(conflict("独立审稿记录和当前报告或研究阶段不匹配"));
        }
        confirmed_patch |= round == Some(2);
    }
    if review.edits.is_empty() {
        return Ok(report);
    }
    if !confirmed_patch {
        return Err(invalid("局部修订需要通过第二轮独立核对才能发布"));
    }

    let old_run = &review.report_run_id;
    let mut output: Value = decode(&conn.query_row(
        "SELECT output_json FROM agent_runs WHERE id=?1 AND task_type='knowledge_task_report'",
        [old_run],
        |row| row.get::<_, String>(0),
    )?)?;
    if output["authored_report"] != true {
        return Err(invalid("早期报告缺少独立的成稿章节和版本，不能自动修订"));
    }
    let body = apply_report_review_edits(&report.content_md, &review.edits)?;
    let frozen = report
        .evidence
        .iter()
        .map(|reference| reference.citation_index)
        .collect();
    if !body_indices(&body)?.is_subset(&frozen) {
        return Err(invalid("报告审稿修订须使用当前报告已读取的原文"));
    }
    update_editorial_ranges(&mut output, &mut report.findings, &review.edits)?;
    output["answer"] = json!(body);
    output["amended_report_run_id"] = json!(old_run);
    output["report_review_runs"] = json!(review.runs);
    output["billing_usage"] = json!("not_applicable");

    let revised_run = uuid::Uuid::new_v4().to_string();
    let input = json!({"knowledge_task_id":claim.task_id,"research_stage_key":"report",
        "reviewed_report_run_id":old_run,"validation_run_id":validation_run,"application_phase":true});
    conn.execute(
        "INSERT INTO agent_runs(id,knowledge_base_id,runtime,task_type,status,input_json,output_json,started_at,finished_at,created_at)
         VALUES(?1,?2,'application','knowledge_task_report','completed',?3,?4,?5,?5,?5)",
        params![revised_run, base, encode(&input)?, encode(&output)?, now],
    )?;
    conn.execute(
        "INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at)
         SELECT ?1,citation_index,kind,object_id,version_id,snapshot_json,?3 FROM agent_run_citations WHERE run_id=?2",
        params![revised_run, old_run, now],
    )?;
    conn.execute(
        "INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at)
         SELECT ?1,kind,object_id,version_id,snapshot_json,?3 FROM agent_run_evidence WHERE run_id=?2",
        params![revised_run, old_run, now],
    )?;
    for reference in &mut report.evidence {
        reference.run_id = revised_run.clone();
    }
    preserve(conn, &claim.task_id, "report", now)?;
    conn.execute(
        "UPDATE knowledge_research_stages SET content_md=?2,findings_json=?3,evidence_json=?4,run_id=?5,content_run_id=?5,revision=revision+1,updated_at=?6 WHERE task_id=?1 AND stage_key='report'",
        params![claim.task_id, body, encode(&report.findings)?, encode(&report.evidence)?, revised_run, now],
    )?;
    preserve(conn, &claim.task_id, "report", now)?;
    let presentation_ready: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM knowledge_research_stages WHERE task_id=?1 AND stage_key='presentation' AND status='completed')",
        [&claim.task_id],
        |row| row.get(0),
    )?;
    if presentation_ready {
        preserve(conn, &claim.task_id, "presentation", now)?;
        conn.execute(
            "UPDATE knowledge_research_stages SET status='stale',error=NULL,revision=revision+1,updated_at=?2 WHERE task_id=?1 AND stage_key='presentation'",
            params![claim.task_id, now],
        )?;
    }
    insert_agent_run_event(
        conn,
        &revised_run,
        "run.completed",
        Some("completed"),
        "报告已核对并保存局部修订",
        &encode(&json!({"source_report_run":old_run,"edits":review.edits.len()}))?,
        now,
    )?;
    read_content(conn, &claim.task_id, "report")
}

fn update_editorial_ranges(
    output: &mut Value,
    findings: &mut [ResearchFinding],
    edits: &[ReportReviewEdit],
) -> Result<(), BrainError> {
    let invalid_range = || invalid("报告章节边界无法应用独立审稿修订");
    let ranges = output["section_ranges"]
        .as_array_mut()
        .ok_or_else(invalid_range)?;
    for range in ranges {
        let offset = |key| {
            range[key]
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(invalid_range)
        };
        let (start, end) = (offset("byte_start")?, offset("byte_end")?);
        let finding_range = offset("finding_start")?..offset("finding_end")?;
        let section_findings = findings.get_mut(finding_range).ok_or_else(invalid_range)?;
        let shifted = |boundary: usize| {
            edits
                .iter()
                .filter(|edit| edit.end <= boundary)
                .try_fold(boundary, |sum, edit| {
                    sum.checked_add(edit.replacement.len())?
                        .checked_sub(edit.expected.len())
                })
                .ok_or_else(invalid_range)
        };
        range["byte_start"] = json!(shifted(start)?);
        range["byte_end"] = json!(shifted(end)?);
        for edit in edits
            .iter()
            .filter(|edit| start < edit.end && edit.start < end)
        {
            if edit.start < start || edit.end > end {
                return Err(invalid_range());
            }
            for key in ["title", "summary"] {
                if let Some(text) = range[key].as_str() {
                    range[key] = json!(text.replace(&edit.expected, &edit.replacement));
                }
            }
            for finding in section_findings.iter_mut() {
                finding.finding = finding.finding.replace(&edit.expected, &edit.replacement);
                for limitation in &mut finding.limitations {
                    *limitation = limitation.replace(&edit.expected, &edit.replacement);
                }
            }
        }
    }
    Ok(())
}
