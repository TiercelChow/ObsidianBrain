-- Logical citations are immutable run snapshots, independent of live entities.
-- Existing runs remain explicitly legacy: do not fabricate their visible text.
CREATE TABLE agent_run_citations (
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    citation_index INTEGER NOT NULL CHECK (citation_index > 0),
    kind TEXT NOT NULL CHECK (kind IN ('entry', 'source_span', 'external')),
    object_id TEXT NOT NULL,
    version_id TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (run_id, citation_index),
    UNIQUE (run_id, kind, object_id, version_id)
);
