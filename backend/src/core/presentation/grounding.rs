use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use super::{citation_ordinal, slide_text_fields, validation_error, PresentationSpec};
use crate::error::BrainError;

static QUANTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)([+-]?[0-9]+(?:[,，][0-9]{3})*(?:\.[0-9]+)?)(?:[ \t]*(毫秒|微秒|纳秒|分钟|线程|请求|[％%秒次倍万亿个条]|GiB|MiB|KiB|GB|MB|KB|ns|ms|[µμu]s|s))?",
    )
    .expect("valid presentation quantity expression")
});

#[derive(Debug, PartialEq, Eq)]
struct Quantity {
    value: String,
    unit: String,
    decimal: bool,
}

#[derive(Debug)]
pub(crate) struct GroundingChecks {
    pub numbers: usize,
    pub quotes: usize,
}

fn unit_name(unit: &str) -> String {
    match unit.trim().to_lowercase().as_str() {
        "％" => "%".into(),
        "毫秒" => "ms".into(),
        "微秒" | "µs" | "μs" => "us".into(),
        "纳秒" => "ns".into(),
        "秒" => "s".into(),
        value => value.to_owned(),
    }
}

fn quantities(text: &str) -> Vec<Quantity> {
    QUANTITY
        .captures_iter(text)
        .filter_map(|captures| {
            let matched = captures.get(0)?;
            let before = &text[..matched.start()];
            let after = &text[matched.end()..];
            let is_identifier = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
            if before.chars().next_back().is_some_and(is_identifier)
                || after.chars().next().is_some_and(is_identifier)
                || after
                    .strip_prefix('.')
                    .is_some_and(|tail| tail.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
                || before.strip_suffix('.').is_some_and(|head| {
                    head.chars()
                        .next_back()
                        .is_some_and(|ch| ch.is_ascii_digit())
                })
            {
                return None;
            }
            let literal = captures.get(1)?.as_str().replace([',', '，'], "");
            let value = literal.parse::<f64>().ok()?.to_string();
            let unit = captures
                .get(2)
                .map(|unit| unit_name(unit.as_str()))
                .unwrap_or_default();
            Some(Quantity {
                value,
                unit,
                decimal: literal.contains('.'),
            })
        })
        .collect()
}

fn matching_number(expected: &Quantity, evidence: &[Quantity]) -> bool {
    evidence.iter().any(|given| {
        given.value == expected.value && (expected.unit.is_empty() || expected.unit == given.unit)
    })
}

pub(crate) fn presentation_source_number_labels(source: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    quantities(source)
        .into_iter()
        .filter(|number| number.decimal || !number.unit.is_empty())
        .map(|number| format!("{}{}", number.value, number.unit))
        .filter(|label| seen.insert(label.clone()))
        .take(24)
        .collect()
}

fn unit_in_heading(line: &str, unit: &str) -> bool {
    if unit.is_empty() {
        return true;
    }
    let canonical = unit_name(line);
    if canonical == unit {
        return true;
    }
    let lower = line.to_lowercase();
    lower
        .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '%')))
        .any(|label| label == unit)
        || match unit {
            "ms" => line.contains("毫秒"),
            "us" => line.contains("微秒") || line.contains("μs") || line.contains("µs"),
            "ns" => line.contains("纳秒"),
            "s" => {
                line.contains('秒')
                    && !["毫秒", "微秒", "纳秒"]
                        .iter()
                        .any(|tag| line.contains(tag))
            }
            "%" => line.contains('％'),
            other => !other.is_ascii() && line.contains(other),
        }
}

fn chart_unit(text: &str) -> String {
    // Chart axes allow a qualifying parenthesis after the displayed unit.
    unit_name(text.split(['（', '(']).next().unwrap_or_default())
}

