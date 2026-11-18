use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;

use chrono::Utc;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use super::topic_pages::{citation_numbers, topic_output_budget};
use super::{estimated_tokens, model_json, BookWikiService};
use crate::error::BrainError;
use crate::infra::book_wiki_store::{
    knowledge_search_terms, topic_review_snapshot, AgentRunCitation,
};
use crate::models::wiki_review::{
    TopicReviewIssue, TopicReviewKind, TopicReviewLocation, TopicReviewPage, TopicReviewReport,
    TopicReviewStatus,
};

const REVIEW_PROMPT: &str = include_str!("../../../prompts/wiki/topic-review.md");
const MAX_REVIEW_INPUT: u64 = 14_000;
const MAX_REVIEW_PARAGRAPHS: usize = 32;
const MAX_REVIEW_FLAGS: usize = 32;

struct Passage<'a> {
    page: &'a TopicReviewPage,
    heading: String,
    line: usize,
    text: String,
}

impl Passage<'_> {
    fn location(&self, quote: String) -> TopicReviewLocation {
        TopicReviewLocation {
            page_id: self.page.id.clone(),
            title: self.page.title.clone(),
            heading: self.heading.clone(),
            line: self.line,
            quote,
        }
    }
}

struct PageMaterial<'a> {
    passages: Vec<Passage<'a>>,
    citations: HashMap<usize, AgentRunCitation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewOutput {
    issues: Vec<SuggestedIssue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestedIssue {
    kind: TopicReviewKind,
    location: SuggestedLocation,
    related: Option<SuggestedLocation>,
    detail: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SuggestedLocation {
    passage: usize,
    quote: String,
}

impl BookWikiService {
    pub fn topic_review_status(&self, base_id: &str) -> Result<TopicReviewStatus, BrainError> {
        self.store.get_base(base_id)?;
        let pages = self.store.topic_review_pages(base_id)?;
        let snapshot = topic_review_snapshot(&pages);
        let material = self.topic_review_material(&pages)?;
        let suspected = material
            .iter()
            .flat_map(lexical_findings)
            .take(MAX_REVIEW_FLAGS)
            .collect();
        let latest = self.store.latest_topic_review(base_id, &snapshot)?;
        Ok(TopicReviewStatus { suspected, latest })
    }

    pub async fn review_wiki_topics(&self, base_id: &str) -> Result<TopicReviewReport, BrainError> {
        let base = self.store.get_active_base(base_id)?;
        let pages = self.store.topic_review_pages(base_id)?;
        let snapshot = topic_review_snapshot(&pages);
        if let Some(report) = self.store.latest_topic_review(base_id, &snapshot)? {
            return Ok(report);
        }
        let material = self.topic_review_material(&pages)?;
        let eligible_passages = material.iter().map(|page| page.passages.len()).sum();
        let profile = self.active_runtime_profile()?;
        let prompt_limit = profile
            .provider_config
            .as_ref()
            .and_then(|provider| provider.context_window)
            .map_or(MAX_REVIEW_INPUT, |window| {
                (u64::from(window) * 2 / 3).min(MAX_REVIEW_INPUT)
            });
        let mut budget = prompt_limit.saturating_sub(estimated_tokens(REVIEW_PROMPT) + 500);
        let mut grouped = material
            .iter()
            .map(|page| {
                let mut passages = page.passages.iter().collect::<Vec<_>>();
                passages.sort_by_key(|passage| {
                    (
                        std::cmp::Reverse(literals(&passage.text).len().min(6)),
                        passage.line,
                    )
                });
                (page, VecDeque::from(passages))
            })
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut input = Vec::new();
        while selected.len() < MAX_REVIEW_PARAGRAPHS {
            let before = selected.len();
            for (page, pending) in &mut grouped {
                let Some(passage) = pending.pop_front() else {
                    continue;
                };
                let evidence = citation_numbers(&passage.text)
                    .into_iter()
                    .filter_map(|number| page.citations.get(&number))
                    .take(3)
                    .map(|citation| {
                        let text = review_excerpt(&passage.text, &citation.content_md);
                        json!({"citation":citation.citation_index,
                            "source":citation.entry.source_path,
                            "excerpt_only":text.len() < citation.content_md.len(),"text":text})
                    })
                    .collect::<Vec<_>>();
                let excerpt = passage.text.chars().take(800).collect::<String>();
                let item = json!({"passage":selected.len()+1,"topic":passage.page.title,
                    "heading":passage.heading,"text":excerpt,"evidence":evidence});
                let cost = estimated_tokens(&item.to_string()) + 4;
                if cost <= budget {
                    budget -= cost;
                    selected.push((passage, excerpt));
                    input.push(item);
                }
                if selected.len() == MAX_REVIEW_PARAGRAPHS {
                    break;
                }
            }
            if before == selected.len() {
                break;
            }
        }
        if selected.is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "当前没有可核对的已发布主题正文".into(),
            ));
        }
        let prompt = format!(
            "{REVIEW_PROMPT}\n\n<topic_review>\n{}\n</topic_review>",
            json!({"book":base.book_name,"passages":input})
        );
        let budget = topic_output_budget(&profile, &prompt, 3_500)?;
        let input = json!({"snapshot":snapshot,"request_timeout_seconds":120,
            "request_max_output_tokens":budget,"passages":selected.len()});
        let (run_id, answer) = self
            .run_audited(base_id, "wiki_topic_review", &input, &profile, prompt, None)
            .await?;
        let report = TopicReviewReport {
            run_id,
            reviewed_at: Utc::now().to_rfc3339(),
            reviewed_pages: selected
                .iter()
                .map(|(passage, _)| &passage.page.id)
                .collect::<HashSet<_>>()
                .len(),
            published_pages: pages.len(),
            reviewed_passages: selected.len(),
            eligible_passages,
            issues: parse_review(&answer, &selected)?,
        };
        self.store.save_topic_review(base_id, &snapshot, &report)?;
        Ok(report)
    }

    fn topic_review_material<'a>(
        &self,
        pages: &'a [TopicReviewPage],
    ) -> Result<Vec<PageMaterial<'a>>, BrainError> {
        pages
            .iter()
            .filter(|page| !page.stale)
            .map(|page| {
                let citations = self
                    .store
                    .list_agent_run_citations(&page.run_id)?
                    .into_iter()
                    .filter(|citation| citation.kind == "source_span")
                    .map(|citation| (citation.citation_index, citation))
                    .collect();
                Ok(PageMaterial {
                    passages: markdown_passages(page),
                    citations,
                })
            })
            .collect()
    }
}

