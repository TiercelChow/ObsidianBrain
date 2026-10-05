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

    #[error("书籍或知识库不存在: {0}")]
    KnowledgeNotFound(String),

    #[error("知识库校验失败: {0}")]
    KnowledgeValidation(String),

    #[error("知识库状态冲突: {0}")]
    KnowledgeConflict(String),

    #[error("小记不存在: {0}")]
    MemoNotFound(String),
    #[error("小记校验失败: {0}")]
    MemoValidation(String),
    #[error("小记版本冲突: {0}")]
    MemoConflict(String),

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
            Self::KnowledgeNotFound(_) => "KNOWLEDGE_NOT_FOUND",
            Self::KnowledgeValidation(_) => "KNOWLEDGE_VALIDATION_ERROR",
            Self::KnowledgeConflict(_) => "KNOWLEDGE_CONFLICT",
            Self::MemoNotFound(_) => "MEMO_NOT_FOUND",
            Self::MemoValidation(_) => "MEMO_VALIDATION_ERROR",
            Self::MemoConflict(_) => "MEMO_VERSION_CONFLICT",
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
            Self::TaskNotFound(_) => Some("请刷新任务列表"),
            Self::TaskValidation(_) => Some("请检查标题、日期、状态和任务层级"),
            Self::TaskVersionConflict(_) => Some("任务已被修改，请刷新后重试"),
            Self::TaskDuplicateId(_) => Some("任务 ID 冲突，请重试或反馈"),
            Self::TaskDocumentCorrupt { .. } => Some("任务数据损坏，请检查数据库"),
            Self::KnowledgeNotFound(_) => Some("请刷新书架或知识库列表后重试"),
            Self::KnowledgeValidation(_) => Some("请检查知识库、实体或任务参数"),
            Self::KnowledgeConflict(_) => Some("请刷新知识库或研究任务状态，复核变化的来源后重试"),
            Self::MemoNotFound(_) => Some("请刷新时光机"),
            Self::MemoValidation(_) => Some("请检查内容和附件"),
            Self::MemoConflict(_) => Some("小记已被修改，请刷新后重新编辑"),
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
            | Self::TaskNotFound(_)
            | Self::KnowledgeNotFound(_) => StatusCode::NOT_FOUND,
            Self::ParseError { .. }
            | Self::TaskValidation(_)
            | Self::TaskDocumentCorrupt { .. }
            | Self::KnowledgeValidation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TaskVersionConflict(_)
            | Self::TaskDuplicateId(_)
            | Self::KnowledgeConflict(_) => StatusCode::CONFLICT,
            Self::MemoNotFound(_) => StatusCode::NOT_FOUND,
            Self::MemoValidation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::MemoConflict(_) => StatusCode::CONFLICT,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_conflict_is_distinct_from_model_output_validation() {
        let conflict = BrainError::KnowledgeConflict("来源版本已变化".into());
        assert_eq!(conflict.error_code(), "KNOWLEDGE_CONFLICT");
        assert_eq!(conflict.status_code(), StatusCode::CONFLICT);

        let validation = BrainError::KnowledgeValidation("研究字段缺失".into());
        assert_eq!(validation.error_code(), "KNOWLEDGE_VALIDATION_ERROR");
        assert_eq!(validation.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
