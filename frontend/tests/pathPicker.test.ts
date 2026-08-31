import assert from 'node:assert/strict'
import test from 'node:test'

import { parentPath, pathSegments, pickableEntries } from '../src/utils/pathPicker.ts'
import type { DirEntry } from '../src/api/reader.ts'

// ── parentPath ────────────────────────────────────────────────────────────

test('parentPath walks up one level; root and empty are their own parent', () => {
  assert.equal(parentPath('/a/b/c'), '/a/b')
  assert.equal(parentPath('/a/b/'), '/a') // trailing slash → same as /a/b, parent is /a
  assert.equal(parentPath('/a'), '/')
  assert.equal(parentPath('/'), '/')
  assert.equal(parentPath(''), '/')
})

test('parentPath handles single-segment absolute paths', () => {
  assert.equal(parentPath('/Users'), '/')
  assert.equal(parentPath('/Users/'), '/')
})

// ── pathSegments ──────────────────────────────────────────────────────────

test('pathSegments returns root-only breadcrumb for "/"', () => {
  assert.deepEqual(pathSegments('/'), [{ name: '/', path: '/' }])
})

test('pathSegments builds an increasing breadcrumb from root to path', () => {
  const segs = pathSegments('/Users/tiercelchow/Documents')
  assert.deepEqual(segs, [
    { name: '/', path: '/' },
    { name: 'Users', path: '/Users' },
    { name: 'tiercelchow', path: '/Users/tiercelchow' },
    { name: 'Documents', path: '/Users/tiercelchow/Documents' },
  ])
})

test('pathSegments tolerates trailing slashes', () => {
  assert.deepEqual(
    pathSegments('/a/b/').map((s) => s.path),
    ['/', '/a', '/a/b'],
  )
})

// ── pickableEntries ───────────────────────────────────────────────────────

function entry(name: string, opts: Partial<DirEntry> = {}): DirEntry {
  return { name, path: '/' + name, is_dir: false, is_markdown: false, is_pdf: false, ...opts }
}

test('pickableEntries keeps dirs + pdfs only, dirs first, case-insensitive sort', () => {
  const entries = [
    entry('z.md', { is_markdown: true }),
    entry('B.pdf', { is_pdf: true }),
    entry('a.txt'),
    entry('A folder', { is_dir: true, path: '/A folder' }),
    entry('a.pdf', { is_pdf: true }),
    entry('Z folder', { is_dir: true, path: '/Z folder' }),
  ]
  const result = pickableEntries(entries)
  // dirs first (A, Z), then PDFs (a, B) — case-insensitive
  assert.deepEqual(
    result.map((e) => e.name),
    ['A folder', 'Z folder', 'a.pdf', 'B.pdf'],
  )
})

test('pickableEntries drops markdown and other non-pdf files', () => {
  const entries = [
    entry('note.md', { is_markdown: true }),
    entry('readme.txt'),
    entry('doc.pdf', { is_pdf: true }),
    entry('sub', { is_dir: true }),
  ]
  assert.deepEqual(
    pickableEntries(entries).map((e) => e.name),
    ['sub', 'doc.pdf'],
  )
})

test('pickableEntries empty-in empty-out', () => {
  assert.deepEqual(pickableEntries([]), [])
})
