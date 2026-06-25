//! Wiki 引擎核心
//!
//! 编排 Ingest / Query / Lint 三大操作。

use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::llm_client::LlmProvider;
use crate::infra::obsidian_client::ObsidianProvider;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::index_manager::{append_log, list_pages, update_index};
use super::link_graph::LinkGraph;
use super::page_writer::{
    ensure_wiki_structure, page_exists, page_path, read_page, to_filename, write_page,
    PageType,
};

/// Wiki 摄入结果
#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub summary_page: String,
    pub created_pages: Vec<String>,
    pub updated_pages: Vec<String>,
    pub entities: Vec<String>,
    pub concepts: Vec<String>,
}

/// Wiki 查询结果
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub answer: String,
    pub cited_pages: Vec<String>,
    pub saved_to: Option<String>,
}

/// Wiki Lint 结果
#[derive(Debug, Clone, Serialize)]
pub struct LintResult {
    pub total_pages: usize,
    pub orphans: Vec<String>,
    pub missing_pages: Vec<String>,
    pub hubs: Vec<(String, usize)>,
    pub fixed: usize,
    pub suggestions: Vec<String>,
}

/// Wiki 状态
#[derive(Debug, Clone, Serialize)]
pub struct WikiStatus {
    pub total_pages: usize,
    pub entities: usize,
    pub concepts: usize,
    pub sources: usize,
    pub synthesis: usize,
    pub initialized: bool,
}

/// LLM Wiki 引擎
pub struct WikiEngine {
    obsidian: ObsidianProvider,
    llm: Arc<dyn LlmProvider>,
}

impl WikiEngine {
    pub fn new(obsidian: ObsidianProvider, llm: Arc<dyn LlmProvider>) -> Self {
        Self { obsidian, llm }
    }

