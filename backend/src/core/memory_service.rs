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
    obsidian: Arc<ObsidianClient>,
    vault_path: PathBuf,
    vault_name: String,
}

#[allow(dead_code)]
impl MemoryService {
    /// Create a new `MemoryService`.
    pub fn new(obsidian: Arc<ObsidianClient>, vault_path: PathBuf, vault_name: String) -> Self {
        Self {
            obsidian,
            vault_path,
            vault_name,
        }
    }

    // ── Search ──

    /// Search notes using Obsidian's native search.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<NoteSummary>, BrainError> {
        let results = self.obsidian.search(query, limit).await?;

        let notes: Vec<NoteSummary> = results
            .into_iter()
            .map(|r| {
                let path = PathBuf::from(&r.filename);
                NoteSummary {
                    path: r.filename,
                    title: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string(),
                    tags: vec![], // Obsidian search doesn't return tags
                    updated_at: None,
                }
            })
            .collect();

        Ok(notes)
    }

    // ── File Operations ──

    /// Read a note's content.
    pub async fn read_note(&self, path: &str) -> Result<String, BrainError> {
        self.obsidian.read_file(path).await
    }

    /// Write content to a note (creates or overwrites).
    pub async fn write_note(&self, path: &str, content: &str) -> Result<(), BrainError> {
        self.obsidian.write_file(path, content).await
    }

    /// Append content to a note.
    pub async fn append_note(&self, path: &str, content: &str) -> Result<(), BrainError> {
        self.obsidian.append_file(path, content).await
    }

    /// Delete a note.
    pub async fn delete_note(&self, path: &str) -> Result<(), BrainError> {
        self.obsidian.delete_file(path).await
    }

    /// List all files in the vault.
    pub async fn list_files(&self) -> Result<Vec<String>, BrainError> {
        self.obsidian.list_files(None).await
    }

    // ── Metadata ──

    /// Get basic vault statistics.
    pub async fn get_stats(&self) -> Result<MemoryStats, BrainError> {
        let files = self.list_files().await?;
        let md_files: Vec<&String> = files.iter().filter(|f| f.ends_with(".md")).collect();

        Ok(MemoryStats {
            total_files: md_files.len(),
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
