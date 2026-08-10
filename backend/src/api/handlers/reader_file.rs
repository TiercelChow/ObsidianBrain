//! Reader binary file endpoint: `GET /v1/reader/raw?path=<abs>`.
//!
//! Serves arbitrary local files as raw bytes with correct Content-Type and
//! HTTP Range support (pdf.js uses range requests to stream large PDFs).
//! Path must be absolute, contain no `..`, and be a file. 100 MB cap.

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use crate::AppContext;

/// Max file size served by the reader binary endpoint (100 MB).
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

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

/// Content-Type by extension (only PDF is special-cased; others octet-stream).
fn content_type_for(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
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

/// `GET /v1/reader/raw?path=<abs>` — serve a local file as raw bytes with Range.
pub async fn serve_reader_file(
    State(_ctx): State<Arc<AppContext>>,
    Query(q): Query<ReaderRawQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let path = validate_path(&q.path)?;
    let meta = tokio::fs::metadata(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取元数据失败: {e}"),
        )
    })?;
    if meta.len() > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "文件过大 ({:.1} MB)，上限 {} MB",
                meta.len() as f64 / 1_048_576.0,
                MAX_FILE_SIZE / 1_048_576
            ),
        ));
    }
    let total = meta.len();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let ct = content_type_for(ext);

    // Read full file into memory, then slice for Range. Acceptable for a
    // local single-user tool with a 100 MB cap; a future optimization can
    // use File::seek for true streaming.
    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("读取文件失败: {e}"),
        )
    })?;

    if let Some(range_hdr) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range_hdr, total) {
            let slice = bytes[start as usize..=end as usize].to_vec();
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
                slice,
            )
                .into_response());
        }
    }

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, ct.to_string()),
            (header::CONTENT_LENGTH, total.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
