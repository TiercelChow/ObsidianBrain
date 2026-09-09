import assert from 'node:assert/strict'
import test from 'node:test'

import { projectMotion, stepSpring } from '../src/utils/motionSpring.ts'

test('projectMotion follows the release direction', () => {
  assert.ok(projectMotion(50, 600) > 50)
  assert.ok(projectMotion(50, -600) < 50)
})

test('stepSpring approaches a resting target without an instantaneous jump', () => {
  let state = { value: 120, velocity: 0 }
  const first = stepSpring(state, 0, 1 / 60)
  assert.ok(first.value < 120)
  assert.ok(first.value > 0)

  state = first
  for (let frame = 0; frame < 180; frame += 1) {
    state = stepSpring(state, 0, 1 / 60)
  }
  assert.ok(Math.abs(state.value) < 0.1)
  assert.ok(Math.abs(state.velocity) < 0.5)
})
