//! Reader binary file endpoint: `GET /v1/reader/raw?path=<abs>`.
//!
//! Serves arbitrary local files as raw bytes with correct Content-Type and
//! HTTP Range support. pdf.js uses range requests to stream large PDFs, so
//! Range requests are served via `File::seek` + `read_exact` (only the
//! requested bytes are read — never the whole file). Full-file (no-Range)
//! requests are streamed via `ReaderStream` so memory use stays bounded
//! regardless of file size. Path must be absolute, contain no `..`, and be
//! a file. No fixed size cap (streaming makes one unnecessary).

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::io::SeekFrom;
use std::path::{Component, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::AppContext;

#[derive(Deserialize)]
pub struct ReaderRawQuery {
    pub path: String,
}

/// Validate a local path: absolute, no `..` traversal, exists, is a file.
fn validate_path(raw: &str) -> Result<PathBuf, (StatusCode, String)> {
    let p = PathBuf::from(raw);
    if !p.is_absolute() {
        return Err((StatusCode::BAD_REQUEST, "路径必须是绝对路径".to_string()));
    }
    for comp in p.components() {
        if matches!(comp, Component::ParentDir) {
            return Err((StatusCode::BAD_REQUEST, "路径禁止包含 `..`".to_string()));
        }
    }
    let meta = match std::fs::metadata(&p) {
        Ok(m) => m,
        Err(_) => return Err((StatusCode::NOT_FOUND, "文件不存在".to_string())),
    };
    if !meta.is_file() {
        return Err((StatusCode::BAD_REQUEST, "路径不是文件".to_string()));
    }
    Ok(p)
}

/// Content-Type by extension (PDF and common image formats; others octet-stream).
fn content_type_for(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Parse a single-range `bytes=start-end` header into (start, end_inclusive).
/// `bytes=0-` (open-ended) and `bytes=0-99` are supported; suffix `bytes=-99` is
/// not (pdf.js uses start-end form). Returns None on malformed input.
fn parse_range(range: &str, total: u64) -> Option<(u64, u64)> {
    let bytes = range.strip_prefix("bytes=")?;
    let (s, e) = bytes.split_once('-')?;
    let start: u64 = s.parse().ok()?;
    let end: u64 = if e.is_empty() {
        total.saturating_sub(1)
    } else {
        e.parse().ok()?
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end.min(total - 1)))
}

/// Read the inclusive byte range `[start, end]` from an open file via seek.
/// Reads only the requested bytes — the rest of the file is never loaded. Safe
/// against the file shrinking between the length check and the read:
/// `read_exact` returns `UnexpectedEof` (an error) rather than panicking.
async fn read_range(file: &mut tokio::fs::File, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(start)).await?;
    let len = (end - start + 1) as usize;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).await?;
    Ok(buf)
}

