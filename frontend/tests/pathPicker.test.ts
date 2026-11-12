import assert from 'node:assert/strict'
import test from 'node:test'

import { normalizePickerPath, parentPath, pathSegments, pickableEntries } from '../src/utils/pathPicker.ts'
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

test('Windows drive breadcrumbs retain the absolute drive root', () => {
  assert.equal(parentPath('C:\\'), 'C:\\')
  assert.equal(parentPath('C:\\Book.epub'), 'C:\\')
  assert.deepEqual(pathSegments('D:/书库/机器 学习/'), [
    { name: 'D:', path: 'D:\\' },
    { name: '书库', path: 'D:\\书库' },
    { name: '机器 学习', path: 'D:\\书库\\机器 学习' },
  ])
})

test('UNC paths never navigate above the share, including the extended form', () => {
  const root = String.raw`\\server\共享`
  const extended = String.raw`\\?\UNC\server\共享`
  assert.deepEqual(pathSegments(`${root}\\书库`), [
    { name: root, path: root },
    { name: '书库', path: `${root}\\书库` },
  ])
  assert.equal(parentPath(root), root)
  assert.equal(parentPath(`${extended}\\书库`), extended)
})

test('pasted paths accept Explorer quotes and drive shorthand, not relative or device paths', () => {
  assert.equal(normalizePickerPath(' "d:/书 库/书.pdf" '), 'D:\\书 库\\书.pdf')
  assert.equal(normalizePickerPath('D:'), 'D:\\')
  assert.equal(normalizePickerPath('//nas/books/目录'), String.raw`\\nas\books\目录`)
  assert.equal(normalizePickerPath(String.raw`/tmp/name\with-slash`), String.raw`/tmp/name\with-slash`)
  for (const value of ['C:book.pdf', '../book.pdf', String.raw`\book.pdf`, String.raw`\\.\PhysicalDrive0`, String.raw`\\server`]) {
    assert.equal(normalizePickerPath(value), null)
  }
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
