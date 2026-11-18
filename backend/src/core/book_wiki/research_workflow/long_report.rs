use super::*;
use crate::infra::book_wiki_store::{ResearchAuthorCheckpoint, ResearchChapter};
use crate::models::book_wiki::ResearchSectionCheck;

pub(super) const RULES: &str = include_str!("../../../../prompts/wiki/research-long.md");
const MIN_CHAPTER_BODY: u32 = 512;
const MAX_CHAPTERS: usize = 24;
const EDIT_INPUT_RESERVE: u64 = 1_500;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WritingOutline {
    thesis: String,
    chapters: Vec<ChapterBrief>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChapterBrief {
    id: String,
    title: String,
    purpose: String,
    question_ids: Vec<String>,
    body_tokens: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportEdit {
    summary: String,
    chapters: Vec<ChapterEdit>,
    section_checks: Vec<ResearchSectionCheck>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChapterEdit {
    id: String,
    title: String,
    #[serde(default)]
    transition: String,
    #[serde(default)]
    omit_repeated_intro: bool,
}

pub(super) fn output_contract(phase: &str) -> Option<Value> {
    Some(match phase {
        "report_outline" => json!({
            "thesis":"统领各节的有界中心判断",
            "chapters":[{"id":"chapter_a","title":"面向读者的节标题",
                "purpose":"此节独立承担的论证职责",
                "question_ids":["使用实际取证主题ID"],"body_tokens":1200}],
        }),
        "report_chapter" => json!({
            "title":"严格沿用大纲所分配的标题","summary":"本节要点摘要",
            "content_md":"带本轮原文引用的完整 Markdown 正文",
            "findings":[{"finding":"本节中实际写出的有界判断","status":"partial",
                "citation_indices":[],"limitations":["具体缺失依据"]}],
        }),
        "report_edit" => json!({
            "summary":"基于已写章节的全文核心判断及其条件",
            "chapters":[{"id":"已写章节的ID","title":"最终标题",
                "transition":"可为空的衔接句","omit_repeated_intro":false}],
            "section_checks":[{"question_id":"使用实际取证主题ID","revision":1,
                "assessment":"insufficient","note":"取得的证据与仍缺的条件"}],
        }),
        _ => return None,
    })
}

fn validation(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(format!("长报告写作：{message}"))
}

fn chapter_body_capacity(profile: &RuntimeProfile) -> u32 {
    ResearchResources::declared_section_output_capacity(profile, 1)
        .map_or(120_000, |capacity| capacity.visible_body_ceiling_tokens)
}

pub(super) fn can_split(profile: &RuntimeProfile, plan: &ResearchPlan) -> bool {
    !plan.questions.is_empty() && chapter_body_capacity(profile) >= 2 * MIN_CHAPTER_BODY
}

pub(super) fn requires_split(error: &BrainError) -> bool {
    matches!(error, BrainError::KnowledgeValidation(message) if [
        "(research_synthesis_output_hard_limit)",
        "(research_output_hard_limit)",
        "(research_input_hard_limit)",
        "(research_integration_hard_limit)",
        "(research_output_retry_exhausted)",
    ].iter().any(|marker| message.contains(marker)))
}

impl WritingOutline {
    fn validate(&self, plan: &ResearchPlan, body_capacity: u32) -> Result<(), BrainError> {
        if !(2..=MAX_CHAPTERS).contains(&self.chapters.len()) || !prose(&self.thesis, 800) {
            return Err(validation("大纲需要一个中心判断和两至二十四节"));
        }
        let notes = plan
            .questions
            .iter()
            .map(|question| question.id.as_str())
            .collect::<HashSet<_>>();
        let mut seen_ids = HashSet::new();
        let mut seen_titles = HashSet::new();
        let mut assigned = HashSet::new();
        for chapter in &self.chapters {
            if !(1..=48).contains(&chapter.id.len())
                || !chapter
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                || !seen_ids.insert(&chapter.id)
                || !prose(&chapter.title, 160)
                || !seen_titles.insert(reader_heading_key(&chapter.title))
                || !prose(&chapter.purpose, 400)
                || chapter.question_ids.is_empty()
                || !(MIN_CHAPTER_BODY..=body_capacity).contains(&chapter.body_tokens)
            {
                return Err(validation(
                    "大纲章节应有唯一标识、读者标题、分工、实际取证主题及可容纳的正文预算",
                ));
            }
            let chapter_notes = chapter
                .question_ids
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            if chapter_notes.len() != chapter.question_ids.len() || !chapter_notes.is_subset(&notes)
            {
                return Err(validation("大纲引用了未知或重复的取证主题"));
            }
            assigned.extend(chapter_notes);
        }
        if assigned != notes {
            return Err(validation("大纲需要为每个已取证主题安排章节"));
        }
        Ok(())
    }
}

fn prose(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= limit
        && !value.contains(['\r', '\n', '[', ']', '`', '|'])
        && !value.trim_start().starts_with('#')
}

fn repeated_intro(chapter: &ResearchChapter) -> Option<&str> {
    let (intro, _) = chapter.content_md.split_once("\n\n")?;
    prose(intro, 240).then_some(intro.trim())
}

fn edited_chapters(
    edits: &[ChapterEdit],
    chapters: &[(String, String, ResearchChapter)],
) -> Result<Vec<(String, ResearchChapter)>, BrainError> {
    if edits.len() != chapters.len() {
        return Err(validation("全文编辑必须保留每个已经成稿的章节"));
    }
    let chapters = chapters
        .iter()
        .map(|(id, run, chapter)| (id.as_str(), (run, chapter)))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let kept_intro = edits
        .iter()
        .filter(|edit| !edit.omit_repeated_intro)
        .filter_map(|edit| {
            chapters
                .get(edit.id.as_str())
                .and_then(|(_, chapter)| repeated_intro(chapter))
        })
        .collect::<HashSet<_>>();
    edits
        .iter()
        .map(|edit| {
            let (run, chapter) = chapters
                .get(edit.id.as_str())
                .ok_or_else(|| validation("全文编辑引用了尚未成稿的章节"))?;
            if !seen.insert(&edit.id)
                || !prose(&edit.title, 160)
                || (!edit.transition.trim().is_empty() && !prose(&edit.transition, 240))
            {
                return Err(validation("全文编辑需要唯一章节、可读标题和简短的衔接句"));
            }
            let mut edited = (*chapter).clone();
            edited.title = edit.title.clone();
            if edit.omit_repeated_intro {
                let intro =
                    repeated_intro(chapter).ok_or_else(|| validation("开场段落无法安全去重"))?;
                if !kept_intro.contains(intro) {
                    return Err(validation("去重后的开场内容须在报告中保留至少一处"));
                }
                if let Some((_, remaining)) = edited.content_md.split_once("\n\n") {
                    edited.content_md = remaining.to_owned();
                }
            }
            if !edit.transition.trim().is_empty() {
                edited.content_md = format!("{}\n\n{}", edit.transition.trim(), edited.content_md);
            }
            Ok(((*run).clone(), edited))
        })
        .collect()
}

fn neutral_citations(text: &str) -> String {
    text.replace("[S", "〔旧引文 S")
}

fn tail_preview(text: &str, characters: usize) -> String {
    let skip = text.chars().count().saturating_sub(characters);
    neutral_citations(&text.chars().skip(skip).collect::<String>())
}

struct LongReport<'a> {
    service: &'a BookWikiService,
    task: &'a KnowledgeTask,
    profile: &'a RuntimeProfile,
    plan: &'a ResearchPlan,
    manifest: Value,
    generation: String,
    requested_body: u32,
}

impl LongReport<'_> {
    fn checkpoint(&self, slot: impl Into<String>) -> ResearchAuthorCheckpoint {
        ResearchAuthorCheckpoint {
            generation: self.generation.clone(),
            slot: slot.into(),
        }
    }

    fn phase(
        &self,
        phase: &'static str,
        checkpoint: &ResearchAuthorCheckpoint,
        body_tokens: u32,
        mut payload: Value,
    ) -> Result<ResearchPhase<'_>, BrainError> {
        let store = &self.service.store;
        let question = ResearchQuestion {
            id: checkpoint.slot.clone(),
            title: "报告编辑".into(),
            question: self.plan.goal.clone(),
            required_evidence: vec!["交付材料的完整论证与证据边界".into()],
            expected_output_tokens: Some(body_tokens),
            target_entry_ids: vec![],
        };
        let previous = store.latest_truncated_research_author_run(&self.task.id, checkpoint)?;
        let resources = self.service.resume_research_phase_resources(
            self.profile,
            ResearchResources::new(self.profile, Some(self.plan), Some(&question), 0),
            previous.as_deref(),
        )?;
        payload["research_author_checkpoint"] = json!(checkpoint);
        payload["frozen_source_receipts"] = json!([]);
        payload["plan"] = json!({"goal":self.plan.goal,"report_title":self.plan.report_title,
            "constraints":self.plan.constraints,"acceptance":self.plan.acceptance,
            "depth":self.plan.depth,"terminology":self.plan.terminology});
        Ok(ResearchPhase {
            task: self.task,
            profile: self.profile,
            key: "synthesis".into(),
            phase,
            payload,
            resources,
            evidence: vec![],
        })
    }

    fn extra_input(&self, phase: &ResearchPhase<'_>) -> Result<u64, BrainError> {
        let (prompt, _) = self.service.research_phase_prompt(phase)?;
        Ok(phase
            .resources
            .prompt_token_limit
            .saturating_sub(estimated_tokens(&prompt))
            .saturating_sub(EDIT_INPUT_RESERVE))
    }

    async fn outline(&self) -> Result<WritingOutline, BrainError> {
        let checkpoint = self.checkpoint("outline");
        let store = &self.service.store;
        if let Some((_, outline)) =
            store.get_research_author_checkpoint(&self.task.id, &checkpoint)?
        {
            return Ok(outline);
        }
        let capacity = chapter_body_capacity(self.profile);
        let body_tokens = (self.plan.questions.len() as u32 * 220 + 512).min(capacity);
        let mut phase = self.phase(
            "report_outline",
            &checkpoint,
            body_tokens,
            json!({
                "requested_body_tokens":self.requested_body,
                "maximum_chapter_body_tokens":capacity,
                "minimum_chapter_body_tokens":MIN_CHAPTER_BODY,
                "maximum_chapters":MAX_CHAPTERS,
            }),
        )?;
        let additional = self.extra_input(&phase)?;
        phase.payload["integration_manifest"] =
            project_integration_manifest(&self.manifest, additional)?;
        self.service
            .persist_model_research_phase(phase, |claim, run, answer| {
                let outline: WritingOutline = parse_phase(answer)?;
                outline.validate(self.plan, capacity)?;
                store.save_research_author_checkpoint(
                    claim,
                    run,
                    &checkpoint,
                    &outline,
                    None,
                    &format!("已确认长报告大纲，待写 {} 节", outline.chapters.len()),
                )?;
                Ok(outline)
            })
            .await
    }

    async fn chapter(
        &self,
        outline: &WritingOutline,
        index: usize,
        previous: Option<&ResearchChapter>,
    ) -> Result<(String, ResearchChapter), BrainError> {
        let chapter = &outline.chapters[index];
        let checkpoint = self.checkpoint(format!("chapter:{}", chapter.id));
        let store = &self.service.store;
        if let Some(saved) = store.get_research_author_checkpoint(&self.task.id, &checkpoint)? {
            return Ok(saved);
        }
        if chapter.body_tokens > chapter_body_capacity(self.profile) {
            return Err(validation(
                "此节计划超出当前模型输出预算，已完成章节仍保留，请恢复原有输出能力后继续",
            ));
        }
        let mut phase = self.phase("report_chapter", &checkpoint, chapter.body_tokens, json!({
            "thesis":outline.thesis,
            "outline":outline.chapters.iter().map(|chapter| json!({"id":chapter.id,"title":chapter.title,"purpose":chapter.purpose})).collect::<Vec<_>>(),
            "chapter":chapter,
            "prior_chapter":previous.map(|chapter| json!({"title":chapter.title,"closing":tail_preview(&chapter.content_md,320)})),
            "available_originals":"长原文按保存的实际读取范围提供；更多条件、算例与反证可通过取证笔记定位",
        }))?;
        let available = self.extra_input(&phase)?;
        let notes = self.manifest["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|note| {
                note["question_id"]
                    .as_str()
                    .is_some_and(|id| chapter.question_ids.iter().any(|item| item == id))
            })
            .collect::<Vec<_>>();
        let manifest = json!({"sections":notes});
        let sources =
            store.research_author_sources_in(&self.task.id, Some(&chapter.question_ids))?;
        let (cards, receipts, used) = select_frozen_sources(&sources, available * 3 / 5);
        phase.payload["frozen_sources"] = json!(cards);
        phase.payload["frozen_source_receipts"] = json!(receipts);
        let remaining = available.saturating_sub(used);
        phase.payload["integration_manifest"] =
            project_integration_manifest(&manifest, remaining * 2 / 3)?;
        let preview_chars = usize::try_from(remaining / (9 * chapter.question_ids.len() as u64))
            .unwrap_or(0)
            .min(12_000);
        if preview_chars >= 100 {
            let mut previews = Vec::new();
            for id in &chapter.question_ids {
                let first =
                    store.read_research_section_page(&self.task.id, id, 0, preview_chars)?;
                let total = first["total_chars"].as_u64().unwrap_or(0) as usize;
                let tail = if total > preview_chars {
                    store.read_research_section_page(
                        &self.task.id,
                        id,
                        total.saturating_sub(preview_chars),
                        preview_chars,
                    )?["content_md"]
                        .clone()
                } else {
                    Value::Null
                };
                previews.push(json!({"id":id,"opening":first["content_md"],"closing":tail,"total_chars":total}));
            }
            phase.payload["note_previews"] = json!(previews);
        }
        self.service
            .persist_model_research_phase(phase, |claim, run, answer| {
                let draft: ResearchChapter = parse_phase(answer)?;
                if draft.title != chapter.title {
                    return Err(validation("分节成稿标题须沿用已保存大纲"));
                }
                store.save_research_author_checkpoint(
                    claim,
                    run,
                    &checkpoint,
                    &draft,
                    Some(&draft),
                    &format!("长报告已成稿 {}/{} 节", index + 1, outline.chapters.len()),
                )?;
                Ok((run.to_string(), draft))
            })
            .await
    }

    async fn edit(
        &self,
        chapters: &[(String, String, ResearchChapter)],
        outline: &WritingOutline,
    ) -> Result<(), BrainError> {
        let checkpoint = self.checkpoint("edit");
        let capacity = chapter_body_capacity(self.profile);
        let body_tokens =
            ((chapters.len() + self.plan.questions.len()) as u32 * 150 + 512).min(capacity);
        let mut phase = self.phase("report_edit", &checkpoint, body_tokens, json!({
            "thesis":outline.thesis,
            "chapter_overview":chapters.iter().map(|(id, _, chapter)| json!({"id":id,"title":chapter.title,
                "summary":neutral_citations(&prefix_with_token_budget(&chapter.summary,160,300)),
                "opening":repeated_intro(chapter)})).collect::<Vec<_>>(),
        }))?;
        let available = self.extra_input(&phase)?;
        phase.payload["integration_manifest"] =
            project_integration_manifest(&self.manifest, available * 2 / 3)?;
        let per_chapter = available / (3 * chapters.len() as u64);
        if per_chapter >= 80 {
            phase.payload["chapter_endings"] = json!(chapters.iter().map(|(id, _, chapter)| json!({
                "id":id,
                "closing":tail_preview(&chapter.content_md, (per_chapter/3).min(480) as usize),
                "main_finding":chapter.findings.first().map(|finding| neutral_citations(&prefix_with_token_budget(&finding.finding,per_chapter/3,300))),
            })).collect::<Vec<_>>());
        }
        self.service
            .persist_model_research_phase(phase, |claim, run, answer| {
                let edit: ReportEdit = parse_phase(answer)?;
                let mut chapters = edited_chapters(&edit.chapters, chapters)?;
                self.service.store.rebase_research_author_chapters(
                    claim,
                    run,
                    &checkpoint,
                    &mut chapters,
                )?;
                self.service.store.save_research_manuscript(
                    claim,
                    run,
                    &ResearchManuscript {
                        summary: edit.summary,
                        chapters: chapters.into_iter().map(|(_, chapter)| chapter).collect(),
                        section_checks: edit.section_checks,
                    },
                )
            })
            .await
    }
}