    /// 摄入一篇原始资料
    pub async fn ingest(
        &self,
        source_path: &str,
        source_type: &str,
        source_url: Option<&str>,
    ) -> Result<IngestResult, BrainError> {
        // 确保 Wiki 结构存在
        ensure_wiki_structure(&self.obsidian).await?;

        // 1. 读取原始资料
        let content = read_page(&self.obsidian, source_path).await?;

        // 2. LLM 生成摘要 + 提取实体和概念
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let source_name = source_path
            .rsplit('/')
            .next()
            .unwrap_or(source_path)
            .trim_end_matches(".md");

        let extract_prompt = format!(
            "你是一个知识库维护助手。请阅读以下资料，生成一个结构化的摘要。\n\n\
            要求：\n\
            1. 生成 200-500 字的中文摘要，提炼核心观点和关键信息\n\
            2. 提取文中提到的实体（人物、项目、工具、组织等）\n\
            3. 提取核心概念（技术主题、理论、方法等）\n\n\
            请严格按以下 JSON 格式返回（不要包含其他文字）：\n\
            ```json\n\
            {{\n\
              \"summary\": \"摘要内容\",\n\
              \"entities\": [\"实体1\", \"实体2\"],\n\
              \"concepts\": [\"概念1\", \"概念2\"]\n\
            }}\n\
            ```\n\n\
            资料内容：\n{}",
            content
        );

        let llm_response = self.llm.generate(&extract_prompt).await?;
        let parsed = parse_llm_json(&llm_response)?;

        let summary = parsed["summary"]
            .as_str()
            .unwrap_or("无法生成摘要")
            .to_string();
        let entities: Vec<String> = parsed["entities"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let concepts: Vec<String> = parsed["concepts"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // 3. 创建源摘要页
        let source_page_name = format!("{}-{}", now, source_name);
        let source_page_path = page_path(&PageType::Source, &source_page_name);
        let source_content = format_source_page(
            &summary,
            source_path,
            source_type,
            source_url,
            &now,
            &entities,
            &concepts,
        );
        write_page(&self.obsidian, &source_page_path, &source_content).await?;

        let mut created_pages = vec![source_page_path.clone()];
        let mut updated_pages = Vec::new();

        // 4. 创建/更新实体页
        for entity in &entities {
            let path = page_path(&PageType::Entity, entity);
            if page_exists(&self.obsidian, &path).await {
                // 更新：追加源引用
                let existing = read_page(&self.obsidian, &path).await.unwrap_or_default();
                let updated = add_source_ref(&existing, &source_page_name, &now);
                write_page(&self.obsidian, &path, &updated).await?;
                updated_pages.push(path);
            } else {
                // 创建新实体页
                let content = format_entity_page(entity, &source_page_name, &now, &concepts);
                write_page(&self.obsidian, &path, &content).await?;
                created_pages.push(path);
            }
        }

        // 5. 创建/更新概念页
        for concept in &concepts {
            let path = page_path(&PageType::Concept, concept);
            if page_exists(&self.obsidian, &path).await {
                let existing = read_page(&self.obsidian, &path).await.unwrap_or_default();
                let updated = add_source_ref(&existing, &source_page_name, &now);
                write_page(&self.obsidian, &path, &updated).await?;
                updated_pages.push(path);
            } else {
                let content = format_concept_page(concept, &source_page_name, &now, &entities);
                write_page(&self.obsidian, &path, &content).await?;
                created_pages.push(path);
            }
        }

        // 6. 更新索引
        let all_entities = list_pages(&self.obsidian, "Wiki/entities").await.unwrap_or_default();
        let all_concepts = list_pages(&self.obsidian, "Wiki/concepts").await.unwrap_or_default();
        let all_sources = list_pages(&self.obsidian, "Wiki/sources").await.unwrap_or_default();
        let all_synthesis = list_pages(&self.obsidian, "Wiki/synthesis").await.unwrap_or_default();
        update_index(
            &self.obsidian,
            &all_entities,
            &all_concepts,
            &all_sources,
            &all_synthesis,
        )
        .await?;
        updated_pages.push("Wiki/index.md".to_string());

        // 7. 追加日志
        let all_affected: Vec<String> = created_pages
            .iter()
            .chain(updated_pages.iter())
            .cloned()
            .collect();
        append_log(
            &self.obsidian,
            "ingest",
            source_path,
            &all_affected,
        )
        .await?;
        updated_pages.push("Wiki/log.md".to_string());

        tracing::info!(
            source = source_path,
            created = created_pages.len(),
            updated = updated_pages.len(),
            entities = entities.len(),
            concepts = concepts.len(),
            "Wiki ingest 完成"
        );

        Ok(IngestResult {
            summary_page: source_page_path,
            created_pages,
            updated_pages,
            entities,
            concepts,
        })
    }

    /// 基于 Wiki 回答问题
    pub async fn query(
        &self,
        question: &str,
        save_answer: bool,
    ) -> Result<QueryResult, BrainError> {
        // 1. 读取索引
        let index = read_page(&self.obsidian, "Wiki/index.md")
            .await
            .unwrap_or_else(|_| "# Wiki 索引\n\n（空）".to_string());

        // 2. LLM 选择相关页面
        let select_prompt = format!(
            "你是一个知识库查询助手。以下是 Wiki 的索引文件，列出了所有可用的知识页面。\n\n\
            用户问题：{}\n\n\
            请从索引中选出与问题最相关的页面（最多 5 个），返回页面名称列表（不含路径和 .md）。\n\
            严格按 JSON 格式返回：{{\"pages\": [\"页面1\", \"页面2\"]}}\n\n\
            索引内容：\n{}",
            question, index
        );

        let select_response = self.llm.generate(&select_prompt).await?;
        let select_parsed = parse_llm_json(&select_response)?;
        let selected_pages: Vec<String> = select_parsed["pages"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // 3. 读取相关页面内容
        let mut page_contents = Vec::new();
        for page_name in &selected_pages {
            // 在各子目录中查找
            for dir in &["Wiki/entities", "Wiki/concepts", "Wiki/sources", "Wiki/synthesis"] {
                let path = format!("{}/{}.md", dir, to_filename(page_name));
                if let Ok(content) = read_page(&self.obsidian, &path).await {
                    page_contents.push((path, content));
                    break;
                }
            }
        }

        // 4. LLM 综合回答
        let context = page_contents
            .iter()
            .map(|(path, content)| format!("---\n来源：{}\n{}\n", path, content))
            .collect::<Vec<_>>()
            .join("\n");

        let answer_prompt = format!(
            "你是一个知识库问答助手。请基于以下 Wiki 页面内容回答用户的问题。\n\n\
            要求：\n\
            1. 回答必须基于提供的 Wiki 内容，不要编造\n\
            2. 引用相关页面时使用 [[页面名]] 格式\n\
            3. 如果信息不足，说明还需要哪些方面的资料\n\n\
            用户问题：{}\n\n\
            Wiki 内容：\n{}",
            question, context
        );

        let answer = self.llm.generate(&answer_prompt).await?;

        let cited_pages: Vec<String> = page_contents.iter().map(|(p, _)| p.clone()).collect();

        // 5. 可选归档
        let saved_to = if save_answer {
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            let name = format!("{}-query-{}", now, to_filename(&question.chars().take(20).collect::<String>()));
            let path = page_path(&PageType::Synthesis, &name);
            let content = format!(
                "---\ntype: synthesis\ncreated: {}\nquery: {}\n---\n\n# {}\n\n{}",
                now, question, question, answer
            );
            write_page(&self.obsidian, &path, &content).await?;
            append_log(&self.obsidian, "query", question, &cited_pages).await?;
            Some(path)
        } else {
            None
        };

        Ok(QueryResult {
            answer,
            cited_pages,
            saved_to,
        })
    }

    /// Wiki 健康检查
    pub async fn lint(&self, auto_fix: bool) -> Result<LintResult, BrainError> {
        // 1. 列出所有 Wiki 页面
        let client = crate::infra::obsidian_client::get_client(&self.obsidian)?;
        let all_files = client.list_all_files().await?;
        let wiki_files: Vec<String> = all_files
            .into_iter()
            .filter(|f| f.starts_with("Wiki/") && f.ends_with(".md") && !f.ends_with(".gitkeep"))
            .collect();

        // 2. 读取所有页面内容
        let mut page_contents = Vec::new();
        for path in &wiki_files {
            if let Ok(content) = client.read_file(path).await {
                page_contents.push((path.clone(), content));
            }
        }

        // 3. 构建链接图谱
        let graph = LinkGraph::build(&page_contents);

        let orphans = graph.find_orphans();
        let missing_pages = graph.find_missing_pages();
        let hubs = graph.find_hubs(5);

        // 4. 自动修复
        let mut fixed = 0;
        if auto_fix {
            // 为缺失页面创建存根
            for missing in &missing_pages {
                let name = missing.rsplit('/').next().unwrap_or(missing).trim_end_matches(".md");
                // 判断是实体还是概念（简化：都放 concepts）
                let path = page_path(&PageType::Concept, name);
                if !page_exists(&self.obsidian, &path).await {
                    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
                    let content = format!(
                        "---\ntype: concept\ncreated: {}\nupdated: {}\n---\n\n# {}\n\n（待补充）\n",
                        now, now, name
                    );
                    write_page(&self.obsidian, &path, &content).await?;
                    fixed += 1;
                }
            }

            // 为孤岛页添加引用（在 index 中列出即可发现）
            // 更复杂的引用添加留给 LLM 后续处理
        }

        // 5. LLM 生成建议
        let suggestions_prompt = format!(
            "你是一个知识库维护助手。以下是 Wiki 的健康检查结果，请给出 2-3 条改进建议。\n\n\
            总页面数：{}\n孤岛页：{:?}\n缺失页面：{:?}\n枢纽页：{:?}\n\n\
            请返回建议列表，每条一句话。严格按 JSON 格式：{{\"suggestions\": [\"建议1\", \"建议2\"]}}",
            wiki_files.len(),
            orphans,
            missing_pages,
            hubs.iter().map(|(p, c)| format!("{}({})", p, c)).collect::<Vec<_>>()
        );

        let suggestions_response = self.llm.generate(&suggestions_prompt).await.unwrap_or_default();
        let suggestions_parsed = parse_llm_json(&suggestions_response).ok();
        let suggestions: Vec<String> = suggestions_parsed
            .and_then(|p| p["suggestions"].as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }))
            .unwrap_or_default();

        // 6. 追加日志
        append_log(
            &self.obsidian,
            "lint",
            &format!("检查 {} 页，修复 {} 处", wiki_files.len(), fixed),
            &[],
        )
        .await?;

        Ok(LintResult {
            total_pages: wiki_files.len(),
            orphans,
            missing_pages,
            hubs,
            fixed,
            suggestions,
        })
    }

    /// 获取 Wiki 状态
    pub async fn status(&self) -> Result<WikiStatus, BrainError> {
        let entities = list_pages(&self.obsidian, "Wiki/entities").await.unwrap_or_default();
        let concepts = list_pages(&self.obsidian, "Wiki/concepts").await.unwrap_or_default();
        let sources = list_pages(&self.obsidian, "Wiki/sources").await.unwrap_or_default();
        let synthesis = list_pages(&self.obsidian, "Wiki/synthesis").await.unwrap_or_default();

        let initialized = page_exists(&self.obsidian, "Wiki/index.md").await;

        Ok(WikiStatus {
            total_pages: entities.len() + concepts.len() + sources.len() + synthesis.len(),
            entities: entities.len(),
            concepts: concepts.len(),
            sources: sources.len(),
            synthesis: synthesis.len(),
            initialized,
        })
    }
}

// ── 辅助函数 ──

/// 解析 LLM 返回的 JSON（容错：提取 ```json 块或裸 JSON）
fn parse_llm_json(text: &str) -> Result<serde_json::Value, BrainError> {
    // 尝试提取 ```json ... ``` 块
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            let json_str = rest[..end].trim();
            return serde_json::from_str(json_str)
                .map_err(|e| BrainError::Internal(format!("LLM JSON 解析失败: {e}")));
        }
    }

    // 尝试提取第一个 { ... } 块
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            return serde_json::from_str(json_str)
                .map_err(|e| BrainError::Internal(format!("LLM JSON 解析失败: {e}")));
        }
    }

    Err(BrainError::Internal("LLM 未返回有效 JSON".to_string()))
}

