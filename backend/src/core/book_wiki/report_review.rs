use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{estimated_tokens, model_json, topic_pages, BookWikiService};
use crate::error::BrainError;
use crate::infra::book_wiki_store::{
    AgentRunCitation, ReportReviewApproval, ReportReviewEdit, ResearchStageClaim,
};
use crate::models::book_wiki::{KnowledgeTask, ResearchPlan, ResearchStageContent, RuntimeProfile};

const REVIEW_RULES: &str = include_str!("../../../prompts/wiki/report-review.md");
pub(super) const REPORT_REVIEW_TASK: &str = "knowledge_task_report_review";
const REVIEW_INPUT_LIMIT: u64 = 22_000;
const REVIEW_OUTPUT_TOKENS: u32 = 8_192;
const REVIEW_BLOCK_CHARACTERS: usize = 2_400;
const REVIEW_PACKET_BLOCKS: usize = 24;

#[derive(Clone, Copy)]
struct ReviewBlock<'a> {
    id: usize,
    line: usize,
    start: usize,
    text: &'a str,
    table: bool,
    editable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewResponse {
    issues: Vec<ReviewIssue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewIssue {
    block: usize,
    quote: String,
    reason: String,
    citation: Option<usize>,
    source_quote: Option<String>,
    replacement: Option<String>,
}

struct ReportIssue {
    line: usize,
    location: Range<usize>,
    reason: String,
    edit: Option<ReportReviewEdit>,
}

struct ReviewRound {
    runs: Vec<String>,
    blocks: usize,
    excerpts_limited: bool,
    issues: Vec<ReportIssue>,
}

struct ReportReviewContext<'a> {
    task: &'a KnowledgeTask,
    plan: &'a ResearchPlan,
    profile: &'a RuntimeProfile,
    claim: &'a ResearchStageClaim,
    validation_run: &'a str,
    report_run: &'a str,
    citations: &'a [AgentRunCitation],
}

fn invalid(message: impl Into<String>) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

fn review_blocks(report: &str) -> Vec<ReviewBlock<'_>> {
    let mut blocks = Vec::new();
    let mut covered_until = 0;
    let mut cursor = 0;
    let mut line = 1;
    for (event, range) in
        Parser::new_ext(report, Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES)
            .into_offset_iter()
    {
        let Event::Start(tag) = event else { continue };
        let (table, editable) = match tag {
            Tag::TableHead | Tag::TableRow => (true, true),
            Tag::CodeBlock(_) | Tag::HtmlBlock => (false, false),
            Tag::Paragraph | Tag::Heading { .. } | Tag::Item => (false, true),
            _ => continue,
        };
        if range.start < covered_until {
            continue;
        }
        covered_until = range.end;
        let mut start = range.start;
        while start < range.end {
            line += report[cursor..start]
                .bytes()
                .filter(|&byte| byte == b'\n')
                .count();
            cursor = start;
            let remaining = &report[start..range.end];
            let text = match remaining.char_indices().nth(REVIEW_BLOCK_CHARACTERS) {
                None => remaining,
                Some((limit, _)) => {
                    let first = &remaining[..limit];
                    let end = first
                        .rfind(['\n', '。', '；', '！', '？'])
                        .map(|index| {
                            index + first[index..].chars().next().map_or(1, char::len_utf8)
                        })
                        .filter(|end| *end > limit / 2)
                        .unwrap_or(limit);
                    &first[..end]
                }
            };
            blocks.push(ReviewBlock {
                id: blocks.len() + 1,
                line,
                start,
                text,
                table,
                editable,
            });
            start += text.len();
        }
    }
    blocks
}

fn review_input_limit(profile: &RuntimeProfile) -> u64 {
    let cap = super::effective_output_cap(profile, Some(REVIEW_OUTPUT_TOKENS))
        .unwrap_or(REVIEW_OUTPUT_TOKENS);
    let context = profile
        .provider_config
        .as_ref()
        .and_then(|provider| provider.context_window)
        .unwrap_or(32_768);
    let reserve = cap.min(context / 3);
    u64::from(context.saturating_sub(reserve).saturating_sub(2_048)).min(REVIEW_INPUT_LIMIT)
}

fn block_card(block: &ReviewBlock<'_>) -> Value {
    json!({"block":block.id,"line":block.line,"text":block.text})
}

