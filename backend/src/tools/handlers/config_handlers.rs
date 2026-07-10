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
    fn name(&self) -> &str {
        "get_config"
    }
    fn description(&self) -> &str {
        "获取系统配置"
    }
    fn input_schema(&self) -> Value {
        definitions::get_config_schema()
    }
    fn module(&self) -> &str {
        "system"
    }

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
                "api_key": config.llm.api_key.as_deref().unwrap_or(""),
                "api_key_env": config.llm.api_key_env.as_deref().unwrap_or(""),
                "base_url": config.llm.base_url.as_deref().unwrap_or(""),
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
    fn name(&self) -> &str {
        "save_config"
    }
    fn description(&self) -> &str {
        "保存系统配置（热更新，无需重启）"
    }
    fn input_schema(&self) -> Value {
        definitions::save_config_schema()
    }
    fn module(&self) -> &str {
        "system"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        // Save to DB
        let config_json = serde_json::to_string(&args)
            .map_err(|e| BrainError::Internal(format!("序列化配置失败: {e}")))?;

        ctx.db.set_state(CONFIG_KEY, &config_json)?;

        // Hot-reload ObsidianClient
        if let Some(obs_cfg) = args.get("obsidian") {
            let enabled = obs_cfg
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if enabled {
                let url = obs_cfg
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("http://127.0.0.1:27123")
                    .to_string();
                let api_key = obs_cfg
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let new_config = crate::config::ObsidianApiConfig {
                    enabled,
                    url,
                    api_key,
                };
                match crate::infra::obsidian_client::ObsidianClient::new(&new_config) {
                    Ok(client) => {
                        crate::infra::obsidian_client::set_client(
                            &ctx.obsidian,
                            Some(Arc::new(client)),
                        );
                        tracing::info!("ObsidianClient hot-reloaded");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to hot-reload ObsidianClient: {e}");
                    }
                }
            } else {
                crate::infra::obsidian_client::set_client(&ctx.obsidian, None);
            }
        }

        // Hot-reload LLM provider
        if let Some(llm_cfg) = args.get("llm") {
            let mut new_config = crate::config::LlmConfig::default();
            if let Some(p) = llm_cfg.get("provider").and_then(|v| v.as_str()) {
                new_config.provider = p.to_string();
            }
            if let Some(m) = llm_cfg.get("model").and_then(|v| v.as_str()) {
                new_config.model = m.to_string();
            }
            if let Some(k) = llm_cfg.get("api_key").and_then(|v| v.as_str()) {
                new_config.api_key = Some(k.to_string());
            }
            if let Some(k) = llm_cfg.get("api_key_env").and_then(|v| v.as_str()) {
                new_config.api_key_env = Some(k.to_string());
            }
            if let Some(u) = llm_cfg.get("base_url").and_then(|v| v.as_str()) {
                new_config.base_url = Some(u.to_string());
            }
            if let Some(t) = llm_cfg.get("max_tokens").and_then(|v| v.as_u64()) {
                new_config.max_tokens = t as u32;
            }
            if let Some(t) = llm_cfg.get("temperature").and_then(|v| v.as_f64()) {
                new_config.temperature = t;
            }

            match crate::infra::llm_client::LlmClientFactory::create(&new_config) {
                Ok(boxed) => {
                    let new_llm: Arc<dyn crate::infra::llm_client::LlmProvider> = Arc::from(boxed);
                    ctx.inspiration_service.set_llm(new_llm);
                    tracing::info!(
                        "LLM provider hot-reloaded: {} / {}",
                        new_config.provider,
                        new_config.model
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to hot-reload LLM: {e}");
                }
            }
        }

        tracing::info!("系统配置已保存并热更新");

        Ok(json!({
            "saved": true,
            "message": "配置已保存并生效",
        }))
    }
}

/// 验证 LLM 配置
pub struct VerifyLlmHandler;

#[async_trait]
impl ToolHandler for VerifyLlmHandler {
    fn name(&self) -> &str {
        "verify_llm"
    }
    fn description(&self) -> &str {
        "验证 LLM 配置是否可用（发送测试消息）"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "provider": { "type": "string" },
                "model": { "type": "string" },
                "api_key": { "type": "string" },
                "api_key_env": { "type": "string" },
                "base_url": { "type": "string" }
            }
        })
    }
    fn module(&self) -> &str {
        "system"
    }

    async fn handle(&self, args: Value, _ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let mut config = crate::config::LlmConfig::default();
        if let Some(p) = args.get("provider").and_then(|v| v.as_str()) {
            config.provider = p.to_string();
        }
        if let Some(m) = args.get("model").and_then(|v| v.as_str()) {
            config.model = m.to_string();
        }
        if let Some(k) = args.get("api_key").and_then(|v| v.as_str()) {
            config.api_key = Some(k.to_string());
        }
        if let Some(k) = args.get("api_key_env").and_then(|v| v.as_str()) {
            config.api_key_env = Some(k.to_string());
        }
        if let Some(u) = args.get("base_url").and_then(|v| v.as_str()) {
            config.base_url = Some(u.to_string());
        }

        // Create provider
        let provider =
            crate::infra::llm_client::LlmClientFactory::create(&config).map_err(|e| {
                BrainError::LlmApiError {
                    provider: config.provider.clone(),
                    detail: format!("创建 LLM 客户端失败: {e}"),
                }
            })?;

        // Send test message
        match provider.generate("请回复：OK").await {
            Ok(response) => Ok(json!({
                "valid": true,
                "message": "LLM 连接成功",
                "response": response.chars().take(100).collect::<String>(),
                "model": config.model,
            })),
            Err(e) => Ok(json!({
                "valid": false,
                "message": format!("LLM 验证失败: {e}"),
                "model": config.model,
            })),
        }
    }
}
