//! 灵感工具处理器

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 获取灵感
pub struct GetInspirationHandler;

#[async_trait]
impl ToolHandler for GetInspirationHandler {
    fn name(&self) -> &str {
        "get_inspiration"
    }
    fn description(&self) -> &str {
        "从用户的知识库中生成灵感。支持三种模式：随机概念组合、反向提问、对立观点。"
    }
    fn input_schema(&self) -> Value {
        definitions::get_inspiration_schema()
    }
    fn module(&self) -> &str {
        "inspiration"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let insp_type = args.get("type").and_then(|v| v.as_str());
        let note_path = args.get("note_path").and_then(|v| v.as_str());

        tracing::debug!(r#type = ?insp_type, note_path = ?note_path, "get_inspiration 调用");

        let result = ctx
            .inspiration_service
            .get_inspiration(insp_type, note_path)
            .await?;
        serde_json::to_value(result)
            .map_err(|e| BrainError::Internal(format!("序列化灵感结果失败: {e}")))
    }
}