fn markdown_passages(page: &TopicReviewPage) -> Vec<Passage<'_>> {
    let mut passages = Vec::new();
    let mut heading = page.title.clone();
    let mut text = String::new();
    let mut active = None;
    let mut line = 0;
    let mut depth = 0_usize;
    for (event, offset) in
        Parser::new_ext(&page.markdown, Options::ENABLE_TABLES).into_offset_iter()
    {
        match event {
            Event::Start(tag) => {
                if active.is_none()
                    && matches!(
                        tag,
                        Tag::Paragraph | Tag::Item | Tag::TableRow | Tag::Heading { .. }
                    )
                {
                    active = Some(tag.to_end());
                    line = page.markdown[..offset.start]
                        .bytes()
                        .filter(|&byte| byte == b'\n')
                        .count()
                        + 1;
                }
                if active.is_some() {
                    depth += 1;
                }
            }
            Event::End(tag) if active.is_some() => {
                depth -= 1;
                if tag == TagEnd::TableCell {
                    text.push(' ');
                }
                if depth == 0 && active == Some(tag) {
                    active = None;
                    let block = std::mem::take(&mut text);
                    let block = block.trim();
                    if matches!(tag, TagEnd::Heading(_)) {
                        heading = block.to_string();
                    } else if block.chars().count() >= 12 {
                        passages.push(Passage {
                            page,
                            heading: heading.clone(),
                            line,
                            text: block.to_owned(),
                        });
                    }
                }
            }
            Event::Text(content) | Event::Code(content) if active.is_some() => {
                text.push_str(&content)
            }
            Event::SoftBreak | Event::HardBreak if active.is_some() => text.push(' '),
            _ => {}
        }
    }
    passages
}