/// `GET /v1/reader/raw?path=<abs>` — serve a local file as raw bytes with Range.
pub async fn serve_reader_file(
    State(_ctx): State<Arc<AppContext>>,
    Query(q): Query<ReaderRawQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let path = validate_path(&q.path)?;
    let mut file = tokio::fs::File::open(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("打开文件失败: {e}"),
        )
    })?;
    // Length from the open handle (fstat) — closer to the read than a separate
    // metadata call. Used only for Range parsing + headers; read_range reads
    // directly from the handle, so a stale length can't panic (read_exact errors).
    let total = file.metadata().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取元数据失败: {e}"),
        )
    })?;
    let total = total.len();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let ct = content_type_for(ext);

    if let Some(range_hdr) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range_hdr, total) {
            // Seek + read only the requested range — no full-file load. This is
            // the path pdf.js hits for every chunk, so large PDFs stream without
            // bounding memory. (A malformed/unsatisfiable Range falls through to
            // the full-file stream below, per RFC 7233 §4.2.)
            let buf = read_range(&mut file, start, end).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("读取范围失败: {e}"),
                )
            })?;
            return Ok((
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE, ct.to_string()),
                    (
                        header::CONTENT_RANGE,
                        format!("bytes {start}-{end}/{total}"),
                    ),
                    (header::CONTENT_LENGTH, (end - start + 1).to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                ],
                buf,
            )
                .into_response());
        }
    }

    // No Range (or malformed Range ignored per RFC): stream the whole file so
    // memory stays bounded for large files (no Vec holding the entire content).
    let stream = ReaderStream::new(file);
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, ct.to_string()),
            (header::CONTENT_LENGTH, total.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use tower::ServiceExt;

    fn write_tmp(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        (dir, p)
    }

    #[test]
    fn test_validate_path_rejects_relative() {
        let err = validate_path("relative/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_path_rejects_parent_dir() {
        let err = validate_path("/Users/x/../y/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_path_rejects_missing() {
        let err = validate_path("/nonexistent/absolute/path/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_validate_path_accepts_existing_file() {
        let (_d, p) = write_tmp("file.pdf", b"%PDF-1.4");
        let got = validate_path(p.to_str().unwrap()).unwrap();
        assert_eq!(got, p);
    }

    #[test]
    fn test_content_type_pdf() {
        assert_eq!(content_type_for("pdf"), "application/pdf");
        assert_eq!(content_type_for("PDF"), "application/pdf");
        assert_eq!(content_type_for("txt"), "application/octet-stream");
    }

    #[test]
    fn test_content_type_images() {
        // Markdown-rendered local images load through this endpoint; without a
        // real image/* Content-Type browsers (esp. for svg) refuse to decode.
        assert_eq!(content_type_for("png"), "image/png");
        assert_eq!(content_type_for("jpg"), "image/jpeg");
        assert_eq!(content_type_for("JPG"), "image/jpeg");
        assert_eq!(content_type_for("jpeg"), "image/jpeg");
        assert_eq!(content_type_for("gif"), "image/gif");
        assert_eq!(content_type_for("webp"), "image/webp");
        assert_eq!(content_type_for("svg"), "image/svg+xml");
        assert_eq!(content_type_for("bmp"), "image/bmp");
        assert_eq!(content_type_for("avif"), "image/avif");
        assert_eq!(content_type_for("ico"), "image/x-icon");
    }

    #[test]
    fn test_parse_range_full_open() {
        // bytes=0- on a 1000-byte file → 0..=999
        assert_eq!(parse_range("bytes=0-", 1000), Some((0, 999)));
    }

    #[test]
    fn test_parse_range_partial() {
        assert_eq!(parse_range("bytes=100-199", 1000), Some((100, 199)));
    }

    #[test]
    fn test_parse_range_clamps_end() {
        // end beyond total clamps to total-1
        assert_eq!(parse_range("bytes=900-2000", 1000), Some((900, 999)));
    }

    #[test]
    fn test_parse_range_rejects_out_of_range_start() {
        assert_eq!(parse_range("bytes=2000-", 1000), None);
    }

    #[test]
    fn test_parse_range_rejects_malformed() {
        assert_eq!(parse_range("items=0-100", 1000), None);
        assert_eq!(parse_range("bytes=abc", 1000), None);
    }

    #[tokio::test]
    async fn test_read_range_returns_exact_slice() {
        let (_d, p) = write_tmp("file.bin", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let mut file = tokio::fs::File::open(&p).await.unwrap();
        // bytes 2..=5 → [2, 3, 4, 5]
        let buf = read_range(&mut file, 2, 5).await.unwrap();
        assert_eq!(buf, vec![2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_read_range_full_file() {
        let (_d, p) = write_tmp("file.bin", b"hello");
        let mut file = tokio::fs::File::open(&p).await.unwrap();
        let buf = read_range(&mut file, 0, 4).await.unwrap();
        assert_eq!(buf, b"hello");
    }

    #[tokio::test]
    async fn test_read_range_start_zero() {
        let (_d, p) = write_tmp("file.bin", b"abcdef");
        let mut file = tokio::fs::File::open(&p).await.unwrap();
        // bytes 0..=2 → [a, b, c] (pdf.js probes from byte 0)
        let buf = read_range(&mut file, 0, 2).await.unwrap();
        assert_eq!(buf, b"abc");
    }

    #[tokio::test]
    async fn test_read_range_beyond_eof_errors_not_panics() {
        // TOCTOU guard: if the file shrank so the requested end is past EOF,
        // read_exact must return an error, not panic.
        let (_d, p) = write_tmp("file.bin", b"abc");
        let mut file = tokio::fs::File::open(&p).await.unwrap();
        // Request 0..=10 on a 3-byte file.
        let res = read_range(&mut file, 0, 10).await;
        assert!(res.is_err(), "expected UnexpectedEof error, got {res:?}");
    }

    // ── Handler-level e2e (closes Minor #4: 206 / 200 / 413 integration) ──

    /// Minimal AppContext wired into the real router — same pattern as
    /// tool_handler tests. The context's own tempdir is dropped here (db file
    /// unlinked-but-open on Unix); each test's `(_d, p)` keeps ITS file alive.
    fn make_app() -> Router {
        let (ctx, _dir, _vault) = crate::AppContext::for_test();
        crate::api::router::create_router(ctx)
    }

    /// Build `/v1/reader/raw?path=<encoded>` for a temp file path.
    fn raw_uri(path: &std::path::Path) -> String {
        let lossy = path.to_string_lossy();
        let encoded = urlencoding::encode(&lossy);
        format!("/v1/reader/raw?path={encoded}")
    }

    #[tokio::test]
    async fn test_serve_reader_file_range_returns_206() {
        // pdf.js probes bytes 0-3 (the %PDF magic) on its first Range request.
        let content = b"%PDF-1.4 lorem ipsum content!";
        let (_d, p) = write_tmp("file.pdf", content);
        let app = make_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(raw_uri(&p))
                    .header(header::RANGE, "bytes=0-3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let h = response.headers();
        assert_eq!(h.get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "application/pdf");
        assert_eq!(
            h.get(header::CONTENT_RANGE).unwrap(),
            &format!("bytes 0-3/{}", content.len())
        );
        assert_eq!(h.get(header::CONTENT_LENGTH).unwrap(), "4");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"%PDF");
    }

    #[tokio::test]
    async fn test_serve_reader_file_no_range_returns_200_streamed() {
        let content = b"%PDF-1.4 hello world";
        let (_d, p) = write_tmp("file.pdf", content);
        let app = make_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(raw_uri(&p))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let h = response.headers();
        assert_eq!(h.get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "application/pdf");
        assert_eq!(
            h.get(header::CONTENT_LENGTH).unwrap(),
            &content.len().to_string()
        );
        // Body::from_stream path — verify the streamed bytes match the file.
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), content);
    }

    #[tokio::test]
    async fn test_serve_reader_file_large_file_not_413() {
        // Regression for the reported 413: a 111 MB PDF exceeded the old
        // 100 MB cap (which existed only because the handler read the whole
        // file into memory). The streaming rewrite removed the cap entirely.
        // Create a 116 MB sparse file (instant, ~zero disk) and assert a
        // Range probe returns 206, not 413.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("huge.pdf");
        {
            let f = std::fs::File::create(&p).unwrap();
            f.set_len(116_000_000).unwrap(); // 116 MB, > old 100 MB cap
        }
        let app = make_app();
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(raw_uri(&p))
                    .header(header::RANGE, "bytes=0-99")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "large file must not be 413'd — the cap was removed"
        );
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 0-99/116000000"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "100"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.len(), 100);
        // `dir` (holding huge.pdf) dropped here, after the request consumed it.
        drop(dir);
    }
}
