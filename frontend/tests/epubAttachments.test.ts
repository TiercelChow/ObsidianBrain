import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  epubPosition,
  epubRestore,
  epubResourceUrl,
} from '../src/utils/epub.ts'
import {
  attachmentSize,
  attachmentValidation,
} from '../src/utils/taskAttachments.ts'
import { deriveKind, validateBookForm } from '../src/utils/readerBooks.ts'
import {
  deriveFileKind,
  readingFileBelongs,
} from '../src/utils/readerProgress.ts'

test('EPUB chapters round-trip through a bounded global position', () => {
  assert.equal(epubPosition(2, 0.5, 10), 0.25)
  assert.deepEqual(epubRestore(0.25, 10), { index: 2, ratio: 0.5 })
  assert.deepEqual(epubRestore(1, 10), { index: 9, ratio: 1 })
  assert.deepEqual(epubRestore(NaN, 0), { index: 0, ratio: 0 })
  assert.equal(
    epubResourceUrl('/v1/reader/epub/b/token/', 'OPS/第一章.xhtml#part'),
    '/v1/reader/epub/b/token/OPS/%E7%AC%AC%E4%B8%80%E7%AB%A0.xhtml#part',
  )
})
test('EPUB is a standalone shelf format and a supported folder document', () => {
  assert.equal(deriveKind('/books/test.EPUB'), 'epub')
  assert.equal(deriveFileKind('/books/test.epub'), 'epub')
  assert.equal(
    validateBookForm('/books/test.epub', null, [], {
      exists: true,
      is_dir: false,
    }),
    null,
  )
  assert.equal(
    readingFileBelongs(
      { kind: 'epub', path: '/books/test.epub' },
      '/books/test.epub',
    ),
    true,
  )
  assert.equal(
    readingFileBelongs(
      { kind: 'epub', path: '/books/test.epub' },
      '/books/test.epub/other',
    ),
    false,
  )
})
test('attachment bounds are explicit and readable', () => {
  assert.equal(
    attachmentValidation({ size: 0, name: 'empty.txt' }),
    '不能上传空文件',
  )
  assert.ok(
    attachmentValidation({ size: 50 * 1024 * 1024 + 1, name: 'large.zip' }),
  )
  assert.equal(attachmentValidation({ size: 1024, name: '资料.zip' }), null)
  assert.equal(attachmentSize(1024), '1 KB')
})
