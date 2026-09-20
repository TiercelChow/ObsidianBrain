CREATE TABLE IF NOT EXISTS llm_provider_profiles (
    id                   TEXT PRIMARY KEY,
    display_name         TEXT NOT NULL,
    api_protocol         TEXT NOT NULL CHECK (api_protocol IN (
                             'openai-completions', 'openai-responses', 'anthropic-messages'
                         )),
    base_url             TEXT NOT NULL,
    model                TEXT NOT NULL,
    credential_source    TEXT NOT NULL CHECK (credential_source IN ('keychain', 'environment')),
    api_key_env          TEXT NOT NULL DEFAULT '',
    api_key_configured   INTEGER NOT NULL DEFAULT 0,
    enabled              INTEGER NOT NULL DEFAULT 1,
    revision             INTEGER NOT NULL DEFAULT 1,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

ALTER TABLE agent_runtime_profiles
    ADD COLUMN provider_id TEXT REFERENCES llm_provider_profiles(id) ON DELETE RESTRICT;

INSERT OR IGNORE INTO llm_provider_profiles (
    id, display_name, api_protocol, base_url, model,
    credential_source, api_key_env, api_key_configured, enabled,
    created_at, updated_at
)
SELECT
    json_extract(config_json, '$.provider.provider_id'),
    json_extract(config_json, '$.provider.display_name'),
    json_extract(config_json, '$.provider.api_protocol'),
    json_extract(config_json, '$.provider.base_url'),
    model,
    'environment',
    json_extract(config_json, '$.provider.api_key_env'),
    0,
    1,
    created_at,
    updated_at
FROM agent_runtime_profiles
WHERE json_type(config_json, '$.provider') = 'object'
  AND COALESCE(json_extract(config_json, '$.provider.provider_id'), '') <> '';

UPDATE agent_runtime_profiles
SET provider_id = json_extract(config_json, '$.provider.provider_id'),
    config_json = '{}'
WHERE json_type(config_json, '$.provider') = 'object'
  AND COALESCE(json_extract(config_json, '$.provider.provider_id'), '') <> '';

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_provider_profiles_name
    ON llm_provider_profiles(display_name COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_llm_provider_profiles_enabled
    ON llm_provider_profiles(enabled, updated_at DESC);
