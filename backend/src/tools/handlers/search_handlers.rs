//! Search-related tool handlers: search_notes, get_note, list_recent_notes.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// Search notes using Obsidian's native search API.
///
/// Returns note-level results with path, title, and score.
pub struct SearchNotesHandler;

#[async_trait]
impl ToolHandler for SearchNotesHandler {
    fn name(&self) -> &str {
        "search_notes"
    }

    fn description(&self) -> &str {
        "搜索 Obsidian 笔记，使用 Obsidian 原生搜索"
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

        let limit = args
            .get("top_k")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(5);

        tracing::debug!(query = %query, limit, "search_notes 调用");

        // Use Obsidian's native search
        let results = ctx.memory_service.search(query, limit).await?;

        let notes_json: Vec<Value> = results
            .iter()
            .map(|note| {
                json!({
                    "path": note.path,
                    "title": note.title,
                    "snippet": note.snippet,
                    "tags": note.tags,
                })
            })
            .collect();

        tracing::debug!(total = notes_json.len(), "search_notes 返回结果");
        Ok(json!({ "notes": notes_json, "total": notes_json.len() }))
    }
}

/// Get a note's full content by path.
pub struct GetNoteHandler;

#[async_trait]
impl ToolHandler for GetNoteHandler {
    fn name(&self) -> &str {
        "get_note"
    }

    fn description(&self) -> &str {
        "获取笔记的完整内容"
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

        let content = ctx.memory_service.read_note(path).await?;

        Ok(json!({
            "path": path,
            "content": content,
        }))
    }
}

/// List recently modified notes.
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

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        tracing::debug!("list_recent_notes 调用");

        // List all files from Obsidian
        let files = ctx.memory_service.list_files().await?;

        // Filter to .md files and take first 20
        let md_files: Vec<String> = files
            .into_iter()
            .filter(|f| f.ends_with(".md"))
            .take(20)
            .collect();

        let notes_json: Vec<Value> = md_files
            .iter()
            .map(|path| {
                let title = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                json!({
                    "path": path,
                    "title": title,
                })
            })
            .collect();

        Ok(json!({ "notes": notes_json, "total": notes_json.len() }))
    }
}
