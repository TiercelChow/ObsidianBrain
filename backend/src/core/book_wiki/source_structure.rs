//! Lossless source locations and structural boundaries, not model summaries.

use std::collections::HashMap;
use std::ops::Range;

use crate::infra::book_wiki_store::SourceSectionDraft;
use crate::models::book_wiki::MARKDOWN_EXTRACTION_VERSION;
use pulldown_cmark::{Event, Options, Parser, Tag};

#[derive(Debug)]
pub(super) struct Heading {
    pub start: usize,
    pub level: usize,
    pub title: String,
    pub path: Vec<String>,
}

#[derive(Debug)]
struct Block {
    range: Range<usize>,
    divisible: bool,
}

#[derive(Debug)]
pub(super) struct DocumentStructure {
    pub headings: Vec<Heading>,
    blocks: Vec<Block>,
    inline_protected: Vec<Range<usize>>,
}

#[derive(Debug)]
pub(super) struct SourceFragment {
    pub content: String,
    pub char_start: usize,
    pub char_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub heading_path: Vec<String>,
    pub oversize_atomic: bool,
}

fn contains(ranges: &[Range<usize>], byte: usize) -> bool {
    let index = ranges.partition_point(|range| range.start <= byte);
    index
        .checked_sub(1)
        .is_some_and(|index| ranges[index].contains(&byte))
}

fn normalize_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = vec![];
    for range in ranges {
        if let Some(last) = merged.last_mut().filter(|last| last.end >= range.start) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

fn include_line_ending(content: &str, end: usize) -> usize {
    if content[end..].starts_with("\r\n") {
        end + 2
    } else if content[end..].starts_with('\n') {
        end + 1
    } else {
        end
    }
}

pub(super) fn line_offsets(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(content.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect()
}

// Titles are navigation labels, not a substitute for the intact source body.
// Malformed, unbounded headings must not consume the model's entire context.
pub(super) fn bounded_heading_path(path: &[String]) -> Vec<String> {
    path.iter()
        .take(6)
        .map(|title| title.chars().take(240).collect())
        .collect()
}

pub(super) fn section_locators(
    content: &str,
    sections: &[SourceSectionDraft],
) -> HashMap<String, serde_json::Value> {
    let offsets = line_offsets(content);
    let outline = parse_document(content);
    sections.iter().enumerate().map(|(index,section)|{
        let start=offsets.get(section.line_start.saturating_sub(1) as usize).copied().unwrap_or(0);
        let path=outline.headings.iter().rev().find(|heading|heading.start<=start).map(|heading|bounded_heading_path(&heading.path)).unwrap_or_default();
        (section.id.clone(),serde_json::json!({
            "extraction_version":MARKDOWN_EXTRACTION_VERSION,"heading_path":path,
            "previous_span_id":index.checked_sub(1).and_then(|previous|sections.get(previous)).map(|s|&s.id),
            "next_span_id":sections.get(index+1).map(|s|&s.id),
            "line_start":section.line_start,"line_end":section.line_end,
        }))
    }).collect()
}

fn frontmatter_range(content: &str) -> Option<Range<usize>> {
    let mut lines = content.split_inclusive('\n');
    let first = lines.next()?;
    let marker = first.trim().trim_start_matches('\u{feff}');
    if marker != "---" && marker != "+++" {
        return None;
    }
    let mut end = first.len();
    for line in lines {
        end += line.len();
        if line.trim() == marker || (marker == "---" && line.trim() == "...") {
            return Some(0..end);
        }
    }
    None
}

fn display_math_ranges(content: &str, excluded: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = vec![];
    let mut current: Option<(usize, &str)> = None;
    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        if contains(excluded, start + line.len() - line.trim_start().len()) {
            continue;
        }
        let text = line.trim();
        if let Some((opening, close)) = current {
            if text.ends_with(close) {
                ranges.push(opening..offset);
                current = None;
            }
        } else if let Some((open, close)) = [("$$", "$$"), ("\\[", "\\]")]
            .into_iter()
            .find(|(open, _)| text.starts_with(open))
        {
            if text[open.len()..].contains(close) {
                ranges.push(start..offset);
            } else {
                current = Some((start, close));
            }
        }
    }
    if let Some((start, _)) = current {
        ranges.push(start..content.len());
    }
    ranges
}

fn escaped(content: &str, byte: usize) -> bool {
    content.as_bytes()[..byte]
        .iter()
        .rev()
        .take_while(|ch| **ch == b'\\')
        .count()
        % 2
        == 1
}

fn inline_math_ranges(content: &str, excluded: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = vec![];
    let mut cursor = 0;
    while cursor < content.len() {
        if let Some(range) = excluded.iter().find(|range| range.contains(&cursor)) {
            cursor = range.end;
            continue;
        }
        let remaining = &content[cursor..];
        let delimiter = if remaining.starts_with("\\(") && !escaped(content, cursor) {
            Some(("\\(", "\\)"))
        } else if remaining.starts_with('$') && !escaped(content, cursor) {
            Some(("$", "$"))
        } else {
            None
        };
        if let Some((open, close)) = delimiter {
            let mut search = cursor + open.len();
            let mut end = None;
            while let Some(relative) = content[search..].find(close) {
                let candidate = search + relative;
                if !escaped(content, candidate) && !contains(excluded, candidate) {
                    end = Some(candidate + close.len());
                    break;
                }
                search = candidate + close.len();
            }
            if let Some(end) = end {
                ranges.push(cursor..end);
                cursor = end;
                continue;
            }
        }
        cursor += content[cursor..].chars().next().map_or(1, char::len_utf8);
    }
    ranges
}

pub(super) fn parse_document(content: &str) -> DocumentStructure {
    let mut depth = 0_usize;
    let mut root: Option<(usize, bool)> = None;
    let mut heading: Option<(usize, usize, String)> = None;
    let mut headings = vec![];
    let mut blocks = vec![];
    let mut code = vec![];
    let mut inline = vec![];
    let mut root_code = false;
    for (event, range) in Parser::new_ext(
        content,
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter()
    {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    root_code = matches!(tag, Tag::CodeBlock(_));
                    root = Some((range.start, matches!(tag, Tag::Paragraph)));
                    if let Tag::Heading { level, .. } = tag {
                        heading = Some((range.start, level as usize, String::new()));
                    }
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    if let Some((start, divisible)) = root.take() {
                        let end = include_line_ending(content, range.end);
                        if root_code {
                            code.push(start..end);
                        }
                        blocks.push(Block {
                            range: start..end,
                            divisible,
                        });
                    }
                    if let Some((start, level, title)) = heading.take() {
                        headings.push(Heading {
                            start,
                            level,
                            title,
                            path: vec![],
                        });
                    }
                }
            }
            Event::Text(text) => {
                if let Some((_, _, title)) = heading.as_mut() {
                    title.push_str(&text);
                }
            }
            Event::Code(text) => {
                inline.push(range);
                if let Some((_, _, title)) = heading.as_mut() {
                    title.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, title)) = heading.as_mut() {
                    title.push(' ');
                }
            }
            Event::Html(_) | Event::Rule if depth == 0 => blocks.push(Block {
                range,
                divisible: false,
            }),
            _ => {}
        }
    }
    if let Some(frontmatter) = frontmatter_range(content) {
        code.push(frontmatter);
    }
    let code = normalize_ranges(code);
    let math = display_math_ranges(content, &code);
    let mut excluded = code;
    excluded.extend(inline.iter().cloned());
    excluded.extend(math.iter().cloned());
    let excluded = normalize_ranges(excluded);
    inline.extend(inline_math_ranges(content, &excluded));
    headings.retain(|h| !contains(&excluded, h.start) && !h.title.trim().is_empty());
    let mut stack: Vec<(usize, String)> = vec![];
    for h in &mut headings {
        while stack.last().is_some_and(|(level, _)| *level >= h.level) {
            stack.pop();
        }
        h.title = h.title.trim().to_string();
        stack.push((h.level, h.title.clone()));
        h.path = stack.iter().map(|(_, title)| title.clone()).collect();
    }
    blocks.extend(math.into_iter().map(|range| Block {
        range,
        divisible: false,
    }));
    if let Some(range) = frontmatter_range(content) {
        blocks.push(Block {
            range,
            divisible: false,
        });
    }
    blocks.sort_by_key(|block| block.range.start);
    let mut merged: Vec<Block> = vec![];
    for block in blocks {
        if let Some(last) = merged
            .last_mut()
            .filter(|last| last.range.end > block.range.start)
        {
            last.range.end = last.range.end.max(block.range.end);
            last.divisible &= block.divisible;
        } else {
            merged.push(block);
        }
    }
    DocumentStructure {
        headings,
        blocks: merged,
        inline_protected: normalize_ranges(inline),
    }
}

