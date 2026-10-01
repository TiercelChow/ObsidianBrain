//! Business resource policy. Harness still owns reasoning and the tool loop.

use crate::error::BrainError;
use crate::models::agent_budget::{
    context_capacity, output_limit, output_timeout_allowance_seconds, MAX_AGENT_OUTPUT_TOKENS,
    MAX_KNOWLEDGE_PHASE_TIMEOUT_SECONDS,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct QaPlan {
    pub goal: String,
    pub constraints: Vec<String>,
    pub subquestions: Vec<String>,
    pub evidence_requirements: Vec<String>,
    pub depth: String,
    pub scope: String,
    pub expected_output_tokens: Option<u32>,
}

impl QaPlan {
    pub fn fallback(question: &str) -> Self {
        // Used only before/when the lightweight planner is unavailable; the
        // model's explicit plan supersedes this conservative heuristic.
        let lower = question.to_lowercase();
        let broad = ["全书", "全部", "所有", "全面", "whole book", "all chapters"]
            .iter()
            .any(|word| lower.contains(word));
        let detailed = ["详细", "深入", "公式", "推导", "比较", "compare", "derive"]
            .iter()
            .any(|word| lower.contains(word));
        Self {
            goal: question.chars().take(800).collect(),
            constraints: vec![],
            subquestions: vec![question.chars().take(400).collect()],
            evidence_requirements: vec!["优先编译正文；公式、条件或冲突需要回读原文".into()],
            depth: if broad {
                "comprehensive"
            } else if detailed {
                "detailed"
            } else {
                "standard"
            }
            .into(),
            scope: if broad {
                "whole_book"
            } else if detailed {
                "cross_topic"
            } else {
                "focused"
            }
            .into(),
            expected_output_tokens: None,
        }
    }

    pub fn normalize(&mut self, question: &str) {
        self.goal = self.goal.trim().chars().take(800).collect();
        if self.goal.is_empty() {
            self.goal = question.chars().take(800).collect();
        }
        self.constraints = bounded_list(&self.constraints, 16, 300);
        self.subquestions = bounded_list(&self.subquestions, 12, 400);
        if self.subquestions.is_empty() {
            self.subquestions.push(question.chars().take(400).collect());
        }
        self.evidence_requirements = bounded_list(&self.evidence_requirements, 12, 400);
        if !["brief", "standard", "detailed", "comprehensive"].contains(&self.depth.as_str()) {
            self.depth = "standard".into();
        }
        if !["focused", "cross_topic", "whole_book"].contains(&self.scope.as_str()) {
            self.scope = "focused".into();
        }
        self.expected_output_tokens = self
            .expected_output_tokens
            .filter(|v| *v > 0)
            .map(|v| v.min(262_144));
    }
}

pub(super) fn bounded_list(values: &[String], items: usize, chars: usize) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let value: String = value.trim().chars().take(chars).collect();
        if !value.is_empty() && !result.contains(&value) {
            result.push(value);
        }
        if result.len() >= items {
            break;
        }
    }
    result
}

