CREATE TABLE IF NOT EXISTS task_documents (
    path            TEXT PRIMARY KEY,
    document_kind   TEXT NOT NULL CHECK (document_kind IN ('short_month', 'long_task')),
    root_id         TEXT,
    storage_month   TEXT,
    revision        INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,
    indexed_at      TEXT NOT NULL,
    sync_error      TEXT
);

CREATE TABLE IF NOT EXISTS task_nodes (
    id                      TEXT PRIMARY KEY,
    root_id                 TEXT NOT NULL,
    parent_id               TEXT,
    storage_path            TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    kind                    TEXT NOT NULL CHECK (kind IN ('short', 'long')),
    role                    TEXT NOT NULL CHECK (role IN ('root', 'subtask')),
    title                   TEXT NOT NULL,
    description             TEXT NOT NULL DEFAULT '',
    status                  TEXT NOT NULL,
    importance              TEXT NOT NULL,
    start_date              TEXT NOT NULL,
    end_date                TEXT NOT NULL,
    position                INTEGER NOT NULL,
    closure_note            TEXT,
    closed_at               TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    revision                INTEGER NOT NULL,
    archived_at             TEXT,
    progress_percent        INTEGER NOT NULL DEFAULT 0 CHECK (progress_percent BETWEEN 0 AND 100),
    completed_leaf_count    INTEGER NOT NULL DEFAULT 0,
    effective_leaf_count    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS task_progress (
    id              TEXT PRIMARY KEY,
    root_id         TEXT NOT NULL,
    task_id         TEXT NOT NULL REFERENCES task_nodes(id) ON DELETE CASCADE,
    storage_path    TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    recorded_at     TEXT NOT NULL,
    note            TEXT NOT NULL,
    percent_after   INTEGER CHECK (percent_after BETWEEN 0 AND 100),
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS task_audit_events (
    id              TEXT PRIMARY KEY,
    root_id         TEXT NOT NULL,
    task_id         TEXT NOT NULL,
    storage_path    TEXT NOT NULL REFERENCES task_documents(path) ON DELETE CASCADE,
    event_type      TEXT NOT NULL,
    from_status     TEXT,
    to_status       TEXT,
    note            TEXT,
    occurred_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS task_sync_queue (
    path            TEXT PRIMARY KEY,
    reason          TEXT NOT NULL,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_nodes_status ON task_nodes(status, archived_at);
CREATE INDEX IF NOT EXISTS idx_task_nodes_dates ON task_nodes(start_date, end_date);
CREATE INDEX IF NOT EXISTS idx_task_nodes_kind_importance ON task_nodes(kind, importance);
CREATE INDEX IF NOT EXISTS idx_task_nodes_root_parent ON task_nodes(root_id, parent_id, position);
CREATE INDEX IF NOT EXISTS idx_task_progress_task_time ON task_progress(task_id, recorded_at DESC);
