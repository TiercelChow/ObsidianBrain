use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

/// Request body for the /v1/tools/call endpoint.
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    /// Name of the tool to invoke.
    pub tool: String,
    /// Arguments to pass to the tool (must conform to its input_schema).
    pub arguments: Value,
}

/// Response body for the /v1/tools/call endpoint.
#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    /// Name of the invoked tool.
    pub tool: String,
    /// "success" or "error".
    pub status: String,
    /// Tool result (present on success).
    pub result: Option<Value>,
    /// Error details (present on failure).
    pub error: Option<ToolErrorDetail>,
}

/// Error detail returned when a tool invocation fails.
#[derive(Debug, Serialize)]
pub struct ToolErrorDetail {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional suggestion for recovery.
    pub suggestion: Option<String>,
}

/// GET /v1/tools -- list all available tools.
pub async fn list_tools(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let tools = ctx.tool_registry.list();
    Json(json!({ "tools": tools }))
}

/// POST /v1/tools/call -- invoke a tool by name.
pub async fn call_tool(
    State(ctx): State<Arc<AppContext>>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode> {
    let tool_name = req.tool.clone();
    let arguments = req.arguments;

    let handler = match ctx.tool_registry.get(&tool_name) {
        Some(h) => h,
        None => {
            tracing::warn!(tool = %tool_name, "请求的工具不存在");
            return Ok(Json(ToolCallResponse {
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolErrorDetail {
                    code: "TOOL_NOT_FOUND".to_string(),
                    message: format!("Tool '{}' not found", tool_name),
                    suggestion: Some("Use GET /v1/tools to list available tools".to_string()),
                }),
            }));
        }
    };

    tracing::debug!(tool = %tool_name, "调用工具");
    match handler.handle(arguments, &ctx).await {
        Ok(result) => {
            tracing::debug!(tool = %tool_name, "工具调用成功");
            Ok(Json(ToolCallResponse {
                tool: tool_name,
                status: "success".to_string(),
                result: Some(result),
                error: None,
            }))
        }
        Err(e) => {
            tracing::warn!(tool = %tool_name, error = %e, "工具调用失败");
            Ok(Json(ToolCallResponse {
                tool: tool_name,
                status: "error".to_string(),
                result: None,
                error: Some(ToolErrorDetail {
                    code: e.error_code().to_string(),
                    message: e.to_string(),
                    suggestion: e.suggestion().map(|s| s.to_string()),
                }),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::BrainError;
    use crate::tools::registry::ToolRegistry;
    use crate::tools::traits::ToolHandler;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    /// A mock tool handler for HTTP integration tests.
    struct EchoTool;

    #[async_trait]
    impl ToolHandler for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "Echoes back the input arguments"
        }

        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })
        }

        fn module(&self) -> &str {
            "test"
        }

        async fn handle(
            &self,
            args: Value,
            _ctx: &Arc<crate::AppContext>,
        ) -> Result<Value, BrainError> {
            Ok(args)
        }
    }

    /// Helper to create a minimal AppContext for tests.
    /// We build the absolute minimum needed — the tool_registry is the only
    /// field used by the tool handlers, so other fields are stubs.
    fn create_test_app(registry: Arc<ToolRegistry>) -> Router {
        // Build a minimal AppContext with just the tool_registry.
        // Other fields are not used by the tool endpoints.
        let ctx = Arc::new(crate::AppContext::for_test(registry));
        crate::api::router::create_router(ctx)
    }

    #[tokio::test]
    async fn test_list_tools_empty() {
        let registry = Arc::new(ToolRegistry::new());
        let app = create_test_app(registry);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let tools = json.get("tools").unwrap().as_array().unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_list_tools_with_registered_tool() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(Arc::new(EchoTool));
        let app = create_test_app(registry);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let tools = json.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[tokio::test]
    async fn test_call_tool_not_found() {
        let registry = Arc::new(ToolRegistry::new());
        let app = create_test_app(registry);

        let request_body = json!({
            "tool": "nonexistent",
            "arguments": {}
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error"]["code"], "TOOL_NOT_FOUND");
        assert_eq!(json["tool"], "nonexistent");
    }

    #[tokio::test]
    async fn test_call_tool_success() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(Arc::new(EchoTool));
        let app = create_test_app(registry);

        let request_body = json!({
            "tool": "echo",
            "arguments": { "message": "hello" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["tool"], "echo");
        assert_eq!(json["result"]["message"], "hello");
    }
}
