//! Per-deliverable resource allocation. Harness owns retrieval and reasoning.

use super::qa_policy::{estimated_tokens, QaPlan, QaResources};
use crate::error::BrainError;
use crate::infra::book_wiki_store::AdaptiveBudgetPolicy;
use crate::models::agent_budget::{context_capacity, output_limit};
use crate::models::book_wiki::{ResearchPlan, ResearchQuestion, RuntimeProfile};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(super) struct ResearchResources {
    pub capacity_tokens: u64,
    pub capacity_basis: &'static str,
    pub output_tokens: u32,
    pub content_output_tokens: u32,
    pub structure_output_tokens: u32,
    pub reasoning_output_tokens: u32,
    pub prompt_token_limit: u64,
    pub initial_entry_target: usize,
    pub policy: AdaptiveBudgetPolicy,
}

impl ResearchResources {
    pub fn new(
        profile: &RuntimeProfile,
        plan: Option<&ResearchPlan>,
        question: Option<&ResearchQuestion>,
        catalog_size: usize,
    ) -> Self {
        let context = profile
            .provider_config
            .as_ref()
            .and_then(|value| value.context_window);
        let output_cap = profile
            .provider_config
            .as_ref()
            .and_then(|value| value.max_output_tokens);
        let mut qa = QaPlan::fallback(
            question
                .map(|q| q.question.as_str())
                .or_else(|| plan.map(|p| p.goal.as_str()))
                .unwrap_or("明确研究范围与证据需求"),
        );
        qa.depth = match plan.map(|p| p.depth.as_str()) {
            Some("brief") => "brief",
            Some("deep") => "comprehensive",
            _ => "detailed",
        }
        .into();
        qa.scope = "cross_topic".into();
        let requirements = question
            .map(|q| q.required_evidence.clone())
            .unwrap_or_else(|| vec!["研究目标、边界、验收条件与具体子问题".into()]);
        qa.subquestions = requirements;
        let content_output_tokens =
            question
                .and_then(|q| q.expected_output_tokens)
                .unwrap_or(match qa.depth.as_str() {
                    "brief" => 1024,
                    "comprehensive" => 16384,
                    _ => 8192,
                });
        let structure_output_tokens =
            1024 + 768 * qa.subquestions.len().max(1).saturating_sub(1) as u32;
        let reasoning_output_tokens = if profile
            .provider_config
            .as_ref()
            .is_some_and(|provider| provider.reasoning_policy == "off")
        {
            0
        } else {
            // Planning estimates final content, not hidden reasoning or JSON.
            // This is request headroom, never measured usage or a length target.
            content_output_tokens.saturating_mul(2).clamp(8192, 65536)
        };
        // QaResources adds the per-requirement structural reserve itself.
        qa.expected_output_tokens = Some(
            content_output_tokens
                .saturating_add(1024)
                .saturating_add(reasoning_output_tokens),
        );
        let resources = QaResources::new(&qa, context, output_cap, catalog_size);
        let capacity_tokens = context_capacity(context);
        let output_tokens = resources.output_tokens;
        let policy = AdaptiveBudgetPolicy {
            initial_prompt_tokens: 0,
            soft_tool_calls: resources.soft_tool_calls,
            hard_tool_calls: resources.hard_tool_calls,
            soft_retrieval_tokens: resources.soft_retrieval_tokens,
            hard_retrieval_tokens: resources.hard_retrieval_tokens,
            context_window: context,
            max_output_tokens: Some(output_tokens),
            timeout_seconds: resources.timeout_seconds,
            subquestions: if qa.subquestions.is_empty() {
                vec!["回答当前明确子问题并列出证据缺口".into()]
            } else {
                qa.subquestions
            },
        };
        Self {
            capacity_tokens,
            capacity_basis: if context.is_some() {
                "configured_model_capacity"
            } else {
                "unknown_model_application_guard"
            },
            output_tokens,
            content_output_tokens,
            structure_output_tokens,
            reasoning_output_tokens,
            prompt_token_limit: resources.prompt_tokens,
            initial_entry_target: resources.initial_entry_target,
            policy,
        }
    }

