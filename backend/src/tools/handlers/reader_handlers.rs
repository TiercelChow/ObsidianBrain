//! Filesystem-scoped reader tools: `list_local_dir`, `read_local_file`.
//!
//! These operate on **arbitrary local paths** (not the Obsidian vault), powering
//! the Markdown Reader UI. They use `tokio::fs` directly; IO errors map to
//! `BrainError::IoError` via `#[from]`, validation errors use `BrainError::Internal`.
//! Neither tool needs `AppContext` state.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::sqlite_store::SqliteStore;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// Max file size accepted by `read_local_file` (5 MB).
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum recursion depth allowed (defense against pathological deep trees).
const MAX_DEPTH_CAP: usize = 6;

/// Directories skipped when building the file tree (build artifacts / VCS / deps).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    "__pycache__",
    "venv",
    ".venv",
    "out",
    ".next",
    ".cache",
    "coverage",
    ".idea",
    ".vscode",
];

/// A single node in the directory tree returned by `list_local_dir`.
#[derive(serde::Serialize)]
struct DirEntry {
    name: String,
    path: String,
    is_dir: bool,
    is_markdown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<DirEntry>>,
}

fn is_markdown_ext(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// Whether an entry should be skipped: hidden entries (leading `.`) or known
/// junk directories. Hidden files (`.DS_Store`, `.gitignore`) are skipped too.
fn should_skip(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

/// Recursively build a directory tree (synchronous — run via `spawn_blocking`).
///
/// Errors reading individual entries are swallowed (the entry is skipped) so one
/// permission-denied subdir doesn't fail the whole listing.
fn build_tree(path: &Path, depth: usize, max_depth: usize) -> Vec<DirEntry> {
    let read_dir = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<DirEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if should_skip(&name) {
                return None;
            }
            let file_type = e.file_type().ok()?;
            let is_dir = file_type.is_dir();
            let is_markdown = !is_dir && is_markdown_ext(&name);
            let entry_path = e.path();
            let children = if is_dir && depth < max_depth {
                Some(build_tree(&entry_path, depth + 1, max_depth))
            } else {
                None
            };
            Some(DirEntry {
                name,
                path: entry_path.to_string_lossy().to_string(),
                is_dir,
                is_markdown,
                children,
            })
        })
        .collect();

    // Sort: directories first, then files; case-insensitive alphabetical.
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });

    entries
}

/// Read a local file as UTF-8 text. Returns `(name, content, size_bytes)`.
///
/// Validates: exists, is a file, under size limit. Used by the handler and tests.
async fn read_local_file_impl(path: &Path) -> Result<(String, String, usize), BrainError> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|e| BrainError::Internal(format!("文件无法访问: {e}")))?;
    if meta.is_dir() {
        return Err(BrainError::Internal("路径是目录，不是文件".to_string()));
    }
    if meta.len() > MAX_FILE_SIZE {
        return Err(BrainError::Internal(format!(
            "文件过大 ({:.1} MB)，上限 {} MB",
            meta.len() as f64 / 1_048_576.0,
            MAX_FILE_SIZE / 1_048_576
        )));
    }

    let bytes = tokio::fs::read(path).await?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    Ok((name, content, bytes.len()))
}

/// List a local directory as a recursive file tree.
pub struct ListLocalDirHandler;