fn packet_ranges(blocks: &[ReviewBlock<'_>], budget: u64) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (index, block) in blocks.iter().enumerate() {
        let cost = estimated_tokens(&block_card(block).to_string()) + 2;
        if index > start && (size + cost > budget || index - start == REVIEW_PACKET_BLOCKS) {
            ranges.push(start..index);
            start = index;
            size = 0;
        }
        size += cost;
    }
    if start < blocks.len() {
        ranges.push(start..blocks.len());
    }
    ranges
}

fn source_card(citation: &AgentRunCitation, excerpt: &str) -> Value {
    json!({
        "citation":citation.citation_index,
        "source":citation.entry.source_path.as_deref().unwrap_or(&citation.entry.title),
        "excerpt_only":excerpt.len() < citation.content_md.len(),
        "text":excerpt,
    })
}

fn packet_prompt(
    context: &ReportReviewContext<'_>,
    blocks: &[ReviewBlock<'_>],
    limit: u64,
    confirmation: bool,
    reconsideration: bool,
) -> Result<(String, BTreeMap<usize, String>, bool), BrainError> {
    let cards = blocks.iter().map(block_card).collect::<Vec<_>>();
    let task = context.task;
    let requested_scope: String = task.description.chars().take(1_000).collect();
    let requirements = |items: &[String]| {
        items
            .iter()
            .take(12)
            .map(|item| item.chars().take(400).collect::<String>())
            .collect::<Vec<_>>()
    };
    let mut input = json!({"goal":task.title,"requested_scope":requested_scope,
        "constraints":requirements(&context.plan.constraints),
        "acceptance":requirements(&context.plan.acceptance),
        "purpose":task.brief.purpose,"blocks":cards,"sources":[]});
    let mut prefix = format!("{REVIEW_RULES}\n\n");
    if confirmation {
        prefix.push_str("本轮复核已经改过的文字。只评价每段 revised 中的修改是否有原文依据，是否与该段剩余文字相接，是否留下原来要解决的事实错误。text 的其他内容提供上下文。\n\n");
    } else if reconsideration {
        prefix.push_str("上一轮留下了暂时未能修改的怀疑，见 review_reconsideration。逐项核对被指出的原句和实际原文。若原文确实支持现稿，撤回误判。公式按输入逐步复算，再判断是否与原文矛盾。若查到确切错误，给出能直接放回原处的修订。仍无法确认的实质缺口保留为意见。\n\n");
    }
    prefix.push_str("<report_review>\n");
    let mut remaining = limit
        .checked_sub(estimated_tokens(&input.to_string()) + estimated_tokens(&prefix) + 256)
        .ok_or_else(|| invalid("报告审稿输入超过当前模型容量，已保存报告草稿"))?;
    let referenced = blocks
        .iter()
        .flat_map(|block| topic_pages::citation_numbers(block.text))
        .collect::<BTreeSet<_>>();
    let mut sources = BTreeMap::new();
    let mut sent_cards = Vec::new();
    let mut excerpts_limited = false;
    let mut ordered: Vec<_> = context.citations.iter().collect();
    ordered.sort_by_key(|citation| !referenced.contains(&citation.citation_index));
    let mut mandatory_left = referenced.len();
    for citation in ordered {
        let mandatory = referenced.contains(&citation.citation_index);
        let full = source_card(citation, &citation.content_md);
        let max_cost = if mandatory {
            let allowed = remaining / mandatory_left.max(1) as u64;
            mandatory_left = mandatory_left.saturating_sub(1);
            allowed
        } else {
            remaining
        };
        let full_cost = estimated_tokens(&full.to_string()) + 2;
        if full_cost <= max_cost {
            sent_cards.push(full);
            sources.insert(citation.citation_index, citation.content_md.clone());
            remaining -= full_cost;
        } else if mandatory {
            excerpts_limited = true;
            let focus = blocks
                .iter()
                .filter(|block| {
                    topic_pages::citation_numbers(block.text).contains(&citation.citation_index)
                })
                .map(|block| block.text)
                .collect::<Vec<_>>()
                .join("\n");
            let excerpt = super::topic_review::review_excerpt(&focus, &citation.content_md);
            let card = source_card(citation, &excerpt);
            let cost = estimated_tokens(&card.to_string()) + 2;
            if cost <= max_cost && !excerpt.trim().is_empty() {
                sent_cards.push(card);
                sources.insert(citation.citation_index, excerpt);
                remaining -= cost;
            }
        }
    }
    input["sources"] = json!(sent_cards);
    Ok((
        format!("{prefix}{input}\n</report_review>"),
        sources,
        excerpts_limited,
    ))
}

fn without_repeated_context<'a>(replacement: &'a str, before: &str, after: &str) -> &'a str {
    let prefix = replacement
        .char_indices()
        .rev()
        .map(|(index, _)| index)
        .find(|&index| {
            index >= 12
                && replacement[..index].chars().count() >= 8
                && before.ends_with(&replacement[..index])
        });
    let replacement = &replacement[prefix.unwrap_or(0)..];
    replacement
        .strip_suffix(['。', '！', '？', '；'])
        .filter(|trimmed| after.starts_with(&replacement[trimmed.len()..]))
        .unwrap_or(replacement)
}

