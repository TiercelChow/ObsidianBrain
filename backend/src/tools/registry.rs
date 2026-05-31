use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::tools::traits::{ToolDefinition, ToolHandler};

/// Central registry for all available tool handlers.
///
/// Tools are registered at startup and can be looked up by name at runtime.
/// The registry uses interior mutability (RwLock) so tools can be registered
/// after the AppContext is constructed, while still allowing concurrent reads.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        ToolRegistry {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool handler. If a tool with the same name already exists,
    /// it will be replaced.
    pub fn register(&self, handler: Arc<dyn ToolHandler>) {
        let name = handler.name().to_string();
        tracing::debug!(tool = %name, "注册工具");
        let mut tools = self.tools.write().unwrap();
        tools.insert(name, handler);
    }

    /// Look up a tool handler by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }

    /// List all registered tools as ToolDefinition metadata.
    pub fn list(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().unwrap();
        tools
            .values()
            .map(|h| ToolDefinition::from_handler(h.as_ref()))
            .collect()
    }

    /// Return the number of registered tools.
    pub fn count(&self) -> usize {
        let tools = self.tools.read().unwrap();
        tools.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::BrainError;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::Arc;

    /// A minimal mock tool for testing the registry.
    struct MockTool {
        name: String,
        description: String,
        module: String,
        schema: Value,
    }

    #[async_trait]
    impl ToolHandler for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn input_schema(&self) -> Value {
            self.schema.clone()
        }

        fn module(&self) -> &str {
            &self.module
        }

        async fn handle(
            &self,
            _args: Value,
            _ctx: &Arc<crate::AppContext>,
        ) -> Result<Value, BrainError> {
            Ok(json!({"mock": true}))
        }
    }

    fn make_mock_tool(name: &str) -> MockTool {
        MockTool {
            name: name.to_string(),
            description: format!("Mock tool: {name}"),
            module: "test".to_string(),
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        }
    }

    #[test]
    fn test_registry_register_and_list() {
        let registry = ToolRegistry::new();
        let tool = Arc::new(make_mock_tool("search_notes"));
        registry.register(tool);

        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "search_notes");
        assert_eq!(list[0].module, "test");
    }

    #[test]
    fn test_registry_get_existing_tool() {
        let registry = ToolRegistry::new();
        let tool = Arc::new(make_mock_tool("get_note"));
        registry.register(tool);

        let found = registry.get("get_note");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "get_note");
    }

    #[test]
    fn test_registry_get_missing_tool() {
        let registry = ToolRegistry::new();
        let found = registry.get("nonexistent");
        assert!(found.is_none());
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(make_mock_tool("my_tool")));
        registry.register(Arc::new(make_mock_tool("my_tool")));

        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_registry_multiple_tools() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(make_mock_tool("tool_a")));
        registry.register(Arc::new(make_mock_tool("tool_b")));
        registry.register(Arc::new(make_mock_tool("tool_c")));

        assert_eq!(registry.count(), 3);
        let list = registry.list();
        let names: Vec<&str> = list.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"tool_a"));
        assert!(names.contains(&"tool_b"));
        assert!(names.contains(&"tool_c"));
    }

    #[test]
    fn test_tool_definition_from_handler() {
        let tool = make_mock_tool("search_notes");
        let def = ToolDefinition::from_handler(&tool);
        assert_eq!(def.name, "search_notes");
        assert_eq!(def.module, "test");
        // Verify the schema is a valid JSON object
        assert!(def.input_schema.is_object());
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = ToolRegistry::default();
        assert_eq!(registry.count(), 0);
    }
}
