use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::response::Json;
use serde_json::{json, Value};

use crate::core::skill_archive::parse_skill_archive;
use crate::error::BrainError;
use crate::AppContext;

const MAX_ARCHIVE_BYTES: usize = 5 * 1024 * 1024;

pub async fn upload_wiki_skill_archive(
    State(ctx): State<Arc<AppContext>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, BrainError> {
    let mut archive_bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| BrainError::KnowledgeValidation(format!("Skill 上传读取失败: {error}")))?
    {
        if field.name() != Some("archive") {
            continue;
        }
        let bytes = field.bytes().await.map_err(|error| {
            BrainError::KnowledgeValidation(format!("Skill ZIP 读取失败: {error}"))
        })?;
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(BrainError::KnowledgeValidation(
                "Skill ZIP 不能超过 5 MB".to_string(),
            ));
        }
        archive_bytes = Some(bytes.to_vec());
        break;
    }
    let archive_bytes = archive_bytes
        .ok_or_else(|| BrainError::KnowledgeValidation("请选择要导入的 Skill ZIP".to_string()))?;
    let store = ctx.book_wiki_service.store().clone();
    let skill = tokio::task::spawn_blocking(move || {
        let imported = parse_skill_archive(&archive_bytes)?;
        store.import_custom_wiki_skill(
            &imported.slug,
            &imported.name,
            &imported.description,
            &imported.files,
        )
    })
    .await
    .map_err(|error| BrainError::Internal(format!("Skill 导入任务失败: {error}")))??;
    Ok(Json(json!({ "skill": skill })))
}
