use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(app: &axum::Router, tool: &str, arguments: Value) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/tools/call")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"tool":tool,"arguments":arguments}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

#[tokio::test]
async fn test_timeline_http_upload_create_edit_delete_without_external_service() {
    let (ctx, _dir, _old) = crate::AppContext::for_test();
    crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
    let app = crate::api::router::create_router(ctx.clone());
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(10, 10)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let mut body=b"--test-boundary\r\nContent-Disposition: form-data; name=\"images\"; filename=\"test.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    body.extend(&png);
    body.extend(b"\r\n--test-boundary--\r\n");
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/upload/images")
                .header(
                    "content-type",
                    "multipart/form-data; boundary=test-boundary",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let upload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    let path = upload["paths"][0].as_str().unwrap();
    let created = call(
        &app,
        "create_memo",
        json!({"content":"before","images":[path],"tags":[]}),
    )
    .await;
    assert_eq!(created["status"], "success");
    let id = &created["result"]["id"];
    let original = app
        .clone()
        .oneshot(
            Request::get(format!("/v1/timeline/images/{path}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(original.headers()["content-type"], "image/png");
    assert_eq!(original.headers()["cache-control"], "no-store");
    assert_eq!(
        to_bytes(original.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .as_ref(),
        png
    );
    let updated=call(&app,"update_memo",json!({"memo_id":id,"expected_revision":1,"content":"after","images":[path],"tags":["edited"]})).await;
    assert_eq!(updated["result"]["revision"], 2);
    let stale = call(
        &app,
        "delete_memo",
        json!({"memo_id":id,"expected_revision":1}),
    )
    .await;
    assert_eq!(stale["error"]["code"], "MEMO_VERSION_CONFLICT");
    let deleted = call(
        &app,
        "delete_memo",
        json!({"memo_id":id,"expected_revision":2}),
    )
    .await;
    assert_eq!(deleted["result"]["pending_cleanup"], 0);
    assert_eq!(ctx.memo_manager.count_memos().unwrap(), 0);
    assert!(ctx.memo_manager.images.original(path).await.is_err());
}

#[tokio::test]
async fn test_config_sanitizes_old_connection_and_preserves_unrelated_settings() {
    let (ctx, _dir, _old) = crate::AppContext::for_test();
    ctx.db.set_state("system_config",&json!({"vault":{"path":"/legacy"},"obsidian":{"enabled":true,"api_key":"legacy-test-secret"},"server":{"port":19876},"llm":{"model":"custom","api_key":"llm-test-secret"}}).to_string()).unwrap();
    crate::tools::handlers::register_all_tools(&ctx.tool_registry, ctx.clone()).await;
    let app = crate::api::router::create_router(ctx.clone());
    let config = call(&app, "get_config", json!({})).await;
    assert!(config["result"].get("vault").is_none());
    assert!(config["result"].get("obsidian").is_none());
    assert_eq!(config["result"]["llm"]["api_key"], "");
    assert_eq!(
        call(
            &app,
            "save_config",
            json!({"timeline":{"cache_limit_mb":1}})
        )
        .await["status"],
        "success"
    );
    let saved: Value =
        serde_json::from_str(&ctx.db.get_state("system_config").unwrap().unwrap()).unwrap();
    assert_eq!(saved["server"]["port"], 19876);
    assert_eq!(saved["llm"]["api_key"], "llm-test-secret");
    assert!(saved.get("obsidian").is_none());
    assert_eq!(
        ctx.db
            .get_state("timeline_legacy_directory")
            .unwrap()
            .as_deref(),
        Some("/legacy")
    );
    assert_eq!(
        ctx.memo_manager
            .images
            .stats()
            .await
            .unwrap()
            .cache_limit_bytes,
        1024 * 1024
    );
}
