use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::{json, Value};

use super::{
    estimated_tokens, hash_text, model_json, retryable_topic_error, topic_output_budget,
    BookWikiService, MAX_PLAN_INPUT_TOKENS, WIKI_EDITORIAL_RULES,
};
use crate::error::BrainError;
use crate::infra::book_wiki_store::topic_id;
use crate::models::book_wiki::{KnowledgeBaseSummary, RuntimeProfile};
use crate::models::wiki_topics::{
    TopicBookBuild, TopicBookBuildPage, TopicBookDocument, TopicLink, TopicMaintenance, TopicPage,
    TopicSourceChange, TopicSourceStamp,
};

pub(super) const UPDATE_PROMPT: &str =
    include_str!("../../../../prompts/wiki/topic-update-plan.md");

pub(super) struct BookUpdate<'a> {
    pub base: &'a KnowledgeBaseSummary,
    pub profile: &'a RuntimeProfile,
    pub rules: &'a str,
    pub attempt: &'a str,
}

pub(super) struct UpdatedPages {
    pub pages: Vec<TopicBookBuildPage>,
    pub draft_ids: HashMap<String, String>,
    pub maintenance: TopicMaintenance,
    pub routed_batches: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Added,
    Modified,
    Removed,
    Unreadable,
    Empty,
}

impl EditKind {
    fn name(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Removed => "removed",
            Self::Unreadable => "unreadable",
            Self::Empty => "empty",
        }
    }
}

