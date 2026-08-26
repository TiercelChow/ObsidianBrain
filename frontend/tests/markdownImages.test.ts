import assert from 'node:assert/strict'
import test from 'node:test'

import {
  convertObsidianImageEmbeds,
  isLocalHref,
  resolveRelativePath,
  safeDecodeHref,
} from '../src/utils/markdownImages.ts'

// ── resolveRelativePath ────────────────────────────────────────────────────

test('resolveRelativePath joins plain relative names under the base dir', () => {
  assert.equal(resolveRelativePath('/Users/x/Timeline', 'images/a.jpg'), '/Users/x/Timeline/images/a.jpg')
})

test('resolveRelativePath strips ./ and walks .. segments', () => {
  assert.equal(resolveRelativePath('/Users/x/Timeline', './a.png'), '/Users/x/Timeline/a.png')
  assert.equal(resolveRelativePath('/Users/x/Timeline/sub', '../a.png'), '/Users/x/Timeline/a.png')
  assert.equal(resolveRelativePath('/Users/x', 'a/../b.png'), '/Users/x/b.png')
})

test('resolveRelativePath keeps spaces and CJK intact for later URL encoding', () => {
  assert.equal(resolveRelativePath('/Users/x/笔记', '我的 图.png'), '/Users/x/笔记/我的 图.png')
})

test('resolveRelativePath tolerates an empty base dir', () => {
  assert.equal(resolveRelativePath('', 'a.png'), 'a.png')
})

// ── isLocalHref ────────────────────────────────────────────────────────────

test('isLocalHref rejects external and protocol URLs', () => {
  assert.equal(isLocalHref('https://example.com/a.png'), false)
  assert.equal(isLocalHref('http://example.com/a.png'), false)
  assert.equal(isLocalHref('data:image/png;base64,xxx'), false)
  assert.equal(isLocalHref('blob:https://x/y'), false)
  assert.equal(isLocalHref('//cdn.example.com/a.png'), false)
})

test('isLocalHref accepts relative and absolute-local hrefs', () => {
  assert.equal(isLocalHref('./a.png'), true)
  assert.equal(isLocalHref('images/a%20b.jpg'), true)
  assert.equal(isLocalHref('/Users/x/a.png'), true)
})

// ── safeDecodeHref ─────────────────────────────────────────────────────────

test('safeDecodeHref decodes percent escapes', () => {
  assert.equal(safeDecodeHref('a%20b.png'), 'a b.png')
  assert.equal(safeDecodeHref('%E7%AC%94%E8%AE%B0/a.png'), '笔记/a.png')
})

test('safeDecodeHref passes malformed sequences through instead of throwing', () => {
  assert.equal(safeDecodeHref('100%.png'), '100%.png')
  assert.equal(safeDecodeHref('a%2'), 'a%2')
})

// ── convertObsidianImageEmbeds ─────────────────────────────────────────────

test('convertObsidianImageEmbeds turns image embeds into markdown images', () => {
  const out = convertObsidianImageEmbeds(
    '![[Timeline/images/2026-07-11-011802-0.jpg]]',
    (target) => `/raw?path=${encodeURIComponent(target)}`,
  )
  assert.equal(out, `![2026-07-11-011802-0.jpg](/raw?path=${encodeURIComponent('Timeline/images/2026-07-11-011802-0.jpg')})`)
})

test('convertObsidianImageEmbeds strips an Obsidian size suffix and resolves the bare path', () => {
  const seen: string[] = []
  const out = convertObsidianImageEmbeds('![[a photo.png|300]]', (target) => {
    seen.push(target)
    return '/raw?path=' + encodeURIComponent(target)
  })
  assert.deepEqual(seen, ['a photo.png'])
  assert.equal(out, '![a photo.png](/raw?path=' + encodeURIComponent('a photo.png') + ')')
})

test('convertObsidianImageEmbeds leaves non-image embeds and plain text alone', () => {
  const md = '看这个 ![[某篇笔记]] 和 ![[note#章节]] 还有 ![[readme.md]]，普通 ![[b.png]] 才是图。'
  const out = convertObsidianImageEmbeds(md, () => '/raw')
  assert.ok(out.includes('![[某篇笔记]]'))
  assert.ok(out.includes('![[note#章节]]'))
  assert.ok(out.includes('![[readme.md]]'))
  assert.ok(!out.includes('![[b.png]]'))
})

test('convertObsidianImageEmbeds keeps the embed when resolve returns null', () => {
  const out = convertObsidianImageEmbeds('![[a.png]] 前后文字', () => null)
  assert.equal(out, '![[a.png]] 前后文字')
})

test('convertObsidianImageEmbeds handles several embeds on one line', () => {
  const out = convertObsidianImageEmbeds(
    '![[a.jpg]] 中间文字 ![[sub/b.png]]',
    (t) => `/raw?${t}`,
  )
  assert.equal(out, '![a.jpg](/raw?a.jpg) 中间文字 ![b.png](/raw?sub/b.png)')
})
