ALTER TABLE knowledge_tasks ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE knowledge_tasks ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 3;
ALTER TABLE knowledge_tasks ADD COLUMN next_attempt_at TEXT;
ALTER TABLE knowledge_tasks ADD COLUMN lease_expires_at TEXT;
ALTER TABLE knowledge_tasks ADD COLUMN last_heartbeat_at TEXT;

CREATE INDEX IF NOT EXISTS idx_knowledge_tasks_dispatch
    ON knowledge_tasks(status, next_attempt_at, updated_at);

CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_tasks_one_running_per_base
    ON knowledge_tasks(knowledge_base_id)
    WHERE status = 'running';