    pub fn check_prompt(&self, prompt: &str) -> Result<(), BrainError> {
        // Reserve the native Skill/tool catalog and output. Do not silently cut
        // a long objective, configuration, formula or report to make it fit.
        if estimated_tokens(prompt)
            .saturating_add(4096)
            .saturating_add(u64::from(self.output_tokens))
            >= self.capacity_tokens
        {
            return Err(BrainError::KnowledgeValidation("(research_input_hard_limit) 本研究阶段完整输入与输出预留超出模型容量；请配置实际模型上下文或拆小研究目标，已有成果保留，未截尾伪装完成".into()));
        }
        Ok(())
    }

    pub fn observe_capacity(&mut self, capacity: u64) {
        if capacity == 0 {
            return;
        }
        let capacity = capacity.min(u64::from(u32::MAX));
        self.capacity_tokens = if self.capacity_basis == "configured_model_capacity" {
            self.capacity_tokens.min(capacity)
        } else {
            self.capacity_basis = "observed_runtime_capacity";
            capacity
        };
        self.policy.context_window = Some(self.capacity_tokens as u32);
    }

    pub fn fit_expansion_to_input(
        &mut self,
        prompt_tokens: u64,
        previous: u32,
    ) -> Result<(), BrainError> {
        let available = self
            .capacity_tokens
            .saturating_sub(prompt_tokens)
            .saturating_sub(6144);
        self.output_tokens = self
            .output_tokens
            .min(available.min(u64::from(u32::MAX)) as u32);
        if self.output_tokens <= previous {
            return Err(BrainError::KnowledgeValidation("(research_output_hard_limit) 当前完整输入与工具预留后已无输出扩容空间；未裁剪材料，部分结果仍保留，请调整实际容量或拆分主题".into()));
        }
        self.policy.max_output_tokens = Some(self.output_tokens);
        self.prompt_token_limit = self
            .capacity_tokens
            .saturating_sub(u64::from(self.output_tokens))
            .saturating_mul(2)
            / 3;
        Ok(())
    }

