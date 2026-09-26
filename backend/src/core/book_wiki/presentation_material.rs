//! Whole-report projection for a bounded slide planner, never prefix clipping.

use super::qa_policy::estimated_tokens;
use super::source_structure::structured_fragments;
use crate::error::BrainError;
use crate::models::book_wiki::ResearchFinding;
use serde_json::{json, Value};

fn invalid(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

fn capacity(message: &str) -> BrainError {
    BrainError::KnowledgeValidation(format!("(research_input_hard_limit) {message}"))
}
fn position(value: &Value, key: &str) -> Result<usize, BrainError> {
    value[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| invalid("完整报告缺少合法的章节位置，不能退回只取开头"))
}

pub(super) fn project_report(
    report: &str,
    ranges: &Value,
    findings: &[ResearchFinding],
    limit: u64,
) -> Result<String, BrainError> {
    let ranges = ranges
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 25)
        .ok_or_else(|| invalid("完整报告没有已验证的主题结构，无法安全选材"))?;
    let mut previous_end = 0;
    let mut previous_finding = 0;
    let mut fragments = vec![];
    let mut sections = vec![];
    let fragment_target = (limit / ranges.len() as u64 / 8).clamp(80, 600) as usize;
    for range in ranges {
        let start = position(range, "byte_start")?;
        let end = position(range, "byte_end")?;
        let first = position(range, "finding_start")?;
        let last = position(range, "finding_end")?;
        let body = report
            .get(start..end)
            .filter(|_| (sections.is_empty() || start == previous_end) && end > start)
            .ok_or_else(|| invalid("报告主题边界损坏，不能截取不完整内容"))?;
        if !body.starts_with("## ") {
            return Err(invalid("报告主题没有完整标题边界，不能从半段正文截取选材"));
        }
        let matrix = findings
            .get(first..last)
            .filter(|_| first == previous_finding && last > first)
            .ok_or_else(|| invalid("报告主题的完整发现矩阵缺失，不能压成无依据要点"))?;
        let parts = structured_fragments(body, fragment_target);
        let tail = parts
            .last()
            .ok_or_else(|| invalid("研究主题没有可选取的完整正文"))?;
        sections.push(json!({"question_id":range["question_id"],"title":range["title"],"summary":range["summary"],"findings":matrix,"selected_fragments":[{"char_start":tail.char_start,"char_end":tail.char_end,"content_md":tail.content}]}));
        fragments.push(parts);
        previous_end = end;
        previous_finding = last;
    }
    if previous_end != report.len() || previous_finding != findings.len() {
        return Err(invalid(
            "报告尾部或发现矩阵不属于任何主题，不能遗漏后段选材",
        ));
    }
    let mut material = json!({"mode":"whole_structure_projection","projection_notice":"这是演示选材，不是完整报告：每个主题保留完整发现与限制、尾段及可容纳的结构片段。未选正文不可补造为事实；完整报告独立保存。全局S编号不变。","source_report_characters":report.chars().count(),"sections":sections,"omitted_characters":0});
    if estimated_tokens(&material.to_string()) > limit {
        return Err(capacity("全部主题的发现、限制和完整尾段超过演示输入预算；保留完整报告，不截尾或遗漏主题伪装交付"));
    }
    let mut candidates = fragments
        .iter()
        .map(|parts| {
            let mut order = (0..parts.len().saturating_sub(1)).collect::<Vec<_>>();
            // Protected math/tables/code remain intact; then opening and body.
            order.sort_by_key(|index| {
                usize::from(
                    !parts[*index].oversize_atomic
                        && !parts[*index].content.contains("$$")
                        && !parts[*index].content.contains("\\[")
                        && !parts[*index].content.contains("```")
                        && !parts[*index].content.contains('|'),
                )
            });
            order.into_iter()
        })
        .collect::<Vec<_>>();
    loop {
        let mut added = false;
        for (section, order) in candidates.iter_mut().enumerate() {
            let Some(index) = order.next() else { continue };
            let fragment = &fragments[section][index];
            let selected = material["sections"][section]["selected_fragments"]
                .as_array_mut()
                .ok_or_else(|| invalid("演示选材结构损坏"))?;
            selected.push(json!({"char_start":fragment.char_start,"char_end":fragment.char_end,"content_md":fragment.content}));
            if estimated_tokens(&material.to_string()) <= limit.saturating_sub(128) {
                added = true;
            } else {
                material["sections"][section]["selected_fragments"]
                    .as_array_mut()
                    .ok_or_else(|| invalid("演示选材结构损坏"))?
                    .pop();
            }
        }
        if !added {
            break;
        }
    }
    let mut selected_characters = 0;
    for section in material["sections"]
        .as_array_mut()
        .ok_or_else(|| invalid("演示主题结构损坏"))?
    {
        let parts = section["selected_fragments"]
            .as_array_mut()
            .ok_or_else(|| invalid("演示片段结构损坏"))?;
        parts.sort_by_key(|part| part["char_start"].as_u64().unwrap_or(0));
        selected_characters += parts
            .iter()
            .map(|part| part["content_md"].as_str().map_or(0, |s| s.chars().count()))
            .sum::<usize>();
    }
    material["omitted_characters"] =
        json!(report.chars().count().saturating_sub(selected_characters));
    let result = material.to_string();
    if estimated_tokens(&result) > limit {
        return Err(capacity("完整选材清单仍超过模型容量，报告保留且未截尾"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection_covers_every_theme_late_findings_and_intact_math_with_bounded_input() {
        let mut report = String::from("# 总目标\n\n");
        let mut ranges = vec![];
        let mut findings = vec![];
        for index in 0..10 {
            let start = report.len();
            report.push_str(&format!("## 主题{index}\n\n{}\n\n$$d_k=d_v=128$$\n\nLAST_CRITICAL_{index} 边界与限制。[S1]\n", "开头的详细解释。\n\n".repeat(2000)));
            ranges.push(json!({"question_id":format!("q{index}"),"title":format!("主题{index}"),"summary":format!("主题{index}结论"),"byte_start":start,"byte_end":report.len(),"finding_start":index,"finding_end":index+1}));
            findings.push(ResearchFinding {
                finding: format!("后段关键证据{index}"),
                status: "partial".into(),
                citation_indices: vec![1],
                limitations: vec!["必须保留限制".into()],
                baseline_entry_id: None,
                baseline_claim_id: None,
            });
        }
        let material = project_report(&report, &json!(ranges), &findings, 7000).unwrap();
        assert!(estimated_tokens(&material) <= 7000);
        let parsed: Value = serde_json::from_str(&material).unwrap();
        assert_eq!(parsed["sections"].as_array().unwrap().len(), 10);
        for index in 0..10 {
            assert!(material.contains(&format!("LAST_CRITICAL_{index}")));
            assert!(material.contains(&format!("后段关键证据{index}")));
        }
        assert!(material.contains("$$d_k=d_v=128$$"));
        assert!(parsed["omitted_characters"].as_u64().unwrap() > 0);
        assert!(project_report(&report, &json!(ranges), &findings, 10).is_err());
        let mut invalid_ranges = ranges;
        invalid_ranges[0]["byte_start"] = json!(3); // inside the UTF-8 goal
        assert!(project_report(&report, &json!(invalid_ranges), &findings, 7000).is_err());
    }
}
