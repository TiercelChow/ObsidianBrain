//! Markdown parser for Obsidian notes.
//!
//! Parses Markdown files into structured `ParsedDocument` with frontmatter,
//! sections, headings, code blocks, and tags.

use std::collections::HashSet;
use std::path::Path;

use chrono::Utc;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser as CmarkParser, Tag, TagEnd};

use crate::error::BrainError;
use crate::models::{CodeBlock, ParsedDocument, Section};

/// Markdown parser that converts raw Markdown content into a structured `ParsedDocument`.
#[allow(dead_code)]
pub struct MarkdownParser;

/// Internal state for section parsing.
#[allow(dead_code)]
struct SectionState {
    /// Whether we are currently inside a heading (between Start(Heading) and End(Heading)).
    in_heading: bool,
    /// Accumulated heading text.
    heading_text: String,
    /// Heading level (1-6) for the heading currently being parsed.
    heading_level: u8,
    /// Whether we are currently inside a code block.
    in_code_block: bool,
    /// Language annotation for the current code block.
    code_block_language: Option<String>,
    /// Accumulated code block content.
    code_block_content: String,
    /// Byte offset where the current code block started.
    code_block_byte_start: usize,
    /// Whether we have accumulated preamble content (before the first heading).
    has_preamble: bool,
    /// Preamble text content.
    preamble_content: String,
    /// Preamble code blocks.
    preamble_code_blocks: Vec<CodeBlock>,
    /// Byte offset where preamble started.
    preamble_byte_start: usize,
    /// The heading stack representing the current hierarchy context.
    /// Each entry is (level, heading_text).
    heading_stack: Vec<(u8, String)>,
    /// The currently active section (after its heading has been finalized).
    current_level: u8,
    current_heading: Option<String>,
    current_content: String,
    current_code_blocks: Vec<CodeBlock>,
    current_byte_start: usize,
    /// All finalized sections.
    sections: Vec<Section>,
}

#[allow(dead_code)]
impl MarkdownParser {
    /// Parse a Markdown file into a `ParsedDocument`.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path (used as the document identifier and for title fallback).
    /// * `content` - The raw Markdown content (including frontmatter if present).
    ///
    /// # Returns
    ///
    /// A `ParsedDocument` with extracted frontmatter, sections, tags, and title.
    pub fn parse(path: &str, content: &str) -> Result<ParsedDocument, BrainError> {
        // 1. Extract frontmatter using gray_matter
        let matter: Matter<YAML> = Matter::new();
        let parsed_entity = matter.parse(content);

        let frontmatter = Self::extract_frontmatter(parsed_entity.data.as_ref());
        let body = parsed_entity.content;

        // 2. Parse the body using pulldown-cmark
        let sections = Self::parse_sections(&body);

        // 3. Extract tags (merge frontmatter tags + inline body tags, excluding code blocks)
        let tags = Self::extract_tags(&frontmatter, &body, &sections);

        // 4. Resolve title (frontmatter.title > first H1 > filename)
        let title = Self::resolve_title(&frontmatter, &sections, path);

        let now = Utc::now();

        Ok(ParsedDocument {
            path: path.to_string(),
            frontmatter,
            title,
            tags,
            sections,
            created_at: now,
            updated_at: now,
        })
    }

    /// Convert gray_matter Pod data into a HashMap of serde_json::Value.
    fn extract_frontmatter(
        pod_data: Option<&gray_matter::Pod>,
    ) -> std::collections::HashMap<String, serde_json::Value> {
        match pod_data {
            Some(pod) => {
                let json_value: serde_json::Value = pod.clone().into();
                match json_value {
                    serde_json::Value::Object(map) => map
                        .into_iter()
                        .collect::<std::collections::HashMap<String, serde_json::Value>>(),
                    _ => std::collections::HashMap::new(),
                }
            }
            None => std::collections::HashMap::new(),
        }
    }

