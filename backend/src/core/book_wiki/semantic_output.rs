use std::collections::HashSet;

use serde_json::Value;

use crate::error::BrainError;

use super::normalize_semantic_candidate;

pub(super) const OUTPUT_SCHEMA: &str =
    include_str!("../../../prompts/wiki/semantic-output.schema.json");

#[derive(Debug)]
pub(super) struct SemanticBatchOutput {
    pub entries: Vec<Value>,
    pub no_material_reason: Option<String>,
    pub normalized_wrapper: bool,
}

fn invalid(message: impl Into<String>) -> BrainError {
    BrainError::KnowledgeValidation(message.into())
}

/// Recover only a single, complete object. Never combine objects or repair JSON
/// escapes heuristically: either could silently change the model's claims.
fn single_json_object(answer: &str) -> Result<(Value, bool), BrainError> {
    let trimmed = answer.trim().trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(invalid("语义编译返回空正文；必须返回完整 JSON 对象"));
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| invalid("语义编译结果不是有效 JSON 对象"))?;
    let prefix = &trimmed[..start];
    if prefix.contains(['[', ']', '}']) || serde_json::from_str::<Value>(prefix.trim()).is_ok() {
        return Err(invalid("语义编译顶层必须为单个对象，不能包装成数组"));
    }
    let mut stream = serde_json::Deserializer::from_str(&trimmed[start..]).into_iter::<Value>();
    let value = stream
        .next()
        .ok_or_else(|| invalid("语义编译缺少 JSON 对象"))?
        .map_err(|error| invalid(format!("语义编译 JSON 解析失败: {error}")))?;
    let suffix = trimmed[start + stream.byte_offset()..].trim();
    if suffix.contains(['{', '}', '[', ']']) {
        return Err(invalid(
            "语义编译返回多个对象或尾随结构；只允许一个完整 JSON 对象",
        ));
    }
    // A second scalar JSON value is also a second response, not commentary.
    if !suffix.is_empty() && serde_json::from_str::<Value>(suffix).is_ok() {
        return Err(invalid("语义编译 JSON 后存在第二个值"));
    }
    Ok((value, start > 0 || !suffix.is_empty()))
}

pub(super) fn parse_semantic_candidates(
    answer: &str,
    span_ids: &HashSet<String>,
    known_slugs: &HashSet<String>,
) -> Result<SemanticBatchOutput, BrainError> {
    let (value, normalized_wrapper) = single_json_object(answer)?;
    let schema: Value = serde_json::from_str(OUTPUT_SCHEMA)
        .map_err(|error| BrainError::Internal(format!("编译协议加载失败: {error}")))?;
    let validator = jsonschema::JSONSchema::compile(&schema)
        .map_err(|error| BrainError::Internal(format!("编译协议无效: {error}")))?;
    if let Err(errors) = validator.validate(&value) {
        let details = errors
            .take(5)
            .map(|error| {
                // Do not echo whole source strings or huge malformed fields in errors.
                let message = error.to_string().chars().take(240).collect::<String>();
                format!("{}: {message}", error.instance_path)
            })
            .collect::<Vec<_>>()
            .join("；");
        return Err(invalid(format!("语义编译字段校验失败: {details}")));
    }
    let entries = value["entries"]
        .as_array()
        .ok_or_else(|| invalid("语义编译缺少 entries 数组"))?;
    let no_material_reason = value
        .get("no_material_reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);
    if entries.is_empty() && no_material_reason.is_none() {
        return Err(invalid(
            "entries 为空时必须提供具体 no_material_reason；单片段也允许提取有价值主题",
        ));
    }
    let mut targets = known_slugs.clone();
    let mut batch_slugs = HashSet::new();
    for entry in entries {
        let slug = string(entry, "slug")?;
        if slug.contains(['/', '\\']) || slug.contains("..") {
            return Err(invalid("slug 不得包含路径或 .."));
        }
        if !batch_slugs.insert(slug.to_string()) {
            return Err(invalid(format!("本批重复 slug: {slug}；请先归并")));
        }
        targets.insert(slug.to_string());
    }
    for (index, entry) in entries.iter().enumerate() {
        let label = format!("entries/{index}");
        let citations = entry["citations"]
            .as_array()
            .ok_or_else(|| invalid(format!("{label}/citations 必须是数组")))?;
        check_citations(citations, span_ids, &label)?;
        if let Some(claims) = entry["claims"].as_array() {
            for (claim_index, claim) in claims.iter().enumerate() {
                let claim_label = format!("{label}/claims/{claim_index}");
                let references = claim["citations"]
                    .as_array()
                    .ok_or_else(|| invalid(format!("{claim_label}/citations 必须是数组")))?;
                check_citations(references, span_ids, &claim_label)?;
                if references
                    .iter()
                    .any(|reference| !citations.contains(reference))
                {
                    return Err(invalid(format!(
                        "{claim_label} 的引用必须包含在条目 citations 中"
                    )));
                }
            }
        }
        if let Some(relations) = entry["relations"].as_array() {
            for (relation_index, relation) in relations.iter().enumerate() {
                let target = string(relation, "to_slug")?;
                if !targets.contains(target) || target == string(entry, "slug")? {
                    return Err(invalid(format!(
                        "{label}/relations/{relation_index}/to_slug 不是已提供的其他主题: {target}"
                    )));
                }
            }
        }
    }
    Ok(SemanticBatchOutput {
        entries: entries
            .iter()
            .cloned()
            .map(normalize_semantic_candidate)
            .collect(),
        no_material_reason,
        normalized_wrapper,
    })
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, BrainError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(format!("{key} 必须为非空字符串")))
}

