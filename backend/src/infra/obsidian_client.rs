//! Obsidian Local REST API client.
//!
//! Wraps the [obsidian-local-rest-api](https://github.com/coddingtonbear/obsidian-local-rest-api)
//! plugin endpoints for vault file CRUD, search, periodic notes, and command execution.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::config::ObsidianApiConfig;
use crate::error::BrainError;

/// Shared, hot-swappable ObsidianClient provider.
/// All services hold a reference to this. When the client is recreated
/// (e.g. after a config change), all services automatically see the new one.
pub type ObsidianProvider = Arc<RwLock<Option<Arc<ObsidianClient>>>>;

/// Create a new provider wrapping the given client (or None).
pub fn new_provider(client: Option<Arc<ObsidianClient>>) -> ObsidianProvider {
    Arc::new(RwLock::new(client))
}

/// Get a clone of the current ObsidianClient from the provider.
pub fn get_client(provider: &ObsidianProvider) -> Result<Arc<ObsidianClient>, BrainError> {
    provider
        .read()
        .map_err(|e| BrainError::Internal(format!("ObsidianProvider lock: {e}")))?
        .clone()
        .ok_or_else(|| BrainError::Internal("Obsidian API 不可用".to_string()))
}

/// Atomically swap the client inside a provider.
pub fn set_client(provider: &ObsidianProvider, client: Option<Arc<ObsidianClient>>) {
    if let Ok(mut guard) = provider.write() {
        *guard = client;
    }
}

/// Client for the Obsidian Local REST API plugin.
#[derive(Clone)]
pub struct ObsidianClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

// ── Response types ──

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct VaultFileInfo {
    pub path: String,
    #[serde(default)]
    pub stat: Option<FileStat>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileStat {
    #[serde(default)]
    pub ctime: Option<u64>,
    #[serde(default)]
    pub mtime: Option<u64>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// A note returned by the API with parsed frontmatter.
#[derive(Debug, Clone, Deserialize)]
pub struct NoteResponse {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub frontmatter: Option<serde_json::Value>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub stat: Option<FileStat>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub filename: String,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandInfo {
    pub id: String,
    pub name: String,
}

/// Represents a periodic note period.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl std::fmt::Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Period::Daily => write!(f, "daily"),
            Period::Weekly => write!(f, "weekly"),
            Period::Monthly => write!(f, "monthly"),
            Period::Quarterly => write!(f, "quarterly"),
            Period::Yearly => write!(f, "yearly"),
        }
    }
}

/// Patch operation for partial file edits.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum PatchOp {
    #[serde(rename = "replace")]
    Replace { from: String, to: String },
    #[serde(rename = "insert")]
    Insert { at: String, content: String },
}