    /// Parse the Markdown body into sections based on heading hierarchy.
    fn parse_sections(body: &str) -> Vec<Section> {
        if body.is_empty() {
            return Vec::new();
        }

        let options = Options::ENABLE_TABLES;
        let cmark_parser = CmarkParser::new_ext(body, options);
        let line_offsets = Self::compute_line_offsets(body);

        let mut state = SectionState {
            in_heading: false,
            heading_text: String::new(),
            heading_level: 0,
            in_code_block: false,
            code_block_language: None,
            code_block_content: String::new(),
            code_block_byte_start: 0,
            has_preamble: false,
            preamble_content: String::new(),
            preamble_code_blocks: Vec::new(),
            preamble_byte_start: 0,
            heading_stack: Vec::new(),
            current_level: 0,
            current_heading: None,
            current_content: String::new(),
            current_code_blocks: Vec::new(),
            current_byte_start: 0,
            sections: Vec::new(),
        };

        for (event, byte_range) in cmark_parser.into_offset_iter() {
            Self::handle_event(
                &mut state,
                event,
                byte_range.start,
                byte_range.end,
                &line_offsets,
            );
        }

        // Finalize remaining section or preamble
        Self::finalize_remaining(&mut state, body.len(), &line_offsets);

        state.sections
    }

    fn handle_event(
        state: &mut SectionState,
        event: Event,
        byte_start: usize,
        byte_end: usize,
        line_offsets: &[usize],
    ) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let heading_level = Self::heading_level_to_u8(level);

                // Finalize previous section or preamble before starting a new heading
                if state.current_heading.is_some() || state.current_level > 0 {
                    // Finalize the current section
                    let line_start =
                        Self::byte_offset_to_line(line_offsets, state.current_byte_start);
                    let line_end = Self::byte_offset_to_line(line_offsets, byte_start);
                    let line_end = if line_end > line_start {
                        line_end - 1
                    } else {
                        line_start
                    };

                    let breadcrumb = state
                        .heading_stack
                        .iter()
                        .map(|(_, text)| text.clone())
                        .collect();

                    let heading = std::mem::take(&mut state.current_heading);
                    let code_blocks = std::mem::take(&mut state.current_code_blocks);
                    let content = std::mem::take(&mut state.current_content);
                    let content = content.trim().to_string();

                    state.sections.push(Section {
                        level: state.current_level,
                        heading,
                        breadcrumb,
                        content,
                        code_blocks,
                        line_start,
                        line_end,
                    });
                } else if state.has_preamble {
                    // Finalize preamble section
                    let line_start =
                        Self::byte_offset_to_line(line_offsets, state.preamble_byte_start);
                    let line_end = Self::byte_offset_to_line(line_offsets, byte_start);
                    let line_end = if line_end > line_start {
                        line_end - 1
                    } else {
                        line_start
                    };

                    let preamble_content = std::mem::take(&mut state.preamble_content);
                    let preamble_code_blocks = std::mem::take(&mut state.preamble_code_blocks);

                    state.sections.push(Section {
                        level: 0,
                        heading: None,
                        breadcrumb: Vec::new(),
                        content: preamble_content.trim().to_string(),
                        code_blocks: preamble_code_blocks,
                        line_start,
                        line_end,
                    });

                    state.has_preamble = false;
                }

