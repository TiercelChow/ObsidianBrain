-- Policies are immutable per Run. Counters estimate model-visible tool payloads,
-- not provider billing tokens. Existing runs intentionally have no policy row.
CREATE TABLE agent_run_adaptive_budgets (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    policy_json TEXT NOT NULL CHECK (json_valid(policy_json)),
    soft_tool_calls INTEGER NOT NULL CHECK (soft_tool_calls > 0),
    soft_retrieval_tokens INTEGER NOT NULL CHECK (soft_retrieval_tokens > 0),
    used_tool_calls INTEGER NOT NULL DEFAULT 0 CHECK (used_tool_calls >= 0),
    used_management_calls INTEGER NOT NULL DEFAULT 0 CHECK (used_management_calls >= 0),
    estimated_tool_payload_tokens INTEGER NOT NULL DEFAULT 0 CHECK (estimated_tool_payload_tokens >= 0),
    estimated_data_payload_tokens INTEGER NOT NULL DEFAULT 0 CHECK (estimated_data_payload_tokens >= 0),
    observed_context_tokens INTEGER CHECK (observed_context_tokens >= 0),
    observed_context_window INTEGER CHECK (observed_context_window > 0),
    context_payload_tokens_at_observation INTEGER NOT NULL DEFAULT 0,
    extension_count INTEGER NOT NULL DEFAULT 0 CHECK (extension_count BETWEEN 0 AND 3),
    last_extension_evidence_segments INTEGER NOT NULL DEFAULT 0,
    coverage_json TEXT NOT NULL CHECK (json_valid(coverage_json)),
    deadline TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (soft_tool_calls <= json_extract(policy_json, '$.hard_tool_calls')),
    CHECK (soft_retrieval_tokens <= json_extract(policy_json, '$.hard_retrieval_tokens')),
    CHECK (used_tool_calls <= json_extract(policy_json, '$.hard_tool_calls')),
    CHECK (used_management_calls <= 128),
    CHECK (estimated_data_payload_tokens <= estimated_tool_payload_tokens),
    CHECK (estimated_tool_payload_tokens <= json_extract(policy_json, '$.hard_retrieval_tokens'))
);

CREATE TRIGGER agent_run_adaptive_policy_immutable
BEFORE UPDATE OF policy_json ON agent_run_adaptive_budgets
WHEN NEW.policy_json != OLD.policy_json
BEGIN
    SELECT RAISE(ABORT, 'adaptive run policy is immutable');
END;
