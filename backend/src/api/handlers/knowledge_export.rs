use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use tokio_util::io::ReaderStream;

use crate::core::book_wiki_export::{export_book_wiki, BookWikiExportFormat};
use crate::error::BrainError;
use crate::AppContext;

pub async fn download_book_wiki_export(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, format)): Path<(String, String)>,
) -> Result<Response, BrainError> {
    let format = BookWikiExportFormat::parse(&format)?;
    let db = ctx.db.clone();
    let exported = tokio::task::spawn_blocking(move || export_book_wiki(&db, &base_id, format))
        .await
        .map_err(|error| BrainError::Internal(format!("Wiki 导出任务失败: {error}")))??;
    let file = tokio::fs::File::open(&exported.path).await?;
    let disposition = format!(
        "attachment; filename=\"book-wiki.zip\"; filename*=UTF-8''{}",
        urlencoding::encode(&exported.filename)
    );
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&exported.mime_type)
            .map_err(|error| BrainError::Internal(format!("Wiki 导出 MIME 无效: {error}")))?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&exported.size_bytes.to_string())
            .map_err(|error| BrainError::Internal(format!("Wiki 导出大小无效: {error}")))?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|error| BrainError::Internal(format!("Wiki 导出文件名无效: {error}")))?,
    );
    Ok(response)
}
