ALTER TABLE llm_provider_profiles ADD COLUMN new_credential_source TEXT NOT NULL DEFAULT 'local_file'
    CHECK (new_credential_source IN ('local_file', 'environment'));

UPDATE llm_provider_profiles
SET new_credential_source = CASE credential_source
        WHEN 'environment' THEN 'environment'
        ELSE 'local_file'
    END,
    api_key_configured = CASE credential_source WHEN 'keychain' THEN 0 ELSE api_key_configured END,
    api_key_env = CASE credential_source WHEN 'keychain' THEN '' ELSE api_key_env END,
    revision = revision + CASE credential_source WHEN 'keychain' THEN 1 ELSE 0 END;

ALTER TABLE llm_provider_profiles DROP COLUMN credential_source;
ALTER TABLE llm_provider_profiles RENAME COLUMN new_credential_source TO credential_source;
