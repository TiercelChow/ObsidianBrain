import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { canFocusDocument } from '../src/utils/modalFocusPolicy.ts'

test('modals can move focus only while their document owns focus', () => {
  assert.equal(canFocusDocument({ visibilityState: 'visible', hasFocus: () => true }), true)
  assert.equal(canFocusDocument({ visibilityState: 'visible', hasFocus: () => false }), false)
  assert.equal(canFocusDocument({ visibilityState: 'hidden', hasFocus: () => true }), false)
})

test('shared overlays, reader search, and background pollers respect the focus policy', async () => {
  const [modal, reader, bases, tasks] = await Promise.all([
    readFile(new URL('../src/composables/useModalEnvironment.ts', import.meta.url), 'utf8'),
    readFile(new URL('../src/views/Reader.vue', import.meta.url), 'utf8'),
    readFile(new URL('../src/views/knowledge/KnowledgeBases.vue', import.meta.url), 'utf8'),
    readFile(new URL('../src/views/knowledge/KnowledgeTasks.vue', import.meta.url), 'utf8'),
  ])
  assert.ok(modal.includes('!canFocusDocument(document) || !panel'))
  assert.ok(modal.includes('if (canFocusDocument(document) && target?.isConnected) target.focus'))
  assert.match(reader, /fileSearchOpen\.value && canFocusDocument\(document\)/)
  assert.ok(bases.includes('if (!canFocusDocument(document)) return'))
  assert.ok(tasks.includes('if (!canFocusDocument(document)) continue'))
})
