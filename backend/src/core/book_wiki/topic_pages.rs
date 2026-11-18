use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};

use super::topic_book_build::topic_transport_timed_out;
use super::{estimated_tokens, hash_text, source_structure, AuditedRunObservers, BookWikiService};
use crate::core::book_wiki::KnowledgeChatStreamEvent;
use crate::error::BrainError;
use crate::infra::book_wiki_store::{knowledge_search_terms, topic_id, BookWikiStore};
use crate::models::book_wiki::{KnowledgeAnswer, RuntimeProfile};
use crate::models::wiki_topics::{
    TopicCompileRequest, TopicExcerpt, TopicLink, TopicPage, TopicPublication, TopicSection,
    TopicSectionMatch, TopicSourceRange, TopicSourceSelection,
};

mod writeback;

pub(super) const WIKI_EDITORIAL_RULES: &str =
    include_str!("../../../prompts/wiki/book-wiki-editorial.md");
pub(super) const PAGE_PROMPT: &str = include_str!("../../../prompts/wiki/topic-page.md");
const ANSWER_PROMPT: &str = include_str!("../../../prompts/wiki/topic-answer.md");
pub(super) const MAX_SOURCE_CHARS: usize = 20_000;
pub(super) const MAX_SOURCE_EXCERPTS: usize = 12;
const MAX_EXCERPT_CHARS: usize = 12_000;
const PAGE_CHARS: usize = 12_000;
const COMPILE_TIMEOUT_SECONDS: u64 = 150;
type ChatSender = tokio::sync::mpsc::UnboundedSender<KnowledgeChatStreamEvent>;

pub(super) struct TopicWritingPlan {
    pub required_documents: Vec<String>,
    pub links: Vec<TopicLink>,
    pub build_id: String,
    pub expected_draft: Option<String>,
}

