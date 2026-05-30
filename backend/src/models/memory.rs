use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single chunk of a note stored in the memory engine (vector + fulltext index).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryChunk {
    /// Unique identifier for this chunk.
    pub id: Uuid,
    /// Path of the source Obsidian note.
    pub note_path: String,
    /// Position of this chunk within the note (0-based).
    pub chunk_index: usize,
    /// Chunk text content.
    pub content: String,
    /// Heading breadcrumb trail (e.g. ["Intro", "Details"]).
    pub breadcrumb: Vec<String>,
    /// Tags inherited from the note frontmatter + inline tags.
    pub tags: Vec<String>,
    /// Title of the source note.
    pub note_title: String,
    /// Approximate token count for the chunk content.
    pub token_count: usize,
    /// Whether this chunk contains at least one code block.
    pub has_code_block: bool,
    /// 1-based line number where this chunk starts in the source note.
    pub line_start: usize,
    /// 1-based line number where this chunk ends in the source note.
    pub line_end: usize,
}

/// Aggregated statistics about the memory engine's indexed content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total number of indexed chunks.
    pub total_chunks: usize,
    /// Total number of indexed notes.
    pub total_notes: usize,
    /// All unique tags across indexed notes.
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_chunk_roundtrip() {
        let chunk = MemoryChunk {
            id: Uuid::new_v4(),
            note_path: "notes/rust-guide.md".to_string(),
            chunk_index: 0,
            content: "Rust is a systems programming language.".to_string(),
            breadcrumb: vec!["Introduction".to_string()],
            tags: vec!["rust".to_string(), "programming".to_string()],
            note_title: "Rust Guide".to_string(),
            token_count: 8,
            has_code_block: false,
            line_start: 1,
            line_end: 50,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: MemoryChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, chunk.id);
        assert_eq!(parsed.note_path, chunk.note_path);
        assert_eq!(parsed.chunk_index, chunk.chunk_index);
        assert_eq!(parsed.content, chunk.content);
        assert_eq!(parsed.breadcrumb, chunk.breadcrumb);
        assert_eq!(parsed.tags, chunk.tags);
        assert_eq!(parsed.note_title, chunk.note_title);
        assert_eq!(parsed.token_count, chunk.token_count);
        assert_eq!(parsed.has_code_block, chunk.has_code_block);
        assert_eq!(parsed.line_start, chunk.line_start);
        assert_eq!(parsed.line_end, chunk.line_end);
    }

    #[test]
    fn test_memory_chunk_with_code_block_roundtrip() {
        let chunk = MemoryChunk {
            id: Uuid::new_v4(),
            note_path: "code/example.md".to_string(),
            chunk_index: 2,
            content: "Here is a code sample:\n```rust\nfn main() {}\n```".to_string(),
            breadcrumb: vec!["Code".to_string(), "Examples".to_string()],
            tags: vec!["code".to_string()],
            note_title: "Code Examples".to_string(),
            token_count: 15,
            has_code_block: true,
            line_start: 10,
            line_end: 25,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: MemoryChunk = serde_json::from_str(&json).unwrap();
        assert!(parsed.has_code_block);
        assert_eq!(parsed.chunk_index, 2);
    }

    #[test]
    fn test_memory_chunk_empty_breadcrumb_and_tags() {
        let chunk = MemoryChunk {
            id: Uuid::new_v4(),
            note_path: "untitled.md".to_string(),
            chunk_index: 0,
            content: "Just a plain paragraph.".to_string(),
            breadcrumb: vec![],
            tags: vec![],
            note_title: "Untitled".to_string(),
            token_count: 5,
            has_code_block: false,
            line_start: 1,
            line_end: 3,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: MemoryChunk = serde_json::from_str(&json).unwrap();
        assert!(parsed.breadcrumb.is_empty());
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn test_memory_stats_roundtrip() {
        let stats = MemoryStats {
            total_chunks: 128,
            total_notes: 32,
            tags: vec![
                "rust".to_string(),
                "obsidian".to_string(),
                "project".to_string(),
            ],
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: MemoryStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_chunks, stats.total_chunks);
        assert_eq!(parsed.total_notes, stats.total_notes);
        assert_eq!(parsed.tags, stats.tags);
    }

    #[test]
    fn test_memory_stats_empty_tags() {
        let stats = MemoryStats {
            total_chunks: 0,
            total_notes: 0,
            tags: vec![],
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: MemoryStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_chunks, 0);
        assert!(parsed.tags.is_empty());
    }
}
