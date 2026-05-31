//! Memory-related tool handlers: search_memory, add_memory, update_memory,
//! forget_memory, get_memory_stats.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// Search memory chunks using hybrid search (RRF fusion).
///
/// Returns chunk-level results with chunk_id, note_path, content, and Obsidian URI.
pub struct SearchMemoryHandler;

#[async_trait]
impl ToolHandler for SearchMemoryHandler {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "搜索记忆碎片，返回 chunk 级结果（混合全文+语义搜索）"
    }

    fn input_schema(&self) -> Value {
        definitions::search_memory_schema()
    }

    fn module(&self) -> &str {
        "memory"
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

        tracing::debug!(query = %query, top_k, "search_memory 调用");

        let results = ctx
            .search_engine
            .search(query, top_k, tags.as_deref())
            .await?;

        let chunks: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "chunk_id": r.chunk_id.to_string(),
                    "note_path": r.note_path,
                    "content": r.content,
                    "score": r.rrf_score,
                    "obsidian_uri": r.obsidian_uri
                })
            })
            .collect();

        tracing::debug!(total = chunks.len(), "search_memory 返回结果");
        Ok(json!({ "chunks": chunks, "total": chunks.len() }))
    }
}

/// Create a new memory chunk.
pub struct AddMemoryHandler;

#[async_trait]
impl ToolHandler for AddMemoryHandler {
    fn name(&self) -> &str {
        "add_memory"
    }

    fn description(&self) -> &str {
        "添加记忆碎片"
    }

    fn input_schema(&self) -> Value {
        definitions::add_memory_schema()
    }

    fn module(&self) -> &str {
        "memory"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let note_path = args
            .get("note_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'note_path'".to_string()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'content'".to_string()))?;

        let tags: Option<Vec<String>> = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

        tracing::debug!(note_path = %note_path, content_len = content.len(), "add_memory 调用");

        let memory_id = ctx
            .memory_service
            .add_memory(note_path, content, tags)
            .await?;

        tracing::debug!(memory_id = %memory_id, "add_memory 成功");
        Ok(json!({ "memory_id": memory_id.to_string(), "status": "created" }))
    }
}

/// Update an existing memory chunk's content.
pub struct UpdateMemoryHandler;

#[async_trait]
impl ToolHandler for UpdateMemoryHandler {
    fn name(&self) -> &str {
        "update_memory"
    }

    fn description(&self) -> &str {
        "更新记忆碎片内容"
    }

    fn input_schema(&self) -> Value {
        definitions::update_memory_schema()
    }

    fn module(&self) -> &str {
        "memory"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let memory_id_str = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");

        let memory_id = Uuid::parse_str(memory_id_str)
            .map_err(|e| BrainError::Internal(format!("无效的 UUID '{}': {e}", memory_id_str)))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'content'".to_string()))?;

        tracing::debug!(memory_id = %memory_id, content_len = content.len(), "update_memory 调用");

        ctx.memory_service.update_memory(memory_id, content).await?;

        tracing::debug!(memory_id = %memory_id, "update_memory 成功");
        Ok(json!({ "memory_id": memory_id.to_string(), "status": "updated" }))
    }
}

/// Delete a memory chunk.
pub struct ForgetMemoryHandler;

#[async_trait]
impl ToolHandler for ForgetMemoryHandler {
    fn name(&self) -> &str {
        "forget_memory"
    }

    fn description(&self) -> &str {
        "删除记忆碎片"
    }

    fn input_schema(&self) -> Value {
        definitions::forget_memory_schema()
    }

