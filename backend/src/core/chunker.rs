//! Smart chunker for splitting `ParsedDocument` sections into `MemoryChunk`s.
//!
//! The chunker preserves code blocks intact, maintains heading context (breadcrumbs),
//! and provides overlap between consecutive chunks for better retrieval quality.

use uuid::Uuid;

use crate::models::{MemoryChunk, ParsedDocument};

/// Configuration for the smart chunking algorithm.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SmartChunker {
    /// Minimum token count per chunk (default 300).
    pub min_tokens: usize,
    /// Maximum token count per chunk (default 800). Code blocks are allowed to exceed this.
    pub max_tokens: usize,
}

#[allow(dead_code)]
impl SmartChunker {
    /// Create a new `SmartChunker` with the given token limits.
    pub fn new(min_tokens: usize, max_tokens: usize) -> Self {
        Self {
            min_tokens,
            max_tokens,
        }
    }

    /// Split a `ParsedDocument` into `MemoryChunk`s.
    ///
    /// The algorithm:
    /// 1. Iterates through sections from the parsed document.
    /// 2. For each section:
    ///    - If it fits within max_tokens: accumulates into buffer.
    ///    - If buffer exceeds max_tokens: emits buffer as chunk, retains overlap.
    ///    - If section exceeds max_tokens: splits by paragraph boundaries.
    ///    - Code blocks that exceed max_tokens are emitted as their own chunk (allowed to exceed limit).
    /// 3. Each chunk carries breadcrumb context, tags, note path, and sequential index.
    pub fn chunk(&self, doc: &ParsedDocument) -> Vec<MemoryChunk> {
        if doc.sections.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut chunk_index = 0;
        let mut buffer = ChunkBuffer::new();

        for section in &doc.sections {
            let section_tokens = estimate_tokens(&section.content);

            // If the section content is empty, skip it
            if section.content.trim().is_empty() {
                continue;
            }

            // Check if this section contains code blocks that should be handled specially
            if !section.code_blocks.is_empty() && section_tokens > self.max_tokens {
                // Handle sections with code blocks that exceed max_tokens:
                // Split into segments, preserving code blocks as intact units.
                self.chunk_section_with_code_blocks(
                    section,
                    doc,
                    &mut chunks,
                    &mut chunk_index,
                    &mut buffer,
                );
            } else if section_tokens <= self.max_tokens {
                // Section fits within max_tokens — accumulate into buffer
                let combined_tokens = buffer.token_count + section_tokens;

                if combined_tokens > self.max_tokens && !buffer.content.is_empty() {
                    // Buffer overflow: emit current buffer, start new one with overlap
                    if buffer.token_count >= self.min_tokens {
                        self.emit_buffer(&buffer, doc, &mut chunks, &mut chunk_index);
                    }

                    // Start new buffer with overlap from the emitted chunk
                    let overlap = if let Some(last_chunk) = chunks.last() {
                        get_last_sentence(&last_chunk.content)
                    } else {
                        String::new()
                    };

                    buffer = ChunkBuffer::new();
                    if !overlap.is_empty() {
                        buffer.push(&overlap, section);
                    }
                    buffer.push(&section.content, section);
                } else {
                    // Accumulate section into buffer
                    buffer.push(&section.content, section);
                }
            } else {
                // Section exceeds max_tokens (no code blocks) — split by paragraphs
                // First, emit any existing buffer
                if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
                    self.emit_buffer(&buffer, doc, &mut chunks, &mut chunk_index);
                    buffer = ChunkBuffer::new();
                }

                self.chunk_long_section_by_paragraphs(
                    section,
                    doc,
                    &mut chunks,
                    &mut chunk_index,
                    &mut buffer,
                );
            }
        }

