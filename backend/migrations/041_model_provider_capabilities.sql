-- Unknown capacities remain NULL; never infer capabilities from a model name.
ALTER TABLE llm_provider_profiles ADD COLUMN context_window INTEGER
    CHECK (context_window IS NULL OR (context_window > 0 AND context_window <= 4294967295));

ALTER TABLE llm_provider_profiles ADD COLUMN max_output_tokens INTEGER
    CHECK (max_output_tokens IS NULL OR
           (max_output_tokens > 0 AND max_output_tokens <= 4294967295
            AND (context_window IS NULL OR max_output_tokens < context_window)));

-- Harness v0.1.5-rc.1 pi-ai THINKING_LEVELS; auto is application-side inheritance.
ALTER TABLE llm_provider_profiles ADD COLUMN reasoning_policy TEXT NOT NULL DEFAULT 'auto'
    CHECK (reasoning_policy IN ('auto', 'off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'));

-- The relationship must also be checked when only context_window is changed.
CREATE TRIGGER llm_provider_capabilities_context_insert
BEFORE INSERT ON llm_provider_profiles
WHEN NEW.context_window IS NOT NULL AND NEW.max_output_tokens IS NOT NULL
     AND NEW.max_output_tokens >= NEW.context_window
BEGIN
    SELECT RAISE(ABORT, 'model output capacity must be smaller than context window');
END;

CREATE TRIGGER llm_provider_capabilities_context_update
BEFORE UPDATE OF context_window, max_output_tokens ON llm_provider_profiles
WHEN NEW.context_window IS NOT NULL AND NEW.max_output_tokens IS NOT NULL
     AND NEW.max_output_tokens >= NEW.context_window
BEGIN
    SELECT RAISE(ABORT, 'model output capacity must be smaller than context window');
END;
