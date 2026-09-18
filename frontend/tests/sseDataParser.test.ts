import assert from 'node:assert/strict'
import test from 'node:test'
import { createSseDataParser } from '../src/utils/sseDataParser.ts'

test('SSE parser emits events before completion even when CRLF splits across reads', () => {
  const events: string[] = []
  const parser = createSseDataParser(data => events.push(data))
  parser.push('data: {"delta":"第一段"}\r')
  assert.deepEqual(events, [])
  parser.push('\n\r\ndata: {"delta":"第二段"}\n\n')
  assert.deepEqual(events, ['{"delta":"第一段"}', '{"delta":"第二段"}'])
  parser.finish()
  assert.equal(events.length, 2)
})

test('SSE parser ignores comments and flushes a final data block without a blank line', () => {
  const events: string[] = []
  const parser = createSseDataParser(data => events.push(data))
  parser.push(': keep-alive\n\ndata: {"type":"completed"}')
  parser.finish()
  assert.deepEqual(events, ['{"type":"completed"}'])
})
