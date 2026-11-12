use crate::{
    infra::task_attachments::{TaskAttachmentStore, MAX_ATTACHMENT_BYTES},
    AppContext,
};

#[cfg(test)]
#[path = "task_attachment_tests.rs"]
mod tests;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

#[derive(Deserialize)]
pub struct AttachmentScope {
    pub progress_id: Option<String>,
    pub upload_key: Option<String>,
}
fn error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"message":e.to_string()})),
    )
        .into_response()
}

pub async fn list(
    State(ctx): State<Arc<AppContext>>,
    Path(task): Path<String>,
    Query(scope): Query<AttachmentScope>,
) -> Response {
    match TaskAttachmentStore::new(ctx.db.clone()).list(&task, scope.progress_id.as_deref()) {
        Ok(items) => Json(items).into_response(),
        Err(e) => error(e),
    }
}
pub async fn upload(
    State(ctx): State<Arc<AppContext>>,
    Path(task): Path<String>,
    Query(scope): Query<AttachmentScope>,
    mut multipart: Multipart,
) -> Response {
    let store = TaskAttachmentStore::new(ctx.db.clone());
    let mut field = match multipart.next_field().await {
        Ok(Some(f)) if f.name() == Some("file") => f,
        Ok(_) => return error("请选择附件文件"),
        Err(e) => return error(e),
    };
    let filename = field.file_name().unwrap_or("").to_string();
    let reservation = if let Some(key) = scope.upload_key.as_deref() {
        store.reserve_with_key(&task, scope.progress_id.as_deref(), &filename, key)
    } else {
        store
            .reserve(&task, scope.progress_id.as_deref(), &filename)
            .map(|item| (item, true))
    };
    let item = match reservation {
        Ok((v, true)) => v,
        Ok((v, false)) => return Json(v).into_response(),
        Err(e) => return error(e),
    };
    // Guard is also dropped on a cancelled request. A durable uploading row
    // covers process crashes, and is recovered once at application startup.
    struct Pending<'a> {
        store: &'a TaskAttachmentStore,
        id: String,
        complete: bool,
    }
    impl Drop for Pending<'_> {
        fn drop(&mut self) {
            if !self.complete {
                if let Err(e) = self.store.delete(&self.id) {
                    tracing::warn!(error=%e,"清理中断附件失败");
                }
            }
        }
    }
    let mut pending = Pending {
        store: &store,
        id: item.id.clone(),
        complete: false,
    };
    let result: Result<u64, String> = async {
        tokio::fs::create_dir_all(store.directory())
            .await
            .map_err(|e| e.to_string())?;
        let path = store.file_path(&item.id).map_err(|e| e.to_string())?;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map_err(|e| e.to_string())?;
        let mut size = 0u64;
        while let Some(chunk) = field.chunk().await.map_err(|e| e.to_string())? {
            size += chunk.len() as u64;
            if size > MAX_ATTACHMENT_BYTES {
                return Err("单个附件最大 50 MiB".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        drop(field);
        store.finish(&item.id, size).map_err(|e| e.to_string())?;
        Ok(size)
    }
    .await;
    match result {
        Ok(size) => {
            pending.complete = true;
            Json(crate::infra::task_attachments::TaskAttachment {
                size_bytes: size,
                ..item
            })
            .into_response()
        }
        Err(e) => error(e),
    }
}
pub async fn download(State(ctx): State<Arc<AppContext>>, Path(id): Path<String>) -> Response {
    let store = TaskAttachmentStore::new(ctx.db.clone());
    let item = match store.get(&id) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let path = match store.file_path(&id) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let file = match tokio::fs::File::open(path).await {
        Ok(v) => v,
        Err(_) => return (StatusCode::NOT_FOUND, "附件文件不存在").into_response(),
    };
    let disposition = format!(
        "attachment; filename=\"attachment\"; filename*=UTF-8''{}",
        urlencoding::encode(&item.filename)
    );
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_LENGTH, item.size_bytes.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response()
}
pub async fn remove(State(ctx): State<Arc<AppContext>>, Path(id): Path<String>) -> Response {
    let store = TaskAttachmentStore::new(ctx.db.clone());
    if let Err(e) = store.get(&id) {
        return error(e);
    }
    match store.delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error(e),
    }
}
