import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('runtime action layout targets action groups, never Element Plus switch internals', async () => {
  const css = await source('views/knowledge/WikiSettings.vue')
  assert.doesNotMatch(css, /\.runtime-actions > div\s*\{/)
  assert.match(css, /\.runtime-actions > div:not\(\.el-switch\)/)
})

test('touch book operations use one explicit 44px target instead of tiny hover actions', async () => {
  const css = await source('components/reader/BookshelfView.vue')
  assert.match(css, /@media \(max-width: 768px\), \(hover: none\)/)
  assert.match(css, /\.bc-more\s*\{[^}]*width: var\(--tap-target\);[^}]*height: var\(--tap-target\)/)
})

test('custom fields and confirmations share focus, disabled and keyboard-safe sheet geometry', async () => {
  const css = await source('styles/controls.css')
  for (const selector of ['.field input:not', '.field textarea', '.glass-textarea', '.document-editor textarea', '.el-message-box', '.el-dialog__headerbtn']) assert.ok(css.includes(selector), selector)
  assert.ok(css.includes(':disabled'))
  assert.ok(css.includes('align-items: flex-end'))
  assert.ok(css.includes('min-height: 0'))
  assert.ok(css.includes('CanvasText'))
})

test('knowledge and home statuses use theme-aware semantic labels rather than fixed light-theme colors', async () => {
  const [knowledge, home] = await Promise.all([source('styles/knowledge.css'), source('views/Home.vue')])
  for (const token of ['glass-success-label', 'glass-warning-label', 'glass-danger-label']) assert.ok(knowledge.includes(`var(--${token})`), token)
  assert.match(home, /\.is-warning \{ color:var\(--glass-warning-label/)
})

test('repo styles cannot change the presentation of dialogs in other modules', async () => {
  const repo = await source('views/CodeRepo.vue')
  assert.ok(!repo.includes('<style>'), 'no lazy route global overrides')
  assert.ok(repo.includes('minmax(min(100%, 320px), 1fr)'))
})

test('legacy inline and per-theme widget recipes no longer compete with the shared system', async () => {
  const [index, app, css] = await Promise.all([readFile(new URL('../index.html', import.meta.url), 'utf8'), source('App.vue'), source('styles/controls.css')])
  assert.ok(!index.includes('<style>'))
  assert.ok(!app.includes(':root[data-theme="dark"] .el-input__wrapper'))
  assert.ok(css.includes('.glass-picker .el-date-table'))
  assert.ok(css.includes('--visual-viewport-height'))
})

test('page-owned sheets inherit the wrapper height so fixed actions are not cropped', async () => {
  const modal = await source('components/motion/MotionModal.vue')
  assert.ok(modal.includes(':deep(:not(.motion-modal__handle))'))
  assert.ok(modal.includes('max-height: inherit'))
  const tasks = await source('views/Tasks.vue')
  assert.ok(tasks.includes('.column-heading button, .progress-column-header button { display: none; }'))
  const tree = await source('components/tasks/TaskTree.vue')
  assert.ok(tree.includes('grid-template-columns: 44px minmax(0, 1fr)'))
  assert.ok(tree.includes('.status-orb { display: none; }'))
})

test('narrow calendars stay inside their page and today uses the theme contrast color', async () => {
  const calendar = await source('components/tasks/TaskCalendar.vue')
  assert.doesNotMatch(calendar, /width: calc\(100% \+ 30px\)/)
  assert.ok(calendar.includes('color: var(--accent-contrast)'))
  assert.doesNotMatch(calendar, /\.calendar-day\.today[^}]*color: white/)
})

test('tablet timeline filters wrap before the date editor can leave the viewport', async () => {
  const timeline = await source('views/Timeline.vue')
  assert.ok(timeline.includes('@media (min-width: 769px) and (max-width: 1100px)'))
  assert.match(timeline, /\.filter-right\s*\{[^}]*flex: 1 1 100%;[^}]*flex-wrap: wrap/)
})

test('range endpoints and today selection keep contrast in the shared calendar', async () => {
  const css = await source('styles/controls.css')
  for (const state of ['start-date', 'end-date', 'today.current']) {
    assert.ok(css.includes(`td.${state} .el-date-table-cell__number`), state)
  }
})

test('task urgency and completed markers share semantic text colors in all themes', async () => {
  const [app, tree, tasks] = await Promise.all([source('App.vue'), source('components/tasks/TaskTree.vue'), source('views/Tasks.vue')])
  assert.match(app, /\.task-pill.status-blocked[^}]*color: var\(--warning\)/)
  assert.match(app, /\.task-pill.status-completed[^}]*color: var\(--success\)/)
  assert.match(tree, /\.importance-urgent[^}]*color: var\(--danger\)/)
  assert.match(tasks, /\.danger \{ color: var\(--danger\)/)
})

test('single-column Wiki navigation resets detail scroll and restores the list position', async () => {
  const wiki = await source('views/knowledge/WikiWorkspace.vue')
  assert.ok(wiki.includes('@click="returnToEntryList"'))
  assert.ok(wiki.includes('entryListScrollTop = scroller?.scrollTop ?? 0'))
  assert.ok(wiki.includes('scroller.scrollTop = entryListScrollTop'))
  assert.ok(wiki.includes('entryDetailRef.value.scrollTop = 0'))
  assert.ok(wiki.includes(':data-entry-id="entry.id"'))
  assert.ok(wiki.includes('focus({ preventScroll: true })'))
  assert.match(wiki, /renderedHtml.value = html[\s\S]*?detailLoading.value = false\s+await nextTick\(\)\s+if \(markdownRef.value\) await enhance/)
  assert.ok(wiki.includes('sequence !== entryReadSequence || selectedEntry.value?.id !== entry.id'))
})
