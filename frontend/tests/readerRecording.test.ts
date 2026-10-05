import assert from 'node:assert/strict'
import test from 'node:test'
import { readFile } from 'node:fs/promises'
import { resolveReadingBookId, mergeProgressStates } from '../src/utils/readerProgress.ts'

test('PDF in a folder belongs to that book, while explicitly opened standalone PDF wins',()=>{
  const books=[{id:'folder',path:'/books/a',kind:'folder' as const},{id:'pdf',path:'/books/a/book.pdf',kind:'pdf' as const}]
  assert.equal(resolveReadingBookId(books,'/books/a','/books/a/book.pdf','folder'),'folder')
  assert.equal(resolveReadingBookId(books,'/books/a','/books/a/book.pdf','pdf'),'pdf')
  assert.equal(resolveReadingBookId(books.slice(0,1),'/books/a','/books/a/book.pdf'),'folder')
  assert.equal(resolveReadingBookId(books,'/books/a','/other/file.md'),null)
})
test('merging old cache preserves the newest server pointer and independent positions',()=>{
  const merged=mergeProgressStates({lastFile:'/a/new.md',lastReadAt:300,byFile:{'/a/new.md':{kind:'md',position:.8,updatedAt:300}}},{lastFile:'/a/old.pdf',byFile:{'/a/old.pdf':{kind:'pdf',position:12,updatedAt:100}}})!
  assert.equal(merged.lastFile,'/a/new.md');assert.equal(merged.lastReadAt,300)
  assert.equal(merged.byFile['/a/old.pdf'].position,12)
})
test('file and folder switches capture outgoing scroll before changing the document',async()=>{
  const reader=await readFile(new URL('../src/views/Reader.vue',import.meta.url),'utf8')
  assert.match(reader,/async function onSelectFile\(path: string\)\s*\{[\s\S]*?flushProgressNow\(\)[\s\S]*?fileRequest\?\.abort\(\)/)
  assert.match(reader,/async function openPath\(rawPath: string\)\s*\{[\s\S]*?flushProgressNow\(\)[\s\S]*?cancelPendingFileSelection\(\)/)
  assert.match(reader,/onBeforeRouteLeave\(/)
})
test('late PDF events are scoped to their source and a restore ignores intermediate pages',async()=>{
  const reader=await readFile(new URL('../src/views/Reader.vue',import.meta.url),'utf8')
  assert.match(reader,/function onPdfPageChange\(page: number, source: string\)\s*\{\s*if \(source !== displayedFile.value\) return/)
  assert.match(reader,/pendingPdfPage !== null && page !== pendingPdfPage/)
  assert.match(reader,/requestAnimationFrame\(\(\) => \{\s*if \(source !== displayedFile.value \|\| pendingPdfPage !== target\) return/)
})
