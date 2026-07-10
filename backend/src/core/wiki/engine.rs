//! Wiki 引擎核心 — Karpathy LLM Wiki 模式
//!
//! LLM 是知识编译器：读取原始资料 → 综合改写成完整文章 → 合并/新建/级联更新。

use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::llm_client::LlmProvider;
use crate::infra::obsidian_client::ObsidianProvider;
use serde::Serialize;
use serde_json::json;

use super::page_writer::{ensure_wiki_structure, page_exists, read_page, write_page};

/// Wiki 摄入结果
#[derive(Debug, Clone, Serialize)]
pub struct IngestResult {
    pub article_path: String,
    pub action: String, // "created" | "merged" | "updated"
    pub title: String,
    pub topic: String,
    pub cascade_updated: Vec<String>,
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
    pub issues: Vec<LintIssue>,
    pub fixed: usize,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LintIssue {
    pub severity: String, // "error" | "warning" | "info"
    pub category: String,
    pub description: String,
    pub file: Option<String>,
}

/// Wiki 状态
#[derive(Debug, Clone, Serialize)]
pub struct WikiStatus {
    pub total_pages: usize,
    pub topics: Vec<String>,
    pub raw_sources: usize,
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

    /// 摄入一篇原始资料：LLM 读取 → 决定合并或新建 → 编译完整文章 → 更新索引
    pub async fn ingest(
        &self,
        source_path: &str,
        source_type: &str,
        source_url: Option<&str>,
    ) -> Result<IngestResult, BrainError> {
        ensure_wiki_structure(&self.obsidian).await?;

        let content = read_page(&self.obsidian, source_path).await?;
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();

        // 1. 读取现有索引，让 LLM 决定如何处理这篇资料
        let index = read_page(&self.obsidian, "Wiki/index.md")
            .await
            .unwrap_or_else(|_| "# Knowledge Base Index\n".to_string());

        let decide_prompt = format!(
            "你是一个知识库编译器。请阅读以下原始资料和现有 Wiki 索引，决定如何处理。\n\n\
            ## 任务\n\
            1. 确定这篇资料属于什么主题（topic），用英文 kebab-case\n\
            2. 判断应该合并到已有文章还是创建新文章\n\
            3. 如果创建新文章，给出文章标题和文件名\n\n\
            ## 严格按 JSON 格式返回：\n\
            ```json\n\
            {{\n\
              \"topic\": \"主题名（kebab-case）\",\n\
              \"action\": \"create\" 或 \"merge\",\n\
              \"merge_target\": \"要合并的文章路径（如 wiki/llm/llm-wiki.md），action=create 时为空\",\n\
              \"title\": \"文章标题\",\n\
              \"filename\": \"文件名（不含 .md，kebab-case）\"\n\
            }}\n\
            ```\n\n\
            ## 现有索引：\n{}\n\n\
            ## 原始资料（前 3000 字）：\n{}",
            index,
            content.chars().take(3000).collect::<String>()
        );

        let decide_response = self.llm.generate(&decide_prompt).await?;
        let decision = parse_llm_json(&decide_response)?;

        let topic = decision["topic"].as_str().unwrap_or("general").to_string();
        let action = decision["action"].as_str().unwrap_or("create").to_string();
        let title = decision["title"].as_str().unwrap_or("Untitled").to_string();
        let filename = decision["filename"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("untitled")
            .to_string();
        let merge_target = decision["merge_target"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // For merge: use merge_target as the article path
        // For create: compute path from topic + filename
        let article_path = if action == "merge" {
            if let Some(ref target) = merge_target {
                target.clone()
            } else {
                format!("Wiki/{}/{}.md", topic, filename)
            }
        } else {
            format!("Wiki/{}/{}.md", topic, filename)
        };

        // 2. 编译文章
        let (article_content, action_label) = if action == "merge" && merge_target.is_some() {
            // 合并模式：读取已有文章，LLM 合并新内容
            let target = merge_target.as_ref().unwrap();
            let existing = read_page(&self.obsidian, target).await.unwrap_or_default();
            let merge_prompt = format!(
                "你是一个知识库编译器。请将新资料合并到已有文章中。\n\n\
                ## 要求\n\
                1. 保留已有文章的结构和内容\n\
                2. 将新资料的信息整合到相关章节\n\
                3. 如果新资料与已有内容冲突，标注冲突\n\
                4. 更新 Sources 和 Raw 字段\n\
                5. 输出完整的合并后文章（Markdown 格式，包含 frontmatter）\n\n\
                ## 已有文章：\n{}\n\n\
                ## 新资料（前 5000 字）：\n{}",
                existing,
                content.chars().take(5000).collect::<String>()
            );
            let merged = self.llm.generate(&merge_prompt).await?;
            (strip_code_block(&merged), "merged")
        } else {
            // 新建模式：LLM 从资料编译完整文章
            let source_desc = format!("{}, {}", source_type, now);
            let source_name = source_path.rsplit('/').next().unwrap_or(source_path);
            let content_preview: String = content.chars().take(8000).collect();
            let compile_prompt = format!(
                "你是一个知识库编译器。请阅读以下资料，编写一篇完整的知识文章。\n\n\
                ## 要求\n\
                1. 不要逐字复制原文，要综合改写和重新组织\n\
                2. 文章应该是一篇完整的知识论述，不是简单的摘要\n\
                3. 包含 Overview 概述段落和多个正文章节\n\
                4. 使用 Markdown 格式\n\n\
                ## 文章格式：\n\
                ```markdown\n\
                # {title}\n\n\
                > Sources: {source_desc}\n\
                > Raw: [{source_name}](../../{source_path})\n\n\
                ## Overview\n\n\
                {{概述段落}}\n\n\
                ## {{正文章节}}\n\n\
                {{综合改写的内容}}\n\
                ```\n\n\
                ## 资料（前 8000 字）：\n{content_preview}",
            );
            let compiled = self.llm.generate(&compile_prompt).await?;
            (strip_code_block(&compiled), "created")
        };

        write_page(&self.obsidian, &article_path, &article_content).await?;

        // 3. 级联更新：检查同主题其他文章是否需要更新
        let cascade_updated = self
            .cascade_updates(&topic, &article_path, &content)
            .await?;

        // 4. 更新索引
        self.update_index(&topic, &title, &article_path, &now)
            .await?;

        // 5. 追加日志
        let log_entry = if cascade_updated.is_empty() {
            format!("\n## [{}] ingest | {}\n", now, title)
        } else {
            let updated_lines: Vec<String> = cascade_updated
                .iter()
                .map(|p| format!("- Updated: {}", p))
                .collect();
            format!(
                "\n## [{}] ingest | {}\n{}\n",
                now,
                title,
                updated_lines.join("\n")
            )
        };
        let log_path = "Wiki/log.md";
        let existing_log = read_page(&self.obsidian, log_path)
            .await
            .unwrap_or_default();
        write_page(
            &self.obsidian,
            log_path,
            &format!("{}{}", existing_log, log_entry),
        )
        .await?;

        tracing::info!(
            source = source_path,
            action = action_label,
            article = article_path,
            cascade = cascade_updated.len(),
            "Wiki ingest 完成"
        );

        Ok(IngestResult {
            article_path,
            action: action_label.to_string(),
            title,
            topic,
            cascade_updated,
        })
    }

    /// 级联更新：检查同主题其他文章是否需要因新资料而更新
    async fn cascade_updates(
        &self,
        topic: &str,
        new_article_path: &str,
        source_content: &str,
    ) -> Result<Vec<String>, BrainError> {
        // 列出同主题的其他文章
        let client = crate::infra::obsidian_client::get_client(&self.obsidian)?;
        let all_files = client.list_all_files().await?;
        let same_topic: Vec<String> = all_files
            .iter()
            .filter(|f| {
                f.starts_with(&format!("Wiki/{}/", topic))
                    && f.ends_with(".md")
                    && *f != new_article_path
                    && !f.ends_with("index.md")
                    && !f.ends_with("log.md")
            })
            .cloned()
            .collect();

        if same_topic.is_empty() {
            return Ok(vec![]);
        }

        // 读取同主题文章的标题，让 LLM 判断是否需要级联更新
        let mut titles = Vec::new();
        for path in &same_topic {
            if let Ok(content) = read_page(&self.obsidian, path).await {
                let title = content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches("# ")
                    .to_string();
                titles.push(format!("- {} ({})", title, path));
            }
        }

        let cascade_prompt = format!(
            "你是一个知识库维护助手。刚摄入了一篇新资料到 Wiki，请判断同主题的其他文章是否需要更新。\n\n\
            ## 新资料摘要（前 1000 字）：\n{}\n\n\
            ## 同主题已有文章：\n{}\n\n\
            ## 要求\n\
            判断哪些文章的内容可能受到新资料影响，需要更新。\n\
            严格按 JSON 格式返回：{{\"need_update\": [\"文章路径1\", \"文章路径2\"]}}\n\
            如果都不需要更新，返回空数组。",
            source_content.chars().take(1000).collect::<String>(),
            titles.join("\n")
        );

        let response = self.llm.generate(&cascade_prompt).await?;
        let parsed = parse_llm_json(&response).unwrap_or(json!({}));
        let to_update: Vec<String> = parsed["need_update"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // 对每篇需要更新的文章，让 LLM 执行更新
        let mut updated = Vec::new();
        for path in &to_update {
            if let Ok(existing) = read_page(&self.obsidian, path).await {
                let update_prompt = format!(
                    "你是一个知识库维护助手。请根据新资料更新这篇已有文章的相关内容。\n\n\
                    ## 要求\n\
                    1. 只更新与新资料相关的部分\n\
                    2. 保留不相关的内容\n\
                    3. 输出完整的更新后文章\n\n\
                    ## 已有文章：\n{}\n\n\
                    ## 新资料（前 3000 字）：\n{}",
                    existing,
                    source_content.chars().take(3000).collect::<String>()
                );
                if let Ok(updated_content) = self.llm.generate(&update_prompt).await {
                    write_page(&self.obsidian, path, &updated_content).await?;
                    updated.push(path.clone());
                }
            }
        }

        Ok(updated)
    }

    /// 基于 Wiki 回答问题
    pub async fn query(
        &self,
        question: &str,
        save_answer: bool,
    ) -> Result<QueryResult, BrainError> {
        let index = read_page(&self.obsidian, "Wiki/index.md")
            .await
            .unwrap_or_else(|_| "# Knowledge Base Index\n".to_string());

        // LLM 选择相关文章
        let select_prompt = format!(
            "你是一个知识库查询助手。以下是 Wiki 的索引。请选出与问题最相关的文章（最多 5 个）。\n\
            严格按 JSON 格式返回：{{\"pages\": [\"wiki/topic/article.md\"]}}\n\n\
            ## 问题：{}\n\n\
            ## 索引：\n{}",
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

        // 读取相关文章
        let mut page_contents = Vec::new();
        for path in &selected_pages {
            if let Ok(content) = read_page(&self.obsidian, path).await {
                page_contents.push((path.clone(), content));
            }
        }

        // LLM 综合回答
        let context = page_contents
            .iter()
            .map(|(path, content)| format!("---\n来源：{}\n{}\n", path, content))
            .collect::<Vec<_>>()
            .join("\n");

        let answer_prompt = format!(
            "你是一个知识库问答助手。请基于以下 Wiki 文章回答问题。\n\n\
            要求：\n\
            1. 回答必须基于提供的 Wiki 内容，不要编造\n\
            2. 引用相关文章\n\
            3. 如果信息不足，说明还需要哪些资料\n\n\
            ## 问题：{}\n\n\
            ## Wiki 内容：\n{}",
            question, context
        );

        let answer = self.llm.generate(&answer_prompt).await?;
        let cited_pages: Vec<String> = page_contents.iter().map(|(p, _)| p.clone()).collect();

        // 可选归档
        let saved_to = if save_answer {
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            let filename = format!(
                "{}-archived-{}",
                now,
                question
                    .chars()
                    .take(20)
                    .collect::<String>()
                    .replace(' ', "-")
            );
            let path = format!("Wiki/archived/{}.md", filename);

            let archive_content = format!(
                "# {}\n\n> Archived: {}\n\n## Overview\n\n{}\n",
                question, now, answer
            );
            write_page(&self.obsidian, &path, &archive_content).await?;

            // 追加日志
            let log_path = "Wiki/log.md";
            let existing_log = read_page(&self.obsidian, log_path)
                .await
                .unwrap_or_default();
            let log_entry = format!("\n## [{}] query | Archived: {}\n", now, question);
            write_page(
                &self.obsidian,
                log_path,
                &format!("{}{}", existing_log, log_entry),
            )
            .await?;

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
    pub async fn lint(&self, _auto_fix: bool) -> Result<LintResult, BrainError> {
        let client = crate::infra::obsidian_client::get_client(&self.obsidian)?;
        let all_files = client.list_all_files().await?;
        let wiki_files: Vec<String> = all_files
            .iter()
            .filter(|f| f.starts_with("Wiki/") && f.ends_with(".md") && !f.ends_with(".gitkeep"))
            .cloned()
            .collect();

        let mut issues = Vec::new();

        // 检查索引一致性
        let index_content = read_page(&self.obsidian, "Wiki/index.md")
            .await
            .unwrap_or_default();
        for f in &wiki_files {
            if f == "Wiki/index.md" || f == "Wiki/log.md" || f == "Wiki/schema.md" {
                continue;
            }
            if !index_content.contains(f) && !index_content.contains(&f.replace("Wiki/", "")) {
                issues.push(LintIssue {
                    severity: "warning".to_string(),
                    category: "index".to_string(),
                    description: format!("文章不在索引中: {}", f),
                    file: Some(f.clone()),
                });
            }
        }

        // 检查孤岛页
        let mut page_contents = Vec::new();
        for path in &wiki_files {
            if let Ok(content) = client.read_file(path).await {
                page_contents.push((path.clone(), content));
            }
        }
        let graph = super::link_graph::LinkGraph::build(&page_contents);
        let orphans = graph.find_orphans();
        for o in &orphans {
            if o == "Wiki/index.md" || o == "Wiki/log.md" {
                continue;
            }
            issues.push(LintIssue {
                severity: "info".to_string(),
                category: "orphan".to_string(),
                description: format!("孤岛页（无入链）: {}", o),
                file: Some(o.clone()),
            });
        }

        let hubs = graph.find_hubs(5);

        // LLM 生成建议
        let suggestions_prompt = format!(
            "你是一个知识库维护助手。以下是检查结果，请给出 2-3 条改进建议。\n\
            严格按 JSON 格式：{{\"suggestions\": [\"建议1\", \"建议2\"]}}\n\n\
            总文章数：{}\n问题数：{}\n孤岛页：{} 个\n知识枢纽：{} 个",
            wiki_files.len(),
            issues.len(),
            orphans.len(),
            hubs.len()
        );

        let suggestions_response = self
            .llm
            .generate(&suggestions_prompt)
            .await
            .unwrap_or_default();
        let suggestions_parsed = parse_llm_json(&suggestions_response).ok();
        let suggestions: Vec<String> = suggestions_parsed
            .and_then(|p| {
                p["suggestions"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
            })
            .unwrap_or_default();

        // 追加日志
        let log_path = "Wiki/log.md";
        let existing_log = read_page(&self.obsidian, log_path)
            .await
            .unwrap_or_default();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let log_entry = format!("\n## [{}] lint | {} issues found\n", now, issues.len());
        write_page(
            &self.obsidian,
            log_path,
            &format!("{}{}", existing_log, log_entry),
        )
        .await?;

        Ok(LintResult {
            total_pages: wiki_files.len(),
            issues,
            fixed: 0,
            suggestions,
        })
    }

    /// 获取 Wiki 状态
    pub async fn status(&self) -> Result<WikiStatus, BrainError> {
        let client = crate::infra::obsidian_client::get_client(&self.obsidian)?;
        let all_files = client.list_all_files().await?;

        let wiki_files: Vec<String> = all_files
            .iter()
            .filter(|f| f.starts_with("Wiki/") && f.ends_with(".md") && !f.ends_with(".gitkeep"))
            .cloned()
            .collect();

        let raw_files: Vec<String> = all_files
            .iter()
            .filter(|f| f.starts_with("Raw/") && f.ends_with(".md"))
            .cloned()
            .collect();

        // 提取主题目录
        let mut topics = std::collections::HashSet::new();
        for f in &wiki_files {
            if let Some(rest) = f.strip_prefix("Wiki/") {
                if let Some(slash_pos) = rest.find('/') {
                    topics.insert(rest[..slash_pos].to_string());
                }
            }
        }

        let initialized = page_exists(&self.obsidian, "Wiki/index.md").await;

        Ok(WikiStatus {
            total_pages: wiki_files.len(),
            topics: topics.into_iter().collect(),
            raw_sources: raw_files.len(),
            initialized,
        })
    }

    /// 更新索引（表格格式）
    async fn update_index(
        &self,
        topic: &str,
        title: &str,
        article_path: &str,
        date: &str,
    ) -> Result<(), BrainError> {
        let index_path = "Wiki/index.md";
        let mut index = read_page(&self.obsidian, index_path)
            .await
            .unwrap_or_default();

        // 检查是否已有这个主题的 section
        let section_header = format!("## {}", topic);
        let article_link = article_path.strip_prefix("Wiki/").unwrap_or(article_path);

        if !index.contains(&section_header) {
            // 新主题
            let new_section = format!(
                "\n\n## {}\n\n| Article | Summary | Updated |\n|---------|---------|---------|\n| [{}]({}) | {} | {} |\n",
                topic, title, article_link, title, date
            );
            index.push_str(&new_section);
        } else {
            // 检查是否已有这个文章的条目
            if index.contains(article_link) {
                // 更新日期
                // 简单策略：替换该行的 Updated 列
                let old_line_pattern = format!("[{}]({})", title, article_link);
                if let Some(pos) = index.find(&old_line_pattern) {
                    // 找到这行的末尾，更新日期
                    let line_start = index[..pos].rfind('|').map(|p| p + 1).unwrap_or(pos);
                    let line_end = index[pos..]
                        .find('\n')
                        .map(|e| pos + e)
                        .unwrap_or(index.len());
                    let new_line =
                        format!(" [{}]({}) | {} | {} ", title, article_link, title, date);
                    index.replace_range(line_start..line_end, &new_line);
                }
            } else {
                // 在该主题的表格中添加新行
                let entry = format!("| [{}]({}) | {} | {} |\n", title, article_link, title, date);
                // 找到该 section 的下一个 section 或文件末尾
                if let Some(section_start) = index.find(&section_header) {
                    let after_section = &index[section_start..];
                    let next_section = after_section[1..].find("\n## ");
                    let insert_pos = match next_section {
                        Some(offset) => section_start + 1 + offset,
                        None => index.len(),
                    };
                    index.insert_str(insert_pos, &entry);
                }
            }
        }

        write_page(&self.obsidian, index_path, &index).await
    }
}

/// 解析 LLM 返回的 JSON
fn parse_llm_json(text: &str) -> Result<serde_json::Value, BrainError> {
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            return serde_json::from_str(rest[..end].trim())
                .map_err(|e| BrainError::Internal(format!("LLM JSON 解析失败: {e}")));
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return serde_json::from_str(&text[start..=end])
                .map_err(|e| BrainError::Internal(format!("LLM JSON 解析失败: {e}")));
        }
    }
    Err(BrainError::Internal("LLM 未返回有效 JSON".to_string()))
}

/// 去除 LLM 返回内容中的 markdown 代码块标记
fn strip_code_block(text: &str) -> String {
    let mut result = text.trim().to_string();
    // 去除开头的 ```markdown 或 ```
    if result.starts_with("```") {
        let first_newline = result.find('\n').unwrap_or(result.len());
        result = result[first_newline..].trim_start().to_string();
    }
    // 去除结尾的 ```
    if result.ends_with("```") {
        result = result[..result.len() - 3].trim_end().to_string();
    }
    result
}
