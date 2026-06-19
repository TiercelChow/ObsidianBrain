//! 系统配置工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

const CONFIG_KEY: &str = "system_config";

/// 获取系统配置
pub struct GetConfigHandler;

#[async_trait]
impl ToolHandler for GetConfigHandler {
    fn name(&self) -> &str { "get_config" }
    fn description(&self) -> &str { "获取系统配置" }
    fn input_schema(&self) -> Value { definitions::get_config_schema() }
    fn module(&self) -> &str { "system" }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        // Try to load from DB cache
        if let Ok(Some(cached)) = ctx.db.get_state(CONFIG_KEY) {
            if let Ok(data) = serde_json::from_str::<Value>(&cached) {
                return Ok(data);
            }
        }

        // Return defaults from current config
        let config = &ctx.config;
        Ok(json!({
            "vault": {
                "path": config.vault.path.to_string_lossy(),
                "name": config.vault.name,
            },
            "obsidian": {
                "enabled": config.obsidian.enabled,
                "url": config.obsidian.url,
                "api_key": config.obsidian.api_key.as_deref().unwrap_or(""),
            },
            "llm": {
                "provider": config.llm.provider,
                "model": config.llm.model,
                "max_tokens": config.llm.max_tokens,
                "temperature": config.llm.temperature,
            }
        }))
    }
}

/// 保存系统配置
pub struct SaveConfigHandler;

#[async_trait]
impl ToolHandler for SaveConfigHandler {
    fn name(&self) -> &str { "save_config" }
    fn description(&self) -> &str { "保存系统配置（需重启生效）" }
    fn input_schema(&self) -> Value { definitions::save_config_schema() }
    fn module(&self) -> &str { "system" }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        // Save to DB
        let config_json = serde_json::to_string(&args)
            .map_err(|e| BrainError::Internal(format!("序列化配置失败: {e}")))?;

        ctx.db.set_state(CONFIG_KEY, &config_json)?;

        tracing::info!("系统配置已保存，重启后生效");

        Ok(json!({
            "saved": true,
            "message": "配置已保存，重启服务后生效",
        }))
    }
}
