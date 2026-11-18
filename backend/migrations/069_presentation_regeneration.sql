ALTER TABLE knowledge_tasks
    ADD COLUMN presentation_only INTEGER NOT NULL DEFAULT 0 CHECK(presentation_only IN (0, 1));
