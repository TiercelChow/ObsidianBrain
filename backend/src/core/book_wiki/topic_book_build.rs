use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use futures::{stream::FuturesUnordered, StreamExt};
use serde_json::json;

use super::topic_pages::{
    citation_numbers, topic_output_budget, TopicWritingPlan, PAGE_PROMPT, WIKI_EDITORIAL_RULES,
};
use super::{
    estimated_tokens, hash_text, is_cancelled_agent_error, is_harness_output_truncation,
    is_retryable_harness_failure, model_json, BookWikiService,
};
use crate::error::BrainError;
use crate::infra::book_wiki_store::{topic_id, TopicBookBuildClaim};
use crate::models::book_wiki::{KnowledgeBaseSummary, RuntimeProfile};
use crate::models::wiki_topics::{
    TopicBookBuild, TopicBookBuildPage, TopicBookDocument, TopicBookPlan, TopicBookSource,
    TopicLink, TopicRevision, TopicWikiIndexPage,
};

mod incremental;
mod outline;
use incremental::{source_stamps, BookUpdate, UPDATE_PROMPT};
use outline::BookMaterial;

const PLAN_PROMPT: &str = include_str!("../../../prompts/wiki/topic-book-plan.md");
const MAX_PLAN_INPUT_TOKENS: u64 = 14_000;
const TOPIC_WRITERS: usize = 3;

impl BookWikiService {
    pub fn enqueue_topic_book_build(&self, base_id: &str) -> Result<TopicBookBuild, BrainError> {
        self.store.get_syncable_base(base_id)?;
        if let Some(claim) = self.store.claim_topic_book_build(base_id)? {
            let service = self.clone();
            let id = base_id.to_string();
            tokio::spawn(async move {
                if let Err(error) = service.run_topic_book_build(&id, claim).await {
                    tracing::error!(base_id = %id, %error, "全库主题编译结束");
                }
            });
        }
        self.store
            .topic_book_build(base_id)?
            .ok_or_else(|| BrainError::Internal("未找到全库编译记录".into()))
    }

    #[cfg(test)]
    async fn build_topic_book_now(&self, base_id: &str) -> Result<TopicBookBuild, BrainError> {
        let claim = self
            .store
            .claim_topic_book_build(base_id)?
            .expect("test owns the build");
        self.run_topic_book_build(base_id, claim).await
    }

    async fn run_topic_book_build(
        &self,
        base_id: &str,
        claim: TopicBookBuildClaim,
    ) -> Result<TopicBookBuild, BrainError> {
        let mut build = claim.previous.unwrap_or_default();
        build.id = claim.attempt_id;
        build.status = "running".into();
        build.phase = "syncing".into();
        build.error = None;
        build.stop_requested = false;
        let mut snapshot = claim.snapshot;
        if let Err(error) = self
            .build_topic_book(base_id, &mut build, &mut snapshot)
            .await
        {
            build.status = if build.pages.iter().any(|page| page.revision_id.is_some()) {
                "partial"
            } else {
                "failed"
            }
            .into();
            build.phase = "finished".into();
            build.error = Some(error.to_string());
            self.store
                .save_topic_book_build(base_id, &build, snapshot.as_deref())?;
            return Err(error);
        }
        self.store
            .save_topic_book_build(base_id, &build, snapshot.as_deref())?;
        Ok(build)
    }

