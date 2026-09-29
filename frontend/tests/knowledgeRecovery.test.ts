import assert from 'node:assert/strict'
import test from 'node:test'

import { interruptedKnowledgeAnswer, knowledgeRuntimeMode, labelForRuntimePhase, validateModelCapabilities } from '../src/utils/knowledgeRuntimePolicy.ts'

test('planning fallback remains visible as a retrieval adjustment', () => {
  assert.equal(labelForRuntimePhase('目录规划未完成，正在改用书内检索继续核查'), '正在调整检索方式')
  assert.equal(labelForRuntimePhase('目录规划结果无效，正在改用书内检索继续核查'), '正在调整检索方式')
})

test('an interrupted provider-batched answer retains all received text, not only displayed frames', () => {
  const recovered = interruptedKnowledgeAnswer('已显示', '已显示但还有待显示的正文', new Error('max_tokens'))
  assert.equal(recovered.content, '已显示但还有待显示的正文')
  assert.equal(recovered.kind, 'truncated')
  assert.equal(recovered.canRetry, true)
  assert.match(recovered.notice, /不完整/)
})

test('a confirmed output hard limit keeps the draft but does not offer a same-budget retry', () => {
  const interrupted = interruptedKnowledgeAnswer('未完成正文', '未完成正文', new Error('(qa_output_hard_limit) 输出已达到单次有效上限 (stop_reason=max_tokens)'))
  assert.equal(interrupted.kind, 'truncated')
  assert.equal(interrupted.content, '未完成正文')
  assert.equal(interrupted.canRetry, false)
  assert.match(interrupted.notice, /调整.*最大输出|调整.*推理策略/)
})

test('an empty max-token answer explains retry exhaustion without pretending a draft exists', () => {
  const interrupted = interruptedKnowledgeAnswer('', '', new Error('DeepSeek Harness 达到输出 token 上限且未返回正文 (stop_reason=max_tokens)'))
  assert.equal(interrupted.content, '')
  assert.equal(interrupted.kind, 'truncated')
  assert.match(interrupted.notice, /没有收到正文/)
  assert.doesNotMatch(interrupted.notice, /继续完成完整答案/)
})

test('a transient failure never switches the next available Runtime request into evidence-only mode', () => {
  const failed = interruptedKnowledgeAnswer('', '', new Error('连接超时'))
  assert.equal(failed.kind, 'failed')
  assert.equal(knowledgeRuntimeMode(true), 'runtime')
  assert.equal(knowledgeRuntimeMode(false), 'evidence_only')
  assert.match(failed.notice, /再次尝试/)
})

test('cancellation and denied credentials are explicit and do not invent answer content', () => {
  const cancelled = interruptedKnowledgeAnswer('', '', new DOMException('已停止', 'AbortError'))
  const denied = interruptedKnowledgeAnswer('部分内容', '部分内容', new Error('Platform secure storage failure: User canceled the operation'))
  assert.equal(cancelled.kind, 'cancelled')
  assert.equal(cancelled.content, '')
  assert.equal(denied.kind, 'credentials')
  assert.equal(denied.content, '部分内容')
  assert.match(denied.notice, /凭据/)
})

test('model capabilities preserve unknown capacity and reject invalid or input-starving limits', () => {
  assert.equal(validateModelCapabilities({ context_window: null, max_output_tokens: null, reasoning_policy: 'auto' }), '')
  assert.equal(validateModelCapabilities({ context_window: 1000000, max_output_tokens: 64000, reasoning_policy: 'high' }), '')
  assert.match(validateModelCapabilities({ context_window: 1000, max_output_tokens: 1000, reasoning_policy: 'auto' }), /小于/)
  for (const invalid of [0, -1, 1.5, Number.NaN, 4294967296]) {
    assert.match(validateModelCapabilities({ context_window: invalid, max_output_tokens: null, reasoning_policy: 'auto' }), /整数/)
  }
  assert.match(validateModelCapabilities({ context_window: null, max_output_tokens: null, reasoning_policy: 'bogus' }), /推理/)
})
