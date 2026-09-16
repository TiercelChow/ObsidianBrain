ALTER TABLE knowledge_compile_checkpoints
    ADD COLUMN compile_fingerprint TEXT NOT NULL DEFAULT '';

CREATE TABLE knowledge_base_skill_bindings_v2 (
    knowledge_base_id   TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    skill_id            TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    usage_scope         TEXT NOT NULL DEFAULT 'both'
                            CHECK (usage_scope IN ('qa', 'research', 'both', 'ingest', 'all')),
    enabled             INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    revision            INTEGER NOT NULL DEFAULT 1,
    updated_at          TEXT NOT NULL,
    PRIMARY KEY (knowledge_base_id, skill_id)
);

INSERT INTO knowledge_base_skill_bindings_v2
    (knowledge_base_id, skill_id, usage_scope, enabled, revision, updated_at)
SELECT knowledge_base_id, skill_id, usage_scope, enabled, revision, updated_at
FROM knowledge_base_skill_bindings;

DROP TABLE knowledge_base_skill_bindings;
ALTER TABLE knowledge_base_skill_bindings_v2 RENAME TO knowledge_base_skill_bindings;

CREATE INDEX idx_skill_bindings_base
    ON knowledge_base_skill_bindings(knowledge_base_id, enabled, usage_scope);
