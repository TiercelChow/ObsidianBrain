use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A fully parsed Obsidian note, including frontmatter and section decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ParsedDocument {
    pub path: PathBuf,
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub title: String,
    pub tags: Vec<String>,
    pub sections: Vec<Section>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A section within a parsed document, representing a heading-level block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Section {
    /// Heading level (1–6).
    pub level: u8,
    /// Heading text, `None` for the root/body section before the first heading.
    pub heading: Option<String>,
    /// Breadcrumb trail from root to this section (e.g. ["Intro", "Details", "Sub"]).
    pub breadcrumb: Vec<String>,
    /// Section body content (excluding the heading line itself).
    pub content: String,
    /// Code blocks found within this section.
    pub code_blocks: Vec<CodeBlock>,
    /// 1-based line number where this section starts.
    pub line_start: usize,
    /// 1-based line number where this section ends.
    pub line_end: usize,
}

/// A fenced code block extracted from a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CodeBlock {
    /// Language annotation (e.g. "rust", "python"), `None` if unspecified.
    pub language: Option<String>,
    /// Raw code content.
    pub code: String,
    /// 1-based line number where the code block starts (including fence).
    pub line_start: usize,
    /// 1-based line number where the code block ends (including fence).
    pub line_end: usize,
}

/// Lightweight summary of a note, used in listing and search result previews.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_document_roundtrip() {
        let doc = ParsedDocument {
            path: PathBuf::from("notes/project.md"),
            frontmatter: HashMap::from([
                (
                    "status".to_string(),
                    serde_json::Value::String("active".to_string()),
                ),
                ("priority".to_string(), serde_json::Value::Number(1.into())),
            ]),
            title: "Project Overview".to_string(),
            tags: vec!["project".to_string(), "overview".to_string()],
            sections: vec![Section {
                level: 1,
                heading: Some("Introduction".to_string()),
                breadcrumb: vec!["Introduction".to_string()],
                content: "This is the intro.".to_string(),
                code_blocks: vec![CodeBlock {
                    language: Some("rust".to_string()),
                    code: "fn main() {}".to_string(),
                    line_start: 5,
                    line_end: 7,
                }],
                line_start: 3,
                line_end: 8,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: ParsedDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, doc.path);
        assert_eq!(parsed.title, doc.title);
        assert_eq!(parsed.tags, doc.tags);
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].heading, Some("Introduction".to_string()));
        assert_eq!(parsed.sections[0].code_blocks.len(), 1);
        assert_eq!(
            parsed.sections[0].code_blocks[0].language,
            Some("rust".to_string())
        );
    }

    #[test]
    fn test_section_roundtrip() {
        let section = Section {
            level: 2,
            heading: Some("Details".to_string()),
            breadcrumb: vec!["Intro".to_string(), "Details".to_string()],
            content: "Some details here.".to_string(),
            code_blocks: vec![],
            line_start: 10,
            line_end: 15,
        };
        let json = serde_json::to_string(&section).unwrap();
        let parsed: Section = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.level, section.level);
        assert_eq!(parsed.heading, section.heading);
        assert_eq!(parsed.breadcrumb, section.breadcrumb);
    }

    #[test]
    fn test_code_block_roundtrip() {
        let block = CodeBlock {
            language: Some("python".to_string()),
            code: "print('hello')".to_string(),
            line_start: 3,
            line_end: 5,
        };
        let json = serde_json::to_string(&block).unwrap();
        let parsed: CodeBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.language, block.language);
        assert_eq!(parsed.code, block.code);
        assert_eq!(parsed.line_start, block.line_start);
        assert_eq!(parsed.line_end, block.line_end);
    }

    #[test]
    fn test_code_block_no_language_roundtrip() {
        let block = CodeBlock {
            language: None,
            code: "raw code".to_string(),
            line_start: 1,
            line_end: 3,
        };
        let json = serde_json::to_string(&block).unwrap();
        let parsed: CodeBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.language, None);
        assert_eq!(parsed.code, block.code);
    }

    #[test]
    fn test_note_summary_roundtrip() {
        let summary = NoteSummary {
            path: "test.md".to_string(),
            title: "Test".to_string(),
            tags: vec!["tag1".to_string()],
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: NoteSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, summary.path);
        assert_eq!(parsed.title, summary.title);
        assert_eq!(parsed.tags, summary.tags);
    }

    #[test]
    fn test_parsed_document_empty_frontmatter() {
        let doc = ParsedDocument {
            path: PathBuf::from("simple.md"),
            frontmatter: HashMap::new(),
            title: "Simple Note".to_string(),
            tags: vec![],
            sections: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: ParsedDocument = serde_json::from_str(&json).unwrap();
        assert!(parsed.frontmatter.is_empty());
        assert!(parsed.tags.is_empty());
        assert!(parsed.sections.is_empty());
    }

    #[test]
    fn test_section_no_heading_roundtrip() {
        // Root/body section before any heading has no heading text.
        let section = Section {
            level: 0,
            heading: None,
            breadcrumb: vec![],
            content: "Preamble text before any heading.".to_string(),
            code_blocks: vec![],
            line_start: 1,
            line_end: 5,
        };
        let json = serde_json::to_string(&section).unwrap();
        let parsed: Section = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.heading, None);
        assert!(parsed.breadcrumb.is_empty());
    }
}
