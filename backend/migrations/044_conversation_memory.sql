-- Explicit conversational intent, separate from messages and factual evidence.
-- Old conversations remain untouched and have no inferred memory row.
CREATE TABLE knowledge_conversation_memories (
    conversation_id TEXT NOT NULL,
    knowledge_base_id TEXT NOT NULL,
    objective TEXT NOT NULL DEFAULT '' CHECK (length(objective) <= 800),
    constraints_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(constraints_json) AND json_type(constraints_json) = 'array'
               AND json_array_length(constraints_json) <= 16),
    unresolved_questions_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(unresolved_questions_json) AND json_type(unresolved_questions_json) = 'array'
               AND json_array_length(unresolved_questions_json) <= 12),
    entity_ids_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(entity_ids_json) AND json_type(entity_ids_json) = 'array'
               AND json_array_length(entity_ids_json) <= 24),
    revision INTEGER NOT NULL CHECK (revision > 0),
    last_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
    last_message_ordinal INTEGER NOT NULL CHECK (last_message_ordinal >= 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (conversation_id, knowledge_base_id),
    FOREIGN KEY (conversation_id, knowledge_base_id)
        REFERENCES knowledge_conversation_scopes(conversation_id, knowledge_base_id) ON DELETE CASCADE
);
