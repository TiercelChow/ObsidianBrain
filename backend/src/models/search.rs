use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single result from hybrid (fulltext + semantic) search, fused via RRF.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct HybridSearchResult {
    /// ID of the matching memory chunk.
    pub chunk_id: Uuid,
    /// Path of the source note.
    pub note_path: PathBuf,
    /// Title of the source note.
    pub note_title: String,
    /// Chunk content that matched the query.
    pub content: String,
    /// Breadcrumb trail as a single joined string (e.g. "Intro > Details > Sub").
    pub breadcrumb: String,
    /// Position of this chunk within the note (0-based).
    pub chunk_index: usize,
    /// Final RRF fusion score.
    pub rrf_score: f64,
    /// Rank in the fulltext (Tantivy) result list, `None` if only semantic match.
    pub fulltext_rank: Option<usize>,
    /// Raw fulltext relevance score, `None` if only semantic match.
    pub fulltext_score: Option<f32>,
    /// Rank in the semantic (Qdrant) result list, `None` if only fulltext match.
    pub semantic_rank: Option<usize>,
    /// Raw semantic similarity score, `None` if only fulltext match.
    pub semantic_score: Option<f32>,
    /// Obsidian URI for opening the note directly (e.g. "obsidian://open?vault=MyVault&file=note").
    pub obsidian_uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_search_result_roundtrip() {
        let result = HybridSearchResult {
            chunk_id: Uuid::new_v4(),
            note_path: PathBuf::from("notes/rust-guide.md"),
            note_title: "Rust Guide".to_string(),
            content: "Rust is a systems programming language.".to_string(),
            breadcrumb: "Introduction".to_string(),
            chunk_index: 0,
            rrf_score: 0.0325,
            fulltext_rank: Some(1),
            fulltext_score: Some(8.5),
            semantic_rank: Some(3),
            semantic_score: Some(0.92),
            obsidian_uri: "obsidian://open?vault=MyVault&file=rust-guide".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: HybridSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.chunk_id, result.chunk_id);
        assert_eq!(parsed.note_path, result.note_path);
        assert_eq!(parsed.note_title, result.note_title);
        assert_eq!(parsed.content, result.content);
        assert_eq!(parsed.breadcrumb, result.breadcrumb);
        assert_eq!(parsed.chunk_index, result.chunk_index);
        assert_eq!(parsed.rrf_score, result.rrf_score);
        assert_eq!(parsed.fulltext_rank, result.fulltext_rank);
        assert_eq!(parsed.fulltext_score, result.fulltext_score);
        assert_eq!(parsed.semantic_rank, result.semantic_rank);
        assert_eq!(parsed.semantic_score, result.semantic_score);
        assert_eq!(parsed.obsidian_uri, result.obsidian_uri);
    }

    #[test]
    fn test_hybrid_search_result_fulltext_only() {
        // Result matched only via fulltext search (no semantic hit).
        let result = HybridSearchResult {
            chunk_id: Uuid::new_v4(),
            note_path: PathBuf::from("notes/search.md"),
            note_title: "Search Guide".to_string(),
            content: "How to search.".to_string(),
            breadcrumb: "Search".to_string(),
            chunk_index: 1,
            rrf_score: 0.0167,
            fulltext_rank: Some(5),
            fulltext_score: Some(3.2),
            semantic_rank: None,
            semantic_score: None,
            obsidian_uri: "obsidian://open?vault=Vault&file=search".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: HybridSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.fulltext_rank, Some(5));
        assert!(parsed.semantic_rank.is_none());
        assert!(parsed.semantic_score.is_none());
    }

    #[test]
    fn test_hybrid_search_result_semantic_only() {
        // Result matched only via semantic search (no fulltext hit).
        let result = HybridSearchResult {
            chunk_id: Uuid::new_v4(),
            note_path: PathBuf::from("notes/concept.md"),
            note_title: "Concept Note".to_string(),
            content: "A conceptual overview.".to_string(),
            breadcrumb: "Concepts".to_string(),
            chunk_index: 0,
            rrf_score: 0.0159,
            fulltext_rank: None,
            fulltext_score: None,
            semantic_rank: Some(2),
            semantic_score: Some(0.85),
            obsidian_uri: "obsidian://open?vault=Vault&file=concept".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: HybridSearchResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.fulltext_rank.is_none());
        assert!(parsed.fulltext_score.is_none());
        assert_eq!(parsed.semantic_rank, Some(2));
    }
}
