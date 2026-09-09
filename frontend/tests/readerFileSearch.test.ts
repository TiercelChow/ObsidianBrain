import assert from 'node:assert/strict'
import test from 'node:test'
import type { DirEntry } from '../src/api/reader.ts'
import { searchReaderFiles } from '../src/utils/readerFileSearch.ts'

const root = '/notes'
const entries: DirEntry[] = [
  {
    name: '项目', path: '/notes/项目', is_dir: true, is_markdown: false, is_pdf: false,
    children: [
      { name: '设计方案.md', path: '/notes/项目/设计方案.md', is_dir: false, is_markdown: true, is_pdf: false },
      { name: '架构图.pdf', path: '/notes/项目/架构图.pdf', is_dir: false, is_markdown: false, is_pdf: true },
      { name: '草稿.txt', path: '/notes/项目/草稿.txt', is_dir: false, is_markdown: false, is_pdf: false },
    ],
  },
  { name: 'README.md', path: '/notes/README.md', is_dir: false, is_markdown: true, is_pdf: false },
]

test('reader directory search matches readable files by name and relative path', () => {
  assert.deepEqual(searchReaderFiles(entries, root, '设计').map((item) => item.relativePath), ['项目/设计方案.md'])
  assert.deepEqual(searchReaderFiles(entries, root, '项目 pdf').map((item) => item.relativePath), ['项目/架构图.pdf'])
  assert.deepEqual(searchReaderFiles(entries, root, 'readme').map((item) => item.relativePath), ['README.md'])
})

test('reader directory search excludes folders and unsupported files', () => {
  assert.deepEqual(searchReaderFiles(entries, root, ''), [])
  assert.deepEqual(searchReaderFiles(entries, root, '项目').map((item) => item.relativePath), [
    '项目/架构图.pdf',
    '项目/设计方案.md',
  ])
  assert.deepEqual(searchReaderFiles(entries, root, '草稿'), [])
})