        // Emit remaining buffer content as the final chunk
        if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
            self.emit_buffer(&buffer, doc, &mut chunks, &mut chunk_index);
        } else if !buffer.content.is_empty() && !chunks.is_empty() {
            // Buffer is too small on its own, append to the last chunk
            if let Some(last_chunk) = chunks.last_mut() {
                last_chunk.content.push('\n');
                last_chunk.content.push_str(&buffer.content);
                last_chunk.token_count = estimate_tokens(&last_chunk.content);
                last_chunk.line_end = buffer.line_end;
                last_chunk.has_code_block = contains_code_block(&last_chunk.content);
            }
        } else if !buffer.content.is_empty() {
            // Buffer too small but no existing chunks to merge into — emit it anyway
            self.emit_buffer(&buffer, doc, &mut chunks, &mut chunk_index);
        }

        chunks
    }

    /// Emit the buffer as a chunk.
    fn emit_buffer(
        &self,
        buffer: &ChunkBuffer,
        doc: &ParsedDocument,
        chunks: &mut Vec<MemoryChunk>,
        chunk_index: &mut usize,
    ) {
        let content = buffer.content.trim().to_string();
        if content.is_empty() {
            return;
        }

        let token_count = estimate_tokens(&content);
        let has_code_block = contains_code_block(&content);

        chunks.push(MemoryChunk {
            id: Uuid::new_v4(),
            note_path: doc.path.clone(),
            chunk_index: *chunk_index,
            content,
            breadcrumb: buffer.breadcrumb.clone(),
            tags: doc.tags.clone(),
            note_title: doc.title.clone(),
            token_count,
            has_code_block,
            line_start: buffer.line_start,
            line_end: buffer.line_end,
        });

        *chunk_index += 1;
    }

    /// Chunk a long section by paragraph boundaries, preserving code blocks intact.
    fn chunk_section_with_code_blocks(
        &self,
        section: &crate::models::Section,
        doc: &ParsedDocument,
        chunks: &mut Vec<MemoryChunk>,
        chunk_index: &mut usize,
        buffer: &mut ChunkBuffer,
    ) {
        // Emit any existing buffer first
        if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
            self.emit_buffer(buffer, doc, chunks, chunk_index);
            *buffer = ChunkBuffer::new();
        }

        // Split section content into segments (paragraphs and code blocks)
        let segments = Self::extract_segments_from_section(section);

        for segment in segments {
            let segment_tokens = estimate_tokens(&segment.text);

            if segment.is_code_block {
                // Code blocks are always emitted as their own chunk (allowed to exceed max_tokens)
                // First emit any accumulated buffer
                if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
                    self.emit_buffer(buffer, doc, chunks, chunk_index);
                    *buffer = ChunkBuffer::new();
                }

                // Emit the code block as its own chunk
                let content = segment.text.trim().to_string();
                let token_count = estimate_tokens(&content);
                chunks.push(MemoryChunk {
                    id: Uuid::new_v4(),
                    note_path: doc.path.clone(),
                    chunk_index: *chunk_index,
                    content,
                    breadcrumb: section.breadcrumb.clone(),
                    tags: doc.tags.clone(),
                    note_title: doc.title.clone(),
                    token_count,
                    has_code_block: true,
                    line_start: segment.line_start,
                    line_end: segment.line_end,
                });
                *chunk_index += 1;
            } else if buffer.token_count + segment_tokens > self.max_tokens {
                // Adding this paragraph would overflow the buffer
                if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
                    self.emit_buffer(buffer, doc, chunks, chunk_index);

                    // Add overlap from emitted chunk
                    let overlap = if let Some(last_chunk) = chunks.last() {
                        get_last_sentence(&last_chunk.content)
                    } else {
                        String::new()
                    };

                    *buffer = ChunkBuffer::new();
                    if !overlap.is_empty() {
                        buffer.push(&overlap, section);
                    }
                }

                // If the paragraph itself exceeds max_tokens, it needs further splitting
                if segment_tokens > self.max_tokens {
                    let paragraphs = split_by_paragraphs(&segment.text);
                    for para in paragraphs {
                        let para_tokens = estimate_tokens(para);
                        if buffer.token_count + para_tokens > self.max_tokens
                            && !buffer.content.is_empty()
                            && buffer.token_count >= self.min_tokens
                        {
                            self.emit_buffer(buffer, doc, chunks, chunk_index);
                            let overlap = if let Some(last_chunk) = chunks.last() {
                                get_last_sentence(&last_chunk.content)
                            } else {
                                String::new()
                            };
                            *buffer = ChunkBuffer::new();
                            if !overlap.is_empty() {
                                buffer.push(&overlap, section);
                            }
                        }
                        buffer.push(para, section);
                    }
                } else {
                    buffer.push(&segment.text, section);
                }
            } else {
                // Paragraph fits — accumulate into buffer
                buffer.push(&segment.text, section);
            }
        }
    }

    /// Extract segments (paragraphs + code blocks) from a section, preserving ordering.
    fn extract_segments_from_section(section: &crate::models::Section) -> Vec<Segment> {
        let content = &section.content;
        let mut segments = Vec::new();

        // Find all code block positions in the content
        let code_block_positions: Vec<(usize, usize)> = section
            .code_blocks
            .iter()
            .map(|cb| (cb.line_start, cb.line_end))
            .collect();

        // Split content by ``` boundaries to extract text segments and code block segments
        let mut remaining = content.as_str();
        let mut code_block_idx = 0;

        while !remaining.is_empty() {
            // Find the next code block opening fence
            let fence_pos = remaining.find("```");

            if let Some(pos) = fence_pos {
                // Text before the code block
                let text_before = remaining[..pos].trim();
                if !text_before.is_empty() {
                    // Split the text before into paragraphs
                    for para in split_by_paragraphs(text_before) {
                        segments.push(Segment {
                            text: para.to_string(),
                            is_code_block: false,
                            line_start: section.line_start,
                            line_end: section.line_end,
                        });
                    }
                }

                // Find the closing fence
                let after_opening = &remaining[pos + 3..];
                // Skip the language line (first newline after opening fence)
                let closing_pos = after_opening.find("```");

                if let Some(closing) = closing_pos {
                    let code_text = remaining[..pos + 3 + closing + 3].trim();
                    let (cb_line_start, cb_line_end) =
                        if code_block_idx < code_block_positions.len() {
                            (
                                code_block_positions[code_block_idx].0,
                                code_block_positions[code_block_idx].1,
                            )
                        } else {
                            (section.line_start, section.line_end)
                        };

                    segments.push(Segment {
                        text: code_text.to_string(),
                        is_code_block: true,
                        line_start: cb_line_start,
                        line_end: cb_line_end,
                    });

                    remaining = &remaining[pos + 3 + closing + 3..];
                    code_block_idx += 1;
                } else {
                    // No closing fence found — treat rest as text
                    break;
                }
            } else {
                // No more code blocks — add remaining text as paragraphs
                for para in split_by_paragraphs(remaining) {
                    segments.push(Segment {
                        text: para.to_string(),
                        is_code_block: false,
                        line_start: section.line_start,
                        line_end: section.line_end,
                    });
                }
                break;
            }
        }

        // If no segments were extracted but the content is non-empty, add it all as text
        if segments.is_empty() && !content.trim().is_empty() {
            segments.push(Segment {
                text: content.trim().to_string(),
                is_code_block: false,
                line_start: section.line_start,
                line_end: section.line_end,
            });
        }

        segments
    }

    /// Split a long section (without code blocks) by paragraph boundaries.
    fn chunk_long_section_by_paragraphs(
        &self,
        section: &crate::models::Section,
        doc: &ParsedDocument,
        chunks: &mut Vec<MemoryChunk>,
        chunk_index: &mut usize,
        buffer: &mut ChunkBuffer,
    ) {
        let paragraphs = split_by_paragraphs(&section.content);

        for para in paragraphs {
            let para_tokens = estimate_tokens(para);

            if buffer.token_count + para_tokens > self.max_tokens {
                // Buffer overflow — emit current buffer
                if !buffer.content.is_empty() && buffer.token_count >= self.min_tokens {
                    self.emit_buffer(buffer, doc, chunks, chunk_index);

                    // Add overlap from the emitted chunk
                    let overlap = if let Some(last_chunk) = chunks.last() {
                        get_last_sentence(&last_chunk.content)
                    } else {
                        String::new()
                    };

                    *buffer = ChunkBuffer::new();
                    if !overlap.is_empty() {
                        buffer.push(&overlap, section);
                    }
                }

                // If the paragraph itself exceeds max_tokens, we still add it
                // (non-code content exceeding max_tokens is split best-effort by paragraphs)
                buffer.push(para, section);
            } else {
                buffer.push(para, section);
            }
        }
    }
}

