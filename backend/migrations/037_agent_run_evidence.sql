-- A run-scoped record of evidence actually exposed to the model. Search
-- candidates are not evidence until their full entry or source is read.
CREATE TABLE IF NOT EXISTS agent_run_evidence (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id       TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL CHECK (kind IN ('entry', 'source_span', 'external')),
    object_id    TEXT NOT NULL,
    version_id   TEXT NOT NULL,
    snapshot_json TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_run_evidence_run
    ON agent_run_evidence(run_id, kind, created_at);