    async fn build_topic_book(
        &self,
        base_id: &str,
        build: &mut TopicBookBuild,
        snapshot: &mut Option<String>,
    ) -> Result<(), BrainError> {
        if self.stop_topic_book_at_checkpoint(base_id, build)? {
            return Ok(());
        }
        let service = self.clone();
        let id = base_id.to_owned();
        let omitted = tokio::task::spawn_blocking(move || {
            let mut failures = Vec::new();
            service.sync_markdown_base(&id, Some(&mut failures))?;
            Ok::<_, BrainError>(failures)
        })
        .await
        .map_err(|error| BrainError::Internal(format!("全库来源同步中断: {error}")))??;
        let base = self.store.get_active_base(base_id)?;
        let (version, inventory) = self.store.topic_book_inventory(base_id)?;
        *snapshot = Some(version);
        build.sources = inventory.iter().map(source_coverage).collect();
        append_omissions(build, omitted);
        build.failed_groups.clear();
        build.reviewed_groups = 0;
        build.total_groups = 0;
        build.maintenance = None;
        build.outdated = false;
        build.phase = "planning".into();
        self.store
            .save_topic_book_build(base_id, build, snapshot.as_deref())?;
        if self.stop_topic_book_at_checkpoint(base_id, build)? {
            return Ok(());
        }

        let profile = self.active_runtime_profile()?;
        let mut runner = self.clone();
        runner.topic_build_profile = Some(Arc::new(profile.clone()));
        let rules = self.topic_rules(base_id, "knowledge.md")?;
        let fingerprint = hash_text(
            &json!([
                WIKI_EDITORIAL_RULES,
                PLAN_PROMPT,
                PAGE_PROMPT,
                UPDATE_PROMPT,
                rules,
                base.book_name,
                profile.provider_id,
                profile.model,
                profile.revision,
                profile
                    .provider_config
                    .as_ref()
                    .map(|provider| provider.revision),
            ])
            .to_string(),
        );
        let mut baseline = source_stamps(&inventory);
        let previous = build.source_baseline.as_deref().filter(|_| {
            build.editorial_fingerprint.as_deref() == Some(&fingerprint) && !build.pages.is_empty()
        });
        let mut material = None;
        let mut overview = None;
        let (pages, draft_ids, maintenance, groups) = if let Some(previous) = previous {
            let update = runner
                .prepare_topic_update(
                    &BookUpdate {
                        base: &base,
                        profile: &profile,
                        rules: &rules,
                        attempt: &build.id,
                    },
                    build,
                    &mut baseline,
                    previous,
                )
                .await?;
            (
                update.pages,
                update.draft_ids,
                Some(update.maintenance),
                update.routed_batches,
            )
        } else {
            if !baseline.iter().any(|document| document.has_text) {
                return Err(BrainError::KnowledgeValidation(
                    "知识库没有可读的 Markdown 正文".into(),
                ));
            }
            build.total_groups = 1;
            self.store
                .save_topic_book_build(base_id, build, snapshot.as_deref())?;
            let book = BookMaterial::read(&self.store, base_id, &inventory)?;
            let plan = runner
                .plan_topic_outline(
                    &base,
                    &profile,
                    &rules,
                    &book,
                    snapshot.as_deref().unwrap_or_default(),
                    &build.id,
                )
                .await?;
            let (pages, draft_ids) = self.prepare_initial_pages(base_id, build, &book, &plan)?;
            material = Some(book);
            overview = Some(plan.overview);
            (pages, draft_ids, None, 1)
        };
        if self.stop_topic_book_at_checkpoint(base_id, build)? {
            return Ok(());
        }
        self.require_current_topic_snapshot(
            base_id,
            build,
            snapshot.as_deref().unwrap_or_default(),
        )?;
        build.total_groups = groups;
        build.reviewed_groups = groups;
        build.phase = "writing".into();
        if let Some(overview) = overview {
            build.overview = overview;
        }
        build.pages = pages;
        build.maintenance = maintenance;
        build.source_baseline = Some(baseline);
        build.editorial_fingerprint = Some(fingerprint);
        self.store
            .save_topic_book_build(base_id, build, snapshot.as_deref())?;

        let context_chars = profile
            .provider_config
            .as_ref()
            .and_then(|provider| provider.context_window)
            .map_or(14_500, |window| {
                (window as usize).saturating_sub(6_000).clamp(3_000, 14_500)
            });
        let mut pending = VecDeque::new();
        for (index, page) in build.pages.iter().enumerate() {
            if page.revision_id.is_some() || page.error.is_some() {
                continue;
            }
            if material.is_none() {
                material = Some(BookMaterial::read(&self.store, base_id, &inventory)?);
            }
            let book = material
                .as_ref()
                .ok_or_else(|| BrainError::Internal("未找到已整理的全书素材".into()))?;
            let (request, required_documents) = book.revision_request(page, context_chars);
            pending.push_back((
                index,
                request,
                TopicWritingPlan {
                    required_documents,
                    links: page.links.clone(),
                    build_id: build.id.clone(),
                    expected_draft: draft_ids.get(&page.page_id).cloned(),
                },
                None,
            ));
        }

        let documents = inventory
            .iter()
            .enumerate()
            .flat_map(|(i, doc)| {
                doc.sections
                    .iter()
                    .map(move |section| (section.id.as_str(), i))
            })
            .collect::<HashMap<_, _>>();
        let mut covered = vec![HashSet::new(); inventory.len()];
        for page in &build.pages {
            if let Some(id) = &page.revision_id {
                let revision = self.store.topic_revision(base_id, &page.page_id, id)?;
                self.record_topic_coverage(
                    &mut build.sources,
                    &documents,
                    &mut covered,
                    &page.page_id,
                    &revision,
                )?;
            }
        }
        let attempt_id = build.id.clone();
        let snapshot_hash = snapshot.as_deref().unwrap_or_default();
        let mut results = FuturesUnordered::new();
        let mut accept_more = true;
        let mut checkpoint_error = None;
        loop {
            let free_slots = TOPIC_WRITERS - results.len();
            if accept_more && free_slots > 0 && !pending.is_empty() {
                let checkpoint = self
                    .store
                    .topic_book_stop_requested(base_id, &attempt_id)
                    .and_then(|stopped| {
                        if !stopped {
                            self.require_current_topic_snapshot(base_id, build, snapshot_hash)?;
                        }
                        Ok(!stopped)
                    });
                match checkpoint {
                    Ok(true) => {
                        for _ in 0..free_slots {
                            let Some((position, request, plan, retry)) = pending.pop_front() else {
                                break;
                            };
                            let runner = &runner;
                            results.push(async move {
                                let result = runner
                                    .compile_planned_topic(
                                        base_id,
                                        request.clone(),
                                        &plan,
                                        retry.as_ref(),
                                    )
                                    .await;
                                (position, request, plan, retry.is_some(), result)
                            });
                        }
                    }
                    Ok(false) => accept_more = false,
                    Err(error) => {
                        accept_more = false;
                        checkpoint_error = Some(error);
                    }
                }
            }
            let Some((index, request, plan, retried, result)) = results.next().await else {
                break;
            };
            let page_id = &build.pages[index].page_id;
            match result {
                Ok(page) => {
                    if let Some(revision) = page.draft.or(page.published) {
                        self.record_topic_coverage(
                            &mut build.sources,
                            &documents,
                            &mut covered,
                            page_id,
                            &revision,
                        )?;
                        build.pages[index].revision_id = Some(revision.id);
                        build.pages[index].error = None;
                    } else {
                        build.pages[index].error = Some("主题编译未产生可读取的修订".into());
                    }
                }
                Err(error) => {
                    build.pages[index].error = Some(error.to_string());
                    // Let unaffected first attempts make progress while this page
                    // waits for one retry. The next checkpoint still owns it.
                    if accept_more && !retried && retryable_topic_error(&error) {
                        pending.push_back((index, request, plan, Some(error)));
                    }
                }
            }
            self.store
                .save_topic_book_build(base_id, build, snapshot.as_deref())?;
        }
        if let Some(error) = checkpoint_error {
            return Err(error);
        }
        if self.stop_topic_book_at_checkpoint(base_id, build)? {
            return Ok(());
        }
        self.require_current_topic_snapshot(base_id, build, snapshot_hash)?;
        for source in &mut build.sources {
            source.page_ids.sort_unstable();
        }
        build.phase = "finished".into();
        build.status = if build.failed_groups.is_empty()
            && build
                .pages
                .iter()
                .all(|page| page.revision_id.is_some() && page.error.is_none())
            && build.sources.iter().all(|source| source.error.is_none())
            && !build.maintenance.as_ref().is_some_and(|update| {
                update
                    .changes
                    .iter()
                    .any(|change| change.disposition == "review")
            }) {
            "ready"
        } else {
            "partial"
        }
        .into();
        Ok(())
    }

