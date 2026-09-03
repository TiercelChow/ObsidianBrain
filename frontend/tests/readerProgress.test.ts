import assert from 'node:assert/strict'
import test from 'node:test'

import {
  captureMdFileProgress,
  capturePdfFileProgress,
  deriveFileKind,
  getDisplayProgress,
  mergeBookState,
  migrateFromLegacy,
  parseProgressMap,
  serializeProgressMap,
  setLastFile,
} from '../src/utils/readerProgress.ts'
import type { BookProgressState } from '../src/utils/readerProgress.ts'

// ── deriveFileKind ───────────────────────────────────────────────────────

test('deriveFileKind maps .pdf (any case) to pdf, everything else to md', () => {
  assert.equal(deriveFileKind('/a/b/Book.PDF'), 'pdf')
  assert.equal(deriveFileKind('/a/b/notes.md'), 'md')
  assert.equal(deriveFileKind('/a/b/notes.MD'), 'md')
  assert.equal(deriveFileKind('/a/b/file'), 'md')
  assert.equal(deriveFileKind('/a/b/readme.txt'), 'md')
})

// ── capture md / pdf ────────────────────────────────────────────────────

test('captureMdFileProgress stores a 0..1 scroll ratio with kind md', () => {
  const p = captureMdFileProgress('/a/n.md', 500, 2000, 1000, 1000)
  assert.equal(p.kind, 'md')
  assert.equal(p.position, 0.5)
  assert.equal(p.updatedAt, 1000)
  assert.equal(p.pageCount, undefined)
})

test('capturePdfFileProgress stores the page + pageCount with kind pdf', () => {
  const p = capturePdfFileProgress('/a/b.pdf', 12, 180, 1000)
  assert.equal(p.kind, 'pdf')
  assert.equal(p.position, 12)
  assert.equal(p.pageCount, 180)
  assert.equal(p.updatedAt, 1000)
})

// ── mergeBookState ──────────────────────────────────────────────────────

test('mergeBookState builds a new state from null and sets lastFile', () => {
  const fp = captureMdFileProgress('/a/n.md', 500, 2000, 1000, 1000)
  const st = mergeBookState(null, '/a/n.md', fp)
  assert.equal(st.lastFile, '/a/n.md')
  assert.deepEqual(st.byFile['/a/n.md'], fp)
})

test('mergeBookState upserts without losing other files and updates lastFile', () => {
  const a = captureMdFileProgress('/a/a.md', 100, 2000, 1000, 1000)
  let st = mergeBookState(null, '/a/a.md', a)
  const b = capturePdfFileProgress('/a/b.pdf', 12, 180, 2000)
  st = mergeBookState(st, '/a/b.pdf', b)
  assert.equal(st.lastFile, '/a/b.pdf')
  assert.equal(st.byFile['/a/a.md'].position, 0.1)
  assert.equal(st.byFile['/a/b.pdf'].position, 12)
  // update a again — b is preserved, lastFile moves back to a
  const a2 = captureMdFileProgress('/a/a.md', 1000, 2000, 1000, 3000)
  st = mergeBookState(st, '/a/a.md', a2)
  assert.equal(st.lastFile, '/a/a.md')
  assert.equal(st.byFile['/a/a.md'].updatedAt, 3000)
  assert.equal(st.byFile['/a/b.pdf'].position, 12)
})

// ── getDisplayProgress ──────────────────────────────────────────────────

test('getDisplayProgress returns null for null/empty state', () => {
  assert.equal(getDisplayProgress(null), null)
  assert.equal(getDisplayProgress({ lastFile: '/x', byFile: {} }), null)
})

test('getDisplayProgress derives the last-opened file progress for the cover bar', () => {
  const a = captureMdFileProgress('/a/a.md', 500, 2000, 1000, 1000) // 0.5
  const b = capturePdfFileProgress('/a/b.pdf', 12, 180, 2000)
  const st: BookProgressState = { lastFile: '/a/b.pdf', byFile: { '/a/a.md': a, '/a/b.pdf': b } }
  const d = getDisplayProgress(st)
  assert.equal(d?.position, 12)
  assert.equal(d?.pageCount, 180)
  assert.equal(d?.updatedAt, 2000)
})

test('getDisplayProgress returns null when lastFile is not in byFile', () => {
  const st: BookProgressState = { lastFile: '/missing', byFile: {} }
  assert.equal(getDisplayProgress(st), null)
})

