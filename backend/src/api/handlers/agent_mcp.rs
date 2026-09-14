use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::core::agent_tool_gateway::{agent_knowledge_tool_schemas, call_agent_knowledge_tool};
use crate::error::BrainError;
use crate::AppContext;

pub async fn agent_mcp(
    State(ctx): State<Arc<AppContext>>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Result<Response, BrainError> {
    let token = bearer_token(&headers)?;
    let grant = ctx
        .book_wiki_service
        .store()
        .validate_agent_run_token(token)?;
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if method.starts_with("notifications/") {
        return Ok(StatusCode::ACCEPTED.into_response());
    }
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "obsidianbrain-book-wiki", "version": "0.1.0" }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let tools = agent_knowledge_tool_schemas()
                .into_iter()
                .filter(|schema| {
                    schema
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| grant.allowed_tools.iter().any(|tool| tool == name))
                })
                .collect::<Vec<_>>();
            Ok(json!({ "tools": tools }))
        }
        "tools/call" => call_tool(&ctx, token, request.get("params")),
        _ => Err((-32601, format!("未知的 MCP 方法: {method}"))),
    };
    let payload = match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        }),
    };
    Ok(Json(payload).into_response())
}

fn call_tool(
    ctx: &Arc<AppContext>,
    token: &str,
    params: Option<&Value>,
) -> Result<Value, (i64, String)> {
    let params = params
        .and_then(Value::as_object)
        .ok_or_else(|| (-32602, "tools/call params 必须是对象".to_string()))?;
    if params.keys().any(|key| key != "name" && key != "arguments") {
        return Err((-32602, "tools/call 包含未知参数".to_string()));
    }
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| (-32602, "tools/call 缺少 name".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match call_agent_knowledge_tool(ctx.book_wiki_service.store(), token, name, arguments) {
        Ok(value) => {
            let text = serde_json::to_string(&value)
                .map_err(|error| (-32603, format!("工具结果序列化失败: {error}")))?;
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": value,
                "isError": false
            }))
        }
        Err(error) => Ok(json!({
            "content": [{ "type": "text", "text": error.to_string() }],
            "isError": true
        })),
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, BrainError> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BrainError::KnowledgeValidation("缺少 Agent 能力令牌".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::api::router::create_router;
    use crate::models::book_wiki::{BookKind, ReaderBook};

    #[tokio::test]
    async fn test_agent_mcp_lists_only_capability_tools_and_revokes_after_run() {
        let (ctx, _dir, vault) = crate::AppContext::for_test();
        ctx.book_wiki_service
            .store()
            .save_reader_books(&[ReaderBook {
                id: "mcp-book".to_string(),
                path: vault.to_string_lossy().to_string(),
                kind: BookKind::Folder,
                name: "MCP 测试书".to_string(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = ctx
            .book_wiki_service
            .store()
            .initialize_base("mcp-book")
            .unwrap();
        let run = ctx
            .book_wiki_service
            .store()
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        let capability = ctx
            .book_wiki_service
            .store()
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &["book_get_context".to_string()],
                300,
            )
            .unwrap();
        let app = create_router(ctx.clone());
        let request = Request::post("/v1/knowledge/agent-mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", capability.token))
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["result"]["tools"].as_array().unwrap().len(), 1);
        assert_eq!(payload["result"]["tools"][0]["name"], "book_get_context");

        ctx.book_wiki_service
            .store()
            .complete_agent_run(&run.id, &json!({}))
            .unwrap();
        let revoked = Request::post("/v1/knowledge/agent-mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", capability.token))
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string(),
            ))
            .unwrap();
        let response = app.oneshot(revoked).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
