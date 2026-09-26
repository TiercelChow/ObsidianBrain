import assert from 'node:assert/strict'
import test from 'node:test'
import { researchStageStatus, researchFindingStatus, researchCoverage } from '../src/utils/knowledgeResearch.ts'

test('research coverage counts only saved planned chapters and never implies fact proof', () => {
  assert.deepEqual(researchCoverage([{ kind: 'plan', status: 'completed' }, { kind: 'section', status: 'completed' }, { kind: 'section', status: 'stale' }, { kind: 'synthesis', status: 'completed' }, { kind: 'presentation', status: 'failed' }]), { saved: 1, planned: 2, unfinished: 1 })
  assert.deepEqual(researchCoverage([]), { saved: 0, planned: 0, unfinished: 0 })
  assert.equal(researchStageStatus('stale'), '依据变化 · 待复核')
  assert.equal(researchFindingStatus('supported'), '模型自报支持')
  assert.equal(researchFindingStatus('missing'), '证据缺失')
  assert.equal(researchStageStatus('unknown'), '未记录状态')
})
