//! LLM Wiki 工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::core::wiki::WikiEngine;
use crate::error::BrainError;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 摄入原始资料到 Wiki
pub struct IngestSourceHandler;

#[async_trait]
impl ToolHandler for IngestSourceHandler {
    fn name(&self) -> &str { "ingest_source" }
    fn description(&self) -> &str { "将原始资料摄入 Wiki：LLM 读取、摘要、提取实体概念、更新交叉引用" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": { "type": "string", "description": "Vault 中的原始资料路径" },
                "source_type": { "type": "string", "enum": ["article", "paper", "book_chapter", "podcast", "meeting", "note"], "default": "article" },
                "source_url": { "type": "string", "description": "原始 URL（可选）" }
            },
            "required": ["source_path"]
        })
    }
    fn module(&self) -> &str { "wiki" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let source_path = args.get("source_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrainError::Internal("缺少 source_path".to_string()))?;
        let source_type = args.get("source_type").and_then(|v| v.as_str()).unwrap_or("article");
        let source_url = args.get("source_url").and_then(|v| v.as_str());

        let llm: Arc<dyn crate::infra::llm_client::LlmProvider> = ctx.inspiration_service.get_llm();
        let engine = WikiEngine::new(ctx.obsidian.clone(), llm);
        let result = engine.ingest(source_path, source_type, source_url).await?;

        Ok(json!({
            "summary_page": result.summary_page,
            "created_pages": result.created_pages,
            "updated_pages": result.updated_pages,
            "entities": result.entities,
            "concepts": result.concepts,
        }))
    }
}

/// 基于 Wiki 回答问题
pub struct QueryWikiHandler;

#[async_trait]
impl ToolHandler for QueryWikiHandler {
    fn name(&self) -> &str { "query_wiki" }
    fn description(&self) -> &str { "基于已编译的 Wiki 回答问题（非 RAG）" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "用户的问题" },
                "save_answer": { "type": "boolean", "default": false, "description": "是否归档为综合论述" }
            },
            "required": ["question"]
        })
    }
    fn module(&self) -> &str { "wiki" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let question = args.get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BrainError::Internal("缺少 question".to_string()))?;
        let save_answer = args.get("save_answer").and_then(|v| v.as_bool()).unwrap_or(false);

        let llm: Arc<dyn crate::infra::llm_client::LlmProvider> = ctx.inspiration_service.get_llm();
        let engine = WikiEngine::new(ctx.obsidian.clone(), llm);
        let result = engine.query(question, save_answer).await?;

        Ok(json!({
            "answer": result.answer,
            "cited_pages": result.cited_pages,
            "saved_to": result.saved_to,
        }))
    }
}

/// Wiki 健康检查
pub struct LintWikiHandler;

#[async_trait]
impl ToolHandler for LintWikiHandler {
    fn name(&self) -> &str { "lint_wiki" }
    fn description(&self) -> &str { "检查 Wiki 健康度：孤岛页、矛盾、缺失引用" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "auto_fix": { "type": "boolean", "default": false, "description": "是否自动修复" }
            }
        })
    }
    fn module(&self) -> &str { "wiki" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let auto_fix = args.get("auto_fix").and_then(|v| v.as_bool()).unwrap_or(false);

        let llm: Arc<dyn crate::infra::llm_client::LlmProvider> = ctx.inspiration_service.get_llm();
        let engine = WikiEngine::new(ctx.obsidian.clone(), llm);
        let result = engine.lint(auto_fix).await?;

        Ok(json!({
            "total_pages": result.total_pages,
            "orphans": result.orphans,
            "missing_pages": result.missing_pages,
            "hubs": result.hubs,
            "fixed": result.fixed,
            "suggestions": result.suggestions,
        }))
    }
}

/// 获取 Wiki 状态
pub struct GetWikiStatusHandler;

#[async_trait]
impl ToolHandler for GetWikiStatusHandler {
    fn name(&self) -> &str { "get_wiki_status" }
    fn description(&self) -> &str { "获取 Wiki 当前状态：页数、类型分布" }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn module(&self) -> &str { "wiki" }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let llm: Arc<dyn crate::infra::llm_client::LlmProvider> = ctx.inspiration_service.get_llm();
        let engine = WikiEngine::new(ctx.obsidian.clone(), llm);
        let status = engine.status().await?;

        Ok(json!({
            "total_pages": status.total_pages,
            "entities": status.entities,
            "concepts": status.concepts,
            "sources": status.sources,
            "synthesis": status.synthesis,
            "initialized": status.initialized,
        }))
    }
}
