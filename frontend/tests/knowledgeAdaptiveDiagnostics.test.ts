import assert from 'node:assert/strict'
import test from 'node:test'
import { adaptiveKnowledgeDiagnostics } from '../src/utils/knowledgeAdaptiveDiagnostics.ts'

test('uses live budget snapshot, explicit plan and self-reported coverage, not thought events', () => {
  const result = adaptiveKnowledgeDiagnostics({
    qa_plan: { goal: '全书比较', subquestions: ['机制', '边界'], depth: 'detailed' },
    planning_stats: { catalog_seen: 50, catalog_total: 100, stop_reason: 'planning_budget_reached' },
    adaptive_state: { used_tool_calls: 18, soft_tool_calls: 30, policy: { hard_tool_calls: 80 }, coverage: [{ question_index: 0, question: '机制', status: 'supported', citation_indices: [3], finding: '已读机制正文' }] },
  }, [{ event_type: 'run.budget_changed', message: '初始预算', payload: { used_tool_calls: 0, soft_tool_calls: 10 } }, { event_type: 'agent.thought', message: '秘密推理', payload: {} }])
  assert.equal(result.plan.goal, '全书比较')
  assert.equal(result.budget.used, 18)
  assert.equal(result.budget.soft, 30)
  assert.equal(result.coverage[0].citationIndices[0], 3)
  assert.equal(result.planning.seen, 50)
  assert.equal(result.expansions.length, 0)
  assert.equal(JSON.stringify(result).includes('秘密推理'), false)
})

test('legacy fields are unknown, budget extension reason and latest coverage remain explicit', () => {
  const result = adaptiveKnowledgeDiagnostics({}, [
    { event_type: 'run.budget_changed', message: '缺少条件', payload: { reason: '缺少条件', budget: { used_tool_calls: 4, soft_tool_calls: 12, policy: { hard_tool_calls: 40 } } } },
    { event_type: 'run.evidence_coverage', message: '', payload: { coverage: { question_index: 0, question: '条件', status: 'missing', citation_indices: [], finding: '无来源' }, agent_reported: true } },
  ])
  assert.equal(result.budget.hard, 40)
  assert.equal(result.planning.total, null)
  assert.equal(result.coverage[0].status, 'missing')
  assert.deepEqual(result.expansions, ['缺少条件'])
})

test('research diagnostics retain phase scope and distinguish capacity guard from model capability', () => {
  const result = adaptiveKnowledgeDiagnostics({
    research_stage_key: 'section:boundaries',
    research_plan: { goal: '比较机制与边界', constraints: ['不新增书外事实'], depth: 'deep' },
    research_question: { title: '边界与反例', question: '哪些条件下不成立？', required_evidence: ['反例与适用条件'] },
    research_resources: { capacity_tokens: 32768, capacity_basis: 'unknown_model_application_guard', initial_entry_target: 14, policy: { timeout_seconds: 480 } },
    adaptive_state: { used_tool_calls: 5, soft_tool_calls: 18, policy: { hard_tool_calls: 72 } },
  }, [])
  assert.equal(result.available, true)
  assert.equal(result.plan.goal, '比较机制与边界')
  assert.equal(result.research.stageKey, 'section:boundaries')
  assert.equal(result.research.question, '哪些条件下不成立？')
  assert.deepEqual(result.research.requirements, ['反例与适用条件'])
  assert.equal(result.research.capacityKnown, false)
  assert.equal(result.research.capacity, 32768)
  assert.equal(result.research.initialTarget, 14)
  assert.equal(result.budget.used, 5)
  assert.equal(result.planning.seen, null, 'seed target is not actual catalog coverage')
  assert.equal(adaptiveKnowledgeDiagnostics({ research_resources: { capacity_tokens: 1000000, capacity_basis: 'configured_model_capacity' } }, []).research.capacityKnown, true)
})

test('presentation diagnostics expose whole-report projection without fabricating usage or coverage', () => {
  const result = adaptiveKnowledgeDiagnostics({ research_stage_key: 'presentation', presentation_materialization: { mode: 'whole_structure_projection', report_characters: 95000, section_count: 10, omitted_characters: 71000, estimated_material_tokens: 8000, billing_usage: false } }, [])
  assert.equal(result.presentation.mode, 'whole_structure_projection')
  assert.equal(result.presentation.sections, 10)
  assert.equal(result.presentation.omittedCharacters, 71000)
  assert.equal(result.presentation.materialTokens, 8000)
  assert.equal(result.budget.tokens, null, 'projection estimate is not tool consumption or model billing')
  assert.equal(result.research.capacityKnown, null)
  assert.equal(adaptiveKnowledgeDiagnostics({}, []).presentation.mode, '')
})