                // Start new heading section
                state.in_heading = true;
                state.heading_text = String::new();
                state.heading_level = heading_level;
                state.current_level = heading_level;
                state.current_heading = None;
                state.current_byte_start = byte_start;
            }

            Event::End(TagEnd::Heading(_)) => {
                state.in_heading = false;

                // Pop heading stack entries with level >= current heading level
                while !state.heading_stack.is_empty()
                    && state
                        .heading_stack
                        .last()
                        .is_some_and(|(l, _)| *l >= state.heading_level)
                {
                    state.heading_stack.pop();
                }

                // Push current heading onto the stack
                let heading_text = state.heading_text.trim().to_string();
                if !heading_text.is_empty() {
                    state
                        .heading_stack
                        .push((state.heading_level, heading_text.clone()));
                    state.current_heading = Some(heading_text);
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                state.in_code_block = true;
                state.code_block_language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        if lang.is_empty() {
                            None
                        } else {
                            Some(lang.to_string())
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                state.code_block_content = String::new();
                state.code_block_byte_start = byte_start;
            }

            Event::End(TagEnd::CodeBlock) => {
                state.in_code_block = false;

                let code_line_start =
                    Self::byte_offset_to_line(line_offsets, state.code_block_byte_start);
                let code_line_end = Self::byte_offset_to_line(line_offsets, byte_end);

                let code_block = CodeBlock {
                    language: state.code_block_language.clone(),
                    code: state.code_block_content.clone(),
                    line_start: code_line_start,
                    line_end: code_line_end,
                };

                if state.current_heading.is_some() {
                    state.current_code_blocks.push(code_block);
                } else if !state.in_heading && state.current_level == 0 {
                    if !state.has_preamble {
                        state.has_preamble = true;
                        state.preamble_byte_start = byte_start;
                    }
                    state.preamble_code_blocks.push(code_block);
                } else {
                    // Heading hasn't been finalized yet (between Start and End of Heading),
                    // but code blocks shouldn't normally appear inside headings.
                    // Still, add to current section's code blocks for robustness.
                    state.current_code_blocks.push(code_block);
                }

                state.code_block_content = String::new();
                state.code_block_language = None;
            }

            Event::Text(text) | Event::Code(text) => {
                if state.in_code_block {
                    state.code_block_content.push_str(&text);
                } else if state.in_heading {
                    state.heading_text.push_str(&text);
                } else if state.current_heading.is_some() || state.current_level > 0 {
                    state.current_content.push_str(&text);
                } else {
                    // Preamble content (before first heading)
                    if !state.has_preamble {
                        state.has_preamble = true;
                        state.preamble_byte_start = byte_start;
                    }
                    state.preamble_content.push_str(&text);
                }
            }

            Event::SoftBreak | Event::HardBreak => {
                if state.in_code_block {
                    state.code_block_content.push('\n');
                } else if state.in_heading {
                    // Line break inside heading is unusual but handle it
                    state.heading_text.push(' ');
                } else if state.current_heading.is_some() || state.current_level > 0 {
                    state.current_content.push('\n');
                } else if state.has_preamble {
                    state.preamble_content.push('\n');
                }
            }

            Event::InlineHtml(html) | Event::Html(html) => {
                if state.in_code_block {
                    state.code_block_content.push_str(&html);
                } else if state.in_heading {
                    state.heading_text.push_str(&html);
                } else if state.current_heading.is_some() || state.current_level > 0 {
                    state.current_content.push_str(&html);
                } else if state.has_preamble {
                    state.preamble_content.push_str(&html);
                }
            }

            // Structural events that don't contribute text content
            Event::Start(Tag::Paragraph)
            | Event::End(TagEnd::Paragraph)
            | Event::Start(Tag::Emphasis)
            | Event::End(TagEnd::Emphasis)
            | Event::Start(Tag::Strong)
            | Event::End(TagEnd::Strong)
            | Event::Start(Tag::List(_))
            | Event::End(TagEnd::List(_))
            | Event::Start(Tag::Item)
            | Event::End(TagEnd::Item)
            | Event::Start(Tag::Link { .. })
            | Event::End(TagEnd::Link)
            | Event::Start(Tag::Image { .. })
            | Event::End(TagEnd::Image)
            | Event::Start(Tag::BlockQuote)
            | Event::End(TagEnd::BlockQuote)
            | Event::Start(Tag::Table(_))
            | Event::End(TagEnd::Table)
            | Event::Start(Tag::TableHead)
            | Event::End(TagEnd::TableHead)
            | Event::Start(Tag::TableRow)
            | Event::End(TagEnd::TableRow)
            | Event::Start(Tag::TableCell)
            | Event::End(TagEnd::TableCell)
            | Event::Start(Tag::Strikethrough)
            | Event::End(TagEnd::Strikethrough)
            | Event::Start(Tag::MetadataBlock(_))
            | Event::End(TagEnd::MetadataBlock(_))
            | Event::Start(Tag::HtmlBlock)
            | Event::End(TagEnd::HtmlBlock)
            | Event::Start(Tag::FootnoteDefinition(_))
            | Event::End(TagEnd::FootnoteDefinition)
            | Event::FootnoteReference(_)
            | Event::Rule => {}

            Event::TaskListMarker(_checked) => {}
        }
    }

    fn finalize_remaining(state: &mut SectionState, body_len: usize, line_offsets: &[usize]) {
        let last_line = if body_len > 0 {
            Self::byte_offset_to_line(line_offsets, body_len.saturating_sub(1))
        } else {
            1
        };

        if state.current_heading.is_some() || state.current_level > 0 {
            let line_start = Self::byte_offset_to_line(line_offsets, state.current_byte_start);
            let breadcrumb = state
                .heading_stack
                .iter()
                .map(|(_, text)| text.clone())
                .collect();

            let heading = std::mem::take(&mut state.current_heading);
            let code_blocks = std::mem::take(&mut state.current_code_blocks);
            let content = std::mem::take(&mut state.current_content);
            let content = content.trim().to_string();

            state.sections.push(Section {
                level: state.current_level,
                heading,
                breadcrumb,
                content,
                code_blocks,
                line_start,
                line_end: last_line,
            });
        } else if state.has_preamble {
            let line_start = Self::byte_offset_to_line(line_offsets, state.preamble_byte_start);
            let preamble_content = std::mem::take(&mut state.preamble_content);
            let preamble_code_blocks = std::mem::take(&mut state.preamble_code_blocks);
            state.sections.push(Section {
                level: 0,
                heading: None,
                breadcrumb: Vec::new(),
                content: preamble_content.trim().to_string(),
                code_blocks: preamble_code_blocks,
                line_start,
                line_end: last_line,
            });
        }
    }

    /// Convert pulldown-cmark HeadingLevel to u8.
    fn heading_level_to_u8(level: HeadingLevel) -> u8 {
        match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
    }

    /// Precompute byte offsets for each line start in the content.
    /// Returns a vector where index i holds the byte offset of line (i+1)'s start.
    fn compute_line_offsets(content: &str) -> Vec<usize> {
        let mut offsets = Vec::new();
        offsets.push(0); // Line 1 starts at byte 0
        for (byte_offset, ch) in content.char_indices() {
            if ch == '\n' {
                offsets.push(byte_offset + 1);
            }
        }
        offsets
    }

    /// Convert a byte offset to a 1-based line number using precomputed line offsets.
    fn byte_offset_to_line(line_offsets: &[usize], byte_offset: usize) -> usize {
        match line_offsets.binary_search(&byte_offset) {
            Ok(line_idx) => line_idx + 1, // 1-based
            Err(line_idx) => {
                if line_idx == 0 {
                    1
                } else if line_idx >= line_offsets.len() {
                    line_offsets.len()
                } else {
                    line_idx // The line that contains this byte offset
                }
            }
        }
    }

    /// Extract tags from frontmatter and body, merge and deduplicate.
    /// Tags from body code blocks are excluded.
    fn extract_tags(
        frontmatter: &std::collections::HashMap<String, serde_json::Value>,
        body: &str,
        sections: &[Section],
    ) -> Vec<String> {
        let mut tags_set: HashSet<String> = HashSet::new();

        // 1. Extract tags from frontmatter
        if let Some(serde_json::Value::Array(arr)) = frontmatter.get("tags") {
            for item in arr {
                if let serde_json::Value::String(s) = item {
                    tags_set.insert(s.clone());
                }
            }
        }

        // 2. Extract inline #tags from body (excluding code blocks)
        // Collect all code block line ranges to skip
        let code_block_ranges: Vec<(usize, usize)> = sections
            .iter()
            .flat_map(|s| s.code_blocks.iter())
            .map(|cb| (cb.line_start, cb.line_end))
            .collect();

        for (line_num, line) in body.lines().enumerate() {
            let line_num_1based = line_num + 1;
            // Check if this line falls within any code block range
            let in_code_block = code_block_ranges
                .iter()
                .any(|(start, end)| line_num_1based >= *start && line_num_1based <= *end);

            if !in_code_block {
                Self::extract_inline_tags_from_text(line, &mut tags_set);
            }
        }

        // Deduplicate and sort alphabetically
        let mut tags: Vec<String> = tags_set.into_iter().collect();
        tags.sort();
        tags
    }

    /// Extract inline #tags from a single line of text.
    /// A tag is a # followed by word characters (alphanumeric, underscore, hyphen),
    /// preceded by whitespace or start of line, and not part of a heading (# at line start).
    fn extract_inline_tags_from_text(line: &str, tags_set: &mut HashSet<String>) {
        let trimmed = line.trim_start();

        // If the line starts with # followed by a space, it's a heading, not a tag line.
        // Skip heading lines entirely to avoid false matches.
        if trimmed.starts_with('#') && trimmed.chars().nth(1) == Some(' ') {
            return;
        }

        let mut chars = line.char_indices().peekable();

        while let Some((i, ch)) = chars.next() {
            if ch == '#' {
                // Check that the character before # is whitespace or start of line
                if i == 0
                    || line[..i]
                        .chars()
                        .next_back()
                        .is_none_or(|c| c.is_whitespace())
                {
                    // Collect the tag name: word characters after #
                    let mut tag_name = String::new();
                    while let Some(&(_, next_ch)) = chars.peek() {
                        if next_ch.is_alphanumeric() || next_ch == '_' || next_ch == '-' {
                            tag_name.push(next_ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if !tag_name.is_empty() {
                        tags_set.insert(tag_name);
                    }
                }
            }
        }
    }

    /// Resolve the document title with priority: frontmatter.title > first H1 > filename stem.
    fn resolve_title(
        frontmatter: &std::collections::HashMap<String, serde_json::Value>,
        sections: &[Section],
        path: &str,
    ) -> String {
        // 1. Check frontmatter.title
        if let Some(serde_json::Value::String(title)) = frontmatter.get("title") {
            return title.clone();
        }

        // 2. Check first H1 heading
        for section in sections {
            if section.level == 1 {
                if let Some(heading) = &section.heading {
                    return heading.clone();
                }
            }
        }

        // 3. Fall back to filename stem
        Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_with_frontmatter_and_heading_hierarchy() {
        let content = r#"---
title: My Project
tags:
  - project
  - overview
---
# Introduction

This is the introduction.

## Details

Some details here.

### Subsection

Sub details.
"#;

        let doc = MarkdownParser::parse("notes/project.md", content).unwrap();

        // Title from frontmatter
        assert_eq!(doc.title, "My Project");

        // Frontmatter tags
        assert!(doc.tags.contains(&"project".to_string()));
        assert!(doc.tags.contains(&"overview".to_string()));

        // Sections: Introduction (H1), Details (H2), Subsection (H3)
        assert_eq!(doc.sections.len(), 3);

        // H1 section
        let intro = &doc.sections[0];
        assert_eq!(intro.level, 1);
        assert_eq!(intro.heading, Some("Introduction".to_string()));
        assert_eq!(intro.breadcrumb, vec!["Introduction".to_string()]);
        assert!(intro.content.contains("This is the introduction."));

        // H2 section
        let details = &doc.sections[1];
        assert_eq!(details.level, 2);
        assert_eq!(details.heading, Some("Details".to_string()));
        assert_eq!(
            details.breadcrumb,
            vec!["Introduction".to_string(), "Details".to_string()]
        );

        // H3 section
        let subsection = &doc.sections[2];
        assert_eq!(subsection.level, 3);
        assert_eq!(subsection.heading, Some("Subsection".to_string()));
        assert_eq!(
            subsection.breadcrumb,
            vec![
                "Introduction".to_string(),
                "Details".to_string(),
                "Subsection".to_string()
            ]
        );
    }

    #[test]
    fn test_extract_tags_from_frontmatter_array() {
        let content = r#"---
tags:
  - rust
  - programming
  - memory
---
Some content.
"#;

        let doc = MarkdownParser::parse("notes/test.md", content).unwrap();

        assert!(doc.tags.contains(&"rust".to_string()));
        assert!(doc.tags.contains(&"programming".to_string()));
        assert!(doc.tags.contains(&"memory".to_string()));
    }

    #[test]
    fn test_extract_inline_tags_from_body_not_from_code_blocks() {
        let content = r#"---
tags:
  - frontmatter-tag
---
# Heading

Here is a #body-tag and another #inline-tag.

```rust
// This #fake-tag should NOT be extracted
let x = 42;
```

After code block, a #post-tag appears.
"#;

        let doc = MarkdownParser::parse("notes/tags.md", content).unwrap();

        // Frontmatter tag should be present
        assert!(doc.tags.contains(&"frontmatter-tag".to_string()));

        // Inline body tags should be present
        assert!(doc.tags.contains(&"body-tag".to_string()));
        assert!(doc.tags.contains(&"inline-tag".to_string()));
        assert!(doc.tags.contains(&"post-tag".to_string()));

        // Tags are sorted alphabetically
        assert_eq!(
            doc.tags,
            vec!["body-tag", "frontmatter-tag", "inline-tag", "post-tag"]
        );

        // The fake-tag from code block should NOT be present
        assert!(
            !doc.tags.contains(&"fake-tag".to_string()),
            "Tags inside code blocks must not be extracted"
        );
    }

    #[test]
    fn test_code_block_preservation_with_language_and_line_numbers() {
        let content = r#"---
title: Code Example
---
# Code

Here is some code:

```rust
fn main() {
    println!("hello");
}
```

And some inline code: `let x = 5`.
"#;

        let doc = MarkdownParser::parse("notes/code.md", content).unwrap();

        assert_eq!(doc.title, "Code Example");
        assert_eq!(doc.sections.len(), 1);

        let section = &doc.sections[0];
        assert_eq!(section.level, 1);
        assert_eq!(section.heading, Some("Code".to_string()));

        // Should have one code block
        assert_eq!(section.code_blocks.len(), 1);

        let code_block = &section.code_blocks[0];
        assert_eq!(code_block.language, Some("rust".to_string()));
        assert!(
            code_block.code.contains("fn main()"),
            "Code block content should contain 'fn main()'"
        );
        assert!(
            code_block.code.contains("println!("),
            "Code block content should contain 'println!'"
        );

        // Line numbers should be valid (1-based)
        assert!(code_block.line_start > 0);
        assert!(code_block.line_end > 0);
        assert!(code_block.line_end >= code_block.line_start);

        // Exact line number assertions (relative to body after frontmatter removal)
        assert_eq!(code_block.line_start, 5);
        assert_eq!(code_block.line_end, 9);
    }

    #[test]
    fn test_code_block_no_language_annotation() {
        let content = r#"---
title: Raw Code
---
# Section

```
plain code here
```
"#;

        let doc = MarkdownParser::parse("notes/raw_code.md", content).unwrap();

        let section = &doc.sections[0];
        assert_eq!(section.code_blocks.len(), 1);

        let code_block = &section.code_blocks[0];
        assert_eq!(code_block.language, None);
        assert!(code_block.code.contains("plain code here"));
    }

    #[test]
    fn test_title_fallback_frontmatter_then_h1_then_filename() {
        // Case 1: frontmatter.title present
        let content1 = r#"---
title: Frontmatter Title
---
# H1 Title

Some content.
"#;
        let doc1 = MarkdownParser::parse("notes/fallback1.md", content1).unwrap();
        assert_eq!(doc1.title, "Frontmatter Title");

        // Case 2: no frontmatter title, H1 present
        let content2 = r#"# H1 Title

Some content.
"#;
        let doc2 = MarkdownParser::parse("notes/fallback2.md", content2).unwrap();
        assert_eq!(doc2.title, "H1 Title");

        // Case 3: no frontmatter title, no H1, fallback to filename
        let content3 = "Just some text without a heading.";
        let doc3 = MarkdownParser::parse("notes/my_note.md", content3).unwrap();
        assert_eq!(doc3.title, "my_note");
    }

    #[test]
    fn test_empty_note_valid_parsed_document() {
        let content = "";
        let doc = MarkdownParser::parse("notes/empty.md", content).unwrap();

        assert_eq!(doc.path, "notes/empty.md");
        assert!(doc.frontmatter.is_empty());
        assert!(doc.sections.is_empty());
        assert!(doc.tags.is_empty());
        // Title falls back to filename
        assert_eq!(doc.title, "empty");
    }

    #[test]
    fn test_note_with_only_frontmatter() {
        let content = r#"---
title: Just Metadata
tags:
  - meta
  - test
---
"#;

        let doc = MarkdownParser::parse("notes/metadata_only.md", content).unwrap();

        assert_eq!(doc.title, "Just Metadata");
        assert!(doc.tags.contains(&"meta".to_string()));
        assert!(doc.tags.contains(&"test".to_string()));
        assert!(doc.frontmatter.contains_key("title"));
        assert!(doc.frontmatter.contains_key("tags"));

        // Body is empty/whitespace after frontmatter removal, so sections should be empty
        assert!(doc.sections.iter().all(|s| s.content.trim().is_empty()));
    }

    #[test]
    fn test_multiple_h1_headings() {
        let content = r#"---
title: Multi H1
---
# First

Content of first.

# Second

Content of second.

## Sub of Second

Sub content.
"#;

        let doc = MarkdownParser::parse("notes/multi_h1.md", content).unwrap();

        assert_eq!(doc.title, "Multi H1"); // frontmatter wins

        // Should have 3 sections: First (H1), Second (H1), Sub of Second (H2)
        assert_eq!(doc.sections.len(), 3);

        let first = &doc.sections[0];
        assert_eq!(first.level, 1);
        assert_eq!(first.heading, Some("First".to_string()));
        assert_eq!(first.breadcrumb, vec!["First".to_string()]);
        assert!(first.content.contains("Content of first."));

        let second = &doc.sections[1];
        assert_eq!(second.level, 1);
        assert_eq!(second.heading, Some("Second".to_string()));
        // When a new H1 appears, the breadcrumb resets
        assert_eq!(second.breadcrumb, vec!["Second".to_string()]);
        assert!(second.content.contains("Content of second."));

        let sub = &doc.sections[2];
        assert_eq!(sub.level, 2);
        assert_eq!(
            sub.breadcrumb,
            vec!["Second".to_string(), "Sub of Second".to_string()]
        );
    }

    #[test]
    fn test_preamble_before_first_heading() {
        let content = r#"---
title: With Preamble
---
Some intro text before any heading.

# First Section

Section content.
"#;

        let doc = MarkdownParser::parse("notes/preamble.md", content).unwrap();

        // Should have preamble section (level 0, no heading) + H1 section
        assert_eq!(doc.sections.len(), 2);

        let preamble = &doc.sections[0];
        assert_eq!(preamble.level, 0);
        assert_eq!(preamble.heading, None);
        assert!(preamble.breadcrumb.is_empty());
        assert!(
            preamble
                .content
                .contains("Some intro text before any heading."),
            "Preamble content: {:?}",
            preamble.content
        );

        let first = &doc.sections[1];
        assert_eq!(first.level, 1);
        assert_eq!(first.heading, Some("First Section".to_string()));
    }

    #[test]
    fn test_frontmatter_non_string_title_ignored() {
        let content = r#"---
title: 42
---
# Real Title
"#;

        let doc = MarkdownParser::parse("notes/number_title.md", content).unwrap();

        // title is a number (42), not a string, so it should fall back to H1
        assert_eq!(doc.title, "Real Title");
    }

    #[test]
    fn test_nested_code_blocks_count() {
        let content = r#"---
title: Multiple Code Blocks
---
# Examples

```python
print("hello")
```

Some text between blocks.

```javascript
console.log("world");
```

End of section.
"#;

        let doc = MarkdownParser::parse("notes/multi_code.md", content).unwrap();

        let section = &doc.sections[0];
        assert_eq!(section.code_blocks.len(), 2);
        assert_eq!(section.code_blocks[0].language, Some("python".to_string()));
        assert_eq!(
            section.code_blocks[1].language,
            Some("javascript".to_string())
        );
    }

    #[test]
    fn test_empty_heading_preserves_content() {
        let content = "# \n\nSome content under empty heading\n\n## Next";
        let doc = MarkdownParser::parse("test.md", content).unwrap();
        // The content under the empty heading should be preserved
        let section = doc
            .sections
            .iter()
            .find(|s| s.content.contains("Some content"))
            .unwrap();
        assert!(section.heading.as_ref().is_none_or(|h| h.is_empty()));
    }
}
