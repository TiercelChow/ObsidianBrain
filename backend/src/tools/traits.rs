use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::error::BrainError;
use crate::AppContext;

/// The core trait that every tool handler must implement.
///
/// Each tool provides its name, description, input JSON Schema, and an async
/// handler function that receives arguments and the application context.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Machine-readable tool name (e.g. "search_notes").
    fn name(&self) -> &str;

    /// Human-readable description of what the tool does.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's input parameters.
    fn input_schema(&self) -> Value;

    /// The module this tool belongs to (e.g. "memory", "code_repo").
    fn module(&self) -> &str;

    /// Execute the tool with the given arguments and shared context.
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError>;
}

/// Metadata about a registered tool, returned by the list endpoint.
#[derive(Debug, serde::Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub module: String,
}

impl ToolDefinition {
    /// Build a ToolDefinition from any ToolHandler implementation.
    pub fn from_handler(handler: &dyn ToolHandler) -> Self {
        ToolDefinition {
            name: handler.name().to_string(),
            description: handler.description().to_string(),
            input_schema: handler.input_schema(),
            module: handler.module().to_string(),
        }
    }
}