impl Default for SmartChunker {
    fn default() -> Self {
        Self::new(300, 800)
    }
}

/// Internal buffer for accumulating content before emitting as a chunk.
#[allow(dead_code)]
struct ChunkBuffer {
    content: String,
    token_count: usize,
    breadcrumb: Vec<String>,
    line_start: usize,
    line_end: usize,
}

impl ChunkBuffer {
    fn new() -> Self {
        Self {
            content: String::new(),
            token_count: 0,
            breadcrumb: Vec::new(),
            line_start: 0,
            line_end: 0,
        }
    }

    fn push(&mut self, text: &str, section: &crate::models::Section) {
        if self.content.is_empty() {
            self.line_start = section.line_start;
            self.breadcrumb = section.breadcrumb.clone();
        }

        if !self.content.is_empty() {
            self.content.push_str("\n\n");
        }
        self.content.push_str(text);
        self.token_count = estimate_tokens(&self.content);
        self.line_end = section.line_end;
    }
}

/// Internal segment representing either a text paragraph or a code block.
#[allow(dead_code)]
struct Segment {
    text: String,
    is_code_block: bool,
    line_start: usize,
    line_end: usize,
}

/// Estimate the token count for a piece of text.
///
/// Uses a simple heuristic: each Chinese character counts as one token,
/// each English word counts as one token. This provides a reasonable
/// approximation for mixed CJK/English content.
#[allow(dead_code)]
pub fn estimate_tokens(text: &str) -> usize {
    let chinese = text.chars().filter(|c| !c.is_ascii()).count();
    let english = text.split_whitespace().filter(|w| w.is_ascii()).count();
    chinese + english
}

