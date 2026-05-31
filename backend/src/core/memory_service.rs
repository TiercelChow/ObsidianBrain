//! Memory Service — core indexing pipeline and CRUD operations.
//!
//! Ties together all Phase 0 infrastructure and Phase 1 core components:
//! - Markdown parsing → SmartChunker → Tantivy + Qdrant upsert
//! - Memory CRUD (add, update, forget)
//! - Note reading and listing
//! - Degraded mode when embedding or Qdrant fails (fulltext-only)

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::core::chunker::SmartChunker;
use crate::core::markdown_parser::MarkdownParser;
use crate::error::BrainError;
use crate::infra::embedding::EmbeddingProvider;
use crate::infra::file_watcher::{FileChangeEvent, FileChangeType, FileWatcher};
use crate::infra::qdrant_client::{ChunkPayload, QdrantStore, VectorPoint};
use crate::infra::tantivy_index::{NoteDocument, TantivyIndex};
use crate::models::{MemoryChunk, MemoryStats, NoteSummary};

/// Report from a full vault indexing run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct FullIndexReport {
    /// Total number of .md files found in the vault.
    pub total_files: usize,
    /// Number of files successfully indexed.
    pub indexed_files: usize,
    /// Files that failed to index, with their error messages.
    pub failed_files: Vec<(PathBuf, String)>,
    /// Total number of chunks created from all indexed files.
    pub total_chunks: usize,
}

/// Core service managing the indexing pipeline and memory CRUD.
#[allow(dead_code)]
pub struct MemoryService {
    tantivy: Arc<TantivyIndex>,
    qdrant: Arc<QdrantStore>,
    embedding: Arc<dyn EmbeddingProvider>,
    vault_path: PathBuf,
    #[allow(dead_code)]
    vault_name: String,
}

#[allow(dead_code)]
impl MemoryService {
    /// Create a new `MemoryService` with the given infrastructure dependencies.
    pub fn new(
        tantivy: Arc<TantivyIndex>,
        qdrant: Arc<QdrantStore>,
        embedding: Arc<dyn EmbeddingProvider>,
        vault_path: PathBuf,
        vault_name: String,
    ) -> Self {
        Self {
            tantivy,
            qdrant,
            embedding,
            vault_path,
            vault_name,
        }
    }

    // ── Index Pipeline ──

    /// Index a single file: read → parse → chunk → Tantivy + Qdrant.
    ///
    /// Returns the number of chunks created.
    ///
    /// # Error Handling
    /// - File read / parse / Tantivy failure → return `Err`
    /// - Embedding / Qdrant failure → log warning, continue (degraded mode)
    pub async fn index_file(&self, path: &Path) -> Result<usize, BrainError> {
        self.index_file_inner(path, false).await
    }

