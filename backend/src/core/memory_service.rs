//! Memory Service — vault file operations and search via Obsidian API.
//!
//! Provides:
//! - File CRUD operations via Obsidian Local REST API
//! - Search via Obsidian's native search
//! - Note reading and listing
//! - File watcher for detecting changes (future use)

use std::path::PathBuf;
use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::file_watcher::FileWatcher;
use crate::infra::obsidian_client::ObsidianClient;
use crate::models::{MemoryStats, NoteSummary};

/// Core service for vault operations via Obsidian API.
#[allow(dead_code)]
pub struct MemoryService {
    obsidian: Option<Arc<ObsidianClient>>,
    vault_path: PathBuf,
    vault_name: String,
}

#[allow(dead_code)]
impl MemoryService {
    /// Create a new `MemoryService`.
    pub fn new(obsidian: Option<Arc<ObsidianClient>>, vault_path: PathBuf, vault_name: String) -> Self {
        Self {
            obsidian,
            vault_path,
            vault_name,
        }
    }

    /// Helper to get the Obsidian client or return an error if not available.
    fn client(&self) -> Result<&Arc<ObsidianClient>, BrainError> {
        self.obsidian.as_ref().ok_or_else(|| {
            BrainError::ConfigError("Obsidian API 客户端未启用，请在配置中设置 obsidian.enabled = true".to_string())
        })
    }

    // ── Search ──

    /// Search notes using Obsidian's native search.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<NoteSummary>, BrainError> {
        let client = self.client()?;
        let results = client.search(query, limit).await?;

        let notes: Vec<NoteSummary> = results
            .into_iter()
            .map(|r| {
                let path = std::path::PathBuf::from(&r.filename);
                NoteSummary {
                    path: r.filename,
                    title: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string(),
                    tags: vec![],
                    updated_at: None,
                    snippet: Some(
                        r.result.as_str().unwrap_or("").to_string()
                    ),
                }
            })
            .collect();

        Ok(notes)
    }

    // ── File Operations ──

    /// Read a note's content.
    pub async fn read_note(&self, path: &str) -> Result<String, BrainError> {
        self.client()?.read_file(path).await
    }

    /// Write content to a note (creates or overwrites).
    pub async fn write_note(&self, path: &str, content: &str) -> Result<(), BrainError> {
        self.client()?.write_file(path, content).await
    }

    /// Append content to a note.
    pub async fn append_note(&self, path: &str, content: &str) -> Result<(), BrainError> {
        self.client()?.append_file(path, content).await
    }

    /// Delete a note.
    pub async fn delete_note(&self, path: &str) -> Result<(), BrainError> {
        self.client()?.delete_file(path).await
    }

    /// List all files in the vault (recursive).
    pub async fn list_files(&self) -> Result<Vec<String>, BrainError> {
        self.client()?.list_all_files().await
    }

    // ── Metadata ──

    /// Get basic vault statistics.
    pub async fn get_stats(&self) -> Result<MemoryStats, BrainError> {
        let files = self.list_files().await?;
        let md_files: Vec<&String> = files.iter().filter(|f| f.ends_with(".md")).collect();

        // Try to collect tags by reading a sample of notes (up to 50)
        let mut all_tags = std::collections::HashSet::new();
        for path in md_files.iter().take(50) {
            if let Ok(content) = self.client()?.read_file(path).await {
                // Extract tags from frontmatter (simple parsing)
                if let Some(tags_start) = content.find("tags:") {
                    let tags_section = &content[tags_start..];
                    if let Some(tags_end) = tags_section.find('\n') {
                        let tags_line = &tags_section[..tags_end];
                        // Parse [tag1, tag2] format
                        if let Some(bracket_start) = tags_line.find('[') {
                            if let Some(bracket_end) = tags_line.find(']') {
                                let tags_str = &tags_line[bracket_start + 1..bracket_end];
                                for tag in tags_str.split(',') {
                                    let tag = tag.trim().trim_matches('"').trim_matches('\'');
                                    if !tag.is_empty() {
                                        all_tags.insert(tag.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(MemoryStats {
            total_notes: md_files.len(),
            total_files: files.len(),
            tags: all_tags.into_iter().collect(),
            vault_path: self.vault_path.to_string_lossy().to_string(),
            vault_name: self.vault_name.clone(),
        })
    }

    // ── File Watcher (for future use) ──

    /// Start the file watcher (currently a no-op, for future use).
    pub fn start_file_watcher(&self, _watcher: FileWatcher) -> Result<(), BrainError> {
        // TODO: Implement file watcher integration
        // For now, we rely on Obsidian API for all operations
        tracing::info!("文件监控已禁用 (使用 Obsidian API 模式)");
        Ok(())
    }
}
