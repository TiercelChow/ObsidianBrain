use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::core::agent_tool_gateway::{
    agent_knowledge_tool_schemas, call_agent_knowledge_tool, AGENT_EXTERNAL_RESEARCH_TOOL,
};
use crate::core::external_research::fetch_external_source;
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
        "tools/call" => call_tool(&ctx, token, request.get("params")).await,
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

async fn call_tool(
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
    let store = ctx.book_wiki_service.store();
    let result = match store.consume_agent_tool_call(token, name) {
        Ok(grant) if name == AGENT_EXTERNAL_RESEARCH_TOOL => {
            fetch_external_source(store, &grant, arguments).await
        }
        Ok(_) => call_agent_knowledge_tool(store, token, name, arguments),
        Err(error) => Err(error),
    };
    let result = result.and_then(|mut value| {
        // Search previews and catalog rows also occupy context. Charge the
        // actual payload before allocating citations for any returned body.
        let grant = store.validate_agent_run_capability(token, name)?;
        if store.get_adaptive_run_budget(&grant.run_id)?.is_some() {
            let tokens = crate::core::book_wiki::estimate_knowledge_payload_tokens(&value);
            let management = matches!(
                name,
                "knowledge_get_run_budget"
                    | "knowledge_request_budget_extension"
                    | "knowledge_report_evidence_coverage"
            );
            let budget = if management {
                store.record_adaptive_management_payload(&grant.run_id, tokens)?
            } else {
                store.record_adaptive_tool_payload(&grant.run_id, tokens)?
            };
            value["budget_feedback"] = json!({
                "used_tool_calls":budget.used_tool_calls,"soft_tool_calls":budget.soft_tool_calls,
                "hard_tool_calls":budget.policy.hard_tool_calls,
                "estimated_data_payload_tokens":budget.estimated_data_payload_tokens,
                "soft_retrieval_tokens":budget.soft_retrieval_tokens,
                "hard_retrieval_tokens":budget.policy.hard_retrieval_tokens,
                "deadline":budget.deadline,"billing_usage":false,
            });
        }
        record_tool_evidence(ctx.book_wiki_service.store(), token, name, &mut value)?;
        Ok(value)
    });
    match result {
        Ok(value) => {
            let text = serde_json::to_string(&value)
                .map_err(|error| (-32603, format!("工具结果序列化失败: {error}")))?;
            Ok(json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": value,
                "isError": false
            }))
        }
        Err(error) => {
            let detail = error.to_string().chars().take(1000).collect::<String>();
            if let Ok(grant) = store.validate_agent_run_token(token) {
                if store
                    .get_adaptive_run_budget(&grant.run_id)
                    .ok()
                    .flatten()
                    .is_some()
                {
                    // Error replies also occupy context. They must not be
                    // counted as evidence or hidden behind successful-only usage.
                    let _ = store.record_adaptive_management_payload(
                        &grant.run_id,
                        crate::core::book_wiki::estimate_knowledge_payload_tokens(
                            &json!({"error":detail}),
                        ),
                    );
                    if ["预算", "上限", "上下文", "期限"]
                        .iter()
                        .any(|word| detail.contains(word))
                    {
                        let _ = store.record_adaptive_limit_event(&grant.run_id, name, &detail);
                    }
                }
            }
            Ok(json!({
                "content": [{ "type": "text", "text": detail }],
                "isError": true
            }))
        }
    }
}

