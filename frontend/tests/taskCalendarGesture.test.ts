import assert from 'node:assert/strict'
import test from 'node:test'

import { claimCalendarSwipe, resolveCalendarSwipe } from '../src/utils/taskCalendarGesture.ts'

test('calendar swipe only claims a gesture after horizontal intent is clear', () => {
  assert.equal(claimCalendarSwipe(7, 1), false)
  assert.equal(claimCalendarSwipe(11, 8), false)
  assert.equal(claimCalendarSwipe(13, 5), true)
  assert.equal(claimCalendarSwipe(-18, 4), true)
})

test('calendar swipe leaves vertical scrolling to the page', () => {
  assert.equal(claimCalendarSwipe(14, 22), false)
  assert.equal(resolveCalendarSwipe({ dx: 14, dy: 22, velocityX: 0.8 }), 0)
})

test('calendar swipe resolves distance or velocity into a month direction', () => {
  assert.equal(resolveCalendarSwipe({ dx: -52, dy: 8, velocityX: -0.15 }), 1)
  assert.equal(resolveCalendarSwipe({ dx: 22, dy: 5, velocityX: 0.5 }), -1)
  assert.equal(resolveCalendarSwipe({ dx: 26, dy: 4, velocityX: 0.12 }), 0)
})