#[async_trait]
impl ToolHandler for ListLocalDirHandler {
    fn name(&self) -> &str {
        "list_local_dir"
    }
    fn description(&self) -> &str {
        "列出本地目录的文件树（任意本地路径，非 vault）。返回结构化树，跳过隐藏与依赖目录。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "本地绝对路径" },
                "depth": { "type": "integer", "default": 3, "minimum": 0, "maximum": 6 }
            },
            "required": ["path"]
        })
    }
    fn module(&self) -> &str {
        "reader"
    }

    async fn handle(&self, args: Value, _ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'path'".to_string()))?;
        let depth = args
            .get("depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(3)
            .min(MAX_DEPTH_CAP);

        let path = PathBuf::from(path_str);
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|e| BrainError::Internal(format!("路径无法访问: {e}")))?;
        if !meta.is_dir() {
            return Err(BrainError::Internal("路径不是目录".to_string()));
        }

        let root = path.clone();
        let entries = tokio::task::spawn_blocking(move || build_tree(&root, 0, depth))
            .await
            .map_err(|e| BrainError::Internal(format!("遍历任务失败: {e}")))?;

        let total = entries.len();
        let entries_json = serde_json::to_value(&entries)
            .map_err(|e| BrainError::Internal(format!("序列化失败: {e}")))?;

        Ok(json!({
            "root": path_str,
            "entries": entries_json,
            "total": total,
        }))
    }
}

/// Read a local file's text content.
pub struct ReadLocalFileHandler;

#[async_trait]
impl ToolHandler for ReadLocalFileHandler {
    fn name(&self) -> &str {
        "read_local_file"
    }
    fn description(&self) -> &str {
        "读取本地文件内容（任意本地路径，5MB 上限）"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "本地文件绝对路径" }
            },
            "required": ["path"]
        })
    }
    fn module(&self) -> &str {
        "reader"
    }

    async fn handle(&self, args: Value, _ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'path'".to_string()))?;

        let path = PathBuf::from(path_str);
        let (name, content, size) = read_local_file_impl(&path).await?;

        Ok(json!({
            "path": path_str,
            "name": name,
            "content": content,
            "size": size,
        }))
    }
}

// ── Reader history (server-stored, shared across all users) ────────────

/// SQLite `app_state` key holding the reader history JSON array.
const HISTORY_KEY: &str = "reader_history";

/// A single reader history entry. Serialized as `{"path","pinned","lastUsed"}`.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
struct HistoryItem {
    path: String,
    pinned: bool,
    last_used: i64,
}

/// Read the reader history from SQLite. Returns an empty vec if unset or unparseable.
fn get_history(db: &SqliteStore) -> Result<Vec<HistoryItem>, BrainError> {
    match db.get_state(HISTORY_KEY)? {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

/// Get the shared reader history.
pub struct GetReaderHistoryHandler;

#[async_trait]
impl ToolHandler for GetReaderHistoryHandler {
    fn name(&self) -> &str {
        "get_reader_history"
    }
    fn description(&self) -> &str {
        "获取 Markdown 阅读器的历史记录（服务端存储，所有用户共享）"
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn module(&self) -> &str {
        "reader"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let items = get_history(&ctx.db)?;
        let items_json = serde_json::to_value(&items)
            .map_err(|e| BrainError::Internal(format!("序列化失败: {e}")))?;
        Ok(json!({ "history": items_json }))
    }
}

/// Save the full reader history list (replaces the existing list).
pub struct SaveReaderHistoryHandler;

#[async_trait]
impl ToolHandler for SaveReaderHistoryHandler {
    fn name(&self) -> &str {
        "save_reader_history"
    }
    fn description(&self) -> &str {
        "保存 Markdown 阅读器的历史记录（整体替换，服务端共享）"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "history": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "pinned": { "type": "boolean" },
                            "lastUsed": { "type": "number" }
                        },
                        "required": ["path", "pinned", "lastUsed"]
                    }
                }
            },
            "required": ["history"]
        })
    }
    fn module(&self) -> &str {
        "reader"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let history_arg = args
            .get("history")
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'history'".to_string()))?;
        let items: Vec<HistoryItem> = serde_json::from_value(history_arg.clone())
            .map_err(|e| BrainError::Internal(format!("history 格式错误: {e}")))?;
        let json = serde_json::to_string(&items)
            .map_err(|e| BrainError::Internal(format!("序列化失败: {e}")))?;
        ctx.db.set_state(HISTORY_KEY, &json)?;
        Ok(json!({ "ok": true, "count": items.len() }))
    }
}

