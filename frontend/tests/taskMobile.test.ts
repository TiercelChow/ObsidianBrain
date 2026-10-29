import assert from 'node:assert/strict'
import test from 'node:test'
import { readFile } from 'node:fs/promises'
import type { TaskSummary } from '../src/api/tasks.ts'
import type { TaskActivityEntry } from '../src/utils/taskActivity.ts'
import { directTaskChildren, mobileTaskMeta, partitionTaskActivity } from '../src/utils/taskMobile.ts'

const task = (id: string, patch: Partial<TaskSummary> = {}) => ({ id, root_id: 'root', parent_id: 'root', position: 0, created_at: '2026-10-20T00:00:00Z', kind: 'long', status: 'planned', importance: 'normal', start_date: '2026-10-20', end_date: '2026-10-27', ...patch }) as TaskSummary

test('mobile rows keep type and timing, and never disguise a completed or cancelled task as overdue', () => {
  assert.equal(mobileTaskMeta(task('one'), '2026-10-27'), '长期 · 今天截止')
  assert.equal(mobileTaskMeta(task('done', { kind: 'short', status: 'completed' }), '2026-10-30'), '待办 · 已完成')
  assert.equal(mobileTaskMeta(task('cancel', { status: 'cancelled' }), '2026-10-30'), '长期 · 已取消')
})

test('mobile drill-down shows only direct children, stable-sorted without mutating the document', () => {
  const nodes = [task('later', { position: 2 }), task('grandchild', { parent_id: 'first' }), task('root', { parent_id: null }), task('first', { position: 0 }), task('tie', { position: 0, created_at: '2026-10-21T00:00:00Z' })]
  const before = nodes.map(node => node.id)
  assert.deepEqual(directTaskChildren(nodes, 'root').map(node => node.id), ['first', 'tie', 'later'])
  assert.deepEqual(directTaskChildren(nodes, 'first').map(node => node.id), ['grandchild'])
  assert.deepEqual(directTaskChildren(nodes, 'missing'), [])
  assert.deepEqual(nodes.map(node => node.id), before)
})

test('progress is primary but audit notes and original order are preserved, including audit-only tasks', () => {
  const entries = [{ id: 'a', type: 'audit', note: '关闭说明' }, { id: 'p', type: 'progress', note: '进展正文' }, { id: 'b', type: 'audit', note: null }] as TaskActivityEntry[]
  const parts = partitionTaskActivity(entries)
  assert.deepEqual(parts.progress.map(item => item.id), ['p'])
  assert.deepEqual(parts.audit.map(item => item.id), ['a', 'b'])
  assert.equal(parts.audit[0].note, '关闭说明')
  assert.deepEqual(partitionTaskActivity([]), { progress: [], audit: [] })
  assert.equal(partitionTaskActivity(entries.slice(0, 1)).audit.length, 1)
})

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')
test('calendar month heading omits the redundant summary and remains prominent on phone and desktop', async () => {
  const calendar = await source('components/tasks/TaskCalendar.vue')
  const heading = calendar.match(/<div class="calendar-heading">([\s\S]*?)<\/div>/)?.[1] || ''
  assert.match(heading, /<h2>\{\{ monthTitle\(anchor\) \}\}<\/h2>/)
  assert.doesNotMatch(heading, /<span|项安排/)
  assert.doesNotMatch(calendar, /currentMonthTaskCount|\.calendar-heading span/)
  assert.match(calendar, /\.calendar-heading h2\s*\{[^}]*font-size:\s*26px[^}]*font-weight:\s*700[^}]*line-height:\s*1\.25/)
  const phone = calendar.slice(calendar.indexOf('@media (max-width: 768px)'))
  assert.match(phone, /\.calendar-heading h2\s*\{[^}]*font-size:\s*22px/)
  assert.doesNotMatch(phone, /\.is-compact \.calendar-heading h2\s*\{/)
})

