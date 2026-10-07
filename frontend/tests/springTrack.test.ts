import assert from 'node:assert/strict'
import test from 'node:test'
import { createSpringTrack } from '../src/utils/springTrack.ts'

function fixture(reduced = false) {
  let now = 0
  let nextId = 0
  const frames = new Map<number, (time: number) => void>()
  const updates: number[] = []
  const track = createSpringTrack(value => updates.push(value), {
    now: () => now,
    request: callback => { frames.set(++nextId, callback); return nextId },
    cancel: id => { frames.delete(id) },
    reduced: () => reduced,
  })
  return { track, updates, frames, tick: () => {
    now += 1000 / 60
    const pending = [...frames.values()]
    frames.clear()
    pending.forEach(callback => callback(now))
  } }
}

test('retargeting preserves presentation and velocity with only one scheduled frame', () => {
  const { track, tick, frames } = fixture()
  track.jump(400)
  track.to(0)
  tick(); tick(); tick()
  const before = track.state()
  let staleCompletion = false
  track.to(400, () => { staleCompletion = true })
  assert.deepEqual(track.state(), before)
  assert.equal(frames.size, 1)
  track.to(0)
  for (let index = 0; index < 100; index++) tick()
  assert.equal(track.state().value, 0)
  assert.equal(staleCompletion, false)
  assert.equal(frames.size, 0)
})

test('gesture takeover cancels automatic frames and releases with its actual velocity', () => {
  const { track, tick, frames } = fixture()
  track.jump(300)
  track.to(0)
  tick()
  track.pause()
  const current = track.state().value
  assert.equal(frames.size, 0)
  track.jump(current + 20, 600)
  track.to(400)
  tick()
  assert.ok(track.state().value > current + 20)
  track.dispose()
  assert.equal(frames.size, 0)
})

test('reduced motion reaches the target synchronously without frames', () => {
  const { track, frames } = fixture(true)
  let completed = 0
  track.jump(400)
  track.to(0, () => completed++)
  assert.deepEqual(track.state(), { value: 0, velocity: 0 })
  assert.equal(completed, 1)
  assert.equal(frames.size, 0)
})
