use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Value};

use crate::error::BrainError;
use crate::infra::book_wiki_store::{knowledge_search_terms, BookWikiStore};
use crate::models::wiki_topics::{
    TopicBookDocument, TopicBookPlan, TopicBookProposal, TopicCompileRequest, TopicSourceRange,
    TopicSourceSelection, TopicWikiIndexPage,
};

use super::super::source_structure::{parse_document, structured_fragments, DocumentStructure};
use super::super::topic_pages::{MAX_SOURCE_CHARS, MAX_SOURCE_EXCERPTS};

pub(super) const MAX_TOPICS: usize = 12;
const PASSAGE_CHARS: usize = 1_550;
const MAX_QUERIES: usize = 6;
const MAX_FOCUS_CHARS: usize = 300;
const MAX_QUERY_CHARS: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DocumentRole {
    Chapter,
    Guide,
    Reference,
}

struct Document<'a> {
    source: &'a TopicBookDocument,
    role: DocumentRole,
    title: String,
    lead: String,
    headings: Vec<String>,
    guide_case: Option<String>,
}

struct Passage<'a> {
    document: usize,
    span_id: &'a str,
    offset: usize,
    length: usize,
    terms: HashSet<String>,
    heading_terms: HashSet<String>,
    heading_key: String,
    has_body: bool,
    worked_example: u8,
}

pub(super) struct BookMaterial<'a> {
    documents: Vec<Document<'a>>,
    chapters: Vec<usize>,
    passages: Vec<Passage<'a>>,
    frequencies: HashMap<String, usize>,
}

impl<'a> BookMaterial<'a> {
    pub(super) fn read(
        store: &BookWikiStore,
        base_id: &str,
        inventory: &'a [TopicBookDocument],
    ) -> Result<Self, BrainError> {
        let mut book = Self {
            documents: Vec::new(),
            chapters: Vec::new(),
            passages: Vec::new(),
            frequencies: HashMap::new(),
        };
        for source in inventory {
            if source.sections.is_empty() {
                continue;
            }
            let position = book.documents.len();
            let mut document = Document {
                source,
                role: document_role(&source.path),
                title: String::new(),
                lead: String::new(),
                headings: Vec::new(),
                guide_case: None,
            };
            for section in &source.sections {
                let span = store.get_current_source_span(base_id, &section.id)?;
                if span.source_version_id != source.version {
                    return Err(BrainError::KnowledgeConflict(
                        "整理全书章节时原文版本已变化".into(),
                    ));
                }
                if document.lead.is_empty() {
                    document.title = section.heading.clone().unwrap_or_else(|| {
                        source
                            .path
                            .rsplit('/')
                            .next()
                            .unwrap_or(&source.path)
                            .into()
                    });
                    document.lead = span.content.chars().take(2_400).collect();
                }
                let structure = parse_document(&span.content);
                if document.role == DocumentRole::Chapter {
                    document.headings.extend(
                        structure
                            .headings
                            .iter()
                            .filter(|heading| heading.level <= 3 && heading.title != document.title)
                            .map(|heading| heading.title.chars().take(56).collect::<String>()),
                    );
                } else if document.role == DocumentRole::Guide && document.guide_case.is_none() {
                    document.guide_case = structure
                        .headings
                        .iter()
                        .find(|heading| heading.level > 1 && is_case_heading(&heading.title))
                        .map(|heading| span.content[heading.start..].chars().take(600).collect());
                }
                book.index_span(position, &section.id, &span.content, &structure);
            }
            if document.role == DocumentRole::Chapter {
                book.chapters.push(position);
            }
            book.documents.push(document);
        }
        if book.chapters.is_empty() {
            book.chapters.extend(0..book.documents.len());
        }
        Ok(book)
    }

