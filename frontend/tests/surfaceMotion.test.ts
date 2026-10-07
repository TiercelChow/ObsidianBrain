import assert from 'node:assert/strict'
import test from 'node:test'
import { effectScope, nextTick, ref } from 'vue'
import { useSurfaceMotion } from '../src/composables/useSurfaceMotion.ts'

test('sheet lifetime covers exit, rapid reversal, gesture handoff and disposal', async () => {
  const previousWindow = globalThis.window
  const previousRequest = globalThis.requestAnimationFrame
  const previousCancel = globalThis.cancelAnimationFrame
  const previousPerformance = globalThis.performance
  let now = 0, nextId = 0
  const frames = new Map<number, FrameRequestCallback>()
  const fakeWindow = { innerWidth: 390, innerHeight: 844, matchMedia: (query: string) => ({matches:query.includes('max-width')}) }
  Object.assign(globalThis, {
    window: fakeWindow,
    performance: { now: () => now },
    requestAnimationFrame: function(this: unknown, callback: FrameRequestCallback) {
      assert.equal(this, undefined, 'browser frame function must not be rebound to a clock object')
      frames.set(++nextId, callback); return nextId
    },
    cancelAnimationFrame: function(this: unknown, id: number) { assert.equal(this, undefined); frames.delete(id) },
  })
  const tick = () => {
    now += 1000/60
    const pending = [...frames.values()]; frames.clear()
    pending.forEach(callback => callback(now))
  }
  const scope = effectScope()
  try {
    const open = ref(false)
    const panel = ref({offsetWidth:390,offsetHeight:400} as HTMLElement)
    const motion = scope.run(() => useSurfaceMotion(() => open.value, panel, 'y'))!
    open.value = true
    await nextTick(); await nextTick()
    assert.equal(motion.present.value, true)
    assert.equal(motion.offset.value, 424)
    for (let index=0;index<100;index++) tick()
    assert.equal(motion.offset.value, 0)
    open.value = false
    await nextTick()
    tick(); tick(); tick()
    assert.equal(motion.present.value, true, 'background stays locked until exit completes')
    const inFlight = motion.offset.value
    open.value = true
    await nextTick()
    assert.equal(motion.offset.value, inFlight, 'reopening preserves the same on-screen surface')
    assert.equal(frames.size, 1)
    for (let index=0;index<100;index++) tick()
    assert.equal(motion.offset.value, 0)
    motion.grab(); motion.follow(160); motion.handoff(600)
    open.value = false
    await nextTick()
    tick()
    assert.ok(motion.offset.value > 160)
    for (let index=0;index<100;index++) tick()
    assert.equal(motion.present.value, false)
    open.value = true
    await nextTick(); await nextTick()
    scope.stop()
    assert.equal(frames.size, 0)
  } finally {
    scope.stop()
    Object.assign(globalThis, {window:previousWindow,requestAnimationFrame:previousRequest,cancelAnimationFrame:previousCancel,performance:previousPerformance})
  }
})
