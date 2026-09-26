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
