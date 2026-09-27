import assert from 'node:assert/strict'
import test from 'node:test'
import { advanceDockGesture, dockReleaseTarget, boundedScrollTop, createDockGesture, stepDockSpring, dockGeometry, dockRebound } from '../src/utils/mobileDockMotion.ts'

test('finger up contracts and finger down expands the dock continuously', () => {
  let state = advanceDockGesture(createDockGesture(0), 36)
  assert.ok(state.progress > 0 && state.progress < 100)
  state = advanceDockGesture(state, 160)
  assert.equal(state.progress, 100)
  state = advanceDockGesture(state, -42)
  assert.ok(state.progress < 100 && state.progress > 0)
  assert.equal(advanceDockGesture(state, -180).progress, 0)
})

test('tiny changes and direction jitter do not toggle the dock', () => {
  let state = createDockGesture(0)
  for (const delta of [-2, 2, -1, 1]) state = advanceDockGesture(state, delta)
  assert.equal(state.progress, 0)
  state = advanceDockGesture(state, 60)
  const progress = state.progress
  state = advanceDockGesture(state, -2)
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

test('dock motion is quicker but still shows the retracting surface instead of snapping', () => {
  let state = { value: 0, velocity: 0 }
  for (let frame = 0; frame < 6; frame++) state = stepDockSpring(state, 100, 1 / 60)
  assert.ok(state.value > 25 && state.value < 40, `100ms progress ${state.value}`)
  for (let frame = 0; frame < 12; frame++) state = stepDockSpring(state, 100, 1 / 60)
  assert.ok(state.value > 80 && state.value < 95, `300ms progress ${state.value}`)
  for (let frame = 0; frame < 120; frame++) state = stepDockSpring(state, 100, 1 / 60)
  assert.ok(Math.abs(state.value - 100) < .01)
})

test('the dock spring preserves velocity when retargeted mid-animation', () => {
  const current = { value: 40, velocity: 150 }
  const next = stepDockSpring(current, 0, 1 / 120)
  assert.ok(next.velocity > 0 && next.velocity < current.velocity, 'reversal decelerates rather than cutting velocity')
  assert.ok(next.value > current.value && next.value - current.value < 2)
  let state = next
  for (let frame = 0; frame < 180; frame++) {
    state = stepDockSpring(state, 0, 1 / 60)
    assert.ok(state.value >= -3 && state.value <= 103)
  }
  assert.ok(Math.abs(state.value) < .01)
})

test('dock motion has a bounded presentation speed even after a flick', () => {
  const current = { value: 30, velocity: 800 }
  const next = stepDockSpring(current, 100, 1 / 60)
  assert.ok(next.velocity <= 300)
  assert.ok(next.value - current.value <= 300 / 60 + 1e-10)
})

test('narrowing, height and curved travel share one continuous progress without phases', () => {
  assert.deepEqual(dockGeometry(0), { shape: 0, sink: 0, travelX: 0, travelY: 0 })
  assert.deepEqual(dockGeometry(100), { shape: 1, sink: 1, travelX: 1, travelY: 1 })
  let previous = dockGeometry(0)
  for (let progress = 1; progress <= 100; progress++) {
    const geometry = dockGeometry(progress)
    assert.ok(geometry.shape >= geometry.sink, 'retraction leads the arc without a holding phase')
    for (const key of ['shape', 'sink', 'travelX', 'travelY'] as const) {
      assert.ok(geometry[key] > previous[key] && geometry[key] <= 1, `${key} never holds mid-flight`)
      assert.ok(geometry[key] - previous[key] < .04, `${key} has no path discontinuity`)
    }
    if (progress < 100) assert.ok(geometry.travelX > geometry.travelY, 'the path bends around the upper-left of the orb')
    previous = geometry
  }
})

test('the arc clears a continuously visible retracting capsule at every frame', () => {
  for (const width of [304, 374, 651]) {
    for (let progress = 0; progress <= 100; progress += .5) {
      const { shape, sink, travelX, travelY } = dockGeometry(progress)
      const left = 12 * (1 - travelX)
      const right = width - 12 - 58 * travelX
      const height = 50 + 12 * sink
      const centerY = -95 + 64 * travelY
      // Distance between the core rectangles of two rounded glass surfaces.
      const primaryRadius = 20 + 11 * shape
      const subRadius = 18 + 2 * sink
      const primaryLeft = (width - 62) * shape
      const subBottom = centerY + height / 2
      const dx = Math.max(primaryLeft + primaryRadius - (right - subRadius), 0)
      const dy = Math.max(-62 + primaryRadius - (subBottom - subRadius), 0)
      const gap = Math.hypot(dx, dy) - primaryRadius - subRadius
      assert.ok(gap > 1, `persistent rounded surfaces keep a gap at ${width}px / ${progress}%: ${gap}`)
      assert.ok((right - left - 2 * (2 + 2 * sink) - 2) / 5 >= 44, 'five subnav targets stay usable')
    }
  }
})

test('the primary dock visibly retracts through capsule widths before becoming a circle', () => {
  const width = 374
  const widths = [0, 10, 25, 50, 75, 100].map(progress => width - (width - 62) * dockGeometry(progress).shape)
  assert.equal(widths[0], width)
  assert.equal(widths.at(-1), 62)
  assert.ok(widths[1] > 240 && widths[2] > 150 && widths[3] > 75, 'intermediate frames are still capsules')
  for (let index = 1; index < widths.length; index++) assert.ok(widths[index] < widths[index - 1])
})

test('endpoint springs gently overshoot and return in both directions', () => {
  for (const target of [0, 100]) {
    let state = { value: 100 - target, velocity: 0 }
    const samples: number[] = []
    for (let frame = 0; frame < 150; frame++) {
      state = stepDockSpring(state, target, 1 / 60)
      samples.push(target === 100 ? state.value - target : target - state.value)
    }
    const peak = Math.max(...samples)
    assert.ok(peak > .3 && peak < 3, `a restrained but visible rebound: ${peak}`)
    assert.ok(Math.abs(state.value - target) < .01)
    assert.ok(Math.abs(state.velocity) < .01)
  }
})

test('following an intermediate scroll position does not oscillate around it', () => {
  let state = { value: 0, velocity: 0 }
  for (let frame = 0; frame < 150; frame++) {
    state = stepDockSpring(state, 45, 1 / 60)
    assert.ok(state.value >= 0 && state.value <= 45)
  }
  assert.ok(Math.abs(state.value - 45) < .01)
})

test('rebound is soft and bounded without changing the final navigation geometry', () => {
  for (const progress of [0, 20, 72, 100, Number.NaN, Infinity]) assert.equal(dockRebound(progress), 0)
  assert.ok(dockRebound(100.5) > 0)
  assert.equal(dockRebound(-.5), dockRebound(100.5))
  assert.ok(dockRebound(103) > dockRebound(100.5))
  assert.ok(dockRebound(1e6) <= 1)
  assert.deepEqual(dockGeometry(-3), dockGeometry(0))
  assert.deepEqual(dockGeometry(103), dockGeometry(100))
})

test('a reverse gesture can interrupt an endpoint rebound without a presentation jump', () => {
  const current = { value: 100.4, velocity: 6 }
  const next = stepDockSpring(current, 0, 1 / 120)
  assert.ok(Math.abs(next.value - current.value) < 2)
  assert.ok(next.velocity < 0)
  assert.equal(createDockGesture(current.value).progress, 100)
  let state = next
  for (let frame = 0; frame < 180; frame++) state = stepDockSpring(state, 0, 1 / 60)
  assert.ok(Math.abs(state.value) < .01)
})

test('endpoint elasticity stays stable across display refresh rates and long frames', () => {
  const peaks: number[] = []
  for (const fps of [30, 60, 120]) {
    let state = { value: 0, velocity: 0 }
    let peak = 0
    for (let frame = 0; frame < fps * 3; frame++) {
      state = stepDockSpring(state, 100, 1 / fps)
      peak = Math.max(peak, state.value)
      assert.ok(Math.abs(state.velocity) <= 300)
    }
    assert.ok(peak > 100.3 && peak < 103, `stable ${fps}Hz peak: ${peak}`)
    peaks.push(peak)
    assert.ok(Math.abs(state.value - 100) < .01)
  }
  assert.ok(Math.max(...peaks) - Math.min(...peaks) < .1, 'low frame rates retain the same elasticity')
  const current = { value: 30, velocity: 100 }
  assert.deepEqual(stepDockSpring(current, 100, 1), stepDockSpring(current, 100, 1 / 30))
})
