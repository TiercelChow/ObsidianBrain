//! Search-related tool handlers: search_notes, get_note, list_recent_notes.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// Search notes using hybrid fulltext + semantic search (RRF fusion).
///
/// Returns note-level results with path, title, snippet, and Obsidian URI.
pub struct SearchNotesHandler;

#[async_trait]
impl ToolHandler for SearchNotesHandler {
    fn name(&self) -> &str {
        "search_notes"
    }

    fn description(&self) -> &str {
        "搜索 Obsidian 笔记，返回笔记级结果（混合全文+语义搜索）"
    }

    fn input_schema(&self) -> Value {
        definitions::search_notes_schema()
    }

    fn module(&self) -> &str {
        "search"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'query'".to_string()))?;

        let top_k = args
            .get("top_k")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(5);

        let tags: Option<Vec<String>> = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

        tracing::debug!(query = %query, top_k, "search_notes 调用");

        // Search with higher limit to allow aggregation by note_path.
        let results = ctx
            .search_engine
            .search(query, top_k * 3, tags.as_deref())
            .await?;

        // Aggregate by note_path: keep highest-scoring chunk per note.
        use std::collections::HashMap;
        let mut note_map: HashMap<&str, &crate::models::HybridSearchResult> = HashMap::new();
        for r in &results {
            note_map
                .entry(&r.note_path)
                .and_modify(|existing| {
                    if r.rrf_score > existing.rrf_score {
                        *existing = r;
                    }
                })
                .or_insert(r);
        }

        let mut notes: Vec<&crate::models::HybridSearchResult> = note_map.into_values().collect();
        notes.sort_by(|a, b| {
            b.rrf_score
                .partial_cmp(&a.rrf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        notes.truncate(top_k);

        let notes_json: Vec<Value> = notes
            .iter()
            .map(|r| {
                json!({
                    "path": r.note_path,
                    "title": r.note_title,
                    "snippet": r.content,
                    "score": r.rrf_score,
                    "obsidian_uri": r.obsidian_uri,
                })
            })
            .collect();

        tracing::debug!(total = notes_json.len(), "search_notes 返回结果");
        Ok(json!({ "notes": notes_json, "total": notes_json.len() }))
    }
}

/// Get a note's full content by its path relative to the vault root.
pub struct GetNoteHandler;

#[async_trait]
impl ToolHandler for GetNoteHandler {
    fn name(&self) -> &str {
        "get_note"
    }

    fn description(&self) -> &str {
        "获取笔记完整内容"
    }

    fn input_schema(&self) -> Value {
        definitions::get_note_schema()
    }

    fn module(&self) -> &str {
        "search"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'path'".to_string()))?;

        tracing::debug!(path = %path, "get_note 调用");

        let content = ctx.memory_service.get_note(path).await?;

        // Derive title from filename stem (consistent with MemoryService behavior).
        let title = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        tracing::debug!(path = %path, content_len = content.len(), "get_note 返回结果");
        Ok(json!({ "path": path, "content": content, "title": title }))
    }
}

/// List recently modified notes within a given time window.
pub struct ListRecentNotesHandler;

#[async_trait]
impl ToolHandler for ListRecentNotesHandler {
    fn name(&self) -> &str {
        "list_recent_notes"
    }

    fn description(&self) -> &str {
        "列出最近修改的笔记"
    }

    fn input_schema(&self) -> Value {
        definitions::list_recent_notes_schema()
    }

    fn module(&self) -> &str {
        "search"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let days = args.get("days").and_then(|v| v.as_u64()).map(|v| v as u32);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        tracing::debug!(days = ?days, limit = ?limit, "list_recent_notes 调用");

        let summaries = ctx.memory_service.list_recent_notes(days, limit).await?;

        let notes: Vec<Value> = summaries
            .iter()
            .map(|s| {
                json!({
                    "path": s.path,
                    "title": s.title,
                    "tags": s.tags,
                    "updated_at": s.updated_at.to_rfc3339()
                })
            })
            .collect();

        tracing::debug!(total = notes.len(), "list_recent_notes 返回结果");
        Ok(json!({ "notes": notes, "total": notes.len() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_vault_file(vault_path: &std::path::Path, relative_path: &str, content: &str) {
        let full_path = vault_path.join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, content).unwrap();
    }

    // ── Handler tests ──

    #[tokio::test]
    async fn test_search_notes_handler_returns_notes_structure() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // Add a memory so there's something to search for.
        ctx.memory_service
            .add_memory(
                "notes/test.md",
                "Rust async programming",
                Some(vec!["rust".to_string()]),
            )
            .await
            .unwrap();

        let handler = SearchNotesHandler;
        let args = json!({ "query": "Rust async" });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["notes"].is_array());
        assert!(result["total"].is_number());

        // Verify each note has the expected fields.
        if let Some(notes) = result["notes"].as_array() {
            for note in notes {
                assert!(note.get("path").is_some());
                assert!(note.get("title").is_some());
                assert!(note.get("snippet").is_some());
                assert!(note.get("score").is_some());
                assert!(note.get("obsidian_uri").is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_search_notes_handler_with_top_k_and_tags() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        ctx.memory_service
            .add_memory(
                "notes/rust.md",
                "Rust language details",
                Some(vec!["rust".to_string()]),
            )
            .await
            .unwrap();

        let handler = SearchNotesHandler;
        let args = json!({ "query": "Rust", "top_k": 3, "tags": ["rust"] });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["notes"].is_array());
        assert!(result["total"].as_u64().unwrap() <= 3);
    }

    #[tokio::test]
    async fn test_search_notes_handler_empty_query() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let handler = SearchNotesHandler;
        let args = json!({ "query": "nonexistent_xyzzy" });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["total"], 0);
        assert!(result["notes"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_notes_handler_missing_query_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let handler = SearchNotesHandler;
        let args = json!({});

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_note_handler_returns_content() {
        let (ctx, _dir, vault) = crate::AppContext::for_test();

        write_vault_file(&vault, "hello.md", "# Hello World\nThis is a test note.");

        let handler = GetNoteHandler;
        let args = json!({ "path": "hello.md" });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["path"], "hello.md");
        assert_eq!(result["title"], "hello");
        assert!(result["content"].as_str().unwrap().contains("Hello World"));
    }

    #[tokio::test]
    async fn test_get_note_handler_nonexistent_file() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let handler = GetNoteHandler;
        let args = json!({ "path": "nonexistent.md" });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_note_handler_missing_path_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let handler = GetNoteHandler;
        let args = json!({});

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_recent_notes_handler_returns_notes() {
        let (ctx, _dir, vault) = crate::AppContext::for_test();

        write_vault_file(
            &vault,
            "note1.md",
            "---\ntitle: First\ntags:\n  - test\n---\n# First\nContent.",
        );
        write_vault_file(
            &vault,
            "note2.md",
            "---\ntitle: Second\ntags:\n  - test\n---\n# Second\nContent.",
        );

        let handler = ListRecentNotesHandler;
        let args = json!({ "days": 7, "limit": 20 });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["notes"].is_array());
        let total = result["total"].as_u64().unwrap();
        assert!(total >= 2);

        // Verify each note has expected fields.
        if let Some(notes) = result["notes"].as_array() {
            for note in notes {
                assert!(note.get("path").is_some());
                assert!(note.get("title").is_some());
                assert!(note.get("tags").is_some());
                assert!(note.get("updated_at").is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_list_recent_notes_handler_default_args() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // No vault files, but the handler should still succeed with empty results.
        let handler = ListRecentNotesHandler;
        let args = json!({});

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["notes"].is_array());
    }

    // ── Schema / metadata tests ──

    #[test]
    fn test_search_notes_handler_metadata() {
        let handler = SearchNotesHandler;
        assert_eq!(handler.name(), "search_notes");
        assert_eq!(handler.module(), "search");
        assert!(handler.input_schema().is_object());
        assert!(handler.input_schema()["required"].is_array());
    }

    #[test]
    fn test_get_note_handler_metadata() {
        let handler = GetNoteHandler;
        assert_eq!(handler.name(), "get_note");
        assert_eq!(handler.module(), "search");
        assert!(handler.input_schema()["required"].is_array());
    }

    #[test]
    fn test_list_recent_notes_handler_metadata() {
        let handler = ListRecentNotesHandler;
        assert_eq!(handler.name(), "list_recent_notes");
        assert_eq!(handler.module(), "search");
        // No required fields — days and limit are optional.
        let schema = handler.input_schema();
        // When there are no required fields, the schema may omit the "required" key
        // or include it as an empty array. Both are valid JSON Schema.
        let required = schema.get("required").and_then(|v| v.as_array());
        match required {
            Some(arr) => assert!(arr.is_empty()),
            None => {} // "required" key omitted — valid, means no required params
        }
    }
}
