use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::BrainError;
use crate::infra::book_wiki_store::{
    stable_id, BookWikiStore, MarkdownSourceDraft, SourceSectionDraft,
};
use crate::models::book_wiki::KnowledgeBaseSummary;

const MAX_MARKDOWN_BYTES: u64 = 10 * 1024 * 1024;
const MAX_SCAN_DEPTH: usize = 24;
const SKIP_DIRECTORIES: &[&str] = &[
    ".git",
    ".obsidian",
    "node_modules",
    "target",
    "dist",
    "build",
    ".cache",
];

#[derive(Clone)]
pub struct BookWikiService {
    store: BookWikiStore,
}

#[derive(Serialize, Clone, Debug)]
pub struct SyncKnowledgeBaseResult {
    pub knowledge_base: KnowledgeBaseSummary,
    pub scanned_sources: usize,
    pub indexed_entries: usize,
    pub requires_harness: bool,
    pub message: String,
}

impl BookWikiService {
    pub fn new(store: BookWikiStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &BookWikiStore {
        &self.store
    }

    pub fn initialize_and_sync(
        &self,
        book_id: &str,
    ) -> Result<SyncKnowledgeBaseResult, BrainError> {
        let base = self.store.initialize_base(book_id)?;
        self.sync(&base.id)
    }

    pub fn sync(&self, base_id: &str) -> Result<SyncKnowledgeBaseResult, BrainError> {
        let base = self.store.get_base(base_id)?;
        self.store
            .set_sync_state(base_id, "scanning", &base.health_state, None)?;

        let result = match base.book_kind.as_str() {
            "folder" => self.sync_markdown_folder(&base),
            "pdf" => self.sync_pdf(&base),
            kind => Err(BrainError::KnowledgeValidation(format!(
                "不支持的书籍类型: {kind}"
            ))),
        };

        if let Err(error) = &result {
            let _ =
                self.store
                    .set_sync_state(base_id, "failed", "warning", Some(&error.to_string()));
        }
        result
    }

    fn sync_markdown_folder(
        &self,
        base: &KnowledgeBaseSummary,
    ) -> Result<SyncKnowledgeBaseResult, BrainError> {
        let root = PathBuf::from(&base.book_path);
        if !root.is_dir() {
            return Err(BrainError::KnowledgeValidation(format!(
                "书籍目录不存在或不可访问: {}",
                root.display()
            )));
        }

        let mut paths = Vec::new();
        collect_markdown_files(&root, 0, &mut paths)?;
        paths.sort();

        let mut sources = Vec::with_capacity(paths.len());
        let mut indexed_entries = 0;
        for (ordinal, path) in paths.iter().enumerate() {
            let metadata = std::fs::metadata(path)?;
            if metadata.len() > MAX_MARKDOWN_BYTES {
                tracing::warn!(path = %path.display(), "跳过超过 10MB 的 Markdown 来源");
                continue;
            }
            let content = std::fs::read_to_string(path).map_err(|error| {
                BrainError::KnowledgeValidation(format!(
                    "Markdown 不是有效 UTF-8 或无法读取 {}: {error}",
                    path.display()
                ))
            })?;
            let relative_path = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let title = document_title(path, &content);
            let content_hash = hash_text(&content);
            let source_id = stable_id("source", &format!("{}:{}", base.id, path.display()));
            let version_id = stable_id("version", &format!("{source_id}:{content_hash}"));
            let sections = split_markdown_sections(&content, &relative_path, &source_id);
            indexed_entries += sections.len();
            sources.push(MarkdownSourceDraft {
                id: source_id,
                version_id,
                original_path: path.to_string_lossy().to_string(),
                relative_path,
                title,
                ordinal: ordinal as i64,
                content_hash,
                size_bytes: metadata.len() as i64,
                modified_at: metadata.modified().ok().map(system_time_to_rfc3339),
                sections,
            });
        }

        self.store.replace_markdown_sources(&base.id, &sources)?;
        let knowledge_base = self.store.get_base(&base.id)?;
        let message = if sources.is_empty() {
            "目录中没有找到可摄入的 Markdown 文件".to_string()
        } else {
            format!(
                "已扫描 {} 个 Markdown 文件，建立 {} 个可检索章节",
                sources.len(),
                indexed_entries
            )
        };
        Ok(SyncKnowledgeBaseResult {
            knowledge_base,
            scanned_sources: sources.len(),
            indexed_entries,
            requires_harness: false,
            message,
        })
    }

    fn sync_pdf(&self, base: &KnowledgeBaseSummary) -> Result<SyncKnowledgeBaseResult, BrainError> {
        let path = PathBuf::from(&base.book_path);
        if !path.is_file() {
            return Err(BrainError::KnowledgeValidation(format!(
                "PDF 文件不存在或不可访问: {}",
                path.display()
            )));
        }
        let metadata = std::fs::metadata(&path)?;
        let hash = hash_file(&path)?;
        self.store.register_pdf_source(
            &base.id,
            &base.book_path,
            &base.book_name,
            &hash,
            metadata.len() as i64,
            metadata
                .modified()
                .ok()
                .map(system_time_to_rfc3339)
                .as_deref(),
        )?;
        Ok(SyncKnowledgeBaseResult {
            knowledge_base: self.store.get_base(&base.id)?,
            scanned_sources: 1,
            indexed_entries: 0,
            requires_harness: true,
            message: "PDF 来源已登记；需要接通 DeepSeek Harness 后执行版面提取与知识建模"
                .to_string(),
        })
    }
}

fn collect_markdown_files(
    directory: &Path,
    depth: usize,
    output: &mut Vec<PathBuf>,
) -> Result<(), BrainError> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || SKIP_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_markdown_files(&path, depth + 1, output)?;
        } else if file_type.is_file() && is_markdown(&path) {
            output.push(path);
        }
    }
    Ok(())
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("md") || value.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

