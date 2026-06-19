//! 知识库洞察工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::core::knowledge_insights::KnowledgeInsightEngine;
use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 获取知识库洞察
pub struct GetKnowledgeInsightsHandler;

#[async_trait]
impl ToolHandler for GetKnowledgeInsightsHandler {
    fn name(&self) -> &str { "get_knowledge_insights" }
    fn description(&self) -> &str { "获取知识库洞察：知识孤岛、枢纽、尘封笔记、新生知识、领域分布" }
    fn input_schema(&self) -> Value { definitions::get_knowledge_insights_schema() }
    fn module(&self) -> &str { "memory" }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let obsidian = ctx.obsidian.clone().ok_or_else(|| {
            BrainError::Internal("Obsidian API 不可用".to_string())
        })?;

        let engine = KnowledgeInsightEngine::new(obsidian);
        let insights = engine.get_insights().await?;

        Ok(json!({
            "islands": insights.islands,
            "hubs": insights.hubs,
            "dormant": insights.dormant,
            "fresh": insights.fresh,
            "domains": insights.domains,
        }))
    }
}