    fn prepare_initial_pages(
        &self,
        base_id: &str,
        build: &TopicBookBuild,
        book: &BookMaterial<'_>,
        plan: &TopicBookPlan,
    ) -> Result<(Vec<TopicBookBuildPage>, HashMap<String, String>), BrainError> {
        let mut draft_ids = HashMap::new();
        let mut pages = Vec::with_capacity(plan.topics.len());
        for topic in &plan.topics {
            let page_id = topic_id(base_id, &topic.title);
            let current = self.store.find_topic_page(base_id, &page_id)?;
            let known = build
                .pages
                .iter()
                .find(|page| page.page_id == page_id)
                .and_then(|page| page.revision_id.as_deref());
            let managed = self.manages_topic_draft(known, current.as_ref())?;
            if let Some(draft) = current.and_then(|page| page.draft).filter(|_| managed) {
                draft_ids.insert(page_id.clone(), draft.id);
            }
            let links = topic
                .related
                .iter()
                .map(|relation| {
                    let target = &plan.topics[relation.topic - 1];
                    TopicLink {
                        page_id: topic_id(base_id, &target.title),
                        title: target.title.clone(),
                        reason: relation.reason.clone(),
                    }
                })
                .collect();
            pages.push(TopicBookBuildPage {
                page_id,
                title: topic.title.clone(),
                focus: Some(topic.focus.clone()),
                revision_id: None,
                error: (!managed).then(|| "此页有独立编辑的草稿，请审核草稿后再更新".into()),
                assigned_documents: book.assigned_chapters(topic),
                queries: topic.queries.clone(),
                links,
                update_notes: Vec::new(),
            });
        }
        Ok((pages, draft_ids))
    }