/// 格式化源摘要页
fn format_source_page(
    summary: &str,
    source_path: &str,
    source_type: &str,
    source_url: Option<&str>,
    date: &str,
    entities: &[String],
    concepts: &[String],
) -> String {
    let entity_links: Vec<String> = entities.iter().map(|e| format!("- [[{}]]", e)).collect();
    let concept_links: Vec<String> = concepts.iter().map(|c| format!("- [[{}]]", c)).collect();
    let url_line = source_url.map(|u| format!("source_url: \"{}\"\n", u)).unwrap_or_default();

    format!(
        "---\ntype: source\nsource_path: \"{}\"\nsource_type: \"{}\"\n{}\ningested: \"{}\"\nentities: [{}]\nconcepts: [{}]\n---\n\n# 摘要：{}\n\n## 核心摘要\n\n{}\n\n## 关键实体\n\n{}\n\n## 关键概念\n\n{}",
        source_path,
        source_type,
        url_line,
        date,
        entities.iter().map(|e| format!("\"{}\"", e)).collect::<Vec<_>>().join(", "),
        concepts.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>().join(", "),
        source_path.rsplit('/').next().unwrap_or(source_path),
        summary,
        entity_links.join("\n"),
        concept_links.join("\n"),
    )
}

/// 格式化实体页
fn format_entity_page(name: &str, source: &str, date: &str, concepts: &[String]) -> String {
    let concept_links: Vec<String> = concepts.iter().map(|c| format!("- [[{}]]", c)).collect();
    format!(
        "---\ntype: entity\nname: \"{}\"\nsources: [\"{}\"]\ncreated: \"{}\"\nupdated: \"{}\"\n---\n\n# {}\n\n（待补充详细内容）\n\n## 相关概念\n\n{}",
        name, source, date, date, name,
        concept_links.join("\n")
    )
}

/// 格式化概念页
fn format_concept_page(name: &str, source: &str, date: &str, entities: &[String]) -> String {
    let entity_links: Vec<String> = entities.iter().map(|e| format!("- [[{}]]", e)).collect();
    format!(
        "---\ntype: concept\nsources: [\"{}\"]\ncreated: \"{}\"\nupdated: \"{}\"\n---\n\n# {}\n\n（待补充详细内容）\n\n## 相关实体\n\n{}",
        source, date, date, name,
        entity_links.join("\n")
    )
}

/// 在已有页面中追加源引用
fn add_source_ref(existing: &str, source_name: &str, date: &str) -> String {
    // 简单策略：在文件末尾追加来源引用
    if existing.contains(&format!("[[{}]]", source_name)) {
        // 已引用，只更新 updated 日期
        return existing.replace(
            "updated: \"",
            &format!("updated: \"{}", date),
        ).replace(
            &format!("updated: \"{}\"", date.chars().take(10).collect::<String>()),
            &format!("updated: \"{}\"", date),
        );
    }

    // 追加引用
    format!(
        "{}\n\n## 新增来源\n\n- [[{}]] ({})\n",
        existing.trim_end(),
        source_name,
        date
    )
}
