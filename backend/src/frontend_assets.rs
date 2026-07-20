//! Embedded frontend assets — the Vue build output is compiled into the binary
//! via `rust-embed`, so the single binary serves the frontend without external files.

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

/// Serve a file from the embedded frontend assets.
/// Falls back to `index.html` for unknown paths (SPA routing).
pub fn serve_asset(path: &str) -> Response {
    // Try the exact path first.
    if let Some(file) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_str())],
            [(header::CACHE_CONTROL, "no-cache")],
            Body::from(file.data.into_owned()),
        )
            .into_response();
    }

    // Fallback to index.html for SPA routing — always serve as text/html.
    if let Some(file) = FrontendAssets::get("index.html") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            [(header::CACHE_CONTROL, "no-cache")],
            Body::from(file.data.into_owned()),
        )
            .into_response();
    }

    (StatusCode::NOT_FOUND, "Not found").into_response()
}
