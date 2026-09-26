import type { SaveModelProviderRequest } from '../api/knowledge'

export function buildModelProviderPayload(provider: SaveModelProviderRequest): Record<string, unknown> {
  const payload: Record<string, unknown> = {
    display_name: provider.display_name,
    api_protocol: provider.api_protocol,
    base_url: provider.base_url,
    model: provider.model,
    credential_source: provider.credential_source,
    enabled: provider.enabled ?? true,
    expected_revision: provider.expected_revision ?? 0,
  }
  // Null is intentional: the user cleared a previously declared capacity.
  if (provider.context_window !== undefined) payload.context_window = provider.context_window
  if (provider.max_output_tokens !== undefined) payload.max_output_tokens = provider.max_output_tokens
  if (provider.reasoning_policy !== undefined) payload.reasoning_policy = provider.reasoning_policy
  if (provider.provider_id) payload.provider_id = provider.provider_id
  if (provider.api_key_env) payload.api_key_env = provider.api_key_env
  if (provider.api_key) payload.api_key = provider.api_key
  if (provider.clear_api_key) payload.clear_api_key = provider.clear_api_key
  return payload
}
