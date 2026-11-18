use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;

use super::{
    citation_markers, citation_numbers, source_structure, split_topic_sections,
    validate_topic_output, BookWikiService, PAGE_CHARS,
};
use crate::error::BrainError;
use crate::models::wiki_topics::{
    TopicDraftCitation, TopicPage, TopicWritebackKind, TopicWritebackMaterial,
    TopicWritebackPassage, TopicWritebackRequest,
};

const MAX_PASSAGES: usize = 16;
type CitationKeys = HashMap<usize, (String, String)>;

impl BookWikiService {
    pub fn topic_writeback_material(
        &self,
        base_id: &str,
        run_id: &str,
    ) -> Result<TopicWritebackMaterial, BrainError> {
        Ok(self.read_writeback_material(base_id, run_id)?.0)
    }

    pub fn draft_topic_from_material(
        &self,
        base_id: &str,
        page_id: &str,
        mut request: TopicWritebackRequest,
    ) -> Result<TopicPage, BrainError> {
        if !(1..=MAX_PASSAGES).contains(&request.passages.len()) {
            return Err(BrainError::KnowledgeValidation(
                "请选择要整理的片段（最多 16 段）".into(),
            ));
        }
        request.passages.sort_unstable();
        request.passages.dedup();
        let (material, source_citations) =
            self.read_writeback_material(base_id, &request.run_id)?;
        let original =
            self.store
                .topic_revision(base_id, page_id, &request.expected_revision_id)?;
        let (_, original_citations) = self.writeback_citation_keys(base_id, &original.run_id)?;
        let mut citations = DraftCitations::default();
        let mut markdown = citations.remap(
            original.markdown.trim_end(),
            &original.run_id,
            &original_citations,
        )?;
        for index in &request.passages {
            let passage = material
                .passages
                .iter()
                .find(|passage| &passage.index == index)
                .ok_or_else(|| {
                    BrainError::KnowledgeValidation("所选片段没有可核对的原文引用".into())
                })?;
            let body = citations.remap(&passage.markdown, &request.run_id, &source_citations)?;
            markdown.push_str("\n\n");
            let has_opening_heading = source_structure::parse_document(&body)
                .headings
                .first()
                .is_some_and(|heading| heading.start == 0 && heading.level <= 2);
            if !has_opening_heading {
                let title = material
                    .title
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                markdown.push_str(match material.kind {
                    TopicWritebackKind::Answer => "## 问答补充 · ",
                    TopicWritebackKind::Report => "## 研究补充 · ",
                });
                markdown.extend(
                    title
                        .chars()
                        .filter(|character| !r#"[]<>#`*_\"#.contains(*character))
                        .take(100),
                );
                markdown.push_str("\n\n");
            }
            markdown.push_str(&body);
        }
        if markdown.chars().count() > PAGE_CHARS {
            return Err(BrainError::KnowledgeValidation(
                "整理后的主题页超过 12000 字，请减少选中的片段".into(),
            ));
        }
        validate_topic_output(&markdown, citations.numbers.len(), true)?;
        if !citation_ranges(&markdown)
            .iter()
            .map(|(_, index)| index)
            .eq(citations.occurrences.iter())
        {
            return Err(BrainError::KnowledgeValidation(
                "合并后的 Markdown 改变了引用，请换一个片段".into(),
            ));
        }
        self.store.save_topic_writeback_draft(
            base_id,
            page_id,
            &request,
            &markdown,
            &citations.origins,
        )
    }

    fn read_writeback_material(
        &self,
        base_id: &str,
        run_id: &str,
    ) -> Result<(TopicWritebackMaterial, CitationKeys), BrainError> {
        let (kind, title) = self.store.topic_writeback_origin(base_id, run_id)?;
        let run = self.store.get_agent_run(run_id)?;
        let body = run
            .output
            .as_ref()
            .and_then(|output| output.get("answer"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if body.trim().is_empty() {
            return Err(BrainError::KnowledgeValidation(
                "没有找到已完整保存的正文".into(),
            ));
        }
        let (evidence_count, citations) = self.writeback_citation_keys(base_id, run_id)?;
        let passages = split_topic_sections(body)
            .into_iter()
            .enumerate()
            .filter_map(|(index, section)| {
                let cited = citation_numbers(&section.markdown);
                (!cited.is_empty() && cited.iter().all(|number| citations.contains_key(number)))
                    .then_some(TopicWritebackPassage {
                        index,
                        title: section.heading,
                        markdown: section.markdown,
                    })
            })
            .collect();
        Ok((
            TopicWritebackMaterial {
                title,
                kind,
                evidence_count,
                passages,
            },
            citations,
        ))
    }

    fn writeback_citation_keys(
        &self,
        base_id: &str,
        run_id: &str,
    ) -> Result<(usize, CitationKeys), BrainError> {
        let citations = self.store.list_agent_run_citations(run_id)?;
        let count = citations.len();
        let keys = citations
            .into_iter()
            .filter(|citation| {
                citation.kind == "source_span"
                    && citation.entry.knowledge_base_id == base_id
                    && !citation.read_ranges.is_empty()
            })
            .map(|citation| {
                (
                    citation.citation_index,
                    (citation.object_id, citation.version_id),
                )
            })
            .collect();
        Ok((count, keys))
    }
}

#[derive(Default)]
struct DraftCitations {
    numbers: HashMap<(String, String), usize>,
    origins: Vec<TopicDraftCitation>,
    occurrences: Vec<usize>,
}

impl DraftCitations {
    fn remap(
        &mut self,
        markdown: &str,
        run_id: &str,
        keys: &CitationKeys,
    ) -> Result<String, BrainError> {
        let mut output = String::with_capacity(markdown.len());
        let mut cursor = 0;
        for (range, old) in citation_ranges(markdown) {
            let key = keys.get(&old).ok_or_else(|| {
                BrainError::KnowledgeValidation(format!("引用 [S{old}] 缺少本书的原文阅读记录"))
            })?;
            let next = self.numbers.len() + 1;
            let index = *self.numbers.entry(key.clone()).or_insert(next);
            self.occurrences.push(index);
            let origin = TopicDraftCitation {
                index,
                run_id: run_id.to_owned(),
                origin_index: old,
            };
            if !self.origins.contains(&origin) {
                self.origins.push(origin);
            }
            output.push_str(&markdown[cursor..range.start]);
            output.push_str(&format!("[S{index}]"));
            cursor = range.end;
        }
        output.push_str(&markdown[cursor..]);
        Ok(output)
    }
}

// Map rendered prose back to source offsets so citation edits leave fenced
// code, inline code and explicit links untouched.
fn citation_ranges(markdown: &str) -> Vec<(Range<usize>, usize)> {
    let mut result = Vec::new();
    let mut text = String::new();
    let mut ranges = Vec::new();
    let mut suppressed = 0usize;
    for (event, source) in Parser::new_ext(markdown, Options::ENABLE_TABLES).into_offset_iter() {
        if let Event::Text(ref value) = event {
            if suppressed == 0 {
                let start = text.len();
                text.push_str(value);
                ranges.push((start..text.len(), source));
            }
            continue;
        }
        collect_citation_ranges(&text, &ranges, &mut result);
        text.clear();
        ranges.clear();
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Image { .. }) => {
                suppressed += 1
            }
            Event::End(TagEnd::CodeBlock | TagEnd::Link | TagEnd::Image) => {
                suppressed = suppressed.saturating_sub(1);
            }
            _ => {}
        }
    }
    collect_citation_ranges(&text, &ranges, &mut result);
    result
}

fn collect_citation_ranges(
    text: &str,
    ranges: &[(Range<usize>, Range<usize>)],
    result: &mut Vec<(Range<usize>, usize)>,
) {
    for (start, end, index) in citation_markers(text) {
        let first = ranges.iter().find(|(display, _)| display.contains(&start));
        let last = ranges
            .iter()
            .find(|(display, _)| display.contains(&(end - 1)));
        if let (Some((display_start, raw_start)), Some((display_end, raw_end))) = (first, last) {
            // Entity references can be wider in the source than their display
            // text. The bracket is the entire rendered entity in that case.
            let from = raw_start.start + (start - display_start.start);
            let to = raw_end.end - (display_end.end - end);
            result.push((from..to, index));
        }
    }
}
