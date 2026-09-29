import assert from 'node:assert/strict'
import test from 'node:test'

import { interruptedKnowledgeAnswer, knowledgeRuntimeMode, labelForRuntimePhase, researchTaskCapacityFailure, researchTaskQuestionFromPlanning, researchTaskQuestionFromRun, researchTaskRouteFromQuestion, researchTaskTurnLimitFailure, validateModelCapabilities } from '../src/utils/knowledgeRuntimePolicy.ts'

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

test('a Harness turn limit is not mistaken for an output-token retry', () => {
  const interrupted = interruptedKnowledgeAnswer('已有分析', '已有分析', new Error('DeepSeek Harness 达到请求轮次上限 (stop_reason=max_turn_requests)'))
  assert.equal(interrupted.kind, 'turn_limit')
  assert.equal(interrupted.canRetry, false)
  assert.equal(interrupted.content, '已有分析')
  assert.match(interrupted.notice, /研究任务/)
  assert.doesNotMatch(interrupted.notice, /提高输出/)
  assert.deepEqual(researchTaskRouteFromQuestion('book-a', ' 分析机制与边界 '), {
    path: '/knowledge/tasks',
    query: { create: '1', base: 'book-a', title: '分析机制与边界', description: '' },
  })
})

test('a clearly infeasible answer plan offers staged research instead of an output retry', () => {
  const interrupted = interruptedKnowledgeAnswer('', '', new Error('(qa_output_scope_limit) 规划篇幅明显超过模型单次输出容量'))
  assert.equal(interrupted.kind, 'scope_limit')
  assert.equal(interrupted.canRetry, false)
  assert.match(interrupted.notice, /分阶段研究任务/)
})

test('research transfer resolves a contextual follow-up using the matching answer run', () => {
  const run = {
    knowledge_base_id: 'book-a',
    task_type: 'knowledge_qa',
    input: { question: '那第二点呢？', standalone_question: '《示例书》第二章提出的第二种机制有哪些适用边界？' },
  }
  const resolved = researchTaskQuestionFromRun('那第二点呢？', 'book-a', run)
  assert.equal(resolved.question, run.input.standalone_question)
  assert.equal(resolved.contextRecovered, true)
  assert.equal(researchTaskQuestionFromRun('那第二点呢？', 'book-b', run).contextRecovered, false)
  assert.equal(researchTaskQuestionFromRun('另一个问题', 'book-a', run).question, '另一个问题')
  assert.equal(researchTaskQuestionFromRun('那第二点呢？', 'book-a', { ...run, input: { ...run.input, standalone_question: '' } }).contextRecovered, false)
  assert.equal(researchTaskQuestionFromRun('那第二点呢？', 'book-a', { ...run, task_type: 'knowledge_task_research' }).contextRecovered, false)
})

test('a planner-only handoff uses its standalone question only for the same book and turn', () => {
  const planning = {
    knowledge_base_id: 'book-a',
    question: '那第二点呢？',
    standalone_question: '《示例书》第二章的第二种机制有哪些适用边界？',
  }
  assert.deepEqual(researchTaskQuestionFromPlanning('那第二点呢？', 'book-a', planning), {
    question: planning.standalone_question,
    contextRecovered: true,
  })
  assert.equal(researchTaskQuestionFromPlanning('那第二点呢？', 'book-b', planning).contextRecovered, false)
  assert.equal(researchTaskQuestionFromPlanning('另一个问题', 'book-a', planning).contextRecovered, false)
  assert.equal(researchTaskQuestionFromPlanning('那第二点呢？', 'book-a', { ...planning, standalone_question: '' }).contextRecovered, false)
})

test('research transfer retains the full resolved question in its editable description', () => {
  const question = '这本书的论证边界与证据链是什么？'.repeat(120)
  const route = researchTaskRouteFromQuestion('book-a', question)
  assert.equal(route.query.title.length, 200)
  assert.equal(route.query.description, question)
})

test('only a failed task whose failure header reached the Harness turn limit gets narrowing advice', () => {
  assert.equal(researchTaskTurnLimitFailure({ status: 'failed', result_summary: '执行未完成：(research_turn_limit) 当前阶段达到轮次上限 (stop_reason=max_turn_requests)' }), true)
  assert.equal(researchTaskTurnLimitFailure({ status: 'failed', result_summary: '执行未完成：(presentation_turn_limit) 演示策划轮次耗尽，完整报告已保留' }), true)
  assert.equal(researchTaskTurnLimitFailure({ status: 'failed', result_summary: '> [!warning] 后续交付未完成\n> 原因：(research_turn_limit) 轮次上限\n\n报告正文' }), true)
  assert.equal(researchTaskTurnLimitFailure({ status: 'completed', result_summary: '执行未完成：(research_turn_limit)' }), false)
  assert.equal(researchTaskTurnLimitFailure({ status: 'failed', result_summary: '执行未完成：输出上限 (stop_reason=max_tokens)' }), false)
  assert.equal(researchTaskTurnLimitFailure({ status: 'failed', result_summary: '执行未完成：普通错误\n\n报告提及 max_turn_requests' }), false)
})

test('only a failed task with a capacity failure header gets model-setting advice', () => {
  assert.equal(researchTaskCapacityFailure({ status: 'failed', result_summary: '执行未完成：(research_section_output_hard_limit) 当前模型单次输出不足' }), true)
  assert.equal(researchTaskCapacityFailure({ status: 'failed', result_summary: '> [!warning] 后续交付未完成\n> 原因：(research_synthesis_output_hard_limit) 综合容量不足\n\n完整报告' }), true)
  assert.equal(researchTaskCapacityFailure({ status: 'failed', result_summary: '执行未完成：(presentation_output_retry_exhausted) 演示策划无法完成' }), true)
  assert.equal(researchTaskCapacityFailure({ status: 'completed', result_summary: '(research_input_hard_limit)' }), false)
  assert.equal(researchTaskCapacityFailure({ status: 'failed', result_summary: '执行未完成：普通错误\n\n报告提及 (research_input_hard_limit)' }), false)
  assert.equal(researchTaskCapacityFailure({ status: 'failed', result_summary: '执行未完成：(research_turn_limit) 请求轮次不足' }), false)
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
