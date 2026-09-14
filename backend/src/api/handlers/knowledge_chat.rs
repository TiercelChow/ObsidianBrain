use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::Stream;
use serde::Deserialize;

use crate::core::book_wiki::KnowledgeChatStreamEvent;
use crate::AppContext;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeChatStreamRequest {
    pub knowledge_base_id: String,
    pub question: String,
    pub conversation_id: Option<String>,
}

pub async fn stream_knowledge_chat(
    State(ctx): State<Arc<AppContext>>,
    Json(request): Json<KnowledgeChatStreamRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (events, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let result = ctx
            .book_wiki_service
            .ask_streaming(
                &request.knowledge_base_id,
                &request.question,
                request.conversation_id.as_deref(),
                events.clone(),
            )
            .await;
        let terminal = match result {
            Ok(result) => KnowledgeChatStreamEvent::Completed { result },
            Err(error) => KnowledgeChatStreamEvent::Error {
                message: error.to_string(),
            },
        };
        let _ = events.send(terminal);
    });

    let stream = futures::stream::unfold(receiver, |mut receiver| async move {
        let event = receiver.recv().await?;
        let data = serde_json::to_string(&event).unwrap_or_else(|error| {
            format!(
                r#"{{"type":"error","message":"事件序列化失败：{}"}}"#,
                error
            )
        });
        Some((Ok(Event::default().data(data)), receiver))
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
