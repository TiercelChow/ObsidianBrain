import assert from 'node:assert/strict'
import test from 'node:test'
import { knowledgeStatusLabel, needsSourceReview, sourceImpactLabel } from '../src/utils/knowledgeSourceImpacts.ts'

test('source expiry remains a review requirement, not a declaration that a claim is false', () => {
  assert.equal(needsSourceReview({ status: 'stale' }), true)
  assert.equal(needsSourceReview({ status: 'verified', source_impact_count: 3 }), true)
  assert.equal(needsSourceReview({ status: 'verified' }), false)
  assert.equal(knowledgeStatusLabel('stale'), '来源需复核')
  assert.equal(sourceImpactLabel('source_changed'), '原文已更新')
  assert.equal(sourceImpactLabel('source_missing'), '原文缺失或不可读')
  assert.equal(sourceImpactLabel('source_reindexed'), '来源定位已更新')
})
