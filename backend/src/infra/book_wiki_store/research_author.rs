use super::*;
use crate::models::book_wiki::ResearchSectionCheck;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResearchManuscript {
    pub summary: String,
    pub chapters: Vec<ResearchChapter>,
    pub section_checks: Vec<ResearchSectionCheck>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct ResearchChapter {
    pub title: String,
    pub summary: String,
    pub content_md: String,
    pub findings: Vec<ResearchFinding>,
}

#[derive(Deserialize, PartialEq, Serialize)]
pub(crate) struct ResearchAuthorCheckpoint {
    pub generation: String,
    pub slot: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct ResearchFrozenRange {
    question_id: String,
    origin: ResearchEvidenceReference,
    segment: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    start: usize,
    characters: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

pub(crate) struct ResearchAuthorSource {
    range: ResearchFrozenRange,
    path: String,
    heading: String,
    offset: u64,
    content: String,
}

impl ResearchAuthorSource {
    pub(crate) fn identity(&self) -> (&str, &str, &str) {
        let origin = &self.range.origin;
        (&origin.kind, &origin.object_id, &origin.version_id)
    }

    pub(crate) fn card(&self, index: usize) -> Value {
        json!({
            "citation": format!("[S{index}]"),
            "note_id": self.range.question_id,
            "note_citation_index": self.range.origin.citation_index,
            "kind": self.range.origin.kind,
            "path": self.path,
            "heading": self.heading,
            "version_id": self.range.origin.version_id,
            "offset_chars": self.offset,
            "content_md": self.content,
        })
    }

    pub(crate) fn receipt(&self) -> Value {
        json!(self.range)
    }
}

pub(super) fn publish_authored_sections(
    report: &mut String,
    manuscript: &str,
    ranges: &[Value],
    mapping: &HashMap<usize, usize>,
) -> Result<Vec<Value>, BrainError> {
    let mut end = 0;
    let mut published = Vec::with_capacity(ranges.len());
    for range in ranges {
        let start = range["byte_start"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok());
        let next_end = range["byte_end"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok());
        if start != Some(end) {
            return Err(invalid("成稿章节边界不连续"));
        }
        let next_end = next_end.ok_or_else(|| invalid("成稿章节缺少结束位置"))?;
        let section = manuscript
            .get(end..next_end)
            .filter(|section| section.starts_with("## "))
            .ok_or_else(|| invalid("成稿章节缺少完整标题或正文"))?;
        let mut published_range = range.clone();
        published_range["byte_start"] = json!(report.len());
        report.push_str(&relabel(section, mapping)?);
        published_range["byte_end"] = json!(report.len());
        published_range["summary"] = json!(relabel(
            range["summary"].as_str().unwrap_or_default(),
            mapping
        )?);
        published.push(published_range);
        end = next_end;
    }
    if end != manuscript.len() || published.is_empty() {
        return Err(invalid("完整成稿未包含在章节边界"));
    }
    Ok(published)
}

const MAX_SOURCE_CARDS: usize = 96;
const SOURCE_CARD_CHARACTERS: usize = 2400;

fn read_origin(
    conn: &Connection,
    reference: &ResearchEvidenceReference,
) -> Result<Value, BrainError> {
    let snapshot: String = conn.query_row(
        "SELECT snapshot_json FROM agent_run_citations WHERE run_id=?1 AND citation_index=?2",
        params![reference.run_id, reference.citation_index],
        |row| row.get(0),
    )?;
    decode(&snapshot)
}

fn checked_segment(snapshot: &Value, range: &ResearchFrozenRange) -> Result<Value, BrainError> {
    let segment = &snapshot["segments"][range.segment];
    let offset = segment["offset_chars"]
        .as_u64()
        .ok_or_else(|| invalid("取证片段缺少原文位置"))?;
    let content = segment["content"]
        .as_str()
        .ok_or_else(|| invalid("取证片段缺少已读正文"))?;
    let excerpt = content
        .chars()
        .skip(range.start)
        .take(range.characters)
        .collect::<String>();
    if !(1..=SOURCE_CARD_CHARACTERS).contains(&range.characters)
        || excerpt.chars().count() != range.characters
        || excerpt.trim().is_empty()
    {
        return Err(conflict("成稿所选的证据范围与取证快照不一致"));
    }
    let offset = offset
        .checked_add(range.start as u64)
        .ok_or_else(|| invalid("取证片段位置溢出"))?;
    Ok(json!({"offset_chars": offset, "content": excerpt}))
}

fn author_generation(conn: &Connection, task: &str) -> Result<String, BrainError> {
    let (base, plan, brief): (String, String, String) = conn.query_row(
        "SELECT w.knowledge_base_id,w.plan_json,t.brief_json
         FROM knowledge_research_workspaces w JOIN knowledge_tasks t ON t.id=w.task_id
         WHERE w.task_id=?1",
        [task],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let saved_plan: ResearchPlan = decode(&plan)?;
    let notes = conn
        .prepare(
            "SELECT stage_key,revision,content_run_id FROM knowledge_research_stages
             WHERE task_id=?1 AND kind='section' AND status='completed' AND content_run_id IS NOT NULL
             ORDER BY stage_key",
        )?
        .query_map([task], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    if notes.len() != saved_plan.questions.len() {
        return Err(conflict("取证阶段已变化，不能复用长稿"));
    }
    let config = conn
        .prepare("SELECT id,revision FROM knowledge_config_documents WHERE scope='global' OR knowledge_base_id=?1 ORDER BY id")?
        .query_map([base], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stable_id(
        "research-long-form-v1",
        &encode(&(plan, brief, notes, config))?,
    ))
}

fn owned_author_run(
    conn: &Connection,
    claim: &ResearchStageClaim,
    run: &str,
    checkpoint: &ResearchAuthorCheckpoint,
) -> Result<String, BrainError> {
    let (base, _) = owned_stage(conn, claim, false)?;
    check_run(conn, claim, run, true)?;
    let raw: String = conn.query_row(
        "SELECT json_extract(input_json,'$.research_author_checkpoint') FROM agent_runs WHERE id=?1",
        [run],
        |row| row.get(0),
    )?;
    if claim.stage_key != "synthesis"
        || decode::<ResearchAuthorCheckpoint>(&raw)? != *checkpoint
        || author_generation(conn, &claim.task_id)? != checkpoint.generation
    {
        return Err(conflict("长稿成稿所用取证或编辑配置已变化"));
    }
    Ok(base)
}

fn chapter_output(chapter: &ResearchChapter) -> Result<ResearchSectionOutput, BrainError> {
    if !text(chapter.title.trim(), 160)
        || chapter.title.contains(['\r', '\n'])
        || chapter.title.trim_start().starts_with('#')
        || !text(&chapter.summary, 2_000)
        || !text(&chapter.content_md, 120_000)
        || !(1..=64).contains(&chapter.findings.len())
    {
        return Err(invalid(
            "报告章节需要单行标题、完整正文、摘要和可回放的发现",
        ));
    }
    let indices = body_indices(&chapter.content_md)?;
    if chapter.findings.iter().any(|finding| {
        finding.baseline_entry_id.is_some()
            || finding.baseline_claim_id.is_some()
            || (matches!(finding.status.as_str(), "supported" | "conflict")
                && !finding
                    .citation_indices
                    .iter()
                    .any(|index| indices.contains(index)))
    }) {
        return Err(invalid(
            "成稿发现须对应本节的原文引用，历史核验保留在取证笔记",
        ));
    }
    Ok(ResearchSectionOutput {
        summary: chapter.summary.clone(),
        content_md: chapter.content_md.clone(),
        findings: chapter.findings.clone(),
    })
}

impl BookWikiStore {
    pub(crate) fn research_author_generation(&self, task: &str) -> Result<String, BrainError> {
        self.db
            .with_connection(|conn| author_generation(conn, task))
    }

    pub(crate) fn get_research_author_checkpoint<T: DeserializeOwned>(
        &self,
        task: &str,
        checkpoint: &ResearchAuthorCheckpoint,
    ) -> Result<Option<(String, T)>, BrainError> {
        self.db.with_connection(|conn| {
            let saved: Option<(String, String, String, String)> = conn.query_row(
                "SELECT r.id,r.knowledge_base_id,json_extract(r.output_json,'$.research_author_deliverable'),
                        json_extract(r.output_json,'$.research_author_evidence')
                 FROM agent_runs r JOIN knowledge_tasks t ON t.id=?1 AND t.knowledge_base_id=r.knowledge_base_id
                 WHERE r.status='completed' AND json_extract(r.input_json,'$.knowledge_task_id')=?1
                   AND json_extract(r.input_json,'$.research_stage_key')='synthesis'
                   AND json_extract(r.input_json,'$.research_author_checkpoint.generation')=?2
                   AND json_extract(r.input_json,'$.research_author_checkpoint.slot')=?3
                   AND json_type(r.output_json,'$.research_author_deliverable')='object'
                 ORDER BY r.rowid DESC LIMIT 1",
                params![task, checkpoint.generation, checkpoint.slot],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            ).optional()?;
            let Some((run, base, raw, raw_evidence)) = saved else { return Ok(None) };
            for reference in decode::<Vec<ResearchEvidenceReference>>(&raw_evidence)? {
                if !current_reference(conn, &base, &reference)? {
                    return Ok(None);
                }
            }
            Ok(Some((run, decode(&raw)?)))
        })
    }

    pub(crate) fn latest_truncated_research_author_run(
        &self,
        task: &str,
        checkpoint: &ResearchAuthorCheckpoint,
    ) -> Result<Option<String>, BrainError> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id FROM agent_runs
                 WHERE json_extract(input_json,'$.knowledge_task_id')=?1
                   AND json_extract(input_json,'$.research_stage_key')='synthesis'
                   AND json_extract(input_json,'$.research_author_checkpoint.generation')=?2
                   AND json_extract(input_json,'$.research_author_checkpoint.slot')=?3
                   AND error LIKE '%stop_reason=max_tokens%'
                 ORDER BY rowid DESC LIMIT 1",
                params![task, checkpoint.generation, checkpoint.slot],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub(crate) fn read_research_author_chapter_page(
        &self,
        task: &str,
        generation: &str,
        chapter: &str,
        offset: usize,
        max_chars: usize,
    ) -> Result<Value, BrainError> {
        if self.research_author_generation(task)? != generation {
            return Err(conflict("编辑时取证或写作配置已变化"));
        }
        let checkpoint = ResearchAuthorCheckpoint {
            generation: generation.to_owned(),
            slot: format!("chapter:{chapter}"),
        };
        let (_, chapter) = self
            .get_research_author_checkpoint::<ResearchChapter>(task, &checkpoint)?
            .ok_or_else(|| invalid("当前大纲下没有完成这一节"))?;
        let body = neutral_section_text(&chapter.content_md)?;
        let total = body.chars().count();
        let from = offset.min(total);
        let limit = max_chars.clamp(1, 12_000);
        Ok(json!({
            "title": chapter.title,
            "content_md": body.chars().skip(from).take(limit).collect::<String>(),
            "offset_chars": from,"total_chars": total,"has_more": from.saturating_add(limit)<total,
            "notice":"供全文编辑检查结构与重复，章节原有编号会在合并后重排",
        }))
    }

    pub(crate) fn save_research_author_checkpoint(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        checkpoint: &ResearchAuthorCheckpoint,
        content: &impl Serialize,
        chapter: Option<&ResearchChapter>,
        progress: &str,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let base = owned_author_run(conn, claim, run, checkpoint)?;
            let evidence = chapter
                .map(|chapter| section_evidence(conn, &base, run, &chapter_output(chapter)?))
                .transpose()?
                .unwrap_or_default();
            conn.execute(
                "UPDATE agent_runs SET output_json=json_set(output_json,'$.research_author_deliverable',json(?2),
                    '$.research_author_evidence',json(?3))
                 WHERE id=?1",
                params![run, encode(content)?, encode(&evidence)?],
            )?;
            preserve(conn, &claim.task_id, &claim.stage_key, &now)?;
            conn.execute(
                "UPDATE knowledge_research_stages SET status='pending',summary=?3,claim_id=NULL,claimed_attempt=NULL,
                 revision=revision+1,updated_at=?4 WHERE task_id=?1 AND stage_key=?2",
                params![claim.task_id, claim.stage_key, progress, now],
            )?;
            conn.execute("UPDATE knowledge_research_workspaces SET updated_at=?2 WHERE task_id=?1",
                params![claim.task_id, now])?;
            Ok(())
        })
    }

    /// Compose the final editor Run's one citation namespace from the chapter
    /// Runs and retain all ranges and their original provenance.
    pub(crate) fn rebase_research_author_chapters(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        checkpoint: &ResearchAuthorCheckpoint,
        chapters: &mut [(String, ResearchChapter)],
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        self.db.transaction(|conn| {
            let base = owned_author_run(conn, claim, run, checkpoint)?;
            let mut indices = HashMap::new();
            let mut citations: Vec<(ResearchEvidenceReference, Value)> = Vec::new();
            for (chapter_run, chapter) in chapters {
                let valid: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE id=?1
                    AND knowledge_base_id=?2 AND status='completed'
                    AND json_extract(input_json,'$.knowledge_task_id')=?3
                    AND json_extract(input_json,'$.research_author_checkpoint.generation')=?4
                    AND json_extract(input_json,'$.research_author_checkpoint.slot') LIKE 'chapter:%'
                    AND json_type(output_json,'$.research_author_deliverable')='object')",
                    params![chapter_run.as_str(), base, claim.task_id, checkpoint.generation], |row| row.get(0))?;
                if !valid {
                    return Err(conflict("完整报告的章节不属于当前大纲与取证版本"));
                }
                let mut mapping = HashMap::new();
                let output = chapter_output(chapter)?;
                for origin in section_evidence(conn, &base, chapter_run, &output)? {
                    let identity = (origin.kind.clone(), origin.object_id.clone(), origin.version_id.clone());
                    let raw = read_origin(conn, &origin)?;
                    let next = citations.len();
                    let position = *indices.entry(identity).or_insert(next);
                    mapping.insert(origin.citation_index, position + 1);
                    if position == next {
                        citations.push((origin.clone(), raw.clone()));
                    }
                    let preserved = &mut citations[position].1;
                    for name in ["segments", "source_span_ids", "research_origins"] {
                        for value in raw[name].as_array().into_iter().flatten() {
                            append_snapshot_value(preserved, name, value.clone())?;
                        }
                    }
                    let provenance = json!({
                        "mode":"research_author_chapter",
                        "run_id":origin.run_id,
                        "citation_index":origin.citation_index,
                        "chapter":chapter.title,
                    });
                    append_snapshot_value(preserved, "research_origins", provenance.clone())?;
                    conn.execute("INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at)
                        VALUES (?1,?2,?3,?4,?5,?6)",params![run, origin.kind, origin.object_id, origin.version_id,
                        encode(&json!({"citation_index":position+1,"origin":provenance}))?, now])?;
                }
                chapter.content_md = relabel(&output.content_md, &mapping)?;
                chapter.summary = relabel(&output.summary, &mapping)?;
                for finding in &mut chapter.findings {
                    finding.finding = relabel(&finding.finding, &mapping)?;
                    for limitation in &mut finding.limitations {
                        *limitation = relabel(limitation, &mapping)?;
                    }
                    for index in &mut finding.citation_indices {
                        *index = *mapping.get(index).ok_or_else(|| invalid("成稿章节引用缺少原文范围"))?;
                    }
                }
            }
            for (index, (origin, snapshot)) in citations.into_iter().enumerate() {
                conn.execute("INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at)
                    VALUES (?1,?2,?3,?4,?5,?6,?7)",params![run,index+1,origin.kind,origin.object_id,origin.version_id,encode(&snapshot)?,now])?;
            }
            Ok(())
        })
    }

    /// Sample every question before seeking additional passages from any one.
    /// Only ranges actually opened during the question's earlier run qualify.
    pub(crate) fn research_author_sources(
        &self,
        task: &str,
    ) -> Result<Vec<ResearchAuthorSource>, BrainError> {
        self.research_author_sources_in(task, None)
    }

    pub(crate) fn research_author_sources_in(
        &self,
        task: &str,
        questions: Option<&[String]>,
    ) -> Result<Vec<ResearchAuthorSource>, BrainError> {
        self.db.with_connection(|conn| {
            let (base, plan): (String, String) = conn.query_row(
                "SELECT knowledge_base_id,plan_json FROM knowledge_research_workspaces WHERE task_id=?1",
                [task],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let plan: ResearchPlan = decode(&plan)?;
            let notes = plan.questions.iter().filter(|question| questions
                .is_none_or(|ids| ids.contains(&question.id))).map(|question| {
                let mut note = read_content(conn, task, &format!("section:{}", question.id))?;
                if note.stage.status != "completed" {
                    return Err(conflict("取证尚未完成，不能冻结成稿依据"));
                }
                // Direct excerpts are the strongest seeds. Keep earlier entry
                // evidence available for tasks started before topic Wiki.
                note.evidence.sort_by_key(|reference| reference.kind == "entry");
                Ok((question.id.as_str(), note.evidence.into_iter()))
            }).collect::<Result<Vec<_>, BrainError>>()?;
            let mut notes = notes;
            let mut sources = Vec::new();
            let mut seen = HashSet::new();
            while sources.len() < MAX_SOURCE_CARDS {
                let mut advanced = false;
                for (question_id, note) in &mut notes {
                    let Some(origin) = note.next() else { continue };
                    advanced = true;
                    if !current_reference(conn, &base, &origin)? {
                        return Err(conflict("成稿前原文已更新，请重试受影响的取证阶段"));
                    }
                    let snapshot = read_origin(conn, &origin)?;
                    let title = snapshot["entry"]["title"].as_str().unwrap_or_default();
                    let path = snapshot["entry"]["source_path"].as_str().unwrap_or(title);
                    let segments = snapshot["segments"].as_array().ok_or_else(|| invalid("取证快照缺少已读范围"))?;
                    let max_segments = if questions.is_some() { usize::MAX } else { 2 };
                    for (segment, value) in segments.iter().enumerate().take(max_segments) {
                        let Some(content) = value["content"].as_str() else { continue };
                        let Some(offset) = value["offset_chars"].as_u64() else { continue };
                        let length = content.chars().count();
                        let total = if questions.is_some() { length } else { length.min(SOURCE_CARD_CHARACTERS) };
                        // Make later material after a long source section's
                        // introduction available to the chapter author as well.
                        for start in (0..total).step_by(SOURCE_CARD_CHARACTERS) {
                            let excerpt = content.chars().skip(start).take(SOURCE_CARD_CHARACTERS).collect::<String>();
                            let absolute = offset.checked_add(start as u64).ok_or_else(|| invalid("取证片段位置溢出"))?;
                            let identity = (origin.kind.clone(), origin.object_id.clone(), origin.version_id.clone(), absolute, excerpt.clone());
                            if excerpt.trim().is_empty() || !seen.insert(identity) {
                                continue;
                            }
                            sources.push(ResearchAuthorSource {
                                range: ResearchFrozenRange {
                                    question_id: question_id.to_string(),
                                    origin: origin.clone(),
                                    segment,
                                    start,
                                    characters: excerpt.chars().count(),
                                },
                                path: path.into(),
                                heading: title.into(),
                                offset: absolute,
                                content: excerpt,
                            });
                            if sources.len() == MAX_SOURCE_CARDS {
                                return Ok(sources);
                            }
                        }
                    }
                }
                if !advanced {
                    break;
                }
            }
            Ok(sources)
        })
    }

    /// Seed the numbered ledger with exactly the excerpts supplied to the
    /// writer, retaining both the original run and the original page range.
    pub(crate) fn import_research_author_sources(
        &self,
        run: &str,
        task: &str,
        receipts: &Value,
    ) -> Result<(), BrainError> {
        let now = Utc::now().to_rfc3339();
        let receipts: Vec<ResearchFrozenRange> = decode(&encode(receipts)?)?;
        if receipts.len() > MAX_SOURCE_CARDS {
            return Err(invalid("成稿预载证据超出安全范围"));
        }
        self.db.transaction(|conn| {
            validate_run_lease(conn, run)?;
            let (base, key, task_id): (String, String, String) = conn.query_row(
                "SELECT knowledge_base_id,json_extract(input_json,'$.research_stage_key'),json_extract(input_json,'$.knowledge_task_id') FROM agent_runs WHERE id=?1",
                [run],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            if key != "synthesis" || task_id != task {
                return Err(conflict("只有当前任务成稿运行可以引用冻结取证"));
            }
            let mut indices = HashMap::new();
            let mut snapshots = Vec::<(&ResearchEvidenceReference, Value)>::new();
            for receipt in &receipts {
                let origin = &receipt.origin;
                let note = read_content(conn, task, &format!("section:{}", receipt.question_id))?;
                if note.stage.status != "completed"
                    || !note.evidence.contains(origin)
                    || !current_reference(conn, &base, origin)?
                {
                    return Err(conflict("成稿取证快照已失效，拒绝提交旧依据"));
                }
                let raw = read_origin(conn, origin)?;
                let segment = checked_segment(&raw, receipt)?;
                let identity = (&origin.kind, &origin.object_id, &origin.version_id);
                let next = snapshots.len();
                let position = *indices.entry(identity).or_insert(next);
                if position == next {
                    let mut snapshot = raw;
                    snapshot["segments"] = json!([]);
                    snapshot["research_origins"] = json!([]);
                    snapshots.push((origin, snapshot));
                }
                let snapshot = &mut snapshots[position].1;
                let segments = snapshot["segments"].as_array_mut().ok_or_else(|| invalid("成稿证据缺少范围"))?;
                if !segments.contains(&segment) {
                    segments.push(segment.clone());
                }
                let provenance = json!({
                    "mode": "research_editor_frozen",
                    "run_id": origin.run_id,
                    "citation_index": origin.citation_index,
                    "question_id": receipt.question_id,
                    "revision": note.stage.revision,
                    "offset_chars": segment["offset_chars"],
                    "returned_chars": receipt.characters,
                });
                let origins = snapshot["research_origins"].as_array_mut().ok_or_else(|| invalid("成稿证据缺少来源身份"))?;
                conn.execute(
                    "INSERT INTO agent_run_evidence(run_id,kind,object_id,version_id,snapshot_json,created_at) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![run, origin.kind, origin.object_id, origin.version_id, encode(&json!({
                        "citation_index": position + 1,
                        "origin": provenance,
                    }))?, now],
                )?;
                if !origins.contains(&provenance) {
                    origins.push(provenance.clone());
                }
            }
            for (index, (origin, snapshot)) in snapshots.into_iter().enumerate() {
                conn.execute(
                    "INSERT INTO agent_run_citations(run_id,citation_index,kind,object_id,version_id,snapshot_json,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![run, index + 1, origin.kind, origin.object_id, origin.version_id, encode(&snapshot)?, now],
                )?;
            }
            Ok(())
        })
    }

    pub(crate) fn save_research_manuscript(
        &self,
        claim: &ResearchStageClaim,
        run: &str,
        manuscript: &ResearchManuscript,
    ) -> Result<(), BrainError> {
        if !(1..=24).contains(&manuscript.chapters.len()) {
            return Err(invalid("成稿须包含 1 至 24 个有阅读职责的章节"));
        }
        let mut titles = HashSet::new();
        let mut body = String::new();
        let mut findings = Vec::new();
        let mut ranges = Vec::new();
        for (index, chapter) in manuscript.chapters.iter().enumerate() {
            let title = chapter.title.trim();
            if !titles.insert(title.split_whitespace().collect::<String>().to_lowercase()) {
                return Err(invalid("报告章节标题重复"));
            }
            let output = chapter_output(chapter)?;
            let byte_start = body.len();
            append_report_section(&mut body, title, "", &output.content_md);
            let finding_start = findings.len();
            findings.extend(output.findings);
            ranges.push(json!({
                "question_id": format!("report_{}", index + 1),
                "title": title,
                "summary": chapter.summary,
                "byte_start": byte_start,
                "byte_end": body.len(),
                "finding_start": finding_start,
                "finding_end": findings.len(),
            }));
        }
        let output = ResearchSynthesisOutput {
            summary: manuscript.summary.clone(),
            content_md: body,
            findings,
            section_checks: manuscript.section_checks.clone(),
        };
        self.save_research_synthesis_inner(claim, run, &output, Some(&ranges))
    }
}

fn append_snapshot_value(
    snapshot: &mut Value,
    field: &str,
    value: Value,
) -> Result<(), BrainError> {
    if snapshot[field].is_null() {
        snapshot[field] = json!([]);
    }
    let values = snapshot[field]
        .as_array_mut()
        .ok_or_else(|| invalid("成稿证据快照字段无效"))?;
    if !values.contains(&value) {
        values.push(value);
    }
    Ok(())
}