    fn record_topic_coverage(
        &self,
        sources: &mut [TopicBookSource],
        documents: &HashMap<&str, usize>,
        covered: &mut [HashSet<String>],
        page_id: &str,
        revision: &TopicRevision,
    ) -> Result<(), BrainError> {
        let quoted = citation_numbers(&revision.markdown);
        for citation in self.store.list_agent_run_citations(&revision.run_id)? {
            if !quoted.contains(&citation.citation_index) {
                continue;
            }
            let Some(&position) = documents.get(citation.object_id.as_str()) else {
                continue;
            };
            let coverage = &mut sources[position];
            if !coverage.page_ids.iter().any(|id| id == page_id) {
                coverage.page_ids.push(page_id.into());
            }
            covered[position].insert(citation.object_id);
            coverage.linked_sections = covered[position].len();
            coverage.status = "linked".into();
        }
        Ok(())
    }

    fn require_current_topic_snapshot(
        &self,
        base_id: &str,
        build: &mut TopicBookBuild,
        snapshot: &str,
    ) -> Result<(), BrainError> {
        if !self.store.topic_book_snapshot_matches(base_id, snapshot)? {
            build.outdated = true;
            return Err(BrainError::KnowledgeConflict(
                "编译时书籍来源发生变化，请用最新来源继续".into(),
            ));
        }
        Ok(())
    }

    fn stop_topic_book_at_checkpoint(
        &self,
        base_id: &str,
        build: &mut TopicBookBuild,
    ) -> Result<bool, BrainError> {
        if !self.store.topic_book_stop_requested(base_id, &build.id)? {
            return Ok(false);
        }
        build.stop_requested = true;
        build.status = "partial".into();
        build.phase = "stopped".into();
        Ok(true)
    }

