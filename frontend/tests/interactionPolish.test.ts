import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('only a page with secondary navigation may contract the phone dock', async () => {
  const [app, dock] = await Promise.all([source('App.vue'), source('components/MobileDock.vue')])
  assert.match(app, /isMobile\.value && !mobileFocusMode\.value && !!mobileSubnav\.value\?\.items\.length/)
  assert.match(dock, /dockGeometry\(effectiveProgress\.value\)/)
  assert.match(dock, /props\.subnav\?\.items\.length \? props\.progress : 0/)
})

test('timeline dates share one explicit axis and phone filters do not shift the feed', async () => {
  const timeline = await source('views/Timeline.vue')
  assert.match(timeline, /--time-nav-axis:/)
  assert.match(timeline, /\.day-dot\s*\{[^}]*left: var\(--time-nav-axis\)/)
  assert.match(timeline, /\.day-line\s*\{[^}]*left: var\(--time-nav-axis\)/)
  assert.match(timeline, /<MotionModal v-model="mobileFiltersOpen" aria-label="时间筛选">/)
  assert.doesNotMatch(timeline, /<Transition name="filter-panel">/)
})

test('original photo loading is accessible but quiet and its footer owns both rows', async () => {
  const timeline = await source('views/Timeline.vue')
  assert.match(timeline, /class="viewer-spinner" role="status" aria-label="加载原图"/)
  assert.doesNotMatch(timeline, /正在加载原图/)
  const footer = timeline.match(/class="viewer-footer"([\s\S]*?)<\/footer>/)?.[1] ?? ''
  assert.match(footer, /class="viewer-counter"/)
  assert.match(footer, /class="viewer-controls"/)
  assert.match(timeline, /\.image-viewer-overlay\s*\{[^}]*grid-template-rows:/)
})

test('task sub-dock exposes icon actions while desktop retains inline queries', async () => {
  const [tasks, dock, nav] = await Promise.all([source('views/Tasks.vue'), source('components/MobileDock.vue'), source('composables/useMobileSubnav.ts')])
  assert.match(tasks, /id: 'search', label: '搜索任务', icon: Search/)
  assert.match(tasks, /id: 'filter', label: '筛选任务', icon: Filter/)
  assert.match(tasks, /<MotionModal v-model="queryPanelOpen"/)
  assert.match(dock, /<component :is="item.icon"/)
  assert.match(dock, /:aria-label="item.label"/)
  assert.match(nav, /icon\?: Component/)
})

test('phone bookshelf fills three equal columns without cramping cover titles', async () => {
  const shelf = await source('components/reader/BookshelfView.vue')
  assert.match(shelf, /grid-template-columns: repeat\(3, minmax\(0, 1fr\)\)/)
  assert.match(shelf, /aspect-ratio: 177 \/ 230/)
  assert.match(shelf, /\.book-cover\s*\{[^}]*width: 100%;[^}]*min-width: 0;/)
  assert.match(shelf, /\.bc-title\s*\{[^}]*padding-right: 10px;/)
  assert.match(shelf, /class="bc-more"[^>]*aria-label=/)
})

test('common sheets stay present for the entire physical exit and share interruptible motion', async () => {
  for (const path of ['components/motion/MotionModal.vue', 'components/motion/MotionDrawer.vue']) {
    const surface = await source(path)
    assert.match(surface, /useSurfaceMotion/)
    assert.match(surface, /v-if="present"/)
    assert.match(surface, /useModalEnvironment\(\(\) => present\.value/)
    assert.match(surface, /:inert="!modelValue \? true : undefined"/)
    assert.doesNotMatch(surface, /<Transition/)
  }
})
