-- Server-owned authority for status-only historical archives. Agent-generated
-- candidates/audit fields cannot opt out of current-evidence validation.
CREATE TABLE IF NOT EXISTS knowledge_historical_archive_authorizations (
    change_set_id TEXT PRIMARY KEY REFERENCES knowledge_change_sets(id) ON DELETE CASCADE,
    knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    entry_id TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),
    before_hash TEXT NOT NULL,
    after_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);
