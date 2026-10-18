import assert from 'node:assert/strict'
import test from 'node:test'
import { createDateWheelMotion } from '../src/utils/dateWheelMotion.ts'

function fixture(count = 31, reduced = false) {
  let now = 0, next = 0
  const frames = new Map<number, (time: number) => void>()
  const motion = createDateWheelMotion({ count: () => count, change: () => {}, clock: {
    now: () => now, reduced: () => reduced,
    request: callback => { frames.set(++next, callback); return next },
    cancel: id => { frames.delete(id) },
  } })
  const tick = (times = 1) => { for (let i = 0; i < times; i++) {
    now += 1000 / 60
    const pending = [...frames.values()]; frames.clear(); pending.forEach(callback => callback(now))
  } }
  return { motion, frames, tick }
}

test('wheel follows the finger and hands off release velocity before damping to a row', () => {
  const { motion, tick } = fixture()
  motion.sync(10)
  motion.down({ id: 1, y: 200, time: 0 })
  motion.move({ id: 1, y: 156, time: 44 })
  assert.equal(motion.state().position, 484)
  motion.move({ id: 1, y: 112, time: 88 })
  const released = motion.state().position
  motion.up({ id: 1, y: 112, time: 90 })
  assert.equal(motion.state().position, released, 'release never jumps to the target')
  tick()
  assert.ok(motion.state().position > released, 'momentum continues in the release direction')
  tick(100)
  assert.equal(motion.state().position % 44, 0)
  assert.equal(motion.state().moving, false)
})

test('edges progressively resist overscroll and return without changing the boundary selection', () => {
  const { motion, tick } = fixture(12)
  motion.sync(0)
  motion.down({ id: 1, y: 100, time: 0 })
  motion.move({ id: 1, y: 200, time: 50 })
  const first = Math.abs(motion.state().position)
  motion.move({ id: 1, y: 300, time: 100 })
  const second = Math.abs(motion.state().position)
  assert.ok(first > 0 && first < 100)
  assert.ok(second > first && second - first < first, 'resistance grows further beyond the edge')
  assert.equal(motion.state().index, 0)
  motion.up({ id: 1, y: 300, time: 110 }); tick(120)
  assert.equal(motion.state().position, 0)
})

test('a moving wheel can be grabbed and reversed without teleporting or firing an old target', () => {
  const { motion, frames, tick } = fixture()
  motion.sync(10); motion.select(20); tick(4)
  const before = motion.state().position
  motion.down({ id: 1, y: 150, time: 100 })
  assert.equal(motion.state().position, before)
  assert.equal(frames.size, 0)
  motion.move({ id: 1, y: 194, time: 150 })
  assert.equal(motion.state().position, before - 44)
  motion.up({ id: 1, y: 194, time: 155 }); tick(120)
  assert.ok(motion.state().index < 20)
})

test('grabbing a rebounding edge preserves its presentation before further resistance', () => {
  const { motion, tick } = fixture(12)
  motion.sync(0); motion.down({ id: 1, y: 100, time: 0 })
  motion.move({ id: 1, y: 230, time: 60 }); motion.up({ id: 1, y: 230, time: 70 }); tick()
  const before = motion.state().position
  assert.ok(before < 0)
  motion.down({ id: 2, y: 100, time: 100 }); motion.move({ id: 2, y: 100, time: 101 })
  assert.ok(Math.abs(motion.state().position - before) < .0001)
  motion.cancel(2); tick(120)
  assert.equal(motion.state().position, 0)
})

test('paused gestures do not retain old flick velocity and cancelled pointers snap safely', () => {
  const { motion, tick } = fixture()
  motion.sync(10); motion.down({ id: 1, y: 100, time: 0 })
  motion.move({ id: 1, y: 56, time: 20 })
  motion.up({ id: 1, y: 56, time: 500 }); tick(120)
  assert.equal(motion.state().index, 11)
  motion.down({ id: 1, y: 100, time: 600 }); motion.move({ id: 1, y: 80, time: 620 })
  motion.cancel(2)
  assert.equal(motion.state().dragging, true, 'a second touch must not cancel the active pointer')
  motion.cancel(1); tick(120)
  assert.equal(motion.state().position % 44, 0)
})

test('confirm freezes the live visible date and external date changes cancel stale momentum', () => {
  const { motion, frames, tick } = fixture()
  motion.sync(5); motion.select(15); tick(3)
  const nearest = Math.round(motion.state().position / 44)
  motion.settle()
  assert.equal(motion.state().index, nearest)
  assert.equal(frames.size, 0)
  motion.select(20); tick(2); motion.sync(3); tick(100)
  assert.equal(motion.state().index, 3)
  motion.dispose(); assert.equal(frames.size, 0)
})

test('reduced motion and wheel input reach valid selections without lingering animations', () => {
  const { motion, frames } = fixture(12, true)
  motion.sync(0); motion.wheel(18); motion.wheel(18)
  assert.equal(motion.state().index, 1, 'small trackpad deltas accumulate')
  motion.select(11); assert.equal(motion.state().index, 11)
  motion.wheel(9999); assert.equal(motion.state().index, 11)
  assert.equal(frames.size, 0)
})

test('rapid keyboard steps accumulate destinations while earlier steps are still moving', () => {
  const { motion, tick } = fixture()
  motion.sync(2)
  motion.select(motion.targetIndex() + 1)
  motion.select(motion.targetIndex() + 1)
  motion.select(motion.targetIndex() + 1)
  tick(100)
  assert.equal(motion.state().index, 5)
})