impl BookWikiService {
    pub fn draft_reader_selection(
        &self,
        base_id: &str,
        source_path: &str,
        selection: &str,
        title: Option<&str>,
    ) -> Result<TopicPage, BrainError> {
        self.store.get_syncable_base(base_id)?;
        let source_path = source_path.trim().trim_start_matches('/');
        let selection = selection.trim();
        if source_path.is_empty() || selection.is_empty() || selection.chars().count() > 5_000 {
            return Err(BrainError::KnowledgeValidation(
                "请选择不超过 5000 字的已同步原文".into(),
            ));
        }
        let excerpt = self
            .store
            .reader_selection_excerpt(base_id, source_path, selection)?
            .filter(|excerpt| excerpt.content.chars().count() <= MAX_EXCERPT_CHARS)
            .ok_or_else(|| {
                BrainError::KnowledgeValidation(
                    "未能在当前原文中定位完整选区，请同步来源或缩短选区后重试".into(),
                )
            })?;
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or(selection)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(48)
            .collect::<String>();
        let title = format!("摘录 · {title}");
        let page_id = topic_id(base_id, &title);
        let prior = self.store.find_topic_page(base_id, &page_id)?;
        let parent = prior.and_then(|page| page.published.map(|revision| revision.id));
        let request = TopicCompileRequest {
            page_id: None,
            title,
            guidance: "沿用原文摘录中的事实和出处".into(),
            sources: vec![TopicSourceSelection {
                document_id: excerpt.document_id.clone(),
                span_id: Some(excerpt.span_id.clone()),
                range: Some(TopicSourceRange {
                    offset: excerpt.offset,
                    length: excerpt.content.chars().count(),
                }),
            }],
        };
        let input = json!({"source_path":source_path, "version":excerpt.version_id,
            "offset":excerpt.offset, "selection":selection});
        let (page_id, claim) = self.store.claim_topic_compile(
            base_id,
            &request,
            parent,
            hash_text(&input.to_string()),
            None,
        )?;
        if let Some(claim) = claim {
            let written = (|| {
                let run =
                    self.store
                        .start_agent_run(base_id, "user", "wiki_reader_excerpt", &input)?;
                let result = (|| {
                    self.store.record_visible_agent_evidence(
                        &run.id, "source_span", &excerpt.span_id, &excerpt.version_id,
                        &json!({"content_md":excerpt.content, "offset_chars":excerpt.offset, "origin":"reader_selection"}),
                    )?;
                    let text = selection
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('[', "&#91;");
                    let quote = text
                        .lines()
                        .map(|line| format!("> {line}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let markdown = format!("## 原文摘录\n\n{quote}\n\n[S1]");
                    self.store
                        .complete_agent_run(&run.id, &json!({"topic_page_id":page_id}))?;
                    self.store.finish_topic_compile(
                        base_id,
                        &claim,
                        &run.id,
                        &markdown,
                        &[excerpt],
                        &[],
                    )
                })();
                if let Err(error) = &result {
                    let _ = self.store.fail_agent_run(&run.id, &error.to_string());
                }
                result
            })();
            if let Err(error) = written {
                self.store.fail_topic_compile(&claim, &error.to_string())?;
                return Err(error);
            }
        }
        self.store.topic_page(base_id, &page_id)
    }

    pub async fn compile_topic(
        &self,
        base_id: &str,
        request: TopicCompileRequest,
    ) -> Result<TopicPage, BrainError> {
        self.compile_topic_inner(base_id, request, None, None).await
    }

    pub(super) async fn compile_planned_topic(
        &self,
        base_id: &str,
        request: TopicCompileRequest,
        plan: &TopicWritingPlan,
        retry: Option<&BrainError>,
    ) -> Result<TopicPage, BrainError> {
        self.compile_topic_inner(base_id, request, Some(plan), retry)
            .await
    }

    async fn compile_topic_inner(
        &self,
        base_id: &str,
        mut request: TopicCompileRequest,
        plan: Option<&TopicWritingPlan>,
        retry: Option<&BrainError>,
    ) -> Result<TopicPage, BrainError> {
        let base = self.store.get_syncable_base(base_id)?;
        request.title = request
            .title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        request.guidance = request.guidance.trim().to_owned();
        if request.title.is_empty()
            || request.title.chars().count() > 120
            || request.guidance.chars().count() > 2_000
            || !(1..=MAX_SOURCE_EXCERPTS).contains(&request.sources.len())
        {
            return Err(BrainError::KnowledgeValidation(
                "请填写主题标题（最多 120 字），选择 1–12 个来源范围，编辑要求最多 2000 字".into(),
            ));
        }
        let page_id = request
            .page_id
            .clone()
            .unwrap_or_else(|| topic_id(base_id, &request.title));
        let page = self.store.find_topic_page(base_id, &page_id)?;
        if request.page_id.is_some() && page.is_none() {
            return Err(BrainError::KnowledgeNotFound(page_id));
        }
        if let Some(page) = &page {
            request.title = page.page.title.clone();
        }
        let previous = page.as_ref().and_then(|page| page.published.as_ref());
        let prior_links = previous
            .into_iter()
            .flat_map(|revision| &revision.links)
            .map(|link| link.link.clone())
            .collect::<Vec<_>>();
        let links = plan.map_or(prior_links.as_slice(), |plan| plan.links.as_slice());
        let questions = page
            .as_ref()
            .into_iter()
            .flat_map(|page| &page.questions)
            .filter(|item| item.resolved_at.is_none())
            .take(8)
            .map(|item| item.question.chars().take(350).collect::<String>())
            .collect::<Vec<_>>();
        let excerpts = compile_excerpts(&self.store, base_id, &request)?;
        if plan.is_some_and(|plan| {
            plan.required_documents.iter().any(|document| {
                !excerpts
                    .iter()
                    .any(|excerpt| &excerpt.document_id == document)
            })
        }) {
            return Err(BrainError::KnowledgeValidation(
                "本页没有取到分配给它的全部正文来源，需要调整该页的范围或取证预算".into(),
            ));
        }
        let required = plan.map(|plan| {
            let numbers = topic_evidence_numbers(&excerpts);
            plan.required_documents
                .iter()
                .map(|document| {
                    excerpts
                        .iter()
                        .zip(&numbers)
                        .filter(|(excerpt, _)| &excerpt.document_id == document)
                        .map(|(excerpt, &number)| (number, excerpt.source_path.as_str()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        });
        let profile = self.active_runtime_profile()?;
        let rules = self.topic_rules(base_id, "knowledge.md")?;
        let source_identity = excerpts
            .iter()
            .map(|excerpt| {
                json!([
                    excerpt.span_id,
                    excerpt.version_id,
                    excerpt.offset,
                    hash_text(&excerpt.content),
                    excerpt.source_path,
                    excerpt.heading
                ])
            })
            .collect::<Vec<_>>();
        let input_hash = hash_text(
            &json!([
                WIKI_EDITORIAL_RULES,
                PAGE_PROMPT,
                base.book_name,
                request.title,
                request.guidance,
                rules,
                questions,
                source_identity,
                required,
                links,
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
        let planned = required.is_some();
        let compact_retry = planned && retry.is_some_and(topic_transport_timed_out);
        let context = json!({
            "book": base.book_name,
            "topic": request.title,
            "editor_request": request.guidance,
            "book_schema": rules,
            "related_pages": links,
            "open_questions": questions,
            "published_markdown": previous.map(|revision| revision.markdown.as_str()),
            "required_originals": required,
            "previous_attempt_issue": retry.map(ToString::to_string),
            "retry_editorial_instruction": compact_retry.then_some(
                "优先完成一篇紧凑的主题页。直接回答本页问题，用少量小节交代机制、固定算例和适用条件。保持所有要求的原文引用，省去逐段复述、重复结论和与主题无关的待验证问题。"
            ),
            "sources": excerpt_payload(&excerpts),
        });
        let prompt = format!("{WIKI_EDITORIAL_RULES}\n\n{PAGE_PROMPT}\n\n<page_material>\n{context}\n</page_material>");
        let requested = if compact_retry {
            3_200
        } else if planned {
            5_120
        } else {
            6_144
        };
        let budget = topic_output_budget(&profile, &prompt, requested)?;
        let (page_id, claim) = self.store.claim_topic_compile(
            base_id,
            &request,
            previous.map(|revision| revision.id.clone()),
            input_hash,
            plan.map(|plan| plan.expected_draft.as_deref()),
        )?;
        if let Some(claim) = claim {
            let input = json!({
                "topic_page_id": claim.page_id,
                "topic_book_build_id": plan.map(|plan| &plan.build_id),
                "source_span_ids": excerpts.iter().map(|excerpt| &excerpt.span_id).collect::<Vec<_>>(),
                "request_timeout_seconds": if planned { 110 } else { COMPILE_TIMEOUT_SECONDS },
                "request_max_output_tokens": budget,
                "bounded_extraction": planned && profile.provider_config.as_ref()
                    .is_some_and(|provider| provider.reasoning_policy == "auto"),
                "required_topic_citations": required,
                "retry": retry.is_some(),
            });
            let result = async {
                let (run_id, answer) = self
                    .run_audited_with_started(
                        base_id,
                        "wiki_topic_compile",
                        &input,
                        &profile,
                        prompt,
                        AuditedRunObservers {
                            stream: None,
                            started_run_id: None,
                            topic_evidence: Some(&excerpts),
                        },
                    )
                    .await?;
                let cited = citation_numbers(&answer);
                let cited_excerpts = topic_evidence_numbers(&excerpts)
                    .into_iter()
                    .zip(excerpts)
                    .filter_map(|(number, excerpt)| cited.contains(&number).then_some(excerpt))
                    .collect::<Vec<_>>();
                self.store.finish_topic_compile(
                    base_id,
                    &claim,
                    &run_id,
                    answer.trim(),
                    &cited_excerpts,
                    links,
                )
            }
            .await;
            if let Err(error) = result {
                if let Err(store_error) = self.store.fail_topic_compile(&claim, &error.to_string())
                {
                    tracing::error!(page_id, error = %store_error, "保存主题编译失败状态时出错");
                }
                return Err(error);
            }
        }
        self.store.topic_page(base_id, &page_id)
    }

    pub fn publish_topic(
        &self,
        base_id: &str,
        page_id: &str,
        revision_id: &str,
    ) -> Result<TopicPage, BrainError> {
        let draft = self.store.topic_revision(base_id, page_id, revision_id)?;
        let sections = split_topic_sections(&draft.markdown);
        self.store
            .publish_topic_sections(base_id, page_id, revision_id, &sections)
    }

    pub fn publish_topic_build(
        &self,
        base_id: &str,
        attempt_id: &str,
    ) -> Result<usize, BrainError> {
        let build = self
            .store
            .topic_book_build(base_id)?
            .filter(|build| {
                build.id == attempt_id
                    && build.phase == "finished"
                    && !build.outdated
                    && build.error.is_none()
            })
            .ok_or_else(|| {
                BrainError::KnowledgeConflict("请刷新并选择已结束且原文有效的编译批次".into())
            })?;
        let mut pages = Vec::new();
        for page in build
            .pages
            .iter()
            .filter(|page| page.focus.is_some() && page.error.is_none())
        {
            if let Some(revision_id) = &page.revision_id {
                let current = self.store.topic_page(base_id, &page.page_id)?;
                if let Some(draft) = current.draft.filter(|draft| &draft.id == revision_id) {
                    pages.push(TopicPublication {
                        page_id: page.page_id.clone(),
                        revision_id: draft.id,
                        sections: split_topic_sections(&draft.markdown),
                    });
                }
            }
        }
        self.store
            .publish_topic_build_sections(base_id, attempt_id, &pages)
    }

    pub fn edit_topic_draft(
        &self,
        base_id: &str,
        page_id: &str,
        revision_id: &str,
        markdown: &str,
    ) -> Result<TopicPage, BrainError> {
        let draft = self.store.topic_revision(base_id, page_id, revision_id)?;
        validate_topic_output(markdown, draft.evidence_count, true)?;
        self.store.edit_topic_markdown(
            base_id,
            page_id,
            revision_id,
            markdown.trim(),
            &citation_numbers(markdown),
        )
    }

    pub async fn ask_topic(
        &self,
        base_id: &str,
        question: &str,
        conversation_id: Option<&str>,
        stream: Option<&ChatSender>,
    ) -> Result<KnowledgeAnswer, BrainError> {
        self.ask_topic_inner(base_id, question, conversation_id, stream, None)
            .await
    }

    pub(super) async fn restart_topic_qa(
        &self,
        base_id: &str,
        question: &str,
        conversation_id: Option<&str>,
        run_id: &str,
        stream: &ChatSender,
    ) -> Result<KnowledgeAnswer, BrainError> {
        let (_, previous_budget) = super::validated_qa_resume(
            &self.store,
            base_id,
            question.trim(),
            conversation_id,
            run_id,
        )?;
        let next = previous_budget.saturating_mul(2).min(12_288);
        self.ask_topic_inner(
            base_id,
            question,
            conversation_id,
            Some(stream),
            Some((run_id, previous_budget, next)),
        )
        .await
    }

    async fn ask_topic_inner(
        &self,
        base_id: &str,
        question: &str,
        conversation_id: Option<&str>,
        stream: Option<&ChatSender>,
        resume: Option<(&str, u32, u32)>,
    ) -> Result<KnowledgeAnswer, BrainError> {
        let question = question.trim();
        if question.is_empty() || question.chars().count() > 2_000 {
            return Err(BrainError::KnowledgeValidation(
                "问题不能为空或超过 2000 个字符".into(),
            ));
        }
        let base = self.store.get_active_base(base_id)?;
        let history = conversation_id
            .map(|id| self.store.recent_conversation_messages(base_id, id, 4))
            .transpose()?
            .unwrap_or_default();
        let overview = super::QaPlan::fallback(question).scope == "whole_book"
            || ["概览", "本书讲了", "这本书讲", "overview"]
                .iter()
                .any(|term| question.contains(term));
        let mut pages = self
            .store
            .search_topic_sections(base_id, question, overview)?;
        let previous_question = history.iter().rev().find(|message| message.role == "user");
        if pages.is_empty() && !overview {
            if let Some(previous) = previous_question {
                pages = self
                    .store
                    .search_topic_sections(base_id, &previous.content, false)?;
            }
        }
        let profile = self.active_runtime_profile()?;
        let mut excerpts = query_excerpts(&self.store, base_id, question, 8)?;
        if excerpts.len() < 2 {
            if let Some(previous) = previous_question {
                for excerpt in query_excerpts(&self.store, base_id, &previous.content, 3)? {
                    if !excerpts
                        .iter()
                        .any(|existing| existing.span_id == excerpt.span_id)
                    {
                        excerpts.push(excerpt);
                    }
                }
            }
        }
        let (notes, revisions) = topic_context(
            &self.store,
            base_id,
            if overview { "" } else { question },
            pages,
            &mut excerpts,
        )?;
        if !self
            .store
            .topic_evidence_is_current(base_id, &revisions, &excerpts)?
        {
            return Err(BrainError::KnowledgeConflict(
                "取证时主题页或原文已变化，请重新提问".into(),
            ));
        }
        let (published_count, stale_count) = self.store.topic_revision_counts(base_id)?;
        let rules = self.topic_rules(base_id, "questions.md")?;
        let history = history
            .iter()
            .map(|message| {
                json!({
                    "role": message.role,
                    "text": message.content.chars().take(600).collect::<String>(),
                })
            })
            .collect::<Vec<_>>();
        let context = json!({
            "book": base.book_name,
            "question": question,
            "book_questions": rules,
            "recent_conversation_for_intent": history,
            "scope": if overview { "whole_book" } else { "question" },
            "published_topics": published_count,
            "topics_awaiting_source_review": stale_count,
            "topic_sections": notes,
            "sources": excerpt_payload(&excerpts),
        });
        let prompt = format!("{WIKI_EDITORIAL_RULES}\n\n{ANSWER_PROMPT}\n\n<answer_material>\n{context}\n</answer_material>");
        let budget =
            topic_output_budget(&profile, &prompt, resume.map_or(4_096, |(_, _, next)| next))?;
        if resume.is_some_and(|(_, previous, _)| budget <= previous) {
            return Err(BrainError::KnowledgeValidation(
                "(qa_output_hard_limit) 本轮已达到可用输出上限，请缩小问题范围后重新提问".into(),
            ));
        }
        let input = json!({
            "question": question,
            "conversation_id": conversation_id,
            "wiki_mode": "topics",
            "topic_revisions": revisions,
            "source_span_ids": excerpts.iter().map(|excerpt| &excerpt.span_id).collect::<Vec<_>>(),
            "model": profile.model,
            "provider_id": profile.provider_id,
            "resume_run_id": resume.map(|(id, _, _)| id),
            "request_timeout_seconds": 120,
            "request_max_output_tokens": budget,
        });
        let (run_id, answer) = self
            .run_audited_with_started(
                base_id,
                "knowledge_qa",
                &input,
                &profile,
                prompt,
                AuditedRunObservers {
                    stream,
                    started_run_id: None,
                    topic_evidence: Some(&excerpts),
                },
            )
            .await?;
        let evidence = self.store.list_agent_run_citation_entries(&run_id)?;
        let conversation_id = self.store.save_conversation_exchange(
            base_id,
            conversation_id,
            question,
            &answer,
            &run_id,
            &evidence,
        )?;
        Ok(KnowledgeAnswer {
            run_id,
            conversation_id,
            answer,
            runtime: "deepseek_harness".into(),
            evidence,
        })
    }

    pub(super) fn topic_rules(&self, base_id: &str, name: &str) -> Result<String, BrainError> {
        Ok(self
            .store
            .list_config_documents(Some(base_id))?
            .into_iter()
            .find(|doc| doc.knowledge_base_id.as_deref() == Some(base_id) && doc.name == name)
            .map(|doc| doc.content_md.chars().take(2_000).collect())
            .unwrap_or_default())
    }
}

fn compile_excerpts(
    store: &BookWikiStore,
    base_id: &str,
    request: &TopicCompileRequest,
) -> Result<Vec<TopicExcerpt>, BrainError> {
    let mut result = Vec::new();
    let mut total = 0;
    let mut seen = BTreeSet::new();
    for selection in &request.sources {
        let (offset, count) = match &selection.range {
            Some(range)
                if selection.span_id.is_some()
                    && (1..=MAX_EXCERPT_CHARS).contains(&range.length) =>
            {
                (range.offset, range.length)
            }
            Some(_) => {
                return Err(BrainError::KnowledgeValidation(
                    "指定范围须属于一个章节，长度为 1–12000 字符".into(),
                ))
            }
            None => (0, MAX_EXCERPT_CHARS + 1),
        };
        let ids = if let Some(span_id) = &selection.span_id {
            vec![span_id.clone()]
        } else {
            store
                .list_topic_source_sections(
                    base_id,
                    &selection.document_id,
                    0,
                    MAX_SOURCE_EXCERPTS + 1,
                )?
                .into_iter()
                .map(|section| section.id)
                .collect::<Vec<_>>()
        };
        if ids.is_empty() || ids.len() > MAX_SOURCE_EXCERPTS {
            return Err(BrainError::KnowledgeValidation(
                "所选原文没有可读正文，或章节过多。请选择具体章节".into(),
            ));
        }
        for id in ids {
            let excerpt = store
                .topic_excerpt(base_id, &id, offset, count)?
                .filter(|excerpt| {
                    excerpt.document_id == selection.document_id
                        && !excerpt.content.trim().is_empty()
                })
                .ok_or_else(|| {
                    BrainError::KnowledgeConflict(
                        "选中的章节已更新或范围已失效，请重新选择来源".into(),
                    )
                })?;
            let length = excerpt.content.chars().count();
            if length > MAX_EXCERPT_CHARS || selection.range.is_some() && length != count {
                return Err(BrainError::KnowledgeValidation(
                    "章节超出单段长度或所选范围越界，请调整起点及长度".into(),
                ));
            }
            if !seen.insert((id, offset, length)) {
                continue;
            }
            total += length;
            if result.len() == MAX_SOURCE_EXCERPTS || total > MAX_SOURCE_CHARS {
                return Err(BrainError::KnowledgeValidation(
                    "单次主题编译支持至多 12 段、合计 20000 字符，请缩小章节范围".into(),
                ));
            }
            result.push(excerpt);
        }
    }
    Ok(result)
}

pub(super) fn topic_evidence_numbers(excerpts: &[TopicExcerpt]) -> Vec<usize> {
    let mut numbers = HashMap::new();
    excerpts
        .iter()
        .map(|excerpt| {
            let next = numbers.len() + 1;
            *numbers
                .entry((&excerpt.span_id, &excerpt.version_id))
                .or_insert(next)
        })
        .collect()
}

fn excerpt_payload(excerpts: &[TopicExcerpt]) -> Value {
    Value::Array(
        excerpts
            .iter()
            .zip(topic_evidence_numbers(excerpts))
            .map(|(excerpt, number)| {
                json!({
                    "citation": format!("[S{number}]"),
                    "offset_chars": excerpt.offset,
                    "path": excerpt.source_path,
                    "heading": excerpt.heading,
                    "markdown": excerpt.content,
                })
            })
            .collect(),
    )
}

fn query_excerpts(
    store: &BookWikiStore,
    base_id: &str,
    question: &str,
    limit: usize,
) -> Result<Vec<TopicExcerpt>, BrainError> {
    let mut excerpts = Vec::new();
    let terms = question_terms(question);
    let parts = question_parts(question);
    let focus = part_term_indices(&parts, &terms);
    let mut candidates = store.search_source_span_previews(base_id, question, limit * 2)?;
    if parts.len() > 1 {
        for part in parts {
            candidates.extend(store.search_source_span_previews(base_id, part, 6)?);
        }
    }
    let mut seen = HashSet::new();
    for candidate in candidates {
        let Some(span_id) = candidate["id"].as_str() else {
            continue;
        };
        let hit = candidate["preview_offset_chars"].as_u64().unwrap_or(0) as usize;
        let offset = hit.saturating_sub(32_000) / 32_000 * 32_000;
        if !seen.insert((span_id.to_owned(), offset)) {
            continue;
        }
        if let Some(source) = store.topic_excerpt(base_id, span_id, offset, 64_000)? {
            // One indexed span can be an entire chapter. Match inside it so
            // an early mention in the contents doesn't hide a later explanation.
            excerpts.extend(question_passages(&terms, &focus, source));
        }
    }
    Ok(excerpts)
}

fn question_passages(
    terms: &[String],
    focus: &[HashSet<usize>],
    source: TopicExcerpt,
) -> Vec<TopicExcerpt> {
    const PASSAGE_CHARS: usize = 1_800;
    let fragments = source_structure::structured_fragments(&source.content, 1_250);
    let mut ranked = fragments
        .iter()
        .enumerate()
        .filter_map(|(index, fragment)| {
            let next = fragments
                .get(index + 1)
                .filter(|next| next.char_end - fragment.char_start <= PASSAGE_CHARS);
            let mut content = fragment
                .content
                .chars()
                .take(PASSAGE_CHARS)
                .collect::<String>();
            if let Some(next) = next {
                content.push_str(&next.content);
            }
            if content.trim().is_empty() {
                return None;
            }
            let hits = matching_terms(terms, &content);
            let score = hits
                .iter()
                .map(|&index| if terms[index].is_ascii() { 2 } else { 1 })
                .sum::<usize>();
            Some((Reverse(score), index, hits, content))
        })
        .collect::<Vec<_>>();
    ranked.sort_unstable_by_key(|(score, index, _, _)| (*score, *index));
    let mut chosen = ranked
        .iter()
        .take(3)
        .map(|entry| entry.1)
        .collect::<HashSet<_>>();
    for part in focus {
        let mut alternatives = ranked
            .iter()
            .enumerate()
            .filter_map(|(rank, entry)| {
                let matches = entry.2.iter().filter(|term| part.contains(term)).count();
                (matches >= 2).then_some((Reverse(matches), rank, entry.1))
            })
            .collect::<Vec<_>>();
        alternatives.sort_unstable();
        chosen.extend(alternatives.into_iter().take(2).map(|entry| entry.2));
    }
    ranked
        .into_iter()
        .filter(|entry| chosen.contains(&entry.1))
        .map(|(_, index, _, content)| TopicExcerpt {
            span_id: source.span_id.clone(),
            document_id: source.document_id.clone(),
            version_id: source.version_id.clone(),
            source_path: source.source_path.clone(),
            heading: fragments[index]
                .heading_path
                .last()
                .cloned()
                .or_else(|| source.heading.clone()),
            offset: source.offset + fragments[index].char_start,
            content,
        })
        .collect()
}

fn topic_context(
    store: &BookWikiStore,
    base_id: &str,
    question: &str,
    mut pages: Vec<TopicSectionMatch>,
    excerpts: &mut Vec<TopicExcerpt>,
) -> Result<(Vec<Value>, Vec<String>), BrainError> {
    let terms = question_terms(question);
    pages.sort_by_cached_key(|page| {
        Reverse(
            matching_terms(&terms, &page.section.markdown)
                .iter()
                .map(|&index| if terms[index].is_ascii() { 3 } else { 1 })
                .sum::<usize>()
                + matching_terms(&terms, &page.section.heading).len(),
        )
    });
    let mut notes = Vec::new();
    let mut revisions = Vec::new();
    let mut counts = HashMap::new();
    let mut seen_citations = HashSet::new();
    let mut leading_section_ranges = HashSet::new();
    let mut seen_ranges = excerpts
        .iter()
        .map(|excerpt| {
            (
                excerpt.span_id.clone(),
                excerpt.offset,
                excerpt.content.chars().count(),
            )
        })
        .collect::<HashSet<_>>();
    for page in pages {
        let count = counts.entry(page.page_id).or_insert(0);
        if *count == 3 {
            continue;
        }
        *count += 1;
        if !revisions.contains(&page.revision_id) {
            revisions.push(page.revision_id);
        }
        let leading_section = notes.is_empty() && !terms.is_empty();
        // Read the section's citations before limiting the text shown to the model.
        for number in citation_numbers(&page.section.markdown) {
            if !seen_citations.insert((page.run_id.clone(), number)) {
                continue;
            }
            let citation = store.get_agent_run_citation(&page.run_id, number - 1)?;
            for range in &citation.read_ranges {
                let length = range.returned_chars.min(1_800);
                if leading_section {
                    leading_section_ranges.insert((citation.object_id.clone(), range.offset_chars));
                }
                if !seen_ranges.insert((citation.object_id.clone(), range.offset_chars, length)) {
                    continue;
                }
                if let Some(excerpt) = store
                    .topic_excerpt(base_id, &citation.object_id, range.offset_chars, length)?
                    .filter(|excerpt| excerpt.version_id == citation.version_id)
                {
                    excerpts.push(excerpt);
                }
            }
        }
        if notes.len() < 6 {
            let body = page
                .section
                .markdown
                .chars()
                .take(1_200)
                .collect::<String>();
            notes.push(json!({"topic": page.title, "section": page.section.heading, "markdown": strip_citations(&body)}));
        }
    }
    let focus = part_term_indices(&question_parts(question), &terms);
    rank_question_excerpts(&terms, &focus, &leading_section_ranges, excerpts);
    Ok((notes, revisions))
}

fn question_terms(question: &str) -> Vec<String> {
    knowledge_search_terms(question)
        .into_iter()
        .filter(|term| {
            term.chars().count() >= 2
                && !["如何", "怎么", "是否", "什么", "可能", "请问"].contains(&term.as_str())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn question_parts(question: &str) -> Vec<&str> {
    let mut parts = question
        .split(['?', '？', '。', '！', '!', '；', ';', ',', '，', '\n'])
        .map(str::trim)
        .filter(|part| part.chars().count() >= 10)
        .collect::<Vec<_>>();
    if parts.len() > 6 {
        parts.drain(3..parts.len() - 3);
    }
    parts
}

fn part_term_indices(parts: &[&str], terms: &[String]) -> Vec<HashSet<usize>> {
    parts
        .iter()
        .map(|part| {
            question_terms(part)
                .iter()
                .filter_map(|term| terms.binary_search(term).ok())
                .collect()
        })
        .collect()
}

fn matching_terms(terms: &[String], text: &str) -> Vec<usize> {
    let keys = knowledge_search_terms(text)
        .into_iter()
        .collect::<HashSet<_>>();
    terms
        .iter()
        .enumerate()
        .filter_map(|(index, term)| keys.contains(term).then_some(index))
        .collect()
}

fn rank_question_excerpts(
    terms: &[String],
    focus: &[HashSet<usize>],
    leading_section_ranges: &HashSet<(String, usize)>,
    excerpts: &mut Vec<TopicExcerpt>,
) {
    let mut term_counts = vec![0; terms.len()];
    let candidates = std::mem::take(excerpts)
        .into_iter()
        .map(|excerpt| {
            let matches = matching_terms(terms, &excerpt.content);
            for &term in &matches {
                term_counts[term] += 1;
            }
            let length = excerpt.content.chars().count();
            (excerpt, length, matches)
        })
        .collect::<Vec<_>>();
    let weights = term_counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            let rarity = candidates.len().saturating_add(1) / (count + 1);
            rarity.clamp(1, 8) * if terms[index].is_ascii() { 2 } else { 1 }
        })
        .collect::<Vec<_>>();
    let mut candidates = candidates
        .into_iter()
        .map(|(excerpt, length, hits)| {
            let mut focused = vec![0; focus.len()];
            if !focus.is_empty() {
                for paragraph in excerpt
                    .content
                    .split("\n\n")
                    .flat_map(|text| text.split("\r\n\r\n"))
                {
                    let hits = matching_terms(terms, paragraph);
                    for (score, part) in focused.iter_mut().zip(focus) {
                        let paragraph_score = hits
                            .iter()
                            .filter(|term| part.contains(term))
                            .map(|&term| weights[term])
                            .sum::<usize>();
                        *score = (*score).max(paragraph_score);
                    }
                }
            }
            let from_leading_section =
                leading_section_ranges.contains(&(excerpt.span_id.clone(), excerpt.offset));
            (excerpt, length, hits, focused, from_leading_section)
        })
        .collect::<Vec<_>>();
    let mut covered = vec![false; terms.len()];
    let mut document_counts = HashMap::<String, usize>::new();
    let mut total_chars = 0;
    let mut reserved = 0;
    while excerpts.len() < MAX_SOURCE_EXCERPTS {
        let next = candidates
            .iter()
            .enumerate()
            .filter(|(_, (source, length, _, _, _))| {
                total_chars + length <= MAX_SOURCE_CHARS
                    && document_counts
                        .get(&source.document_id)
                        .copied()
                        .unwrap_or(0)
                        < 3
                    && !excerpts.iter().any(|prior| {
                        if prior.span_id != source.span_id || prior.version_id != source.version_id
                        {
                            return false;
                        }
                        let prior_length = prior.content.chars().count();
                        let end = (prior.offset + prior_length).min(source.offset + length);
                        let overlap = end.saturating_sub(prior.offset.max(source.offset));
                        overlap * 2 >= prior_length.min(*length)
                    })
            })
            .max_by_key(|(index, (_, _, hits, focused, from_leading_section))| {
                let score = hits
                    .iter()
                    .map(|&term| weights[term] * if covered[term] { 1 } else { 3 })
                    .sum::<usize>();
                (
                    excerpts.len() < 2 && *from_leading_section,
                    focused.get(excerpts.len() - reserved).copied().unwrap_or(0),
                    score,
                    Reverse(*index),
                )
            })
            .map(|(index, _)| index);
        let Some(index) = next else { break };
        let (excerpt, length, hits, _, from_leading_section) = candidates.remove(index);
        if excerpts.len() < 2 && from_leading_section {
            reserved += 1;
        }
        for term in hits {
            covered[term] = true;
        }
        total_chars += length;
        *document_counts
            .entry(excerpt.document_id.clone())
            .or_default() += 1;
        excerpts.push(excerpt);
    }
}

pub(super) fn topic_output_budget(
    profile: &RuntimeProfile,
    prompt: &str,
    requested: u32,
) -> Result<u32, BrainError> {
    let provider_limit = super::effective_output_cap(profile, Some(requested)).unwrap_or(requested);
    let input = estimated_tokens(prompt).saturating_add(1_024);
    let context_limit = profile
        .provider_config
        .as_ref()
        .and_then(|provider| provider.context_window)
        .map(|window| u64::from(window).saturating_sub(input));
    let available = u64::from(provider_limit).min(context_limit.unwrap_or(u64::MAX));
    if available < 768 {
        return Err(BrainError::KnowledgeValidation(
            "当前模型留给正文的空间不足，请缩小来源范围或调整模型容量".into(),
        ));
    }
    Ok(available as u32)
}

fn split_topic_sections(markdown: &str) -> Vec<TopicSection> {
    let headings = source_structure::parse_document(markdown).headings;
    let starts = std::iter::once((0, "导读".to_string()))
        .chain(
            headings
                .into_iter()
                .filter(|heading| heading.level <= 2)
                .map(|heading| (heading.start, heading.title)),
        )
        .collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, (start, title))| {
            let end = starts
                .get(index + 1)
                .map_or(markdown.len(), |(start, _)| *start);
            let section = markdown[*start..end].trim();
            (!section.is_empty()).then(|| TopicSection {
                heading: title.clone(),
                markdown: section.into(),
            })
        })
        .collect()
}

pub(super) fn validate_topic_output(
    markdown: &str,
    count: usize,
    is_page: bool,
) -> Result<(), BrainError> {
    let output = markdown.trim();
    if output.is_empty() || is_page && output.chars().count() > PAGE_CHARS {
        return Err(BrainError::KnowledgeValidation(
            "模型返回了空正文或主题页过长，请缩小范围重试".into(),
        ));
    }
    let references = citation_numbers(output);
    if let Some(number) = references
        .iter()
        .find(|&&number| number == 0 || number > count)
    {
        return Err(BrainError::KnowledgeValidation(format!(
            "正文使用了未分配的原文引用 [S{number}]"
        )));
    }
    if is_page && references.is_empty() {
        return Err(BrainError::KnowledgeValidation(
            "主题稿缺少可核对的行内原文引用，请重试".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_planned_topic_output(
    markdown: &str,
    required: &Value,
) -> Result<(), BrainError> {
    let Some(documents) = required.as_array() else {
        return Ok(());
    };
    let cited = citation_numbers(markdown);
    for document in documents.iter().filter_map(Value::as_array) {
        if !document.iter().any(|reference| {
            reference[0]
                .as_u64()
                .is_some_and(|number| cited.contains(&(number as usize)))
        }) {
            let path = document
                .first()
                .and_then(|reference| reference[1].as_str())
                .unwrap_or("正文");
            return Err(BrainError::KnowledgeValidation(format!(
                "主题页未实际引用分配的原文「{path}」，请把它提供的依据写入正文"
            )));
        }
    }
    let mut in_cell = false;
    let mut cell = String::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::TableCell) => {
                in_cell = true;
                cell.clear();
            }
            Event::Text(value) | Event::Code(value) if in_cell => cell.push_str(&value),
            Event::End(TagEnd::TableCell) => {
                in_cell = false;
                if cell.split_whitespace().any(|part| {
                    part.strip_suffix(['?', '？']).is_some_and(|prefix| {
                        prefix.bytes().any(|ch| ch.is_ascii_digit())
                            && prefix
                                .bytes()
                                .all(|ch| ch.is_ascii_digit() || b",.+-".contains(&ch))
                    })
                }) {
                    return Err(BrainError::KnowledgeValidation(format!(
                        "表格存在未填完的数字占位「{}」，请核对并填完或写明未知",
                        cell.trim().chars().take(40).collect::<String>()
                    )));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn citation_numbers(markdown: &str) -> BTreeSet<usize> {
    let mut numbers = BTreeSet::new();
    let mut skipped = 0usize;
    let mut text = String::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Image { .. }) => {
                numbers.extend(citation_markers(&text).map(|(_, _, number)| number));
                text.clear();
                skipped += 1;
            }
            Event::End(TagEnd::CodeBlock | TagEnd::Link | TagEnd::Image) => {
                skipped = skipped.saturating_sub(1)
            }
            Event::Text(value) if skipped == 0 => text.push_str(&value),
            _ => {
                numbers.extend(citation_markers(&text).map(|(_, _, number)| number));
                text.clear();
            }
        }
    }
    numbers.extend(citation_markers(&text).map(|(_, _, number)| number));
    numbers
}

fn strip_citations(markdown: &str) -> String {
    let mut output = String::new();
    let mut start = 0;
    for (from, to, _) in citation_markers(markdown) {
        output.push_str(&markdown[start..from]);
        start = to;
    }
    output.push_str(&markdown[start..]);
    output
}

fn citation_markers(text: &str) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    text.match_indices("[S").filter_map(|(start, _)| {
        let tail = &text[start + 2..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        (digits > 0 && tail.as_bytes().get(digits) == Some(&b']')).then(|| {
            (
                start,
                start + 3 + digits,
                tail[..digits].parse().unwrap_or(usize::MAX),
            )
        })
    })
}
