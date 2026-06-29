//! LLM 创意生成器

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use crate::error::BrainError;
use crate::infra::llm_client::LlmProvider;
use crate::models::inspiration::{
    ComboOutput, CounterpointOutput, InspirationConfig, InspirationType,
    QuestionOutput,
};

/// LLM 创意生成器
pub struct LlmCreativeGenerator {
    llm: Arc<std::sync::RwLock<Arc<dyn LlmProvider>>>,
    #[allow(dead_code)]
    config: InspirationConfig,
}

impl LlmCreativeGenerator {
    pub fn new(llm: Arc<dyn LlmProvider>, config: InspirationConfig) -> Self {
        Self {
            llm: Arc::new(std::sync::RwLock::new(llm)),
            config,
        }
    }

    /// 获取底层 LLM provider（供外部模块复用）
    pub fn get_llm(&self) -> Arc<dyn LlmProvider> {
        self.llm.read().unwrap().clone()
    }

    /// 热更新 LLM provider
    pub fn set_llm(&self, new_llm: Arc<dyn LlmProvider>) {
        *self.llm.write().unwrap() = new_llm;
    }

    /// 生成概念组合
    pub async fn generate_combo(
        &self,
        term_a: &str,
        context_a: &str,
        term_b: &str,
        context_b: &str,
    ) -> Result<ComboOutput, BrainError> {
        let prompt = format!(
            r#"你是一个创意催化剂，擅长在不同领域的知识之间建立出人意料的联系。

## 任务

请基于以下两个概念，生成一个跨界创意联想。这个联想应该是合理的、有启发性的，而不是牵强的。

## 概念 A：{}
相关笔记内容：
{}

## 概念 B：{}
相关笔记内容：
{}

## 输出要求

请以 JSON 格式输出，包含以下字段：

{{
  "inspiration": "一段 200-400 字的跨界创意联想",
  "suggestions": ["具体实践建议 1", "具体实践建议 2", "具体实践建议 3"],
  "experiment_idea": "一个可以在 1-2 周内完成的小型实验方案（可为 null）"
}}

请直接输出 JSON，不要有其他文字。"#,
            term_a, context_a, term_b, context_b
        );

        let response = self.get_llm().generate(&prompt).await?;
        self.parse_json_response(&response)
    }

    /// 生成反向提问
    pub async fn generate_questions(
        &self,
        title: &str,
        content: &str,
    ) -> Result<QuestionOutput, BrainError> {
        let truncated = if content.len() > 6000 {
            &content[..6000]
        } else {
            content
        };

        let prompt = format!(
            r#"你是一个苏格拉底式的提问者，擅长通过深刻的问题帮助人们发现自己思维中的盲点和隐含假设。

## 任务

阅读以下笔记，然后生成 3 个作者可能从未想过但值得深入思考的问题。

## 笔记标题：{}

## 笔记内容

{}

## 输出要求

请以 JSON 格式输出：

{{
  "questions": [
    {{
      "question": "问题的完整表述",
      "why_it_matters": "为什么这个问题值得思考",
      "question_type": "counterfactual | extension | logic_check | temporal_projection"
    }}
  ]
}}

请直接输出 JSON，不要有其他文字。"#,
            title, truncated
        );

        let response = self.get_llm().generate(&prompt).await?;
        self.parse_json_response(&response)
    }

    /// 生成对立观点
    pub async fn generate_counterpoints(
        &self,
        title: &str,
        content: &str,
    ) -> Result<CounterpointOutput, BrainError> {
        let truncated = if content.len() > 6000 {
            &content[..6000]
        } else {
            content
        };

        let prompt = format!(
            r#"你是一位严谨的学术审稿人和"魔鬼代言人"，你的任务是帮助作者发现自己论证中的盲点和薄弱环节。

## 任务

阅读以下笔记，识别其中的核心主张，然后对每个主张生成反方观点、指出逻辑漏洞，并提供完善论证的建议。

## 笔记标题：{}

## 笔记内容

{}

## 输出要求

请以 JSON 格式输出：

{{
  "counterpoints": [
    {{
      "claim": "笔记中的主张原文",
      "counter": "反方观点",
      "weakness": "逻辑漏洞分析",
      "suggestion": "完善论证的具体建议"
    }}
  ],
  "overall_assessment": "整体评估：论证的整体强度、最薄弱的环节、最值得优先完善的方向"
}}

请直接输出 JSON，不要有其他文字。"#,
            title, truncated
        );

        let response = self.get_llm().generate(&prompt).await?;
        self.parse_json_response(&response)
    }

    /// 解析 JSON 响应
    fn parse_json_response<T: serde::de::DeserializeOwned>(&self, response: &str) -> Result<T, BrainError> {
        // 尝试提取 JSON（可能被 markdown 代码块包裹）
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        serde_json::from_str(json_str).map_err(|e| {
            BrainError::Internal(format!("LLM 响应解析失败: {e}\n原始响应: {}",
                if response.len() > 500 { &response[..500] } else { response }
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_response_clean() {
        let generator = create_test_generator();
        let json = r#"{"inspiration": "test", "suggestions": ["a"], "experiment_idea": null}"#;
        let result: Result<ComboOutput, _> = generator.parse_json_response(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_json_response_with_markdown() {
        let generator = create_test_generator();
        let json = r#"```json
{"inspiration": "test", "suggestions": ["a"], "experiment_idea": null}
```"#;
        let result: Result<ComboOutput, _> = generator.parse_json_response(json);
        assert!(result.is_ok());
    }

    fn create_test_generator() -> LlmCreativeGenerator {
        use crate::infra::llm_client::OllamaProvider;
        use crate::config::LlmConfig;

        let config = LlmConfig::default();
        let llm: Arc<dyn LlmProvider> = Arc::new(OllamaProvider::new(&config).unwrap());
        let insp_config = InspirationConfig::default();
        LlmCreativeGenerator::new(llm, insp_config)
    }
}
