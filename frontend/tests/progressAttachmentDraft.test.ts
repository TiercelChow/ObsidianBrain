import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  createProgressDraft,
  appendProgressFiles,
  submitProgressDraft,
} from '../src/utils/progressAttachmentDraft.ts'

test('files are selected in the editor, validated as a batch, and limited to 20', () => {
  const draft = createProgressDraft<{ name: string; size: number }>()
  appendProgressFiles(draft, [{ name: 'notes.txt', size: 30 }])
  assert.equal(draft.files[0].status, 'pending')
  assert.throws(() => appendProgressFiles(draft, [{ name: 'empty', size: 0 }]))
  assert.equal(draft.files.length, 1)
  assert.throws(() =>
    appendProgressFiles(
      draft,
      Array.from({ length: 20 }, () => ({ name: 'more', size: 1 })),
    ),
  )
})
test('one submit creates the progress and uploads all selected files', async () => {
  const draft = createProgressDraft<{ name: string; size: number }>()
  appendProgressFiles(draft, [
    { name: 'a', size: 1 },
    { name: 'b', size: 1 },
  ])
  let creates = 0
  const uploaded: string[] = []
  assert.equal(
    await submitProgressDraft(draft, {
      create: async () => {
        creates++
        return 'progress'
      },
      upload: async (file, id, onProgress) => {
        assert.equal(id, 'progress')
        onProgress(100)
        uploaded.push(file.name)
      },
    }),
    true,
  )
  assert.equal(creates, 1)
  assert.deepEqual(uploaded, ['a', 'b'])
  assert.ok(draft.files.every((f) => f.status === 'uploaded'))
})
test('retry preserves the saved progress and successful files, without duplicates', async () => {
  const draft = createProgressDraft<{ name: string; size: number }>()
  appendProgressFiles(draft, [
    { name: 'a', size: 1 },
    { name: 'b', size: 1 },
  ])
  let creates = 0,
    fail = true
  const uploaded: string[] = []
  const deps = {
    create: async () => {
      creates++
      return 'saved'
    },
    upload: async (file: { name: string }, id: string) => {
      assert.equal(id, 'saved')
      uploaded.push(file.name)
      if (file.name === 'b' && fail) throw new Error('offline')
    },
  }
  assert.equal(await submitProgressDraft(draft, deps), false)
  assert.equal(draft.progressId, 'saved')
  assert.equal(draft.files[1].error, 'offline')
  fail = false
  assert.equal(await submitProgressDraft(draft, deps), true)
  assert.equal(creates, 1)
  assert.deepEqual(uploaded, ['a', 'b', 'b'])
})
test('a failed note write starts no uploads, while text-only progress still works', async () => {
  const draft = createProgressDraft<{ name: string; size: number }>()
  appendProgressFiles(draft, [{ name: 'a', size: 1 }])
  let uploaded = false
  await assert.rejects(
    submitProgressDraft(draft, {
      create: async () => {
        throw new Error('conflict')
      },
      upload: async () => {
        uploaded = true
      },
    }),
  )
  assert.equal(uploaded, false)
  assert.equal(draft.progressId, null)
  const empty = createProgressDraft<{ name: string; size: number }>()
  assert.equal(
    await submitProgressDraft(empty, {
      create: async () => 'text-only',
      upload: async () => {
        throw new Error('unexpected')
      },
    }),
    true,
  )
})
