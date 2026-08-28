import assert from 'node:assert/strict'
import test from 'node:test'

import {
  bookProgressLabel,
  clampPdfPage,
  defaultBookName,
  deriveKind,
  findBookByPath,
  makeBookId,
  scrollRatio,
  sortBooks,
  validateBookForm,
} from '../src/utils/readerBooks.ts'
import type { ReaderBook } from '../src/api/reader.ts'

// ── deriveKind / defaultBookName / makeBookId ──────────────────────────────

test('deriveKind maps .pdf (any case) to pdf, everything else to folder', () => {
  assert.equal(deriveKind('/a/b/Book.PDF'), 'pdf')
  assert.equal(deriveKind('/a/b/book.pdf'), 'pdf')
  assert.equal(deriveKind('/a/docs'), 'folder')
  assert.equal(deriveKind('/a/notes.md'), 'folder')
})

test('defaultBookName takes the last path segment', () => {
  assert.equal(defaultBookName('/a/docs'), 'docs')
  assert.equal(defaultBookName('/a/机器学习.pdf'), '机器学习.pdf')
  assert.equal(defaultBookName('/'), '')
})

test('makeBookId yields unique non-empty ids', () => {
  const ids = new Set(Array.from({ length: 200 }, () => makeBookId()))
  assert.equal(ids.size, 200)
})

// ── scrollRatio / clampPdfPage ─────────────────────────────────────────────

test('scrollRatio returns clamped ratio and 0 for non-scrollable content', () => {
  assert.equal(scrollRatio(500, 2000, 1000), 0.5)
  assert.equal(scrollRatio(0, 1000, 1000), 0)
  assert.equal(scrollRatio(9999, 2000, 1000), 1)
})

test('clampPdfPage clamps into [1, pageCount] and tolerates unknown pageCount', () => {
  assert.equal(clampPdfPage(12, 180), 12)
  assert.equal(clampPdfPage(200, 180), 180)
  assert.equal(clampPdfPage(0, 180), 1)
  assert.equal(clampPdfPage(2.7, 180), 2)
  assert.equal(clampPdfPage(7, 0), 7)
})

// ── bookProgressLabel / sortBooks ──────────────────────────────────────────

function folderBook(p?: { position: number }): ReaderBook {
  return {
    id: 'a',
    path: '/d',
    kind: 'folder',
    name: 'n',
    description: '',
    category: '',
    addedAt: 1,
    ...(p ? { progress: { position: p.position, updatedAt: 2 } } : {}),
  }
}

test('bookProgressLabel covers md/pdf/unread cases', () => {
  assert.equal(bookProgressLabel(folderBook()), '未开始')
  assert.equal(bookProgressLabel(folderBook({ position: 0.424 })), '读到 42%')
  assert.equal(bookProgressLabel(folderBook({ position: 0 })), '未开始')
  const pdf: ReaderBook = {
    id: 'b',
    path: '/x.pdf',
    kind: 'pdf',
    name: 'n',
    description: '',
    category: '',
    addedAt: 1,
    progress: { position: 12, pageCount: 180, updatedAt: 2 },
  }
  assert.equal(bookProgressLabel(pdf), '第 12/180 页')
  const pdfNoCount: ReaderBook = { ...pdf, progress: { position: 12, updatedAt: 2 } }
  assert.equal(bookProgressLabel(pdfNoCount), '第 12 页')
})

test('sortBooks orders by last activity (progress.updatedAt ?? addedAt) descending', () => {
  const b = (id: string, t: number, p?: number) => ({
    id,
    path: '/' + id,
    kind: 'folder' as const,
    name: id,
    description: '',
    category: '',
    addedAt: t,
    ...(p !== undefined ? { progress: { position: p, updatedAt: t + 100 } } : {}),
  })
  // keys: a=50 (addedAt), b=110 (progress.updatedAt), c=200 (addedAt)
  assert.deepEqual(
    sortBooks([b('a', 50), b('b', 10, 0.5), b('c', 200)]).map((x) => x.id),
    ['c', 'b', 'a'],
  )
})

// ── findBookByPath / validateBookForm ──────────────────────────────────────

test('findBookByPath matches exact path', () => {
  const books = [folderBook()]
  assert.equal(findBookByPath(books, '/d')?.id, 'a')
  assert.equal(findBookByPath(books, '/doc'), undefined)
})

test('validateBookForm rejects empty, missing, wrong-kind, duplicate paths', () => {
  const books = [folderBook()] // path /d
  const dirStat = { exists: true, is_dir: true }
  assert.match(validateBookForm('', null, books, null)!, /路径/)
  assert.match(validateBookForm('/nope', null, books, { exists: false, is_dir: false })!, /不存在/)
  assert.match(validateBookForm('/x.txt', null, books, { exists: true, is_dir: false })!, /文件夹或 PDF/)
  assert.match(validateBookForm('/d', null, books, dirStat)!, /已在书架/)
  // 编辑同一本书时路径未变不算重复
  assert.equal(validateBookForm('/d', 'a', books, dirStat), null)
  assert.equal(validateBookForm('/new', null, books, dirStat), null)
  assert.equal(validateBookForm('/new.pdf', null, books, { exists: true, is_dir: false }), null)
  // stat 未返回（校验中）时不报路径类型错误，由调用方控制提交时机
  assert.equal(validateBookForm('/new', null, books, null), null)
})
