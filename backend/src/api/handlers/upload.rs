use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::http::header;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

/// Upload images for memo (timeline) feature.
/// Accepts multipart form data with field name "images".
/// Stores images in Obsidian vault under `Timeline/images/`.
/// Returns JSON array of vault-relative paths.
pub async fn upload_images(
    State(ctx): State<Arc<AppContext>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let obsidian = ctx.config.obsidian.enabled.then(|| {
        crate::infra::obsidian_client::ObsidianClient::new(&ctx.config.obsidian)
            .ok()
    }).flatten();

    let obsidian = obsidian.ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Obsidian API 不可用" })),
        )
    })?;

    let now = chrono::Local::now();
    let prefix = now.format("%Y-%m-%d-%H%M%S").to_string();
    let mut paths: Vec<String> = Vec::new();
    let mut idx = 0u32;

    while let Ok(Some(field)) = multipart.next_field().await {
        let content_type = field.content_type().unwrap_or("image/png").to_string();
        let ext = match content_type.as_str() {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            _ => "png",
        };

        let filename = format!("{}-{}.{}", prefix, idx, ext);
        let vault_path = format!("Timeline/images/{}", filename);

        let data = field.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("读取文件失败: {}", e) })),
            )
        })?;

        obsidian
            .write_binary(&vault_path, &data, &content_type)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("写入 Obsidian 失败: {}", e) })),
                )
            })?;

        tracing::info!(path = %vault_path, size = data.len(), "图片上传成功");
        paths.push(vault_path);
        idx += 1;
    }

    Ok(Json(json!({ "paths": paths })))
}

/// Serve a vault image by path.
/// GET /v1/vault/images/*path
pub async fn serve_vault_image(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response, StatusCode> {
    let obsidian = crate::infra::obsidian_client::ObsidianClient::new(&ctx.config.obsidian)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let (bytes, content_type) = obsidian
        .read_binary(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((
        [(header::CONTENT_TYPE, content_type)],
        bytes,
    )
        .into_response())
}