fn record_tool_evidence(
    store: &crate::infra::book_wiki_store::BookWikiStore,
    token: &str,
    tool: &str,
    result: &mut Value,
) -> Result<(), BrainError> {
    let (kind, object, version, snapshot) = match tool {
        "knowledge_get_entry" => {
            let id = result.get("id").and_then(Value::as_str);
            let revision = result.get("revision").and_then(Value::as_i64);
            let (Some(id), Some(revision)) = (id, revision) else {
                return Err(BrainError::Internal(
                    "知识条目工具结果缺少证据版本".to_string(),
                ));
            };
            (
                "entry",
                id.to_string(),
                revision.to_string(),
                json!({
                    "title": result.get("title"),
                    "summary": result.get("summary"),
                    "source_path": result.get("source_path"),
                    "content_md": result.get("content_md"),
                    "offset_chars": result.get("offset_chars"),
                }),
            )
        }
        "book_read_source_span" => {
            let span = result.get("span");
            let id = span
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str);
            let version = span
                .and_then(|value| value.get("source_version_id"))
                .and_then(Value::as_str);
            let (Some(id), Some(version)) = (id, version) else {
                return Err(BrainError::Internal(
                    "来源片段工具结果缺少证据版本".to_string(),
                ));
            };
            (
                "source_span",
                id.to_string(),
                version.to_string(),
                json!({
                    "source_path": span.and_then(|value| value.get("source_path")),
                    "heading": span.and_then(|value| value.get("heading")),
                    "line_start": span.and_then(|value| value.get("line_start")),
                    "line_end": span.and_then(|value| value.get("line_end")),
                    "offset_chars": span.and_then(|value| value.get("offset_chars")),
                    "returned_chars": span.and_then(|value| value.get("content")).and_then(Value::as_str).map(|value| value.chars().count()),
                    "total_chars": span.and_then(|value| value.get("total_chars")),
                    "content_md": span.and_then(|value| value.get("content")),
                }),
            )
        }
        AGENT_EXTERNAL_RESEARCH_TOOL => {
            let url = result.get("url").and_then(Value::as_str);
            let text = result.get("text").and_then(Value::as_str);
            let (Some(url), Some(text)) = (url, text) else {
                return Err(BrainError::Internal(
                    "外部资料工具结果缺少地址或正文".to_string(),
                ));
            };
            let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
            (
                "external",
                url.to_string(),
                hash.clone(),
                json!({
                    "url": url,
                    "content_sha256": hash,
                    "content_type": result.get("content_type"),
                    "request_number": result.get("request_number"),
                    "content_md": text,
                }),
            )
        }
        _ => return Ok(()),
    };
    let grant = store.validate_agent_run_token(token)?;
    let index =
        store.record_visible_agent_evidence(&grant.run_id, kind, &object, &version, &snapshot)?;
    let citation = json!({"citation_index":index,"label":format!("S{index}"),"object_id":object,"version_id":version});
    result["citation"] = citation.clone();
    if kind == "source_span" {
        result["span"]["citation"] = citation;
    }
    Ok(())
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
    use crate::infra::book_wiki_store::{MarkdownSourceDraft, SourceSectionDraft};
    use crate::models::book_wiki::{BookKind, ReaderBook};

    #[tokio::test]
    async fn test_adaptive_mcp_rejects_oversize_body_before_citation_and_recovers_with_paging() {
        let (ctx, _dir, vault) = crate::AppContext::for_test();
        let store = ctx.book_wiki_service.store();
        store
            .save_reader_books(&[ReaderBook {
                id: "adaptive-mcp-book".into(),
                path: vault.display().to_string(),
                kind: BookKind::Folder,
                name: "预算MCP".into(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("adaptive-mcp-book").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[MarkdownSourceDraft {
                    id: "adaptive-source".into(),
                    version_id: "adaptive-version".into(),
                    original_path: vault.join("mock.md").display().to_string(),
                    relative_path: "mock.md".into(),
                    title: "来源".into(),
                    ordinal: 0,
                    content_hash: "adaptive-hash".into(),
                    size_bytes: 10000,
                    modified_at: None,
                    sections: vec![SourceSectionDraft {
                        id: "adaptive-span".into(),
                        entry_id: "adaptive-entry".into(),
                        slug: "adaptive-entry".into(),
                        title: "原文".into(),
                        summary: "原文条件".into(),
                        content_md: "需要完整取证的条件。".repeat(500),
                        line_start: 1,
                        line_end: 1,
                        content_hash: "adaptive-section".into(),
                    }],
                }],
            )
            .unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        store
            .init_adaptive_run_budget(
                &run.id,
                &crate::infra::book_wiki_store::AdaptiveBudgetPolicy {
                    initial_prompt_tokens: 100,
                    soft_tool_calls: 2,
                    hard_tool_calls: 12,
                    soft_retrieval_tokens: 600,
                    hard_retrieval_tokens: 32000,
                    context_window: Some(32768),
                    max_output_tokens: Some(2048),
                    timeout_seconds: 60,
                    subquestions: vec!["条件是什么".into()],
                },
            )
            .unwrap();
        let capability = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &[
                    "book_read_source_span".into(),
                    "knowledge_get_run_budget".into(),
                    "knowledge_request_budget_extension".into(),
                    "knowledge_report_evidence_coverage".into(),
                ],
                300,
            )
            .unwrap();
        let params = json!({"name":"book_read_source_span","arguments":{"knowledge_base_id":base.id,"source_span_id":"adaptive-span"}});
        let response = call_tool(&ctx, &capability.token, Some(&params))
            .await
            .unwrap();
        assert_eq!(response["isError"], true);
        assert!(store.list_agent_run_citations(&run.id).unwrap().is_empty());
        let extension = json!({"name":"knowledge_request_budget_extension","arguments":{"knowledge_base_id":base.id,"reason":"需要补读条件，先缩小分页","missing_question_indices":[0]}});
        let response = call_tool(&ctx, &capability.token, Some(&extension))
            .await
            .unwrap();
        assert_eq!(response["isError"], false);
        let params = json!({"name":"book_read_source_span","arguments":{"knowledge_base_id":base.id,"source_span_id":"adaptive-span","max_chars":20}});
        let response = call_tool(&ctx, &capability.token, Some(&params))
            .await
            .unwrap();
        assert_eq!(response["isError"], false, "{response}");
        assert_eq!(response["structuredContent"]["citation"]["label"], "S1");
        let citations = store.list_agent_run_citations(&run.id).unwrap();
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].content_md.chars().count(), 20);
        let invalid = json!({"name":"knowledge_report_evidence_coverage","arguments":{"knowledge_base_id":base.id,"question_index":0,"status":"supported","citation_indices":[99],"finding":"假引用不能通过"}});
        assert_eq!(
            call_tool(&ctx, &capability.token, Some(&invalid))
                .await
                .unwrap()["isError"],
            true
        );
        let valid = json!({"name":"knowledge_report_evidence_coverage","arguments":{"knowledge_base_id":base.id,"question_index":0,"status":"partial","citation_indices":[1],"finding":"仅核对了开头的条件"}});
        assert_eq!(
            call_tool(&ctx, &capability.token, Some(&valid))
                .await
                .unwrap()["isError"],
            false
        );
        let state = store.get_adaptive_run_budget(&run.id).unwrap().unwrap();
        assert_eq!(state.coverage[0].status, "partial");
        assert!(state.estimated_tool_payload_tokens > 0);
        let wrong = json!({"name":"knowledge_get_run_budget","arguments":{"knowledge_base_id":"not-authorized"}});
        assert_eq!(
            call_tool(&ctx, &capability.token, Some(&wrong))
                .await
                .unwrap()["isError"],
            true
        );
        assert!(!store
            .validate_agent_run_token(&capability.token)
            .unwrap()
            .allowed_tools
            .contains(&"book_fetch_external".into()));
    }

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

        let unauthorized_external = Request::post("/v1/knowledge/agent-mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", capability.token))
            .body(Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "book_fetch_external",
                        "arguments": { "url": "https://example.com/" }
                    }
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(unauthorized_external).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["result"]["isError"], true);
        assert!(payload["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("工具权限"));

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

    #[tokio::test]
    async fn test_agent_mcp_records_source_span_actually_read() {
        let (ctx, _dir, vault) = crate::AppContext::for_test();
        let store = ctx.book_wiki_service.store();
        store
            .save_reader_books(&[ReaderBook {
                id: "evidence-book".to_string(),
                path: vault.to_string_lossy().to_string(),
                kind: BookKind::Folder,
                name: "证据测试书".to_string(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .unwrap();
        let base = store.initialize_base("evidence-book").unwrap();
        store
            .sync_markdown_sources(
                &base.id,
                &[MarkdownSourceDraft {
                    id: "source-evidence".to_string(),
                    version_id: "version-evidence".to_string(),
                    original_path: vault.join("evidence.md").to_string_lossy().to_string(),
                    relative_path: "evidence.md".to_string(),
                    title: "证据".to_string(),
                    ordinal: 0,
                    content_hash: "hash-evidence".to_string(),
                    size_bytes: 10,
                    modified_at: None,
                    sections: vec![SourceSectionDraft {
                        id: "span-evidence".to_string(),
                        entry_id: "entry-evidence".to_string(),
                        slug: "evidence".to_string(),
                        title: "证据".to_string(),
                        summary: "原文证据".to_string(),
                        content_md: "原文证据".repeat(1_500),
                        line_start: 1,
                        line_end: 1,
                        content_hash: "section-hash-evidence".to_string(),
                    }],
                }],
            )
            .unwrap();
        let run = store
            .start_agent_run(&base.id, "deepseek_harness", "knowledge_qa", &json!({}))
            .unwrap();
        let capability = store
            .issue_agent_run_capability(
                &run.id,
                std::slice::from_ref(&base.id),
                &["book_read_source_span".to_string()],
                300,
            )
            .unwrap();
        let app = create_router(ctx.clone());
        let request = Request::post("/v1/knowledge/agent-mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", capability.token))
            .body(Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "book_read_source_span",
                        "arguments": {
                            "knowledge_base_id": base.id,
                            "source_span_id": "span-evidence"
                        }
                    }
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["result"]["isError"], false);
        let ledger = store.list_agent_run_evidence(&run.id).unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].kind, "source_span");
        assert_eq!(ledger[0].object_id, "span-evidence");
        assert_eq!(ledger[0].version_id, "version-evidence");
        assert_eq!(ledger[0].snapshot["offset_chars"], 0);
        assert_eq!(ledger[0].snapshot["returned_chars"], 4_000);
        assert_eq!(
            payload["result"]["structuredContent"]["citation"]["label"],
            "S1"
        );
        assert_eq!(
            store
                .get_agent_run_citation(&run.id, 0)
                .unwrap()
                .content_md
                .chars()
                .count(),
            4_000
        );

        let next_page = Request::post("/v1/knowledge/agent-mcp")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", capability.token))
            .body(Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "book_read_source_span",
                        "arguments": {
                            "knowledge_base_id": base.id,
                            "source_span_id": "span-evidence",
                            "offset_chars": 4000
                        }
                    }
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.oneshot(next_page).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["result"]["structuredContent"]["span"]["has_more"],
            false
        );
        let ledger = store.list_agent_run_evidence(&run.id).unwrap();
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger[1].snapshot["offset_chars"], 4_000);
        assert_eq!(
            payload["result"]["structuredContent"]["citation"]["label"],
            "S1"
        );
        let citations = store.list_agent_run_citations(&run.id).unwrap();
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].content_md.chars().count(), 6_000);
    }
}
