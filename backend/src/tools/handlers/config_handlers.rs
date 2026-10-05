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
        let config = &ctx.config;
        let mut data = json!({"llm":{
            "provider":config.llm.provider,"model":config.llm.model,"api_key":"",
            "api_key_env":config.llm.api_key_env.as_deref().unwrap_or(""),
            "base_url":config.llm.base_url.as_deref().unwrap_or(""),
            "max_tokens":config.llm.max_tokens,"temperature":config.llm.temperature
        }});
        if let Some(cached) = ctx.db.get_state(CONFIG_KEY)? {
            let saved: Value = serde_json::from_str(&cached)
                .map_err(|e| BrainError::ConfigError(e.to_string()))?;
            if let Some(llm) = saved.get("llm").and_then(Value::as_object) {
                if let Some(current) = data["llm"].as_object_mut() {
                    current.extend(llm.clone());
                    current.insert("api_key".into(), json!(""));
                }
            }
        }
        let storage = ctx.memo_manager.images.stats().await?;
        data["timeline"] = json!({"cache_limit_mb":storage.cache_limit_bytes/(1024*1024)});
        Ok(data)
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
        "保存系统配置；图片缓存容量立即生效，通用 LLM 启动配置重启后使用"
    }
    fn input_schema(&self) -> Value {
        definitions::save_config_schema()
    }
    fn module(&self) -> &str {
        "system"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        if let Some(mb) = args
            .get("timeline")
            .and_then(|v| v.get("cache_limit_mb"))
            .and_then(Value::as_u64)
        {
            let bytes = mb * 1024 * 1024;
            ctx.db
                .set_state("timeline_cache_limit_bytes", &bytes.to_string())?;
            ctx.memo_manager.images.set_budget(bytes).await?;
        }
        let mut saved = ctx
            .db
            .get_state(CONFIG_KEY)?
            .map(|v| serde_json::from_str::<Value>(&v))
            .transpose()
            .map_err(|e| BrainError::ConfigError(e.to_string()))?
            .unwrap_or_else(|| json!({}));
        if let Some(llm) = args.get("llm") {
            let old_key = saved.get("llm").and_then(|l| l.get("api_key")).cloned();
            saved["llm"] = llm.clone();
            if saved["llm"].get("api_key").and_then(Value::as_str) == Some("") {
                if let Some(key) = old_key {
                    saved["llm"]["api_key"] = key;
                }
            }
        }
        if let Some(object) = saved.as_object_mut() {
            if ctx.db.get_state("timeline_legacy_directory")?.is_none() {
                if let Some(path) = object
                    .get("vault")
                    .and_then(|v| v.get("path"))
                    .and_then(Value::as_str)
                    .filter(|p| !p.is_empty())
                {
                    ctx.db.set_state("timeline_legacy_directory", path)?;
                }
            }
            object.remove("obsidian");
            object.remove("vault");
        }
        ctx.db.set_state(CONFIG_KEY, &saved.to_string())?;
        tracing::info!("系统配置已保存，图片缓存上限立即生效");

        Ok(json!({
            "saved": true,
            "message": "配置已保存；缓存上限立即生效，通用 LLM 启动配置重启后使用",
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