    fn module(&self) -> &str {
        "memory"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let memory_id_str = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");

        let memory_id = Uuid::parse_str(memory_id_str)
            .map_err(|e| BrainError::Internal(format!("无效的 UUID '{}': {e}", memory_id_str)))?;

        tracing::debug!(memory_id = %memory_id, "forget_memory 调用");

        let deleted = ctx.memory_service.forget_memory(memory_id).await?;

        tracing::debug!(memory_id = %memory_id, deleted, "forget_memory 结果");
        Ok(json!({ "memory_id": memory_id.to_string(), "deleted": deleted }))
    }
}

/// Get statistics about indexed memories.
pub struct GetMemoryStatsHandler;

#[async_trait]
impl ToolHandler for GetMemoryStatsHandler {
    fn name(&self) -> &str {
        "get_memory_stats"
    }

    fn description(&self) -> &str {
        "获取记忆索引统计信息"
    }

    fn input_schema(&self) -> Value {
        definitions::get_memory_stats_schema()
    }

    fn module(&self) -> &str {
        "memory"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        tracing::debug!("get_memory_stats 调用");

        let stats = ctx.memory_service.get_memory_stats().await?;

        tracing::debug!(
            total_chunks = stats.total_chunks,
            total_notes = stats.total_notes,
            "get_memory_stats 结果"
        );
        Ok(json!({
            "total_chunks": stats.total_chunks,
            "total_notes": stats.total_notes,
            "tags": stats.tags
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Handler tests ──

    #[tokio::test]
    async fn test_add_memory_handler_returns_created_status() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = AddMemoryHandler;

        let args = json!({
            "note_path": "manual/test.md",
            "content": "Rust is a systems programming language.",
            "tags": ["rust", "programming"]
        });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["status"], "created");
        assert!(result["memory_id"].is_string());
        // Verify UUID format.
        let id = result["memory_id"].as_str().unwrap();
        assert!(Uuid::parse_str(id).is_ok());
    }

    #[tokio::test]
    async fn test_add_memory_handler_no_tags() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = AddMemoryHandler;

        let args = json!({
            "note_path": "manual/no_tags.md",
            "content": "Simple content without tags."
        });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["status"], "created");
    }

    #[tokio::test]
    async fn test_add_memory_handler_missing_note_path_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = AddMemoryHandler;

        let args = json!({
            "content": "Some content."
        });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_memory_handler_missing_content_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = AddMemoryHandler;

        let args = json!({
            "note_path": "manual/test.md"
        });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memory_handler_returns_chunks_structure() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // Add a memory so there's something to find.
        ctx.memory_service
            .add_memory(
                "manual/search.md",
                "Python data analysis",
                Some(vec!["python".to_string()]),
            )
            .await
            .unwrap();

        let handler = SearchMemoryHandler;
        let args = json!({ "query": "Python data" });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["chunks"].is_array());
        assert!(result["total"].is_number());

