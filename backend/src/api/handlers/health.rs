use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppContext;

pub async fn health_check(State(ctx): State<Arc<AppContext>>) -> Json<Value> {
    let components = ctx.components.lock().unwrap();

    // Live check on SQLite and Tantivy
    let sqlite_status = if ctx.db.health_check() {
        components.sqlite.clone()
    } else {
        "unhealthy".to_string()
    };

    let tantivy_status = if ctx.tantivy.health_check() {
        components.tantivy.clone()
    } else {
        "unhealthy".to_string()
    };

    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "server": components.server,
            "sqlite": sqlite_status,
            "qdrant": components.qdrant,
            "tantivy": tantivy_status,
            "embedding": components.embedding,
            "llm": components.llm,
        }
    }))
}
