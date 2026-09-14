CREATE TABLE IF NOT EXISTS agent_run_capabilities (
    id                  TEXT PRIMARY KEY,
    run_id              TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    token_hash          TEXT NOT NULL UNIQUE,
    allowed_tools_json  TEXT NOT NULL,
    expires_at          TEXT NOT NULL,
    revoked_at          TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_run_capabilities_run
    ON agent_run_capabilities(run_id, expires_at);

CREATE TABLE IF NOT EXISTS agent_run_capability_scopes (
    capability_id       TEXT NOT NULL REFERENCES agent_run_capabilities(id) ON DELETE CASCADE,
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    PRIMARY KEY (capability_id, knowledge_base_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_run_capability_scopes_base
    ON agent_run_capability_scopes(knowledge_base_id, capability_id);
