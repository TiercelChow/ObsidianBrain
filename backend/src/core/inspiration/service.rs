//! 灵感服务

use chrono::Utc;
use serde_json::json;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::core::inspiration::concept_pool::ConceptPoolBuilder;
use crate::core::inspiration::generator::LlmCreativeGenerator;
use crate::core::inspiration::selector::ConceptSelector;
use crate::error::BrainError;
use crate::infra::llm_client::LlmProvider;
use crate::infra::obsidian_client::ObsidianClient;
use crate::infra::sqlite_store::SqliteStore;
use crate::models::inspiration::*;

/// 灵感服务
pub struct InspirationService {
    pool_builder: ConceptPoolBuilder,
    selector: ConceptSelector,
    generator: LlmCreativeGenerator,
    db: Arc<SqliteStore>,
    obsidian: Option<Arc<ObsidianClient>>,
    config: InspirationConfig,
    cached_pool: RwLock<Option<(ConceptPool, chrono::DateTime<Utc>)>>,
}

impl InspirationService {
    pub fn new(
        db: Arc<SqliteStore>,
        obsidian: Option<Arc<ObsidianClient>>,
        llm: Arc<dyn LlmProvider>,
        config: InspirationConfig,
    ) -> Self {
        let pool_builder = ConceptPoolBuilder::new(db.clone(), obsidian.clone(), config.clone());
        let selector = ConceptSelector::new(config.clone());
        let generator = LlmCreativeGenerator::new(llm, config.clone());

        Self {
            pool_builder,
            selector,
            generator,
            db,
            obsidian,
            config,
            cached_pool: RwLock::new(None),
        }
    }

    /// 获取灵感（主入口）
    pub async fn get_inspiration(
        &self,
        inspiration_type: Option<&str>,
        note_path: Option<&str>,
    ) -> Result<InspirationResult, BrainError> {
        let insp_type = inspiration_type.unwrap_or("concept_combo");

        match insp_type {
            "concept_combo" => self.handle_concept_combo().await,
            "reverse_question" => self.handle_reverse_question(note_path).await,
            "counterpoint" => self.handle_counterpoint(note_path).await,
            _ => Err(BrainError::Internal(format!(
                "未知灵感类型: {}。支持: concept_combo, reverse_question, counterpoint",
                insp_type
            ))),
        }
    }

    /// 处理概念组合
    async fn handle_concept_combo(&self) -> Result<InspirationResult, BrainError> {
        // 1. 获取或构建概念池
        let pool = self.get_or_build_pool().await?;

        // 2. 获取最近的配对记录（用于去重）
        let recent_pairs = self.get_recent_pairs()?;

        // 3. 选择两个概念
        let (idx_a, idx_b) = self.selector.select_pair(&pool, &recent_pairs)
            .ok_or_else(|| BrainError::Internal("无法选择概念对，概念池可能太小".to_string()))?;

        let concept_a = &pool.concepts[idx_a];
        let concept_b = &pool.concepts[idx_b];

        // 4. 获取相关笔记内容作为上下文
        let context_a = self.get_concept_context(concept_a).await;
        let context_b = self.get_concept_context(concept_b).await;

        // 5. LLM 生成创意
        let output = self.generator.generate_combo(
            &concept_a.term, &context_a,
            &concept_b.term, &context_b,
        ).await?;

        // 6. 保存到历史
        self.save_history(
            InspirationType::ConceptCombo,
            &json!({"a": concept_a.term, "b": concept_b.term}),
            &serde_json::to_string(&output).unwrap_or_default(),
        )?;

        // 7. 构建结果
        Ok(InspirationResult::ConceptCombo {
            concept_a: ConceptRef {
                term: concept_a.term.clone(),
                source: format!("{:?}", concept_a.source),
                source_path: concept_a.note_paths.first().cloned(),
                obsidian_uri: concept_a.note_paths.first().map(|p| self.make_obsidian_uri(p)),
            },
            concept_b: ConceptRef {
                term: concept_b.term.clone(),
                source: format!("{:?}", concept_b.source),
                source_path: concept_b.note_paths.first().cloned(),
                obsidian_uri: concept_b.note_paths.first().map(|p| self.make_obsidian_uri(p)),
            },
            inspiration: output.inspiration,
            suggestions: output.suggestions,
            experiment_idea: output.experiment_idea,
            generated_at: Utc::now(),
        })
    }

