import assert from 'node:assert/strict'
import test from 'node:test'
import { buildModelProviderPayload } from '../src/utils/modelProviderPayload.ts'

const provider = { display_name: 'test', api_protocol: 'openai-completions' as const, base_url: 'https://example.test/v1', model: 'model', credential_source: 'environment' as const }

test('provider save payload forwards capacity and reasoning declarations', () => {
  const payload = buildModelProviderPayload({ ...provider, context_window: 1000000, max_output_tokens: 64000, reasoning_policy: 'high' })
  assert.equal(payload.context_window, 1000000)
  assert.equal(payload.max_output_tokens, 64000)
  assert.equal(payload.reasoning_policy, 'high')
})

test('provider save payload sends null and auto to explicitly clear prior declarations', () => {
  const payload = buildModelProviderPayload({ ...provider, context_window: null, max_output_tokens: null, reasoning_policy: 'auto' })
  assert.ok(Object.hasOwn(payload, 'context_window'))
  assert.ok(Object.hasOwn(payload, 'max_output_tokens'))
  assert.equal(payload.context_window, null)
  assert.equal(payload.max_output_tokens, null)
  assert.equal(payload.reasoning_policy, 'auto')
  assert.ok(!Object.hasOwn(payload, 'api_key'))
})
