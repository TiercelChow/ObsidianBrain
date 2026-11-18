use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::error::BrainError;
use crate::models::wiki_review::{TopicReviewReport, TopicReviewStatus};
use crate::AppContext;

pub(super) async fn read(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<TopicReviewStatus>, BrainError> {
    Ok(Json(ctx.book_wiki_service.topic_review_status(&base_id)?))
}

pub(super) async fn run(
    State(ctx): State<Arc<AppContext>>,
    Path(base_id): Path<String>,
) -> Result<Json<TopicReviewReport>, BrainError> {
    Ok(Json(
        ctx.book_wiki_service.review_wiki_topics(&base_id).await?,
    ))
}
