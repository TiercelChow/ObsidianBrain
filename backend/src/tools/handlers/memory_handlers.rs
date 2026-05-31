//! Memory-related tool handlers: get_memory_stats.
//!
//! Note: search_memory, add_memory, update_memory, forget_memory have been removed
//! as the system now uses Obsidian's native search and file operations directly.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// Get vault statistics.
pub struct GetMemoryStatsHandler;

#[async_trait]
impl ToolHandler for GetMemoryStatsHandler {
    fn name(&self) -> &str {
        "get_memory_stats"
    }

    fn description(&self) -> &str {
        "获取 vault 统计信息"
    }

    fn input_schema(&self) -> Value {
        definitions::get_memory_stats_schema()
    }

    fn module(&self) -> &str {
        "memory"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        tracing::debug!("get_memory_stats 调用");

        let stats = ctx.memory_service.get_stats().await?;

        tracing::debug!(
            total_files = stats.total_files,
            vault_name = %stats.vault_name,
            "get_memory_stats 结果"
        );
        Ok(json!({
            "total_files": stats.total_files,
            "vault_path": stats.vault_path,
            "vault_name": stats.vault_name,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
