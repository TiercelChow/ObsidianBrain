//! Resource allocation for a pure compile transform, not an agent loop.

use serde::Serialize;

use super::qa_policy::estimated_tokens;
use crate::error::BrainError;

const HARNESS_HEADROOM: u64 = 2048;
const REPAIR_HEADROOM: u64 = 4096;

#[derive(Debug, Serialize)]
pub(super) struct CompileResources {
    pub capacity_tokens: u64,
    pub capacity_basis: &'static str,
    pub source_character_budget: usize,
    pub identity_tokens: u64,
    pub output_reserve_tokens: u32,
}

#[derive(Debug, Serialize)]
pub(super) struct CompileAllocation {
    pub estimated_prompt_tokens: u64,
    pub output_tokens: u32,
    pub retry_output_tokens: u32,
    pub timeout_seconds: u64,
}

impl CompileResources {
    pub fn new(
        context: Option<u32>,
        output_cap: Option<u32>,
        fixed_prompt: &str,
    ) -> Result<Self, BrainError> {
        let capacity_tokens = u64::from(context.unwrap_or(32768)).min(1048576);
        let output_reserve_tokens = (capacity_tokens / 5)
            .min(65536)
            .min(u64::from(output_cap.unwrap_or(65536)))
            .max(1) as u32;
        let identity_tokens = (capacity_tokens / 12).clamp(256, 16384);
        let source_tokens = capacity_tokens
            .saturating_sub(
                estimated_tokens(fixed_prompt)
                    .saturating_add(HARNESS_HEADROOM + REPAIR_HEADROOM + identity_tokens)
                    .saturating_add(u64::from(output_reserve_tokens)),
            )
            .min(u64::from(output_reserve_tokens) * 3)
            .min(262144);
        if source_tokens < 512 {
            return Err(BrainError::KnowledgeValidation("模型上下文不足以容纳编译契约、配置、完整知识正文及修复余量；请核实模型容量或缩减本书配置，未截断来源".into()));
        }
        Ok(Self {
            capacity_tokens,
            capacity_basis: if context.is_some() {
                "configured_model_capacity"
            } else {
                "unknown_model_application_guard"
            },
            source_character_budget: (source_tokens / 2) as usize,
            identity_tokens,
            output_reserve_tokens,
        })
    }

    pub fn allocate(
        &self,
        prompt: &str,
        source: &str,
        span_count: usize,
    ) -> Result<CompileAllocation, BrainError> {
        let structures = ["$$", "\\[", "```", "~~~", "|---", "| ---"]
            .iter()
            .map(|marker| source.matches(marker).count())
            .sum::<usize>()
            .min(24) as u64;
        let desired =
            1536 + estimated_tokens(source) / 2 + structures * 192 + span_count.min(12) as u64 * 96;
        let output_tokens = desired.min(u64::from(self.output_reserve_tokens)).max(1) as u32;
        let retry_output_tokens = (u64::from(output_tokens) * 3 / 2)
            .min(u64::from(self.output_reserve_tokens))
            .max(1) as u32;
        self.check_prompt(prompt, retry_output_tokens)?;
        Ok(CompileAllocation {
            estimated_prompt_tokens: estimated_tokens(prompt),
            output_tokens,
            retry_output_tokens,
            timeout_seconds: (90
                + estimated_tokens(source) / 128
                + u64::from(retry_output_tokens) / 96)
                .clamp(90, 600),
        })
    }

    pub fn check_prompt(&self, prompt: &str, output: u32) -> Result<(), BrainError> {
        if estimated_tokens(prompt)
            .saturating_add(HARNESS_HEADROOM + REPAIR_HEADROOM)
            .saturating_add(u64::from(output))
            > self.capacity_tokens
        {
            return Err(BrainError::KnowledgeValidation(format!("完整编译输入或不可拆分的表格/公式/代码超过本轮上下文护栏（{} tokens，{}）；请核实模型容量或将超大独立结构拆为有明确边界的原文章节，未截断或推进检查点", self.capacity_tokens, self.capacity_basis)));
        }
        Ok(())
    }

    pub fn check_repair_prompt(&self, prompt: &str, output: u32) -> Result<(), BrainError> {
        if estimated_tokens(prompt)
            .saturating_add(HARNESS_HEADROOM)
            .saturating_add(u64::from(output))
            > self.capacity_tokens
        {
            return Err(BrainError::KnowledgeValidation(
                "编译格式修复输入超过模型上下文护栏，未重复超容量请求或推进检查点".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_policy_scales_source_and_output_with_model_capacity() {
        let small = CompileResources::new(None, None, "基础契约").unwrap();
        let large = CompileResources::new(Some(262144), Some(65536), "基础契约").unwrap();
        assert!(large.source_character_budget > small.source_character_budget);
        let short = small.allocate("简短原文", "定义", 1).unwrap();
        let detailed = large
            .allocate(
                &"复杂正文".repeat(8000),
                &"$$x_i$$\n|A|B|\n```rust\n".repeat(30),
                10,
            )
            .unwrap();
        assert!(detailed.output_tokens > short.output_tokens);
        assert!(detailed.timeout_seconds > short.timeout_seconds);
        assert!(detailed.output_tokens <= 65536);
        assert!(large
            .check_prompt(&"超大公式".repeat(100000), detailed.output_tokens)
            .is_err());
        assert_eq!(small.capacity_basis, "unknown_model_application_guard");
        assert_eq!(large.capacity_basis, "configured_model_capacity");
    }

    #[test]
    fn test_compile_policy_respects_small_output_and_rejects_oversize_contract() {
        let resources = CompileResources::new(Some(32768), Some(2048), "契约").unwrap();
        let allocation = resources
            .allocate("正文", &"步骤\n".repeat(300), 5)
            .unwrap();
        assert!(allocation.output_tokens <= 2048);
        assert!(allocation.retry_output_tokens <= 2048);
        assert!(CompileResources::new(Some(4096), None, &"长配置".repeat(3000)).is_err());
    }
}
