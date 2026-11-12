use crate::{
    error::BrainError,
    infra::{epub, reader_progress_store::belongs},
    AppContext,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};

#[cfg(test)]
#[path = "reader_epub_tests.rs"]
mod tests;

#[derive(Deserialize)]
pub struct EpubQuery {
    pub path: String,
}
fn err(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"message":e.to_string()})),
    )
        .into_response()
}
fn authorized(ctx: &AppContext, id: &str, path: &str) -> Result<PathBuf, BrainError> {
    let invalid = || BrainError::KnowledgeValidation("EPUB 路径无效或不属于书架中的书籍".into());
    let (root, kind) = ctx.db.with_connection(|c| {
        c.query_row(
            "SELECT path,kind FROM reader_books WHERE id=?1 AND shelf_state='active'",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(invalid)
    })?;
    if !belongs(path, &root, &kind)? || !path.to_ascii_lowercase().ends_with(".epub") {
        return Err(invalid());
    }
    let file = PathBuf::from(path);
    if !file.is_absolute() {
        return Err(invalid());
    }
    let real = file.canonicalize()?;
    let root = PathBuf::from(root).canonicalize()?;
    if (kind == "folder" && !real.starts_with(&root)) || (kind != "folder" && real != root) {
        return Err(invalid());
    }
    Ok(real)
}
pub async fn metadata(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<String>,
    Query(query): Query<EpubQuery>,
) -> Response {
    let task = tokio::task::spawn_blocking(move || {
        let path = authorized(&ctx, &id, &query.path)?;
        let book = epub::metadata(&path)?;
        let resource_base = format!(
            "/v1/reader/epub/{}/{}/",
            urlencoding::encode(&id),
            hex::encode(query.path.as_bytes())
        );
        Ok::<_, BrainError>(serde_json::json!({"book":book,"resource_base":resource_base}))
    })
    .await;
    match task {
        Ok(Ok(book)) => Json(book).into_response(),
        Ok(Err(e)) => err(e),
        Err(e) => err(e),
    }
}
pub async fn resource(
    State(ctx): State<Arc<AppContext>>,
    Path((id, token, entry)): Path<(String, String, String)>,
) -> Response {
    let task = tokio::task::spawn_blocking(move || {
        if token.len() > 16384 {
            return Err(BrainError::KnowledgeValidation("EPUB 路径过长".into()));
        }
        let decoded =
            hex::decode(token).map_err(|e| BrainError::KnowledgeValidation(e.to_string()))?;
        let path = String::from_utf8(decoded)
            .map_err(|e| BrainError::KnowledgeValidation(e.to_string()))?;
        let path = authorized(&ctx, &id, &path)?;
        epub::resource(&path, &entry)
    })
    .await;
    match task {
        Ok(Ok((bytes, mime))) => {
            let policy="default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; sandbox allow-same-origin";
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
                    (
                        header::CACHE_CONTROL,
                        "private, max-age=0, must-revalidate".into(),
                    ),
                    (header::CONTENT_SECURITY_POLICY, policy.into()),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(Err(e)) => err(e),
        Err(e) => err(e),
    }
}
