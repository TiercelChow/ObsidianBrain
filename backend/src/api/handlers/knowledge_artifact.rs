use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use tokio_util::io::ReaderStream;

use crate::error::BrainError;
use crate::AppContext;

pub async fn download_knowledge_artifact(
    State(ctx): State<Arc<AppContext>>,
    Path(artifact_id): Path<String>,
) -> Result<Response, BrainError> {
    let (path, mime_type, title) = ctx.book_wiki_service.artifact_path(&artifact_id)?;
    let file = tokio::fs::File::open(&path).await?;
    let size = file.metadata().await?.len();
    let filename = format!("{}.pptx", title.trim_end_matches(".pptx"));
    let disposition = format!(
        "attachment; filename=\"knowledge-artifact.pptx\"; filename*=UTF-8''{}",
        urlencoding::encode(&filename)
    );
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime_type)
            .map_err(|error| BrainError::Internal(format!("成果 MIME 类型无效: {error}")))?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&size.to_string())
            .map_err(|error| BrainError::Internal(format!("成果文件大小无效: {error}")))?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|error| BrainError::Internal(format!("成果文件名无效: {error}")))?,
    );
    Ok(response)
}
