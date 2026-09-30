import assert from 'node:assert/strict'
import test from 'node:test'

import { visibleResearchBriefFields } from '../src/utils/researchBriefChoices.ts'

test('preflight initially shows only decisions that need user attention', () => {
  const preflight = { focus_decisions: ['presentation_theme', 'purpose', 'tone'] as const }
  assert.deepEqual(
    visibleResearchBriefFields(preflight, 'presentation', false),
    ['purpose', 'tone', 'presentation_theme'],
  )
  assert.deepEqual(
    visibleResearchBriefFields(preflight, 'presentation', true),
    ['audience', 'purpose', 'tone', 'depth', 'presentation_theme', 'presentation_format'],
  )
})

test('report and manual fallback never expose irrelevant or hidden required choices', () => {
  assert.deepEqual(
    visibleResearchBriefFields({ focus_decisions: ['purpose', 'presentation_theme'] }, 'report', false),
    ['purpose'],
  )
  assert.deepEqual(
    visibleResearchBriefFields(null, 'report', false),
    ['audience', 'purpose', 'tone', 'depth'],
  )
  assert.deepEqual(
    visibleResearchBriefFields({ focus_decisions: [] }, 'presentation', false),
    ['audience', 'purpose', 'tone', 'depth', 'presentation_theme', 'presentation_format'],
  )
})
