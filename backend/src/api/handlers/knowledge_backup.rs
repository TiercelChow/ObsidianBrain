use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Json, Response};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use crate::error::BrainError;
use crate::AppContext;

const MAX_BACKUP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub async fn download_knowledge_backup(
    State(ctx): State<Arc<AppContext>>,
    Path(filename): Path<String>,
) -> Result<Response, BrainError> {
    let path = ctx.db.managed_backup_path(&filename)?;
    let file = tokio::fs::File::open(&path).await?;
    let size = file.metadata().await?.len();
    let disposition = format!(
        "attachment; filename=\"brain-backup.sqlite3\"; filename*=UTF-8''{}",
        urlencoding::encode(&filename)
    );
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.sqlite3"),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&size.to_string())
            .map_err(|error| BrainError::Internal(format!("备份文件大小无效: {error}")))?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|error| BrainError::Internal(format!("备份文件名无效: {error}")))?,
    );
    Ok(response)
}

pub async fn upload_and_restore_knowledge_backup(
    State(ctx): State<Arc<AppContext>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, BrainError> {
    let temp_dir = tempfile::tempdir()?;
    let upload_path = temp_dir.path().join("restore-upload.sqlite3");
    let mut uploaded = false;
    let mut confirmation = String::new();

    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        BrainError::KnowledgeValidation(format!("恢复文件上传读取失败: {error}"))
    })? {
        match field.name() {
            Some("confirmation") => {
                confirmation = field.text().await.map_err(|error| {
                    BrainError::KnowledgeValidation(format!("恢复确认信息读取失败: {error}"))
                })?;
            }
            Some("backup") if !uploaded => {
                let mut output = tokio::fs::File::create(&upload_path).await?;
                let mut total = 0_u64;
                while let Some(chunk) = field.chunk().await.map_err(|error| {
                    BrainError::KnowledgeValidation(format!("恢复文件分块读取失败: {error}"))
                })? {
                    total = total.saturating_add(chunk.len() as u64);
                    if total > MAX_BACKUP_BYTES {
                        return Err(BrainError::KnowledgeValidation(
                            "数据库恢复文件不能超过 2 GB".to_string(),
                        ));
                    }
                    output.write_all(&chunk).await?;
                }
                output.flush().await?;
                uploaded = true;
            }
            _ => {}
        }
    }

    if !uploaded {
        return Err(BrainError::KnowledgeValidation(
            "请选择要恢复的 SQLite 备份文件".to_string(),
        ));
    }
    let db = ctx.db.clone();
    let retention = db.backup_retention();
    // Prevent image GC and memo writes from racing a database replacement.
    let _image_guard = ctx.memo_manager.images.mutation.lock().await;
    let report = tokio::task::spawn_blocking(move || {
        db.restore_database_file(&upload_path, confirmation.trim(), retention)
    })
    .await
    .map_err(|error| BrainError::Internal(format!("恢复任务失败: {error}")))??;
    ctx.book_wiki_service
        .store()
        .rebuild_knowledge_search_indexes()?;
    let budget = ctx
        .db
        .get_state("timeline_cache_limit_bytes")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(crate::infra::timeline_images::DEFAULT_CACHE_BYTES);
    ctx.memo_manager.images.set_budget(budget).await?;
    Ok(Json(json!({ "validation": report })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::api::router::create_router;

    #[tokio::test]
    async fn test_download_knowledge_backup_serves_only_managed_snapshot() {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        let backup = ctx.db.create_managed_backup("download-test", 7).unwrap();
        let app = create_router(ctx);
        let response = app
            .oneshot(
                Request::get(format!("/v1/knowledge/backups/{}", backup.filename))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/vnd.sqlite3"
        );
        assert!(!to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty());
    }
}
