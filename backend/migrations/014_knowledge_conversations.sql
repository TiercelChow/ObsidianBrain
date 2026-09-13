CREATE TABLE IF NOT EXISTS knowledge_conversations (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_conversation_scopes (
    conversation_id    TEXT NOT NULL REFERENCES knowledge_conversations(id) ON DELETE CASCADE,
    knowledge_base_id  TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    ordinal            INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (conversation_id, knowledge_base_id)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_conversation_scopes_base
    ON knowledge_conversation_scopes(knowledge_base_id, ordinal, conversation_id);

CREATE TABLE IF NOT EXISTS knowledge_messages (
    id               TEXT PRIMARY KEY,
    conversation_id  TEXT NOT NULL REFERENCES knowledge_conversations(id) ON DELETE CASCADE,
    ordinal          INTEGER NOT NULL,
    role             TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content          TEXT NOT NULL,
    run_id           TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_messages_conversation
    ON knowledge_messages(conversation_id, ordinal);

CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_messages_ordinal
    ON knowledge_messages(conversation_id, ordinal);

CREATE TABLE IF NOT EXISTS knowledge_message_citations (
    message_id  TEXT NOT NULL REFERENCES knowledge_messages(id) ON DELETE CASCADE,
    ordinal     INTEGER NOT NULL,
    entry_id    TEXT NOT NULL REFERENCES knowledge_entries(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, ordinal),
    UNIQUE (message_id, entry_id)
);
