ALTER TABLE agent_runs ADD COLUMN input_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_runs ADD COLUMN output_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_runs ADD COLUMN reasoning_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_runs ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_runs ADD COLUMN cache_write_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_runs ADD COLUMN usage_source TEXT NOT NULL DEFAULT 'unavailable'
    CHECK (usage_source IN ('unavailable', 'estimated', 'measured'));

CREATE INDEX IF NOT EXISTS idx_agent_runs_usage_time
    ON agent_runs(status, finished_at, task_type, usage_source);
