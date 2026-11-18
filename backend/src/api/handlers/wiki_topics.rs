use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::error::BrainError;
use crate::models::book_wiki::SourceDocumentSummary;
use crate::models::wiki_topics::{
    TopicBookBuild, TopicCompileRequest, TopicPage, TopicPageSummary, TopicRevision,
    TopicSourceSection, TopicWikiIndex, TopicWritebackMaterial, TopicWritebackRequest,
};
use crate::AppContext;

pub fn routes() -> Router<Arc<AppContext>> {
    Router::new()
        .route("/", get(list_pages).post(compile))
        .route("/build", get(read_build).post(start_build))
        .route("/build/:attempt_id/stop", post(stop_build))
        .route("/build/:attempt_id/publish", post(publish_build))
        .route("/index", get(read_index))
        .route(
            "/semantic-review",
            get(super::wiki_topic_review::read).post(super::wiki_topic_review::run),
        )
        .route("/sources", get(list_sources))
        .route("/sources/:document_id", get(list_source_sections))
        .route("/writeback-material/:run_id", get(read_writeback_material))
        .route("/:page_id", get(read_page))
        .route(
            "/:page_id/revisions/:revision_id",
            get(read_revision).put(edit_draft),
        )
        .route("/:page_id/revisions/:revision_id/publish", post(publish))
        .route("/:page_id/writeback-draft", post(draft_from_material))
        .route("/:page_id/questions", post(save_question))
        .route(
            "/:page_id/questions/:question_id/resolve",
            post(resolve_question),
        )
}

async fn read_index(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<TopicWikiIndex>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service.store().topic_wiki_index(&base_id)?,
    ))
}

async fn read_build(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<Option<TopicBookBuild>>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service.store().topic_book_build(&base_id)?,
    ))
}

async fn start_build(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<(axum::http::StatusCode, Json<TopicBookBuild>), BrainError> {
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(ctx.book_wiki_service.enqueue_topic_book_build(&base_id)?),
    ))
}

async fn stop_build(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, attempt_id)): Path<(String, String)>,
) -> Result<Json<TopicBookBuild>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .store()
            .request_topic_book_stop(&base_id, &attempt_id)?,
    ))
}

async fn publish_build(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, attempt_id)): Path<(String, String)>,
) -> Result<Json<usize>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .publish_topic_build(&base_id, &attempt_id)?,
    ))
}

async fn list_pages(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<Vec<TopicPageSummary>>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service.store().list_topic_pages(&base_id)?,
    ))
}

async fn compile(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
    Json(request): Json<TopicCompileRequest>,
) -> Result<Json<TopicPage>, BrainError> {
    let service = ctx.book_wiki_service.clone();
    // A page draft can finish after the client navigates away.
    let page = tokio::spawn(async move { service.compile_topic(&base_id, request).await })
        .await
        .map_err(|error| BrainError::Internal(format!("主题编译执行中断: {error}")))??;
    Ok(Json(page))
}

async fn list_sources(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<Vec<SourceDocumentSummary>>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .store()
            .list_current_source_documents(&base_id)?,
    ))
}

#[derive(Deserialize)]
struct SourceSectionQuery {
    #[serde(default)]
    offset: u32,
}

async fn list_source_sections(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, document_id)): Path<(String, String)>,
    Query(query): Query<SourceSectionQuery>,
) -> Result<Json<Vec<TopicSourceSection>>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service.store().list_topic_source_sections(
            &base_id,
            &document_id,
            query.offset as usize,
            50,
        )?,
    ))
}

async fn read_page(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id)): Path<(String, String)>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .store()
            .topic_page(&base_id, &page_id)?,
    ))
}

async fn read_revision(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id, revision_id)): Path<(String, String, String)>,
) -> Result<Json<TopicRevision>, BrainError> {
    Ok(Json(ctx.book_wiki_service.store().topic_revision(
        &base_id,
        &page_id,
        &revision_id,
    )?))
}

async fn publish(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id, revision_id)): Path<(String, String, String)>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(ctx.book_wiki_service.publish_topic(
        &base_id,
        &page_id,
        &revision_id,
    )?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftEditRequest {
    markdown: String,
}

async fn edit_draft(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id, revision_id)): Path<(String, String, String)>,
    Json(request): Json<DraftEditRequest>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(ctx.book_wiki_service.edit_topic_draft(
        &base_id,
        &page_id,
        &revision_id,
        &request.markdown,
    )?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveQuestionRequest {
    run_id: String,
}

async fn save_question(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id)): Path<(String, String)>,
    Json(request): Json<SaveQuestionRequest>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(ctx.book_wiki_service.store().save_topic_question(
        &base_id,
        &page_id,
        &request.run_id,
    )?))
}

async fn read_writeback_material(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, run_id)): Path<(String, String)>,
) -> Result<Json<TopicWritebackMaterial>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .topic_writeback_material(&base_id, &run_id)?,
    ))
}

async fn draft_from_material(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id)): Path<(String, String)>,
    Json(request): Json<TopicWritebackRequest>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service
            .draft_topic_from_material(&base_id, &page_id, request)?,
    ))
}

async fn resolve_question(
    State(ctx): State<Arc<AppContext>>,
    Path((base_id, page_id, question_id)): Path<(String, String, String)>,
) -> Result<Json<TopicPage>, BrainError> {
    Ok(Json(ctx.book_wiki_service.store().resolve_topic_question(
        &base_id,
        &page_id,
        &question_id,
    )?))
}
