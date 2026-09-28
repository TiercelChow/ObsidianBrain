import assert from 'node:assert/strict'
import test from 'node:test'
import { researchReadiness } from '../src/utils/researchReadiness.ts'

const preparedBase = {
  lifecycle: 'active' as const,
  source_available: true,
  sync_state: 'clean' as const,
  compile_mode: 'smart' as const,
  compile_state: 'ready' as const,
  compile_phase: '',
  source_count: 12,
  entry_count: 34,
  pending_review_count: 0,
}

test('research readiness reports usable material without claiming answer coverage', () => {
  const readiness = researchReadiness(preparedBase)
  assert.equal(readiness.status, 'ready')
  assert.equal(readiness.canCreate, true)
  assert.match(readiness.summary, /已同步.*智能 Wiki/)
  assert.ok(readiness.notes.some(note => note.includes('不保证足以回答')))
})

test('research readiness blocks inactive bases before paid preflight or creation', () => {
  for (const lifecycle of ['paused', 'archived', 'uninitialized'] as const) {
    const readiness = researchReadiness({ ...preparedBase, lifecycle })
    assert.equal(readiness.status, 'blocked')
    assert.equal(readiness.canCreate, false)
    assert.ok(readiness.notes.some(note => note.includes(lifecycle === 'uninitialized' ? '启用' : '恢复')))
  }
})

test('research readiness warns about missing original sources but permits historical research', () => {
  const readiness = researchReadiness({ ...preparedBase, source_available: false })
  assert.equal(readiness.status, 'attention')
  assert.equal(readiness.canCreate, true)
  assert.ok(readiness.notes.some(note => note.includes('原书目录不可用')))
})

test('research readiness distinguishes chapter fallback and pending review from published semantic knowledge', () => {
  const readiness = researchReadiness({
    ...preparedBase,
    compile_mode: 'chapter',
    compile_state: 'not_started',
    pending_review_count: 2,
  })
  assert.equal(readiness.status, 'attention')
  assert.ok(readiness.notes.some(note => note.includes('章节索引')))
  assert.ok(readiness.notes.some(note => note.includes('2 项待审核')))
  assert.ok(readiness.notes.every(note => !note.includes('无法研究')))
})

test('research readiness never calls a chapter-only base intelligent Wiki or invents a review count', () => {
  const readiness = researchReadiness({
    ...preparedBase,
    compile_mode: 'chapter',
    compile_phase: 'waiting_review',
  })
  assert.equal(readiness.status, 'attention')
  assert.ok(readiness.notes.some(note => note.includes('章节索引')))
  assert.ok(readiness.notes.some(note => note.includes('等待审核')))
  assert.ok(readiness.notes.every(note => !note.includes('0 项待审核')))
})

test('research readiness exposes sync and compilation failures without hiding available raw material', () => {
  const readiness = researchReadiness({
    ...preparedBase,
    sync_state: 'failed',
    compile_state: 'failed',
    entry_count: 0,
  })
  assert.equal(readiness.status, 'attention')
  assert.equal(readiness.canCreate, true)
  assert.ok(readiness.notes.some(note => note.includes('同步失败')))
  assert.ok(readiness.notes.some(note => note.includes('编译失败')))
  assert.ok(readiness.notes.some(note => note.includes('没有可检索条目')))
})
