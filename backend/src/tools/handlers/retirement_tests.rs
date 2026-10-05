use super::*;
use axum::{body::Body, http::Request};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn test_register_all_tools_excludes_retired_modules_and_keeps_daily_tools() {
    let (ctx, _dir, _vault) = AppContext::for_test();
    register_all_tools(&ctx.tool_registry, ctx.clone()).await;
    for name in [
        "get_inspiration",
        "get_radar",
        "add_to_vault",
        "dismiss_radar_item",
        "search_notes",
        "get_note",
        "list_recent_notes",
        "list_files",
        "get_memory_stats",
        "sync_memos",
    ] {
        assert!(
            ctx.tool_registry.get(name).await.is_none(),
            "retired tool {name}"
        );
    }
    for name in [
        "list_code_repos",
        "get_repo_detail",
        "open_in_vscode",
        "browse_timeline",
        "update_memo",
        "delete_memo",
        "get_timeline_storage",
        "list_tasks",
        "list_book_knowledge_bases",
    ] {
        assert!(
            ctx.tool_registry.get(name).await.is_some(),
            "retained tool {name}"
        );
    }
    assert!(ctx
        .tool_registry
        .list()
        .await
        .iter()
        .all(|tool| !["inspiration", "radar"].contains(&tool.module.as_str())));
}

#[tokio::test]
async fn test_retired_tool_calls_return_tool_not_found() {
    let (ctx, _dir, _vault) = AppContext::for_test();
    register_all_tools(&ctx.tool_registry, ctx.clone()).await;
    let app = crate::api::router::create_router(ctx);
    for name in [
        "get_inspiration",
        "get_radar",
        "add_to_vault",
        "dismiss_radar_item",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"tool": name, "arguments": {}}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result["error"]["code"], "TOOL_NOT_FOUND");
        assert_eq!(result["tool"], name);
    }
}