struct SourceEdit<'a> {
    source: &'a TopicSourceStamp,
    preceding: Option<&'a str>,
    kind: EditKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateResponse {
    sources: Vec<RoutedSource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutedSource {
    source: usize,
    reason: String,
    #[serde(default)]
    pages: Vec<String>,
    #[serde(default)]
    new_pages: Vec<NewPage>,
    #[serde(default)]
    needs_review: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPage {
    title: String,
    focus: String,
    #[serde(default)]
    queries: Vec<String>,
    related: Vec<PageReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageReference {
    page_id: String,
    reason: String,
}

pub(super) fn source_stamps(documents: &[TopicBookDocument]) -> Vec<TopicSourceStamp> {
    documents
        .iter()
        .map(|document| TopicSourceStamp {
            document_id: document.id.clone(),
            version: document.version.clone(),
            path: document.path.clone(),
            has_text: !document.sections.is_empty(),
        })
        .collect()
}

fn source_edits<'a>(
    previous: &'a [TopicSourceStamp],
    current: &'a [TopicSourceStamp],
    unavailable: &HashSet<&str>,
) -> Vec<SourceEdit<'a>> {
    let before: HashMap<_, _> = previous
        .iter()
        .map(|item| (&item.document_id, item))
        .collect();
    let after: HashSet<_> = current.iter().map(|item| &item.document_id).collect();
    let mut edits = Vec::new();
    for source in previous {
        let kind = if unavailable.contains(source.path.as_str()) {
            EditKind::Unreadable
        } else if !after.contains(&source.document_id) {
            EditKind::Removed
        } else {
            continue;
        };
        edits.push(SourceEdit {
            source,
            preceding: Some(&source.version),
            kind,
        });
    }
    for source in current {
        let old = before.get(&source.document_id);
        if unavailable.contains(source.path.as_str())
            || old.is_some_and(|old| old.version == source.version && old.path == source.path)
        {
            continue;
        }
        let kind = if !source.has_text {
            EditKind::Empty
        } else if old.is_none() {
            EditKind::Added
        } else {
            EditKind::Modified
        };
        edits.push(SourceEdit {
            source,
            preceding: old
                .filter(|old| old.has_text)
                .map(|old| old.version.as_str()),
            kind,
        });
    }
    edits
}

impl BookWikiService {
    pub(super) async fn prepare_topic_update(
        &self,
        context: &BookUpdate<'_>,
        build: &TopicBookBuild,
        baseline: &mut Vec<TopicSourceStamp>,
        previous: &[TopicSourceStamp],
    ) -> Result<UpdatedPages, BrainError> {
        let base_id = &context.base.id;
        let unavailable: HashSet<_> = build
            .sources
            .iter()
            .filter(|item| item.error.is_some())
            .map(|item| item.path.as_str())
            .collect();
        let edits = source_edits(previous, baseline, &unavailable);
        let mut pages = build.pages.clone();
        let mut draft_ids = HashMap::new();
        let mut protected = HashSet::new();
        let mut required_pages = vec![Vec::new(); edits.len()];
        let mut catalogue = Vec::with_capacity(pages.len());
        let available: HashSet<_> = baseline
            .iter()
            .filter(|source| source.has_text && !unavailable.contains(source.path.as_str()))
            .map(|source| source.document_id.as_str())
            .collect();
        for record in &mut pages {
            if record.revision_id.is_some() {
                record.update_notes.clear();
            }
            let current = self.store.find_topic_page(base_id, &record.page_id)?;
            let (draft, published) = current.as_ref().map_or((None, None), |page| {
                (page.draft.as_ref(), page.published.as_ref())
            });
            let managed =
                self.manages_topic_draft(record.revision_id.as_deref(), current.as_ref())?;
            let existing = draft.filter(|_| managed).or(published);
            let mut refresh =
                record.revision_id.is_none() || record.error.is_some() || existing.is_none();
            if !edits.is_empty() {
                let mut dependencies = record.assigned_documents.clone();
                for revision in published.into_iter().chain(draft) {
                    dependencies.extend(
                        self.store
                            .topic_revision_source_documents(base_id, &revision.id)?,
                    );
                }
                for (index, edit) in edits.iter().enumerate() {
                    if dependencies.contains(&edit.source.document_id) {
                        required_pages[index].push(record.page_id.clone());
                        refresh = true;
                    }
                }
            }
            if let (Some(page), Some(revision)) = (&current, existing) {
                refresh |= page.questions.iter().any(|question| {
                    question.created_at > revision.created_at
                        || question
                            .resolved_at
                            .as_ref()
                            .is_some_and(|at| at > &revision.created_at)
                });
            }
            let summary = published.or(existing).map(|revision| {
                revision
                    .markdown
                    .split("\n\n")
                    .map(str::trim)
                    .find(|part| !part.is_empty() && !part.starts_with('#'))
                    .unwrap_or_default()
                    .chars()
                    .take(250)
                    .collect::<String>()
            });
            catalogue.push(json!({"page_id":record.page_id, "title":record.title,
                "focus":record.focus, "summary":summary}));
            if refresh {
                record.revision_id = None;
            } else if let Some(revision) = existing {
                record.revision_id = Some(revision.id.clone());
            }
            record.error = None;
            if !managed {
                protected.insert(record.page_id.clone());
            }
            if let Some(draft) = draft.filter(|_| managed) {
                draft_ids.insert(record.page_id.clone(), draft.id.clone());
            }
        }

        let titles = pages
            .iter()
            .map(|page| (page.page_id.clone(), page.title.clone()))
            .collect::<HashMap<_, _>>();
        let readable = edits
            .iter()
            .enumerate()
            .filter(|(_, edit)| matches!(edit.kind, EditKind::Added | EditKind::Modified))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut routes = HashMap::new();
        let mut seen_ids: HashSet<_> = titles.keys().cloned().collect();
        if !readable.is_empty() {
            seen_ids.extend(
                self.store
                    .list_topic_pages(base_id)?
                    .into_iter()
                    .map(|page| page.id),
            );
        }
        let token_budget = update_input_limit(context.profile)
            .saturating_sub(estimated_tokens(&update_prompt(context, &[], &catalogue)) + 300);
        let mut routed_batches = 0;
        let mut start = 0;
        while start < readable.len() {
            if self
                .store
                .topic_book_stop_requested(base_id, context.attempt)?
            {
                break;
            }
            let mut materials = Vec::new();
            let mut used = 0;
            for &index in readable[start..].iter().take(8) {
                let edit = &edits[index];
                let mut material = self.source_update_material(
                    base_id,
                    index + 1,
                    edit,
                    &required_pages[index],
                    &titles,
                )?;
                let mut tokens = estimated_tokens(&material.to_string());
                if used + tokens > token_budget && !materials.is_empty() {
                    break;
                }
                for limit in [900, 400, 160] {
                    if tokens <= token_budget {
                        break;
                    }
                    truncate_material(&mut material, limit);
                    tokens = estimated_tokens(&material.to_string());
                }
                used += tokens;
                materials.push(material);
            }
            let end = start + materials.len();
            let batch = &readable[start..end];
            let response = self
                .route_topic_sources(context, &materials, &catalogue, batch, &titles, &seen_ids)
                .await?;
            for route in response.sources {
                seen_ids.extend(
                    route
                        .new_pages
                        .iter()
                        .map(|page| topic_id(base_id, &page.title)),
                );
                routes.insert(route.source - 1, route);
            }
            routed_batches += 1;
            start = end;
        }
        let mut deferred = HashSet::new();
        let mut unreadable_pages = HashSet::new();
        let mut changes = Vec::with_capacity(edits.len());
        for (index, edit) in edits.into_iter().enumerate() {
            let document_id = &edit.source.document_id;
            let mut affected = std::mem::take(&mut required_pages[index]);
            let mut disposition = "update";
            let reason = if let Some(route) = routes.remove(&index) {
                for page_id in route.pages {
                    let record = pages
                        .iter_mut()
                        .find(|page| page.page_id == page_id)
                        .ok_or_else(|| BrainError::Internal("来源分配后主题页消失".into()))?;
                    record.revision_id = None;
                    if !affected.contains(&page_id) {
                        affected.push(page_id);
                    }
                }
                for proposal in route.new_pages {
                    let record = new_build_page(base_id, document_id, proposal, &titles);
                    affected.push(record.page_id.clone());
                    pages.push(record);
                    disposition = "add";
                }
                if route.needs_review {
                    deferred.insert(document_id.clone());
                    disposition = "review";
                } else if affected.is_empty() {
                    disposition = "no_change";
                }
                route.reason
            } else {
                match edit.kind {
                    EditKind::Unreadable => {
                        unreadable_pages.extend(affected.iter().cloned());
                        deferred.insert(document_id.clone());
                        disposition = "review";
                        "原文暂时不可读取，保留依赖此来源的现有页面".into()
                    }
                    _ if affected.is_empty() => {
                        disposition = "no_change";
                        "此来源没有可更新的现有主题".into()
                    }
                    _ => "原文已删除或清空，核对依赖该来源的现有主题".into(),
                }
            };
            for record in pages
                .iter_mut()
                .filter(|page| affected.contains(&page.page_id))
            {
                if matches!(edit.kind, EditKind::Added | EditKind::Modified)
                    && !record.assigned_documents.contains(document_id)
                {
                    record.assigned_documents.push(document_id.clone());
                }
                let note = format!("{} · {reason}", edit.source.path);
                if !record.update_notes.contains(&note) {
                    if record.update_notes.len() == 6 {
                        record.update_notes.remove(0);
                    }
                    record.update_notes.push(note);
                }
            }
            changes.push(TopicSourceChange {
                path: edit.source.path.clone(),
                change: edit.kind.name().into(),
                disposition: disposition.into(),
                reason,
                page_ids: affected,
            });
        }
        for record in &mut pages {
            if record.revision_id.is_some() || record.error.is_some() {
                continue;
            }
            if protected.contains(&record.page_id) {
                record.error = Some("此页有独立编辑的草稿，请审核草稿后再更新".into());
            } else if unreadable_pages.contains(&record.page_id) {
                record.error = Some("此页的原文暂不可读，恢复来源后可重试".into());
            } else if !record
                .assigned_documents
                .iter()
                .any(|id| available.contains(id.as_str()))
            {
                record.error = Some("此页的原有依据已移除，补充来源后可重试".into());
            }
        }
        baseline.retain(|item| !deferred.contains(&item.document_id));
        baseline.extend(
            previous
                .iter()
                .filter(|item| deferred.contains(&item.document_id))
                .cloned(),
        );
        let reused_pages = pages
            .iter()
            .filter(|page| page.revision_id.is_some())
            .count();
        Ok(UpdatedPages {
            pages,
            draft_ids,
            maintenance: TopicMaintenance {
                changes,
                reused_pages,
            },
            routed_batches,
        })
    }

    pub(super) fn manages_topic_draft(
        &self,
        known_revision: Option<&str>,
        current: Option<&TopicPage>,
    ) -> Result<bool, BrainError> {
        let Some((page, draft)) =
            current.and_then(|page| page.draft.as_ref().map(|draft| (page, draft)))
        else {
            return Ok(true);
        };
        if known_revision == Some(draft.id.as_str()) {
            return Ok(true);
        }
        let parent = page.published.as_ref().map(|revision| revision.id.as_str());
        if draft.parent_id.as_deref() != parent {
            return Ok(false);
        }
        let run = self.store.get_agent_run(&draft.run_id)?;
        Ok(run.input["topic_book_build_id"].is_string())
    }

    fn source_update_material(
        &self,
        base_id: &str,
        number: usize,
        edit: &SourceEdit<'_>,
        pages: &[String],
        titles: &HashMap<String, String>,
    ) -> Result<Value, BrainError> {
        let current = self
            .store
            .topic_book_source_text(base_id, &edit.source.version)?;
        let previous = edit
            .preceding
            .map(|version| self.store.topic_book_source_text(base_id, version))
            .transpose()?
            .unwrap_or_default();
        let before = changed_lines(&previous, &current);
        let after = changed_lines(&current, &previous);
        let query: String = after.chars().take(540).collect();
        let mut seen = HashSet::new();
        let matches = self
            .store
            .search_topic_sections(base_id, &query, false)?
            .into_iter()
            .filter(|item| {
                titles.contains_key(&item.page_id)
                    && !pages.contains(&item.page_id)
                    && seen.insert(item.page_id.clone())
            })
            .take(3)
            .map(|item| {
                json!({"page_id":item.page_id, "heading":item.section.heading,
                "excerpt":item.section.markdown.chars().take(240).collect::<String>()})
            })
            .collect::<Vec<_>>();
        const EXCERPT_CHARS: usize = 1_800;
        Ok(json!({
            "source":number, "path":edit.source.path, "change": edit.kind.name(),
            "required_pages":pages,
            "related_passages":matches,
            "removed_lines":before.chars().take(EXCERPT_CHARS).collect::<String>(),
            "current_lines":after.chars().take(EXCERPT_CHARS).collect::<String>(),
            "excerpt_truncated":before.chars().count() > EXCERPT_CHARS || after.chars().count() > EXCERPT_CHARS,
        }))
    }

    async fn route_topic_sources(
        &self,
        context: &BookUpdate<'_>,
        materials: &[Value],
        catalogue: &[Value],
        batch: &[usize],
        titles: &HashMap<String, String>,
        seen_ids: &HashSet<String>,
    ) -> Result<UpdateResponse, BrainError> {
        let prompt = update_prompt(context, materials, catalogue);
        if estimated_tokens(&prompt) > update_input_limit(context.profile) {
            return Err(BrainError::KnowledgeValidation(
                "来源变化和主题目录超出当前模型的规划空间".into(),
            ));
        }
        let prompt_hash = hash_text(&prompt);
        let mut retry = None;
        loop {
            let prompt = match &retry {
                Some(error) => format!("{prompt}\n上一轮问题：{error}。请按约定返回完整 JSON。"),
                None => prompt.clone(),
            };
            let budget = topic_output_budget(context.profile, &prompt, 3_072)?;
            let input = json!({"plan_input_hash":prompt_hash,
                "changed_sources":materials.len(), "request_timeout_seconds":100,
                "bounded_extraction":context.profile.provider_config.as_ref()
                    .is_some_and(|provider|provider.reasoning_policy=="auto"),
                "request_max_output_tokens":budget, "retry":retry.is_some()});
            let result = self
                .run_audited(
                    &context.base.id,
                    "wiki_topic_update",
                    &input,
                    context.profile,
                    prompt,
                    None,
                )
                .await
                .and_then(|(_, answer)| {
                    parse_routes(&answer, &context.base.id, batch, titles, seen_ids)
                });
            match result {
                Err(error)
                    if retry.is_none()
                        && retryable_topic_error(&error)
                        && !self
                            .store
                            .topic_book_stop_requested(&context.base.id, context.attempt)? =>
                {
                    retry = Some(error);
                }
                result => return result,
            }
        }
    }
}

fn update_prompt(context: &BookUpdate<'_>, materials: &[Value], catalogue: &[Value]) -> String {
    let scope = json!({"book":context.base.book_name, "rules":context.rules,
        "changed_sources":materials, "existing_pages":catalogue});
    format!(
        "{WIKI_EDITORIAL_RULES}\n\n{UPDATE_PROMPT}\n\n<source_update>\n{scope}\n</source_update>"
    )
}

fn update_input_limit(profile: &RuntimeProfile) -> u64 {
    profile
        .provider_config
        .as_ref()
        .and_then(|provider| provider.context_window)
        .map_or(MAX_PLAN_INPUT_TOKENS, |window| {
            (u64::from(window) * 2 / 3).min(MAX_PLAN_INPUT_TOKENS)
        })
}

fn truncate_material(material: &mut Value, limit: usize) {
    for key in ["removed_lines", "current_lines"] {
        if let Some(text) = material.get(key).and_then(Value::as_str) {
            let shortened: String = text.chars().take(limit).collect();
            if shortened.len() < text.len() {
                material[key] = Value::String(shortened);
                material["excerpt_truncated"] = Value::Bool(true);
            }
        }
    }
    if limit < 500 {
        material["related_passages"] = json!([]);
    }
}

fn parse_routes(
    answer: &str,
    base_id: &str,
    batch: &[usize],
    titles: &HashMap<String, String>,
    seen_ids: &HashSet<String>,
) -> Result<UpdateResponse, BrainError> {
    let (value, _) = model_json::single_json_object(answer)?;
    let result: UpdateResponse = serde_json::from_value(value).map_err(|error| {
        BrainError::KnowledgeValidation(format!("来源变更计划格式有误：{error}"))
    })?;
    let mut received = HashSet::new();
    let mut ids = seen_ids.clone();
    for route in &result.sources {
        let mut targets = HashSet::new();
        if !route
            .source
            .checked_sub(1)
            .is_some_and(|index| batch.contains(&index))
            || !received.insert(route.source)
            || !fits(&route.reason, 300)
            || route.new_pages.len() > 2
            || (route.needs_review && (!route.pages.is_empty() || !route.new_pages.is_empty()))
            || route
                .pages
                .iter()
                .any(|id| !titles.contains_key(id) || !targets.insert(id))
        {
            return Err(BrainError::KnowledgeValidation(format!(
                "原文 {} 的更新目标或理由无效",
                route.source
            )));
        }
        for page in &route.new_pages {
            let mut links = HashSet::new();
            if !fits(&page.title, 120)
                || !fits(&page.focus, 300)
                || !ids.insert(topic_id(base_id, &page.title))
                || page.queries.len() > 6
                || page.queries.iter().any(|query| !fits(query, 100))
                || !(1..=4).contains(&page.related.len())
                || page.related.iter().any(|link| {
                    !titles.contains_key(&link.page_id)
                        || !fits(&link.reason, 160)
                        || !links.insert(&link.page_id)
                })
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "新增主题「{}」的标题、范围或阅读关系无效",
                    page.title
                )));
            }
        }
    }
    if received.len() != batch.len() {
        return Err(BrainError::KnowledgeValidation(
            "来源维护计划遗漏了变化的原文".into(),
        ));
    }
    Ok(result)
}