// ── setLastFile ─────────────────────────────────────────────────────────

test('setLastFile seeds a fresh md file at position 0 when absent', () => {
  const st = setLastFile(null, '/a/n.md', 'md', 1000)
  assert.equal(st.lastFile, '/a/n.md')
  assert.equal(st.byFile['/a/n.md'].kind, 'md')
  assert.equal(st.byFile['/a/n.md'].position, 0)
})

test('setLastFile seeds a fresh pdf file at page 1 when absent', () => {
  const st = setLastFile(null, '/a/b.pdf', 'pdf', 1000)
  assert.equal(st.byFile['/a/b.pdf'].kind, 'pdf')
  assert.equal(st.byFile['/a/b.pdf'].position, 1)
})

test('setLastFile moves the pointer without clobbering a saved file entry', () => {
  const a = captureMdFileProgress('/a/a.md', 500, 2000, 1000, 1000) // 0.5
  let st = mergeBookState(null, '/a/a.md', a)
  // user opens file b (no saved progress) — pointer moves, a's 0.5 preserved
  st = setLastFile(st, '/a/b.md', 'md', 2000)
  assert.equal(st.lastFile, '/a/b.md')
  assert.equal(st.byFile['/a/b.md'].position, 0) // seeded fresh
  assert.equal(st.byFile['/a/a.md'].position, 0.5) // untouched
  // user reopens a — pointer moves back, a's entry NOT reset
  st = setLastFile(st, '/a/a.md', 'md', 3000)
  assert.equal(st.lastFile, '/a/a.md')
  assert.equal(st.byFile['/a/a.md'].position, 0.5) // still 0.5
})

// ── migrateFromLegacy ──────────────────────────────────────────────────

test('migrateFromLegacy: folder book seeds byFile from the md lastFile', () => {
  // legacy: { lastFile, position (ratio), updatedAt } for a folder book
  const st = migrateFromLegacy({ lastFile: '/a/n.md', position: 0.42, updatedAt: 1000 }, '/a', 'folder')
  assert.equal(st?.lastFile, '/a/n.md')
  assert.equal(st?.byFile['/a/n.md'].kind, 'md')
  assert.equal(st?.byFile['/a/n.md'].position, 0.42)
  assert.equal(st?.byFile['/a/n.md'].updatedAt, 1000)
})

test('migrateFromLegacy: single pdf book seeds byFile keyed by book.path', () => {
  const st = migrateFromLegacy({ position: 12, pageCount: 180, updatedAt: 1000 }, '/a/b.pdf', 'pdf')
  assert.equal(st?.lastFile, '/a/b.pdf')
  assert.equal(st?.byFile['/a/b.pdf'].kind, 'pdf')
  assert.equal(st?.byFile['/a/b.pdf'].position, 12)
  assert.equal(st?.byFile['/a/b.pdf'].pageCount, 180)
})

test('migrateFromLegacy: null/missing legacy → null (start fresh)', () => {
  assert.equal(migrateFromLegacy(null, '/a', 'folder'), null)
  assert.equal(migrateFromLegacy({ position: 0, updatedAt: 0 }, '/a', 'folder'), null)
})

test('migrateFromLegacy: folder book whose lastFile is a pdf (legacy corruption) seeds it as pdf', () => {
  // A folder book where the user opened a pdf: onPdfPageChange saved {position: page}
  // without updating lastFile, so lastFile stayed an md. But if lastFile itself is a
  // pdf path, treat the position as a page.
  const st = migrateFromLegacy({ lastFile: '/a/x.pdf', position: 5, updatedAt: 1000 }, '/a', 'folder')
  assert.equal(st?.byFile['/a/x.pdf'].kind, 'pdf')
  assert.equal(st?.byFile['/a/x.pdf'].position, 5)
})

// ── parse / serialize ───────────────────────────────────────────────────

test('serializeProgressMap / parseProgressMap round-trip', () => {
  const fp = captureMdFileProgress('/a/n.md', 500, 2000, 1000, 1000)
  const map = { 'b1': mergeBookState(null, '/a/n.md', fp) }
  const json = serializeProgressMap(map)
  const back = parseProgressMap(json)
  assert.deepEqual(back, map)
})

test('parseProgressMap returns empty map on bad JSON', () => {
  assert.deepEqual(parseProgressMap('{not json'), {})
  assert.deepEqual(parseProgressMap('null'), {})
  assert.deepEqual(parseProgressMap(''), {})
})