    fn index_span(
        &mut self,
        document: usize,
        span_id: &'a str,
        content: &str,
        structure: &DocumentStructure,
    ) {
        // Start each evidence window in its own section. A late example must
        // rank under its real heading, even when the source span is an entire file.
        let starts = std::iter::once(0)
            .chain(
                structure
                    .headings
                    .iter()
                    .map(|heading| heading.start)
                    .filter(|&start| start > 0),
            )
            .chain(std::iter::once(content.len()))
            .collect::<Vec<_>>();
        let mut section_offset = 0;
        for bounds in starts.windows(2) {
            let section = &content[bounds[0]..bounds[1]];
            for fragment in structured_fragments(section, PASSAGE_CHARS) {
                let heading = fragment.heading_path.last().map_or("", String::as_str);
                let heading_terms = search_terms(heading);
                let heading_key = heading_key(heading);
                let mut offset = section_offset + fragment.char_start;
                let mut text = fragment.content.as_str();
                while !text.is_empty() {
                    let end = text
                        .char_indices()
                        .nth(PASSAGE_CHARS * 2)
                        .map_or(text.len(), |(index, _)| index);
                    let (body, rest) = text.split_at(end);
                    let length = body.chars().count();
                    if !body.trim().is_empty() {
                        let terms = search_terms(body);
                        for term in &terms {
                            *self.frequencies.entry(term.clone()).or_default() += 1;
                        }
                        self.passages.push(Passage {
                            document,
                            span_id,
                            offset,
                            length,
                            terms,
                            heading_terms: heading_terms.clone(),
                            heading_key: heading_key.clone(),
                            has_body: body
                                .lines()
                                .map(str::trim)
                                .any(|line| !line.is_empty() && !line.starts_with('#')),
                            worked_example: worked_example_priority(heading, body),
                        });
                    }
                    offset += length;
                    text = rest;
                }
            }
            section_offset += section.chars().count();
        }
    }

    pub(super) fn chapter_count(&self) -> usize {
        self.chapters.len()
    }

    pub(super) fn chapters_per_topic(&self) -> usize {
        self.chapters.len().div_ceil(self.topic_limit()).max(4)
    }

    pub(super) fn topic_limit(&self) -> usize {
        let chars = self
            .chapters
            .iter()
            .flat_map(|&index| &self.documents[index].source.sections)
            .map(|section| section.total_chars)
            .sum::<usize>();
        self.chapters
            .len()
            .max(chars.div_ceil(36_000))
            .clamp(1, MAX_TOPICS)
    }

    pub(super) fn chapter_outline(&self, lead_chars: usize) -> Vec<Value> {
        self.chapters
            .iter()
            .enumerate()
            .map(|(index, &position)| {
                let document = &self.documents[position];
                let headings = if lead_chars == 0 {
                    Vec::new()
                } else {
                    let step = document.headings.len().div_ceil(9);
                    document
                        .headings
                        .iter()
                        .step_by(step.max(1))
                        .take(9)
                        .collect()
                };
                json!({
                    "number":index+1,
                    "file":document.source.path.chars().take(150).collect::<String>(),
                    "title":document.title.chars().take(110).collect::<String>(),
                    "headings":headings,
                    "intro":document.lead.chars().take(lead_chars).collect::<String>(),
                })
            })
            .collect()
    }

    pub(super) fn guide_excerpts(&self, limit: usize) -> Vec<Value> {
        self.documents
            .iter()
            .filter(|doc| doc.role == DocumentRole::Guide)
            .take(2)
            .map(|doc| {
                let intro_limit = if doc.guide_case.is_some() {
                    limit / 2
                } else {
                    limit
                };
                json!({
                    "from":doc.source.path,
                    "intro":doc.lead.chars().take(intro_limit).collect::<String>(),
                    "case":doc.guide_case.as_deref().unwrap_or_default()
                        .chars().take(limit - intro_limit).collect::<String>(),
                })
            })
            .collect()
    }

