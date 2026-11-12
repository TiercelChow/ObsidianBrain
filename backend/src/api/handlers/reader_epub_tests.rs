use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use tower::ServiceExt;

#[tokio::test]
async fn test_epub_http_metadata_safe_resources_and_shelf_boundary() {
    let (ctx, _dir, _vault) = AppContext::for_test();
    let (_book_dir, path) = epub::tests::book();
    let raw = path.to_string_lossy();
    ctx.db.with_connection(|c|{c.execute("INSERT INTO reader_books(id,path,kind,name,added_at) VALUES ('epub',?1,'epub','Test',1)",[raw.as_ref()])?;Ok(())}).unwrap();
    let app = crate::api::router::create_router(ctx);
    let get = |uri: &str| Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/v1/reader/epub/epub/metadata?path={}",
            urlencoding::encode(&raw)
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let meta: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 32 * 1024).await.unwrap()).unwrap();
    let base = meta["resource_base"].as_str().unwrap();
    assert_eq!(meta["book"]["chapters"][0]["title"], "开始阅读");
    let response = app
        .clone()
        .oneshot(get(&format!("{base}OPS/chapters/one.xhtml")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("sandbox allow-same-origin"));
    let html = String::from_utf8(
        to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("你好"));
    assert!(!html.contains("<script"));
    let response = app
        .oneshot(get(&format!(
            "/v1/reader/epub/other/metadata?path={}",
            urlencoding::encode(&raw)
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
