use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api::handlers::agent_mcp::agent_mcp;
use crate::api::handlers::health::health_check;
use crate::api::handlers::knowledge_artifact::download_knowledge_artifact;
use crate::api::handlers::knowledge_backup::{
    download_knowledge_backup, upload_and_restore_knowledge_backup,
};
use crate::api::handlers::knowledge_chat::stream_knowledge_chat;
use crate::api::handlers::knowledge_export::download_book_wiki_export;
use crate::api::handlers::knowledge_skill::upload_wiki_skill_archive;
use crate::api::handlers::reader_file::serve_reader_file;
use crate::api::handlers::tool_handler::{call_tool, list_tools};
use crate::api::handlers::upload::{serve_thumbnail, serve_vault_image, upload_images};
use crate::frontend_assets;
use crate::AppContext;

/// Create the application router with all routes, middleware, and fallbacks.
pub fn create_router(ctx: Arc<AppContext>) -> Router {
    // CORS — permissive for local development (allow all origins).
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes under /v1 prefix.
    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/tools", get(list_tools))
        .route("/tools/call", post(call_tool))
        .route("/upload/images", post(upload_images))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB for image uploads
        .route("/vault/images/*path", get(serve_vault_image))
        .route("/vault/thumbnails/*path", get(serve_thumbnail))
        .route("/reader/raw", get(serve_reader_file))
        .route("/knowledge/agent-mcp", post(agent_mcp))
        .route("/knowledge/chat/stream", post(stream_knowledge_chat))
        .route(
            "/knowledge/artifacts/:artifact_id",
            get(download_knowledge_artifact),
        )
        .route(
            "/knowledge/backups/:filename",
            get(download_knowledge_backup),
        )
        .route(
            "/knowledge/backups/restore/upload",
            post(upload_and_restore_knowledge_backup)
                .layer(DefaultBodyLimit::max(2 * 1024 * 1024 * 1024)),
        )
        .route(
            "/knowledge/bases/:base_id/export/:format",
            get(download_book_wiki_export),
        )
        .route("/knowledge/skills/import", post(upload_wiki_skill_archive))
        .with_state(ctx);

    // Compose the main router with middleware.
    let app = Router::new()
        .nest("/v1", api_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Serve embedded frontend assets as the fallback (SPA routing).
    tracing::info!("Serving embedded frontend assets");
    app.fallback(serve_frontend_asset)
}

/// Fallback handler — serves embedded frontend files (SPA routing).
async fn serve_frontend_asset(req: axum::extract::Request) -> axum::response::Response {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    frontend_assets::serve_asset(path)
}