    async fn plan_topic_outline(
        &self,
        base: &KnowledgeBaseSummary,
        profile: &RuntimeProfile,
        rules: &str,
        book: &BookMaterial<'_>,
        snapshot: &str,
        attempt_id: &str,
    ) -> Result<TopicBookPlan, BrainError> {
        let max_topics = book.topic_limit();
        let input_limit = profile
            .provider_config
            .as_ref()
            .and_then(|provider| provider.context_window)
            .map_or(MAX_PLAN_INPUT_TOKENS, |window| {
                (u64::from(window) * 2 / 3).min(MAX_PLAN_INPUT_TOKENS)
            });
        let mut existing = self.store.topic_index_pages(&base.id)?;
        existing.truncate(outline::MAX_TOPICS);
        let hints = serde_json::to_string(&existing)
            .map_err(|error| BrainError::Internal(error.to_string()))?;
        let mut scope = None;
        for lead in [320, 120, 0] {
            let input = json!({
                "book":base.book_name,
                "rules":rules,
                "max_topics":max_topics,
                "max_chapters_per_topic":book.chapters_per_topic(),
                "target_topics":book.chapter_count().div_ceil(2).clamp(1, max_topics.min(10)),
                "chapters":book.chapter_outline(lead),
                "book_guides":book.guide_excerpts(lead * 5),
            })
            .to_string();
            let cost = estimated_tokens(&input)
                + estimated_tokens(WIKI_EDITORIAL_RULES)
                + estimated_tokens(PLAN_PROMPT)
                + estimated_tokens(&hints)
                + 500;
            if cost <= input_limit {
                scope = Some(input);
                break;
            }
        }
        let scope = scope.ok_or_else(|| {
            BrainError::KnowledgeValidation(
                "整书篇目超出当前模型的规划空间，原文已完成索引，可查阅已发布的主题".into(),
            )
        })?;
        let hash = hash_text(
            &json!([
                WIKI_EDITORIAL_RULES,
                PLAN_PROMPT,
                snapshot,
                scope,
                profile.model,
                profile.provider_id,
                profile.revision,
                profile
                    .provider_config
                    .as_ref()
                    .map(|provider| provider.revision)
            ])
            .to_string(),
        );
        // Keep a stable plan for the same original versions, even after it was published.
        if let Some(mut plan) = self.store.read_topic_book_plan(&base.id, &hash)? {
            book.validate_plan(&mut plan, &existing)?;
            return Ok(plan);
        }
        let prompt = format!(
            "{WIKI_EDITORIAL_RULES}\n\n{PLAN_PROMPT}\n\n<book_outline>\n{scope}\n</book_outline>\n<existing_pages>\n{hints}\n</existing_pages>"
        );
        let request = BookPlanRequest {
            material: book,
            existing: &existing,
            prompt: &prompt,
            input_hash: &hash,
        };
        let first = self.request_book_plan(base, profile, &request, None).await;
        let plan = match first {
            Err(error)
                if retryable_topic_error(&error)
                    && !self.store.topic_book_stop_requested(&base.id, attempt_id)? =>
            {
                self.request_book_plan(base, profile, &request, Some(&error))
                    .await?
            }
            result => result?,
        };
        self.store.save_topic_book_plan(&base.id, &hash, &plan)?;
        Ok(plan)
    }