    pub fn expand_output_after_truncation(
        mut self,
        profile: &RuntimeProfile,
        previous: u32,
    ) -> Result<Self, BrainError> {
        let provider_cap = profile
            .provider_config
            .as_ref()
            .and_then(|value| value.max_output_tokens);
        let context = Some(self.capacity_tokens.min(u64::from(u32::MAX)) as u32);
        let output = self
            .output_tokens
            .max(previous.saturating_mul(2))
            .min(output_limit(context, provider_cap));
        if output <= previous {
            return Err(BrainError::KnowledgeValidation("(research_output_hard_limit) 研究章节输出已达配置或容量硬上限；已有阶段保留，请拆分主题或配置真实模型能力，不重复同一截断请求".into()));
        }
        self.output_tokens = output;
        self.policy.max_output_tokens = Some(output);
        let available = self.capacity_tokens.saturating_sub(u64::from(output));
        self.prompt_token_limit = (available * 2 / 3).saturating_sub(1024);
        self.policy.soft_retrieval_tokens =
            self.policy.soft_retrieval_tokens.min(available / 3).max(1);
        self.policy.hard_retrieval_tokens = self.policy.soft_retrieval_tokens.saturating_mul(4);
        self.policy.timeout_seconds = self
            .policy
            .timeout_seconds
            .saturating_add(output.saturating_sub(previous) / 128)
            .min(600);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::{book_wiki_store::BookWikiStore, sqlite_store::SqliteStore};
    use std::sync::Arc;

    #[test]
    fn test_research_expected_body_includes_json_and_reasoning_headroom_under_one_million_default()
    {
        let dir = tempfile::tempdir().unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("headroom.db")).unwrap(),
        ));
        let profile = store.list_runtime_profiles().unwrap().remove(0);
        let question = ResearchQuestion {
            id: "version".into(),
            title: "版本与缺口".into(),
            question: "检查版本与缺口".into(),
            required_evidence: vec!["版本".into(), "限制".into(), "核验".into()],
            expected_output_tokens: Some(3000),
            target_entry_ids: vec![],
        };
        let resources = ResearchResources::new(&profile, None, Some(&question), 100);
        assert_eq!(resources.capacity_tokens, 1_048_576);
        assert!(
            resources.output_tokens >= 12_000,
            "正文估计不能直接充当含推理的请求上限"
        );
        assert!(resources.policy.context_window.is_none());
        assert!(resources.check_prompt(&"长原文".repeat(60_000)).is_ok());
        let expanded = resources
            .clone()
            .expand_output_after_truncation(&profile, resources.output_tokens)
            .unwrap();
        assert!(expanded.output_tokens >= resources.output_tokens * 2);
        assert!(expanded.output_tokens > 10_922);
        assert_eq!(resources.content_output_tokens, 3000);
        assert_eq!(resources.structure_output_tokens, 2560);
        assert_eq!(resources.reasoning_output_tokens, 8192);
        assert_eq!(resources.output_tokens, 13752);

        let mut observed = resources.clone();
        observed.observe_capacity(65_536);
        assert_eq!(observed.capacity_basis, "observed_runtime_capacity");
        let mut observed = observed
            .expand_output_after_truncation(&profile, 20_000)
            .unwrap();
        observed.fit_expansion_to_input(30_000, 20_000).unwrap();
        assert_eq!(observed.output_tokens, 29_392);
        assert_eq!(observed.policy.max_output_tokens, Some(29_392));
        assert!(observed.fit_expansion_to_input(45_000, 20_000).is_err());
    }

    #[test]
    fn test_research_budget_scales_with_depth_evidence_gaps_and_known_model_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let store = BookWikiStore::new(Arc::new(
            SqliteStore::new(&dir.path().join("policy.db")).unwrap(),
        ));
        let profile = store.list_runtime_profiles().unwrap().remove(0);
        let question = ResearchQuestion {
            id: "q".into(),
            title: "条件".into(),
            question: "比较所有机制、完整公式与边界".into(),
            required_evidence: vec!["机制".into(), "完整公式".into(), "边界和反例".into()],
            expected_output_tokens: None,
            target_entry_ids: vec![],
        };
        let mut plan = ResearchPlan {
            goal: "比较机制".into(),
            constraints: vec![],
            acceptance: vec!["报告缺口".into()],
            depth: "brief".into(),
            terminology: vec![],
            questions: vec![question.clone()],
        };
        let brief = ResearchResources::new(&profile, Some(&plan), Some(&question), 1000);
        plan.depth = "deep".into();
        let deep = ResearchResources::new(&profile, Some(&plan), Some(&question), 1000);
        assert!(deep.output_tokens > brief.output_tokens);
        assert!(deep.policy.timeout_seconds > brief.policy.timeout_seconds);
        assert!(deep.policy.hard_tool_calls > 20);
        assert!(deep.check_prompt(&"长原文".repeat(60000)).is_ok());
        assert!(deep.check_prompt(&"长原文".repeat(600000)).is_err());
        assert_eq!(deep.capacity_basis, "unknown_model_application_guard");
        let mut longer = question.clone();
        longer.expected_output_tokens = Some(24000);
        plan.depth = "brief".into();
        let custom = ResearchResources::new(&profile, Some(&plan), Some(&longer), 1000);
        assert!(custom.output_tokens > brief.output_tokens);
        let expanded = brief
            .clone()
            .expand_output_after_truncation(&profile, brief.output_tokens)
            .unwrap();
        assert!(expanded.output_tokens > brief.output_tokens);
        assert!(expanded.output_tokens <= crate::models::agent_budget::MAX_AGENT_OUTPUT_TOKENS);
        assert!(expanded.check_prompt("完整输入").is_ok());
        let mut configured = profile.clone();
        configured.provider_config = Some(
            store
                .save_model_provider_profile(
                    "research-provider",
                    "测试大上下文",
                    "openai-completions",
                    "https://example.com/v1",
                    "test-model",
                    "environment",
                    "TEST_RESEARCH_KEY",
                    false,
                    true,
                    Some(1048576),
                    Some(65536),
                    "auto",
                    0,
                )
                .unwrap(),
        );
        longer.expected_output_tokens = Some(50000);
        let large = ResearchResources::new(&configured, Some(&plan), Some(&longer), 1000);
        assert!(large.output_tokens >= 50000);
        assert!(large.output_tokens <= 65536);
        assert_eq!(large.capacity_basis, "configured_model_capacity");
        assert!(large
            .expand_output_after_truncation(&configured, 65536)
            .is_err());
    }
}
