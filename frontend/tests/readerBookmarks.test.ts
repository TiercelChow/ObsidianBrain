import assert from 'node:assert/strict'
import test from 'node:test'
import { readFile } from 'node:fs/promises'
import { resolveBookmarkBlock, bookmarkPositionLabel, clampBookmarkUnit } from '../src/utils/readerBookmarks.ts'

test('bookmark blocks survive inserted paragraphs and resolve duplicate text near the old index', () => {
  const location = { kind: 'md' as const, block_index: 2, text: '目标 段落', ratio: .5, offset: .3 }
  assert.equal(resolveBookmarkBlock(['标题', '新增', '引言', '目标\n段落', '末尾'], location), 3)
  assert.equal(resolveBookmarkBlock(['目标 段落', '引言', '目标 段落'], location), 2)
  assert.equal(resolveBookmarkBlock(['内容已完全更改'], location), -1)
  assert.equal(resolveBookmarkBlock(['标题', '', '文字'], {...location,text:'',block_index:1}), 1, 'image-only blocks retain a position anchor')
})
test('bookmark labels and coordinates remain bounded', () => {
  assert.equal(clampBookmarkUnit(NaN), 0)
  assert.equal(clampBookmarkUnit(2), 1)
  assert.equal(clampBookmarkUnit(-1), 0)
  assert.equal(bookmarkPositionLabel({kind:'pdf',page:12,offset:.6,x:0}), '第 12 页 · 60%')
  assert.equal(bookmarkPositionLabel({kind:'md',block_index:2,text:'',ratio:.42,offset:0}), '42%')
})

test('selected text anchors a new bookmark without prefilling its note', async () => {
  const reader = await readFile(new URL('../src/views/Reader.vue', import.meta.url), 'utf8')
  const creation = reader.slice(reader.indexOf('function openBookmarks('), reader.indexOf('function onBookmarkSaved('))
  assert.match(creation, /note_md:\s*''/)
  assert.match(creation, /hasSelection\?captureBookmarkLocation\(pane,fileKind\.value\)/)
  assert.doesNotMatch(creation, /selection\.toString\(|quoted\.split\(/)
})
