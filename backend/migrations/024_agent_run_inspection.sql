CREATE TABLE IF NOT EXISTS agent_run_inspections (
    run_id                  TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    prompt_text             TEXT NOT NULL,
    prompt_hash             TEXT NOT NULL,
    prompt_characters       INTEGER NOT NULL,
    skill_snapshots_json    TEXT NOT NULL DEFAULT '[]',
    config_snapshots_json   TEXT NOT NULL DEFAULT '[]',
    tool_names_json         TEXT NOT NULL DEFAULT '[]',
    evidence_refs_json      TEXT NOT NULL DEFAULT '{}',
    created_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_run_inspections_created
    ON agent_run_inspections(created_at DESC);
