use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_attachment_http_upload_list_download_remove_and_scope() {
    let (ctx, _dir, _vault) = AppContext::for_test();
    ctx.db.with_connection(|c|{c.execute_batch("INSERT INTO task_documents(path,document_kind,revision,content_hash,indexed_at) VALUES ('test','long_task',1,'test','now');
        INSERT INTO task_nodes(id,root_id,storage_path,kind,role,title,description,status,importance,start_date,end_date,position,created_at,updated_at,revision) VALUES ('task','task','test','long','root','test','','planned','normal','2026-01-01','2026-12-31',0,'now','now',1);
        INSERT INTO task_progress(id,root_id,task_id,storage_path,recorded_at,note,created_at) VALUES ('progress','task','task','test','now','note','now');")?;Ok(())}).unwrap();
    let app = crate::api::router::create_router(ctx.clone());
    let body="--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"notes.txt\"\r\nContent-Type: text/plain\r\n\r\nattachment bytes\r\n--boundary--\r\n";
    let request = |scope: &str| {
        Request::builder()
            .method("POST")
            .uri(format!(
                "/v1/tasks/task/attachments?progress_id={scope}&upload_key=stable-upload"
            ))
            .header("content-type", "multipart/form-data; boundary=boundary")
            .body(Body::from(body))
            .unwrap()
    };
    let denied = app.clone().oneshot(request("wrong")).await.unwrap();
    assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
    let response = app.clone().oneshot(request("progress")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let item: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    let id = item["id"].as_str().unwrap();
    let retry = app.clone().oneshot(request("progress")).await.unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    let retried: serde_json::Value =
        serde_json::from_slice(&to_bytes(retry.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(retried["id"], id);
    let get = |path: &str| Request::builder().uri(path).body(Body::empty()).unwrap();
    let response = app
        .clone()
        .oneshot(get("/v1/tasks/task/attachments?progress_id=progress"))
        .await
        .unwrap();
    let list: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(list[0]["id"], id);
    let response = app
        .clone()
        .oneshot(get(&format!("/v1/tasks/attachments/{id}")))
        .await
        .unwrap();
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"attachment\"; filename*=UTF-8''notes.txt"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        &to_bytes(response.into_body(), 4096).await.unwrap()[..],
        b"attachment bytes"
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/tasks/attachments/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!TaskAttachmentStore::new(ctx.db.clone())
        .file_path(id)
        .unwrap()
        .exists());
}