/// Stat a local path — returns whether it exists, is a dir/file, name, ext, size.
/// Used by the reader to decide how to preview a link target (md / folder / code).
pub struct StatLocalPathHandler;

#[async_trait]
impl ToolHandler for StatLocalPathHandler {
    fn name(&self) -> &str {
        "stat_local_path"
    }
    fn description(&self) -> &str {
        "获取本地路径的类型信息（文件/目录、扩展名、大小），用于预览跳转决策"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "本地路径" } },
            "required": ["path"]
        })
    }
    fn module(&self) -> &str {
        "reader"
    }

    async fn handle(&self, args: Value, _ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'path'".to_string()))?;
        let path = PathBuf::from(path_str);
        let meta = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => return Ok(json!({ "exists": false })),
        };
        let is_dir = meta.is_dir();
        let is_file = meta.is_file();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let size = if is_file { meta.len() } else { 0 };
        Ok(json!({
            "exists": true,
            "is_dir": is_dir,
            "is_file": is_file,
            "name": name,
            "ext": ext,
            "size": size,
            "path": path_str,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a temp tree:
    /// ```text
    /// root/
    ///   a.md
    ///   sub/
    ///     b.md
    ///     note.txt
    ///   .git/
    ///     x.md          (should be skipped)
    /// ```
    async fn make_tree(root: &Path) {
        tokio::fs::create_dir(root.join("sub")).await.unwrap();
        tokio::fs::create_dir(root.join(".git")).await.unwrap();
        tokio::fs::write(root.join("a.md"), "# A").await.unwrap();
        tokio::fs::write(root.join("sub").join("b.md"), "# B")
            .await
            .unwrap();
        tokio::fs::write(root.join("sub").join("note.txt"), "txt")
            .await
            .unwrap();
        tokio::fs::write(root.join(".git").join("x.md"), "hidden")
            .await
            .unwrap();
    }

    #[test]
    fn test_build_tree_returns_structure_and_skips_hidden() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // Build synchronously for the sync build_tree test.
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join("a.md"), "# A").unwrap();
        std::fs::write(root.join("sub").join("b.md"), "# B").unwrap();
        std::fs::write(root.join("sub").join("note.txt"), "txt").unwrap();
        std::fs::write(root.join(".git").join("x.md"), "hidden").unwrap();

        let entries = build_tree(root, 0, 3);
        // Top-level: "sub" (dir) and "a.md" (file). ".git" skipped.
        assert_eq!(entries.len(), 2, "should list sub and a.md, skip .git");
        // Directories sort before files.
        assert!(entries[0].is_dir, "first entry should be the directory");
        assert_eq!(entries[0].name, "sub");
        assert!(!entries[0].is_markdown);
        assert_eq!(entries[1].name, "a.md");
        assert!(entries[1].is_markdown);
        assert!(!entries[1].is_dir);

        // sub children: b.md (markdown) + note.txt (non-markdown). x.md in .git skipped.
        let sub_children = entries[0].children.as_ref().expect("sub has children");
        assert_eq!(sub_children.len(), 2);
        assert!(sub_children
            .iter()
            .any(|c| c.name == "b.md" && c.is_markdown));
        assert!(sub_children
            .iter()
            .any(|c| c.name == "note.txt" && !c.is_markdown));
    }

    #[test]
    fn test_build_tree_respects_depth_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join("l1")).unwrap();
        std::fs::create_dir(root.join("l1").join("l2")).unwrap();
        std::fs::write(root.join("l1").join("l2").join("deep.md"), "deep").unwrap();

        // depth 0: do not recurse into l1.
        let entries = build_tree(root, 0, 0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "l1");
        assert!(
            entries[0].children.is_none(),
            "depth 0 should not expand l1"
        );

        // depth 1: expand l1 but not l2.
        let entries = build_tree(root, 0, 1);
        let l1 = &entries[0];
        let l1_children = l1.children.as_ref().expect("l1 expanded at depth 1");
        assert_eq!(l1_children.len(), 1);
        assert_eq!(l1_children[0].name, "l2");
        assert!(
            l1_children[0].children.is_none(),
            "l2 should not be expanded at depth 1"
        );
    }

    #[tokio::test]
    async fn test_read_local_file_impl_returns_content() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("note.md");
        tokio::fs::write(&file, "# 标题\n正文").await.unwrap();

        let (name, content, size) = read_local_file_impl(&file).await.unwrap();
        assert_eq!(name, "note.md");
        assert!(content.contains("# 标题"));
        assert!(content.contains("正文"));
        assert_eq!(size, "# 标题\n正文".len());
    }

    #[tokio::test]
    async fn test_read_local_file_impl_missing_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nope.md");
        let err = read_local_file_impl(&missing).await.unwrap_err();
        assert!(matches!(err, BrainError::Internal(_)));
    }

    #[tokio::test]
    async fn test_read_local_file_impl_dir_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let err = read_local_file_impl(tmp.path()).await.unwrap_err();
        assert!(matches!(err, BrainError::Internal(_)));
    }

    #[tokio::test]
    async fn test_read_local_file_impl_too_large_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("big.md");
        // Write a file just over the limit.
        let big = vec![b'a'; (MAX_FILE_SIZE + 1) as usize];
        tokio::fs::write(&file, &big).await.unwrap();
        let err = read_local_file_impl(&file).await.unwrap_err();
        assert!(matches!(err, BrainError::Internal(_)));
    }

    #[tokio::test]
    async fn test_list_local_dir_handle_end_to_end() {
        // Sanity-check the handler wiring without AppContext by exercising build_tree
        // serialization shape (the handler is a thin wrapper around build_tree).
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_tree(root).await;

        let entries = build_tree(root, 0, 3);
        let json = serde_json::to_value(&entries).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
        // Each entry has the expected fields.
        let first = &json[0];
        assert!(first["is_dir"].is_boolean());
        assert!(first["name"].is_string());
        assert!(first["path"].is_string());
    }

    #[tokio::test]
    async fn test_reader_history_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let db = SqliteStore::new(&tmp.path().join("history.db")).unwrap();

        // Empty initially.
        assert!(get_history(&db).unwrap().is_empty());

        // Save a list (pinned first, mixed lastUsed).
        let items = vec![
            HistoryItem {
                path: "/pinned/path".into(),
                pinned: true,
                last_used: 100,
            },
            HistoryItem {
                path: "/other/path".into(),
                pinned: false,
                last_used: 200,
            },
        ];
        let json = serde_json::to_string(&items).unwrap();
        db.set_state(HISTORY_KEY, &json).unwrap();

        // Read back — preserves fields and camelCase roundtrip.
        let got = get_history(&db).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].path, "/pinned/path");
        assert!(got[0].pinned);
        assert_eq!(got[0].last_used, 100);
        assert_eq!(got[1].last_used, 200);

        // CamelCase serialization (the wire format the frontend uses).
        let wire = serde_json::to_value(&items).unwrap();
        assert!(wire[0].get("lastUsed").is_some());
        assert!(wire[0].get("last_used").is_none());
    }

    #[tokio::test]
    async fn test_stat_local_path_classification() {
        // The handler is a thin wrapper around tokio::fs::metadata; verify the
        // file/dir/missing classification it relies on.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let file = root.join("note.md");
        std::fs::write(&file, "hi").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();

        assert!(tokio::fs::metadata(&file).await.unwrap().is_file());
        assert!(tokio::fs::metadata(root.join("sub"))
            .await
            .unwrap()
            .is_dir());
        assert!(tokio::fs::metadata(root.join("missing")).await.is_err());
    }
}