fn same_label(expected: &str, text: &str) -> bool {
    let lower = text.to_lowercase();
    let expected = expected.trim().to_lowercase();
    lower.match_indices(&expected).any(|(start, matched)| {
        let end = start + matched.len();
        // Preserve complete identifiers such as CLOCK and FIFO.
        (!expected.starts_with(|ch: char| ch.is_ascii_alphanumeric())
            || !lower[..start]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
            && (!expected.ends_with(|ch: char| ch.is_ascii_alphanumeric())
                || !lower[end..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
    })
}

fn chart_point_is_supported(
    source: &str,
    category: &str,
    expected: &Quantity,
    categories: &[String],
) -> bool {
    let mut table_header: Vec<&str> = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with('|') {
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if table_header.is_empty() {
                table_header = cells.clone();
            }
            for (index, cell) in cells.iter().enumerate() {
                let header = table_header.get(index).copied().unwrap_or_default();
                let category_in_column = same_label(category, header);
                // Table rows can hold the workload and strategy in distinct
                // columns. A combined chart label must match the same row.
                let category_in_row = category
                    .split_whitespace()
                    .all(|term| cells.iter().any(|cell| same_label(term, cell)));
                if !category_in_column && !category_in_row {
                    continue;
                }
                let numbers = quantities(cell);
                if matching_number(expected, &numbers)
                    || (unit_in_heading(header, &expected.unit)
                        && numbers
                            .iter()
                            .any(|number| number.value == expected.value && number.unit.is_empty()))
                    || (category_in_column
                        && cells
                            .first()
                            .is_some_and(|label| unit_in_heading(label, &expected.unit))
                        && numbers
                            .iter()
                            .any(|number| number.value == expected.value && number.unit.is_empty()))
                {
                    return true;
                }
            }
            continue;
        }
        table_header.clear();
        for clause in line.split(['，', '。', '；', ';', '、']) {
            if same_label(category, clause)
                && !categories
                    .iter()
                    .filter(|other| *other != category)
                    .any(|other| same_label(other, clause))
                && matching_number(expected, &quantities(clause))
            {
                return true;
            }
        }
    }
    false
}

fn canonical_quote(text: &str) -> String {
    text.trim_matches(['“', '”', '"', '‘', '’', '\''])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn validate_presentation_grounding(
    spec: &PresentationSpec,
    originals: &BTreeMap<usize, &str>,
) -> Result<GroundingChecks, BrainError> {
    let mut checks = GroundingChecks {
        numbers: 0,
        quotes: 0,
    };
    let original_numbers = originals
        .iter()
        .map(|(&ordinal, text)| (ordinal, quantities(text)))
        .collect::<BTreeMap<_, _>>();
    let mut unsupported = BTreeSet::new();
    for (index, slide) in spec.slides.iter().enumerate() {
        let field = format!("slides[{index}]");
        let sources = slide
            .citations
            .iter()
            .map(|citation| {
                citation_ordinal(citation)
                    .and_then(|ordinal| originals.get(&ordinal).copied())
                    .ok_or_else(|| {
                        validation_error(&format!("{field} 的 {citation} 缺少冻结来源正文"))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source_numbers = sources
            .iter()
            .flat_map(|text| quantities(text))
            .collect::<Vec<_>>();
        let displayed = slide_text_fields(slide).into_iter().flat_map(quantities);
        for amount in displayed.filter(|value| !value.unit.is_empty() || value.decimal) {
            if !matching_number(&amount, &source_numbers) {
                let candidates = original_numbers
                    .iter()
                    .filter(|(_, numbers)| matching_number(&amount, numbers))
                    .take(3)
                    .map(|(ordinal, _)| format!("S{ordinal}"))
                    .collect::<Vec<_>>();
                let guidance = if candidates.is_empty() {
                    "保留原始测量值和比较条件".into()
                } else {
                    format!("现有报告可复查的出处为 {}", candidates.join("、"))
                };
                unsupported.insert(format!(
                    "{field} 的数值 {}{} 无法在本页引文中核对（{guidance}）",
                    amount.value, amount.unit
                ));
            }
            checks.numbers += 1;
        }
        if let Some(metric) = &slide.metric {
            let values = quantities(&metric.value);
            if values.is_empty()
                || values
                    .iter()
                    .any(|value| !matching_number(value, &source_numbers))
            {
                return Err(validation_error(&format!(
                    "{field}.metric.value 须来自本页引文中的可核对数值"
                )));
            }
            checks.numbers += values
                .iter()
                .filter(|value| value.unit.is_empty() && !value.decimal)
                .count();
        }
        if let Some(chart) = &slide.chart {
            let unit = chart_unit(&chart.unit);
            for (category, amount) in chart.categories.iter().zip(&chart.values) {
                let expected = Quantity {
                    value: amount.to_string(),
                    unit: unit.clone(),
                    decimal: amount.fract() != 0.0,
                };
                if !sources.iter().any(|source| {
                    chart_point_is_supported(source, category, &expected, &chart.categories)
                }) {
                    return Err(validation_error(&format!(
                        "{field}.chart 在本页引文中缺少「{category}」与数值 {amount}{} 的对应依据",
                        unit
                    )));
                }
                checks.numbers += 1;
            }
        }
        if !slide.quote.trim().is_empty() {
            let quote = canonical_quote(slide.quote.trim());
            if quote.is_empty()
                || !sources
                    .iter()
                    .any(|source| canonical_quote(source).contains(&quote))
            {
                return Err(validation_error(&format!(
                    "{field}.quote 无法在本页引文中核对原话"
                )));
            }
            checks.quotes += 1;
        }
    }
    if !unsupported.is_empty() {
        return Err(validation_error(
            &unsupported
                .into_iter()
                .take(10)
                .collect::<Vec<_>>()
                .join("。"),
        ));
    }
    Ok(checks)
}
