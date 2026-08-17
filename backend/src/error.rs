use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::path::PathBuf;

/// Unified error type for the entire application.
#[derive(Debug, thiserror::Error)]
pub enum BrainError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Vault not found: {0}")]
    VaultNotFound(PathBuf),

    #[error("Note not found: {0}")]
    NoteNotFound(PathBuf),

    #[error("Parse error in {path}: {detail}")]
    ParseError { path: PathBuf, detail: String },

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("Repository not found: {0}")]
    RepoNotFound(PathBuf),

    #[error("任务不存在: {0}")]
    TaskNotFound(String),

    #[error("任务校验失败: {0}")]
    TaskValidation(String),

    #[error("任务文档版本冲突: {0}")]
    TaskVersionConflict(String),

    #[error("任务 ID 冲突: {0}")]
    TaskDuplicateId(String),

    #[error("任务文档损坏 {path}: {detail}")]
    TaskDocumentCorrupt { path: String, detail: String },

    #[error("Obsidian API 不可用")]
    ObsidianUnavailable,

    #[error("Git error in {path}: {detail}")]
    GitError { path: PathBuf, detail: String },

    #[error("Qdrant error: {0}")]
    QdrantError(String),

    #[error("LLM API error ({provider}): {detail}")]
    LlmApiError { provider: String, detail: String },

    #[error("Fetch error for {url}: {detail}")]
    FetchError { url: String, detail: String },

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl BrainError {
    /// Returns a machine-readable error code string.
    pub(crate) fn error_code(&self) -> &'static str {
        match self {
            Self::ConfigError(_) => "CONFIG_ERROR",
            Self::VaultNotFound(_) => "VAULT_NOT_FOUND",
            Self::NoteNotFound(_) => "NOTE_NOT_FOUND",
            Self::ParseError { .. } => "PARSE_ERROR",
            Self::SearchError(_) => "SEARCH_ERROR",
            Self::EmbeddingError(_) => "EMBEDDING_ERROR",
            Self::RepoNotFound(_) => "REPO_NOT_FOUND",
            Self::TaskNotFound(_) => "TASK_NOT_FOUND",
            Self::TaskValidation(_) => "TASK_VALIDATION_ERROR",
            Self::TaskVersionConflict(_) => "TASK_VERSION_CONFLICT",
            Self::TaskDuplicateId(_) => "TASK_DUPLICATE_ID",
            Self::TaskDocumentCorrupt { .. } => "TASK_DOCUMENT_CORRUPT",
            Self::ObsidianUnavailable => "OBSIDIAN_UNAVAILABLE",
            Self::GitError { .. } => "GIT_ERROR",
            Self::QdrantError(_) => "QDRANT_ERROR",
            Self::LlmApiError { .. } => "LLM_API_ERROR",
            Self::FetchError { .. } => "FETCH_ERROR",
            Self::IoError(_) => "IO_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Returns an optional user-facing suggestion for recovery.
    pub(crate) fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::ConfigError(_) => Some("Check config/default.toml or environment variables"),
            Self::VaultNotFound(_) => {
                Some("Verify the vault path in configuration and ensure the directory exists")
            }
            Self::NoteNotFound(_) => Some("Check the note path and ensure the file exists"),
            Self::ParseError { .. } => {
                Some("Check the file for malformed frontmatter or invalid Markdown")
            }
            Self::SearchError(_) => {
                Some("Verify that Tantivy index and Qdrant collection are initialized")
            }
            Self::EmbeddingError(_) => {
                Some("Check your embedding API key and network connectivity")
            }
            Self::RepoNotFound(_) => Some("Register the repository first via the code_repo tools"),
            Self::TaskNotFound(_) => Some("请刷新任务列表或从 Obsidian 重新同步"),
            Self::TaskValidation(_) => Some("请检查标题、日期、状态和任务层级"),
            Self::TaskVersionConflict(_) => Some("任务已被修改，请刷新后重试"),
            Self::TaskDuplicateId(_) => Some("请在 Obsidian 中修复重复任务 ID 后重新同步"),
            Self::TaskDocumentCorrupt { .. } => Some("请检查 Tasks 文件的 YAML frontmatter"),
            Self::ObsidianUnavailable => Some("请启用并检查 Obsidian Local REST API 插件"),
            Self::GitError { .. } => {
                Some("Ensure the repository is a valid git repo and git is accessible")
            }
            Self::QdrantError(_) => Some("Ensure Qdrant is running: docker compose up -d"),
            Self::LlmApiError { .. } => {
                Some("Check your LLM API key, model name, and network connectivity")
            }
            Self::FetchError { .. } => Some("Check the URL and your network connection"),
            Self::IoError(_) => Some("Check file permissions and disk space"),
            Self::Internal(_) => Some("This is a bug — please report it with the error details"),
        }
    }

    /// Maps the error to an appropriate HTTP status code.
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ConfigError(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::VaultNotFound(_)
            | Self::NoteNotFound(_)
            | Self::RepoNotFound(_)
            | Self::TaskNotFound(_) => StatusCode::NOT_FOUND,
            Self::ParseError { .. }
            | Self::TaskValidation(_)
            | Self::TaskDocumentCorrupt { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TaskVersionConflict(_) | Self::TaskDuplicateId(_) => StatusCode::CONFLICT,
            Self::ObsidianUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::SearchError(_)
            | Self::EmbeddingError(_)
            | Self::QdrantError(_)
            | Self::LlmApiError { .. }
            | Self::FetchError { .. } => StatusCode::BAD_GATEWAY,
            Self::GitError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<rusqlite::Error> for BrainError {
    fn from(e: rusqlite::Error) -> Self {
        BrainError::Internal(format!("SQLite 错误: {e}"))
    }
}

impl From<reqwest::Error> for BrainError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            BrainError::Internal(format!("HTTP 超时: {e}"))
        } else if e.is_connect() {
            BrainError::Internal(format!("连接失败: {e}"))
        } else {
            BrainError::Internal(format!("HTTP 错误: {e}"))
        }
    }
}

/// Axum IntoResponse implementation — returns JSON error envelope.
impl IntoResponse for BrainError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error_code": self.error_code(),
            "message": self.to_string(),
            "suggestion": self.suggestion(),
        });
        (status, Json(body)).into_response()
    }
}
