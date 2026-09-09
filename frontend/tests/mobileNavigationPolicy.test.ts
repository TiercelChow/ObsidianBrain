import assert from 'node:assert/strict'
import test from 'node:test'

import {
  getMobileNavSection,
  isMobileFocusRoute,
} from '../src/utils/mobileNavigationPolicy.ts'

test('getMobileNavSection keeps the three daily modules direct and groups the rest', () => {
  assert.equal(getMobileNavSection('/reader'), 'reader')
  assert.equal(getMobileNavSection('/timeline'), 'timeline')
  assert.equal(getMobileNavSection('/tasks'), 'tasks')
  assert.equal(getMobileNavSection('/memory'), 'more')
  assert.equal(getMobileNavSection('/'), 'more')
})

test('isMobileFocusRoute reserves the dock area for reader and task context actions', () => {
  assert.equal(isMobileFocusRoute('/reader', { view: 'read' }), true)
  assert.equal(isMobileFocusRoute('/reader', { view: 'shelf' }), false)
  assert.equal(isMobileFocusRoute('/tasks', { task: 'task-1' }), true)
  assert.equal(isMobileFocusRoute('/tasks', { view: 'calendar', task: 'task-1' }), false)
  assert.equal(isMobileFocusRoute('/tasks', { view: 'calendar' }), false)
  assert.equal(isMobileFocusRoute('/timeline', {}), false)
})
