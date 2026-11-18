use super::*;
use crate::core::presentation::{
    edit_page_text, page_editing_content, render_pptx_page_revision, PresentationLayout,
    PresentationPageEditingContent, PresentationRole,
};
use crate::infra::book_wiki_store::{AgentRunCitation, PresentationArtifactRevision};
use crate::models::book_wiki::{KnowledgeArtifact, PresentationPagination, ResearchFinding};
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationPageEditRequest {
    report_run_id: String,
    page: usize,
    changes: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationPlanPreviewRequest {
    report_run_id: String,
    pagination: PresentationPagination,
}

#[derive(Serialize)]
struct PresentationSource {
    index: usize,
    title: String,
    source_path: Option<String>,
}

impl From<AgentRunCitation> for PresentationSource {
    fn from(citation: AgentRunCitation) -> Self {
        Self {
            index: citation.citation_index,
            title: citation.entry.title,
            source_path: citation.entry.source_path,
        }
    }
}

#[derive(Serialize)]
struct SavedPageOutline {
    page: usize,
    title: String,
    layout: PresentationLayout,
    role: Option<PresentationRole>,
    estimated_seconds: Option<u32>,
    citations: Vec<String>,
    #[serde(flatten)]
    content: PresentationPageEditingContent,
}

#[derive(Serialize)]
pub(crate) struct SavedPresentationOutline {
    artifact_id: String,
    report_run_id: String,
    cover_title: String,
    cover_subtitle: String,
    core_message: String,
    pages: Vec<SavedPageOutline>,
    sources: Vec<PresentationSource>,
    read_only_reason: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct PresentationOutlineSection {
    title: String,
    finding_start: usize,
    finding_end: usize,
}

#[derive(Serialize)]
pub(crate) struct PresentationOutlinePreview {
    report_run_id: String,
    paging: PresentationPagingPlan,
    reference_total: usize,
    tight: bool,
    sections: Vec<PresentationOutlineSection>,
    findings: Vec<ResearchFinding>,
    sources: Vec<PresentationSource>,
}

struct SavedPresentation {
    artifact: KnowledgeArtifact,
    task_id: String,
    report_run_id: String,
    spec: PresentationSpec,
    sources: Vec<AgentRunCitation>,
}

fn saved_for_edit(
    store: &BookWikiStore,
    artifact_id: &str,
) -> Result<SavedPresentation, BrainError> {
    let artifact = store.get_artifact(artifact_id)?;
    if artifact.artifact_type != "pptx" || artifact.validation_state != "valid" {
        return Err(BrainError::KnowledgeValidation(
            "需要一份已校验的 PPTX".into(),
        ));
    }
    let legacy =
        || BrainError::KnowledgeValidation("这份演示没有保存逐页规划，请从报告生成新版".into());
    let task_id = artifact.knowledge_task_id.clone().ok_or_else(legacy)?;
    let run = store.get_agent_run(artifact.agent_run_id.as_deref().ok_or_else(legacy)?)?;
    if run.knowledge_base_id.as_deref() != Some(artifact.knowledge_base_id.as_str())
        || run.input["knowledge_task_id"] != task_id
        || run.status != "completed"
        || !matches!(
            run.task_type.as_str(),
            "knowledge_task_presentation_plan" | "knowledge_task_presentation_page_edit"
        )
    {
        return Err(legacy());
    }
    let report_run_id = run.input["research_run_id"]
        .as_str()
        .ok_or_else(legacy)?
        .to_owned();
    let sources = store.list_agent_run_citations(&report_run_id)?;
    let output = run.output.as_ref().ok_or_else(legacy)?;
    let spec = match output.get("presentation_spec") {
        Some(spec) => serde_json::from_value(spec.clone())
            .map_err(|error| BrainError::Internal(format!("已保存的演示规格损坏: {error}")))?,
        None => {
            parse_presentation_spec(output["answer"].as_str().ok_or_else(legacy)?, sources.len())?
        }
    };
    Ok(SavedPresentation {
        artifact,
        task_id,
        report_run_id,
        spec,
        sources,
    })
}

fn outline_sections(
    ranges: Value,
    title: String,
    findings: &[ResearchFinding],
) -> Result<Vec<PresentationOutlineSection>, BrainError> {
    if ranges.is_null() {
        return Ok(vec![PresentationOutlineSection {
            title,
            finding_start: 0,
            finding_end: findings.len(),
        }]);
    }
    let sections: Vec<PresentationOutlineSection> = serde_json::from_value(ranges)
        .map_err(|error| BrainError::Internal(format!("无法读取报告大纲: {error}")))?;
    let mut next = 0;
    for section in &sections {
        if section.finding_start != next
            || section.finding_end < next
            || section.finding_end > findings.len()
        {
            return Err(BrainError::Internal("报告大纲缺少完整的发现范围".into()));
        }
        next = section.finding_end;
    }
    if sections.is_empty() || next != findings.len() {
        return Err(BrainError::Internal("报告大纲未覆盖全部章节发现".into()));
    }
    Ok(sections)
}

impl BookWikiService {
    pub(crate) fn preview_presentation_outline(
        &self,
        task_id: &str,
        request: PresentationPlanPreviewRequest,
    ) -> Result<PresentationOutlinePreview, BrainError> {
        request
            .pagination
            .validate()
            .map_err(BrainError::KnowledgeValidation)?;
        self.store
            .require_current_presentation_report(task_id, &request.report_run_id)?;
        let task = self.store.get_task(task_id)?;
        let report = self
            .store
            .get_research_stage_content(task_id, "report", None)?;
        if report.content_run_id.as_deref() != Some(request.report_run_id.as_str()) {
            return Err(BrainError::KnowledgeConflict(
                "研究报告已更新，请重新打开大纲".into(),
            ));
        }
        let sections = outline_sections(
            self.store.research_report_outline(&request.report_run_id)?,
            task.title,
            &report.findings,
        )?;
        let mut brief = task.brief;
        brief.presentation_pagination = PresentationPagination::Auto;
        let reference = PresentationPagingPlan::new(&brief, &report.findings);
        brief.presentation_pagination = request.pagination;
        let paging = PresentationPagingPlan::new(&brief, &report.findings);
        let sources = self
            .store
            .list_agent_run_citations(&request.report_run_id)?;
        Ok(PresentationOutlinePreview {
            report_run_id: request.report_run_id,
            paging,
            reference_total: reference.preferred_total,
            tight: paging.preferred_total < reference.preferred_total,
            sections,
            findings: report.findings,
            sources: sources.into_iter().map(PresentationSource::from).collect(),
        })
    }

    pub(crate) fn saved_presentation_outline(
        &self,
        artifact_id: &str,
    ) -> Result<SavedPresentationOutline, BrainError> {
        let saved = saved_for_edit(&self.store, artifact_id)?;
        let read_only_reason = if saved.spec.schema_version != "2.0" {
            Some("此版本可浏览大纲，从报告生成新版后即可逐页编辑。".into())
        } else {
            match self.store.require_editable_presentation_artifact(
                &saved.artifact,
                &saved.task_id,
                &saved.report_run_id,
            ) {
                Ok(()) => None,
                Err(
                    BrainError::KnowledgeConflict(reason) | BrainError::KnowledgeValidation(reason),
                ) => Some(reason),
                Err(error) => return Err(error),
            }
        };
        let pages = saved
            .spec
            .slides
            .iter()
            .enumerate()
            .map(|(index, slide)| SavedPageOutline {
                page: index + 2,
                title: slide.title.clone(),
                layout: slide.layout,
                role: slide.role,
                estimated_seconds: slide.estimated_seconds,
                citations: slide.citations.clone(),
                content: page_editing_content(slide),
            })
            .collect();
        Ok(SavedPresentationOutline {
            artifact_id: saved.artifact.id,
            report_run_id: saved.report_run_id,
            cover_title: saved.spec.title,
            cover_subtitle: saved.spec.subtitle,
            core_message: saved.spec.core_message,
            pages,
            sources: saved
                .sources
                .into_iter()
                .map(PresentationSource::from)
                .collect(),
            read_only_reason,
        })
    }

    pub(crate) fn edit_presentation_page(
        &self,
        artifact_id: &str,
        request: PresentationPageEditRequest,
    ) -> Result<KnowledgeArtifact, BrainError> {
        let mut saved = saved_for_edit(&self.store, artifact_id)?;
        if saved.report_run_id != request.report_run_id || saved.spec.schema_version != "2.0" {
            return Err(BrainError::KnowledgeConflict(
                "逐页修订需要当前报告的新版演示".into(),
            ));
        }
        self.store.require_editable_presentation_artifact(
            &saved.artifact,
            &saved.task_id,
            &saved.report_run_id,
        )?;
        edit_page_text(&mut saved.spec, request.page, &request.changes)?;
        let plan = validate_presentation_spec(&saved.spec, saved.sources.len())?;
        let task = self.store.get_task(&saved.task_id)?;
        ensure_task_presentation_format(&saved.spec, &task)?;
        let originals = saved
            .sources
            .iter()
            .map(|citation| (citation.citation_index, citation.content_md.as_str()))
            .collect();
        let grounding = validate_presentation_grounding(&saved.spec, &originals)?;

        let original = self.artifact_path(artifact_id)?.0;
        if hash_file(&original)? != saved.artifact.content_hash {
            return Err(BrainError::KnowledgeConflict(
                "PPTX 文件已修改，请从研究报告生成新版本".into(),
            ));
        }
        let artifact_id = uuid::Uuid::new_v4().to_string();
        let relative_path = format!(
            "{}/{}/{}.pptx",
            saved.artifact.knowledge_base_id, saved.task_id, artifact_id
        );
        let destination = self.artifact_root.join(&relative_path);
        let staged = destination.with_extension("pptx.part");
        let result = (|| {
            render_pptx_page_revision(&saved.spec, &original, &staged, request.page)?;
            let validation = validate_pptx(&staged)?;
            if validation.slide_count != saved.spec.slides.len() + 1 {
                return Err(BrainError::KnowledgeConflict(
                    "PPTX 文件页数与已保存的大纲不一致".into(),
                ));
            }
            let mut quality = build_presentation_quality_report(&saved.spec, &plan, &validation);
            quality.checks.push(PresentationQualityCheck {
                code: "source_literals".into(),
                label: "数字和引语".into(),
                passed: true,
                detail: format!(
                    "逐页核对 {} 处数值和 {} 处引语",
                    grounding.numbers, grounding.quotes
                ),
            });
            let mut details = serde_json::to_value(&quality)
                .map_err(|error| BrainError::Internal(format!("无法保存演示校验结果: {error}")))?;
            if let Some(paging) = saved.artifact.validation_details.get("pagination") {
                details["pagination"] = paging.clone();
            }
            details["edited_page"] = json!(request.page);
            details["revises_artifact_id"] = json!(saved.artifact.id);
            let spec = serde_json::to_value(&saved.spec)
                .map_err(|error| BrainError::Internal(format!("无法保存逐页修订: {error}")))?;
            let hash = hash_file(&staged)?;
            let size = std::fs::metadata(&staged)?.len() as i64;
            std::fs::rename(&staged, &destination)?;
            self.store.save_presentation_page_revision(
                &saved.artifact,
                &saved.task_id,
                PresentationArtifactRevision {
                    publication_id: &artifact_id,
                    report_run_id: &saved.report_run_id,
                    page: request.page,
                    relative_path: &relative_path,
                    hash: &hash,
                    size,
                    validation_message: &quality.summary,
                    validation_details: &details,
                    spec: &spec,
                },
            )
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&staged);
            let _ = std::fs::remove_file(&destination);
        }
        result
    }
}
