import assert from 'node:assert/strict'
import test from 'node:test'
import { fitPhoto, constrainPhoto, zoomPhoto, createImageViewerMotion, type PhotoFrame } from '../src/utils/imageViewerMotion.ts'

function harness(count = 3, reduced = false) {
  let now = 0, sequence = 0
  const pending = new Map<number, (time: number) => void>()
  const navigated: number[] = []
  let frame: PhotoFrame
  const motion = createImageViewerMotion({
    count: () => count, navigate: direction => navigated.push(direction), change: value => { frame = value },
    clock: { now: () => now, request: callback => { pending.set(++sequence, callback); return sequence }, cancel: id => { pending.delete(id) }, reduced: () => reduced },
  })
  motion.fit(1200, 800, 390, 650)
  return {
    motion, navigated, frame: () => frame!,
    point: (id: number, x: number, y: number, time: number) => ({ id, x, y, time }),
    settle() { for (let i = 0; i < 250 && pending.size; i++) { now += 1000 / 120; const ticks = [...pending.values()]; pending.clear(); ticks.forEach(tick => tick(now)) } assert.equal(pending.size, 0) },
    tick() { now += 1000 / 120; const ticks = [...pending.values()]; pending.clear(); ticks.forEach(tick => tick(now)) },
  }
}

test('photo fits portrait, landscape and unavailable dimensions without distortion', () => {
  assert.deepEqual(fitPhoto(600, 1200, 390, 650), { width: 325, height: 650 })
  assert.deepEqual(fitPhoto(1200, 600, 390, 650), { width: 390, height: 195 })
  assert.deepEqual(fitPhoto(0, 0, 0, 0), { width: 0, height: 0 })
})

test('center anchored zoom does not drift and pointer zoom keeps the same image point', () => {
  const size = { width: 390, height: 260, stageWidth: 390, stageHeight: 650 }
  assert.deepEqual(zoomPhoto({ x: 0, y: 0, scale: 1 }, 2, { x: 0, y: 0 }, size), { x: 0, y: 0, scale: 2 })
  assert.deepEqual(zoomPhoto({ x: 0, y: 0, scale: 1 }, 2, { x: 60, y: 0 }, size), { x: -60, y: 0, scale: 2 })
  assert.equal(zoomPhoto({ x: 0, y: 0, scale: 1 }, .1, { x: 0, y: 0 }, size).scale, 1)
  assert.equal(zoomPhoto({ x: 0, y: 0, scale: 1 }, 50, { x: 0, y: 0 }, size).scale, 8)
})

test('pan bounds center small axes and keep enlarged content in the viewport', () => {
  assert.deepEqual(constrainPhoto({ x: 1000, y: -1000, scale: 2 }, { width: 390, height: 260, stageWidth: 390, stageHeight: 650 }), { x: 195, y: 0, scale: 2 })
})

test('button zoom and reset remain centered through rapid retargeting', () => {
  const h = harness()
  h.motion.zoom(1.25); h.tick(); h.motion.zoom(1.25); h.settle()
  assert.equal(h.frame().scale, 1.5625)
  assert.equal(h.frame().x, 0); assert.equal(h.frame().y, 0)
  h.motion.reset(); h.settle()
  assert.equal(h.frame().scale, 1)
})

test('left and right fit gestures track the pointer and navigate after settling', () => {
  for (const [delta, direction] of [[-120, 1], [120, -1]]) {
    const h = harness()
    h.motion.down(h.point(1, 200, 300, 0))
    h.motion.move(h.point(1, 200 + delta, 302, 120))
    assert.equal(h.frame().slide, delta)
    h.motion.up(h.point(1, 200 + delta, 302, 130))
    h.settle()
    assert.deepEqual(h.navigated, [direction])
    assert.equal(h.frame().slide, 0)
    assert.equal(h.motion.consumeClick(), true)
  }
})

