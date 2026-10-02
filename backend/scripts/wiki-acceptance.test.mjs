import test from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import { mkdtemp, readFile, readdir, realpath, rm, symlink } from 'node:fs/promises'
import os from 'node:os'
import {
  acceptanceBookId,
  assertIsolatedTarget,
  assertLocalTarget,
  assertTaskScope,
  canonicalHealth,
  compilationNeedsPolling,
  parseSseEvents,
  selectQaCase,
  summarizeStream,
  taskNeedsPolling,
} from './wiki-acceptance.mjs'

const workspace = '/tmp/obsidianbrain-wiki-acceptance.example'
const book = { id: acceptanceBookId(workspace), path: path.join(workspace, 'sources') }
const health = { vault: { path: path.join(workspace, 'vault') }, components: { obsidian: 'disabled' } }

test('task observation is read-only and limited to an explicitly selected task in the isolated book', () => {
  const task = { id: 'owned-task', knowledge_base_id: 'test-base', status: 'running' }
  assert.doesNotThrow(() => assertTaskScope(task, 'test-base', 'owned-task'))
  for (const value of [undefined, { ...task, id: 'other' }, { ...task, knowledge_base_id: 'personal' }]) {
    assert.throws(() => assertTaskScope(value, 'test-base', 'owned-task'), /隔离知识库/)
  }
  assert.equal(taskNeedsPolling(task), true)
  assert.equal(taskNeedsPolling({ status: 'queued' }), true)
  for (const status of ['draft', 'completed', 'failed', 'cancelled']) assert.equal(taskNeedsPolling({ status }), false)
})

test('compile observation stops at human review without approving or looping forever', () => {
  assert.equal(compilationNeedsPolling({ compile_state: 'compiling', compile_phase: 'request_sent' }), true)
  assert.equal(compilationNeedsPolling({ compile_state: 'compiling', compile_phase: 'waiting_review' }), false)
  assert.equal(compilationNeedsPolling({ compile_state: 'failed', compile_phase: 'failed' }), false)
  assert.equal(compilationNeedsPolling({ compile_state: 'ready', compile_phase: 'idle' }), false)
})

test('only the exact isolated loopback origin is accepted', () => {
  assert.equal(assertLocalTarget('http://127.0.0.1:19988'), 'http://127.0.0.1:19988')
  for (const url of ['http://127.0.0.1:9876', 'http://localhost:19988', 'https://127.0.0.1:19988', 'http://127.0.0.1:19988/v1', 'http://u:p@127.0.0.1:19988', 'http://127.0.0.1:19988?x=1']) {
    assert.throws(() => assertLocalTarget(url), /隔离/)
  }
})

test('mutations require an isolated vault and an empty or owned bookshelf', () => {
  assert.doesNotThrow(() => assertIsolatedTarget(health, [], workspace))
  assert.doesNotThrow(() => assertIsolatedTarget(health, [book], workspace))
  assert.throws(() => assertIsolatedTarget({ ...health, vault: { path: '/Users/example/vault' } }, [], workspace), /Vault/)
  assert.throws(() => assertIsolatedTarget({ ...health, components: { obsidian: 'ok' } }, [], workspace), /Obsidian/)
  assert.throws(() => assertIsolatedTarget(health, [{ id: 'personal-book', path: '/personal' }], workspace), /书架/)
  assert.throws(() => assertIsolatedTarget(health, [{ ...book, path: '/personal' }], workspace), /书架/)
  assert.throws(() => assertIsolatedTarget(health, [book, book], workspace), /书架/)
  assert.throws(() => assertIsolatedTarget(health, [], '/Users/example/project'), /临时目录/)
})

test('health paths are canonicalized so macOS temporary directory aliases match safely', async t => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'obsidianbrain-wiki-acceptance.test.'))
  t.after(() => rm(temporary, { recursive: true, force: true }))
  const alias = path.join(temporary, 'alias')
  await symlink(temporary, alias)
  const snapshot = { vault: { path: alias }, components: { obsidian: 'disabled' } }
  assert.equal((await canonicalHealth(snapshot)).vault.path, await realpath(temporary))
  assert.equal(snapshot.vault.path, alias)
})

