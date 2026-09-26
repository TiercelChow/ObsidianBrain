import assert from 'node:assert/strict'
import test from 'node:test'
import { knowledgeRunDiagnostics } from '../src/utils/knowledgeRunDiagnostics.ts'

test('runtime diagnostics separates cumulative currency cost, context occupancy and completion', () => {
  const diagnostics = knowledgeRunDiagnostics([
    { event_type: 'run.usage_cost', payload: { amount: 0.01, currency: 'USD' } },
    { event_type: 'run.usage', payload: { context_used: 4500, context_size: 100000 } },
    { event_type: 'run.usage_cost', payload: { amount: 0.04, currency: 'USD' } },
    { event_type: 'run.runtime_completed', payload: { stop_reason: 'max_tokens', complete: false } },
  ], { context_window: 100000, effective_max_output_tokens: 16000, reasoning_policy: 'high', timeout_seconds: 600, tool_call_limit: 80 })
  assert.deepEqual(diagnostics.cost, { amount: 0.04, currency: 'USD' })
  assert.deepEqual(diagnostics.context, { used: 4500, size: 100000 })
  assert.deepEqual(diagnostics.completion, { stopReason: 'max_tokens', complete: false })
  assert.equal(diagnostics.budget.context_window, 100000)
})

test('legacy or malformed diagnostics do not claim unknown capacity, completion or a bill', () => {
  const diagnostics = knowledgeRunDiagnostics([{ event_type: 'run.usage_cost', payload: { amount: 'tokens', currency: 1 } }], null)
  assert.equal(diagnostics.cost, null)
  assert.equal(diagnostics.context, null)
  assert.equal(diagnostics.completion, null)
  assert.equal(diagnostics.budget.context_window, null)
  assert.equal(diagnostics.budget.reasoning_policy, 'auto')
})