fn document_title(path: &Path, content: &str) -> String {
    let mut in_fence = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if is_fence(trimmed) {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            if let Some(title) = heading_title(trimmed) {
                return title;
            }
        }
    }
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名文档")
        .to_string()
}

fn split_markdown_sections(
    content: &str,
    relative_path: &str,
    source_id: &str,
) -> Vec<SourceSectionDraft> {
    let lines: Vec<&str> = content.lines().collect();
    let fallback_title = Path::new(relative_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名文档")
        .to_string();
    let mut boundaries: Vec<(usize, String)> = Vec::new();
    let mut in_fence = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if is_fence(trimmed) {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            if let Some(title) = heading_title(trimmed) {
                boundaries.push((index, title));
            }
        }
    }

    if boundaries.first().is_none_or(|(line, _)| *line > 0) {
        boundaries.insert(0, (0, fallback_title.clone()));
    }
    if lines.is_empty() {
        boundaries.clear();
        boundaries.push((0, fallback_title));
    }

    boundaries
        .iter()
        .enumerate()
        .filter_map(|(ordinal, (start, title))| {
            let end = boundaries
                .get(ordinal + 1)
                .map(|(next, _)| *next)
                .unwrap_or(lines.len());
            let content_md = if lines.is_empty() {
                String::new()
            } else {
                lines[*start..end].join("\n")
            };
            if content_md.trim().is_empty() && !lines.is_empty() {
                return None;
            }
            let identity = format!("{source_id}:{ordinal}:{title}");
            let slug = format!(
                "{}--{}",
                slugify(relative_path),
                stable_id("s", &identity).trim_start_matches("s-")
            );
            let content_hash = hash_text(&content_md);
            Some(SourceSectionDraft {
                id: stable_id("span", &format!("{identity}:{content_hash}")),
                entry_id: stable_id("entry", &identity),
                slug,
                title: title.clone(),
                summary: summarize_markdown(&content_md),
                content_md,
                line_start: *start as i64 + 1,
                line_end: end.max(*start + 1) as i64,
                content_hash,
            })
        })
        .collect()
}

fn heading_title(line: &str) -> Option<String> {
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    if hashes == 0 || hashes > 6 || line.as_bytes().get(hashes) != Some(&b' ') {
        return None;
    }
    let title = line[hashes + 1..].trim().trim_end_matches('#').trim();
    (!title.is_empty()).then(|| title.to_string())
}

fn is_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

fn summarize_markdown(content: &str) -> String {
    let mut summary = String::new();
    let mut in_frontmatter = content.trim_start().starts_with("---");
    for line in content.lines() {
        let trimmed = line.trim();
        if in_frontmatter {
            if trimmed == "---" && !summary.is_empty() {
                in_frontmatter = false;
            } else if trimmed == "---" {
                summary.push(' ');
            }
            continue;
        }
        if trimmed.is_empty() {
            if !summary.is_empty() {
                break;
            }
            continue;
        }
        if heading_title(trimmed).is_some() || is_fence(trimmed) {
            continue;
        }
        let cleaned = trimmed
            .trim_start_matches(['>', '-', '*', '+', ' '])
            .replace(['`', '*', '_'], "");
        if cleaned.is_empty() {
            continue;
        }
        if !summary.is_empty() {
            summary.push(' ');
        }
        summary.push_str(&cleaned);
        if summary.chars().count() >= 180 {
            break;
        }
    }
    let mut chars = summary.chars();
    let shortened: String = chars.by_ref().take(180).collect();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

fn slugify(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn hash_text(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

fn hash_file(path: &Path) -> Result<String, BrainError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn system_time_to_rfc3339(value: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::models::book_wiki::{BookKind, ReaderBook};
    use std::sync::Arc;

    #[test]
    fn test_split_markdown_sections_ignores_headings_inside_fences() {
        let content = "# 第一章\n正文\n```md\n# 不是标题\n```\n## 第二节\n内容";
        let sections = split_markdown_sections(content, "demo.md", "source-1");

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "第一章");
        assert_eq!(sections[1].title, "第二节");
        assert!(sections[0].content_md.contains("# 不是标题"));
    }

    #[test]
    fn test_sync_markdown_folder_creates_database_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let book_path = dir.path().join("book");
        std::fs::create_dir(&book_path).expect("book dir");
        std::fs::write(
            book_path.join("intro.md"),
            "# 入门\n这是第一段。\n\n## 细节\n$d_k=d_v=128$",
        )
        .expect("source write");
        let db = Arc::new(SqliteStore::new(&dir.path().join("test.db")).expect("db"));
        let store = BookWikiStore::new(db);
        store
            .save_reader_books(&[ReaderBook {
                id: "book-1".to_string(),
                path: book_path.to_string_lossy().to_string(),
                kind: BookKind::Folder,
                name: "测试书".to_string(),
                description: String::new(),
                category: String::new(),
                added_at: 1,
                progress: None,
            }])
            .expect("save book");
        let service = BookWikiService::new(store.clone());

        let result = service.initialize_and_sync("book-1").expect("sync");

        assert_eq!(result.scanned_sources, 1);
        assert_eq!(result.indexed_entries, 2);
        assert_eq!(result.knowledge_base.sync_state, "clean");
        let entries = store
            .list_entries(&result.knowledge_base.id, Some("d_k"), None, 10)
            .expect("search");
        assert_eq!(entries.len(), 1);
    }
}
