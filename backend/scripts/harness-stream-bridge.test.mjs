import { test } from 'node:test'
import assert from 'node:assert/strict'
import { installBridge } from '../plugins/harness-stream-bridge.mjs'

function setup() {
  const hooks = new Map(), frames = []
  installBridge({ on: (name, fn) => hooks.set(name, fn) }, frame => frames.push(frame))
  return { hooks, frames, events: () => frames.map(f => f.params._meta['obsidianbrain/stream']) }
}
function frames(s, session, id, chunks) {
  const hook = s.hooks.get('agent/assistant-stream'), agent = { session: { id: session } }
  hook({ agent, frame: { type: 'start', attemptId: id } })
  for (const chunk of chunks) hook({ agent, frame: { type: 'chunk', attemptId: id, chunk } })
}

test('forwards real agent deltas and never sends reasoning text', () => {
  const s = setup()
  const values = [{ type: 'reasoning-delta', text: 'private' }, { type: 'text-delta', text: '你' }, { type: 'text-delta', text: '好' }, { type: 'finish', reason: { kind: 'stop' } }]
  frames(s, 's', 's:1', values)
  assert.deepEqual(s.events().map(e => e.op), ['begin', 'thinking', 'append', 'append', 'finish'])
  assert.equal(JSON.stringify(s.frames).includes('private'), false)
  assert.equal(s.hooks.has('llm/stream'), false)
  assert.equal(s.frames.length, 5)
})

test('tool rounds and retries have separate identities; commit uses durable text only', () => {
  const s = setup()
  frames(s, 'a', 'a:1', [{ type: 'text-delta', text: '查找中' }, { type: 'finish', reason: { kind: 'tool-calls' } }])
  s.hooks.get('session/event')({ header: { id: 'a' } }, { type: 'assistant/message', data: { message: { content: [{ type: 'text', text: '查找中' }, { type: 'tool-call' }] } } })
  frames(s, 'a', 'a:2', [{ type: 'text-delta', text: '回答' }])
  s.hooks.get('session/event')({ header: { id: 'b' } }, { type: 'assistant/message', data: { message: { content: [{ type: 'text', text: '别的会话' }] } } })
  s.hooks.get('session/event')({ header: { id: 'a' } }, { type: 'assistant/message', data: { message: { content: [{ type: 'reasoning', text: 'private' }, { type: 'text', text: '校正回答' }] } } })
  const events = s.events()
  assert.notEqual(events[0].call_id, events[4].call_id)
  assert.equal(events[3].tool_round, true)
  assert.equal(events.at(-1).text, '校正回答')
  assert.equal(events.some(e => e.text === '别的会话'), false)
  assert.equal(JSON.stringify(s.frames).includes('private'), false)
})