fn fits(text: &str, max_chars: usize) -> bool {
    (1..=max_chars).contains(&text.trim().chars().count())
}

fn new_build_page(
    base_id: &str,
    document_id: &str,
    page: NewPage,
    titles: &HashMap<String, String>,
) -> TopicBookBuildPage {
    let title = page.title.split_whitespace().collect::<Vec<_>>().join(" ");
    TopicBookBuildPage {
        page_id: topic_id(base_id, &title),
        title,
        focus: Some(page.focus.trim().into()),
        revision_id: None,
        error: None,
        assigned_documents: vec![document_id.into()],
        queries: page.queries,
        links: page
            .related
            .into_iter()
            .filter_map(|link| {
                titles.get(&link.page_id).map(|title| TopicLink {
                    page_id: link.page_id,
                    title: title.clone(),
                    reason: link.reason.trim().into(),
                })
            })
            .collect(),
        update_notes: Vec::new(),
    }
}

fn changed_lines(text: &str, other: &str) -> String {
    let other: HashSet<_> = other.lines().map(str::trim).collect();
    let lines = text.lines().collect::<Vec<_>>();
    let mut last_heading = None;
    let mut selected = Vec::new();
    let mut used_heading = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            last_heading = Some(index);
        }
        if !trimmed.is_empty() && !other.contains(trimmed) {
            if let Some(heading) =
                last_heading.filter(|&heading| heading != index && used_heading != Some(heading))
            {
                selected.push(lines[heading]);
            }
            selected.push(line);
            used_heading = last_heading;
        }
    }
    selected.join("\n")
}
