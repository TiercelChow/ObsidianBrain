import assert from 'node:assert/strict'
import test from 'node:test'
import { researchStageStatus, researchFindingStatus, researchCoverage, pickResearchStageToOpen, shouldRefreshResearchStage } from '../src/utils/knowledgeResearch.ts'

test('research coverage counts only saved planned chapters and never implies fact proof', () => {
  assert.deepEqual(researchCoverage([{ kind: 'plan', status: 'completed' }, { kind: 'section', status: 'completed' }, { kind: 'section', status: 'stale' }, { kind: 'synthesis', status: 'completed' }, { kind: 'presentation', status: 'failed' }]), { saved: 1, planned: 2, unfinished: 1 })
  assert.deepEqual(researchCoverage([]), { saved: 0, planned: 0, unfinished: 0 })
  assert.equal(researchStageStatus('stale'), '依据变化 · 待复核')
  assert.equal(researchFindingStatus('supported'), '模型自报支持')
  assert.equal(researchFindingStatus('missing'), '证据缺失')
  assert.equal(researchStageStatus('unknown'), '未记录状态')
})

test('research workspace opens the stage requiring attention before older completed content', () => {
  const plan = { stage_key: 'plan', kind: 'plan', status: 'completed', revision: 1, content_characters: 100 }
  const section = { stage_key: 'section-1', kind: 'section', status: 'completed', revision: 1, content_characters: 200 }
  const running = { stage_key: 'section-2', kind: 'section', status: 'running', revision: 1, content_characters: 0 }
  const failed = { stage_key: 'synthesis', kind: 'synthesis', status: 'failed', revision: 1, content_characters: 0 }
  assert.equal(pickResearchStageToOpen([plan, section, running, failed])?.stage_key, 'section-2')
  assert.equal(pickResearchStageToOpen([plan, section, failed])?.stage_key, 'synthesis')
  assert.equal(pickResearchStageToOpen([plan, section])?.stage_key, 'section-1')
  assert.equal(pickResearchStageToOpen([
    plan,
    { ...section, stage_key: 'report' },
    { ...section, stage_key: 'validation' },
  ])?.stage_key, 'report')
  assert.equal(pickResearchStageToOpen([{ ...running, status: 'pending' }])?.stage_key, 'section-2')
  assert.equal(pickResearchStageToOpen([]), undefined)
})

test('research workspace refreshes current stage changes but preserves an explicitly selected historical revision', () => {
  const oldCurrent = { stage_key: 'section-1', status: 'running', revision: 2, run_id: 'run-1', content_characters: 0 }
  const completed = { ...oldCurrent, status: 'completed', revision: 3, run_id: 'run-2', content_characters: 200 }
  assert.equal(shouldRefreshResearchStage(oldCurrent, [completed], true), true)
  assert.equal(shouldRefreshResearchStage(oldCurrent, [{ ...oldCurrent, status: 'failed' }], true), true)
  assert.equal(shouldRefreshResearchStage(oldCurrent, [oldCurrent], true), false)
  assert.equal(shouldRefreshResearchStage({ ...oldCurrent, revision: 1 }, [completed], false), false)
  assert.equal(shouldRefreshResearchStage(oldCurrent, [], true), false)
})