test('phone Dock uses one icon view toggle and owns root task creation with the selected calendar day', async () => {
  const page = await source('views/Tasks.vue')
  const dock = page.slice(page.indexOf('useMobileSubnav(()'), page.indexOf("const searchQuery"))
  assert.match(dock, /id: 'view'/)
  assert.doesNotMatch(dock, /\.map\(mode/)
  assert.match(dock, /切换到日历视图/)
  assert.match(dock, /切换到任务视图/)
  assert.match(dock, /icon: viewMode\.value === 'tasks' \? Calendar : List/)
  assert.match(dock, /id: 'create'.*icon: Plus.*openCreate\(viewMode\.value === 'calendar' \? selectedDate\.value : today\)/)
  assert.match(page, /<div v-else class="task-workspace"/)
  const list = await source('components/tasks/MobileTaskList.vue')
  assert.doesNotMatch(list, /@click=.*create|我的任务|glass-surface/)
  assert.match(list, /TaskStatusMark/)
})

test('phone child detail and editor share one modal; both dates use the existing system wheel', async () => {
  const page = await source('views/Tasks.vue')
  assert.match(page, /<MotionModal v-if="!isPhone" v-model="childDetailOpen"/)
  assert.match(page, /<MotionModal v-model="taskSurfaceOpen"/)
  assert.match(page, /v-if="isPhone && !sheetOpen && drawerNode"/)
  assert.match(page, /class="task-date-fields field full"/)
  assert.match(page, /v-model="form.start_date"[\s\S]*?type="date"/)
  assert.match(page, /v-model="form.end_date"[\s\S]*?type="date"/)
  const detail = await source('components/tasks/MobileTaskDetail.vue')
  assert.match(detail, /directTaskChildren/)
  assert.match(detail, /aria-label="详情栏目与新增"/)
  assert.match(detail, /:label="section === 'breakdown' \? '添加子任务' : '记录进展'"/)
  assert.doesNotMatch(detail, /TaskTree/)
})

test('list scroll restoration waits until the route has restored the non-focused viewport', async () => {
  const [page, list] = await Promise.all([source('views/Tasks.vue'), source('components/tasks/MobileTaskList.vue')])
  assert.match(page, /:restore-ready="!route.query.task"/)
  assert.match(page, /if \(store.selectedTaskId \|\| route.query.task\) return/)
  assert.match(list, /!props.loading && props.restoreReady/)
  assert.match(list, /watch\(\[\(\) => props.loading, \(\) => props.restoreReady\]/)
})

test('short task completion keeps a closure note without an inapplicable child cascade option', async () => {
  const page = await source('views/Tasks.vue')
  const cascade = page.match(/<label[^>]*v-if="[^"]*isTerminalStatus[^"]*"[^>]*class="check-field"/)?.[0] || ''
  assert.match(cascade, /targetNode\?\.kind === 'long'/)
  assert.match(page, /v-model="statusForm.note"/)
})

test('phone task surfaces consume the existing palette rather than undefined or fixed warning colors', async () => {
  for (const path of ['components/tasks/MobileTaskList.vue', 'components/tasks/MobileTaskDetail.vue', 'components/tasks/MobileTaskActivity.vue']) {
    const text = await source(path)
    assert.doesNotMatch(text, /--accent-bg|--color-danger|#[0-9a-f]{3,8}\b/i)
  }
  const page = await source('views/Tasks.vue')
  assert.doesNotMatch(page, /--accent-bg/)
  assert.match(page, /\.sheet-error\s*\{[^}]*color:var\(--danger\)/)
})

test('phone tasks use separate light cards with progress subordinate to the title, without a second glass container', async () => {
  const list = await source('components/tasks/MobileTaskList.vue')
  assert.match(list, /class="mobile-task-row"[^>]*data-glass="subcard"/)
  const copy = list.slice(list.indexOf('class="mobile-task-copy"'), list.indexOf('class="ui-chevron mobile-row-chevron"'))
  assert.match(copy, /class="mobile-task-meta-row"[\s\S]*class="mobile-task-percent"/)
  assert.match(copy, /class="mobile-card-progress"/)
  assert.match(list, /\.mobile-task-rows\s*\{[^}]*display:grid;[^}]*gap:10px/)
  assert.match(list, /\.mobile-task-row\s*\{[^}]*border-radius:18px/)
  assert.doesNotMatch(list, /border-bottom:1px|mobile-task-row:last-child|v-glass-lens/)
})

test('phone detail puts metadata below the title and keeps its only edit action alongside the title', async () => {
  const detail = await source('components/tasks/MobileTaskDetail.vue')
  const nav = detail.match(/<header class="mobile-task-detail-nav"[^>]*>([\s\S]*?)<\/header>/)?.[1] || ''
  assert.doesNotMatch(nav, /EditPen|编辑任务/)
  const titleRow = detail.match(/<div class="mobile-detail-title-row">([\s\S]*?)<\/div>/)?.[1] || ''
  assert.match(titleRow, /<h2>\{\{ node.title \}\}<\/h2>/)
  assert.match(titleRow, /:icon="EditPen" label="编辑任务" @click="\$emit\('edit', node\)"/)
  assert.equal((detail.match(/label="编辑任务"/g) || []).length, 1)
  const title = detail.indexOf('class="mobile-detail-title-row"')
  const metadata = detail.indexOf('class="mobile-detail-meta"')
  const dates = detail.indexOf('class="mobile-detail-dates"')
  assert.ok(title >= 0 && title < metadata && metadata < dates)
  assert.match(detail, /\.mobile-detail-title-row\s*\{[^}]*display:flex;[^}]*align-items:flex-start/)
  assert.match(detail, /\.mobile-detail-title-row h2\s*\{[^}]*min-width:0/)
})

test('phone task detail has a truly centered page title and CSS-only scroll edges above and below its content', async () => {
  const detail = await source('components/tasks/MobileTaskDetail.vue')
  assert.match(detail, /<h1 class="mobile-detail-page-title">任务详情<\/h1>/)
  assert.match(detail, /class="mobile-task-detail-nav"[^>]*v-glass-lens="'css'"[^>]*data-glass="scroll-edge"/)
  assert.match(detail, /class="mobile-detail-context"[^>]*v-glass-lens="'css'"[^>]*data-glass="scroll-edge"[^>]*data-glass-edge="bottom"/)
  assert.match(detail, /\.mobile-task-detail-nav\s*\{[^}]*position:absolute;[^}]*grid-template-columns:44px minmax\(0,1fr\) 44px/)
  assert.match(detail, /\.mobile-detail-page-title\s*\{[^}]*text-align:center;[^}]*font-size:19px/)
  const scroll = detail.match(/\.mobile-task-detail-scroll\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(scroll, /padding:[^;]*var\(--task-detail-header-height\)[^;]*var\(--task-detail-footer-height\)[^;]*var\(--glass-scroll-edge-fade\)/)
  assert.match(detail, /class="mobile-parent-path"/)
  assert.match(detail, /:class="\{ 'header-scrolled': scrolled \}"/)
  assert.match(detail, /scrolled.value = .*scrollTop > 8/)
  assert.match(detail, /emit\('scroll', event\)/)
})