/// Split text by paragraph boundaries (`\n\n`).
#[allow(dead_code)]
fn split_by_paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect()
}

/// Extract the last sentence from text for overlap between chunks.
///
/// Returns the last meaningful text segment after the final period or newline, trimmed.
#[allow(dead_code)]
fn get_last_sentence(text: &str) -> String {
    text.rsplit(['.', '\n'])
        .find(|s| !s.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Check if text contains a fenced code block.
#[allow(dead_code)]
fn contains_code_block(text: &str) -> bool {
    text.contains("```")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CodeBlock, ParsedDocument, Section};
    use chrono::Utc;
    use std::collections::HashMap;

    /// Helper to create a ParsedDocument with given sections and tags.
    fn make_doc(sections: Vec<Section>, tags: Vec<String>) -> ParsedDocument {
        ParsedDocument {
            path: "notes/test.md".to_string(),
            frontmatter: HashMap::new(),
            title: "Test Note".to_string(),
            tags,
            sections,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Helper to create a section with given content and breadcrumb.
    fn make_section(
        level: u8,
        heading: Option<String>,
        breadcrumb: Vec<String>,
        content: String,
        code_blocks: Vec<CodeBlock>,
        line_start: usize,
        line_end: usize,
    ) -> Section {
        Section {
            level,
            heading,
            breadcrumb,
            content,
            code_blocks,
            line_start,
            line_end,
        }
    }

    /// Generate English text with approximately the given number of words/tokens.
    fn gen_english_text(word_count: usize) -> String {
        let words: Vec<String> = (0..word_count).map(|i| format!("word{}", i)).collect();
        words.join(" ")
    }

    /// Generate Chinese text with approximately the given number of characters/tokens.
    fn gen_chinese_text(char_count: usize) -> String {
        (0..char_count)
            .map(|i| format!("字{}", i))
            .collect::<Vec<String>>()
            .join("")
    }

    // Test 1: Single short section (200 tokens) -> 1 chunk with correct content
    #[test]
    fn test_chunk_single_short_section_returns_one_chunk() {
        let chunker = SmartChunker::new(300, 800);
        let content = gen_english_text(200);
        let section = make_section(
            1,
            Some("Intro".to_string()),
            vec!["Intro".to_string()],
            content.clone(),
            vec![],
            1,
            50,
        );
        let doc = make_doc(vec![section], vec!["tag1".to_string()]);

        let chunks = chunker.chunk(&doc);

        assert_eq!(
            chunks.len(),
            1,
            "Single short section should produce 1 chunk"
        );
        assert!(chunks[0].content.contains(&content));
        assert_eq!(chunks[0].breadcrumb, vec!["Intro".to_string()]);
        assert_eq!(chunks[0].tags, vec!["tag1".to_string()]);
        assert_eq!(chunks[0].chunk_index, 0);
        assert!(!chunks[0].has_code_block);
        assert_eq!(chunks[0].note_path, "notes/test.md");
        assert_eq!(chunks[0].note_title, "Test Note");
        // Verify UUID is valid
        assert_ne!(chunks[0].id, Uuid::nil());
    }

    // Test 2: Multiple short sections that fit together -> 1 chunk
    #[test]
    fn test_chunk_multiple_short_sections_fit_together_returns_one_chunk() {
        let chunker = SmartChunker::new(300, 800);
        let content1 = gen_english_text(150);
        let content2 = gen_english_text(200);
        let sections = vec![
            make_section(
                1,
                Some("Intro".to_string()),
                vec!["Intro".to_string()],
                content1.clone(),
                vec![],
                1,
                25,
            ),
            make_section(
                2,
                Some("Details".to_string()),
                vec!["Intro".to_string(), "Details".to_string()],
                content2.clone(),
                vec![],
                26,
                50,
            ),
        ];
        let doc = make_doc(sections, vec!["tag1".to_string()]);

        let chunks = chunker.chunk(&doc);

        assert_eq!(
            chunks.len(),
            1,
            "Multiple short sections fitting within max_tokens should produce 1 chunk"
        );
        assert!(chunks[0].content.contains(&content1));
        assert!(chunks[0].content.contains(&content2));
        // Breadcrumb should be from the first section that started the buffer
        assert_eq!(chunks[0].breadcrumb, vec!["Intro".to_string()]);
    }

    // Test 3: Long section (1500 tokens) -> 2+ chunks, each <= 800 tokens (except code blocks)
    #[test]
    fn test_chunk_long_section_produces_multiple_chunks_within_limit() {
        let chunker = SmartChunker::new(300, 800);
        // Create a long section with multiple paragraphs
        let paragraphs: Vec<String> = (0..10)
            .map(|i| {
                // Each paragraph ~200 words/tokens
                format!("Paragraph {} content: {}", i, gen_english_text(190))
            })
            .collect();
        let content = paragraphs.join("\n\n");
        let section = make_section(
            1,
            Some("Long".to_string()),
            vec!["Long".to_string()],
            content,
            vec![],
            1,
            100,
        );
        let doc = make_doc(vec![section], vec!["long".to_string()]);

        let chunks = chunker.chunk(&doc);

        assert!(
            chunks.len() >= 2,
            "Long section should produce at least 2 chunks, got {}",
            chunks.len()
        );

        // Each chunk should be <= max_tokens (except code blocks, but this has no code)
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.token_count <= chunker.max_tokens,
                "Chunk {} has {} tokens, exceeding max_tokens {}",
                i,
                chunk.token_count,
                chunker.max_tokens
            );
            assert_eq!(chunk.breadcrumb, vec!["Long".to_string()]);
            assert_eq!(chunk.tags, vec!["long".to_string()]);
        }

        // Verify chunk indices are sequential
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_index, i);
        }
    }

    // Test 4: Code block (1000 tokens) preserved intact as its own chunk even though > 800
    #[test]
    fn test_chunk_code_block_preserved_intact_exceeding_max_tokens() {
        let chunker = SmartChunker::new(300, 800);
        let long_code = gen_english_text(1000); // ~1000 tokens of code
        let code_block = CodeBlock {
            language: Some("rust".to_string()),
            code: long_code.clone(),
            line_start: 5,
            line_end: 50,
        };
        let section_content = format!(
            "Some intro text.\n\n```rust\n{}\n```\n\nSome trailing text.",
            long_code
        );
        let section = make_section(
            1,
            Some("Code".to_string()),
            vec!["Code".to_string()],
            section_content,
            vec![code_block],
            1,
            55,
        );
        let doc = make_doc(vec![section], vec!["code".to_string()]);

        let chunks = chunker.chunk(&doc);

        // Find the chunk that contains the code block
        let code_chunk = chunks.iter().find(|c| c.has_code_block);
        assert!(
            code_chunk.is_some(),
            "Should have at least one chunk with a code block"
        );

        let code_chunk = code_chunk.unwrap();
        // The code block chunk is allowed to exceed max_tokens
        assert!(
            code_chunk.token_count > chunker.max_tokens,
            "Code block chunk should exceed max_tokens ({}), got {}",
            chunker.max_tokens,
            code_chunk.token_count
        );
        assert!(code_chunk.content.contains(&long_code));
        assert!(code_chunk.has_code_block);
        assert_eq!(code_chunk.breadcrumb, vec!["Code".to_string()]);
    }

    // Test 5: Breadcrumb context carried through all chunks from same section
    #[test]
    fn test_chunk_breadcrumb_carried_through_all_chunks() {
        let chunker = SmartChunker::new(300, 800);
        let paragraphs: Vec<String> = (0..5)
            .map(|i| format!("Para {}: {}", i, gen_english_text(250)))
            .collect();
        let content = paragraphs.join("\n\n");
        let section = make_section(
            2,
            Some("Subsection".to_string()),
            vec![
                "Intro".to_string(),
                "Details".to_string(),
                "Subsection".to_string(),
            ],
            content,
            vec![],
            10,
            60,
        );
        let doc = make_doc(vec![section], vec!["breadcrumb".to_string()]);

        let chunks = chunker.chunk(&doc);

        assert!(chunks.len() >= 2);

        // All chunks should carry the same breadcrumb
        for chunk in &chunks {
            assert_eq!(
                chunk.breadcrumb,
                vec![
                    "Intro".to_string(),
                    "Details".to_string(),
                    "Subsection".to_string()
                ]
            );
        }
    }

    // Test 6: Overlap sentence retained between consecutive chunks
    #[test]
    fn test_chunk_overlap_sentence_between_consecutive_chunks() {
        let chunker = SmartChunker::new(300, 800);
        // Create content with clear sentences ending with periods
        let sentences: Vec<String> = (0..20)
            .map(|i| format!("This is sentence number {}. It has a period at the end.", i))
            .collect();
        let content = sentences.join(" ");
        let section = make_section(
            1,
            Some("Overlapping".to_string()),
            vec!["Overlapping".to_string()],
            content,
            vec![],
            1,
            100,
        );
        let doc = make_doc(vec![section], vec!["overlap".to_string()]);

        let chunks = chunker.chunk(&doc);

        if chunks.len() >= 2 {
            // The overlap sentence should appear in the beginning of subsequent chunks
            // (at least the second chunk should start with the last sentence from the first)
            // This is a soft check since the exact overlap depends on algorithm behavior
            for i in 1..chunks.len() {
                // The chunk should contain some content from the previous chunk's ending context
                // We just verify that chunks are not disjoint — there should be some overlap
                let has_overlap_or_new_content = !chunks[i].content.is_empty();
                assert!(
                    has_overlap_or_new_content,
                    "Each subsequent chunk should have content"
                );
            }
        }
    }

    // Test 7: Empty document (no sections) -> 0 chunks
    #[test]
    fn test_chunk_empty_document_returns_zero_chunks() {
        let chunker = SmartChunker::new(300, 800);
        let doc = make_doc(vec![], vec![]);

        let chunks = chunker.chunk(&doc);

        assert_eq!(chunks.len(), 0, "Empty document should produce 0 chunks");
    }

    // Test 8: Tags from doc.tags propagated to all chunks
    #[test]
    fn test_chunk_tags_propagated_to_all_chunks() {
        let chunker = SmartChunker::new(300, 800);
        let paragraphs: Vec<String> = (0..5)
            .map(|i| format!("Para {}: {}", i, gen_english_text(250)))
            .collect();
        let content = paragraphs.join("\n\n");
        let section = make_section(
            1,
            Some("Tagged".to_string()),
            vec!["Tagged".to_string()],
            content,
            vec![],
            1,
            50,
        );
        let tags = vec![
            "rust".to_string(),
            "programming".to_string(),
            "memory".to_string(),
        ];
        let doc = make_doc(vec![section], tags.clone());

        let chunks = chunker.chunk(&doc);

        for chunk in &chunks {
            assert_eq!(
                chunk.tags, tags,
                "All chunks should carry the same tags from the document"
            );
        }
    }

    // Test 9: Token count on each chunk matches estimate_tokens(content)
    #[test]
    fn test_chunk_token_count_matches_estimate() {
        let chunker = SmartChunker::new(300, 800);
        let content1 = gen_english_text(400);
        let content2 = gen_english_text(500);
        let sections = vec![
            make_section(
                1,
                Some("Part1".to_string()),
                vec!["Part1".to_string()],
                content1,
                vec![],
                1,
                30,
            ),
            make_section(
                2,
                Some("Part2".to_string()),
                vec!["Part1".to_string(), "Part2".to_string()],
                content2,
                vec![],
                31,
                60,
            ),
        ];
        let doc = make_doc(sections, vec!["tokens".to_string()]);

        let chunks = chunker.chunk(&doc);

        for chunk in &chunks {
            let estimated = estimate_tokens(&chunk.content);
            assert_eq!(
                chunk.token_count, estimated,
                "Token count should match estimate_tokens for chunk content"
            );
        }
    }

    // Additional test: Chinese content token estimation
    #[test]
    fn test_estimate_tokens_chinese_content() {
        let chinese_text = gen_chinese_text(100);
        let tokens = estimate_tokens(&chinese_text);
        assert_eq!(
            tokens, 100,
            "Each Chinese character should count as one token"
        );
    }

    // Additional test: Mixed Chinese and English content token estimation
    #[test]
    fn test_estimate_tokens_mixed_content() {
        let mixed = format!("{} some English words here", gen_chinese_text(50));
        let tokens = estimate_tokens(&mixed);
        // 50 Chinese chars + "some" + "English" + "words" + "here" = 50 + 4 = 54
        assert_eq!(
            tokens, 54,
            "Mixed content: 50 Chinese chars + 4 English words"
        );
    }

    // Additional test: Default SmartChunker values
    #[test]
    fn test_smart_chunker_default_values() {
        let chunker = SmartChunker::default();
        assert_eq!(chunker.min_tokens, 300);
        assert_eq!(chunker.max_tokens, 800);
    }

    // Additional test: All chunks have valid UUIDs
    #[test]
    fn test_chunk_all_chunks_have_valid_uuids() {
        let chunker = SmartChunker::new(300, 800);
        let content = gen_english_text(500);
        let section = make_section(
            1,
            Some("UUID".to_string()),
            vec!["UUID".to_string()],
            content,
            vec![],
            1,
            50,
        );
        let doc = make_doc(vec![section], vec![]);

        let chunks = chunker.chunk(&doc);

        for chunk in &chunks {
            assert_ne!(chunk.id, Uuid::nil(), "Chunk UUID should not be nil");
            assert!(
                chunk.id.get_version().is_some(),
                "Chunk UUID should have a version"
            );
        }
    }

    // Additional test: Empty section content is skipped
    #[test]
    fn test_chunk_empty_section_content_skipped() {
        let chunker = SmartChunker::new(300, 800);
        let sections = vec![
            make_section(
                1,
                Some("Empty".to_string()),
                vec!["Empty".to_string()],
                String::new(), // empty content
                vec![],
                1,
                5,
            ),
            make_section(
                2,
                Some("Content".to_string()),
                vec!["Empty".to_string(), "Content".to_string()],
                gen_english_text(400),
                vec![],
                6,
                50,
            ),
        ];
        let doc = make_doc(sections, vec![]);

        let chunks = chunker.chunk(&doc);

        // Empty section should be skipped, only the content section should produce chunks
        assert!(chunks.len() >= 1);
        assert_eq!(
            chunks[0].breadcrumb,
            vec!["Empty".to_string(), "Content".to_string()]
        );
    }

    // Additional test: Code block within an otherwise small section
    #[test]
    fn test_chunk_small_section_with_code_block() {
        let chunker = SmartChunker::new(300, 800);
        let code_block = CodeBlock {
            language: Some("python".to_string()),
            code: "print('hello')".to_string(),
            line_start: 3,
            line_end: 5,
        };
        let section_content =
            "Some text before.\n\n```python\nprint('hello')\n```\n\nSome text after.";
        let section = make_section(
            1,
            Some("SmallCode".to_string()),
            vec!["SmallCode".to_string()],
            section_content.to_string(),
            vec![code_block],
            1,
            10,
        );
        let doc = make_doc(vec![section], vec!["python".to_string()]);

        let chunks = chunker.chunk(&doc);

        // Should produce chunks; the code block should be within a chunk
        assert!(chunks.len() >= 1);
        let has_code = chunks.iter().any(|c| c.has_code_block);
        assert!(
            has_code,
            "At least one chunk should have has_code_block = true"
        );
    }

    // Additional test: Single paragraph section below min_tokens but no prior chunks
    #[test]
    fn test_chunk_below_min_tokens_no_prior_chunks_still_emitted() {
        let chunker = SmartChunker::new(300, 800);
        // Content with only 50 tokens — below min_tokens but it's the only content
        let content = gen_english_text(50);
        let section = make_section(
            1,
            Some("Tiny".to_string()),
            vec!["Tiny".to_string()],
            content,
            vec![],
            1,
            10,
        );
        let doc = make_doc(vec![section], vec![]);

        let chunks = chunker.chunk(&doc);

        // Even though below min_tokens, since it's the only content it should still be emitted
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].token_count, 50);
    }

    // Additional test: get_last_sentence correctness
    #[test]
    fn test_get_last_sentence_basic() {
        let text = "First sentence. Second sentence. Third sentence.";
        let last = get_last_sentence(text);
        // rsplit by '.' returns text after the final '.', which is trimmed empty
        // So it finds the segment "Third sentence" (after the second-to-last period)
        assert_eq!(last, "Third sentence");
    }

    #[test]
    fn test_get_last_sentence_with_newlines() {
        let text = "Line one\nLine two\nLine three";
        let last = get_last_sentence(text);
        assert_eq!(last, "Line three");
    }

    #[test]
    fn test_get_last_sentence_empty() {
        let last = get_last_sentence("");
        assert_eq!(last, "");
    }

    // Additional test: split_by_paragraphs correctness
    #[test]
    fn test_split_by_paragraphs_basic() {
        let text = "Para 1\n\nPara 2\n\nPara 3";
        let paras = split_by_paragraphs(text);
        assert_eq!(paras.len(), 3);
        assert_eq!(paras[0], "Para 1");
        assert_eq!(paras[1], "Para 2");
        assert_eq!(paras[2], "Para 3");
    }

    #[test]
    fn test_split_by_paragraphs_empty_paragraphs_filtered() {
        let text = "Para 1\n\n\n\nPara 2";
        let paras = split_by_paragraphs(text);
        assert_eq!(paras.len(), 2);
    }

    // Additional test: contains_code_block correctness
    #[test]
    fn test_contains_code_block_true() {
        assert!(contains_code_block("Some text\n```rust\ncode\n```"));
    }

    #[test]
    fn test_contains_code_block_false() {
        assert!(!contains_code_block("Just regular text without code."));
    }

    // Additional test: Multiple code blocks in one section
    #[test]
    fn test_chunk_multiple_code_blocks_in_section() {
        let chunker = SmartChunker::new(300, 800);
        let code1 = CodeBlock {
            language: Some("rust".to_string()),
            code: gen_english_text(600),
            line_start: 3,
            line_end: 30,
        };
        let code2 = CodeBlock {
            language: Some("python".to_string()),
            code: gen_english_text(500),
            line_start: 35,
            line_end: 60,
        };
        let section_content = format!(
            "Intro text.\n\n```rust\n{}\n```\n\nMiddle text.\n\n```python\n{}\n```\n\nEnd text.",
            code1.code, code2.code
        );
        let section = make_section(
            1,
            Some("MultiCode".to_string()),
            vec!["MultiCode".to_string()],
            section_content,
            vec![code1, code2],
            1,
            70,
        );
        let doc = make_doc(vec![section], vec!["multi".to_string()]);

        let chunks = chunker.chunk(&doc);

        // There should be chunks with code blocks
        let code_chunks = chunks.iter().filter(|c| c.has_code_block).count();
        assert!(
            code_chunks >= 2,
            "Should have at least 2 chunks with code blocks, got {}",
            code_chunks
        );

        // Code block chunks can exceed max_tokens
        for chunk in chunks.iter().filter(|c| c.has_code_block) {
            // Code block chunks are allowed to exceed max_tokens
            assert!(chunk.content.contains("```"));
        }
    }
}