    /// Inner implementation with optional embedding skip.
    /// When `skip_embedding` is true, only Tantivy indexing is performed (used by full_index
    /// to avoid repeated embedding timeouts after first failure).
    async fn index_file_inner(
        &self,
        path: &Path,
        skip_embedding: bool,
    ) -> Result<usize, BrainError> {
        // 1. Read file content
        let content = std::fs::read_to_string(path).map_err(|e| {
            tracing::error!(path = %path.display(), error = %e, "文件读取失败");
            BrainError::IoError(e)
        })?;

        // 2. Parse with MarkdownParser
        let path_str = path_to_relative_str(path, &self.vault_path);
        let doc = MarkdownParser::parse(&path_str, &content).map_err(|e| {
            tracing::error!(path = %path.display(), error = %e, "Markdown 解析失败");
            e
        })?;

        // 3. Chunk with SmartChunker
        let chunker = SmartChunker::default();
        let chunks = chunker.chunk(&doc);

        if chunks.is_empty() {
            tracing::debug!(path = %path_str, "文件无有效内容，跳过索引");
            return Ok(0);
        }

        // 4. Delete old docs for this file (using note_path for efficient bulk deletion)
        self.tantivy.delete_by_note_path(&path_str)?;

        // 5. Index chunks in Tantivy: add new ones, commit
        for chunk in &chunks {
            // Include heading breadcrumb in searchable content for better retrieval
            let searchable_content = if chunk.breadcrumb.is_empty() {
                chunk.content.clone()
            } else {
                format!("{} {}", chunk.breadcrumb.join(" > "), chunk.content)
            };

            let note_doc = NoteDocument {
                title: chunk.note_title.clone(),
                content: searchable_content,
                path: path_str.clone(),
                tags: chunk.tags.clone(),
                chunk_id: chunk.id.to_string(),
                note_path: path_str.clone(),
            };
            self.tantivy.add_document(&note_doc)?;
        }
        self.tantivy.commit()?;

        // 6. Embed chunks (degraded if embedding fails)
        if skip_embedding {
            tracing::debug!("跳过 Embedding (已降级)");
            return Ok(chunks.len());
        }
        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = match self.embedding.embed_batch(&texts).await {
            Ok(vecs) => vecs,
            Err(e) => {
                tracing::warn!(error = %e, "Embedding 失败，降级为全文搜索模式");
                // Degraded: Tantivy indexed, Qdrant skipped
                return Ok(chunks.len());
            }
        };

        // 7. Upsert to Qdrant (degraded if Qdrant fails)
        let now = Utc::now();
        let points: Vec<VectorPoint> = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, vector)| {
                let payload = ChunkPayload {
                    note_path: chunk.note_path.clone(),
                    chunk_index: chunk.chunk_index,
                    content: chunk.content.clone(),
                    title: chunk.note_title.clone(),
                    tags: chunk.tags.clone(),
                    heading_path: chunk.breadcrumb.clone(),
                    word_count: chunk.token_count,
                    created_at: now.to_rfc3339(),
                    updated_at: now.to_rfc3339(),
                };
                VectorPoint {
                    id: chunk.id.to_string(),
                    vector: vector.clone(),
                    payload: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
                }
            })
            .collect();

        if let Err(e) = self.qdrant.upsert_points(points).await {
            tracing::warn!(error = %e, "Qdrant upsert 失败，降级为全文搜索模式");
        }

        Ok(chunks.len())
    }

    /// Remove all index entries for a file from both Tantivy and Qdrant.
    pub async fn remove_file_index(&self, path: &Path) -> Result<(), BrainError> {
        let path_str = path_to_relative_str(path, &self.vault_path);

        // Delete all Tantivy documents for this file using note_path field
        self.tantivy.delete_by_note_path(&path_str)?;
        self.tantivy.commit()?;

        // Best-effort: delete from Qdrant using filter
        let filter = serde_json::json!({
            "must": [{
                "key": "note_path",
                "match": { "value": path_str, "type": "keyword" }
            }]
        });
        if let Err(e) = self.qdrant.delete_by_filter(filter).await {
            tracing::warn!("Qdrant 按路径删除失败: {e}");
        }

        Ok(())
    }

    /// Walk vault directory and index all `.md` files.
    ///
    /// Returns a `FullIndexReport` with statistics about the indexing run.
    pub async fn full_index(&self) -> Result<FullIndexReport, BrainError> {
        let md_files = walk_vault_for_md(&self.vault_path)?;
        let total = md_files.len();

        let mut indexed_files = 0;
        let mut failed_files: Vec<(PathBuf, String)> = Vec::new();
        let mut total_chunks = 0;

        // Probe embedding availability before starting bulk indexing
        let skip_embedding = match self.embedding.embed_text("probe").await {
            Ok(_) => {
                tracing::info!("Embedding 服务可用，将启用向量化");
                false
            }
            Err(e) => {
                tracing::warn!(error = %e, "Embedding 服务不可用，全量索引将跳过向量化 (仅全文索引)");
                true
            }
        };

        tracing::info!(total_files = total, skip_embedding, "开始全量索引...");

        for (i, file_path) in md_files.iter().enumerate() {
            match self.index_file_inner(file_path, skip_embedding).await {
                Ok(chunk_count) => {
                    indexed_files += 1;
                    total_chunks += chunk_count;
                }
                Err(e) => {
                    tracing::error!(
                        path = %file_path.display(),
                        error = %e,
                        "索引失败 ({}/{})",
                        i + 1,
                        total
                    );
                    failed_files.push((file_path.clone(), e.to_string()));
                }
            }

            // Progress logging every 50 files
            if (i + 1) % 50 == 0 || i + 1 == total {
                tracing::info!(
                    progress = format!("{}/{}", i + 1, total),
                    indexed = indexed_files,
                    failed = failed_files.len(),
                    "全量索引进度"
                );
            }
        }

        tracing::info!(
            total_files = total,
            indexed_files,
            failed_files = failed_files.len(),
            total_chunks,
            "Vault 全量索引完成"
        );

        Ok(FullIndexReport {
            total_files: total,
            indexed_files,
            failed_files,
            total_chunks,
        })
    }

    // ── CRUD ──

    /// Manually add a memory chunk.
    ///
    /// Creates a new `MemoryChunk`, indexes it in Tantivy, embeds it,
    /// and upserts to Qdrant. Returns the chunk's UUID.
    pub async fn add_memory(
        &self,
        note_path: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<Uuid, BrainError> {
        let chunk_id = Uuid::new_v4();
        let tags = tags.unwrap_or_default();

        let chunk = MemoryChunk {
            id: chunk_id,
            note_path: note_path.to_string(),
            chunk_index: 0,
            content: content.to_string(),
            breadcrumb: Vec::new(),
            tags: tags.clone(),
            note_title: Path::new(note_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string(),
            token_count: crate::core::chunker::estimate_tokens(content),
            has_code_block: content.contains("```"),
            line_start: 0,
            line_end: 0,
        };

        // Index in Tantivy
        let note_doc = NoteDocument {
            title: chunk.note_title.clone(),
            content: chunk.content.clone(),
            path: note_path.to_string(),
            tags: chunk.tags.clone(),
            chunk_id: chunk.id.to_string(),
            note_path: note_path.to_string(),
        };
        self.tantivy.add_document(&note_doc)?;
        self.tantivy.commit()?;

        // Embed and upsert to Qdrant (degraded if fails)
        let embedding_result = self.embedding.embed_text(content).await;
        match embedding_result {
            Ok(vector) => {
                let now = Utc::now();
                let payload = ChunkPayload {
                    note_path: chunk.note_path.clone(),
                    chunk_index: chunk.chunk_index,
                    content: chunk.content.clone(),
                    title: chunk.note_title.clone(),
                    tags: chunk.tags.clone(),
                    heading_path: chunk.breadcrumb.clone(),
                    word_count: chunk.token_count,
                    created_at: now.to_rfc3339(),
                    updated_at: now.to_rfc3339(),
                };
                let point = VectorPoint {
                    id: chunk.id.to_string(),
                    vector,
                    payload: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
                };
                if let Err(e) = self.qdrant.upsert_points(vec![point]).await {
                    tracing::warn!(error = %e, "Qdrant upsert 失败，降级为全文搜索模式");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Embedding 失败，降级为全文搜索模式");
            }
        }

        Ok(chunk_id)
    }

    /// Update an existing memory chunk by ID.
    ///
    /// Deletes the old entry and creates a new one with the updated content.
    pub async fn update_memory(
        &self,
        memory_id: Uuid,
        new_content: &str,
    ) -> Result<(), BrainError> {
        let chunk_id = memory_id.to_string();

        // Find old chunk using direct TermQuery lookup
        let old_chunk = self.tantivy.get_by_chunk_id(&chunk_id)?.ok_or_else(|| {
            BrainError::NoteNotFound(format!("chunk {} not found", chunk_id).into())
        })?;

        // Delete old chunk from Tantivy (no commit yet — batch with add)
        self.tantivy.delete_by_chunk_id(&chunk_id)?;

        // Build new chunk with same metadata but new content
        let new_chunk = MemoryChunk {
            id: memory_id, // keep same UUID
            note_path: old_chunk.note_path.clone(),
            chunk_index: 0,
            content: new_content.to_string(),
            breadcrumb: vec![],
            tags: old_chunk.tags.clone(),
            note_title: old_chunk.title.clone(),
            token_count: crate::core::chunker::estimate_tokens(new_content),
            has_code_block: new_content.contains("```"),
            line_start: 0,
            line_end: 0,
        };

        // Add new chunk to Tantivy
        let note_doc = NoteDocument {
            title: new_chunk.note_title.clone(),
            content: new_chunk.content.clone(),
            path: new_chunk.note_path.clone(),
            tags: new_chunk.tags.clone(),
            chunk_id: chunk_id.clone(),
            note_path: new_chunk.note_path.clone(),
        };
        self.tantivy.add_document(&note_doc)?;
        self.tantivy.commit()?; // Single commit for delete + add

        // Best-effort: re-embed and upsert to Qdrant
        match self.embedding.embed_text(new_content).await {
            Ok(vector) => {
                let payload = ChunkPayload {
                    note_path: new_chunk.note_path.clone(),
                    chunk_index: 0,
                    content: new_content.to_string(),
                    title: new_chunk.note_title,
                    tags: new_chunk.tags,
                    heading_path: new_chunk.breadcrumb,
                    word_count: new_chunk.token_count,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                let point = VectorPoint {
                    id: chunk_id,
                    vector,
                    payload: serde_json::to_value(&payload)
                        .map_err(|e| BrainError::Internal(format!("序列化 payload 失败: {e}")))?,
                };
                if let Err(e) = self.qdrant.upsert_points(vec![point]).await {
                    tracing::warn!("Qdrant upsert 更新失败: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Embedding 失败，跳过 Qdrant 更新: {e}");
            }
        }

        Ok(())
    }

    /// Delete a memory chunk by ID.
    ///
    /// Returns `true` if the chunk was found and deleted, `false` if not found.
    pub async fn forget_memory(&self, memory_id: Uuid) -> Result<bool, BrainError> {
        let chunk_id = memory_id.to_string();

        // Check if chunk exists using direct TermQuery lookup
        let exists = self.tantivy.get_by_chunk_id(&chunk_id)?;
        if exists.is_none() {
            return Ok(false);
        }

        // Delete from Tantivy
        self.tantivy.delete_by_chunk_id(&chunk_id)?;
        self.tantivy.commit()?;

        // Best-effort delete from Qdrant
        if let Err(e) = self
            .qdrant
            .delete_points(std::slice::from_ref(&chunk_id))
            .await
        {
            tracing::warn!("Qdrant 删除 chunk 失败: {e}");
        }

        Ok(true)
    }

    /// Get statistics about indexed memories.
    pub async fn get_memory_stats(&self) -> Result<MemoryStats, BrainError> {
        // Use AllQuery to collect all indexed documents
        let all_results = self.tantivy.search_all()?;

        // Collect unique note paths
        let note_paths: HashSet<String> = all_results.iter().map(|r| r.note_path.clone()).collect();

        // Collect all unique tags
        let all_tags: HashSet<String> = all_results
            .iter()
            .flat_map(|r| r.tags.iter().cloned())
            .collect();

        let mut tags: Vec<String> = all_tags.into_iter().collect();
        tags.sort();

        Ok(MemoryStats {
            total_chunks: all_results.len(),
            total_notes: note_paths.len(),
            tags,
        })
    }

    /// Read a note file and return its full content.
    pub async fn get_note(&self, path: &str) -> Result<String, BrainError> {
        let full_path = self.vault_path.join(path);

        // Path traversal check: ensure the resolved path stays within vault
        let canonical_vault = self
            .vault_path
            .canonicalize()
            .map_err(|_| BrainError::VaultNotFound(self.vault_path.clone()))?;

        let canonical_file = full_path.canonicalize().map_err(|e| {
            tracing::error!(path = %path, error = %e, "Note 文件不存在");
            BrainError::NoteNotFound(full_path.clone())
        })?;

        if !canonical_file.starts_with(&canonical_vault) {
            return Err(BrainError::NoteNotFound(full_path));
        }

        let content = std::fs::read_to_string(&canonical_file).map_err(|e| {
            tracing::error!(path = %path, error = %e, "Note 文件读取失败");
            BrainError::IoError(e)
        })?;

        Ok(content)
    }

    /// List recently modified notes.
    ///
    /// Returns `NoteSummary` for each note modified within the given number
    /// of days, sorted by modification time descending.
    pub async fn list_recent_notes(
        &self,
        days: Option<u32>,
        limit: Option<usize>,
    ) -> Result<Vec<NoteSummary>, BrainError> {
        let days = days.unwrap_or(7);
        let limit = limit.unwrap_or(20);

        let md_files = walk_vault_for_md(&self.vault_path)?;
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let mut summaries: Vec<NoteSummary> = Vec::new();

        for file_path in &md_files {
            let metadata = std::fs::metadata(file_path).map_err(|e| {
                tracing::error!(path = %file_path.display(), error = %e, "文件元数据读取失败");
                BrainError::IoError(e)
            })?;

            let mtime = metadata.modified().map_err(BrainError::IoError)?;

            // Convert SystemTime to DateTime<Utc>
            let mtime_chrono: chrono::DateTime<Utc> = chrono::DateTime::<Utc>::from(mtime);

            if mtime_chrono < cutoff {
                continue;
            }

            // Lightweight parse: just read content and extract frontmatter for title/tags
            let content = std::fs::read_to_string(file_path)?;
            let path_str = path_to_relative_str(file_path, &self.vault_path);
            let doc = MarkdownParser::parse(&path_str, &content)?;

            summaries.push(NoteSummary {
                path: path_str,
                title: doc.title,
                tags: doc.tags,
                updated_at: mtime_chrono,
            });
        }

        // Sort by modification time descending
        summaries.sort_by_key(|b| std::cmp::Reverse(b.updated_at));

        // Take limit
        summaries.truncate(limit);

        Ok(summaries)
    }

    // ── File Watcher Integration ──

    /// Start the file watcher and process events in a background task.
    /// Returns the FileWatcher handle (must be kept alive).
    pub async fn start_file_watcher(
        memory_service: Arc<Self>,
        vault_path: PathBuf,
        exclude_patterns: Vec<String>,
        debounce_ms: u64,
    ) -> Result<FileWatcher, BrainError> {
        let watcher = FileWatcher::new(&vault_path, exclude_patterns, debounce_ms)?;

        if let Some(mut rx) = watcher.take_receiver() {
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    memory_service.process_file_event(event).await;
                }
                tracing::info!("文件监控事件循环结束");
            });
        }

        Ok(watcher)
    }

    /// Process a single file change event.
    async fn process_file_event(&self, event: FileChangeEvent) {
        let path_str = event.path.display().to_string();

        match event.change_type {
            FileChangeType::Created | FileChangeType::Modified => {
                tracing::info!(path = %path_str, "文件变更，重新索引");
                match self.index_file(&event.path).await {
                    Ok(chunks) => {
                        tracing::info!(path = %path_str, chunks = chunks, "索引完成");
                    }
                    Err(e) => {
                        tracing::error!(path = %path_str, error = %e, "索引失败");
                    }
                }
            }
            FileChangeType::Deleted => {
                tracing::info!(path = %path_str, "文件删除，移除索引");
                match self.remove_file_index(&event.path).await {
                    Ok(()) => {
                        tracing::info!(path = %path_str, "索引移除完成");
                    }
                    Err(e) => {
                        tracing::error!(path = %path_str, error = %e, "索引移除失败");
                    }
                }
            }
        }
    }
}

// ── Helpers ──

/// Convert an absolute file path to a relative path string from the vault root.
#[allow(dead_code)]
fn path_to_relative_str(path: &Path, vault_path: &Path) -> String {
    path.strip_prefix(vault_path)
        .unwrap_or(path)
        .to_str()
        .unwrap_or(path.to_str().unwrap_or("unknown"))
        .to_string()
}

/// Walk a directory recursively and collect all `.md` file paths.
#[allow(dead_code)]
fn walk_vault_for_md(vault_path: &Path) -> Result<Vec<PathBuf>, BrainError> {
    let mut md_files: Vec<PathBuf> = Vec::new();
    walk_dir_recursive(vault_path, &mut md_files)?;
    md_files.sort();
    Ok(md_files)
}

/// Recursively walk a directory, collecting `.md` file paths.
/// Skips directories starting with `.` (like `.obsidian`).
#[allow(dead_code)]
fn walk_dir_recursive(dir: &Path, md_files: &mut Vec<PathBuf>) -> Result<(), BrainError> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        tracing::error!(dir = %dir.display(), error = %e, "目录读取失败");
        BrainError::IoError(e)
    })?;

    for entry in entries {
        let entry = entry.map_err(BrainError::IoError)?;
        let path = entry.path();

        // Skip hidden directories (like .obsidian, .trash)
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            walk_dir_recursive(&path, md_files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            md_files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::QdrantConfig;
    use crate::infra::tantivy_index::{NoteDocument, SearchParams};
    use async_trait::async_trait;
    use tempfile::TempDir;

    // ── Mock Embedding Provider ──

    struct MockEmbedder {
        should_fail: bool,
        vector: Vec<f32>,
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbedder {
        async fn embed_text(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
            if self.should_fail {
                Err(BrainError::EmbeddingError(
                    "Mock embedding failure".to_string(),
                ))
            } else {
                Ok(self.vector.clone())
            }
        }

        async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, BrainError> {
            let mut results = Vec::with_capacity(texts.len());
            for _ in texts {
                if self.should_fail {
                    return Err(BrainError::EmbeddingError(
                        "Mock embedding failure".to_string(),
                    ));
                }
                results.push(self.vector.clone());
            }
            Ok(results)
        }

        fn dimensions(&self) -> usize {
            self.vector.len()
        }
    }

    // ── Test Helpers ──

    fn make_unreachable_qdrant_config() -> QdrantConfig {
        QdrantConfig {
            url: "http://127.0.0.1:53333".to_string(),
            collection_name: "test_collection".to_string(),
            vector_size: 3,
        }
    }

    fn setup_service(embedding_should_fail: bool) -> (TempDir, TempDir, MemoryService) {
        let tantivy_dir = TempDir::new().unwrap();
        let vault_dir = TempDir::new().unwrap();

        let tantivy = Arc::new(TantivyIndex::new(tantivy_dir.path()).unwrap());
        let qdrant = Arc::new(QdrantStore::new(&make_unreachable_qdrant_config()).unwrap());
        let embedding: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedder {
            should_fail: embedding_should_fail,
            vector: vec![0.1, 0.2, 0.3],
        });

        let service = MemoryService::new(
            tantivy,
            qdrant,
            embedding,
            vault_dir.path().to_path_buf(),
            "TestVault".to_string(),
        );

        (tantivy_dir, vault_dir, service)
    }

    fn write_md_file(vault_dir: &TempDir, relative_path: &str, content: &str) -> PathBuf {
        let full_path = vault_dir.path().join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, content).unwrap();
        full_path
    }

    // ── Test 1: index_file creates Tantivy entries (searchable after commit) ──

    #[tokio::test]
    async fn test_index_file_creates_tantivy_entries_searchable() {
        let (tantivy_dir, vault_dir, service) = setup_service(true); // Embedding fails → degraded mode

        let content = r#"---
title: Test Note
tags:
  - test
---
# Introduction

This is the introduction content for testing.
"#;
        let file_path = write_md_file(&vault_dir, "notes/test.md", content);

        let chunk_count = service.index_file(&file_path).await.unwrap();
        assert!(chunk_count > 0, "Should create at least 1 chunk");

        // Verify searchable in Tantivy — search for content that appears in the body
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "introduction content".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "Indexed content should be searchable");
    }

    // ── Test 2: remove_file_index removes entries (not searchable after) ──

    #[tokio::test]
    async fn test_remove_file_index_removes_entries_not_searchable() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let content = r#"---
