import assert from 'node:assert/strict'
import test from 'node:test'

import { pickActiveDate, type SpyHeader } from '../src/utils/timelineSpy.ts'

test('pickActiveDate returns the last header that crossed the threshold', () => {
  const headers: SpyHeader[] = [
    { date: '2026-08-23', top: -40 },
    { date: '2026-08-22', top: 30 },
    { date: '2026-08-21', top: 88 },
    { date: '2026-08-20', top: 91 },
    { date: '2026-08-19', top: 400 },
  ]
  assert.equal(pickActiveDate(headers, 90), '2026-08-21')
})

test('pickActiveDate falls back to the first header when none crossed the threshold', () => {
  const headers: SpyHeader[] = [
    { date: '2026-08-23', top: 120 },
    { date: '2026-08-22', top: 300 },
  ]
  assert.equal(pickActiveDate(headers, 90), '2026-08-23')
})

test('pickActiveDate returns null for an empty header list', () => {
  assert.equal(pickActiveDate([], 90), null)
})
