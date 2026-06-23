use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api::handlers::health::health_check;
use crate::api::handlers::tool_handler::{call_tool, list_tools};
use crate::api::handlers::upload::{upload_images, serve_vault_image, serve_thumbnail};
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
        .with_state(ctx);

    // Compose the main router with middleware.
    let app = Router::new()
        .nest("/v1", api_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Static file fallback: serve frontend build output if the directory exists.
    let frontend_dist = std::path::Path::new("../frontend/dist");
    if frontend_dist.exists() {
        tracing::info!(
            path = %frontend_dist.display(),
            "Serving frontend static files"
        );
        app.fallback_service(tower_http::services::ServeDir::new(frontend_dist).fallback(
            tower_http::services::ServeFile::new(frontend_dist.join("index.html")),
        ))
    } else {
        tracing::warn!(
            path = %frontend_dist.display(),
            "Frontend dist directory not found; unmatched routes will return 404"
        );
        app
    }
}