/// A conservative UTF-8 estimate, NOT the provider's tokenizer or bill. Runtime
/// occupancy supersedes it once ACP reports a context update.
pub(super) fn estimated_tokens(value: &str) -> u64 {
    let (cjk, other_bytes) = value.chars().fold((0_u64, 0_u64), |(cjk, other), ch| {
        if matches!(ch as u32,0x3400..=0x4dbf|0x4e00..=0x9fff|0xf900..=0xfaff) {
            (cjk + 1, other)
        } else {
            (cjk, other + ch.len_utf8() as u64)
        }
    });
    cjk.saturating_mul(2) + other_bytes.div_ceil(3)
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct QaResources {
    pub prompt_tokens: u64,
    pub output_tokens: u32,
    pub tool_reserve_tokens: u64,
    pub initial_entry_target: usize,
    pub soft_tool_calls: u32,
    pub hard_tool_calls: u32,
    pub soft_retrieval_tokens: u64,
    pub hard_retrieval_tokens: u64,
    pub timeout_seconds: u32,
    pub planning_token_limit: u64,
    pub planning_seconds: u32,
    pub context_capacity_known: bool,
}

impl QaResources {
    /// Reject an estimated answer that will substantially exceed a known
    /// request bound. Keep a wider tolerance when only the application guard
    /// is known, because it is not a provider output capability declaration.
    pub fn check_visible_output_fit(
        plan: &QaPlan,
        context: Option<u32>,
        output_cap: Option<u32>,
        reasoning_policy: &str,
    ) -> Result<(), BrainError> {
        let Some(expected) = plan.expected_output_tokens else {
            return Ok(());
        };
        if context.is_none() && output_cap.is_none() {
            return Ok(());
        }
        let cap = output_limit(context, output_cap);
        let reasoning_margin = if reasoning_policy == "off" {
            0
        } else {
            (cap / 4).min(8192)
        };
        let visible = cap.saturating_sub(reasoning_margin);
        let extra_questions =
            u32::try_from(plan.subquestions.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let planned = expected.saturating_add(extra_questions.saturating_mul(768));
        let constrained = output_cap.is_some()
            || context
                .is_some_and(|window| output_limit(Some(window), None) < MAX_AGENT_OUTPUT_TOKENS);
        let tolerance = if constrained {
            visible.saturating_add(visible / 4)
        } else {
            visible.saturating_mul(2)
        };
        if planned > tolerance {
            return Err(BrainError::KnowledgeValidation(format!(
                "(qa_output_scope_limit) 规划器估计本轮完整回答约需 {planned} token 可见正文；当前单次有效输出上限为 {cap} token，扣除估计推理余量后约有 {visible} token 可用于正文。该估计不是实际用量，但差距过大，按当前深度发起回答很可能截断；尚未启动回答 Run。请缩小问题、提高模型单次输出能力，或转为分阶段研究任务。"
            )));
        }
        Ok(())
    }

    pub fn with_reasoning_headroom(
        mut self,
        context: Option<u32>,
        cap: Option<u32>,
        reasoning_policy: &str,
    ) -> Self {
        if reasoning_policy == "off" {
            return self;
        }
        // The planner estimates visible answer length. Harness may consume
        // output tokens while reasoning before emitting any answer text.
        let reserve = self.output_tokens.saturating_mul(2).clamp(8192, 65536);
        let output = self
            .output_tokens
            .saturating_add(reserve)
            .min(output_limit(context, cap));
        if output > self.output_tokens {
            let previous = self.output_tokens;
            self.output_tokens = output;
            let available = context_capacity(context).saturating_sub(u64::from(output));
            self.tool_reserve_tokens = (available / 3).max(1);
            self.prompt_tokens = available
                .saturating_sub(self.tool_reserve_tokens)
                .saturating_sub(1024);
            self.soft_retrieval_tokens = self
                .tool_reserve_tokens
                .min(u64::from(self.soft_tool_calls) * 2500);
            self.hard_retrieval_tokens = self.soft_retrieval_tokens.saturating_mul(4);
            self.timeout_seconds = self
                .timeout_seconds
                .saturating_add(output_timeout_allowance_seconds(
                    output.saturating_sub(previous),
                ))
                .min(MAX_KNOWLEDGE_PHASE_TIMEOUT_SECONDS);
        }
        self
    }

    pub fn for_resume(
        mut self,
        previous_output: u32,
        context: Option<u32>,
        cap: Option<u32>,
    ) -> Self {
        let capacity = context_capacity(context);
        self.context_capacity_known |= context.is_some();
        let previous = self.output_tokens;
        self.output_tokens = self
            .output_tokens
            .max(previous_output.saturating_mul(2))
            .min(output_limit(context, cap));
        let available = capacity.saturating_sub(u64::from(self.output_tokens));
        self.tool_reserve_tokens = available / 3;
        self.prompt_tokens = available
            .saturating_sub(self.tool_reserve_tokens)
            .saturating_sub(1024);
        self.soft_retrieval_tokens = self
            .tool_reserve_tokens
            .min(u64::from(self.soft_tool_calls) * 2500);
        self.hard_retrieval_tokens = self.soft_retrieval_tokens.saturating_mul(4);
        self.timeout_seconds = self
            .timeout_seconds
            .saturating_add(output_timeout_allowance_seconds(
                self.output_tokens.saturating_sub(previous),
            ))
            .min(MAX_KNOWLEDGE_PHASE_TIMEOUT_SECONDS);
        self
    }
    pub fn new(
        plan: &QaPlan,
        context: Option<u32>,
        output_cap: Option<u32>,
        catalog_size: usize,
    ) -> Self {
        // Unknown capacity is intentionally an application request guard, not
        // a fabricated claim about the model. Known large contexts are usable.
        let capacity = context_capacity(context);
        let weight: u32 = match plan.depth.as_str() {
            "brief" => 1,
            "detailed" => 3,
            "comprehensive" => 5,
            _ => 2,
        };
        let breadth: u32 = match plan.scope.as_str() {
            "whole_book" => 5,
            "cross_topic" => 3,
            _ => 1,
        };
        let questions = plan.subquestions.len().max(1) as u32;
        let desired = plan
            .expected_output_tokens
            .unwrap_or(match plan.depth.as_str() {
                "brief" => 1024,
                "detailed" => 8192,
                "comprehensive" => 16384,
                _ => 4096,
            })
            .saturating_add(768 * questions.saturating_sub(1));
        let output_tokens = desired.min(output_limit(context, output_cap)).max(1);
        let available = capacity.saturating_sub(u64::from(output_tokens));
        let tool_reserve_tokens = (available / 3).max(1);
        let prompt_tokens = available
            .saturating_sub(tool_reserve_tokens)
            .saturating_sub(1024);
        let target = (if plan.scope == "whole_book" {
            catalog_size.max(6)
        } else {
            (breadth * 4 + questions * 2) as usize
        })
        .min(if catalog_size == 0 {
            usize::MAX
        } else {
            catalog_size
        })
        .min((prompt_tokens / 768).max(1) as usize);
        let soft_tool_calls = (6 + questions * 3 + breadth * 4 + (target / 4) as u32).min(80);
        let hard_tool_calls = soft_tool_calls.saturating_mul(4).min(240);
        let soft_retrieval_tokens = tool_reserve_tokens.min(u64::from(soft_tool_calls) * 2500);
        Self {
            prompt_tokens,
            output_tokens,
            tool_reserve_tokens,
            initial_entry_target: target,
            soft_tool_calls,
            hard_tool_calls,
            soft_retrieval_tokens,
            hard_retrieval_tokens: soft_retrieval_tokens.saturating_mul(4),
            timeout_seconds: (90 + weight * 45 + questions * 10)
                .saturating_add(output_timeout_allowance_seconds(output_tokens))
                .min(MAX_KNOWLEDGE_PHASE_TIMEOUT_SECONDS),
            planning_token_limit: capacity.saturating_mul(2).min(2_097_152),
            planning_seconds: 60 + breadth * 30,
            context_capacity_known: context.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_large_answer_plan_requires_staged_research_before_answer_run() {
        let mut plan = QaPlan::fallback("全面比较全书所有机制及证据");
        plan.subquestions = vec!["机制".into(), "证据".into(), "边界".into()];
        plan.expected_output_tokens = Some(50_000);
        let error =
            QaResources::check_visible_output_fit(&plan, Some(1_048_576), Some(4_096), "auto")
                .unwrap_err();
        assert!(error.to_string().contains("qa_output_scope_limit"));
        assert!(error.to_string().contains("分阶段研究任务"));
        assert!(QaResources::check_visible_output_fit(&plan, None, None, "auto").is_ok());
        assert!(QaResources::check_visible_output_fit(
            &plan,
            Some(1_048_576),
            Some(65_536),
            "auto"
        )
        .is_ok());
        plan.subquestions.truncate(1);
        plan.expected_output_tokens = Some(5_000);
        assert!(
            QaResources::check_visible_output_fit(&plan, Some(1_048_576), Some(4_096), "auto")
                .unwrap_err()
                .to_string()
                .contains("qa_output_scope_limit")
        );
        assert!(
            QaResources::check_visible_output_fit(&plan, Some(8_192), None, "auto")
                .unwrap_err()
                .to_string()
                .contains("qa_output_scope_limit")
        );
        plan.expected_output_tokens = Some(3_600);
        assert!(
            QaResources::check_visible_output_fit(&plan, Some(1_048_576), Some(4_096), "auto")
                .is_ok()
        );
        plan.expected_output_tokens = None;
        assert!(
            QaResources::check_visible_output_fit(&plan, Some(1_048_576), Some(4_096), "auto")
                .is_ok()
        );
    }

    #[test]
    fn test_qa_reasoning_headroom_is_separate_from_answer_length_and_respects_model_cap() {
        let mut plan = QaPlan::fallback("解释机制");
        plan.expected_output_tokens = Some(7000);
        let base = QaResources::new(&plan, None, None, 14);
        assert_eq!(base.output_tokens, 7000);
        let reasoning = base.clone().with_reasoning_headroom(None, None, "auto");
        assert_eq!(reasoning.output_tokens, 21_000);
        assert!(reasoning.prompt_tokens < base.prompt_tokens);
        assert_eq!(
            base.clone()
                .with_reasoning_headroom(None, None, "off")
                .output_tokens,
            7000
        );
        assert_eq!(
            base.with_reasoning_headroom(Some(65_536), Some(12_000), "auto")
                .output_tokens,
            12_000
        );
    }

    #[test]
    fn test_large_answer_timeout_grows_with_requested_output_and_stays_bounded() {
        let mut plan = QaPlan::fallback("系统梳理整本书的机制与反例");
        plan.depth = "comprehensive".into();
        plan.expected_output_tokens = Some(64_000);
        let initial = QaResources::new(&plan, None, None, 20);
        assert!(initial.timeout_seconds > 600);
        assert!(initial.timeout_seconds < 2_400);
        let expanded = initial.clone().with_reasoning_headroom(None, None, "auto");
        assert!(expanded.timeout_seconds > initial.timeout_seconds);
        assert!(expanded.timeout_seconds <= 2_400);
        let resumed = expanded
            .clone()
            .for_resume(expanded.output_tokens, None, None);
        assert!(resumed.timeout_seconds >= expanded.timeout_seconds);
        assert!(resumed.timeout_seconds <= 2_400);
    }

    #[test]
    fn test_unknown_context_allows_large_answers_and_resume_without_a_fixed_fraction_cap() {
        let mut plan = QaPlan::fallback("详细研究");
        plan.expected_output_tokens = Some(120_000);
        let budget = QaResources::new(&plan, None, None, 100);
        assert_eq!(budget.output_tokens, 120_000);
        assert!(!budget.context_capacity_known);
        assert!(budget.prompt_tokens > 100_000);
        let resumed = budget.for_resume(120_000, Some(1_048_576), Some(500_000));
        assert_eq!(resumed.output_tokens, 240_000);
        let mut medium = plan;
        medium.expected_output_tokens = Some(20_000);
        let budget = QaResources::new(&medium, Some(65_536), Some(40_000), 100);
        assert_eq!(budget.output_tokens, 20_000);
        assert_eq!(
            budget
                .for_resume(20_000, Some(65_536), Some(40_000))
                .output_tokens,
            40_000
        );
    }

    #[test]
    fn test_broad_question_has_larger_but_bounded_budget() {
        let small = QaPlan::fallback("KV cache是什么？");
        let broad = QaPlan::fallback("请详细比较全书的全部优化机制，给出公式、步骤和边界");
        let small_budget = QaResources::new(&small, None, None, 100);
        let broad_budget = QaResources::new(&broad, Some(131_072), Some(20_000), 100);
        assert!(broad_budget.output_tokens > small_budget.output_tokens);
        assert!(broad_budget.initial_entry_target > 10);
        assert!(broad_budget.soft_tool_calls > small_budget.soft_tool_calls);
        assert!(broad_budget.output_tokens <= 20_000);
        assert!(broad_budget.prompt_tokens + u64::from(broad_budget.output_tokens) < 131_072);
    }

    #[test]
    fn test_small_declared_context_reserves_output_and_tool_room() {
        let plan = QaPlan::fallback("请详细解释全部章节");
        let budget = QaResources::new(&plan, Some(8192), Some(2048), 1000);
        assert!(
            budget.prompt_tokens + u64::from(budget.output_tokens) + budget.tool_reserve_tokens
                <= 8192
        );
        assert!(budget.output_tokens <= 2048);
        assert!(budget.initial_entry_target < 1000);
    }

    #[test]
    fn test_budget_estimate_is_conservative_for_cjk_and_handles_empty() {
        assert_eq!(estimated_tokens(""), 0);
        assert!(estimated_tokens("测试abcd") >= 5);
    }

    #[test]
    fn test_plan_normalization_does_not_accept_unbounded_model_fields() {
        let mut plan = QaPlan {
            goal: "g".repeat(3000),
            constraints: vec!["c".repeat(1000); 100],
            subquestions: vec!["q".repeat(1000); 100],
            evidence_requirements: vec!["e".repeat(1000); 100],
            depth: "invalid".into(),
            scope: "invalid".into(),
            expected_output_tokens: None,
        };
        plan.normalize("current question");
        assert_eq!(plan.goal.chars().count(), 800);
        assert_eq!(plan.subquestions.len(), 1);
        assert_eq!(plan.depth, "standard");
        assert_eq!(plan.scope, "focused");
        assert!(!plan.subquestions[0].is_empty());
    }
}