    async fn request_book_plan(
        &self,
        base: &KnowledgeBaseSummary,
        profile: &RuntimeProfile,
        request: &BookPlanRequest<'_>,
        retry: Option<&BrainError>,
    ) -> Result<TopicBookPlan, BrainError> {
        let material = request.material;
        let chapter_count = material.chapter_count();
        let maximum = material.topic_limit();
        let prompt = format!(
            "{}\n提交前逐项核对：主题最多 {maximum} 篇；合并问题重叠的主题。chapters 的正文编号 1–{chapter_count} 必须全部出现在 sources。每章先安排主要归属，再添加有实质关联的跨章材料。用短 focus、少量关键检索词及 1 至 2 个有用链接。现在直接输出完整 JSON。",
            request.prompt,
        );
        let prompt = match retry {
            Some(error) => {
                format!("{prompt}\n上一轮问题：{error}。按原合同重交完整目录。优先保证章节归属，把说明留给正文。")
            }
            None => prompt,
        };
        let first_output =
            (1024 + maximum as u32 * 380 + chapter_count.min(120) as u32 * 20).max(4_096);
        let requested = if retry.is_some() {
            first_output.saturating_mul(2).min(12_288)
        } else {
            first_output
        };
        let budget = topic_output_budget(profile, &prompt, requested)?;
        let input = json!({
            "plan_input_hash":request.input_hash,
            "chapter_count":chapter_count,
            "maximum_topics":maximum,
            "request_timeout_seconds":if chapter_count > maximum { 155 } else { 100 },
            "request_max_output_tokens":budget,
            "bounded_extraction":profile.provider_config.as_ref()
                .is_some_and(|provider|provider.reasoning_policy=="auto" && chapter_count <= maximum),
            "retry":retry.is_some(),
        });
        let (run, answer) = self
            .run_audited(&base.id, "wiki_topic_plan", &input, profile, prompt, None)
            .await?;
        let parsed = (|| {
            let (value, _) = model_json::single_json_object(&answer)?;
            let mut plan: TopicBookPlan = serde_json::from_value(value).map_err(|error| {
                BrainError::KnowledgeValidation(format!("主题目录结构有误：{error}"))
            })?;
            let placed = material.place_unassigned_chapters(&mut plan);
            material.validate_plan(&mut plan, request.existing)?;
            if !placed.is_empty() {
                self.store.append_agent_run_event(
                    &run,
                    "run.phase_changed",
                    Some("validating"),
                    "按篇目内容补齐原文的编辑归属",
                    &json!({"chapters":placed}),
                )?;
            }
            Ok::<_, BrainError>(plan)
        })();
        if let Err(error) = &parsed {
            self.store.append_agent_run_event(
                &run,
                "run.validation_rejected",
                Some("validating"),
                "目录规划未通过编译校验",
                &json!({"reason":error.to_string()}),
            )?;
        }
        parsed
    }
}

struct BookPlanRequest<'a> {
    material: &'a BookMaterial<'a>,
    existing: &'a [TopicWikiIndexPage],
    prompt: &'a str,
    input_hash: &'a str,
}

fn retryable_topic_error(error: &BrainError) -> bool {
    if is_cancelled_agent_error(error) {
        return false;
    }
    matches!(error, BrainError::KnowledgeValidation(_))
        || is_retryable_harness_failure(error)
        || is_harness_output_truncation(error)
        || topic_transport_timed_out(error)
}

pub(super) fn topic_transport_timed_out(error: &BrainError) -> bool {
    matches!(error, BrainError::LlmApiError {provider, detail}
        if provider == "deepseek_harness" && detail.contains("没有完成回答"))
}

fn source_coverage(document: &TopicBookDocument) -> TopicBookSource {
    TopicBookSource {
        path: document.path.clone(),
        page_ids: Vec::new(),
        total_sections: document.sections.len(),
        linked_sections: 0,
        status: if document
            .sections
            .iter()
            .all(|section| section.total_chars == 0)
        {
            "empty"
        } else {
            "indexed"
        }
        .into(),
        error: None,
    }
}

fn append_omissions(build: &mut TopicBookBuild, file_errors: Vec<(String, String)>) {
    build.sources.extend(
        file_errors
            .into_iter()
            .map(|(path, reason)| TopicBookSource {
                path,
                page_ids: Vec::new(),
                total_sections: 0,
                linked_sections: 0,
                status: "unreadable".into(),
                error: Some(reason),
            }),
    );
}

#[cfg(test)]
mod tests;
