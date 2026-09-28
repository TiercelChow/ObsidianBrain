//! Shared application defaults, not declarations of a provider's capabilities.

pub const DEFAULT_AGENT_CONTEXT_TOKENS: u32 = 1_048_576;
pub const MAX_AGENT_OUTPUT_TOKENS: u32 = 262_144;

pub fn context_capacity(declared: Option<u32>) -> u64 {
    u64::from(declared.unwrap_or(DEFAULT_AGENT_CONTEXT_TOKENS))
}

pub fn output_limit(context: Option<u32>, declared_output: Option<u32>) -> u32 {
    let capacity = context_capacity(context);
    // Leave room for instructions/tools even on explicitly small models, but
    // do not confuse a fixed fraction of context with the output capability.
    let reserve = (capacity / 2).min(8192);
    capacity
        .saturating_sub(reserve)
        .min(u64::from(
            declared_output.unwrap_or(MAX_AGENT_OUTPUT_TOKENS),
        ))
        .min(u64::from(MAX_AGENT_OUTPUT_TOKENS))
        .max(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_defaults_do_not_override_declared_small_or_larger_models() {
        assert_eq!(context_capacity(None), 1_048_576);
        assert_eq!(context_capacity(Some(8192)), 8192);
        assert_eq!(context_capacity(Some(2_097_152)), 2_097_152);
    }

    #[test]
    fn test_output_limit_respects_provider_cap_and_leaves_context_room() {
        assert_eq!(output_limit(None, Some(65_536)), 65_536);
        assert_eq!(output_limit(Some(65_536), Some(40_000)), 40_000);
        assert_eq!(output_limit(Some(8192), Some(2048)), 2048);
        assert_eq!(output_limit(None, None), MAX_AGENT_OUTPUT_TOKENS);
    }
}