    /// 处理反向提问
    async fn handle_reverse_question(&self, note_path: Option<&str>) -> Result<InspirationResult, BrainError> {
        let obsidian = self.obsidian.as_ref()
            .ok_or_else(|| BrainError::ConfigError("Obsidian API 未启用".to_string()))?;

        // 1. 确定笔记路径
        let path = match note_path {
            Some(p) => p.to_string(),
            None => self.get_recent_note_path().await?,
        };

        // 2. 读取笔记内容
        let content = obsidian.read_file(&path).await?;
        if content.len() < 200 {
            return Err(BrainError::Internal("笔记内容太短（<200字），无法生成有意义的问题".to_string()));
        }

        let title = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未知")
            .to_string();

        // 3. LLM 生成问题
        let output = self.generator.generate_questions(&title, &content).await?;

        // 4. 保存到历史
        self.save_history(
            InspirationType::ReverseQuestion,
            &json!({"note_path": path}),
            &serde_json::to_string(&output).unwrap_or_default(),
        )?;

        // 5. 构建结果
        Ok(InspirationResult::ReverseQuestion {
            note: NoteRef {
                path: path.clone(),
                title: title.clone(),
                obsidian_uri: self.make_obsidian_uri(&path),
            },
            questions: output.questions,
            generated_at: Utc::now(),
        })
    }

    /// 处理对立观点
    async fn handle_counterpoint(&self, note_path: Option<&str>) -> Result<InspirationResult, BrainError> {
        let obsidian = self.obsidian.as_ref()
            .ok_or_else(|| BrainError::ConfigError("Obsidian API 未启用".to_string()))?;

        let path = note_path
            .ok_or_else(|| BrainError::Internal("counterpoint 模式需要指定 note_path".to_string()))?
            .to_string();

        // 1. 读取笔记内容
        let content = obsidian.read_file(&path).await?;
        if content.len() < 300 {
            return Err(BrainError::Internal("笔记内容太短（<300字），无法生成有意义的对立观点".to_string()));
        }

        let title = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未知")
            .to_string();

        // 2. LLM 生成对立观点
        let output = self.generator.generate_counterpoints(&title, &content).await?;

        // 3. 保存到历史
        self.save_history(
            InspirationType::Counterpoint,
            &json!({"note_path": path}),
            &serde_json::to_string(&output).unwrap_or_default(),
        )?;

        // 4. 构建结果
        Ok(InspirationResult::Counterpoint {
            note: NoteRef {
                path: path.clone(),
                title: title.clone(),
                obsidian_uri: self.make_obsidian_uri(&path),
            },
            counterpoints: output.counterpoints,
            overall_assessment: output.overall_assessment,
            related_notes: vec![],
            generated_at: Utc::now(),
        })
    }

    /// 获取或构建概念池（带缓存）
    async fn get_or_build_pool(&self) -> Result<ConceptPool, BrainError> {
        // 检查缓存
        {
            let pool = self.cached_pool.read().unwrap();
            if let Some((ref p, built_at)) = *pool {
                let age = Utc::now() - built_at;
                if age.num_seconds() < self.config.cache_ttl_secs as i64 {
                    return Ok(p.clone());
                }
            }
        }

        // 构建新概念池
        let pool = self.pool_builder.build().await?;

        // 更新缓存
        {
            let mut cached = self.cached_pool.write().unwrap();
            *cached = Some((pool.clone(), Utc::now()));
        }

        Ok(pool)
    }

