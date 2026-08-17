<template>
  <div class="tasks-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">任务中枢</h1>
        <p class="page-subtitle">让临期待办轻巧落地，让长期目标稳步生长</p>
      </div>
      <div class="header-actions">
        <button class="glass-button secondary" type="button" :disabled="store.syncing" @click="syncNow">
          <span :class="{ spinning: store.syncing }">↻</span>
          {{ store.syncing ? '同步中' : '同步 Obsidian' }}
        </button>
        <button class="glass-button primary" type="button" @click="openCreate()">＋ 新建任务</button>
      </div>
    </header>

    <section class="task-toolbar glass-surface">
      <div class="view-switch" aria-label="视图切换">
        <span class="switch-indicator" :class="viewMode"></span>
        <button type="button" :class="{ active: viewMode === 'tasks' }" @click="changeView('tasks')">任务</button>
        <button type="button" :class="{ active: viewMode === 'calendar' }" @click="changeView('calendar')">日历</button>
      </div>
      <label class="task-search">
        <span>⌕</span>
        <input v-model="searchQuery" type="search" placeholder="搜索标题或描述" />
      </label>
      <div class="filters">
        <select v-model="kindFilter" aria-label="任务类型">
          <option value="all">全部类型</option>
          <option value="short">短期待办</option>
          <option value="long">长期任务</option>
        </select>
        <select v-model="statusFilter" aria-label="任务状态">
          <option value="active">进行中的</option>
          <option value="all">全部状态</option>
          <option v-for="option in statusOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
        </select>
      </div>
    </section>

    <Transition name="error-banner">
      <div v-if="store.error" class="error-banner" role="alert">
        <span>{{ store.error }}</span>
        <button type="button" @click="store.clearError">关闭</button>
      </div>
    </Transition>

    <div v-if="viewMode === 'tasks'" class="task-workspace" :class="{ 'detail-open': !!store.selectedDetail }">
      <aside class="task-list-panel glass-surface">
        <div class="panel-heading">
          <div>
            <strong>我的任务</strong>
            <span>{{ store.tasks.length }} 项</span>
          </div>
          <button type="button" aria-label="新建任务" @click="openCreate()">＋</button>
        </div>

        <div v-if="store.loading" class="task-skeletons" aria-label="加载中">
          <span v-for="index in 5" :key="index"></span>
        </div>
        <div v-else-if="store.tasks.length" class="task-list">
          <section v-for="group in groupedTasks" :key="group.key" class="task-group">
            <div class="task-group-title"><span>{{ group.label }}</span><small>{{ group.tasks.length }}</small></div>
            <TransitionGroup name="task-card" tag="div" class="task-group-items">
              <button
                v-for="task in group.tasks"
                :key="task.id"
                type="button"
                class="task-card"
                :class="[
                  `importance-${task.importance}`,
                  { selected: store.selectedTaskId === task.id, overdue: task.derived.overdue },
                ]"
                @click="openTask(task.id)"
              >
                <span class="card-accent"></span>
                <span class="card-main">
                  <span class="card-topline">
                    <strong>{{ task.title }}</strong>
                    <span class="kind-badge">{{ task.kind === 'short' ? '待办' : '长期' }}</span>
                  </span>
                  <span class="card-description">{{ task.description || '暂无描述' }}</span>
                  <span class="card-footer">
                    <span :class="{ danger: task.derived.overdue }">{{ formatTaskDateRange(task.start_date, task.end_date) }}</span>
                    <span>{{ statusLabel(task.status) }}</span>
                  </span>
                  <span v-if="task.kind === 'long'" class="mini-progress">
                    <i :style="{ width: `${task.progress_percent}%` }"></i>
                  </span>
                </span>
                <span class="card-chevron">›</span>
              </button>
            </TransitionGroup>
          </section>
        </div>
        <div v-else class="empty-list">
          <div class="empty-orb">✓</div>
          <strong>从一件小事开始</strong>
          <p>新建短期待办，或拆解一个长期目标。</p>
          <button type="button" @click="openCreate()">新建第一个任务</button>
        </div>
      </aside>

      <main class="task-detail-panel glass-surface">
        <div v-if="store.detailLoading" class="detail-loading">
          <span></span><span></span><span></span>
        </div>
        <template v-else-if="detail && activeNode">
          <div class="mobile-detail-nav">
            <button type="button" @click="closeMobileDetail">‹ 任务列表</button>
            <span>{{ detail.root.kind === 'short' ? '短期待办' : '长期任务' }}</span>
          </div>

          <header class="detail-header">
            <div class="detail-title-group">
              <div class="detail-kicker">
                <span class="status-dot" :class="`status-${activeNode.status}`"></span>
                {{ statusLabel(activeNode.status) }} · {{ importanceLabel(activeNode.importance) }}
              </div>
              <h2>{{ activeNode.title }}</h2>
              <p>{{ activeNode.description || '这个任务还没有描述。' }}</p>
            </div>
            <div class="detail-actions">
              <button type="button" @click="openStatus(activeNode)">更改状态</button>
              <button type="button" @click="openEdit(activeNode)">编辑</button>
              <button type="button" class="more-button" aria-label="归档任务" @click="archiveCurrent">···</button>
            </div>
          </header>

          <div class="detail-facts">
            <div><span>开始</span><strong>{{ activeNode.start_date }}</strong></div>
            <div><span>结束</span><strong>{{ activeNode.end_date }}</strong></div>
            <div><span>优先级</span><strong>{{ importanceLabel(activeNode.importance) }}</strong></div>
            <div><span>位置</span><strong>{{ detail.storage_path }}</strong></div>
          </div>

          <section v-if="detail.root.kind === 'long'" class="progress-overview">
            <div class="section-heading">
              <div>
                <span>整体进展</span>
                <strong>{{ detail.progress_percent }}%</strong>
              </div>
              <small>{{ detail.completed_leaf_count }} / {{ detail.effective_leaf_count }} 个叶子任务完成</small>
            </div>
            <div class="progress-track"><i :style="{ width: `${detail.progress_percent}%` }"></i></div>
          </section>

          <section v-if="detail.root.kind === 'long'" class="detail-section task-breakdown">
            <div class="section-heading">
              <div><span>任务拆解</span><strong>{{ detail.tasks.length }} 个节点</strong></div>
              <button type="button" @click="openSubtask(activeNode)">＋ 添加子任务</button>
            </div>
            <TaskTree
              :tasks="detail.tasks"
              :selected-id="activeNodeId"
              @select="activeNodeId = $event"
              @add="openSubtask"
              @progress="openProgress"
              @status="openStatus"
              @move="openMove"
              @reorder="quickMove"
            />
          </section>

          <section class="detail-section activity-section">
            <div class="section-heading">
              <div><span>进展与记录</span><strong>{{ detail.progress.length + detail.audit.length }} 条</strong></div>
              <button type="button" @click="openProgress(activeNode)">＋ 添加进展</button>
            </div>
            <div v-if="activity.length" class="activity-list">
              <article v-for="item in activity" :key="item.id" class="activity-item">
                <span class="activity-dot" :class="item.type"></span>
                <div>
                  <strong>{{ item.title }}</strong>
                  <p v-if="item.note">{{ item.note }}</p>
                  <time>{{ formatTimestamp(item.time) }}</time>
                </div>
              </article>
            </div>
            <button v-else type="button" class="activity-empty" @click="openProgress(activeNode)">记录第一条进展</button>
          </section>
        </template>
        <div v-else class="empty-detail">
          <div class="detail-illustration"><span></span><i></i></div>
          <strong>选择一项任务</strong>
          <p>在这里查看计划、拆解与每一步进展。</p>
        </div>
      </main>
    </div>

    <TaskCalendar
      v-else
      :anchor="calendarAnchor"
      :selected-date="selectedDate"
      :tasks="store.calendarTasks"
      :loading="store.calendarLoading"
      @shift="shiftCalendar"
      @today="goToday"
      @select-date="selectCalendarDate"
      @open-task="openFromCalendar"
      @create="openCreate"
    />

    <MotionModal v-model="sheetOpen" :aria-label="sheetTitle">
      <form class="task-sheet glass-surface-heavy" @submit.prevent="submitSheet">
        <header class="sheet-header">
          <div>
            <span>{{ sheetEyebrow }}</span>
            <h3>{{ sheetTitle }}</h3>
          </div>
          <button type="button" aria-label="关闭" @click="sheetOpen = false">×</button>
        </header>

        <div v-if="sheetMode === 'progress'" class="sheet-body">
          <label class="field full">
            <span>进展说明</span>
            <textarea v-model="progressForm.note" rows="5" required placeholder="记录发生了什么、下一步是什么…"></textarea>
          </label>
          <label class="check-field">
            <input v-model="progressForm.includePercent" type="checkbox" />
            <span>同时更新明确完成度</span>
          </label>
          <label v-if="progressForm.includePercent" class="field full">
            <span>完成度</span>
            <div class="range-field">
              <input v-model.number="progressForm.percent" type="range" min="0" max="100" step="5" />
              <output>{{ progressForm.percent }}%</output>
            </div>
          </label>
        </div>

        <div v-else-if="sheetMode === 'status'" class="sheet-body">
          <label class="field full">
            <span>任务状态</span>
            <select v-model="statusForm.status">
              <option v-for="option in statusSheetOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
            </select>
          </label>
          <label v-if="isTerminalStatus" class="field full">
            <span>关闭说明（可选）</span>
            <textarea v-model="statusForm.note" rows="4" placeholder="总结结果、原因或后续安排…"></textarea>
          </label>
          <label v-if="targetNode?.role === 'root' && isTerminalStatus" class="check-field">
            <input v-model="statusForm.cascade" type="checkbox" />
            <span>同时关闭所有未完成的子任务</span>
          </label>
        </div>

        <div v-else-if="sheetMode === 'move'" class="sheet-body">
          <label class="field full">
            <span>新的上级任务</span>
            <select v-model="moveForm.parentId" required>
              <option v-for="candidate in moveCandidates" :key="candidate.id" :value="candidate.id">{{ candidate.title }}</option>
            </select>
          </label>
          <p class="sheet-hint">移动后，任务会排在新上级的子任务末尾。</p>
        </div>

        <div v-else class="sheet-body form-grid">
          <label v-if="sheetMode === 'create'" class="field full segmented-field">
            <span>任务类型</span>
            <span class="kind-segment">
              <button type="button" :class="{ active: form.kind === 'short' }" @click="form.kind = 'short'">短期待办</button>
              <button type="button" :class="{ active: form.kind === 'long' }" @click="form.kind = 'long'">长期任务</button>
            </span>
          </label>
          <label class="field full">
            <span>标题</span>
            <input v-model.trim="form.title" maxlength="200" required autofocus placeholder="清晰描述要完成的事" />
          </label>
          <label class="field full">
            <span>描述</span>
            <textarea v-model="form.description" rows="4" maxlength="10000" placeholder="补充背景、目标或完成标准…"></textarea>
          </label>
          <label class="field">
            <span>开始日期</span>
            <input v-model="form.start_date" type="date" required />
          </label>
          <label class="field">
            <span>结束日期</span>
            <input v-model="form.end_date" type="date" required />
          </label>
          <label class="field full">
            <span>重要程度</span>
            <select v-model="form.importance">
              <option v-for="option in importanceOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
            </select>
          </label>
        </div>

        <footer class="sheet-footer">
          <button type="button" class="cancel" @click="sheetOpen = false">取消</button>
          <button type="submit" class="confirm" :disabled="store.saving">{{ store.saving ? '保存中…' : sheetAction }}</button>
        </footer>
      </form>
    </MotionModal>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useRoute, useRouter } from 'vue-router'