    pub(super) fn validate_plan(
        &self,
        plan: &mut TopicBookPlan,
        existing: &[TopicWikiIndexPage],
    ) -> Result<(), BrainError> {
        let max_topics = self.topic_limit();
        if !(1..=max_topics).contains(&plan.topics.len()) {
            return Err(BrainError::KnowledgeValidation(format!(
                "全书主题数量须为 1–{max_topics}，实际返回 {} 篇",
                plan.topics.len()
            )));
        }
        plan.overview = plan.overview.trim().to_owned();
        if plan.overview.chars().count() > 600 {
            return Err(BrainError::KnowledgeValidation(
                "全书导览应在 600 字以内".into(),
            ));
        }
        let independence_reason = plan
            .independent_topics_reason
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if plan.topics.len() > 1
            && plan.topics.iter().all(|topic| topic.related.is_empty())
            && !(1..=300).contains(&independence_reason.chars().count())
        {
            return Err(BrainError::KnowledgeValidation(
                "整书主题缺少阅读关系，请给出有实际阅读价值的关联，或说明各篇主题为何独立".into(),
            ));
        }
        let mut titles = HashSet::new();
        let mut assigned = HashSet::new();
        let topic_count = plan.topics.len();
        for (index, topic) in plan.topics.iter_mut().enumerate() {
            if let Some(id) = &topic.reuse_page_id {
                let previous = existing
                    .iter()
                    .find(|page| &page.page_id == id)
                    .ok_or_else(|| {
                        BrainError::KnowledgeValidation(format!(
                            "主题「{}」试图延续未提供的主题页",
                            topic.title
                        ))
                    })?;
                topic.title = previous.title.clone();
            }
            topic.title = topic.title.split_whitespace().collect::<Vec<_>>().join(" ");
            topic.focus = topic.focus.trim().to_string();
            if topic.title.is_empty()
                || topic.title.chars().count() > 120
                || !titles.insert(topic.title.to_lowercase())
            {
                return Err(BrainError::KnowledgeValidation(
                    "主题标题须为 1–120 字，合并同名主题".into(),
                ));
            }
            if topic.focus.is_empty()
                || topic.focus.chars().count() > MAX_FOCUS_CHARS
                || topic.queries.len() > MAX_QUERIES
                || topic
                    .queries
                    .iter()
                    .any(|query| query.chars().count() > MAX_QUERY_CHARS)
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "主题「{}」的 focus 须为 1–{MAX_FOCUS_CHARS} 字，queries 至多 {MAX_QUERIES} 条且每条最多 {MAX_QUERY_CHARS} 字",
                    topic.title
                )));
            }
            if topic.sources.is_empty()
                || topic
                    .sources
                    .iter()
                    .any(|number| !(1..=self.chapters.len()).contains(number))
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "主题「{}」使用了无效原文编号，sources 须使用 chapters 中的编号",
                    topic.title
                )));
            }
            let mut seen = HashSet::new();
            topic.sources.retain(|number| seen.insert(*number));
            if topic.sources.len() > self.chapters_per_topic() {
                return Err(BrainError::KnowledgeValidation(format!(
                    "主题「{}」分配的正文超过 {} 篇，请按独立问题补充或调整主题",
                    topic.title,
                    self.chapters_per_topic()
                )));
            }
            assigned.extend(topic.sources.iter().copied());
            let mut targets = HashSet::new();
            if topic.related.len() > 4
                || topic.related.iter_mut().any(|relation| {
                    relation.reason = relation.reason.trim().to_owned();
                    relation.topic == index + 1
                        || !(1..=topic_count).contains(&relation.topic)
                        || !targets.insert(relation.topic)
                        || !(1..=160).contains(&relation.reason.chars().count())
                })
            {
                return Err(BrainError::KnowledgeValidation(format!(
                    "主题「{}」的关联页编号或阅读理由无效",
                    topic.title
                )));
            }
        }
        if assigned.len() != self.chapters.len() {
            let missing = (1..=self.chapters.len())
                .filter(|number| !assigned.contains(number))
                .take(20)
                .map(|number| number.to_string())
                .collect::<Vec<_>>()
                .join("、");
            return Err(BrainError::KnowledgeValidation(format!(
                "以下正文原文编号未分配主题，请纳入已有主题：{missing}"
            )));
        }
        Ok(())
    }

    /// Complete the placement of chapters omitted from an otherwise editable
    /// outline. This assigns sources to a writer, who still has to read and cite
    /// them. Prefer a topic's own terms, then nearby chapters for broad headings.
    pub(super) fn place_unassigned_chapters(&self, plan: &mut TopicBookPlan) -> Vec<usize> {
        if !(1..=self.topic_limit()).contains(&plan.topics.len())
            || plan.topics.iter().any(|topic| {
                topic.sources.is_empty()
                    || topic
                        .sources
                        .iter()
                        .any(|number| !(1..=self.chapters.len()).contains(number))
            })
        {
            return Vec::new();
        }
        let terms = plan
            .topics
            .iter()
            .map(|topic| {
                search_terms(&format!(
                    "{} {} {}",
                    topic.title,
                    topic.focus,
                    topic.queries.join(" ")
                ))
            })
            .collect::<Vec<_>>();
        let assigned = plan
            .topics
            .iter()
            .flat_map(|topic| topic.sources.iter().copied())
            .collect::<HashSet<_>>();
        let mut placed = Vec::new();
        for number in (1..=self.chapters.len()).filter(|number| !assigned.contains(number)) {
            let document = &self.documents[self.chapters[number - 1]];
            let heading = search_terms(&format!(
                "{} {}",
                document.title,
                document.headings.join(" ")
            ));
            let lead = search_terms(&document.lead);
            let limit = self.chapters_per_topic();
            let best = plan
                .topics
                .iter()
                .enumerate()
                .filter(|(_, topic)| topic.sources.len() < limit)
                .max_by_key(|(index, topic)| {
                    let lexical = terms[*index].intersection(&heading).count() * 3
                        + terms[*index].intersection(&lead).count();
                    let distance = topic
                        .sources
                        .iter()
                        .map(|other| number.abs_diff(*other))
                        .min()
                        .unwrap_or(usize::MAX);
                    (
                        lexical,
                        Reverse(distance),
                        Reverse(topic.sources.len()),
                        Reverse(*index),
                    )
                });
            if let Some((index, _)) = best {
                plan.topics[index].sources.push(number);
                placed.push(number);
            }
        }
        placed
    }

    pub(super) fn assigned_chapters(&self, topic: &TopicBookProposal) -> Vec<String> {
        topic
            .sources
            .iter()
            .map(|&number| self.documents[self.chapters[number - 1]].source.id.clone())
            .collect()
    }

    pub(super) fn revision_request(
        &self,
        page: &crate::models::wiki_topics::TopicBookBuildPage,
        char_budget: usize,
    ) -> (TopicCompileRequest, Vec<String>) {
        let positions = page
            .assigned_documents
            .iter()
            .filter_map(|id| {
                self.documents
                    .iter()
                    .position(|document| &document.source.id == id)
            })
            .collect::<Vec<_>>();
        let queries = page
            .queries
            .iter()
            .chain(&page.update_notes)
            .cloned()
            .collect::<Vec<_>>();
        let mut guidance = page.focus.clone().unwrap_or_default();
        if !page.update_notes.is_empty() {
            let notes = page.update_notes.join("\n");
            guidance.push_str("\n\n本轮编辑依据\n");
            guidance.extend(notes.chars().take(1_500));
        }
        self.request_by_position(&page.title, &guidance, &queries, &positions, char_budget)
    }

    fn request_by_position(
        &self,
        title: &str,
        focus: &str,
        queries: &[String],
        required: &[usize],
        char_budget: usize,
    ) -> (TopicCompileRequest, Vec<String>) {
        let query_terms = queries
            .iter()
            .map(|query| (heading_key(query), search_terms(query)))
            .collect::<Vec<_>>();
        let overview_terms = search_terms(&format!("{title} {focus}"));
        let relevance = self
            .passages
            .iter()
            .map(|passage| {
                self.score(passage, &overview_terms)
                    + query_terms
                        .iter()
                        .map(|(_, terms)| self.score(passage, terms))
                        .max()
                        .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut per_document = HashMap::<usize, usize>::new();
        let mut total = 0;
        let primary_ids = required
            .iter()
            .map(|&index| self.documents[index].source.id.clone())
            .collect();
        let mut add = |index: usize| {
            let passage = &self.passages[index];
            let count = per_document.entry(passage.document).or_default();
            let limit = if required.contains(&passage.document) {
                MAX_QUERIES + 2
            } else {
                2
            };
            if selected.len() == MAX_SOURCE_EXCERPTS
                || total + passage.length > char_budget.min(MAX_SOURCE_CHARS)
                || *count == limit
                || selected.contains(&index)
            {
                return false;
            }
            *count += 1;
            total += passage.length;
            selected.push(index);
            true
        };
        for document in required {
            if let Some((index, _)) = self
                .passages
                .iter()
                .enumerate()
                .filter(|(_, passage)| passage.document == *document)
                .max_by_key(|(index, _)| (relevance[*index], Reverse(*index)))
            {
                add(index);
            }
        }
        // Titles often occur again in a chapter's table of contents. Keep its
        // worked example separately, then let exact headings lead each lookup.
        let mut examples = Vec::new();
        for document in required {
            if let Some((index, _)) = self
                .passages
                .iter()
                .enumerate()
                .filter(|(_, passage)| {
                    passage.document == *document && passage.worked_example > 0 && passage.has_body
                })
                .max_by_key(|(index, passage)| {
                    (passage.worked_example, relevance[*index], Reverse(*index))
                })
            {
                add(index);
                examples.push(index);
            }
        }
        // Exact mechanisms, examples and counterexamples can appear far outside
        // a chapter's synopsis, including in chapters assigned to another page.
        for (query, terms) in &query_terms {
            if let Some((index, _, score)) = self
                .passages
                .iter()
                .enumerate()
                .map(|(index, passage)| {
                    (
                        index,
                        query.chars().count() >= 6
                            && passage.heading_key.contains(query)
                            && passage.has_body,
                        self.prefer_chapter(passage, self.score(passage, terms), required),
                    )
                })
                .max_by_key(|&(index, exact, score)| (exact, score, Reverse(index)))
            {
                if score > 0 {
                    add(index);
                    let passage = &self.passages[index];
                    if passage.worked_example > 0
                        && required.contains(&passage.document)
                        && !examples.contains(&index)
                    {
                        examples.push(index);
                    }
                }
            }
        }
        // A chapter can put the setup and the calculation in separate sections.
        for index in examples {
            let document = self.passages[index].document;
            let mut neighbors = index
                .checked_sub(1)
                .into_iter()
                .chain([index + 1])
                .filter(|&other| {
                    self.passages.get(other).is_some_and(|passage| {
                        passage.document == document
                            && passage.worked_example > 0
                            && passage.has_body
                    })
                })
                .collect::<Vec<_>>();
            neighbors.sort_by_key(|&other| {
                Reverse((self.passages[other].worked_example, relevance[other]))
            });
            for neighbor in neighbors {
                if add(neighbor) {
                    break;
                }
            }
        }
        let mut ranked = self
            .passages
            .iter()
            .enumerate()
            .filter_map(|(index, passage)| {
                let score = self.prefer_chapter(passage, relevance[index], required);
                (score > 0).then_some((index, score))
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|&(index, score)| (Reverse(score), index));
        // Keep writing context lean after adding an excerpt from each chapter.
        for (index, _) in ranked.into_iter().take(8) {
            add(index);
        }
        let sources = selected
            .into_iter()
            .map(|index| {
                let passage = &self.passages[index];
                TopicSourceSelection {
                    document_id: self.documents[passage.document].source.id.clone(),
                    span_id: Some(passage.span_id.into()),
                    range: Some(TopicSourceRange {
                        offset: passage.offset,
                        length: passage.length,
                    }),
                }
            })
            .collect();
        (
            TopicCompileRequest {
                page_id: None,
                title: title.to_owned(),
                guidance: focus.to_owned(),
                sources,
            },
            primary_ids,
        )
    }

    fn score(&self, passage: &Passage<'_>, terms: &HashSet<String>) -> usize {
        terms
            .iter()
            .filter(|term| passage.terms.contains(*term) || passage.heading_terms.contains(*term))
            .map(|term| {
                let frequency = self.frequencies.get(term).copied().unwrap_or_default();
                let weight = (self.passages.len().max(1) / (1 + frequency)).clamp(1, 16);
                weight
                    * if passage.heading_terms.contains(term) {
                        2
                    } else {
                        1
                    }
            })
            .sum()
    }

    fn prefer_chapter(&self, passage: &Passage<'_>, score: usize, required: &[usize]) -> usize {
        match self.documents[passage.document].role {
            DocumentRole::Reference => score / 3,
            DocumentRole::Chapter if required.contains(&passage.document) => score * 4 / 3,
            _ => score,
        }
    }
}

fn search_terms(text: &str) -> HashSet<String> {
    knowledge_search_terms(text)
        .into_iter()
        .filter(|term| (2..=60).contains(&term.chars().count()))
        .collect()
}

fn heading_key(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn worked_example_priority(heading: &str, body: &str) -> u8 {
    let numbers = body
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|number| number.len() >= 2)
        .take(3)
        .count();
    if numbers < 3 {
        return 0;
    }
    if is_case_heading(heading) {
        2
    } else if ["案例", "算例", "为例", "worked example", "for example"]
        .iter()
        .any(|phrase| body.contains(phrase))
    {
        1
    } else {
        0
    }
}

fn document_role(path: &str) -> DocumentRole {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let name = stem
        .trim_start_matches(|ch: char| ch.is_ascii_digit() || matches!(ch, '_' | '-' | ' '))
        .to_lowercase()
        .replace(['-', ' '], "_");
    match name.as_str() {
        "index" | "readme" | "toc" | "table_of_contents" | "version_and_scope" | "目录" => {
            DocumentRole::Guide
        }
        "evidence_ledger" | "source_navigation" | "source_index" | "task_board"
        | "review_report" | "references" | "bibliography" | "license" | "changelog"
        | "证据账本" | "源码导航" | "参考文献" => DocumentRole::Reference,
        _ if name.ends_with("_execution_review") || name.ends_with("_implementation_labs") => {
            DocumentRole::Reference
        }
        _ => DocumentRole::Chapter,
    }
}

fn is_case_heading(title: &str) -> bool {
    let title = title.to_lowercase();
    ["案例", "算例", "实验口径", "working example", "case study"]
        .iter()
        .any(|term| title.contains(term))
}
