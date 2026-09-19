ALTER TABLE knowledge_artifacts
    ADD COLUMN validation_details_json TEXT NOT NULL DEFAULT '{}';
