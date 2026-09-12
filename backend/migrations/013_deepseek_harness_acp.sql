UPDATE agent_runtime_profiles
SET executable = 'npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp',
    revision = revision + 1,
    updated_at = CURRENT_TIMESTAMP
WHERE id = 'runtime-deepseek-harness'
  AND executable IN ('deepseek-harness', 'dsh');