test('SSE parsing preserves Chinese deltas across buffers, CRLF and keepalives', () => {
  const parser = parseSseEvents()
  const event = JSON.stringify({ type: 'text_delta', delta: '中文输出' })
  const bytes = new TextEncoder().encode(': keep-alive\r\n\r\ndata: ' + event + '\r\n\r\n')
  const split = bytes.indexOf(0xe4) + 1
  const decoder = new TextDecoder()
  assert.deepEqual(parser.push(decoder.decode(bytes.slice(0, split), { stream: true })), [])
  assert.deepEqual(parser.push(decoder.decode(bytes.slice(split))), [{ type: 'text_delta', delta: '中文输出' }])
  assert.deepEqual(parser.push('data: {"type":"completed",\ndata: "result":{"answer":"完成"}}\n\n'), [{ type: 'completed', result: { answer: '完成' } }])
  assert.deepEqual(parser.finish(), [])
})

test('SSE EOF is flushed, malformed events are reported rather than silently dropped', () => {
  const parser = parseSseEvents()
  parser.push('data: {"type":"error","message":"缺凭据"}')
  assert.deepEqual(parser.finish(), [{ type: 'error', message: '缺凭据' }])
  assert.throws(() => parseSseEvents().push('data: invalid\n\n'), /JSON/)
})

test('stream metrics distinguish first status, first content and complete delivery', () => {
  const summary = summarizeStream([
    { elapsed_ms: 3, event: { type: 'phase', message: '检索中' } },
    { elapsed_ms: 10, event: { type: 'text_delta', delta: '' } },
    { elapsed_ms: 22, event: { type: 'text_delta', delta: '正文' } },
    { elapsed_ms: 30, event: { type: 'text_delta', delta: '继续' } },
    { elapsed_ms: 50, event: { type: 'completed', result: { answer: '正文继续', run_id: 'run', conversation_id: 'conversation' } } },
  ])
  assert.equal(summary.first_status_ms, 3)
  assert.equal(summary.first_content_ms, 22)
  assert.equal(summary.delta_count, 2)
  assert.equal(summary.duration_ms, 50)
  assert.equal(summary.delivery, 'incremental')
  assert.equal(summary.stream_matches_final, true)
  assert.equal(summary.evidence_before_content_end, false)
  assert.equal(summary.semantic_review, 'not_reviewed')
  assert.equal(summarizeStream([{ elapsed_ms: 2, event: { type: 'completed', result: { answer: '全部' } } }]).delivery, 'completion_only')
  assert.equal(summarizeStream([{ elapsed_ms: 2, event: { type: 'text_delta', delta: '部分' } }]).completed, false)
})

test('acceptance does not report a successful stream after a terminal transport error', () => {
  const result = summarizeStream([
    { elapsed_ms: 1, event: { type: 'evidence', evidence: [] } },
    { elapsed_ms: 2, event: { type: 'text_delta', delta: '不完整' } },
    { elapsed_ms: 3, event: { type: 'completed', result: { answer: '不完整正文' } } },
    { elapsed_ms: 4, event: { type: 'error', message: 'JSON parse failed' } },
  ])
  assert.equal(result.completed, false)
  assert.equal(result.stream_matches_final, false)
  assert.equal(result.evidence_before_content_end, true)
})

test('only explicit QA cases can make model calls; corpus count and anchors are frozen', async () => {
  const fixture = new URL('../../docs/superpowers/fixtures/wiki-acceptance/', import.meta.url)
  const manifest = JSON.parse(await readFile(new URL('cases.json', fixture), 'utf8'))
  const files = (await readdir(new URL('sources/', fixture))).filter(name => name.endsWith('.md'))
  assert.equal(files.length, manifest.source_count)
  assert.equal(manifest.fictional, true)
  assert.equal(selectQaCase(manifest, 'Q3').turns.length, 3)
  assert.throws(() => selectQaCase(manifest, 'R1'), /问答/)
  assert.throws(() => selectQaCase(manifest, 'not-found'), /问答/)
  assert.equal(new Set(manifest.cases.map(item => item.id)).size, manifest.cases.length)
  for (const item of manifest.cases) {
    for (const source of item.sources ?? []) assert.ok(files.includes(source))
    assert.ok(item.expected.length > 0)
  }
})
