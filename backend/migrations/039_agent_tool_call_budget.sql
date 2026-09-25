ALTER TABLE agent_run_capabilities
    ADD COLUMN used_calls INTEGER NOT NULL DEFAULT 0;

ALTER TABLE agent_run_capabilities
    ADD COLUMN max_calls INTEGER NOT NULL DEFAULT 64;
