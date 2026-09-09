import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const frontendRoot = new URL('../', import.meta.url)

async function source(path: string) {
  return readFile(new URL(path, frontendRoot), 'utf8')
}

test('reader keeps navigation chrome until immersive mode and balances its bottom toolbar', async () => {
  const [reader, bookshelf] = await Promise.all([
    source('src/views/Reader.vue'),
    source('src/components/reader/BookshelfView.vue'),
  ])

  assert.doesNotMatch(reader, /is-read-view\.has-document \.reader-topbar/)
  assert.match(reader, /class="mobile-toolbar-side mobile-toolbar-left"/)
  assert.match(reader, /class="mobile-toolbar-side mobile-toolbar-right"/)
  assert.match(reader, /ref="bookshelfRef"/)
  assert.match(bookshelf, /defineExpose\(\{ openAdd \}\)/)
})

test('reader mobile toolbar keeps folder and fullscreen on the left with full-height controls', async () => {
  const reader = await source('src/views/Reader.vue')
  const leftGroup = reader.match(/class="mobile-toolbar-side mobile-toolbar-left"([\s\S]*?)<\/div>/)?.[1] ?? ''
  const rightGroup = reader.match(/class="mobile-toolbar-side mobile-toolbar-right"([\s\S]*?)<\/div>/)?.[1] ?? ''

  assert.match(leftGroup, /<FolderOpened \/>[\s\S]*<FullScreen \/>/)
  assert.match(rightGroup, /<Minus \/>[\s\S]*<Plus \/>/)
  assert.match(rightGroup, /<Menu \/>[\s\S]*<Search \/>/)
  assert.match(reader, /v-model="fileSearchQuery"/)
  assert.match(reader, /aria-label="搜索当前目录文件"/)
  assert.match(reader, /\.reader-mobile-toolbar button \{[\s\S]*?height: 52px;/)
  assert.match(reader, /\.reader-mobile-toolbar button \.el-icon \{ font-size: 24px; \}/)
})

test('timeline composes from an icon beside mobile search', async () => {
  const timeline = await source('src/views/Timeline.vue')
  const searchAt = timeline.indexOf('class="search-box glass-surface"')
  const composeAt = timeline.indexOf('class="mobile-compose-action"')
  const filterAt = timeline.indexOf('class="mobile-filter-summary glass-surface"')

  assert.match(timeline, /class="mobile-compose-action"/)
  assert.match(timeline, /aria-label="写小记"/)
  assert.ok(searchAt >= 0 && searchAt < composeAt && composeAt < filterAt)
  assert.match(timeline, /class="mobile-filter-summary glass-surface"[\s\S]*?<MoreFilled \/>/)
  assert.match(timeline, /\.memo-time \{ position: absolute; top: 14px; left: 16px; right: auto;/)
})

test('task detail owns a fixed action row and child detail omits redundant root return', async () => {
  const tasks = await source('src/views/Tasks.vue')

  assert.match(tasks, /class="mobile-detail-scroll"/)
  assert.doesNotMatch(tasks, /返回任务/)
  assert.match(tasks, /\.page-create \{ display: none; \}/)
})

test('all modules opens as a bottom sheet and mobile titles use stronger hierarchy', async () => {
  const app = await source('src/App.vue')

  assert.match(app, /transform: translate3d\(0, 100%, 0\)/)
  assert.match(app, /\.mobile-page-title \{[\s\S]*?font-size: 19px;[\s\S]*?font-weight: 700;/)
})