import MotionModal from '@/components/motion/MotionModal.vue'
import TaskCalendar from '@/components/tasks/TaskCalendar.vue'
import TaskTree from '@/components/tasks/TaskTree.vue'
import {
  type TaskFields,
  type TaskImportance,
  type TaskKind,
  type TaskNode,
  type TaskStatus,
} from '@/api/tasks'
import { useTasksStore } from '@/stores/tasks'
import {
  addLocalDays,
  buildMonthGrid,
  formatTaskDateRange,
  shiftMonth,
  todayLocal,
} from '@/utils/taskDates'

type ViewMode = 'tasks' | 'calendar'
type SheetMode = 'create' | 'edit' | 'subtask' | 'progress' | 'status' | 'move'

const route = useRoute()
const router = useRouter()
const store = useTasksStore()
const viewMode = ref<ViewMode>(route.query.view === 'calendar' ? 'calendar' : 'tasks')
const searchQuery = ref('')
const kindFilter = ref<'all' | TaskKind>('all')
const statusFilter = ref<'active' | 'all' | TaskStatus>('active')
const today = todayLocal()
const calendarAnchor = ref(`${today.slice(0, 7)}-01`)
const selectedDate = ref(typeof route.query.date === 'string' ? route.query.date : today)
const activeNodeId = ref<string | null>(null)
const sheetOpen = ref(false)
const sheetMode = ref<SheetMode>('create')
const targetNode = ref<TaskNode | null>(null)
const form = ref<TaskFields & { kind: TaskKind }>({
  kind: 'short', title: '', description: '', start_date: today, end_date: today, importance: 'normal',
})
const progressForm = ref({ note: '', percent: 0, includePercent: false })
const statusForm = ref<{ status: TaskStatus; note: string; cascade: boolean }>({ status: 'in_progress', note: '', cascade: false })
const moveForm = ref({ parentId: '' })

