-- v51 was deployed before synthesis was added to its CHECK constraint.
-- Replaying CREATE TABLE IF NOT EXISTS cannot upgrade an existing table.
-- The migration runner wraps this rebuild in a transaction with foreign keys ON.
-- Save the child snapshots before dropping the parent: ON DELETE CASCADE would
-- otherwise silently erase all historical stage versions.
CREATE TEMP TABLE research_stage_versions_v52_backup AS
SELECT task_id,stage_key,revision,snapshot_json,created_at
FROM knowledge_research_stage_versions;

CREATE TABLE knowledge_research_stages_v52 (
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
INSERT INTO knowledge_research_stages_v52
    (task_id,stage_key,title,kind,ordinal,status,revision,claim_id,claimed_attempt,
     run_id,content_run_id,summary,content_md,findings_json,evidence_json,error,updated_at)
SELECT task_id,stage_key,title,kind,ordinal,status,revision,claim_id,claimed_attempt,
       run_id,content_run_id,summary,content_md,findings_json,evidence_json,error,updated_at
FROM knowledge_research_stages;

DROP TABLE knowledge_research_stage_versions;
DROP TABLE knowledge_research_stages;
ALTER TABLE knowledge_research_stages_v52 RENAME TO knowledge_research_stages;

CREATE TABLE knowledge_research_stage_versions (
    task_id TEXT NOT NULL,
    stage_key TEXT NOT NULL,
    revision INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(task_id,stage_key,revision),
    FOREIGN KEY(task_id,stage_key) REFERENCES knowledge_research_stages(task_id,stage_key) ON DELETE CASCADE
);
INSERT INTO knowledge_research_stage_versions
    (task_id,stage_key,revision,snapshot_json,created_at)
SELECT task_id,stage_key,revision,snapshot_json,created_at
FROM research_stage_versions_v52_backup;
DROP TABLE research_stage_versions_v52_backup;

CREATE INDEX idx_research_stages_task_order
ON knowledge_research_stages(task_id,ordinal);
