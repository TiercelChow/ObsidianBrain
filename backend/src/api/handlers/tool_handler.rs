use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::AppContext;

/// Request body for the /v1/tools/call endpoint.
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    /// Name of the tool to invoke.
    pub tool: String,
    /// Arguments to pass to the tool (must conform to its input_schema).
    /// Defaults to `Value::Null` when omitted, supporting tools that take no arguments.
    #[serde(default)]
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
    let tools = ctx.tool_registry.list().await;
    Json(json!({ "tools": tools }))
}

/// POST /v1/tools/call -- invoke a tool by name.
pub async fn call_tool(
    State(ctx): State<Arc<AppContext>>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, StatusCode> {
    let request_id = Uuid::new_v4();
    let start = Instant::now();
    let tool_name = req.tool.clone();

    let handler = match ctx.tool_registry.get(&tool_name).await {
        Some(h) => h,
        None => {
            tracing::warn!(tool = %tool_name, "请求的工具不存在");
            let response = ToolCallResponse {
                tool: tool_name.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ToolErrorDetail {
                    code: "TOOL_NOT_FOUND".to_string(),
                    message: format!("Tool '{}' not found", tool_name),
                    suggestion: Some("Use GET /v1/tools to list available tools".to_string()),
                }),
            };
            let duration_ms = start.elapsed().as_millis() as u64;
            tracing::info!(
                request_id = %request_id,
                tool = %tool_name,
                status = "error",
                duration_ms = duration_ms,
                "Tool call completed"
            );
            return Ok(Json(response));
        }
    };

    // Validate arguments against the tool's JSON Schema.
    let schema = handler.input_schema();
    if !schema.is_null() {
        match JSONSchema::compile(&schema) {
            Ok(compiled) => {
                let validation_result = compiled.validate(&req.arguments);
                if let Err(errors) = validation_result {
                    let error_messages: Vec<String> =
                        errors.take(5).map(|e| e.to_string()).collect();
                    tracing::warn!(
                        tool = %tool_name,
                        errors = ?error_messages,
                        "参数校验失败"
                    );
                    let response = ToolCallResponse {
                        tool: tool_name.clone(),
                        status: "error".to_string(),
                        result: None,
                        error: Some(ToolErrorDetail {
                            code: "INVALID_PARAMS".to_string(),
                            message: format!("参数校验失败: {}", error_messages.join("; ")),
                            suggestion: Some("请检查工具参数的 JSON Schema".to_string()),
                        }),
                    };
                    let duration_ms = start.elapsed().as_millis() as u64;
                    tracing::info!(
                        request_id = %request_id,
                        tool = %tool_name,
                        status = "error",
                        duration_ms = duration_ms,
                        "Tool call completed"
                    );
                    return Ok(Json(response));
                }
            }
            Err(e) => {
                tracing::warn!(
                    tool = %tool_name,
                    error = %e,
                    "工具的 input_schema 编译失败，跳过参数校验"
                );
            }
        }
    }

    tracing::debug!(tool = %tool_name, "调用工具");
    let status;
    let response = match handler.handle(req.arguments, &ctx).await {
        Ok(result) => {
            status = "success";
            tracing::debug!(tool = %tool_name, "工具调用成功");
            ToolCallResponse {
                tool: tool_name,
                status: "success".to_string(),
                result: Some(result),
                error: None,
            }
        }
        Err(e) => {
            status = "error";
            tracing::warn!(tool = %tool_name, error = %e, "工具调用失败");
            ToolCallResponse {
                tool: tool_name,
                status: "error".to_string(),
                result: None,
                error: Some(ToolErrorDetail {
                    code: e.error_code().to_string(),
                    message: e.to_string(),
                    suggestion: e.suggestion().map(|s| s.to_string()),
                }),
            }
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    tracing::info!(
        request_id = %request_id,
        tool = %response.tool,
        status = %status,
        duration_ms = duration_ms,
        "Tool call completed"
    );

    Ok(Json(response))
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

    /// A mock tool that takes no arguments.
    struct NoArgsTool;

    #[async_trait]
    impl ToolHandler for NoArgsTool {
        fn name(&self) -> &str {
            "get_memory_stats"
        }

        fn description(&self) -> &str {
            "Returns memory statistics"
        }

        fn input_schema(&self) -> Value {
            json!({})
        }

        fn module(&self) -> &str {
            "memory"
        }

        async fn handle(
            &self,
            _args: Value,
            _ctx: &Arc<crate::AppContext>,
        ) -> Result<Value, BrainError> {
            Ok(json!({"total_memories": 0}))
        }
    }

    /// Helper to create a minimal AppContext for tests.
    fn create_test_app(registry: Arc<ToolRegistry>) -> Router {
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
        registry.register(Arc::new(EchoTool)).await;
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
        registry.register(Arc::new(EchoTool)).await;
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

    #[tokio::test]
    async fn test_call_tool_no_arguments() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(Arc::new(NoArgsTool)).await;
        let app = create_test_app(registry);

        // Omit "arguments" entirely — serde(default) should make it Value::Null
        let request_body = json!({
            "tool": "get_memory_stats"
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
        assert_eq!(json["tool"], "get_memory_stats");
        assert_eq!(json["result"]["total_memories"], 0);
    }

    #[tokio::test]
    async fn test_call_tool_invalid_params() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(Arc::new(EchoTool)).await;
        let app = create_test_app(registry);

        // Missing required "message" field
        let request_body = json!({
            "tool": "echo",
            "arguments": { "wrong_field": "hello" }
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
        assert_eq!(json["error"]["code"], "INVALID_PARAMS");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("参数校验失败"));
    }
}
