//! 知识探索工具处理器
//!
//! 基于 Wiki 的已有知识，发现知识缺口和未探索的连接。

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::llm_client::LlmProvider;
use crate::infra::obsidian_client::get_client;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

/// 分析 Wiki 知识缺口
pub struct DiscoverGapsHandler;

#[async_trait]
impl ToolHandler for DiscoverGapsHandler {
    fn name(&self) -> &str {
        "discover_gaps"
    }
    fn description(&self) -> &str {
        "分析 Wiki 知识缺口：缺失连接、薄弱主题"
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn module(&self) -> &str {
        "wiki"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let obsidian = get_client(&ctx.obsidian)?;
        let llm: Arc<dyn LlmProvider> = ctx.inspiration_service.get_llm();

        // 读取 Wiki index
        let index = obsidian
            .read_file("Wiki/index.md")
            .await
            .unwrap_or_else(|_| "# Wiki 索引\n\n（空）".to_string());

        // 读取所有概念页面标题
        let all_files = obsidian.list_all_files().await?;
        let concept_pages: Vec<String> = all_files
            .iter()
            .filter(|f| f.starts_with("Wiki/concepts/") && f.ends_with(".md"))
            .map(|f| {
                f.rsplit('/')
                    .next()
                    .unwrap_or(f)
                    .trim_end_matches(".md")
                    .to_string()
            })
            .collect();

        // LLM 分析缺口
        let prompt = format!(
            "你是一个知识库分析助手。以下是 Wiki 的索引和所有概念列表。\n\n\
            请分析：\n\
            1. 哪些概念之间应该有关联但目前没有交叉引用？\n\
            2. 哪些主题的源摘要很少（知识薄弱区）？\n\
            3. 哪些交叉领域值得探索？\n\n\
            返回 3-5 条缺口分析，每条包含：两个概念、为什么应该关联、建议的探索方向。\n\
            严格按 JSON 格式返回：\n\
            ```json\n\
            {{\n\
              \"gaps\": [\n\
                {{\n\
                  \"concept_a\": \"概念A\",\n\
                  \"concept_b\": \"概念B\",\n\
                  \"reason\": \"为什么应该关联\",\n\
                  \"direction\": \"建议探索方向\"\n\
                }}\n\
              ]\n\
            }}\n\
            ```\n\n\
            概念列表：{:?}\n\n\
            索引：\n{}",
            concept_pages, index
        );

        let response = llm.generate(&prompt).await?;
        let parsed = parse_llm_json(&response)?;

        Ok(json!({
            "gaps": parsed.get("gaps").cloned().unwrap_or(json!([])),
            "total_concepts": concept_pages.len(),
        }))
    }
}

/// 生成研究问题
pub struct GenerateQuestionsHandler;

#[async_trait]
impl ToolHandler for GenerateQuestionsHandler {
    fn name(&self) -> &str {
        "generate_questions"
    }
    fn description(&self) -> &str {
        "基于 Wiki 内容生成研究问题"
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn module(&self) -> &str {
        "wiki"
    }

    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let obsidian = get_client(&ctx.obsidian)?;
        let llm: Arc<dyn LlmProvider> = ctx.inspiration_service.get_llm();

        let index = obsidian
            .read_file("Wiki/index.md")
            .await
            .unwrap_or_else(|_| "# Wiki 索引\n\n（空）".to_string());

        let prompt = format!(
            "你是一个研究引导助手。基于以下 Wiki 索引，生成 5 个值得深入研究的开放性问题。\n\n\
            要求：\n\
            1. 问题应该跨越多个概念，促使交叉思考\n\
            2. 每个问题附 1-2 个相关 Wiki 页面引用\n\
            3. 问题应该是当前 Wiki 还没有明确答案的\n\n\
            严格按 JSON 格式返回：\n\
            ```json\n\
            {{\n\
              \"questions\": [\n\
                {{\n\
                  \"question\": \"问题内容\",\n\
                  \"related\": [\"概念1\", \"概念2\"],\n\
                  \"why\": \"为什么值得问\"\n\
                }}\n\
              ]\n\
            }}\n\
            ```\n\n\
            索引：\n{}",
            index
        );

        let response = llm.generate(&prompt).await?;
        let parsed = parse_llm_json(&response)?;

        Ok(json!({
            "questions": parsed.get("questions").cloned().unwrap_or(json!([])),
        }))
    }
}

/// 概念碰撞
pub struct ConceptCollisionHandler;

#[async_trait]
impl ToolHandler for ConceptCollisionHandler {
    fn name(&self) -> &str {
        "concept_collision"
    }
    fn description(&self) -> &str {
        "从 Wiki 概念中随机选取两个，分析交叉点"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "concept_a": { "type": "string", "description": "概念A（留空随机）" },
                "concept_b": { "type": "string", "description": "概念B（留空随机）" }
            }
        })
    }
    fn module(&self) -> &str {
        "wiki"
    }

    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let obsidian = get_client(&ctx.obsidian)?;
        let llm: Arc<dyn LlmProvider> = ctx.inspiration_service.get_llm();

        let all_files = obsidian.list_all_files().await?;
        let concepts: Vec<String> = all_files
            .iter()
            .filter(|f| f.starts_with("Wiki/concepts/") && f.ends_with(".md"))
            .map(|f| {
                f.rsplit('/')
                    .next()
                    .unwrap_or(f)
                    .trim_end_matches(".md")
                    .to_string()
            })
            .collect();

        if concepts.len() < 2 {
            return Ok(json!({
                "error": "Wiki 中概念不足 2 个，无法碰撞",
                "concept_a": null,
                "concept_b": null,
                "analysis": null,
            }));
        }

        // 选择概念
        let concept_a = args
            .get("concept_a")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                let ts = chrono::Local::now().timestamp() as usize;
                concepts[ts % concepts.len()].clone()
            });
        let concept_b = args
            .get("concept_b")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                let ts = chrono::Local::now().timestamp() as usize;
                concepts[(ts + 1) % concepts.len()].clone()
            });

        // 读取两个概念页内容
        let path_a = format!("Wiki/concepts/{}.md", concept_a);
        let path_b = format!("Wiki/concepts/{}.md", concept_b);
        let content_a = obsidian.read_file(&path_a).await.unwrap_or_default();
        let content_b = obsidian.read_file(&path_b).await.unwrap_or_default();

        let prompt = format!(
            "你是一个创意分析助手。请分析以下两个概念之间的交叉点和潜在连接。\n\n\
            概念A：{}\n{}\n\n\
            概念B：{}\n{}\n\n\
            请分析：\n\
            1. 这两个概念的交叉点是什么？\n\
            2. 它们组合后能产生什么新洞察？\n\
            3. 建议在两个页面间添加交叉引用吗？\n\n\
            严格按 JSON 格式返回：\n\
            ```json\n\
            {{\n\
              \"intersection\": \"交叉点描述\",\n\
              \"insight\": \"新洞察\",\n\
              \"should_link\": true/false,\n\
              \"link_reason\": \"引用理由\"\n\
            }}\n\
            ```",
            concept_a, content_a, concept_b, content_b
        );

        let response = llm.generate(&prompt).await?;
        let parsed = parse_llm_json(&response)?;

        Ok(json!({
            "concept_a": concept_a,
            "concept_b": concept_b,
            "analysis": parsed,
        }))
    }
}

/// 解析 LLM 返回的 JSON
fn parse_llm_json(text: &str) -> Result<Value, BrainError> {
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
