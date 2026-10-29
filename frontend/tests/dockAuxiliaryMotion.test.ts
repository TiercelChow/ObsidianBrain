import assert from 'node:assert/strict'
import test from 'node:test'
import { auxiliaryDockLayout, stepAuxiliaryDock } from '../src/utils/dockAuxiliaryMotion.ts'

test('the circle keeps its existing anchor and the expanded bar keeps its original dock geometry', () => {
  const circle = auxiliaryDockLayout(374, 'action', 0)
  assert.deepEqual(circle, { x: 310, y: -74, width: 56, height: 56, barOpacity: 0, actionOpacity: 1 })
  const bar = auxiliaryDockLayout(374, 'subnav', 0)
  assert.deepEqual(bar, { x: 12, y: -70, width: 350, height: 50, barOpacity: 1, actionOpacity: 0 })
  const compact = auxiliaryDockLayout(374, 'subnav', 100)
  assert.deepEqual(compact, { x: 0, y: 0, width: 304, height: 62, barOpacity: 1, actionOpacity: 0 })
  assert.equal(auxiliaryDockLayout(350, 'action', 0, { actionInset: 0 }).x, 294)
})

test('every morph frame stays finite and keeps its circle-to-pill contour and clear foreground', () => {
  for (const width of [304, 374, 651]) {
    const circle = auxiliaryDockLayout(width, 'action', 0)
    const bar = auxiliaryDockLayout(width, 'subnav', 0)
    let state = { frame: circle, velocity: {} }
    for (let i = 0; i < 180; i++) {
      state = stepAuxiliaryDock(state, bar, 1 / 120)
      for (const value of Object.values(state.frame)) assert.ok(Number.isFinite(value))
      assert.ok(state.frame.width >= 56 && state.frame.width <= bar.width + .01)
      assert.ok(state.frame.height >= 49.99 && state.frame.height <= 56)
      assert.ok(state.frame.barOpacity + state.frame.actionOpacity > .99)
    }
    assert.ok(Math.abs(state.frame.width - bar.width) < .01)
  }
})

test('reversal inherits the current geometry and velocity instead of restarting at an endpoint', () => {
  const circle = auxiliaryDockLayout(374, 'action', 0), bar = auxiliaryDockLayout(374, 'subnav', 0)
  let state = { frame: circle, velocity: {} }
  for (let i = 0; i < 10; i++) state = stepAuxiliaryDock(state, bar, 1 / 120)
  const before = state.frame.width
  const reversed = stepAuxiliaryDock(state, circle, 1 / 120)
  assert.ok(before > circle.width && before < bar.width)
  assert.ok(Math.abs(reversed.frame.width - before) < 16)
  assert.ok(reversed.frame.width > before, 'existing forward velocity is handed off')
  for (let i = 0; i < 180; i++) state = stepAuxiliaryDock(state, circle, 1 / 120)
  assert.ok(Math.abs(state.frame.width - circle.width) < .01)
})

test('30/60/120Hz settle at the same geometry within an elegant but responsive interval', () => {
  const target = auxiliaryDockLayout(374, 'subnav', 0)
  for (const fps of [30, 60, 120]) {
    let state = { frame: auxiliaryDockLayout(374, 'action', 0), velocity: {} }
    let enteredAt = 0
    for (let i = 0; i < fps; i++) {
      state = stepAuxiliaryDock(state, target, 1 / fps)
      if (!enteredAt && state.frame.width >= 56 + (target.width - 56) * .95) enteredAt = (i + 1) / fps
    }
    assert.ok(enteredAt >= .2 && enteredAt <= .5)
    assert.ok(Math.abs(state.frame.width - target.width) < .1)
  }
})

test('responsive and malformed dimensions cannot create negative surfaces', () => {
  for (const width of [0, 30, Number.NaN, Number.POSITIVE_INFINITY]) {
    const frame = auxiliaryDockLayout(width, 'action', Number.NaN)
    for (const value of Object.values(frame)) assert.ok(Number.isFinite(value))
    assert.ok(frame.width >= 0 && frame.height >= 44)
  }
})
