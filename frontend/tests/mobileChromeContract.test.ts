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

test('bookshelf moves phone creation above the dock and retains its desktop toolbar action', async () => {
  const reader = await source('src/views/Reader.vue')
  const bookshelf = await source('src/components/reader/BookshelfView.vue')
  assert.equal((reader.match(/aria-label="添加书籍"/g) || []).length, 1)
  assert.equal((bookshelf.match(/aria-label="添加书籍"/g) || []).length, 0)
  assert.match(reader, /class="reader-shelf-add ui-icon-action"[^>]*@click="bookshelfRef\?\.openAdd\(\)"/)
  assert.match(reader, /<button[^>]*v-if="!isMobile"[^>]*class="reader-shelf-add ui-icon-action"/)
  assert.match(reader, /<MobilePageAction\s+v-if="isMobile && viewMode === 'shelf'"[\s\S]*?label="添加书籍"[\s\S]*?@click="bookshelfRef\?\.openAdd\(\)"/)
  assert.doesNotMatch(bookshelf, /class="shelf-add|\.shelf-add\s*\{/)
  assert.match(bookshelf, /aria-label="添加第一本书"/)
  assert.match(bookshelf, /@media \(max-width: 768px\)[\s\S]*?\.shelf-empty-add\s*\{\s*display:\s*none/)
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

test('reader file search reuses the original centered path dialog instead of a drawer', async () => {
  const reader = await source('src/views/Reader.vue')
  const pathOverlay = reader.match(/class="path-overlay"([\s\S]*?)<\/transition>\s*<\/div>\s*<\/transition>/)?.[1] ?? ''

  assert.match(pathOverlay, /v-model="fileSearchQuery"/)
  assert.match(pathOverlay, /class="reader-search-results"/)
  assert.doesNotMatch(reader, /<MotionDrawer v-model="fileSearchDrawer"/)
  assert.match(reader, /function openFileSearch\(\) \{[\s\S]*?fileSearchOpen\.value = true/)
})

test('reader routes folder changes through the shelf and keeps read controls focused', async () => {
  const reader = await source('src/views/Reader.vue')

  assert.match(reader, /class="path-trigger reader-file-search-trigger reader-search-field"[^>]*@click="openFileSearch"/)
  assert.match(reader, /class="icon-btn reader-fullscreen-btn"/)
  assert.match(reader, /@click="openMobileFilePicker"/)
  assert.match(reader, /function openFileSearch\(\) \{[\s\S]*?if \(!rootPath\.value\) \{[\s\S]*?changeView\('shelf'\)/)
  assert.match(reader, /function openMobileFilePicker\(\) \{[\s\S]*?if \(rootPath\.value\) treeDrawer\.value = true[\s\S]*?else changeView\('shelf'\)/)
  assert.doesNotMatch(reader, /reader-folder-btn|drawer-folder-action|openHistoryOverlay|更换文件夹/)
  assert.match(reader, /@media \(max-width: 768px\)[\s\S]*?\.reader-file-search-trigger,[\s\S]*?\.reader-fullscreen-btn \{ display: none !important; \}/)
})

test('timeline composes from its floating phone action and gives search the freed width', async () => {
  const timeline = await source('src/views/Timeline.vue')
  const searchAt = timeline.indexOf('class="search-box glass-surface"')
  const filterAt = timeline.indexOf('class="mobile-filter-summary glass-surface"')

  assert.doesNotMatch(timeline, /mobile-compose-action/)
  assert.match(timeline, /<MobilePageAction v-if="isMobile" :icon="Plus" label="写小记" @click="openCreateDialog"/)
  assert.match(timeline, /aria-label="写小记"/)
  assert.ok(searchAt >= 0 && searchAt < filterAt)
  assert.match(timeline, /grid-template-columns: minmax\(0, 1fr\) var\(--tap-target\);/)
  assert.doesNotMatch(timeline, /grid-template-columns: minmax\(0, 1fr\) var\(--tap-target\) var\(--tap-target\)/)
  assert.match(timeline, /\.mobile-filter-summary\s*\{\s*grid-column: 2;/)
  assert.match(timeline, /class="mobile-filter-summary glass-surface"[\s\S]*?<MoreFilled \/>/)
  assert.match(timeline, /\.memo-time \{ position: absolute; top: 14px; left: 16px; right: auto;/)
})

test('task detail owns a fixed action row and child detail omits redundant root return', async () => {
  const tasks = await source('src/views/Tasks.vue')
  const mobile = await source('src/components/tasks/MobileTaskDetail.vue')

  assert.match(mobile, /class="mobile-task-detail-scroll"/)
  assert.match(mobile, /class="mobile-detail-context" aria-label="详情栏目与新增"/)
  const childDialog = tasks.match(/<MotionModal v-if="!isPhone" v-model="childDetailOpen"[\s\S]*?<\/MotionModal>/)?.[0] || ''
  assert.doesNotMatch(childDialog, /返回任务/)
  assert.match(mobile, /'返回任务列表'/)
  assert.doesNotMatch(tasks, /class="page-create"/)
})

test('phone navigation uses direct destinations instead of a module sheet and keeps clear titles', async () => {
  const app = await source('src/App.vue')

  assert.doesNotMatch(app, /mobile-overlay|open-modules|全部功能/)
  assert.match(app, /<MobileDock/)
  assert.match(app, /\.mobile-page-title \{[\s\S]*?font-size: 19px;[\s\S]*?font-weight: 700;/)
})