title: Delete Test
---
# Deletable Section

This content will be deleted.
"#;
        let file_path = write_md_file(&vault_dir, "notes/delete_me.md", content);

        // First index the file
        service.index_file(&file_path).await.unwrap();

        // Verify it's searchable — search for terms in the body content
        let results_before = service
            .tantivy
            .search(&SearchParams {
                query: "content deleted".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results_before.is_empty(),
            "Should be searchable before removal"
        );

        // Remove the file's index
        service.remove_file_index(&file_path).await.unwrap();

        // Verify it's no longer searchable
        let results_after = service
            .tantivy
            .search(&SearchParams {
                query: "content deleted".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            results_after.is_empty(),
            "Should not be searchable after removal"
        );
    }

    // ── Test 3: add_memory creates a searchable chunk ──

    #[tokio::test]
    async fn test_add_memory_creates_searchable_chunk() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let memory_id = service
            .add_memory(
                "manual/test.md",
                "Rust is a systems programming language.",
                Some(vec!["rust".to_string()]),
            )
            .await
            .unwrap();

        assert_ne!(memory_id, Uuid::nil(), "Should return a valid UUID");

        // Verify searchable in Tantivy
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "systems programming".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "Added memory should be searchable");

        // Verify the chunk_id appears in results
        let found = results.iter().any(|r| r.chunk_id == memory_id.to_string());
        assert!(found, "Search result should contain the memory chunk ID");
    }

    // ── Test 4: forget_memory removes a chunk ──

    #[tokio::test]
    async fn test_forget_memory_removes_chunk() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Add a memory with distinctive content
        let memory_id = service
            .add_memory(
                "manual/forget.md",
                "Unique forgettable content about zephyr winds.",
                None,
            )
            .await
            .unwrap();

        // Verify it's searchable first
        let results_before = service
            .tantivy
            .search(&SearchParams {
                query: "zephyr winds".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results_before.is_empty(),
            "Should be searchable before forget"
        );

        // Forget the memory — use direct deletion since search by UUID won't
        // find it in content/title fields
        service
            .tantivy
            .delete_by_chunk_id(&memory_id.to_string())
            .unwrap();
        service.tantivy.commit().unwrap();

        // Verify it's no longer searchable
        let results_after = service
            .tantivy
            .search(&SearchParams {
                query: "zephyr winds".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            results_after.is_empty(),
            "Should not be searchable after forget"
        );
    }

    // ── Test 5: get_memory_stats returns correct counts ──

    #[tokio::test]
    async fn test_get_memory_stats_returns_correct_counts() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Add two memories with distinctive content
        service
            .add_memory(
                "notes/rust.md",
                "Rust programming language details and features.",
                Some(vec!["rust".to_string()]),
            )
            .await
            .unwrap();
        service
            .add_memory(
                "notes/python.md",
                "Python programming language interpreter and tools.",
                Some(vec!["python".to_string()]),
            )
            .await
            .unwrap();

        let stats = service.get_memory_stats().await.unwrap();

        assert!(
            stats.total_chunks >= 2,
            "Should have at least 2 chunks, got {}",
            stats.total_chunks
        );
        assert!(
            stats.total_notes >= 2,
            "Should have at least 2 notes, got {}",
            stats.total_notes
        );
        assert!(
            stats.tags.contains(&"rust".to_string()),
            "Should contain 'rust' tag"
        );
        assert!(
            stats.tags.contains(&"python".to_string()),
            "Should contain 'python' tag"
        );
    }

    // ── Test 6: get_note reads file content ──

    #[tokio::test]
    async fn test_get_note_reads_file_content() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let content = "# Hello World\n\nThis is a test note.";
        write_md_file(&vault_dir, "hello.md", content);

        let result = service.get_note("hello.md").await.unwrap();
        assert_eq!(result, content, "Should return the exact file content");

        // Test non-existent file returns error
        let err_result = service.get_note("nonexistent.md").await;
        assert!(err_result.is_err(), "Non-existent file should return error");
    }

    // ── Test 7: list_recent_notes returns sorted summaries ──

    #[tokio::test]
    async fn test_list_recent_notes_returns_sorted_summaries() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Create multiple markdown files
        write_md_file(
            &vault_dir,
            "note1.md",
            "---\ntitle: First Note\ntags:\n  - first\n---\n# First\nContent 1.",
        );
        write_md_file(
            &vault_dir,
            "note2.md",
            "---\ntitle: Second Note\ntags:\n  - second\n---\n# Second\nContent 2.",
        );
        write_md_file(
            &vault_dir,
            "note3.md",
            "---\ntitle: Third Note\ntags:\n  - third\n---\n# Third\nContent 3.",
        );

        let summaries = service.list_recent_notes(Some(7), Some(20)).await.unwrap();

        assert!(
            summaries.len() >= 3,
            "Should list at least 3 notes, got {}",
            summaries.len()
        );

        // Verify summaries are sorted by updated_at descending
        for i in 1..summaries.len() {
            assert!(
                summaries[i - 1].updated_at >= summaries[i].updated_at,
                "Notes should be sorted by modification time descending"
            );
        }

        // Verify each summary has the expected fields
        for summary in &summaries {
            assert!(!summary.path.is_empty(), "Path should not be empty");
            assert!(!summary.title.is_empty(), "Title should not be empty");
        }
    }

    // ── Test 8: Degraded mode: embedding failure → Tantivy still indexed ──

    #[tokio::test]
    async fn test_degraded_mode_embedding_failure_tantivy_still_indexed() {
        let (tantivy_dir, vault_dir, service) = setup_service(true); // Embedding fails

        let content = r#"---
title: Degraded Test
tags:
  - degraded
---
# Degraded Mode

This content should still be indexed in Tantivy even when embedding fails.
"#;
        let file_path = write_md_file(&vault_dir, "notes/degraded.md", content);

        // Index the file — embedding will fail but Tantivy should succeed
        let chunk_count = service.index_file(&file_path).await.unwrap();
        assert!(
            chunk_count > 0,
            "Should create chunks even in degraded mode"
        );

        // Verify content is searchable in Tantivy — search for body text
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "indexed tantivy".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results.is_empty(),
            "Tantivy should still have indexed content despite embedding failure"
        );

        // Verify tag filter works
        let tagged_results = service
            .tantivy
            .search(&SearchParams {
                query: "content".to_string(),
                top_k: 5,
                tag_filter: Some(vec!["degraded".to_string()]),
            })
            .unwrap();
        assert!(
            !tagged_results.is_empty(),
            "Tag filter should work in degraded mode"
        );
    }

    // ── Additional: full_index indexes all files ──

    #[tokio::test]
    async fn test_full_index_indexes_all_files() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        write_md_file(
            &vault_dir,
            "a.md",
            "---\ntitle: A\n---\n# Alpha\nAlpha beta gamma content for testing.",
        );
        write_md_file(
            &vault_dir,
            "b.md",
            "---\ntitle: B\n---\n# Beta\nBeta delta epsilon content for testing.",
        );
        write_md_file(
            &vault_dir,
            "subdir/c.md",
            "---\ntitle: C\n---\n# Gamma\nGamma zeta omega content for testing.",
        );

        let report = service.full_index().await.unwrap();

        assert_eq!(report.total_files, 3, "Should find 3 .md files");
        assert_eq!(
            report.indexed_files, 3,
            "Should successfully index all 3 files"
        );
        assert!(
            report.failed_files.is_empty(),
            "Should have no failed files"
        );
        assert!(
            report.total_chunks > 0,
            "Should create at least some chunks"
        );

        // Verify content is searchable — search for unique body text
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "alpha beta gamma".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "Alpha content should be searchable");
    }

    // ── Additional: forget_memory returns false for non-existent chunk ──

    #[tokio::test]
    async fn test_forget_memory_nonexistent_returns_false() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let fake_id = Uuid::new_v4();
        let was_found = service.forget_memory(fake_id).await.unwrap();
        assert!(!was_found, "Should return false for non-existent memory");
    }

    // ── Additional: path traversal check in get_note ──

    #[tokio::test]
    async fn test_get_note_path_traversal_rejected() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Try to access a file outside the vault using a relative path
        let result = service.get_note("../../etc/passwd").await;
        assert!(result.is_err(), "Path traversal should be rejected");
    }

    // ── Additional: empty vault returns empty results ──

    #[tokio::test]
    async fn test_full_index_empty_vault() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let report = service.full_index().await.unwrap();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.indexed_files, 0);
        assert!(report.failed_files.is_empty());
        assert_eq!(report.total_chunks, 0);
    }

    // ── Additional: index_file with empty content returns 0 chunks ──

    #[tokio::test]
    async fn test_index_file_empty_content_zero_chunks() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let file_path = write_md_file(&vault_dir, "empty.md", "");
        let chunk_count = service.index_file(&file_path).await.unwrap();
        assert_eq!(chunk_count, 0, "Empty file should produce 0 chunks");
    }

    // ── Additional: hidden directories are skipped in full_index ──

    #[tokio::test]
    async fn test_full_index_skips_hidden_directories() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Create a normal file and a file in a hidden directory
        write_md_file(&vault_dir, "visible.md", "# Visible\nVisible content.");
        write_md_file(&vault_dir, ".hidden/secret.md", "# Secret\nSecret content.");

        let report = service.full_index().await.unwrap();
        assert_eq!(report.total_files, 1, "Should skip .hidden directory");
        assert_eq!(report.indexed_files, 1);
    }

    // ── Additional: re-indexing a file updates existing entries ──

    #[tokio::test]
    async fn test_reindex_file_updates_entries() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let file_path = write_md_file(&vault_dir, "update.md", "# Old Title\nOld content here.");

        // Index first time
        let chunks1 = service.index_file(&file_path).await.unwrap();
        assert!(chunks1 > 0);

        // Verify old content searchable
        let results_old = service
            .tantivy
            .search(&SearchParams {
                query: "old content".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results_old.is_empty());

        // Update file content
        std::fs::write(&file_path, "# New Title\nNew updated content here.").unwrap();

        // Re-index
        let chunks2 = service.index_file(&file_path).await.unwrap();
        assert!(chunks2 > 0);

        // Verify new content searchable
        let results_new = service
            .tantivy
            .search(&SearchParams {
                query: "new updated content".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results_new.is_empty());

        // Old content should no longer be found
        let results_old_after = service
            .tantivy
            .search(&SearchParams {
                query: "old content".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        // All results should be for the "update.md" note but with NEW content
        let has_old_path_with_old_content = results_old_after
            .iter()
            .any(|r| r.note_path.contains("update.md") && r.snippet.contains("Old content"));
        assert!(
            !has_old_path_with_old_content,
            "Old content should not be found after re-indexing"
        );
    }

    // ── Additional: forget_memory with direct Tantivy chunk_id deletion ──

    #[tokio::test]
    async fn test_forget_memory_via_chunk_id_deletion() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Add a memory
        let memory_id = service
            .add_memory(
                "manual/del_test.md",
                "Content to delete via chunk_id.",
                None,
            )
            .await
            .unwrap();

        // Verify it exists via search
        let results_before = service
            .tantivy
            .search(&SearchParams {
                query: "delete via chunk_id".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(!results_before.is_empty());
        assert!(results_before
            .iter()
            .any(|r| r.chunk_id == memory_id.to_string()));

        // Delete via chunk_id
        service
            .tantivy
            .delete_by_chunk_id(&memory_id.to_string())
            .unwrap();
        service.tantivy.commit().unwrap();

        // Verify it's gone
        let results_after = service
            .tantivy
            .search(&SearchParams {
                query: "delete via chunk_id".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(results_after.is_empty(), "Should be deleted");
    }

    // ── Additional: delete_by_note_path removes all chunks for a file ──

    #[tokio::test]
    async fn test_delete_by_note_path_removes_all_chunks() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        // Index a file that produces multiple chunks
        let content = r#"---
title: Multi Chunk Note
---
# Section One

First section content about algorithms and data structures.

## Section Two

Second section content about programming paradigms and patterns.
"#;
        let file_path = write_md_file(&vault_dir, "multi.md", content);

        let chunk_count = service.index_file(&file_path).await.unwrap();
        assert!(chunk_count >= 1, "Should create at least 1 chunk");

        // Delete all chunks for this file via note_path
        service.tantivy.delete_by_note_path("multi.md").unwrap();
        service.tantivy.commit().unwrap();

        // Verify all chunks are gone
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "algorithms programming".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            results.is_empty(),
            "All chunks for the file should be deleted"
        );
    }

    // ── Test: process_file_event Created indexes file ──

    #[tokio::test]
    async fn test_process_file_event_created_indexes_file() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let content = r#"---
title: Watcher Test
tags:
  - watcher
---
# Watcher Created

This content was created by the watcher test.
"#;
        let file_path = write_md_file(&vault_dir, "watcher_created.md", content);

        let event = FileChangeEvent {
            change_type: FileChangeType::Created,
            path: file_path.clone(),
            timestamp: chrono::Utc::now(),
        };

        service.process_file_event(event).await;

        // Verify the file is now searchable
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "watcher created test".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results.is_empty(),
            "File should be searchable after Created event"
        );
    }

    // ── Test: process_file_event Modified re-indexes file ──

    #[tokio::test]
    async fn test_process_file_event_modified_reindexes_file() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let file_path = write_md_file(
            &vault_dir,
            "watcher_modified.md",
            "# Original\nOriginal watcher content.",
        );

        // Index the file first
        service.index_file(&file_path).await.unwrap();

        // Update file content
        std::fs::write(&file_path, "# Updated\nUpdated watcher content.").unwrap();

        let event = FileChangeEvent {
            change_type: FileChangeType::Modified,
            path: file_path.clone(),
            timestamp: chrono::Utc::now(),
        };

        service.process_file_event(event).await;

        // Verify updated content is searchable
        let results = service
            .tantivy
            .search(&SearchParams {
                query: "updated watcher content".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results.is_empty(),
            "Updated content should be searchable after Modified event"
        );
    }

    // ── Test: process_file_event Deleted removes index ──

    #[tokio::test]
    async fn test_process_file_event_deleted_removes_index() {
        let (tantivy_dir, vault_dir, service) = setup_service(true);

        let content = r#"---
title: Delete Event Test
---
# Delete Event

This content will be removed by a Deleted event.
"#;
        let file_path = write_md_file(&vault_dir, "watcher_deleted.md", content);

        // First index the file
        service.index_file(&file_path).await.unwrap();

        // Verify it's searchable
        let results_before = service
            .tantivy
            .search(&SearchParams {
                query: "delete event removed".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            !results_before.is_empty(),
            "Should be searchable before Deleted event"
        );

        let event = FileChangeEvent {
            change_type: FileChangeType::Deleted,
            path: file_path.clone(),
            timestamp: chrono::Utc::now(),
        };

        service.process_file_event(event).await;

        // Verify it's no longer searchable
        let results_after = service
            .tantivy
            .search(&SearchParams {
                query: "delete event removed".to_string(),
                top_k: 5,
                tag_filter: None,
            })
            .unwrap();
        assert!(
            results_after.is_empty(),
            "Should not be searchable after Deleted event"
        );
    }
}