test('research output headroom is estimated and expansion events do not reset evidence budget state', () => {
  const result = adaptiveKnowledgeDiagnostics({
    research_resources: { capacity_tokens: 65536, capacity_basis: 'observed_runtime_capacity', content_output_tokens: 3000, structure_output_tokens: 2560, reasoning_output_tokens: 8192, output_tokens: 27504 },
  }, [
    { event_type: 'run.budget_changed', payload: { used_tool_calls: 5, soft_tool_calls: 20, policy: { hard_tool_calls: 80 } } },
    { event_type: 'run.output_budget_expanded', message: '输出截断，扩大当前阶段', payload: { previous_output_tokens: 13752, next_output_tokens: 27504 } },
  ])
  assert.equal(result.research.capacityKnown, true)
  assert.equal(result.research.contentTokens, 3000)
  assert.equal(result.research.structureTokens, 2560)
  assert.equal(result.research.reasoningTokens, 8192)
  assert.equal(result.research.outputTokens, 27504)
  assert.equal(result.budget.used, 5)
  assert.deepEqual(result.expansions, ['输出截断，扩大当前阶段'])
  const legacy = adaptiveKnowledgeDiagnostics({}, [])
  assert.equal(legacy.research.reasoningTokens, null)
})

test('projected synthesis diagnostics show returned matrix pages without claiming full verification', () => {
  const result = adaptiveKnowledgeDiagnostics({
    research_stage_key: 'synthesis',
    research_plan: { goal: '综合多个主题' },
    research_manifest_projected: true,
  }, [
    { event_type: 'run.research_manifest_page', payload: { question_id: 'a', offset_chars: 0, returned_chars: 1200 } },
    { event_type: 'run.research_manifest_page', payload: { question_id: 'a', offset_chars: 1200, returned_chars: 900 } },
  ])
  assert.equal(result.research.manifestProjected, true)
  assert.equal(result.research.manifestPages, 2)
  assert.equal(adaptiveKnowledgeDiagnostics({}, []).research.manifestProjected, false)
})

test('projected synthesis distinguishes complete topic reads from gaps and mixed versions', () => {
  const refs = {
    research_stage_key: 'synthesis',
    research_plan: { goal: '跨主题综合' },
    research_manifest_projected: true,
    research_manifest_topics: [
      { question_id: 'mechanism', title: '机制' },
      { question_id: 'boundary', title: '边界' },
    ],
  }
  const pages = [
    { event_type: 'run.research_manifest_page', payload: { question_id: 'mechanism', offset_chars: 0, returned_chars: 5, total_chars: 10, manifest_hash: 'v1' } },
    { event_type: 'run.research_manifest_page', payload: { question_id: 'mechanism', offset_chars: 5, returned_chars: 5, total_chars: 10, manifest_hash: 'v1' } },
    { event_type: 'run.research_manifest_page', payload: { question_id: 'boundary', offset_chars: 0, returned_chars: 5, total_chars: 10, manifest_hash: 'v1' } },
    { event_type: 'run.research_manifest_page', payload: { question_id: 'boundary', offset_chars: 5, returned_chars: 5, total_chars: 10, manifest_hash: 'v2' } },
  ]
  const partial = adaptiveKnowledgeDiagnostics(refs, pages)
  assert.equal(partial.research.manifestReadCount, 1)
  assert.equal(partial.research.manifestTopicCount, 2)
  assert.deepEqual(partial.research.manifestTopics.map(item => [item.id, item.complete]), [
    ['mechanism', true], ['boundary', false],
  ])
  const global = adaptiveKnowledgeDiagnostics(refs, [
    ...pages,
    { event_type: 'run.research_manifest_page', payload: { question_id: null, offset_chars: 0, returned_chars: 6, total_chars: 10, manifest_hash: 'global' } },
    { event_type: 'run.research_manifest_page', payload: { question_id: null, offset_chars: 6, returned_chars: 4, total_chars: 10, manifest_hash: 'global' } },
  ])
  assert.equal(global.research.manifestReadCount, 2)
  assert.ok(global.research.manifestTopics.every(item => item.complete))
})