impl BookWikiService {
    pub(super) fn has_long_report_outline(&self, task: &str) -> Result<bool, BrainError> {
        let checkpoint = ResearchAuthorCheckpoint {
            generation: self.store.research_author_generation(task)?,
            slot: "outline".into(),
        };
        Ok(self
            .store
            .get_research_author_checkpoint::<WritingOutline>(task, &checkpoint)?
            .is_some())
    }

    pub(super) async fn write_long_report(
        &self,
        task: &KnowledgeTask,
        profile: &RuntimeProfile,
        plan: &ResearchPlan,
        requested_body: u32,
    ) -> Result<(), BrainError> {
        let generation = self.store.research_author_generation(&task.id)?;
        let author = LongReport {
            service: self,
            task,
            profile,
            plan,
            manifest: self.store.research_integration_manifest(&task.id)?,
            generation,
            requested_body,
        };
        let outline = author.outline().await?;
        let mut saved: Vec<(String, String, ResearchChapter)> =
            Vec::with_capacity(outline.chapters.len());
        for (index, chapter) in outline.chapters.iter().enumerate() {
            let (run, content) = author
                .chapter(&outline, index, saved.last().map(|(_, _, chapter)| chapter))
                .await?;
            saved.push((chapter.id.clone(), run, content));
        }
        author.edit(&saved, &outline).await
    }
}
