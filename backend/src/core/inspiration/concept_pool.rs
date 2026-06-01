//! 概念池构建器

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::BrainError;
use crate::models::inspiration::{Concept, ConceptPool, ConceptSource, InspirationConfig};
use crate::infra::sqlite_store::SqliteStore;
use crate::infra::obsidian_client::ObsidianClient;
use crate::infra::llm_client::LlmProvider;

/// 概念池构建器
pub struct ConceptPoolBuilder {
    db: Arc<SqliteStore>,
    obsidian: Option<Arc<ObsidianClient>>,
    config: InspirationConfig,
}

impl ConceptPoolBuilder {
    pub fn new(db: Arc<SqliteStore>, obsidian: Option<Arc<ObsidianClient>>, config: InspirationConfig) -> Self {
        Self { db, obsidian, config }
    }

    /// 构建概念池
    pub async fn build(&self) -> Result<ConceptPool, BrainError> {
        let mut concepts: Vec<Concept> = Vec::new();

        // 1. 从 vault 标签提取概念
        if let Some(obsidian) = &self.obsidian {
            if let Ok(notes) = obsidian.list_all_files().await {
                let mut tag_counts: HashMap<String, Vec<String>> = HashMap::new();

                for note_path in &notes {
                    if !note_path.ends_with(".md") {
                        continue;
                    }
                    // 读取笔记，提取 frontmatter 中的标签
                    if let Ok(content) = obsidian.read_file(note_path).await {
                        let tags = self.extract_tags_from_content(&content);
                        for tag in tags {
                            tag_counts.entry(tag).or_default().push(note_path.clone());
                        }
                    }
                }

                // 计算 TF-IDF 权重（简化版：标签出现频率）
                let total_notes = notes.len() as f64;
                for (tag, note_paths) in tag_counts {
                    let df = note_paths.len() as f64;
                    let tfidf = df / total_notes; // 简化版 TF-IDF
                    if tfidf >= self.config.min_tfidf {
                        concepts.push(Concept {
                            term: tag,
                            weight: tfidf,
                            source: ConceptSource::NoteTag,
                            note_paths,
                        });
                    }
                }
            }
        }

        // 2. 从笔记标题提取关键词（简化版）
        if let Some(obsidian) = &self.obsidian {
            if let Ok(notes) = obsidian.list_all_files().await {
                for note_path in &notes {
                    if let Some(name) = std::path::Path::new(note_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                    {
                        // 清理文件名（移除日期前缀等）
                        let clean_name = name
                            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_')
                            .to_string();

                        if !clean_name.is_empty() && clean_name.len() > 2 {
                            concepts.push(Concept {
                                term: clean_name,
                                weight: 1.0,
                                source: ConceptSource::NoteKeyword,
                                note_paths: vec![note_path.clone()],
                            });
                        }
                    }
                }
            }
        }

        // 3. 按权重排序，限制数量
        concepts.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        concepts.truncate(self.config.max_concepts);

        Ok(ConceptPool {
            concepts,
            built_at: Utc::now(),
        })
    }

    /// 从内容中提取标签
    fn extract_tags_from_content(&self, content: &str) -> Vec<String> {
        let mut tags = Vec::new();

        // 从 frontmatter 提取 tags
        if let Some(fm_start) = content.find("---") {
            if let Some(fm_end) = content[fm_start + 3..].find("---") {
                let fm = &content[fm_start + 3..fm_start + 3 + fm_end];
                for line in fm.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("tags:") {
                        let value = trimmed[5..].trim();
                        if value.starts_with('[') && value.ends_with(']') {
                            // 数组格式: [tag1, tag2]
                            let inner = &value[1..value.len()-1];
                            for item in inner.split(',') {
                                let tag = item.trim().trim_matches('"').trim_matches('\'');
                                if !tag.is_empty() {
                                    tags.push(tag.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 从内容提取 #tag
        for line in content.lines() {
            // 跳过代码块
            if line.trim().starts_with("```") {
                continue;
            }
            // 提取 #tag（排除 ## heading）
            for word in line.split_whitespace() {
                if word.starts_with('#') && !word.starts_with("##") && word.len() > 1 {
                    let tag = word[1..].trim_end_matches(|c: char| c.is_ascii_punctuation());
                    if !tag.is_empty() {
                        tags.push(tag.to_string());
                    }
                }
            }
        }

        tags.sort();
        tags.dedup();
        tags
    }

    /// 计算两个概念之间的距离（Jaccard 距离）
    pub fn compute_distance(concept_a: &Concept, concept_b: &Concept) -> f64 {
        let notes_a: std::collections::HashSet<&String> = concept_a.note_paths.iter().collect();
        let notes_b: std::collections::HashSet<&String> = concept_b.note_paths.iter().collect();

        let intersection = notes_a.intersection(&notes_b).count();
        let union = notes_a.union(&notes_b).count();

        if union == 0 {
            return 0.8; // 默认距离
        }

        let co_occurrence = intersection as f64 / union as f64;
        1.0 - co_occurrence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::sync::Arc;
    use crate::infra::sqlite_store::SqliteStore;

    fn create_builder() -> (TempDir, ConceptPoolBuilder) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(SqliteStore::new(&dir.path().join("test.db")).unwrap());
        let config = InspirationConfig::default();
        let builder = ConceptPoolBuilder::new(db, None, config);
        (dir, builder)
    }

    #[test]
    fn test_extract_tags_from_frontmatter() {
        let (_, builder) = create_builder();
        let content = "---\ntitle: Test\ntags: [rust, async, tokio]\n---\n# Test\nSome content";
        let tags = builder.extract_tags_from_content(content);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"async".to_string()));
        assert!(tags.contains(&"tokio".to_string()));
    }

    #[test]
    fn test_extract_tags_from_inline() {
        let (_, builder) = create_builder();
        let content = "# Test\nThis is a #rust note about #async programming";
        let tags = builder.extract_tags_from_content(content);
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"async".to_string()));
    }

    #[test]
    fn test_extract_tags_skips_headings() {
        let (_, builder) = create_builder();
        let content = "# Test\n## Not a tag\nSome #real-tag content";
        let tags = builder.extract_tags_from_content(content);
        assert!(!tags.contains(&"Not".to_string()));
        assert!(tags.contains(&"real-tag".to_string()));
    }

    #[test]
    fn test_compute_distance_same_notes() {
        let concept_a = Concept {
            term: "a".to_string(),
            weight: 1.0,
            source: ConceptSource::NoteTag,
            note_paths: vec!["note1.md".to_string(), "note2.md".to_string()],
        };
        let concept_b = Concept {
            term: "b".to_string(),
            weight: 1.0,
            source: ConceptSource::NoteTag,
            note_paths: vec!["note1.md".to_string(), "note2.md".to_string()],
        };
        let distance = ConceptPoolBuilder::compute_distance(&concept_a, &concept_b);
        assert!(distance < 0.1); // 应该接近 0（相同笔记集合）
    }

    #[test]
    fn test_compute_distance_different_notes() {
        let concept_a = Concept {
            term: "a".to_string(),
            weight: 1.0,
            source: ConceptSource::NoteTag,
            note_paths: vec!["note1.md".to_string()],
        };
        let concept_b = Concept {
            term: "b".to_string(),
            weight: 1.0,
            source: ConceptSource::NoteTag,
            note_paths: vec!["note2.md".to_string()],
        };
        let distance = ConceptPoolBuilder::compute_distance(&concept_a, &concept_b);
        assert!(distance > 0.9); // 应该接近 1（完全不同）
    }
}