fn check_citations(
    citations: &[Value],
    span_ids: &HashSet<String>,
    label: &str,
) -> Result<(), BrainError> {
    for citation in citations {
        if !citation.as_str().is_some_and(|id| span_ids.contains(id)) {
            return Err(invalid(format!(
                "{label}/citations 含非本批来源 ID: {citation}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn candidate() -> Value {
        json!({
            "_classification": "new", "entry_type": "concept", "slug": "layering",
            "title": "分层", "summary": "将职责分为多个层次。", "aliases": [],
            "confidence": 0.8, "citations": ["s1"],
            "claims": [{"claim_text": "界面层负责交互。", "predicate": "states",
                "object_text": "界面层", "confidence": 0.8, "citations": ["s1"]}],
            "relations": []
        })
    }

    fn parse(answer: &str) -> Result<SemanticBatchOutput, BrainError> {
        parse_semantic_candidates(answer, &HashSet::from(["s1".to_string()]), &HashSet::new())
    }

    #[test]
    fn test_semantic_output_accepts_one_object_and_safe_wrapping() {
        let mut entry = candidate();
        entry["summary"] = json!("原文包含 {x}、\"引号\" 与 \\alpha。\n保留条件。");
        let answer = json!({"entries": [entry]}).to_string();
        for wrapped in [
            answer.clone(),
            format!("\u{feff}```json\n{answer}\n```"),
            format!("结果如下：\n{answer}\n编译完成。"),
        ] {
            let parsed = parse(&wrapped).unwrap();
            assert_eq!(parsed.entries.len(), 1);
            assert!(parsed.entries[0]["content_md"]
                .as_str()
                .unwrap()
                .contains("\\alpha"));
        }
    }

    #[test]
    fn test_semantic_output_rejects_ambiguous_truncated_and_empty_responses() {
        let answer = json!({"entries": [candidate()]}).to_string();
        for invalid in [
            format!("{answer}\n{answer}"),
            format!("[{answer}]"),
            format!("null {answer}"),
            answer[..answer.len() - 1].to_string(),
            String::new(),
            "{\"entries\":[]}".to_string(),
            "{\"entries\":[],\"no_material_reason\":\" \"}".to_string(),
        ] {
            assert!(parse(&invalid).is_err(), "accepted: {invalid}");
        }
        let empty = parse(
            r#"{"entries":[],"no_material_reason":"本批仅包含目录和导航链接，无事实内容。"}"#,
        )
        .unwrap();
        assert!(empty.entries.is_empty());
        assert!(empty.no_material_reason.unwrap().contains("目录"));
    }

    #[test]
    fn test_semantic_output_rejects_invalid_contracts_before_persistence() {
        for (pointer, value) in [
            ("/_classification", json!("new|update|disputed")),
            ("/entry_type", json!("unknown")),
            ("/confidence", json!(1.5)),
            ("/title", json!(" ")),
            ("/slug", json!("../unsafe")),
            ("/citations", json!(["other-book-span"])),
            ("/claims/0/citations", json!([])),
            ("/claims/0/citations", json!(["other-book-span"])),
            (
                "/relations",
                json!([{"to_slug":"unknown", "relation_type":"依赖", "strength":0.8,"evidence":"依赖关系"}]),
            ),
        ] {
            let mut entry = candidate();
            *entry.pointer_mut(pointer).unwrap() = value;
            assert!(
                parse(&json!({"entries":[entry]}).to_string()).is_err(),
                "accepted {pointer}"
            );
        }
        let mut entry = candidate();
        entry["extra"] = json!(true);
        assert!(parse(&json!({"entries":[entry]}).to_string()).is_err());
    }

    #[test]
    fn test_semantic_output_checks_claim_coverage_and_relation_targets() {
        let spans = HashSet::from(["s1".to_string(), "s2".to_string()]);
        let known = HashSet::from(["existing-topic".to_string()]);
        let mut entry = candidate();
        entry["relations"] = json!([{"to_slug":"existing-topic", "relation_type":"依赖",
            "strength":0.7, "evidence":"来源说明它需要既有主题的条件。"}]);
        let answer = json!({"entries":[entry.clone()]}).to_string();
        assert!(parse_semantic_candidates(&answer, &spans, &known).is_ok());

        entry["claims"][0]["citations"] = json!(["s2"]);
        let error =
            parse_semantic_candidates(&json!({"entries":[entry]}).to_string(), &spans, &known)
                .unwrap_err();
        assert!(error.to_string().contains("条目 citations"));
        assert!(parse(&json!({"entries":[candidate(), candidate()]}).to_string()).is_err());
        assert!(parse(&json!({"entries":vec![candidate(); 6]}).to_string()).is_err());
    }
}
