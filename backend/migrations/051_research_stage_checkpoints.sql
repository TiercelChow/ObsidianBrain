-- Business scope, findings, report sections and checks; no chain-of-thought.
CREATE TABLE IF NOT EXISTS knowledge_research_workspaces (
    task_id TEXT PRIMARY KEY REFERENCES knowledge_tasks(id) ON DELETE CASCADE,
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    original_request_json TEXT NOT NULL,
    original_request_hash TEXT NOT NULL,
    plan_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS knowledge_research_stages (
    task_id TEXT NOT NULL REFERENCES knowledge_research_workspaces(task_id) ON DELETE CASCADE,
    stage_key TEXT NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('plan','section','synthesis','report','validation','presentation')),
    ordinal INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','stale')),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
    claim_id TEXT,
    claimed_attempt INTEGER,
    run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    content_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    summary TEXT NOT NULL DEFAULT '',
    content_md TEXT NOT NULL DEFAULT '',
    findings_json TEXT NOT NULL DEFAULT '[]',
    evidence_json TEXT NOT NULL DEFAULT '[]',
    error TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(task_id,stage_key)
);
CREATE TABLE IF NOT EXISTS knowledge_research_stage_versions (
    task_id TEXT NOT NULL,
    stage_key TEXT NOT NULL,
    revision INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_id,stage_key,revision),
    FOREIGN KEY(task_id,stage_key) REFERENCES knowledge_research_stages(task_id,stage_key) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_research_stages_task_order
ON knowledge_research_stages(task_id,ordinal);

-- Frozen business inputs, not current evidence or automatically trusted facts.
CREATE TABLE IF NOT EXISTS knowledge_research_baselines (
    task_id TEXT NOT NULL REFERENCES knowledge_research_workspaces(task_id) ON DELETE CASCADE,
    question_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    captured_at TEXT NOT NULL,
    PRIMARY KEY(task_id,question_id,entry_id)
);