const statusOptions: Array<{ value: TaskStatus; label: string }> = [
  { value: 'open', label: '待处理' },
  { value: 'planned', label: '已计划' },
  { value: 'in_progress', label: '进行中' },
  { value: 'blocked', label: '受阻' },
  { value: 'completed', label: '已完成' },
  { value: 'cancelled', label: '已取消' },
]
const importanceOptions: Array<{ value: TaskImportance; label: string }> = [
  { value: 'low', label: '低' },
  { value: 'normal', label: '普通' },
  { value: 'high', label: '重要' },
  { value: 'urgent', label: '紧急' },
]

const detail = computed(() => store.selectedDetail)
const activeNode = computed(() => detail.value?.tasks.find((task) => task.id === activeNodeId.value) || detail.value?.root || null)
const isTerminalStatus = computed(() => statusForm.value.status === 'completed' || statusForm.value.status === 'cancelled')
const statusSheetOptions = computed(() => targetNode.value?.kind === 'short'
  ? statusOptions.filter((option) => ['open', 'completed', 'cancelled'].includes(option.value))
  : statusOptions.filter((option) => option.value !== 'open'))
const groupedTasks = computed(() => {
  const soon = addLocalDays(today, 7)
  const closed = (status: TaskStatus) => status === 'completed' || status === 'cancelled'
  const definitions = [
    { key: 'overdue', label: '逾期', test: (task: typeof store.tasks[number]) => task.derived.overdue },
    { key: 'today', label: '今天', test: (task: typeof store.tasks[number]) => !closed(task.status) && (task.derived.active_today || task.derived.due_today || task.start_date === today) },
    { key: 'soon', label: '即将到期', test: (task: typeof store.tasks[number]) => !closed(task.status) && task.end_date > today && task.end_date <= soon },
    { key: 'long', label: '长期任务', test: (task: typeof store.tasks[number]) => !closed(task.status) && task.kind === 'long' },
    { key: 'later', label: '之后', test: (task: typeof store.tasks[number]) => !closed(task.status) },
    { key: 'closed', label: '已关闭', test: (task: typeof store.tasks[number]) => closed(task.status) },
  ]
  const assigned = new Set<string>()
  return definitions.map((definition) => ({
    key: definition.key,
    label: definition.label,
    tasks: store.tasks.filter((task) => {
      if (assigned.has(task.id) || !definition.test(task)) return false
      assigned.add(task.id)
      return true
    }),
  })).filter((group) => group.tasks.length > 0)
})
const moveCandidates = computed(() => {
  if (!detail.value || !targetNode.value) return []
  const blocked = descendantIds(targetNode.value.id, detail.value.tasks)
  return detail.value.tasks.filter((task) => !blocked.has(task.id) && task.id !== targetNode.value?.id)
})
const activity = computed(() => {
  if (!detail.value || !activeNode.value) return []
  const taskId = activeNode.value.id
  const progress = detail.value.progress
    .filter((item) => item.task_id === taskId)
    .map((item) => ({ id: item.id, type: 'progress', title: item.percent_after == null ? '记录了新进展' : `进展更新为 ${item.percent_after}%`, note: item.note, time: item.recorded_at }))
  const audit = detail.value.audit
    .filter((item) => item.task_id === taskId)
    .map((item) => ({ id: item.id, type: 'audit', title: auditTitle(item), note: item.note, time: item.occurred_at }))
  return [...progress, ...audit].sort((a, b) => b.time.localeCompare(a.time))
})
const sheetTitle = computed(() => ({
  create: '新建任务', edit: '编辑任务', subtask: '添加子任务', progress: '记录进展', status: '更改状态', move: '移动子任务',
})[sheetMode.value])
const sheetEyebrow = computed(() => ({
  create: '新的开始', edit: '调整计划', subtask: '拆解下一步', progress: '留下轨迹', status: '更新状态', move: '调整结构',
})[sheetMode.value])
const sheetAction = computed(() => ({ create: '创建', edit: '保存', subtask: '添加', progress: '记录', status: '更新', move: '移动' })[sheetMode.value])