impl ObsidianClient {
    /// Create a new Obsidian client from config.
    pub fn new(config: &ObsidianApiConfig) -> Result<Self, BrainError> {
        if !config.enabled {
            return Err(BrainError::ConfigError(
                "Obsidian API client is disabled".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true) // self-signed cert
            .build()
            .map_err(|e| BrainError::Internal(format!("Obsidian HTTP 客户端创建失败: {e}")))?;

        Ok(Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        })
    }

    // ── Health ──

    /// Check if the Obsidian API server is running (no auth required).
    pub async fn health_check(&self) -> bool {
        self.client
            .get(format!("{}/", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    // ── Vault file operations ──

    /// Read a file from the vault. Returns the raw markdown content.
    pub async fn read_file(&self, path: &str) -> Result<String, BrainError> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/vault/{}", encode_path(path)),
            )
            .send()
            .await
            .map_err(|e| self.map_error("读取文件", e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BrainError::NoteNotFound(path.into()));
        }

        self.check_response(&resp)?;
        resp.text()
            .await
            .map_err(|e| BrainError::Internal(format!("读取响应失败: {e}")))
    }

    /// Read a file with parsed frontmatter.
    pub async fn read_note(&self, path: &str) -> Result<NoteResponse, BrainError> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Accept", "application/vnd.olrapi.note+json")
            .send()
            .await
            .map_err(|e| self.map_error("读取笔记", e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BrainError::NoteNotFound(path.into()));
        }

        self.check_response(&resp)?;
        resp.json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析笔记响应失败: {e}")))
    }

    /// Write (overwrite) a file in the vault.
    pub async fn write_file(&self, path: &str, content: &str) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| self.map_error("写入文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// Write binary data (e.g. images) to a file in the vault.
    pub async fn write_binary(
        &self,
        path: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Content-Type", content_type)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| self.map_error("写入二进制文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// Read binary data from a file in the vault.
    pub async fn read_binary(&self, path: &str) -> Result<(Vec<u8>, String), BrainError> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Accept", "application/octet-stream")
            .send()
            .await
            .map_err(|e| self.map_error("读取二进制文件", e))?;

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        self.check_response(&resp)?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| self.map_error("读取二进制数据", e))?;
        Ok((bytes.to_vec(), content_type))
    }

    /// Append content to a file in the vault.
    pub async fn append_file(&self, path: &str, content: &str) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| self.map_error("追加文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// Apply patch operations to a file (surgical edits).
    pub async fn patch_file(&self, path: &str, operations: &[PatchOp]) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::PATCH,
                &format!("/vault/{}", encode_path(path)),
            )
            .header("Content-Type", "application/json")
            .json(operations)
            .send()
            .await
            .map_err(|e| self.map_error("编辑文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// Delete a file from the vault.
    pub async fn delete_file(&self, path: &str) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::DELETE,
                &format!("/vault/{}", encode_path(path)),
            )
            .send()
            .await
            .map_err(|e| self.map_error("删除文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// List files in the vault root or a subdirectory (non-recursive).
    pub async fn list_files(&self, dir: Option<&str>) -> Result<Vec<String>, BrainError> {
        let path = dir.unwrap_or("");
        let url_path = if path.is_empty() {
            "/vault/".to_string()
        } else {
            format!("/vault/{}", encode_path(path))
        };

        let resp = self
            .request(reqwest::Method::GET, &url_path)
            .send()
            .await
            .map_err(|e| self.map_error("列出文件", e))?;

        self.check_response(&resp)?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析文件列表失败: {e}")))?;

        body.get("files")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| BrainError::Internal("响应中缺少 files 字段".to_string()))
    }

    /// Recursively list all files in the vault (walks directories).
    pub async fn list_all_files(&self) -> Result<Vec<String>, BrainError> {
        let mut all_files = Vec::new();
        let mut dirs_to_walk = vec![String::new()];

        while let Some(dir) = dirs_to_walk.pop() {
            let entries = self
                .list_files(if dir.is_empty() { None } else { Some(&dir) })
                .await?;

            for entry in entries {
                let full_path = if dir.is_empty() {
                    entry.clone()
                } else {
                    format!("{}{}", dir, entry)
                };

                if entry.ends_with('/') {
                    dirs_to_walk.push(full_path);
                } else {
                    all_files.push(full_path);
                }
            }
        }

        Ok(all_files)
    }

    // ── Active file ──

    /// Get the currently active (open) file in Obsidian.
    pub async fn get_active_file(&self) -> Result<NoteResponse, BrainError> {
        let resp = self
            .request(reqwest::Method::GET, "/active/")
            .header("Accept", "application/vnd.olrapi.note+json")
            .send()
            .await
            .map_err(|e| self.map_error("获取活动文件", e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BrainError::NoteNotFound("active".into()));
        }

        self.check_response(&resp)?;
        resp.json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析活动文件失败: {e}")))
    }

    // ── Search ──

    /// Search across the vault using JsonLogic query.
    /// Uses `in` operator for substring matching on file content.
    /// Results are truncated client-side to `limit`.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, BrainError> {
        // Build JsonLogic query: check if query string is a substring of content
        let json_logic = serde_json::json!({
            "in": [query, {"var": "content"}]
        });

        let resp = self
            .request(reqwest::Method::POST, "/search/")
            .header("Content-Type", "application/vnd.olrapi.jsonlogic+json")
            .json(&json_logic)
            .send()
            .await
            .map_err(|e| self.map_error("搜索", e))?;

        self.check_response(&resp)?;

        let mut results: Vec<SearchResult> = resp
            .json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析搜索结果失败: {e}")))?;

        // Truncate to limit client-side (API doesn't support limit)
        results.truncate(limit);
        Ok(results)
    }

    // ── Periodic notes ──

    /// Get or create a periodic note (daily/weekly/monthly/etc).
    /// If `date` is provided, targets that specific date.
    pub async fn get_periodic_note(
        &self,
        period: Period,
        date: Option<&str>,
    ) -> Result<NoteResponse, BrainError> {
        let url = if let Some(d) = date {
            format!("/periodic/{}/{}", period, d)
        } else {
            format!("/periodic/{}/", period)
        };

        let resp = self
            .request(reqwest::Method::GET, &url)
            .header("Accept", "application/vnd.olrapi.note+json")
            .send()
            .await
            .map_err(|e| self.map_error("获取周期性笔记", e))?;

        self.check_response(&resp)?;
        resp.json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析周期性笔记失败: {e}")))
    }

    /// Write content to a periodic note.
    pub async fn write_periodic_note(
        &self,
        period: Period,
        date: Option<&str>,
        content: &str,
    ) -> Result<(), BrainError> {
        let url = if let Some(d) = date {
            format!("/periodic/{}/{}", period, d)
        } else {
            format!("/periodic/{}/", period)
        };

        let resp = self
            .request(reqwest::Method::PUT, &url)
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| self.map_error("写入周期性笔记", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    /// Append content to a periodic note.
    pub async fn append_periodic_note(
        &self,
        period: Period,
        date: Option<&str>,
        content: &str,
    ) -> Result<(), BrainError> {
        let url = if let Some(d) = date {
            format!("/periodic/{}/{}", period, d)
        } else {
            format!("/periodic/{}/", period)
        };

        let resp = self
            .request(reqwest::Method::POST, &url)
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| self.map_error("追加周期性笔记", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    // ── Commands ──

    /// List all available Obsidian commands.
    pub async fn list_commands(&self) -> Result<Vec<CommandInfo>, BrainError> {
        let resp = self
            .request(reqwest::Method::GET, "/command/")
            .send()
            .await
            .map_err(|e| self.map_error("列出命令", e))?;

        self.check_response(&resp)?;
        resp.json()
            .await
            .map_err(|e| BrainError::Internal(format!("解析命令列表失败: {e}")))
    }

    /// Execute an Obsidian command by its ID.
    pub async fn execute_command(&self, command_id: &str) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/command/{}", urlencoding::encode(command_id)),
            )
            .send()
            .await
            .map_err(|e| self.map_error("执行命令", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    // ── Open file in Obsidian ──

    /// Open a file in the Obsidian UI.
    pub async fn open_file(&self, path: &str) -> Result<(), BrainError> {
        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/open/{}", encode_path(path)),
            )
            .send()
            .await
            .map_err(|e| self.map_error("打开文件", e))?;

        self.check_response(&resp)?;
        Ok(())
    }

    // ── Internal helpers ──

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{}", self.base_url, path)
        };

        let mut req = self.client.request(method, &url);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        req
    }

    fn check_response(&self, resp: &reqwest::Response) -> Result<(), BrainError> {
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(BrainError::FetchError {
                url: resp.url().to_string(),
                detail: format!("HTTP {}", resp.status()),
            })
        }
    }

    fn map_error(&self, context: &str, e: reqwest::Error) -> BrainError {
        if e.is_connect() {
            BrainError::FetchError {
                url: self.base_url.clone(),
                detail: format!("{}: Obsidian API 连接失败 (插件是否已启用?)", context),
            }
        } else if e.is_timeout() {
            BrainError::FetchError {
                url: self.base_url.clone(),
                detail: format!("{}: Obsidian API 请求超时", context),
            }
        } else {
            BrainError::FetchError {
                url: self.base_url.clone(),
                detail: format!("{}: {}", context, e),
            }
        }
    }
}

/// URL-encode a vault file path (encode `/` as `%2F` for path segments).
fn encode_path(path: &str) -> String {
    urlencoding::encode(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_path() {
        assert_eq!(encode_path("folder/note.md"), "folder%2Fnote.md");
        assert_eq!(
            encode_path("中文笔记.md"),
            "%E4%B8%AD%E6%96%87%E7%AC%94%E8%AE%B0.md"
        );
    }

    #[test]
    fn test_period_display() {
        assert_eq!(Period::Daily.to_string(), "daily");
        assert_eq!(Period::Weekly.to_string(), "weekly");
        assert_eq!(Period::Monthly.to_string(), "monthly");
    }

    #[test]
    fn test_disabled_client_returns_error() {
        let config = ObsidianApiConfig {
            enabled: false,
            url: "http://127.0.0.1:27123".to_string(),
            api_key: None,
        };
        assert!(ObsidianClient::new(&config).is_err());
    }

    #[test]
    fn test_enabled_client_creates_successfully() {
        let config = ObsidianApiConfig {
            enabled: true,
            url: "http://127.0.0.1:27123".to_string(),
            api_key: Some("test-key".to_string()),
        };
        assert!(ObsidianClient::new(&config).is_ok());
    }

    #[test]
    fn test_patch_op_serialization() {
        let ops = vec![PatchOp::Replace {
            from: "old text".to_string(),
            to: "new text".to_string(),
        }];
        let json = serde_json::to_string(&ops).unwrap();
        assert!(json.contains("replace"));
        assert!(json.contains("old text"));
    }
}
