import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../src/', import.meta.url)
const source = (path: string) => readFile(new URL(path, root), 'utf8')

test('shelf, reading trigger and file search dialog share one search field class', async () => {
  const reader = await source('views/Reader.vue')
  const classes = [...reader.matchAll(/\bclass="([^"]+)"/g)].map(match => match[1].split(/\s+/))
  for (const className of ['shelf-search', 'reader-file-search-trigger', 'reader-file-search']) {
    const field = classes.find(tokens => tokens.includes(className))
    assert.ok(field?.includes('reader-search-field'), `${className} uses shared field styling`)
  }
})

test('reader search field owns shared geometry, typography and clear affordance', async () => {
  const reader = await source('views/Reader.vue')
  const field = reader.match(/\.reader-search-field \{([^}]+)\}/)?.[1] ?? ''
  for (const rule of ['height: 44px', 'flex-shrink: 0', 'gap: 10px', 'padding: 0 14px', 'border-radius: 12px', 'font-size: 14px']) {
    assert.ok(field.includes(rule), rule)
  }
  assert.match(reader, /\.reader-search-field input \{[^}]*font: inherit/)
  assert.match(reader, /\.reader-search-field > \.el-icon \{[^}]*font-size: 16px/)
  assert.match(reader, /\.reader-search-field button \{[^}]*width: 44px;[^}]*height: 42px/)
  assert.match(reader, /@media \(max-width: 768px\)[\s\S]*?\.reader-search-field \{ font-size: 16px; \}/)
  assert.doesNotMatch(reader, /\.pt-path \{[^}]*font-size|\.shelf-search:focus-within \{|\.path-trigger:hover/)
})

test('reader search fields share the same nested control material and focus state', async () => {
  const css = await source('styles/materials.css')
  const material = css.match(/:root\[data-theme\] :is\(\.task-toolbar \.task-search, \.reader-search-field\) \{([^}]+)\}/)?.[1] ?? ''
  assert.match(material, /--material-fill: var\(--glass-control-fill\)/)
  assert.match(material, /--material-filter: none/)
  assert.match(css, /:root\[data-theme\] \.reader-search-field \{ --material-sheen: none; \}/)
  assert.match(css, /:is\([^)]*\.reader-search-field[^)]*\):focus-within \{\s*--material-border: var\(--accent-border\)/)
})

test('visual unification preserves separate shelf filtering and directory search behavior', async () => {
  const reader = await source('views/Reader.vue')
  assert.match(reader, /v-model="shelfQuery"[^>]*aria-label="搜索书籍"/)
  assert.match(reader, /:query="shelfQuery"/)
  assert.match(reader, /class="[^"]*reader-file-search-trigger[^>]*@click="openFileSearch"/)
  assert.match(reader, /v-model="fileSearchQuery"/)
  assert.match(reader, /aria-label="清除搜索" @click="shelfQuery = ''"/)
  assert.match(reader, /aria-label="清除文件搜索" @click="fileSearchQuery = ''"/)
})