fn matches_source_quote(mut source: &str, quote: &str) -> bool {
    quote.trim().split("\n\n").all(|part| {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        let Some(offset) = source.find(part) else {
            return false;
        };
        source = &source[offset + part.len()..];
        true
    })
}

fn parse_review(
    answer: &str,
    blocks: &[ReviewBlock<'_>],
    sources: &BTreeMap<usize, String>,
) -> Result<Vec<ReportIssue>, BrainError> {
    let (value, _) = model_json::single_json_object(answer)?;
    let response: ReviewResponse = serde_json::from_value(value)
        .map_err(|error| invalid(format!("报告审稿返回了无效的格式：{error}")))?;
    response
        .issues
        .into_iter()
        .map(|issue| {
            let invalid_locator = || {
                invalid(format!(
                    "审稿第 {} 项的位置和原文引语必须能逐字定位。请核对原句：{}",
                    issue.block,
                    issue.quote.chars().take(120).collect::<String>()
                ))
            };
            let block = blocks
                .iter()
                .find(|block| block.id == issue.block)
                .ok_or_else(invalid_locator)?;
            if issue.quote.trim().is_empty()
                || issue.quote.chars().count() > 360
                || issue.reason.trim().is_empty()
                || issue.reason.chars().count() > 600
            {
                return Err(invalid_locator());
            }
            let mut matches = block.text.match_indices(&issue.quote);
            let (offset, _) = matches.next().ok_or_else(invalid_locator)?;
            if matches.next().is_some() {
                return Err(invalid_locator());
            }
            let grounded = match (issue.citation, issue.source_quote.as_deref()) {
                (Some(index), Some(quote)) if !quote.trim().is_empty() => sources
                    .get(&index)
                    .filter(|source| matches_source_quote(source, quote))
                    .map(|_| index),
                (None, None) => None,
                _ => return Err(invalid_locator()),
            };
            if issue.citation.is_some() && grounded.is_none() {
                return Err(invalid_locator());
            }
            let start = block.start + offset;
            let location = start..start + issue.quote.len();
            let edit = issue
                .replacement
                .map(|mut replacement| {
                    let invalid_edit = || invalid(format!(
                        "审稿第 {} 段的替换需有已提供的原文依据，并保持单行和表格结构。请核对替换范围：{}",
                        issue.block, issue.quote
                    ));
                    let source_index = grounded.ok_or_else(invalid_edit)?;
                    let old_references = topic_pages::citation_numbers(&issue.quote);
                    let marker = format!("[S{source_index}]");
                    let remaining = &block.text[offset + issue.quote.len()..];
                    replacement = without_repeated_context(&replacement, &block.text[..offset], remaining).to_owned();
                    if remaining.trim_start().starts_with(&marker)
                        && !old_references.contains(&source_index)
                    {
                        if let Some(text) = replacement.trim_end().strip_suffix(&marker) {
                            replacement = text.trim_end().to_owned();
                        }
                    }
                    if issue.quote.trim_end().ends_with(&marker)
                        && old_references == BTreeSet::from([source_index])
                        && !replacement.trim().is_empty()
                        && topic_pages::citation_numbers(&replacement).is_empty()
                    {
                        replacement = format!("{} {marker}", replacement.trim_end());
                    }
                    let new_references = topic_pages::citation_numbers(&replacement);
                    let changes_references = new_references != old_references;
                    if !block.editable
                        || replacement == issue.quote
                        || replacement.trim().is_empty()
                        || replacement.chars().count() > 500
                        || replacement.contains(['\n', '\r'])
                        || issue.quote.contains(['\n', '\r'])
                        || (block.table && (replacement.contains('|') || issue.quote.contains('|')))
                        || (changes_references && (!new_references.contains(&source_index)
                            || new_references.difference(&old_references).any(|number|*number != source_index)))
                    {
                        return Err(invalid_edit());
                    }
                    Ok(ReportReviewEdit {
                        start: location.start,
                        end: location.end,
                        expected: issue.quote,
                        replacement,
                        source_index,
                    })
                })
                .transpose()?;
            Ok(ReportIssue {
                line: block.line,
                location,
                reason: issue.reason,
                edit,
            })
        })
        .collect()
}

fn revised_ranges(edits: &[ReportReviewEdit]) -> Vec<Range<usize>> {
    let mut prior_end = 0;
    let mut revised_end = 0;
    edits
        .iter()
        .map(|edit| {
            let start = revised_end + (edit.start - prior_end);
            revised_end = start + edit.replacement.len();
            prior_end = edit.end;
            start..revised_end
        })
        .collect()
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

impl BookWikiService {
    pub(super) async fn audit_research_report(
        &self,
        task: &KnowledgeTask,
        plan: &ResearchPlan,
        profile: &RuntimeProfile,
        claim: &ResearchStageClaim,
        validation_run: &str,
        report: &ResearchStageContent,
    ) -> Result<Option<ReportReviewApproval>, BrainError> {
        let report_run = report
            .content_run_id
            .as_deref()
            .ok_or_else(|| invalid("报告缺少可审稿的版本"))?;
        let origin = self.store.get_agent_run(report_run)?;
        if !origin
            .output
            .as_ref()
            .is_some_and(|output| output["authored_report"] == true)
        {
            return Ok(None);
        }
        let citations = self.store.list_agent_run_citations(report_run)?;
        let context = ReportReviewContext {
            task,
            plan,
            profile,
            claim,
            validation_run,
            report_run,
            citations: &citations,
        };
        let mut round = self
            .review_report_round(&context, &report.content_md, &[], &[])
            .await?;
        let (mut repairable, blockers): (Vec<_>, Vec<_>) = round
            .issues
            .into_iter()
            .partition(|issue| issue.edit.is_some());
        if !blockers.is_empty() {
            let checked = self
                .review_report_round(&context, &report.content_md, &[], &blockers)
                .await?;
            repairable.extend(checked.issues);
            round.runs.extend(checked.runs);
            round.excerpts_limited |= checked.excerpts_limited;
        }
        round.issues = repairable;
        let mut edits = Vec::new();
        for issue in &round.issues {
            let Some(edit) = &issue.edit else {
                return Err(review_blocked(&round.issues));
            };
            edits.push(edit.clone());
        }
        edits.sort_by_key(|edit| edit.start);
        edits.dedup();
        if !edits.is_empty() {
            let candidate = crate::infra::book_wiki_store::apply_report_review_edits(
                &report.content_md,
                &edits,
            )?;
            let changed = revised_ranges(&edits);
            let checked = self
                .review_report_round(&context, &candidate, &changed, &[])
                .await?;
            if !checked.issues.is_empty() {
                return Err(review_blocked(&checked.issues));
            }
            round.runs.extend(checked.runs);
            round.excerpts_limited |= checked.excerpts_limited;
        }
        Ok(Some(ReportReviewApproval {
            report_run_id: report_run.to_owned(),
            runs: round.runs,
            reviewed_blocks: round.blocks,
            excerpts_limited: round.excerpts_limited,
            edits,
        }))
    }

    async fn review_report_round(
        &self,
        context: &ReportReviewContext<'_>,
        markdown: &str,
        changed: &[Range<usize>],
        blockers: &[ReportIssue],
    ) -> Result<ReviewRound, BrainError> {
        let confirmation = !changed.is_empty();
        let reconsideration = !blockers.is_empty();
        let round = if confirmation {
            2
        } else if reconsideration {
            3
        } else {
            1
        };
        let in_scope = |location: &Range<usize>| {
            if confirmation {
                changed.iter().any(|range| overlaps(range, location))
            } else if reconsideration {
                blockers
                    .iter()
                    .any(|issue| overlaps(&issue.location, location))
            } else {
                true
            }
        };
        let blocks = review_blocks(markdown)
            .into_iter()
            .filter(|block| in_scope(&(block.start..block.start + block.text.len())))
            .collect::<Vec<_>>();
        if blocks.is_empty() {
            return Err(invalid("报告缺少可审查的 Markdown 正文"));
        }
        let limit = review_input_limit(context.profile);
        let body_limit = limit.saturating_sub(estimated_tokens(REVIEW_RULES) + 384) * 2 / 3;
        let (tables, prose): (Vec<_>, Vec<_>) = blocks
            .iter()
            .copied()
            .partition(|block| blocks.len() > REVIEW_PACKET_BLOCKS && block.table);
        let packets = [&tables[..], &prose[..]]
            .into_iter()
            .flat_map(|blocks| {
                packet_ranges(blocks, body_limit)
                    .into_iter()
                    .map(move |range| &blocks[range])
            })
            .collect::<Vec<_>>();
        let packet_count = packets.len();
        let mut result = ReviewRound {
            runs: Vec::new(),
            blocks: blocks.len(),
            excerpts_limited: false,
            issues: Vec::new(),
        };
        for (index, slice) in packets.into_iter().enumerate() {
            self.store.ensure_report_review_is_current(
                context.claim,
                context.validation_run,
                context.report_run,
            )?;
            let (prompt, originals, limited) =
                packet_prompt(context, slice, limit, confirmation, reconsideration)?;
            let prompt = if confirmation {
                let amendments = slice
                    .iter()
                    .map(|block| {
                        let region = block.start..block.start + block.text.len();
                        let text = changed
                            .iter()
                            .filter(|range| overlaps(range, &region))
                            .map(|range| {
                                &markdown[range.start.max(region.start)..range.end.min(region.end)]
                            })
                            .collect::<Vec<_>>();
                        json!({"block":block.id,"revised":text})
                    })
                    .collect::<Vec<_>>();
                format!(
                    "{prompt}\n<review_changes>\n{}\n</review_changes>",
                    json!(amendments)
                )
            } else if reconsideration {
                let issues = blockers.iter().filter(|issue|slice.iter().any(|block|
                    overlaps(&issue.location, &(block.start..block.start+block.text.len()))))
                    .map(|issue|json!({"line":issue.line,"quote":&markdown[issue.location.clone()],"reason":issue.reason}))
                    .collect::<Vec<_>>();
                format!(
                    "{prompt}\n<review_reconsideration>\n{}\n</review_reconsideration>",
                    json!(issues)
                )
            } else {
                prompt
            };
            let mut output = None;
            let mut retry = None;
            for attempt in 0..=1 {
                let prompt = match &retry {
                    Some(prior) => format!(
                        "{prompt}\n上一轮包含未通过定位或引用校验的意见。逐项对照下面的初稿与报告，保留有材料依据的实质问题，纠正引用与替换范围，撤回误判。原句与替换必须覆盖同一个范围。\n<rejected_review>\n{prior}\n</rejected_review>"
                    ),
                    None => prompt.clone(),
                };
                let budget = topic_pages::topic_output_budget(
                    context.profile,
                    &prompt,
                    REVIEW_OUTPUT_TOKENS,
                )?;
                let input = json!({
                    "knowledge_task_id": context.task.id,
                    "research_claim_attempt": context.claim.attempt,
                    "validation_run_id": context.validation_run,
                    "reviewed_report_run_id": context.report_run,
                    "report_review_round": round,
                    "review_packet": index + 1,
                    "review_packets": packet_count,
                    "review_packet_first_line": slice[0].line,
                    "request_max_output_tokens": budget,
                    "request_timeout_seconds": 180,
                    "bounded_extraction": context.profile.provider_config.as_ref()
                        .is_some_and(|provider|provider.reasoning_policy=="auto"),
                });
                self.store.append_agent_run_event(
                    context.validation_run,
                    "run.phase_changed",
                    Some("validating"),
                    &format!("正在独立核对报告第 {}/{} 组段落", index + 1, packet_count),
                    &input,
                )?;
                let (run, answer) = self
                    .run_audited(
                        &context.task.knowledge_base_id,
                        REPORT_REVIEW_TASK,
                        &input,
                        context.profile,
                        prompt,
                        None,
                    )
                    .await?;
                match parse_review(&answer, slice, &originals) {
                    Ok(mut issues) => {
                        issues.retain(|issue| in_scope(&issue.location));
                        output = Some((run, issues));
                        break;
                    }
                    Err(error) if attempt == 0 => {
                        retry = Some(json!({"error":error.to_string(),
                            "draft":answer.chars().take(6_000).collect::<String>()}));
                        self.store.append_agent_run_event(
                            &run,
                            "run.validation_rejected",
                            Some("validating"),
                            "审稿摘录未能精确定位，重新核对一次",
                            &json!({"reason":error.to_string()}),
                        )?;
                    }
                    Err(error) => return Err(error),
                }
            }
            let (run, issues) = output.ok_or_else(|| invalid("未收到有效的独立审稿意见"))?;
            result.runs.push(run);
            result.issues.extend(issues);
            result.excerpts_limited |= limited;
        }
        Ok(result)
    }
}

fn review_blocked(issues: &[ReportIssue]) -> BrainError {
    let detail = issues
        .iter()
        .take(3)
        .map(|issue| format!("第 {} 行：{}", issue.line, issue.reason))
        .collect::<Vec<_>>()
        .join("；");
    invalid(format!(
        "独立审稿发现 {} 项待复核问题，已保留当前稿和取证。{detail}",
        issues.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAD: &str = "过期读取与策略选择无关";
    const FIX: &str = "过期校验由读路径负责，改变淘汰策略是否影响过期读取率仍待实测";
    const SOURCE: &str = "切换淘汰策略不能替代 TTL 修复";

    #[test]
    fn test_report_audit_keeps_neighboring_clauses_and_punctuation_once() {
        let revised = without_repeated_context(
            "最直接的缺口是 P99 实测，因为书内只报告平均延迟。",
            "其中最直接的缺口是 P99 实测，因为",
            "。其余条件也需要验证。",
        );
        assert_eq!(revised, "书内只报告平均延迟");
        assert_eq!(
            without_repeated_context("P99<2 ms", "门槛是 ", "。"),
            "P99<2 ms"
        );
    }

    #[test]
    fn test_report_audit_locates_multiple_exact_excerpts_in_their_source_order() {
        let source = "条件和版本范围。\n\n实验结果及其限制。\n\n可验证建议。";
        assert!(matches_source_quote(
            source,
            "条件和版本范围。\n\n可验证建议。"
        ));
        assert!(!matches_source_quote(
            source,
            "可验证建议。\n\n条件和版本范围。"
        ));
        assert!(!matches_source_quote(
            source,
            "条件和版本范围。\n\n不可定位的结论。"
        ));
    }

    #[test]
    fn test_report_audit_repairs_a_table_cell_and_preserves_the_correct_paragraph() {
        let report = format!(
            "# 验收报告\n\n## 上线判断\n\n| 条件 | 结论 |\n| --- | --- |\n| 线上服务 | {BAD} [S1] |\n\n过期校验由读路径负责，两组事件率需要实测。[S1]\n"
        );
        let blocks = review_blocks(&report);
        let affected = blocks
            .iter()
            .find(|block| block.text.contains(BAD))
            .unwrap();
        let answer = json!({"issues":[{
            "block":affected.id,"quote":BAD,"reason":"职责分开不足以判断事件率。",
            "citation":1,"source_quote":SOURCE,"replacement":FIX
        }]});
        let sources = BTreeMap::from([(1, SOURCE.to_owned())]);
        let issues = parse_review(&answer.to_string(), &blocks, &sources).unwrap();
        let edit = issues[0].edit.as_ref().unwrap();
        assert_eq!(&report[edit.start..edit.end], BAD);
        assert_eq!(edit.replacement, FIX);
        assert_eq!(affected.line, 7);
    }

    #[test]
    fn test_report_audit_rejects_fabricated_locators_and_unverified_citation_changes() {
        let report = format!("工作判断是 {BAD} [S1]。");
        let blocks = review_blocks(&report);
        let sources = BTreeMap::from([(1, SOURCE.to_owned())]);
        let mut issue = json!({"issues":[{
            "block":1,"quote":BAD,"reason":"职责分开不足以判断事件率。",
            "citation":1,"source_quote":"原文没有这样写","replacement":FIX
        }]});
        assert!(parse_review(&issue.to_string(), &blocks, &sources).is_err());
        issue["issues"][0]["source_quote"] = json!(SOURCE);
        issue["issues"][0]["replacement"] = json!(format!("{FIX} [S2]"));
        assert!(parse_review(&issue.to_string(), &blocks, &sources).is_err());
        issue["issues"][0]["replacement"] = json!(format!("{FIX} [S1]"));
        let issues = parse_review(&issue.to_string(), &blocks, &sources).unwrap();
        assert_eq!(issues[0].edit.as_ref().unwrap().replacement, FIX);
        let mismatched = review_blocks("过期读取与策略选择无关 [S2]");
        issue["issues"][0]["quote"] = json!(format!("{BAD} [S2]"));
        let corrected = parse_review(&issue.to_string(), &mismatched, &sources).unwrap();
        assert_eq!(
            corrected[0].edit.as_ref().unwrap().replacement,
            format!("{FIX} [S1]")
        );
        issue["issues"][0]["replacement"] = json!(FIX);
        issue["issues"][0]["quote"] = json!("报告中不存在这个句子");
        assert!(parse_review(&issue.to_string(), &blocks, &sources).is_err());
    }

    #[test]
    fn test_report_audit_allows_threshold_expressions_and_rejects_new_html() {
        let source = "门槛是 P99<2 ms，采样窗口未规定。";
        let report = "| 项目 | 要求 |\n| --- | --- |\n| 门槛 | 尚未规定 [S1] |";
        let blocks = review_blocks(report);
        let sources = BTreeMap::from([(1, source.to_owned())]);
        let mut response = json!({"issues":[{"block":blocks.last().unwrap().id,"quote":"尚未规定",
            "reason":"原文列有门槛","citation":1,"source_quote":source,"replacement":"门槛 P99<2 ms"}]});
        let issues = parse_review(&response.to_string(), &blocks, &sources).unwrap();
        let revised = crate::infra::book_wiki_store::apply_report_review_edits(
            report,
            &[issues[0].edit.clone().unwrap()],
        )
        .unwrap();
        assert!(revised.contains("P99<2 ms [S1]"));
        response["issues"][0]["replacement"] = json!("<img src=x>");
        let issues = parse_review(&response.to_string(), &blocks, &sources).unwrap();
        assert!(crate::infra::book_wiki_store::apply_report_review_edits(
            report,
            &[issues[0].edit.clone().unwrap()],
        )
        .is_err());
    }

    #[tokio::test]
    async fn test_report_audit_blocks_delivery_and_resumes_from_the_saved_manuscript() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        use async_trait::async_trait;

        use crate::infra::book_wiki_store::BookWikiStore;
        use crate::infra::deepseek_harness::{AgentPromptRequest, AgentRuntime};
        use crate::infra::sqlite_store::SqliteStore;
        use crate::models::book_wiki::{BookKind, ReaderBook};

        struct ReviewerRuntime {
            audit_rounds: AtomicUsize,
            writing_calls: AtomicUsize,
        }

        #[async_trait]
        impl AgentRuntime for ReviewerRuntime {
            async fn prompt(&self, request: AgentPromptRequest) -> Result<String, BrainError> {
                if let Some((_, payload)) = request.prompt.split_once("<report_review>\n") {
                    let (payload, _) = payload.split_once("\n</report_review>").unwrap();
                    let material: Value = serde_json::from_str(payload).unwrap();
                    let round = self.audit_rounds.fetch_add(1, Ordering::SeqCst);
                    if round == 3 {
                        assert!(payload.contains(FIX));
                        assert!(!payload.contains(BAD));
                        return Ok(json!({"issues":[]}).to_string());
                    }
                    let block = material["blocks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|block| block["text"].as_str().unwrap().contains(BAD))
                        .unwrap();
                    assert!(material["sources"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains(SOURCE));
                    return Ok(json!({"issues":[{
                        "block":block["block"],"quote":BAD,"reason":"能力归属不能断言事件率。",
                        "citation":1,"source_quote":SOURCE,"replacement":if round<2 {None} else {Some(FIX)}
                    }]}).to_string());
                }
                self.writing_calls.fetch_add(1, Ordering::SeqCst);
                if request
                    .prompt
                    .contains("<research_phase>plan</research_phase>")
                {
                    return Ok(json!({"goal":"核对过期数据","report_title":"过期读取验收",
                        "constraints":["按原文解释"],"acceptance":["区分职责和观测结果"],"depth":"brief","terminology":[],
                        "questions":[{"id":"condition","title":"过期校验的条件","question":"过期如何核对","required_evidence":["读路径职责"],"expected_output_tokens":400}]
                    }).to_string());
                }
                if request
                    .prompt
                    .contains("<research_phase>section</research_phase>")
                {
                    assert!(request.prompt.contains(SOURCE));
                    return Ok(json!({"summary":"过期校验的职责",
                        "content_md":format!("{SOURCE}。[S1] 两组指标仍缺实测。"),
                        "findings":[{"finding":"缺发生率数据","status":"partial","citation_indices":[1],"limitations":["需测试"]}]
                    }).to_string());
                }
                assert!(request
                    .prompt
                    .contains("<research_phase>synthesis</research_phase>"));
                let scope = request
                    .prompt
                    .split_once("<phase_scope>\n")
                    .unwrap()
                    .1
                    .split_once("\n</phase_scope>")
                    .unwrap()
                    .0;
                let scope: Value = serde_json::from_str(scope).unwrap();
                let section = &scope["integration_manifest"]["sections"][0];
                Ok(json!({"summary":"需实测","chapters":[{
                    "title":"过期读取与测量","summary":"保留对照实验",
                    "content_md":format!("| 条件 | 判断 |\n| --- | --- |\n| 灰度 | {BAD} [S1] |\n\n两组指标仍缺实测。[S1]"),
                    "findings":[{"finding":"缺发生率数据","status":"partial","citation_indices":[1],"limitations":["需测试"]}]
                }],"section_checks":[{"question_id":section["question_id"],"revision":section["revision"],
                    "assessment":"insufficient","note":"保持实验范围"}]}).to_string())
            }
        }

        let runtime = Arc::new(ReviewerRuntime {
            audit_rounds: AtomicUsize::new(0),
            writing_calls: AtomicUsize::new(0),
        });
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("book");
        std::fs::create_dir(&book).unwrap();
        std::fs::write(
            book.join("过期.md"),
            format!("# 过期校验的条件\n\n{SOURCE}。"),
        )
        .unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("audit.db")).unwrap(),
        ));
        store
            .save_reader_books(&[ReaderBook {
                id: "review-book".into(),
                path: book.to_string_lossy().into_owned(),
                kind: BookKind::Folder,
                name: "校验来源".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let service = BookWikiService::new(store.clone(), runtime.clone());
        let base = service
            .initialize_and_sync("review-book")
            .unwrap()
            .knowledge_base;
        let task = store
            .create_task(
                &base.id,
                "过期校验的条件",
                "需要核对 TTL 与淘汰职责",
                "research",
            )
            .unwrap();
        let initial_error = service.execute_task(&task.id).await.unwrap_err();
        assert!(
            initial_error.to_string().contains("待复核"),
            "{initial_error}"
        );
        let draft = service.get_task_result(&task.id).unwrap();
        assert!(!draft.report_verified);
        assert!(draft.task.result_summary.contains(BAD));
        let old_revision = store
            .get_research_stage_content(&task.id, "report", None)
            .unwrap()
            .stage
            .revision;

        let result = service.execute_task(&task.id).await.unwrap();
        assert!(result.report_verified);
        assert_eq!(
            result.task.result_summary,
            draft.task.result_summary.replace(BAD, FIX)
        );
        assert_ne!(result.run_id, draft.run_id);
        assert_eq!(result.evidence, draft.evidence);
        assert_eq!(runtime.writing_calls.load(Ordering::SeqCst), 3);
        assert_eq!(runtime.audit_rounds.load(Ordering::SeqCst), 4);
        assert!(store
            .get_research_stage_content(&task.id, "report", Some(old_revision))
            .unwrap()
            .content_md
            .contains(BAD));
        let outline = store.research_report_outline(&result.run_id).unwrap();
        let end = outline[0]["byte_end"].as_u64().unwrap() as usize;
        assert_eq!(end, result.task.result_summary.len());
        let validation = store
            .get_research_stage_content(&task.id, "validation", None)
            .unwrap();
        assert!(validation.stage.summary.contains("修订 1 处"));
    }
}
