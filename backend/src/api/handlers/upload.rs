use axum::extract::{Multipart, Path, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

/// Thumbnail max dimension (width or height, whichever is larger)
const THUMBNAIL_MAX_SIZE: u32 = 400;

/// Upload images for memo (timeline) feature.
/// Accepts multipart form data with field name "images".
/// Stores images in Obsidian vault under `Timeline/images/`.
/// Also generates low-res thumbnails in ./data/thumbnails/.
/// Returns JSON array of vault-relative paths.
pub async fn upload_images(
    State(ctx): State<Arc<AppContext>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let obsidian = crate::infra::obsidian_client::get_client(&ctx.obsidian).map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Obsidian API 不可用" })),
        )
    })?;

    let now = chrono::Local::now();
    let prefix = now.format("%Y-%m-%d-%H%M%S").to_string();
    let mut paths: Vec<String> = Vec::new();
    let mut idx = 0u32;

    // Ensure thumbnail directory exists
    let thumb_dir = crate::paths::thumbnails_dir();
    let _ = std::fs::create_dir_all(&thumb_dir);

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

        // Write original to Obsidian
        obsidian
            .write_binary(&vault_path, &data, &content_type)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("写入 Obsidian 失败: {}", e) })),
                )
            })?;

        // Generate thumbnail in background (non-blocking) — skip for SVG
        if ext != "svg" {
            let data_owned = data.to_vec();
            let filename_owned = filename.clone();
            tokio::task::spawn_blocking(move || {
                generate_thumbnail(&data_owned, &filename_owned, ext);
            });
        }

        tracing::info!(path = %vault_path, size = data.len(), "图片上传成功");
        paths.push(vault_path);
        idx += 1;
    }

    Ok(Json(json!({ "paths": paths })))
}

/// Generate a low-resolution thumbnail from image data.
/// Saves to ./data/thumbnails/{filename} as JPEG (smaller size).
fn generate_thumbnail(data: &[u8], filename: &str, _ext: &str) {
    // Change extension to .jpg for thumbnails (smaller file size)
    let thumb_name = filename
        .rsplit_once('.')
        .map(|(stem, _)| format!("{}.jpg", stem))
        .unwrap_or_else(|| format!("{}.jpg", filename));
    let thumb_dir = crate::paths::thumbnails_dir();
    let thumb_path = thumb_dir.join(&thumb_name);

    // Ensure thumbnail directory exists
    if let Err(e) = std::fs::create_dir_all(thumb_dir) {
        tracing::warn!("缩略图目录创建失败: {e}");
        return;
    }

    match image::load_from_memory(data) {
        Ok(img) => {
            // Triangle is ~5x faster than Lanczos3 with negligible quality
            // difference at thumbnail size (400px).
            let thumbnail = img.resize(
                THUMBNAIL_MAX_SIZE,
                THUMBNAIL_MAX_SIZE,
                image::imageops::FilterType::Triangle,
            );
            match thumbnail.save_with_format(&thumb_path, image::ImageFormat::Jpeg) {
                Ok(_) => {
                    tracing::info!(thumb = %thumb_path.display(), "缩略图生成成功");
                }
                Err(e) => {
                    tracing::warn!("缩略图保存失败: {e}");
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "缩略图生成失败，跳过");
        }
    }
}

/// Serve a vault image by path (original).
/// GET /v1/vault/images/*path
pub async fn serve_vault_image(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response, StatusCode> {
    let obsidian = crate::infra::obsidian_client::get_client(&ctx.obsidian)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let (bytes, content_type) = obsidian
        .read_binary(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

/// Serve a thumbnail by path.
/// GET /v1/vault/thumbnails/*path
/// Tries to serve from local ./data/thumbnails/, falls back to original if not found.
pub async fn serve_thumbnail(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response, StatusCode> {
    // Convert vault path to thumbnail filename
    // path = "Timeline/images/2026-06-13-174140-0.jpg"
    // thumbnail = "./data/thumbnails/2026-06-13-174140-0.jpg"
    let filename = path.rsplit('/').next().unwrap_or(&path);
    let thumb_name = filename
        .rsplit_once('.')
        .map(|(stem, _)| format!("{}.jpg", stem))
        .unwrap_or_else(|| format!("{}.jpg", filename));
    let thumb_path = crate::paths::thumbnails_dir().join(&thumb_name);

    // Try serving thumbnail first
    if thumb_path.exists() {
        if let Ok(bytes) = std::fs::read(&thumb_path) {
            return Ok(([(header::CONTENT_TYPE, "image/jpeg".to_string())], bytes).into_response());
        }
    }

    // Fallback: generate thumbnail on-the-fly from original
    let obsidian = crate::infra::obsidian_client::get_client(&ctx.obsidian)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let (data, _) = obsidian
        .read_binary(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Try to generate and cache thumbnail
    generate_thumbnail(&data, &thumb_name, "jpg");

    // Try serving the just-generated thumbnail
    if thumb_path.exists() {
        if let Ok(bytes) = std::fs::read(&thumb_path) {
            return Ok(([(header::CONTENT_TYPE, "image/jpeg".to_string())], bytes).into_response());
        }
    }

    // Final fallback: serve original
    Ok(([(header::CONTENT_TYPE, "image/jpeg".to_string())], data).into_response())
}
