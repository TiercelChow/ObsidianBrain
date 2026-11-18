import assert from 'node:assert/strict'
import test from 'node:test'
import { researchReadiness } from '../src/utils/researchReadiness.ts'

const preparedBase = {
  lifecycle: 'active' as const,
  source_available: true,
  sync_state: 'clean' as const,
  source_count: 12,
  entry_count: 34,
}

test('research readiness reports usable material without claiming answer coverage', () => {
  const readiness = researchReadiness(preparedBase)
  assert.equal(readiness.status, 'ready')
  assert.equal(readiness.canCreate, true)
  assert.match(readiness.summary, /原文已同步/)
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

test('research readiness exposes source sync failures without hiding available raw material', () => {
  const readiness = researchReadiness({
    ...preparedBase,
    sync_state: 'failed',
    entry_count: 0,
  })
  assert.equal(readiness.status, 'attention')
  assert.equal(readiness.canCreate, true)
  assert.ok(readiness.notes.some(note => note.includes('同步失败')))
  assert.ok(readiness.notes.some(note => note.includes('没有可检索条目')))
})