function taskFilters() {
  const activeStatuses: TaskStatus[] = ['open', 'planned', 'in_progress', 'blocked']
  return {
    ...(kindFilter.value !== 'all' ? { kinds: [kindFilter.value] } : {}),
    ...(statusFilter.value === 'active' ? { statuses: activeStatuses } : statusFilter.value !== 'all' ? { statuses: [statusFilter.value] } : {}),
    ...(searchQuery.value.trim() ? { query: searchQuery.value.trim() } : {}),
  }
}

async function loadList() {
  await store.loadTasks(taskFilters()).catch(() => undefined)
}

async function loadCalendar() {
  const grid = buildMonthGrid(calendarAnchor.value)
  await store.loadCalendar(grid[0].date, grid[grid.length - 1].date, { ...taskFilters(), include_subtasks: true }).catch(() => undefined)
}

async function refreshCurrent() {
  await loadList()
  if (viewMode.value === 'calendar') await loadCalendar()
}

async function openTask(id: string) {
  const loaded = await store.loadDetail(id).catch(() => null)
  if (!loaded) return
  activeNodeId.value = loaded.root.id
  await router.replace({ query: { ...route.query, view: 'tasks', task: loaded.root.id } })
}

async function openFromCalendar(id: string) {
  changeView('tasks')
  await openTask(id)
}

function closeMobileDetail() {
  store.clearSelection()
  activeNodeId.value = null
  const query = { ...route.query }
  delete query.task
  void router.replace({ query })
}

function changeView(mode: ViewMode) {
  viewMode.value = mode
  void router.replace({ query: { ...route.query, view: mode } })
  if (mode === 'calendar') void loadCalendar()
}

function shiftCalendar(months: number) {
  calendarAnchor.value = shiftMonth(calendarAnchor.value, months)
  selectedDate.value = calendarAnchor.value
  void updateCalendarRoute()
  void loadCalendar()
}

function goToday() {
  calendarAnchor.value = `${today.slice(0, 7)}-01`
  selectedDate.value = today
  void updateCalendarRoute()
  void loadCalendar()
}

function selectCalendarDate(date: string) {
  selectedDate.value = date
  void updateCalendarRoute()
}

function updateCalendarRoute() {
  return router.replace({ query: { ...route.query, view: 'calendar', date: selectedDate.value } })
}

function resetForm(date = today) {
  form.value = { kind: 'short', title: '', description: '', start_date: date, end_date: date, importance: 'normal' }
}

function openCreate(date = today) {
  sheetMode.value = 'create'
  targetNode.value = null
  resetForm(date)
  sheetOpen.value = true
}

function openEdit(task: TaskNode) {
  sheetMode.value = 'edit'
  targetNode.value = task
  form.value = { kind: task.kind, title: task.title, description: task.description, start_date: task.start_date, end_date: task.end_date, importance: task.importance }
  sheetOpen.value = true
}

function openSubtask(parent: TaskNode) {
  sheetMode.value = 'subtask'
  targetNode.value = parent
  form.value = { kind: 'long', title: '', description: '', start_date: parent.start_date, end_date: parent.end_date, importance: parent.importance }
  sheetOpen.value = true
}

function openProgress(task: TaskNode) {
  sheetMode.value = 'progress'
  targetNode.value = task
  progressForm.value = { note: '', percent: task.status === 'completed' ? 100 : detail.value?.progress_percent || 0, includePercent: false }
  sheetOpen.value = true
}

function openStatus(task: TaskNode) {
  sheetMode.value = 'status'
  targetNode.value = task
  statusForm.value = { status: task.status, note: '', cascade: false }
  sheetOpen.value = true
}

function openMove(task: TaskNode) {
  sheetMode.value = 'move'
  targetNode.value = task
  moveForm.value.parentId = task.parent_id || detail.value?.root.id || ''
  sheetOpen.value = true
}