test('short drags, vertical drags and a one-photo gallery return without navigation', () => {
  for (const [count, dx, dy] of [[3, 12, 0], [3, 25, 130], [1, -150, 0]]) {
    const h = harness(count)
    h.motion.down(h.point(1, 100, 200, 0)); h.motion.move(h.point(1, 100 + dx, 200 + dy, 200))
    h.motion.up(h.point(1, 100 + dx, 200 + dy, 500)); h.settle()
    assert.deepEqual(h.navigated, []); assert.equal(h.frame().slide, 0)
  }
})

test('a short fast flick can advance but holding after a drag does not preserve stale velocity', () => {
  for (const [upTime, expected] of [[45, [1]], [500, []]]) {
    const h = harness()
    h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 168, 300, 40))
    h.motion.up(h.point(1, 168, 300, upTime)); h.settle()
    assert.deepEqual(h.navigated, expected)
  }
})

test('zoomed one-finger drags pan rather than changing photos', () => {
  const h = harness()
  h.motion.zoom(3); h.settle()
  h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 80, 310, 100))
  h.motion.up(h.point(1, 80, 310, 120)); h.settle()
  assert.deepEqual(h.navigated, []); assert.equal(h.frame().x, -120)
})

test('zoomed pan edges resist progressively and settle back without leaving a blank viewport', () => {
  const h = harness()
  h.motion.zoom(2); h.settle()
  h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 1200, 300, 100))
  assert.ok(h.frame().x > 195 && h.frame().x < 1000)
  h.motion.up(h.point(1, 1200, 300, 110)); h.settle()
  assert.equal(h.frame().x, 195); assert.deepEqual(h.navigated, [])
})

test('pinch zoom anchors its midpoint and never turns into a swipe when one finger lifts', () => {
  const h = harness()
  h.motion.down(h.point(1, 100, 300, 0)); h.motion.down(h.point(2, 200, 300, 10))
  h.motion.move(h.point(2, 300, 300, 100))
  assert.equal(h.frame().scale, 2)
  h.motion.up(h.point(2, 300, 300, 110)); h.motion.move(h.point(1, 20, 300, 150))
  h.motion.up(h.point(1, 20, 300, 160)); h.settle()
  assert.deepEqual(h.navigated, [])
})

test('cancelled and interrupted swipe springs remain continuous and do not commit the old direction', () => {
  const h = harness()
  h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 90, 300, 100))
  h.motion.up(h.point(1, 90, 300, 110)); h.tick()
  const before = h.frame().slide
  h.motion.down(h.point(2, 100, 300, 120))
  assert.equal(h.frame().slide, before)
  h.motion.cancel(); h.settle()
  assert.deepEqual(h.navigated, []); assert.equal(h.frame().slide, 0)
})

test('reduced motion commits immediately and resize/dispose cancel obsolete navigation', () => {
  const h = harness(3, true)
  h.motion.navigate(1); assert.deepEqual(h.navigated, [1]); assert.equal(h.frame().slide, 0)
  const pending = harness()
  pending.motion.navigate(1); pending.motion.fit(600, 1200, 320, 600); pending.settle()
  assert.deepEqual(pending.navigated, [])
  pending.motion.navigate(-1); pending.motion.dispose(); pending.settle()
  assert.deepEqual(pending.navigated, [])
})

test('ordinary capture release preserves swipe completion but unexpected capture loss cancels it', () => {
  const h = harness()
  h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 80, 300, 100))
  h.motion.up(h.point(1, 80, 300, 110)); h.motion.cancel(1); h.settle()
  assert.deepEqual(h.navigated, [1])
  h.motion.down(h.point(2, 200, 300, 200)); h.motion.move(h.point(2, 80, 300, 300))
  h.motion.cancel(2); h.settle()
  assert.deepEqual(h.navigated, [1]); assert.equal(h.frame().slide, 0)
})

test('an original arriving during a swipe updates photo size without cancelling the gesture or navigation', () => {
  const h = harness()
  h.motion.down(h.point(1, 200, 300, 0)); h.motion.move(h.point(1, 80, 300, 100))
  h.motion.load(600, 1200)
  assert.equal(h.frame().slide, -120)
  h.motion.up(h.point(1, 80, 300, 110)); h.tick()
  h.motion.load(600, 1200); h.settle()
  assert.deepEqual(h.navigated, [1])
})
