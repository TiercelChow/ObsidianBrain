import assert from 'node:assert/strict'
import test from 'node:test'

import { createBookshelf } from '../src/composables/useBookshelf.ts'
import type { ReaderBook } from '../src/api/reader.ts'

function book(id: string, path: string): ReaderBook {
  return { id, path, kind: 'folder', name: id, description: '', category: '', addedAt: 1 }
}

function makeDeps() {
  let saved: ReaderBook[] = []
  let fail = false
  const deps = {
    load: async () => [{ ...book('a', '/a') }] as ReaderBook[],
    persist: async (b: ReaderBook[]) => {
      if (fail) throw new Error('boom')
      saved = b
    },
    get saved() {
      return saved
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

test('addBook persists; failure rolls back and returns false', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(await shelf.addBook(book('b', '/b')), true)
  assert.equal(shelf.books.value.length, 2)
  assert.deepEqual(deps.saved.map((b) => b.id), ['a', 'b'])
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
  assert.equal(await shelf.removeBook('zzz'), false) // 不存在的 id 也走保存失败路径
})

test('updateProgress merges patch with updatedAt and keeps book on save failure', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  shelf.updateProgress('a', { lastFile: '/a/x.md', position: 0.5 })
  const p = shelf.books.value[0].progress!
  assert.equal(p.lastFile, '/a/x.md')
  assert.equal(p.position, 0.5)
  assert.ok(p.updatedAt > 0)
  await new Promise((r) => setTimeout(r, 10))
  assert.equal(deps.saved[0].progress!.position, 0.5)
  deps.fail = true
  shelf.updateProgress('a', { position: 0.9 }) // 失败不回滚、不抛出
  assert.equal(shelf.books.value[0].progress!.position, 0.9)
})

test('findBook matches exact path', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(shelf.findBook('/a')?.id, 'a')
  assert.equal(shelf.findBook('/zzz'), undefined)
})