async function submitSheet() {
  try {
    if (form.value.end_date < form.value.start_date) {
      ElMessage.warning('结束日期不能早于开始日期')
      return
    }
    if (sheetMode.value === 'create') await store.create(form.value.kind, form.value)
    else if (sheetMode.value === 'edit' && targetNode.value) await store.update(targetNode.value.id, form.value)
    else if (sheetMode.value === 'subtask' && targetNode.value) await store.addSubtask(targetNode.value.id, form.value)
    else if (sheetMode.value === 'progress' && targetNode.value) await store.addProgress(targetNode.value.id, progressForm.value.note, progressForm.value.includePercent ? progressForm.value.percent : undefined)
    else if (sheetMode.value === 'status' && targetNode.value) await store.setStatus(targetNode.value.id, statusForm.value.status, statusForm.value.note || undefined, statusForm.value.cascade)
    else if (sheetMode.value === 'move' && targetNode.value) await store.moveSubtask(targetNode.value.id, moveForm.value.parentId, 9999)
    sheetOpen.value = false
    activeNodeId.value = targetNode.value?.id || store.selectedDetail?.root.id || null
    await refreshCurrent()
    ElMessage.success(`${sheetAction.value}成功`)
  } catch {
    // The store exposes the actionable error in the page banner.
  }
}

async function quickMove(taskId: string, parentId: string) {
  try {
    await store.moveSubtask(taskId, parentId, 9999)
    await refreshCurrent()
  } catch { /* shown by store */ }
}

async function archiveCurrent() {
  if (!detail.value) return
  try {
    await ElMessageBox.confirm('归档后默认不会出现在任务列表中，但文件仍会保留在 Obsidian。', '归档任务', { confirmButtonText: '归档', cancelButtonText: '取消' })
    await store.archive(detail.value.root.id, true)
    closeMobileDetail()
    await refreshCurrent()
    ElMessage.success('任务已归档')
  } catch { /* cancellation or store error */ }
}

async function syncNow() {
  try {
    const result = await store.sync()
    await refreshCurrent()
    ElMessage.success(`同步完成：${result.created} 个新增，${result.updated} 个更新`)
  } catch { /* shown by store */ }
}

function descendantIds(id: string, tasks: TaskNode[]) {
  const result = new Set<string>([id])
  let changed = true
  while (changed) {
    changed = false
    for (const task of tasks) {
      if (task.parent_id && result.has(task.parent_id) && !result.has(task.id)) {
        result.add(task.id)
        changed = true
      }
    }
  }
  return result
}

function statusLabel(status: TaskStatus) {
  return statusOptions.find((item) => item.value === status)?.label || status
}

function importanceLabel(importance: TaskImportance) {
  return importanceOptions.find((item) => item.value === importance)?.label || importance
}

function formatTimestamp(value: string) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function auditTitle(item: { event_type: string; from_status: TaskStatus | null; to_status: TaskStatus | null }) {
  if (item.event_type === 'status_changed' && item.to_status) return `状态变为${statusLabel(item.to_status)}`
  return ({ created: '创建了任务', updated: '更新了任务', moved: '移动了任务', archived: '归档了任务', reopened: '重新打开任务' } as Record<string, string>)[item.event_type] || '任务发生变化'
}

let filterTimer: number | undefined
watch([searchQuery, kindFilter, statusFilter], () => {
  window.clearTimeout(filterTimer)
  filterTimer = window.setTimeout(() => {
    void loadList()
    if (viewMode.value === 'calendar') void loadCalendar()
  }, 220)
})

onMounted(async () => {
  const tasks = await store.loadTasks(taskFilters()).catch(() => [])
  if (tasks.length === 0) {
    await store.sync().catch(() => undefined)
    await loadList()
  }
  if (viewMode.value === 'calendar') await loadCalendar()
  if (typeof route.query.task === 'string') await openTask(route.query.task)
})
</script>