        // Verify chunk-level fields.
        if let Some(chunks) = result["chunks"].as_array() {
            for chunk in chunks {
                assert!(chunk.get("chunk_id").is_some());
                assert!(chunk.get("note_path").is_some());
                assert!(chunk.get("content").is_some());
                assert!(chunk.get("score").is_some());
                assert!(chunk.get("obsidian_uri").is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_search_memory_handler_missing_query_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = SearchMemoryHandler;

        let args = json!({});

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memory_handler_returns_updated_status() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // First add a memory.
        let memory_id = ctx
            .memory_service
            .add_memory("manual/update.md", "Old content here.", None)
            .await
            .unwrap();

        let handler = UpdateMemoryHandler;
        let args = json!({
            "memory_id": memory_id.to_string(),
            "content": "Updated content here."
        });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["status"], "updated");
        assert_eq!(result["memory_id"], memory_id.to_string());
    }

    #[tokio::test]
    async fn test_update_memory_handler_invalid_uuid() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = UpdateMemoryHandler;

        let args = json!({
            "memory_id": "not-a-uuid",
            "content": "Some content."
        });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_memory_handler_missing_content_rejected() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let memory_id = ctx
            .memory_service
            .add_memory("manual/update2.md", "Old content.", None)
            .await
            .unwrap();

        let handler = UpdateMemoryHandler;
        let args = json!({
            "memory_id": memory_id.to_string()
        });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_forget_memory_handler_returns_deleted_flag() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // Add a memory first.
        let memory_id = ctx
            .memory_service
            .add_memory("manual/forget.md", "Forgettable content.", None)
            .await
            .unwrap();

        let handler = ForgetMemoryHandler;
        let args = json!({ "memory_id": memory_id.to_string() });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["deleted"], true);
        assert_eq!(result["memory_id"], memory_id.to_string());
    }

    #[tokio::test]
    async fn test_forget_memory_handler_nonexistent_returns_false() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = ForgetMemoryHandler;

        let fake_id = Uuid::new_v4();
        let args = json!({ "memory_id": fake_id.to_string() });

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["deleted"], false);
        assert_eq!(result["memory_id"], fake_id.to_string());
    }

    #[tokio::test]
    async fn test_forget_memory_handler_invalid_uuid() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let handler = ForgetMemoryHandler;

        let args = json!({ "memory_id": "bad-uuid" });

        let result = handler.handle(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_memory_stats_handler_returns_statistics() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        // Add two memories so stats reflect them.
        ctx.memory_service
            .add_memory("stats/a.md", "Content A.", Some(vec!["tag1".to_string()]))
            .await
            .unwrap();
        ctx.memory_service
            .add_memory("stats/b.md", "Content B.", Some(vec!["tag2".to_string()]))
            .await
            .unwrap();

        let handler = GetMemoryStatsHandler;
        let args = json!({});

        let result = handler.handle(args, &ctx).await.unwrap();
        assert!(result["total_chunks"].as_u64().unwrap() >= 2);
        assert!(result["total_notes"].as_u64().unwrap() >= 2);
        assert!(result["tags"].is_array());
    }

    #[tokio::test]
    async fn test_get_memory_stats_handler_empty_vault() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();

        let handler = GetMemoryStatsHandler;
        let args = json!({});

        let result = handler.handle(args, &ctx).await.unwrap();
        assert_eq!(result["total_chunks"], 0);
        assert_eq!(result["total_notes"], 0);
        assert!(result["tags"].as_array().unwrap().is_empty());
    }

    // ── Schema / metadata tests ──

    #[test]
    fn test_search_memory_handler_metadata() {
        let handler = SearchMemoryHandler;
        assert_eq!(handler.name(), "search_memory");
        assert_eq!(handler.module(), "memory");
        assert!(handler.input_schema().is_object());
        assert_eq!(
            handler.input_schema()["required"].as_array().unwrap()[0],
            "query"
        );
    }

    #[test]
    fn test_add_memory_handler_metadata() {
        let handler = AddMemoryHandler;
        assert_eq!(handler.name(), "add_memory");
        assert_eq!(handler.module(), "memory");
        let schema = handler.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("note_path".to_string())));
        assert!(required.contains(&serde_json::Value::String("content".to_string())));
    }

    #[test]
    fn test_update_memory_handler_metadata() {
        let handler = UpdateMemoryHandler;
        assert_eq!(handler.name(), "update_memory");
        assert_eq!(handler.module(), "memory");
        let schema = handler.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
    }

    #[test]
    fn test_forget_memory_handler_metadata() {
        let handler = ForgetMemoryHandler;
        assert_eq!(handler.name(), "forget_memory");
        assert_eq!(handler.module(), "memory");
        let schema = handler.input_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "memory_id");
    }

    #[test]
    fn test_get_memory_stats_handler_metadata() {
        let handler = GetMemoryStatsHandler;
        assert_eq!(handler.name(), "get_memory_stats");
        assert_eq!(handler.module(), "memory");
        // No required fields.
        let schema = handler.input_schema();
        let required = schema.get("required").and_then(|v| v.as_array());
        match required {
            Some(arr) => assert!(arr.is_empty()),
            None => {} // "required" key omitted — valid, means no required params
        }
    }
}
