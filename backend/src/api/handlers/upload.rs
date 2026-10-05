use crate::{error::BrainError, AppContext};
use axum::extract::{Multipart, Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};
use std::sync::Arc;

/// Originals live under the app's data directory, never in an external vault.
pub async fn upload_images(
    State(ctx): State<Arc<AppContext>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, BrainError> {
    let mut paths = Vec::new();
    let upload = async {
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| BrainError::MemoValidation(format!("读取上传内容失败: {e}")))?
        {
            if field.name() != Some("images") {
                return Err(BrainError::MemoValidation("仅接受 images 图片字段".into()));
            }
            if paths.len() >= 9 {
                return Err(BrainError::MemoValidation("一次最多上传 9 张图片".into()));
            }
            let bytes = field
                .bytes()
                .await
                .map_err(|e| BrainError::MemoValidation(format!("读取图片失败: {e}")))?;
            paths.push(ctx.memo_manager.images.upload(&bytes).await?);
        }
        if paths.is_empty() {
            return Err(BrainError::MemoValidation("没有图片".into()));
        }
        Ok(())
    }
    .await;
    if let Err(error) = upload {
        if let Err(cleanup) = ctx.memo_manager.images.discard(&paths).await {
            tracing::warn!(%cleanup,"失败上传清理待重试");
        }
        return Err(error);
    }
    Ok(Json(json!({"paths":paths})))
}

pub async fn serve_image(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response, BrainError> {
    let (bytes, mime) = ctx.memo_manager.images.original(&path).await?;
    Ok(image_response(bytes, mime))
}

pub async fn serve_thumbnail(
    State(ctx): State<Arc<AppContext>>,
    Path(path): Path<String>,
) -> Result<Response, BrainError> {
    let (bytes, mime) = ctx.memo_manager.images.thumbnail(&path).await?;
    Ok(image_response(bytes, mime))
}

fn image_response(bytes: Vec<u8>, mime: String) -> Response {
    // Browser storage is deliberately not a second, unbounded persistent image cache.
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-store".into()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        bytes,
    )
        .into_response()
}