<style scoped>
.tasks-page { min-height: 100%; padding: 28px 32px 44px; color: var(--text-primary); }
.page-header { display: flex; align-items: flex-end; justify-content: space-between; gap: 18px; margin-bottom: 20px; }
.page-title { margin: 0; font-size: 30px; font-weight: 690; letter-spacing: var(--tracking-tight); }
.page-subtitle { margin: 6px 0 0; color: var(--text-muted); font-size: 14px; }
.header-actions { display: flex; gap: 9px; }
.glass-button { min-height: 42px; padding: 0 15px; border: 1px solid var(--border-subtle); border-radius: 13px; background: var(--bg-glass); box-shadow: var(--shadow-sm), var(--inset-highlight); color: var(--text-primary); font-weight: 580; cursor: pointer; transition: transform var(--motion-instant) ease, background var(--motion-fast) ease, box-shadow var(--motion-fast) ease; }
.glass-button:active { transform: scale(.97); }
.glass-button.primary { background: var(--accent); border-color: transparent; color: white; box-shadow: 0 8px 24px color-mix(in srgb, var(--accent) 25%, transparent); }
.glass-button:disabled { opacity: .55; cursor: wait; }
.spinning { display: inline-block; animation: spin .8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.glass-surface { background: var(--bg-glass); border: 1px solid var(--border-glass); box-shadow: var(--shadow-sm), var(--inset-highlight); backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate)); -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate)); }
.task-toolbar { min-height: 58px; display: flex; align-items: center; gap: 12px; padding: 8px 10px; margin-bottom: 14px; border-radius: 18px; }
.view-switch { position: relative; display: grid; grid-template-columns: 1fr 1fr; width: 174px; padding: 3px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 5%, transparent); isolation: isolate; }
.switch-indicator { position: absolute; inset: 3px auto 3px 3px; width: calc(50% - 3px); border-radius: 10px; background: var(--bg-glass-strong); box-shadow: var(--shadow-sm), var(--inset-highlight); transition: transform var(--motion-normal) var(--ease-spring-gentle); z-index: -1; }
.switch-indicator.calendar { transform: translateX(100%); }
.view-switch button { min-height: 36px; border: 0; background: transparent; color: var(--text-muted); font-weight: 570; cursor: pointer; }
.view-switch button.active { color: var(--text-primary); }
.task-search { flex: 1; min-width: 170px; height: 40px; display: flex; align-items: center; gap: 7px; padding: 0 11px; border: 1px solid transparent; border-radius: 12px; color: var(--text-faint); transition: border-color var(--motion-fast) ease, background var(--motion-fast) ease; }
.task-search:focus-within { background: var(--bg-glass-strong); border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
.task-search input { width: 100%; border: 0; outline: 0; background: transparent; color: var(--text-primary); font: inherit; }
.filters { display: flex; gap: 7px; }
.filters select, .field input, .field textarea, .field select { border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--text-primary); font: inherit; outline: none; }
.filters select { height: 40px; padding: 0 30px 0 11px; }
.error-banner { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 12px; padding: 10px 13px; border: 1px solid color-mix(in srgb, #ff3b30 28%, transparent); border-radius: 13px; background: color-mix(in srgb, #ff3b30 8%, var(--bg-glass)); color: var(--text-secondary); font-size: 13px; }
.error-banner button { border: 0; background: transparent; color: #ff3b30; cursor: pointer; }
.error-banner-enter-active, .error-banner-leave-active { transition: opacity var(--motion-fast) ease, transform var(--motion-fast) ease; }
.error-banner-enter-from, .error-banner-leave-to { opacity: 0; transform: translateY(-5px); }
.task-workspace { min-height: 620px; height: calc(100dvh - 208px); display: grid; grid-template-columns: minmax(270px, 340px) minmax(0, 1fr); gap: 14px; }
.task-list-panel, .task-detail-panel { border-radius: 22px; min-height: 0; overflow: auto; }
.task-list-panel { padding: 10px; }
.panel-heading { height: 45px; display: flex; align-items: center; justify-content: space-between; padding: 0 7px 6px; }
.panel-heading div { display: flex; align-items: baseline; gap: 7px; }
.panel-heading strong { font-size: 14px; }
.panel-heading span { color: var(--text-faint); font-size: 11px; }
.panel-heading button { width: 38px; height: 38px; border: 0; border-radius: 11px; background: transparent; color: var(--accent); font-size: 21px; cursor: pointer; }
.panel-heading button:hover { background: var(--bg-glass-strong); }
.task-list { display: grid; gap: 14px; }
.task-group { display: grid; gap: 5px; }
.task-group-title { display: flex; align-items: center; gap: 6px; padding: 0 7px; color: var(--text-muted); }
.task-group-title span { font-size: 11px; font-weight: 650; }
.task-group-title small { color: var(--text-faint); font-size: 9px; }
.task-group-items { display: grid; gap: 5px; }
.task-card { position: relative; min-height: 104px; display: grid; grid-template-columns: 4px minmax(0, 1fr) 18px; gap: 10px; padding: 12px 10px 11px 0; border: 1px solid transparent; border-radius: 16px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; overflow: hidden; transition: background var(--motion-fast) ease, border-color var(--motion-fast) ease, transform var(--motion-instant) ease; }
.task-card:hover { background: var(--bg-glass-strong); }
.task-card:active { transform: scale(.985); }
.task-card.selected { background: var(--bg-glass-strong); border-color: var(--border-subtle); box-shadow: var(--shadow-sm); }
.card-accent { align-self: stretch; border-radius: 0 4px 4px 0; background: var(--accent); }
.task-card.importance-low .card-accent { background: #8e8e93; }
.task-card.importance-high .card-accent { background: #ff9500; }
.task-card.importance-urgent .card-accent { background: #ff3b30; }
.card-main { min-width: 0; display: grid; align-content: center; }
.card-topline { min-width: 0; display: flex; align-items: center; gap: 7px; }
.card-topline strong { min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
.kind-badge { flex: none; padding: 2px 6px; border-radius: 6px; background: color-mix(in srgb, var(--accent) 9%, transparent); color: var(--text-muted); font-size: 9px; }
.card-description { margin-top: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-muted); font-size: 11px; }
.card-footer { display: flex; justify-content: space-between; margin-top: 8px; color: var(--text-faint); font-size: 10px; }
.danger { color: #ff3b30; }
.mini-progress { height: 3px; margin-top: 7px; border-radius: 3px; background: color-mix(in srgb, var(--text-primary) 7%, transparent); overflow: hidden; }
.mini-progress i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transition: width var(--motion-slow) var(--ease-spring-gentle); }
.card-chevron { align-self: center; color: var(--text-faint); font-size: 20px; }
.task-card-enter-active, .task-card-leave-active { transition: opacity var(--motion-normal) ease, transform var(--motion-normal) var(--ease-spring-gentle); }
.task-card-enter-from, .task-card-leave-to { opacity: 0; transform: translateY(7px); }
.task-skeletons { display: grid; gap: 8px; }
.task-skeletons span { height: 96px; border-radius: 16px; background: linear-gradient(100deg, transparent 20%, color-mix(in srgb, var(--text-primary) 7%, transparent) 45%, transparent 70%); background-size: 220% 100%; animation: shimmer 1.4s infinite; }
@keyframes shimmer { to { background-position: -220% 0; } }
.empty-list, .empty-detail { height: 100%; display: grid; place-content: center; justify-items: center; text-align: center; color: var(--text-muted); }
.empty-list { min-height: 420px; }
.empty-list p, .empty-detail p { margin: 7px 0 15px; color: var(--text-faint); font-size: 12px; }
.empty-list button, .activity-empty { min-height: 40px; padding: 0 13px; border: 1px solid var(--border-subtle); border-radius: 12px; background: var(--bg-glass); color: var(--accent); cursor: pointer; }
.empty-orb { width: 54px; height: 54px; display: grid; place-items: center; margin-bottom: 13px; border-radius: 18px; background: color-mix(in srgb, var(--accent) 10%, transparent); color: var(--accent); font-size: 25px; }
.task-detail-panel { padding: 24px 26px; }
.mobile-detail-nav { display: none; }
.detail-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 18px; }
.detail-title-group { min-width: 0; }
.detail-kicker { display: flex; align-items: center; gap: 6px; color: var(--text-muted); font-size: 11px; font-weight: 580; }
.status-dot { width: 7px; height: 7px; border-radius: 50%; background: #8e8e93; box-shadow: 0 0 0 4px color-mix(in srgb, #8e8e93 10%, transparent); }
.status-in_progress { background: var(--accent); box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 11%, transparent); }
.status-blocked { background: #ff9500; }.status-completed { background: #34c759; }.status-cancelled { background: #8e8e93; }
.detail-title-group h2 { margin: 7px 0 5px; font-size: clamp(22px, 3vw, 30px); line-height: 1.18; letter-spacing: var(--tracking-tight); }
.detail-title-group p { max-width: 720px; margin: 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.detail-actions { display: flex; gap: 7px; flex: none; }
.detail-actions button, .section-heading button { min-height: 38px; padding: 0 11px; border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--text-secondary); cursor: pointer; }
.detail-actions .more-button { width: 40px; padding: 0; }
.detail-facts { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin: 22px 0; }
.detail-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.detail-facts span { color: var(--text-faint); font-size: 10px; }
.detail-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
.progress-overview, .detail-section { padding: 16px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.detail-section { margin-top: 12px; }
.section-heading { min-height: 38px; display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
.section-heading div { display: flex; align-items: baseline; gap: 8px; }
.section-heading span { color: var(--text-muted); font-size: 12px; }
.section-heading strong { font-size: 17px; }.section-heading small { color: var(--text-faint); font-size: 10px; }
.progress-track { height: 8px; border-radius: 8px; background: color-mix(in srgb, var(--text-primary) 6%, transparent); overflow: hidden; }
.progress-track i { display: block; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 65%, #34c759)); transition: width var(--motion-slow) var(--ease-spring-gentle); }
.activity-list { display: grid; gap: 0; }
.activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; min-height: 56px; }
.activity-item > div { padding: 0 0 14px 3px; border-bottom: 1px solid var(--border-subtle); }
.activity-item:last-child > div { border-bottom: 0; }
.activity-dot { width: 9px; height: 9px; margin-top: 4px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.activity-dot.audit { border-color: var(--text-faint); }
.activity-item strong { font-size: 12px; }.activity-item p { margin: 5px 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }.activity-item time { color: var(--text-faint); font-size: 10px; }
.detail-loading { height: 100%; display: flex; align-items: center; justify-content: center; gap: 5px; }.detail-loading span { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); animation: pulse 1s infinite alternate; }.detail-loading span:nth-child(2) { animation-delay: .15s; }.detail-loading span:nth-child(3) { animation-delay: .3s; }
@keyframes pulse { to { opacity: .25; transform: translateY(-4px); } }
.detail-illustration { position: relative; width: 78px; height: 64px; margin-bottom: 13px; }.detail-illustration span, .detail-illustration i { position: absolute; display: block; border: 1px solid var(--border-subtle); border-radius: 17px; background: var(--bg-glass); box-shadow: var(--shadow-sm); }.detail-illustration span { inset: 0 13px 8px 0; transform: rotate(-6deg); }.detail-illustration i { inset: 8px 0 0 13px; transform: rotate(5deg); }
.task-sheet { max-height: calc(100dvh - 48px); display: flex; flex-direction: column; border-radius: 24px; border: 1px solid var(--border-glass); background: color-mix(in srgb, var(--bg-glass-strong) 82%, var(--bg-base)); box-shadow: var(--shadow-lg), var(--inset-highlight); backdrop-filter: blur(30px) saturate(1.35); -webkit-backdrop-filter: blur(30px) saturate(1.35); overflow: hidden; }
.sheet-header { display: flex; align-items: center; justify-content: space-between; padding: 21px 22px 14px; }.sheet-header span { color: var(--text-faint); font-size: 10px; letter-spacing: .08em; }.sheet-header h3 { margin: 4px 0 0; font-size: 21px; }.sheet-header button { width: 38px; height: 38px; border: 0; border-radius: 12px; background: var(--bg-glass); color: var(--text-muted); font-size: 23px; cursor: pointer; }
.sheet-body { overflow-y: auto; padding: 8px 22px 18px; }.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 13px; }.field { display: grid; gap: 6px; }.field.full { grid-column: 1 / -1; }.field > span:first-child { color: var(--text-muted); font-size: 11px; font-weight: 570; }.field input, .field select { height: 44px; padding: 0 11px; }.field textarea { resize: vertical; padding: 10px 11px; line-height: 1.5; }.field input:focus, .field select:focus, .field textarea:focus { border-color: color-mix(in srgb, var(--accent) 45%, transparent); box-shadow: var(--focus-ring); }
.kind-segment { display: grid; grid-template-columns: 1fr 1fr; padding: 3px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 5%, transparent); }.kind-segment button { height: 38px; border: 0; border-radius: 10px; background: transparent; color: var(--text-muted); cursor: pointer; }.kind-segment button.active { background: var(--bg-glass-strong); box-shadow: var(--shadow-sm); color: var(--text-primary); font-weight: 600; }
.range-field { display: grid; grid-template-columns: 1fr 48px; align-items: center; gap: 12px; }.range-field input { width: 100%; padding: 0; accent-color: var(--accent); }.range-field output { color: var(--accent); font-weight: 650; text-align: right; }.check-field { display: flex; align-items: center; gap: 9px; min-height: 44px; color: var(--text-muted); font-size: 12px; }.check-field input { width: 18px; height: 18px; accent-color: var(--accent); }.sheet-hint { margin: 0; color: var(--text-faint); font-size: 11px; }
.sheet-footer { display: grid; grid-template-columns: 1fr 1.4fr; gap: 9px; padding: 13px 22px 21px; border-top: 1px solid var(--border-subtle); }.sheet-footer button { height: 44px; border-radius: 13px; font-weight: 620; cursor: pointer; }.sheet-footer .cancel { border: 1px solid var(--border-subtle); background: var(--bg-glass); color: var(--text-secondary); }.sheet-footer .confirm { border: 0; background: var(--accent); color: white; box-shadow: 0 8px 22px color-mix(in srgb, var(--accent) 22%, transparent); }.sheet-footer .confirm:disabled { opacity: .55; }

@media (max-width: 1050px) {
  .task-workspace { grid-template-columns: 290px minmax(0, 1fr); }.detail-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }.detail-actions { flex-wrap: wrap; justify-content: flex-end; }
}

@media (max-width: 768px) {
  .tasks-page { padding: 70px 12px 28px; }.page-header { align-items: center; margin-bottom: 14px; }.page-title { font-size: 24px; }.page-subtitle { font-size: 12px; }.header-actions .secondary { width: 44px; padding: 0; font-size: 0; }.header-actions .secondary span { font-size: 18px; }.glass-button { min-height: 44px; }.task-toolbar { flex-wrap: wrap; padding: 7px; }.view-switch { width: 100%; }.view-switch button { min-height: 38px; }.task-search { order: 2; min-width: 0; flex: 1; min-height: 44px; }.filters { order: 3; width: 100%; }.filters select { flex: 1; min-width: 0; height: 44px; }.task-workspace { min-height: calc(100dvh - 260px); height: auto; display: block; }.task-list-panel, .task-detail-panel { min-height: calc(100dvh - 260px); border-radius: 20px; }.task-detail-panel { display: none; padding: 12px; }.task-workspace.detail-open .task-list-panel { display: none; }.task-workspace.detail-open .task-detail-panel { display: block; }.mobile-detail-nav { height: 45px; display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }.mobile-detail-nav button { min-height: 44px; border: 0; background: transparent; color: var(--accent); font-weight: 600; }.mobile-detail-nav span { color: var(--text-faint); font-size: 11px; }.detail-header { display: block; }.detail-title-group h2 { font-size: 23px; }.detail-actions { display: grid; grid-template-columns: 1fr 1fr 44px; margin-top: 14px; }.detail-actions button { min-height: 44px; }.detail-facts { grid-template-columns: 1fr 1fr; margin: 14px 0; }.detail-facts div:last-child { grid-column: 1 / -1; }.progress-overview, .detail-section { padding: 13px; }.section-heading button { min-height: 44px; }.task-card { min-height: 110px; }.panel-heading button { min-width: 44px; min-height: 44px; }.task-sheet { max-height: min(90dvh, 760px); border-radius: 24px 24px 0 0; border-bottom: 0; }.sheet-header { padding-top: 32px; }.sheet-body { overscroll-behavior: contain; }.form-grid { grid-template-columns: 1fr; }.field.full { grid-column: auto; }.field input, .field select { min-height: 48px; }.sheet-footer { padding-bottom: max(18px, env(safe-area-inset-bottom)); }.sheet-footer button { min-height: 48px; }
}

@media (prefers-reduced-motion: reduce) {
  .switch-indicator, .task-card, .mini-progress i, .progress-track i, .error-banner-enter-active, .error-banner-leave-active, .task-card-enter-active, .task-card-leave-active { transition-duration: 1ms !important; }.spinning, .task-skeletons span, .detail-loading span { animation: none !important; }
}
@media (prefers-reduced-transparency: reduce) { .glass-surface, .task-sheet { backdrop-filter: none; -webkit-backdrop-filter: none; background: var(--bg-primary); } }
@media (prefers-contrast: more) { .task-card.selected, .progress-overview, .detail-section { border-color: var(--text-muted); } }
</style>
