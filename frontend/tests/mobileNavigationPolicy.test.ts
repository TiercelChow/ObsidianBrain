import assert from 'node:assert/strict'
import test from 'node:test'

import {
  getMobileNavSection,
  getMobileRouteRedirect,
  isMobileFocusRoute,
} from '../src/utils/mobileNavigationPolicy.ts'

test('getMobileNavSection identifies all five mobile destinations and every Wiki page', () => {
  assert.equal(getMobileNavSection('/reader'), 'reader')
  assert.equal(getMobileNavSection('/timeline'), 'timeline')
  assert.equal(getMobileNavSection('/tasks'), 'tasks')
  assert.equal(getMobileNavSection('/'), 'home')
  for (const path of ['/knowledge', '/knowledge/wiki', '/knowledge/chat', '/knowledge/tasks', '/knowledge/settings']) {
    assert.equal(getMobileNavSection(path), 'wiki', path)
  }
  assert.equal(getMobileNavSection('/knowledge-other'), 'home')
  assert.equal(getMobileNavSection('/code-repo'), 'home')
})

test('code repositories are desktop-only, including mobile bookmarks and breakpoint changes', () => {
  assert.equal(getMobileRouteRedirect('/code-repo', 390), '/')
  assert.equal(getMobileRouteRedirect('/code-repo', 768), '/')
  assert.equal(getMobileRouteRedirect('/code-repo', 769), null)
  assert.equal(getMobileRouteRedirect('/knowledge/wiki', 390), null)
  assert.equal(getMobileRouteRedirect('/', 390), null)
  assert.equal(getMobileRouteRedirect('/code-repo', Number.NaN), null)
})

test('isMobileFocusRoute reserves the dock area for reader and task context actions', () => {
  assert.equal(isMobileFocusRoute('/reader', { view: 'read' }), true)
  assert.equal(isMobileFocusRoute('/reader', { view: 'shelf' }), false)
  assert.equal(isMobileFocusRoute('/tasks', { task: 'task-1' }), true)
  assert.equal(isMobileFocusRoute('/tasks', { view: 'calendar', task: 'task-1' }), false)
  assert.equal(isMobileFocusRoute('/tasks', { view: 'calendar' }), false)
  assert.equal(isMobileFocusRoute('/timeline', {}), false)
})
