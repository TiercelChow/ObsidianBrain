import assert from 'node:assert/strict'
import test from 'node:test'
import { advanceDockGesture, dockReleaseTarget, boundedScrollTop, createDockGesture } from '../src/utils/mobileDockMotion.ts'

test('finger down contracts and finger up expands the dock continuously', () => {
  let state = advanceDockGesture(createDockGesture(0), -36)
  assert.ok(state.progress > 0 && state.progress < 100)
  state = advanceDockGesture(state, -160)
  assert.equal(state.progress, 100)
  state = advanceDockGesture(state, 42)
  assert.ok(state.progress < 100 && state.progress > 0)
  assert.equal(advanceDockGesture(state, 180).progress, 0)
})

test('tiny changes and direction jitter do not toggle the dock', () => {
  let state = createDockGesture(0)
  for (const delta of [-2, 2, -1, 1]) state = advanceDockGesture(state, delta)
  assert.equal(state.progress, 0)
  state = advanceDockGesture(state, -60)
  const progress = state.progress
  state = advanceDockGesture(state, 2)
  assert.equal(state.progress, progress)
})

test('release takes current position and velocity into account', () => {
  assert.equal(dockReleaseTarget(20, 0), 0)
  assert.equal(dockReleaseTarget(80, 0), 100)
  assert.equal(dockReleaseTarget(40, 300), 100)
  assert.equal(dockReleaseTarget(60, -300), 0)
})

test('overscroll and invalid values cannot produce dock progress outside bounds', () => {
  assert.equal(boundedScrollTop(-30, 400, 100), 0)
  assert.equal(boundedScrollTop(500, 400, 100), 300)
  assert.equal(boundedScrollTop(20, 80, 100), 0)
  assert.equal(boundedScrollTop(Number.NaN, 400, 100), 0)
  assert.deepEqual(advanceDockGesture(createDockGesture(20), Number.NaN), createDockGesture(20))
})