fn literals(text: &str) -> Vec<&str> {
    static PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(pattern) = PATTERN.get_or_init(|| Regex::new(
        r"(?i)\d{4}-\d{1,2}-\d{1,2}|[+-]?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?(?:\s*(?:%|％|毫秒|微秒|秒|分钟|字节|ms|ns|μs|µs|us|[KMGTP]i?B|rps|tps|tokens?|个位置|个请求))?"
    ).ok()) else { return Vec::new(); };
    pattern
        .find_iter(text)
        .filter_map(|value| {
            let raw = value.as_str();
            // Skip generated source numbers and low-information bare counts.
            let before = &text[..value.start()];
            if before.ends_with("[S") && text[value.end()..].starts_with(']') {
                return None;
            }
            let digits = raw.bytes().filter(u8::is_ascii_digit).count();
            (digits >= 4
                || raw.contains('.')
                || raw.chars().any(|character| {
                    character.is_alphabetic() || character == '%' || character == '％'
                }))
            .then_some(raw)
        })
        .collect()
}

fn normalized_number(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace() && *character != ',')
        .collect::<String>()
        .replace("毫秒", "ms")
        .replace("微秒", "us")
        .replace(['µ', 'μ'], "u")
        .replace('％', "%")
}

fn quotations(text: &str) -> Vec<&str> {
    static PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(pattern) =
        PATTERN.get_or_init(|| Regex::new(r#"“([^”\n]{15,150})”|"([^"\n]{24,150})""#).ok())
    else {
        return Vec::new();
    };
    pattern
        .captures_iter(text)
        .filter_map(|groups| {
            groups
                .get(1)
                .or_else(|| groups.get(2))
                .map(|item| item.as_str())
        })
        .collect()
}

fn lexical_findings(page: &PageMaterial<'_>) -> Vec<TopicReviewIssue> {
    let mut findings = Vec::new();
    let mut seen_values = HashSet::new();
    let mut seen_quotes = HashSet::new();
    for passage in &page.passages {
        let mut cited = citation_numbers(&passage.text)
            .into_iter()
            .filter_map(|number| page.citations.get(&number))
            .collect::<Vec<_>>();
        let source_scope = if cited.is_empty() {
            // Some tables and short passages use a nearby citation.
            cited.extend(page.citations.values());
            "该主题保存的"
        } else {
            "行内引用的"
        };
        let values = cited
            .iter()
            .flat_map(|citation| literals(&citation.content_md))
            .map(normalized_number)
            .collect::<HashSet<_>>();
        for literal in literals(&passage.text) {
            let normalized = normalized_number(literal);
            if !values.contains(&normalized) && seen_values.insert(normalized) {
                findings.push(TopicReviewIssue {
                    kind: TopicReviewKind::Number,
                    location: passage.location(passage.text.chars().take(200).collect()),
                    related: None,
                    detail: format!(
                        "本段的「{literal}」未在{source_scope}原文范围找到。请核对单位、推导或出处。"
                    ),
                });
            }
        }
        for quote in quotations(&passage.text) {
            let needle = quote.split_whitespace().collect::<Vec<_>>().join(" ");
            if !cited.iter().any(|citation| {
                citation
                    .content_md
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains(&needle)
            }) && seen_quotes.insert(needle)
            {
                findings.push(TopicReviewIssue {
                    kind: TopicReviewKind::Quotation,
                    location: passage.location(quote.to_owned()),
                    related: None,
                    detail: format!("该段引语未在{source_scope}原文中逐字匹配，请核对是否为转述。"),
                });
            }
        }
        if findings.len() >= MAX_REVIEW_FLAGS {
            break;
        }
    }
    findings
}

pub(super) fn review_excerpt(claim: &str, evidence: &str) -> String {
    const EXCERPT_CHARACTERS: usize = 680;
    if evidence.chars().count() <= EXCERPT_CHARACTERS {
        return evidence.to_owned();
    }
    let terms = knowledge_search_terms(claim)
        .into_iter()
        .collect::<HashSet<_>>();
    let numbers = literals(claim)
        .into_iter()
        .map(normalized_number)
        .collect::<HashSet<_>>();
    let lines = evidence.lines().collect::<Vec<_>>();
    let strongest = lines
        .iter()
        .enumerate()
        .max_by_key(|(position, line)| {
            (
                literals(line)
                    .into_iter()
                    .map(normalized_number)
                    .filter(|value| numbers.contains(value))
                    .count(),
                knowledge_search_terms(line)
                    .iter()
                    .filter(|term| terms.contains(*term))
                    .count(),
                std::cmp::Reverse(*position),
            )
        })
        .map_or(0, |(index, _)| index);
    lines
        .iter()
        .skip(strongest.saturating_sub(1))
        .take(6)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(EXCERPT_CHARACTERS)
        .collect()
}

fn parse_review(
    raw: &str,
    passages: &[(&Passage<'_>, String)],
) -> Result<Vec<TopicReviewIssue>, BrainError> {
    let (value, _) = model_json::single_json_object(raw)?;
    let result: ReviewOutput = serde_json::from_value(value)
        .map_err(|error| BrainError::KnowledgeValidation(format!("体检结果格式不正确：{error}")))?;
    let invalid =
        || BrainError::KnowledgeValidation("体检结果使用了不对应的正文编号、引文或问题类型".into());
    if result.issues.len() > 12 {
        return Err(invalid());
    }
    let locate = |item: SuggestedLocation| {
        let (passage, excerpt) = item
            .passage
            .checked_sub(1)
            .and_then(|index| passages.get(index))
            .ok_or_else(invalid)?;
        if !(8..=280).contains(&item.quote.chars().count()) || !excerpt.contains(&item.quote) {
            return Err(invalid());
        }
        Ok(passage.location(item.quote))
    };
    result
        .issues
        .into_iter()
        .map(|issue| {
            let location = locate(issue.location)?;
            let related = issue.related.map(locate).transpose()?;
            let valid_pair = match issue.kind {
                TopicReviewKind::Contradiction => related
                    .as_ref()
                    .is_some_and(|other| other.page_id != location.page_id),
                TopicReviewKind::Unsupported | TopicReviewKind::MissingCondition => {
                    related.is_none()
                }
                _ => false,
            };
            if !valid_pair || !(1..=320).contains(&issue.detail.trim().chars().count()) {
                return Err(invalid());
            }
            Ok(TopicReviewIssue {
                kind: issue.kind,
                location,
                related,
                detail: issue.detail,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, body: &str) -> TopicReviewPage {
        TopicReviewPage {
            id: id.into(),
            title: id.into(),
            revision: id.into(),
            run_id: id.into(),
            markdown: body.into(),
            stale: false,
        }
    }

    #[test]
    fn test_markdown_review_keeps_prose_and_location_and_skips_code() {
        let page = page(
            "page",
            "## 延迟\n\n1. 当前是 1.8 ms。[S3]\n\n```\n无关的 90ms\n```\n",
        );
        let blocks = markdown_passages(&page);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].heading, "延迟");
        assert_eq!(blocks[0].line, 3);
        assert_eq!(literals(&blocks[0].text), vec!["1.8 ms"]);
        assert_eq!(
            citation_numbers(&blocks[0].text)
                .into_iter()
                .collect::<Vec<_>>(),
            [3]
        );
        assert_eq!(normalized_number("1.8 毫秒"), normalized_number("1.8ms"));
    }

    #[test]
    fn test_review_rejects_hallucinated_quotes_and_accepts_a_cross_page_conflict() {
        let left = page("left", "相同试验中默认时效为 60 秒。");
        let right = page("right", "同一参数表把默认时效写成 30 秒。");
        let (left, right) = (markdown_passages(&left), markdown_passages(&right));
        let input = [
            (&left[0], left[0].text.clone()),
            (&right[0], right[0].text.clone()),
        ];
        let valid = json!({"issues":[{"kind":"contradiction","location":{"passage":1,"quote":"相同试验中默认时效为 60 秒。"},"related":{"passage":2,"quote":"同一参数表把默认时效写成 30 秒。"},"detail":"同一设置的默认值冲突。"}]});
        assert_eq!(
            parse_review(&valid.to_string(), &input).unwrap()[0]
                .related
                .as_ref()
                .unwrap()
                .page_id,
            "right"
        );
        let invalid = valid.to_string().replace("30 秒", "90 秒");
        assert!(parse_review(&invalid, &input).is_err());
    }

    #[test]
    fn test_review_excerpt_keeps_short_table_and_finds_numeric_rows_in_long_sources() {
        let table = "| 负载 | 策略 | 平均延迟 |\n| --- | --- | --- |\n| 热点 | CLOCK | 1.8 ms |\n| 扫描 | CLOCK | 4.1 ms |";
        assert_eq!(review_excerpt("热点平均延迟", table), table);
        let source = format!("{}\n{table}", "同一热点实验结果。\n".repeat(100));
        assert!(review_excerpt("热点实验记录 CLOCK 为 1.8 ms", &source)
            .contains("| 热点 | CLOCK | 1.8 ms |"));
    }

    #[tokio::test]
    async fn test_published_topic_review_flags_uncited_values_and_expires_with_the_source() {
        use super::super::topic_page_tests::Fixture;
        use crate::models::wiki_topics::{TopicCompileRequest, TopicSourceSelection};

        let fixture = Fixture::new();
        let document = fixture
            .service
            .store()
            .list_current_source_documents(&fixture.base)
            .unwrap()
            .remove(0);
        fixture.runtime.reply(
            "## 实验结果\n\n本轮 CLOCK 的平均延迟为 8.4 ms。[S1]\n\n实验原文记录的平均延迟是 1.8 毫秒。\n\n本轮没有引用的平均延迟是 4.8 ms。",
        );
        let page = fixture
            .service
            .compile_topic(
                &fixture.base,
                TopicCompileRequest {
                    page_id: None,
                    title: "平均延迟".into(),
                    guidance: String::new(),
                    sources: vec![TopicSourceSelection {
                        document_id: document.id,
                        span_id: None,
                        range: None,
                    }],
                },
            )
            .await
            .unwrap();
        fixture
            .service
            .publish_topic(&fixture.base, &page.page.id, &page.draft.unwrap().id)
            .unwrap();

        let status = fixture.service.topic_review_status(&fixture.base).unwrap();
        assert_eq!(status.suspected.len(), 2);
        assert!(status.suspected[0].detail.contains("8.4 ms"));
        assert!(status.suspected[1].detail.contains("4.8 ms"));
        assert!(status.latest.is_none());

        fixture.runtime.reply(r#"{"issues":[{"kind":"unsupported","location":{"passage":1,"quote":"本轮 CLOCK 的平均延迟为 8.4 ms。"},"related":null,"detail":"原文记录的是 1.8 ms。"}]}"#);
        let report = fixture
            .service
            .review_wiki_topics(&fixture.base)
            .await
            .unwrap();
        assert_eq!(report.reviewed_pages, 1);
        assert_eq!(report.issues[0].kind, TopicReviewKind::Unsupported);
        assert_eq!(
            fixture
                .service
                .review_wiki_topics(&fixture.base)
                .await
                .unwrap()
                .run_id,
            report.run_id
        );

        std::fs::write(
            fixture.book.join("avg.md"),
            "# 新实验\n\nCLOCK 平均延迟为 2.0 ms。",
        )
        .unwrap();
        fixture.service.sync(&fixture.base).unwrap();
        let after = fixture.service.topic_review_status(&fixture.base).unwrap();
        assert!(after.latest.is_none());
        assert!(after.suspected.is_empty());
    }
}
