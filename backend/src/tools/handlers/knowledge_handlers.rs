//! 知识库洞察工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::core::knowledge_insights::KnowledgeInsightEngine;
use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

const CACHE_KEY: &str = "knowledge_insights";

/// 获取知识库洞察
pub struct GetKnowledgeInsightsHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeInsightsHandler {
    fn name(&self) -> &str { "get_knowledge_insights" }
    fn description(&self) -> &str { "获取知识库洞察：知识孤岛、枢纽、尘封笔记、新生知识、领域分布" }
    fn input_schema(&self) -> Value { definitions::get_knowledge_insights_schema() }
    fn module(&self) -> &str { "memory" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        // Try cache first (unless force refresh)
        if !force {
            if let Ok(Some(cached)) = ctx.db.get_state(CACHE_KEY) {
                if let Ok(data) = serde_json::from_str::<Value>(&cached) {
                    tracing::debug!("返回缓存的知识库洞察数据");
                    return Ok(data);
                }
            }
        }

        // Calculate fresh insights
        let obsidian = crate::infra::obsidian_client::get_client(&ctx.obsidian)?;

        let engine = KnowledgeInsightEngine::new(ctx.obsidian.clone());
        let insights = engine.get_insights().await?;

        let result = json!({
            "islands": insights.islands,
            "hubs": insights.hubs,
            "dormant": insights.dormant,
            "fresh": insights.fresh,
            "domains": insights.domains,
        });

        // Cache the result
        if let Ok(cached_str) = serde_json::to_string(&result) {
            let _ = ctx.db.set_state(CACHE_KEY, &cached_str);
            tracing::info!("知识库洞察数据已缓存");
        }

        Ok(result)
    }
}
