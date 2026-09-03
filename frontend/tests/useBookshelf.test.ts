import assert from 'node:assert/strict'
import test from 'node:test'

import { createBookshelf } from '../src/composables/useBookshelf.ts'
import type { ReaderBook } from '../src/api/reader.ts'

function book(id: string, path: string, kind: 'folder' | 'pdf' = 'folder'): ReaderBook {
  return { id, path, kind, name: id, description: '', category: '', addedAt: 1 }
}

function makeDeps() {
  let saved: ReaderBook[] = []
  let fail = false
  let progressStore: Record<string, unknown> = {}
  const deps = {
    load: async () => [{ ...book('a', '/a') }] as ReaderBook[],
    persist: async (b: ReaderBook[]) => {
      if (fail) throw new Error('boom')
      saved = b
    },
    loadProgress: () => progressStore as Record<string, unknown>,
    saveProgress: (m: Record<string, unknown>) => {
      progressStore = { ...m }
    },
    get saved() {
      return saved
    },
    get progress() {
      return progressStore
    },
    set fail(v: boolean) {
      fail = v
    },
  }
  return deps
}

test('ensureLoaded fetches once and caches', async () => {
  const deps = makeDeps()
  let calls = 0
  deps.load = async () => {
    calls++
    return [book('a', '/a')]
  }
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  await shelf.ensureLoaded()
  assert.equal(calls, 1)
  assert.equal(shelf.books.value.length, 1)
  assert.equal(shelf.loaded.value, true)
})

test('addBook persists metadata; failure rolls back and returns false', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(await shelf.addBook(book('b', '/b')), true)
  assert.equal(shelf.books.value.length, 2)
  assert.deepEqual(deps.saved.map((b) => b.id), ['a', 'b'])
  // progress stripped from the backend payload (metadata-only)
  assert.equal(deps.saved[0].progress, undefined)
  deps.fail = true
  assert.equal(await shelf.addBook(book('c', '/c')), false)
  assert.equal(shelf.books.value.length, 2)
})

test('removeBook persists; updateBook replaces by id', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  const renamed = { ...book('a', '/a'), name: 'renamed' }
  assert.equal(await shelf.updateBook(renamed), true)
  assert.equal(shelf.books.value[0].name, 'renamed')
  assert.equal(await shelf.removeBook('a'), true)
  assert.equal(shelf.books.value.length, 0)
  deps.fail = true
  assert.equal(await shelf.removeBook('zzz'), false)
})

// ── per-file progress (localStorage) ───────────────────────────────────

test('saveFileProgress writes per-file state to the progress store, not the backend', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  shelf.saveFileProgress('a', '/a/n.md', { kind: 'md', position: 0.5, updatedAt: 1000 })
  // display progress reflected on the book (for the cover bar)
  assert.equal(shelf.books.value[0].progress?.position, 0.5)
  // backend NOT written for progress
  assert.equal(deps.saved.length, 0)
  // full per-file state retrievable for restore
  const st = shelf.getFileProgressState('a')
  assert.equal(st?.lastFile, '/a/n.md')
  assert.equal(st?.byFile['/a/n.md'].position, 0.5)
})

test('saveFileProgress keeps each file independent (md ratio + pdf page in one folder book)', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  shelf.saveFileProgress('a', '/a/a.md', { kind: 'md', position: 0.5, updatedAt: 1000 })
  shelf.saveFileProgress('a', '/a/b.pdf', { kind: 'pdf', position: 12, pageCount: 180, updatedAt: 2000 })
  const st = shelf.getFileProgressState('a')!
  assert.equal(st.lastFile, '/a/b.pdf')
  assert.equal(st.byFile['/a/a.md'].position, 0.5) // md ratio preserved
  assert.equal(st.byFile['/a/b.pdf'].position, 12) // pdf page preserved
  // display follows the lastFile (pdf page)
  assert.equal(shelf.books.value[0].progress?.position, 12)
  assert.equal(shelf.books.value[0].progress?.pageCount, 180)
})

test('openFile seeds a fresh file but never clobbers an existing entry', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  shelf.saveFileProgress('a', '/a/a.md', { kind: 'md', position: 0.5, updatedAt: 1000 })
  // open a new file b — seeded at 0, a's 0.5 untouched
  shelf.openFile('a', '/a/b.md', 'md')
  let st = shelf.getFileProgressState('a')!
  assert.equal(st.lastFile, '/a/b.md')
  assert.equal(st.byFile['/a/b.md'].position, 0)
  assert.equal(st.byFile['/a/a.md'].position, 0.5)
  // reopen a — pointer moves back, a NOT reset
  shelf.openFile('a', '/a/a.md', 'md')
  st = shelf.getFileProgressState('a')!
  assert.equal(st.lastFile, '/a/a.md')
  assert.equal(st.byFile['/a/a.md'].position, 0.5)
})

test('legacy backend progress migrates to per-file state on first load', async () => {
  const deps = makeDeps()
  deps.load = async () =>
    [
      { ...book('a', '/a'), progress: { lastFile: '/a/n.md', position: 0.42, updatedAt: 1000 } },
    ] as ReaderBook[]
  // localStorage empty → migration seeds from the backend progress
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  const st = shelf.getFileProgressState('a')!
  assert.equal(st.byFile['/a/n.md'].kind, 'md')
  assert.equal(st.byFile['/a/n.md'].position, 0.42)
  assert.equal(shelf.books.value[0].progress?.position, 0.42)
  // migration persisted to localStorage
  assert.ok(deps.progress['a'])
})

test('legacy single-pdf book migrates keyed by book.path', async () => {
  const deps = makeDeps()
  deps.load = async () =>
    [
      { ...book('a', '/a/b.pdf', 'pdf'), progress: { position: 12, pageCount: 180, updatedAt: 1000 } },
    ] as ReaderBook[]
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  const st = shelf.getFileProgressState('a')!
  assert.equal(st.lastFile, '/a/b.pdf')
  assert.equal(st.byFile['/a/b.pdf'].kind, 'pdf')
  assert.equal(st.byFile['/a/b.pdf'].position, 12)
})

test('findBook matches exact path', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(shelf.findBook('/a')?.id, 'a')
  assert.equal(shelf.findBook('/zzz'), undefined)
})
