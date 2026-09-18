import assert from 'node:assert/strict'
import test from 'node:test'
import { createStreamedTextBuffer } from '../src/utils/streamedText.ts'

function manualFrames(reducedMotion = false) {
  let nextId = 0
  const frames = new Map<number, FrameRequestCallback>()
  return {
    scheduler: {
      request: (callback: FrameRequestCallback) => {
        const id = ++nextId
        frames.set(id, callback)
        return id
      },
      cancel: (id: number) => { frames.delete(id) },
      reducedMotion: () => reducedMotion,
    },
    flushOne: () => {
      const [id, callback] = frames.entries().next().value || []
      if (callback) {
        frames.delete(id)
        callback(0)
      }
    },
    hasFrames: () => frames.size > 0,
  }
}

test('streamed text exposes large late chunks over frames without losing characters', async () => {
  const clock = manualFrames()
  const emitted: string[] = []
  const buffer = createStreamedTextBuffer(chunk => emitted.push(chunk), clock.scheduler)
  const answer = `${'a'.repeat(23)}😀${'中文内容'.repeat(100)}`
  buffer.push(answer)
  const drained = buffer.drain()
  clock.flushOne()
  assert.ok(emitted.join('').length > 0)
  assert.ok(emitted.join('').length < answer.length)
  while (clock.hasFrames()) clock.flushOne()
  await drained
  assert.equal(emitted.join(''), answer)
  assert.ok(emitted.every(chunk => !chunk.endsWith('\uD83D')))
})

test('reduced motion flushes buffered text at once and cancellation stops future writes', async () => {
  const clock = manualFrames(true)
  let output = ''
  const buffer = createStreamedTextBuffer(chunk => { output += chunk }, clock.scheduler)
  buffer.push('完整回答')
  const drained = buffer.drain()
  clock.flushOne()
  await drained
  assert.equal(output, '完整回答')
  buffer.push('不应出现')
  buffer.cancel()
  assert.equal(clock.hasFrames(), false)
  assert.equal(output, '完整回答')
})

test('cancel resolves a pending drain without writing buffered text', async () => {
  const clock = manualFrames()
  let output = ''
  const buffer = createStreamedTextBuffer(chunk => { output += chunk }, clock.scheduler)
  buffer.push('尚未显示的内容')
  const drained = buffer.drain()
  buffer.cancel()
  await drained
  assert.equal(output, '')
  assert.equal(clock.hasFrames(), false)
})
