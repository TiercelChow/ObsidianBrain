//! 雷达工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 获取推荐列表
pub struct GetRadarHandler;

#[async_trait]
impl ToolHandler for GetRadarHandler {
    fn name(&self) -> &str {
        "get_radar"
    }
    fn description(&self) -> &str {
        "获取智识雷达推荐列表，返回按相关性排序的外部文章"
    }
    fn input_schema(&self) -> Value {
        definitions::get_radar_schema()
    }
    fn module(&self) -> &str {
        "radar"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(10);

        tracing::debug!(limit = limit, "get_radar 调用");
        let items = ctx.radar_service.get_radar(limit).await?;

        let items_json: Vec<Value> = items
            .iter()
            .map(|item| {
                json!({
                    "id": item.id,
                    "title": item.title,
                    "summary": item.summary,
                    "source": item.source,
                    "url": item.url,
                    "relevance_score": item.relevance_score,
                    "status": item.status,
                })
            })
            .collect();

        Ok(json!({ "items": items_json, "total": items_json.len() }))
    }
}

/// 保存到 Vault
pub struct AddToVaultHandler;

#[async_trait]
impl ToolHandler for AddToVaultHandler {
    fn name(&self) -> &str {
        "add_to_vault"
    }
    fn description(&self) -> &str {
        "将雷达中的文章保存到 Obsidian Vault"
    }
    fn input_schema(&self) -> Value {
        definitions::add_to_vault_schema()
    }
    fn module(&self) -> &str {
        "radar"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let article_id = args
            .get("article_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'article_id'".to_string()))?;

        let target_dir = args.get("target_dir").and_then(|v| v.as_str());

        tracing::debug!(article_id = %article_id, "add_to_vault 调用");
        let result = ctx
            .radar_service
            .add_to_vault(article_id, target_dir)
            .await?;

        Ok(json!({
            "note_path": result.note_path,
            "obsidian_uri": result.obsidian_uri,
            "summary": result.summary,
            "tags": result.tags,
            "word_count": result.word_count,
        }))
    }
}

/// 忽略条目
pub struct DismissRadarItemHandler;

#[async_trait]
impl ToolHandler for DismissRadarItemHandler {
    fn name(&self) -> &str {
        "dismiss_radar_item"
    }
    fn description(&self) -> &str {
        "忽略雷达条目，不再推荐"
    }
    fn input_schema(&self) -> Value {
        definitions::dismiss_radar_item_schema()
    }
    fn module(&self) -> &str {
        "radar"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let article_id = args
            .get("article_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'article_id'".to_string()))?;

        tracing::debug!(article_id = %article_id, "dismiss_radar_item 调用");
        let dismissed = ctx.radar_service.dismiss_radar_item(article_id).await?;

        Ok(json!({
            "article_id": article_id,
            "dismissed": dismissed,
        }))
    }
}