    /// 获取最近的配对记录
    fn get_recent_pairs(&self) -> Result<Vec<(String, String)>, BrainError> {
        let rows = self.db.get_recent_inspirations("concept_combo", 50)?;
        let mut pairs = Vec::new();

        for (_, _, input_refs, _) in rows {
            if let Ok(refs) = serde_json::from_str::<serde_json::Value>(&input_refs) {
                if let (Some(a), Some(b)) = (refs.get("a").and_then(|v| v.as_str()), refs.get("b").and_then(|v| v.as_str())) {
                    let pair = if a < b {
                        (a.to_string(), b.to_string())
                    } else {
                        (b.to_string(), a.to_string())
                    };
                    pairs.push(pair);
                }
            }
        }

        Ok(pairs)
    }

    /// 获取概念的上下文内容
    async fn get_concept_context(&self, concept: &Concept) -> String {
        if let Some(obsidian) = &self.obsidian {
            if let Some(path) = concept.note_paths.first() {
                if let Ok(content) = obsidian.read_file(path).await {
                    // 截取前 500 字符作为上下文
                    return if content.len() > 500 {
                        content[..500].to_string()
                    } else {
                        content
                    };
                }
            }
        }
        String::new()
    }

    /// 获取最近修改的笔记路径
    async fn get_recent_note_path(&self) -> Result<String, BrainError> {
        let obsidian = self.obsidian.as_ref()
            .ok_or_else(|| BrainError::ConfigError("Obsidian API 未启用".to_string()))?;

        let files = obsidian.list_all_files().await?;
        let md_files: Vec<&String> = files.iter().filter(|f| f.ends_with(".md")).collect();

        // 返回第一个找到的 markdown 文件（后续可以改进为按修改时间排序）
        md_files.first()
            .map(|s| s.to_string())
            .ok_or_else(|| BrainError::Internal("Vault 中没有找到笔记".to_string()))
    }

    /// 生成 Obsidian URI
    fn make_obsidian_uri(&self, path: &str) -> String {
        let vault_name = &self.config.max_concepts.to_string(); // TODO: use actual vault name
        let encoded = urlencoding::encode(path);
        format!("obsidian://open?vault={}&file={}", vault_name, encoded)
    }

    /// 保存灵感记录到历史
    fn save_history(&self, insp_type: InspirationType, input_refs: &serde_json::Value, output: &str) -> Result<(), BrainError> {
        let id = Uuid::new_v4().to_string();
        let refs_str = serde_json::to_string(input_refs).unwrap_or_default();
        self.db.insert_inspiration(&id, &insp_type.to_string(), &refs_str, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::infra::sqlite_store::SqliteStore;
    use crate::infra::llm_client::OllamaProvider;
    use crate::config::LlmConfig;

    fn create_service() -> (TempDir, InspirationService) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(SqliteStore::new(&dir.path().join("test.db")).unwrap());
        let config = InspirationConfig::default();
        let llm_config = LlmConfig::default();
        let llm: Arc<dyn LlmProvider> = Arc::new(OllamaProvider::new(&llm_config).unwrap());
        let service = InspirationService::new(db, None, llm, config);
        (dir, service)
    }

    #[tokio::test]
    async fn test_get_inspiration_unknown_type() {
        let (_dir, service) = create_service();
        let result = service.get_inspiration(Some("unknown"), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_inspiration_requires_obsidian_for_questions() {
        let (_dir, service) = create_service();
        let result = service.get_inspiration(Some("reverse_question"), Some("test.md")).await;
        assert!(result.is_err()); // Obsidian 未配置
    }

    #[tokio::test]
    async fn test_get_inspiration_requires_note_for_counterpoint() {
        let (_dir, service) = create_service();
        let result = service.get_inspiration(Some("counterpoint"), None).await;
        assert!(result.is_err()); // 需要 note_path
    }
}