fn paragraph_pieces(
    content: &str,
    range: Range<usize>,
    target: usize,
    protected: &[Range<usize>],
) -> Vec<Range<usize>> {
    let mut result = vec![];
    let mut start = range.start;
    while start < range.end {
        let mut end = content[start..range.end]
            .char_indices()
            .nth(target)
            .map_or(range.end, |(offset, _)| start + offset);
        if end < range.end {
            let preferred = content[start..end]
                .char_indices()
                .filter(|(_, ch)| "\n。！？.!?；; ".contains(*ch))
                .map(|(offset, ch)| start + offset + ch.len_utf8())
                .next_back();
            if let Some(preferred) = preferred.filter(|value| *value > start + (end - start) / 2) {
                end = preferred;
            }
            while let Some(atom) = protected
                .iter()
                .find(|atom| atom.start < end && atom.end > end)
            {
                end = atom.end.min(range.end);
            }
        }
        result.push(start..end);
        start = end;
    }
    result
}

pub(super) fn structured_fragments(content: &str, target: usize) -> Vec<SourceFragment> {
    let target = target.max(1);
    let structure = parse_document(content);
    let mut units = vec![];
    let mut cursor = 0;
    for block in &structure.blocks {
        if block.divisible {
            units.extend(paragraph_pieces(
                content,
                cursor..block.range.end,
                target,
                &structure.inline_protected,
            ));
        } else {
            units.push(cursor..block.range.end);
        }
        cursor = block.range.end;
    }
    if cursor < content.len() {
        units.push(cursor..content.len());
    }
    let mut pieces: Vec<Range<usize>> = vec![];
    let mut current = 0..0;
    let mut chars = 0;
    for unit in units {
        let size = content[unit.clone()].chars().count();
        if chars > 0 && chars + size > target {
            pieces.push(current.clone());
            current = unit.start..unit.start;
            chars = 0;
        }
        current.end = unit.end;
        chars += size;
    }
    if current.end > current.start {
        pieces.push(current);
    }
    let mut char_offset = 0;
    let mut line = 1;
    pieces
        .into_iter()
        .map(|range| {
            let value = &content[range.clone()];
            let size = value.chars().count();
            let newline_count = value.bytes().filter(|ch| *ch == b'\n').count();
            let fragment = SourceFragment {
                content: value.to_string(),
                char_start: char_offset,
                char_end: char_offset + size,
                line_start: line,
                line_end: (line + newline_count)
                    .saturating_sub(usize::from(value.ends_with('\n')))
                    .max(line),
                heading_path: structure
                    .headings
                    .iter()
                    .rev()
                    .find(|h| h.start <= range.start)
                    .map(|h| h.path.clone())
                    .unwrap_or_default(),
                oversize_atomic: size > target,
            };
            char_offset += size;
            line += newline_count;
            fragment
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_section_pipeline_reuses_ast_and_retains_original_line_endings() {
        let source =
            "主章\r\n====\r\n\r\n正文\r\n$$\r\n# 不是章节\r\n$$\r\n\r\n另一章\r\n====\r\n尾部\r\n";
        let sections = super::super::split_markdown_sections(source, "demo.md", "source", "v1");
        assert_eq!(
            sections
                .iter()
                .map(|s| s.title.as_str())
                .collect::<Vec<_>>(),
            vec!["主章", "另一章"]
        );
        assert_eq!(
            sections
                .iter()
                .map(|s| s.content_md.as_str())
                .collect::<String>(),
            source
        );
        assert_eq!(sections[1].line_start, 9);
        let locations = section_locators(source, &sections);
        assert!(locations[&sections[0].id]["previous_span_id"].is_null());
        assert_eq!(locations[&sections[0].id]["next_span_id"], sections[1].id);
        assert_eq!(
            locations[&sections[1].id]["previous_span_id"],
            sections[0].id
        );
        assert_eq!(
            locations[&sections[1].id]["heading_path"],
            serde_json::json!(["另一章"])
        );
    }

    #[test]
    fn test_outline_understands_setext_and_ignores_frontmatter_quotes_code_and_math() {
        let source = "---\ntitle: Demo\n---\n\n主章\n====\n\n> # 引用中的标题\n\n    # 缩进代码\n\n````md\n```\n# 代码中的标题\n````\n\n$$\n# 公式中的字符\nx=1\n$$\n\n## 子章 $E=mc^2$\n正文";
        let outline = parse_document(source);
        assert_eq!(
            outline
                .headings
                .iter()
                .map(|h| h.title.as_str())
                .collect::<Vec<_>>(),
            vec!["主章", "子章 $E=mc^2$"]
        );
        assert_eq!(outline.headings[1].path, vec!["主章", "子章 $E=mc^2$"]);
    }

    #[test]
    fn test_fragmenting_is_lossless_and_does_not_break_tables_code_or_display_math() {
        let table = format!("| A | B |\n|---|---|\n{}", "| 内容 | 数据 |\n".repeat(35));
        let code = format!("````rust\n{}\n```\n````\n", "let x = 1;\n".repeat(35));
        let math = format!("$$\n{}\n$$\n", "x_i = y_i + z_i \\\\\n".repeat(35));
        let source = format!("# 主章\r\n前文。\r\n\r\n{table}\n{code}\n{math}\n尾部。\n");
        let fragments = structured_fragments(&source, 120);
        assert_eq!(
            fragments
                .iter()
                .map(|f| f.content.as_str())
                .collect::<String>(),
            source
        );
        for protected in [&table, &code, &math] {
            assert!(
                fragments.iter().any(|f| f.content.contains(protected)),
                "protected block was split"
            );
        }
        assert!(fragments.iter().any(|f| f.oversize_atomic));
        for pair in fragments.windows(2) {
            assert_eq!(pair[0].char_end, pair[1].char_start);
        }
    }

    #[test]
    fn test_long_paragraph_preserves_inline_formula_and_code_when_fragmented() {
        let formula = format!("${}$", "\\alpha_i + ".repeat(30));
        let code = format!("`{}`", "do_not_split_this ".repeat(30));
        let source = format!(
            "{} {formula} then {code} finally {}",
            "prefix ".repeat(50),
            "end ".repeat(50)
        );
        let fragments = structured_fragments(&source, 80);
        assert_eq!(
            fragments
                .iter()
                .map(|f| f.content.as_str())
                .collect::<String>(),
            source
        );
        assert!(fragments.iter().any(|f| f.content.contains(&formula)));
        assert!(fragments.iter().any(|f| f.content.contains(&code)));
    }
}
