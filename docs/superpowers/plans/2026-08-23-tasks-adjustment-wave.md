# 任务中枢调整波 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落实用户验收后的 8 项 UI 调整：抽屉子任务列表与按钮重组、左缘收起把手、活动流「类型胶囊 + 明细」条目、分隔线间距、属性胶囊、下半部分留白、整页滚动锁定。

**Architecture:** 纯前端调整。`utils/taskActivity.ts` 的条目模型增加 `detail` 字段并将 `title` 改为名词式类型标签；`SubtaskDrawer.vue` 整体改版（子任务栏目、右上角图标按钮、栏目加号、收起把手）；`Tasks.vue` 消费新模型并锁定页面布局；属性胶囊 `.task-pill` 定义于 App.vue 全局样式块供两处共用。

**Tech Stack:** Vue 3 `<script setup>` + TS、scoped CSS、Element Plus 图标、`node --test --experimental-strip-types`。

**Spec:** `docs/superpowers/specs/2026-08-22-tasks-activity-drawer-design.md` §12（调整波）。

## Global Constraints

- **后端零改动**；不动 `TaskTree.vue`；不新增前端依赖。
- 绝不触碰 `backend/target/release/obsidian-brain` 的安装路径（`/usr/local/bin`）与运行中的进程；不写 `~/.obsidian-brain/brain.db`；不碰 Obsidian vault。
- 测试门禁：`cd frontend && npx vue-tsc -b && npm test`（当前 30 个测试，调整后 Task 1 会改写其中 1 个并新增 1 个 → 31 个）。utils 跨模块只能 `import type`；utils 内部用相对路径 import（`'../api/tasks'`）。
- 现行分支 `feature/tasks-subtask-drawer`；所有工作直接提交到该分支，Conventional Commits。
- `.task-pill` 的 CSS 类名与修饰类（`status-*`/`importance-*`/`type-*`）为全局契约，Task 2 定义、Task 3 消费，不得改名。
- `TaskActivityEntry` 新增 `detail: string | null` 字段是 Task 1→Task 3 的接口契约。

---

### Task 1: taskActivity util — 类型标签与 detail 字段

**Files:**
- Modify: `frontend/src/utils/taskActivity.ts`（整文件替换）
- Test: `frontend/tests/taskActivity.test.ts`（整文件替换）

**Interfaces:**
- Produces: `TaskActivityEntry { id, type: 'progress' | 'audit', taskId, taskTitle, title, detail: string | null, note: string | null, time }`；`buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 签名不变。
- `title` 语义变更：名词式类型标签（进展/状态变更/重新打开/归档/取消归档/级联完成/移动/创建/更新/兜底 变更）。

- [ ] **Step 1: 改写测试（先红）**

将 `frontend/tests/taskActivity.test.ts` 整文件替换为：

```ts
import assert from 'node:assert/strict'
import test from 'node:test'

import { buildTaskActivity, taskImportanceLabel, taskStatusLabel } from '../src/utils/taskActivity.ts'

const nodes = [
  {
    id: 'root', root_id: 'root', parent_id: null, kind: 'long' as const, role: 'root' as const,
    title: '根任务', description: '', start_date: '2026-08-01', end_date: '2026-08-31',
    importance: 'high' as const, status: 'in_progress' as const, position: 0,
    closure_note: null, closed_at: null, created_at: '2026-08-01T00:00:00Z', updated_at: '2026-08-01T00:00:00Z',
    revision: 1, archived_at: null,
  },
  {
    id: 'child', root_id: 'root', parent_id: 'root', kind: 'long' as const, role: 'subtask' as const,
    title: '子任务甲', description: '', start_date: '2026-08-01', end_date: '2026-08-31',
    importance: 'normal' as const, status: 'planned' as const, position: 0,
    closure_note: null, closed_at: null, created_at: '2026-08-02T00:00:00Z', updated_at: '2026-08-02T00:00:00Z',
    revision: 1, archived_at: null,
  },
]

const progress = [
  { id: 'p1', root_id: 'root', task_id: 'root', recorded_at: '2026-08-17T09:31:51Z', note: 'process2', percent_after: 35, created_at: '2026-08-17T09:31:51Z' },
  { id: 'p2', root_id: 'root', task_id: 'child', recorded_at: '2026-08-21T09:52:16Z', note: '111111', percent_after: null, created_at: '2026-08-21T09:52:16Z' },
]

const audit = [
  { id: 'a1', root_id: 'root', task_id: 'root', event_type: 'status_changed', from_status: 'planned', to_status: 'in_progress', note: null, occurred_at: '2026-08-17T09:35:32Z' },
  { id: 'a2', root_id: 'root', task_id: 'child', event_type: 'moved', from_status: null, to_status: null, note: '移动到 根任务', occurred_at: '2026-08-21T08:36:07Z' },
]

test('aggregates progress and audit across the whole tree with type labels and details', () => {
  const entries = buildTaskActivity(nodes, progress, audit)

  assert.deepEqual(entries.map((entry) => [entry.id, entry.taskTitle, entry.title, entry.detail]), [
    ['progress:p2', '子任务甲', '进展', null],
    ['audit:a2', '子任务甲', '移动', null],
    ['audit:a1', '根任务', '状态变更', '已计划 → 进行中'],
    ['progress:p1', '根任务', '进展', '完成度 35%'],
  ])
  assert.equal(entries[0].note, '111111')
  assert.equal(entries[0].time, '2026-08-21T09:52:16Z')
})

test('scopeTaskId filters entries to a single task for the drawer', () => {
  const entries = buildTaskActivity(nodes, progress, audit, 'child')

  assert.deepEqual(entries.map((entry) => entry.id), ['progress:p2', 'audit:a2'])
})

test('unknown task ids fall back to 未知任务', () => {
  const entries = buildTaskActivity([], progress, [])

  assert.equal(entries.length, 2)
  assert.ok(entries.every((entry) => entry.taskTitle === '未知任务'))
})

test('audit type labels and status details cover the event enum', () => {
  const auditEvents = [
    { id: 'r1', root_id: 'root', task_id: 'root', event_type: 'reopened', from_status: 'completed', to_status: 'in_progress', note: null, occurred_at: '2026-08-21T10:00:00Z' },
    { id: 'r2', root_id: 'root', task_id: 'root', event_type: 'archived', from_status: null, to_status: null, note: null, occurred_at: '2026-08-21T10:01:00Z' },
    { id: 'r3', root_id: 'root', task_id: 'root', event_type: 'unarchived', from_status: null, to_status: null, note: null, occurred_at: '2026-08-21T10:02:00Z' },
    { id: 'r4', root_id: 'root', task_id: 'root', event_type: 'cascade_completed', from_status: 'planned', to_status: 'completed', note: null, occurred_at: '2026-08-21T10:03:00Z' },
    { id: 'r5', root_id: 'root', task_id: 'root', event_type: 'mystery', from_status: null, to_status: 'blocked', note: null, occurred_at: '2026-08-21T10:04:00Z' },
  ]
  const entries = buildTaskActivity(nodes, [], auditEvents)

  assert.deepEqual(entries.map((entry) => [entry.title, entry.detail]), [
    ['变更', '受阻'],
    ['级联完成', '已计划 → 已完成'],
    ['取消归档', null],
    ['归档', null],
    ['重新打开', '已完成 → 进行中'],
  ])
})

test('status and importance labels cover enum values', () => {
  assert.equal(taskStatusLabel('open'), '待处理')
  assert.equal(taskStatusLabel('in_progress'), '进行中')
  assert.equal(taskStatusLabel('cancelled'), '已取消')
  assert.equal(taskImportanceLabel('urgent'), '紧急')
  assert.equal(taskImportanceLabel('normal'), '普通')
})
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && npm test`
Expected: FAIL —— 断言里的 `'进展'`/`'移动'`/`'状态变更'` 与现实现的 `'记录了新进展'` 等不一致；`detail` 字段不存在。

- [ ] **Step 3: 实现新 util**

将 `frontend/src/utils/taskActivity.ts` 整文件替换为：

```ts
import type { AuditEvent, ProgressEntry, TaskImportance, TaskNode, TaskStatus } from '../api/tasks'

/** A single attributed row in the task activity feed (progress or audit). */
export interface TaskActivityEntry {
  id: string
  type: 'progress' | 'audit'
  taskId: string
  taskTitle: string
  /** Terse type label rendered as a pill: 进展 / 状态变更 / 移动 / … */
  title: string
  /** One-line specifics under the label: 完成度 35% / 已计划 → 进行中 / null */
  detail: string | null
  note: string | null
  time: string
}

export function taskStatusLabel(status: TaskStatus): string {
  return ({
    open: '待处理',
    planned: '已计划',
    in_progress: '进行中',
    blocked: '受阻',
    completed: '已完成',
    cancelled: '已取消',
  } satisfies Record<TaskStatus, string>)[status]
}

export function taskImportanceLabel(importance: TaskImportance): string {
  return ({ low: '低', normal: '普通', high: '重要', urgent: '紧急' } satisfies Record<TaskImportance, string>)[importance]
}

/** Noun-style type labels — no narrative phrasing ("记录了新进展" and friends are gone). */
const AUDIT_TYPE_LABELS: Record<string, string> = {
  status_changed: '状态变更',
  reopened: '重新打开',
  archived: '归档',
  unarchived: '取消归档',
  cascade_completed: '级联完成',
  moved: '移动',
  created: '创建',
  updated: '更新',
}

function auditTitle(item: Pick<AuditEvent, 'event_type'>): string {
  return AUDIT_TYPE_LABELS[item.event_type] || '变更'
}

function auditDetail(item: Pick<AuditEvent, 'from_status' | 'to_status'>): string | null {
  const from = item.from_status ? taskStatusLabel(item.from_status) : null
  const to = item.to_status ? taskStatusLabel(item.to_status) : null
  if (from && to) return `${from} → ${to}`
  return to
}

/**
 * Build the attributed activity feed for a task document.
 * Without scopeTaskId every progress/audit entry of the tree is returned
 * (root overview); with scopeTaskId only that task's entries (drawer view).
 * Sorted newest first.
 */
export function buildTaskActivity(
  nodes: readonly TaskNode[],
  progress: readonly ProgressEntry[],
  audit: readonly AuditEvent[],
  scopeTaskId?: string,
): TaskActivityEntry[] {
  const titles = new Map(nodes.map((node) => [node.id, node.title]))
  const titleOf = (taskId: string) => titles.get(taskId) || '未知任务'
  const progressEntries = progress
    .filter((item) => !scopeTaskId || item.task_id === scopeTaskId)
    .map((item) => ({
      id: `progress:${item.id}`,
      type: 'progress' as const,
      taskId: item.task_id,
      taskTitle: titleOf(item.task_id),
      title: '进展',
      detail: item.percent_after == null ? null : `完成度 ${item.percent_after}%`,
      note: item.note,
      time: item.recorded_at,
    }))
  const auditEntries = audit
    .filter((item) => !scopeTaskId || item.task_id === scopeTaskId)
    .map((item) => ({
      id: `audit:${item.id}`,
      type: 'audit' as const,
      taskId: item.task_id,
      taskTitle: titleOf(item.task_id),
      title: auditTitle(item),
      detail: auditDetail(item),
      note: item.note,
      time: item.occurred_at,
    }))
  return [...progressEntries, ...auditEntries].sort((a, b) => b.time.localeCompare(a.time))
}
```

- [ ] **Step 4: 运行测试确认通过 + 类型检查**

Run: `cd frontend && npm test && npx vue-tsc -b`
Expected: 31 个测试全部通过（注意：`Tasks.vue`/`SubtaskDrawer.vue` 此时尚未消费 `detail`，`vue-tsc` 应无错误——它们只用 `title`/`note`，仍合法）。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/utils/taskActivity.ts frontend/tests/taskActivity.test.ts
git commit -m "refactor(tasks): noun-style activity type labels with detail line"
```

---

### Task 2: 全局胶囊样式 + SubtaskDrawer 改版

**Files:**
- Modify: `frontend/src/App.vue`（全局 style 块插入 `.task-pill` 系列）
- Modify: `frontend/src/components/tasks/SubtaskDrawer.vue`（整文件替换）

**Interfaces:**
- Consumes: Task 1 的 `TaskActivityEntry`（`title` 类型标签 + `detail`）。
- Produces: 全局类 `.task-pill` + `status-*`/`importance-*`/`type-*` 修饰类（Task 3 消费）；`SubtaskDrawer` 新 props `children: TaskNode[]`、`parent: TaskNode | null`，新 emit `select: [taskId: string]`（Task 3 绑定 `focusTask`）；`expose revealActivity()` 不变。

- [ ] **Step 1: App.vue 全局样式块插入胶囊规则**

在 `frontend/src/App.vue` 的全局（非 scoped）`<style>` 块中，紧跟 `.header-actions { display: flex; gap: 8px; flex-shrink: 0; }` 一行之后、`/* ── Scrollbar ── */` 注释之前插入：

```css

/* ── Task attribute pills (shared by Tasks view + subtask drawer) ── */
.task-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 22px;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 620;
  line-height: 1;
  white-space: nowrap;
}
.task-pill.status-open, .task-pill.status-cancelled { background: color-mix(in srgb, #8e8e93 15%, transparent); color: color-mix(in srgb, #8e8e93 85%, var(--text-primary)); }
.task-pill.status-planned { background: color-mix(in srgb, var(--text-primary) 7%, transparent); color: var(--text-secondary); }
.task-pill.status-in_progress { background: color-mix(in srgb, var(--accent) 13%, transparent); color: color-mix(in srgb, var(--accent) 82%, var(--text-primary)); }
.task-pill.status-blocked { background: color-mix(in srgb, #ff9500 15%, transparent); color: color-mix(in srgb, #ff9500 85%, var(--text-primary)); }
.task-pill.status-completed { background: color-mix(in srgb, #34c759 15%, transparent); color: color-mix(in srgb, #34c759 85%, var(--text-primary)); }
.task-pill.importance-low { background: color-mix(in srgb, #8e8e93 13%, transparent); color: var(--text-muted); }
.task-pill.importance-normal { background: color-mix(in srgb, var(--text-primary) 6%, transparent); color: var(--text-secondary); }
.task-pill.importance-high { background: color-mix(in srgb, #ff9500 15%, transparent); color: color-mix(in srgb, #ff9500 85%, var(--text-primary)); }
.task-pill.importance-urgent { background: color-mix(in srgb, #ff3b30 15%, transparent); color: color-mix(in srgb, #ff3b30 85%, var(--text-primary)); }
.task-pill.type-progress { background: color-mix(in srgb, var(--accent) 12%, transparent); color: color-mix(in srgb, var(--accent) 82%, var(--text-primary)); }
.task-pill.type-audit { background: color-mix(in srgb, var(--text-primary) 6%, transparent); color: var(--text-secondary); }
```

- [ ] **Step 2: 整文件替换 SubtaskDrawer.vue**

将 `frontend/src/components/tasks/SubtaskDrawer.vue` 整文件替换为：

```vue
<template>
  <div class="drawer-backdrop" :class="{ open: !!node }" @click="emit('close')"></div>
  <div class="subtask-drawer-slot" :class="{ open: !!node }">
    <aside v-if="node" class="subtask-drawer" role="dialog" aria-label="子任务详情">
      <header class="drawer-header">
        <button v-if="parent" type="button" class="drawer-back" :title="`返回 ${parent.title}`" @click="emit('select', parent.id)">
          <span class="drawer-back-arrow" aria-hidden="true">‹</span>
          <span class="drawer-back-title">{{ parent.title }}</span>
        </button>
        <div class="drawer-row">
          <div class="drawer-title-group">
            <div class="drawer-kicker">
              <span class="task-pill" :class="`status-${node.status}`">{{ taskStatusLabel(node.status) }}</span>
              <span class="task-pill" :class="`importance-${node.importance}`">{{ taskImportanceLabel(node.importance) }}</span>
            </div>
            <h3>{{ node.title }}</h3>
          </div>
          <div class="drawer-corner">
            <button type="button" class="corner-button" aria-label="编辑任务" title="编辑" @click="emit('edit', node)"><el-icon><EditPen /></el-icon></button>
            <button type="button" class="corner-button" aria-label="更改状态" title="更改状态" @click="emit('status', node)"><el-icon><Refresh /></el-icon></button>
            <button type="button" class="corner-button" aria-label="移动任务" title="移动" @click="emit('move', node)"><el-icon><Rank /></el-icon></button>
          </div>
        </div>
      </header>

      <div class="drawer-scroll">
        <p v-if="node.description" class="drawer-description">{{ node.description }}</p>

        <div class="drawer-facts">
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
          <div><span>优先级</span><strong>{{ taskImportanceLabel(node.importance) }}</strong></div>
        </div>

        <section class="drawer-section">
          <div class="drawer-section-heading">
            <div><span>子任务</span><strong>{{ children.length }} 个</strong></div>
            <button type="button" class="drawer-add" aria-label="添加子任务" title="添加子任务" @click="emit('add', node)">＋</button>
          </div>
          <div v-if="children.length" class="drawer-children">
            <button
              v-for="child in children"
              :key="child.id"
              type="button"
              class="drawer-child"
              @click="emit('select', child.id)"
            >
              <span class="status-dot" :class="`status-${child.status}`"></span>
              <strong>{{ child.title }}</strong>
              <span class="drawer-child-chevron" aria-hidden="true">›</span>
            </button>
          </div>
          <p v-else class="drawer-section-empty">还没有子任务</p>
        </section>

        <section ref="activityRef" class="drawer-section">
          <div class="drawer-section-heading">
            <div><span>进展与记录</span><strong>{{ activity.length }} 条</strong></div>
            <button type="button" class="drawer-add" aria-label="记录进展" title="记录进展" @click="emit('progress', node)">＋</button>
          </div>
          <div v-if="activity.length" class="drawer-activity">
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item">
              <span class="activity-dot" :class="item.type"></span>
              <div class="drawer-activity-copy">
                <div class="drawer-activity-head">
                  <strong><span class="task-pill" :class="`type-${item.type}`">{{ item.title }}</span></strong>
                  <time>{{ formatTimestamp(item.time) }}</time>
                </div>
                <p v-if="item.detail" class="drawer-activity-detail">{{ item.detail }}</p>
                <p v-if="item.note" class="drawer-activity-note">{{ item.note }}</p>
              </div>
            </article>
          </div>
          <p v-else class="drawer-section-empty">暂无进展记录</p>
        </section>
      </div>

      <button ref="collapseRef" type="button" class="drawer-collapse" aria-label="收起子任务详情" title="收起" @click="emit('close')">‹</button>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { EditPen, Rank, Refresh } from '@element-plus/icons-vue'
import type { TaskNode } from '@/api/tasks'
import { taskImportanceLabel, taskStatusLabel, type TaskActivityEntry } from '@/utils/taskActivity'
import { formatTimestamp } from '@/utils/taskDates'

const props = defineProps<{
  node: TaskNode | null
  activity: TaskActivityEntry[]
  children: TaskNode[]
  parent: TaskNode | null
}>()

const emit = defineEmits<{
  close: []
  select: [taskId: string]
  progress: [task: TaskNode]
  add: [task: TaskNode]
  status: [task: TaskNode]
  edit: [task: TaskNode]
  move: [task: TaskNode]
}>()

const activityRef = ref<HTMLElement | null>(null)
const collapseRef = ref<HTMLButtonElement | null>(null)

// Move focus into the drawer when it opens so keyboard users land on the collapse affordance.
watch(() => props.node?.id, async (id) => {
  await nextTick()
  if (id) collapseRef.value?.focus()
})

function revealActivity() {
  activityRef.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ revealActivity })
</script>

<style scoped>
.drawer-backdrop { display: none; }

/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone.
   The 24px padding strip left of the drawer hosts the collapse handle. */
.subtask-drawer-slot {
  flex: 0 0 auto;
  width: 0;
  padding-left: 0;
  overflow: hidden;
  transition: width var(--motion-normal) var(--ease-emphasized), padding-left var(--motion-normal) var(--ease-emphasized);
}
.subtask-drawer-slot.open { width: calc(min(400px, 30vw) + 24px); padding-left: 24px; }
.subtask-drawer {
  position: relative;
  width: min(400px, 30vw);
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-glass);
  border-radius: 22px;
  background: var(--bg-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
}

.drawer-header { display: flex; flex-direction: column; gap: 10px; padding: 16px 18px 0; }
.drawer-back {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 570;
  cursor: pointer;
}
.drawer-back:hover { color: var(--accent); }
.drawer-back-arrow { flex: none; font-size: 14px; line-height: 1; }
.drawer-back-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.drawer-row { display: flex; align-items: flex-start; gap: 10px; }
.drawer-title-group { min-width: 0; flex: 1; }
.drawer-kicker { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
.drawer-title-group h3 { margin: 8px 0 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }
.drawer-corner { flex: none; display: flex; gap: 5px; }
.corner-button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: 11px;
  background: var(--bg-glass);
  color: var(--text-muted);
  font-size: 15px;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease;
}
.corner-button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

.drawer-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 0 18px max(18px, env(safe-area-inset-bottom)); }
.drawer-description { margin: 16px 0 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.drawer-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 18px 0 0; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }

.drawer-section { margin: 20px 0 0; padding: 18px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.drawer-section-heading { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 12px; }
.drawer-section-heading > div { display: flex; align-items: baseline; gap: 8px; }
.drawer-section-heading span { font-size: 12px; font-weight: 650; }
.drawer-section-heading strong { color: var(--text-faint); font-size: 11px; font-weight: 600; }
.drawer-add {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-glass);
  color: var(--accent);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
  transition: background var(--motion-fast) ease;
}
.drawer-add:hover { background: var(--bg-glass-strong); }
.drawer-section-empty { margin: 2px 0 0; color: var(--text-faint); font-size: 12px; }

.drawer-children { display: grid; gap: 6px; }
.drawer-child {
  display: grid;
  grid-template-columns: 10px minmax(0, 1fr) 16px;
  align-items: center;
  gap: 9px;
  min-height: 42px;
  padding: 8px 11px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: color-mix(in srgb, var(--text-primary) 3%, transparent);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  transition: background var(--motion-fast) ease, border-color var(--motion-fast) ease;
}
.drawer-child:hover { background: var(--bg-glass-strong); border-color: var(--border-subtle); }
.drawer-child strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 570; }
.drawer-child-chevron { color: var(--text-faint); font-size: 15px; text-align: right; }

.status-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: #8e8e93; }
.status-dot.status-in_progress { background: var(--accent); }
.status-dot.status-blocked { background: #ff9500; }
.status-dot.status-completed { background: #34c759; }
.status-dot.status-cancelled { background: #8e8e93; }

.drawer-activity { display: grid; gap: 0; }
/* Dividers breathe on both sides: the next entry carries the top border plus its own padding. */
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 12px 0; }
.drawer-activity-item + .drawer-activity-item { border-top: 1px solid var(--border-subtle); }
.drawer-activity-copy { min-width: 0; }
.drawer-activity-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.drawer-activity-head strong { min-width: 0; display: flex; align-items: center; }
.drawer-activity-head time { flex: none; padding-top: 1px; text-align: right; color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }
.drawer-activity-item .activity-dot { width: 9px; height: 9px; margin-top: 5px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.drawer-activity-item .activity-dot.audit { border-color: var(--text-faint); }
.drawer-activity-detail { margin: 6px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.5; }
.drawer-activity-note { margin: 5px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }

/* Collapse handle: rides the drawer's left edge, vertically centered. */
.drawer-collapse {
  position: absolute;
  left: -24px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 64px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-glass);
  border-right: 0;
  border-radius: 12px 0 0 12px;
  background: var(--bg-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  color: var(--text-muted);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease;
}
.drawer-collapse:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

/* Below 1150px: no room to push — overlay from the right with a backdrop. */
@media (max-width: 1149px) {
  .drawer-backdrop { display: block; position: fixed; inset: 0; z-index: 2300; background: color-mix(in srgb, #0f121a 32%, transparent); opacity: 0; pointer-events: none; transition: opacity var(--motion-normal) ease; }
  .drawer-backdrop.open { opacity: 1; pointer-events: auto; }
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; padding: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
  .subtask-drawer-slot.open { transform: translateX(0); }
  .subtask-drawer { width: 100%; height: 100%; border-radius: 22px 0 0 22px; }
}

@media (max-width: 768px) {
  .subtask-drawer-slot { width: min(88vw, 400px); }
  .drawer-header, .drawer-scroll { padding-left: 16px; padding-right: 16px; }
  .drawer-corner .corner-button { width: 38px; height: 38px; }
  .drawer-add { width: 36px; height: 36px; }
  .drawer-facts { grid-template-columns: 1fr 1fr; }
  .drawer-facts div:last-child { grid-column: 1 / -1; }
}

@media (prefers-reduced-motion: reduce) {
  .subtask-drawer-slot, .drawer-backdrop { transition-duration: 1ms !important; }
}
</style>
```

说明（供 reviewer 对照 spec §12）：props/emits 变更见 script；`drawer-collapse` 把手桌面端位于 slot 的 24px 把手槽（`padding-left: 24px`），<1150px 浮层下 `left: -24px` 悬于遮罩上；眉标为两枚胶囊（`.task-pill` 全局类）。

- [ ] **Step 3: 类型检查 + 测试（此时 Tasks.vue 尚未传新 props，vue-tsc 会报缺失 props 错误——本任务只需确认错误仅来自 Tasks.vue 的 `:children`/`:parent` 缺失）**

Run: `cd frontend && npm test`
Expected: 31 个测试通过（util 测试不受影响）。
Run: `cd frontend && npx vue-tsc -b`
Expected: FAIL，且错误信息仅涉及 `SubtaskDrawer` 缺少 `children`/`parent` 必填 props（在 Tasks.vue 的使用处）。这是预期的中间态，Task 3 修复。**不要**为消除该错误给 props 加默认值。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/App.vue frontend/src/components/tasks/SubtaskDrawer.vue
git commit -m "feat(tasks): restructure subtask drawer with children list and edge handle"
```

---

### Task 3: Tasks.vue 集成 — 新条目结构、胶囊眉标、抽屉 wiring、页面锁定

**Files:**
- Modify: `frontend/src/views/Tasks.vue`（以下 13 组锚点替换）

**Interfaces:**
- Consumes: Task 1 的 `TaskActivityEntry.detail`；Task 2 的 `SubtaskDrawer` props/emits 与 `.task-pill` 全局类。

- [ ] **Step 1: 模板锚点替换（4 组）**

**T3.1** 根元素挂视图锁定的动态类。
锚点：
```html
  <div class="tasks-page">
```
替换为：
```html
  <div class="tasks-page" :class="{ 'view-tasks': viewMode === 'tasks' }">
```

**T3.2** 眉标改胶囊。
锚点：
```html
              <div class="detail-kicker">
                <span class="status-dot" :class="`status-${detail.root.status}`"></span>
                {{ taskStatusLabel(detail.root.status) }} · {{ taskImportanceLabel(detail.root.importance) }}
              </div>
```
替换为：
```html
              <div class="detail-kicker">
                <span class="task-pill" :class="`status-${detail.root.status}`">{{ taskStatusLabel(detail.root.status) }}</span>
                <span class="task-pill" :class="`importance-${detail.root.importance}`">{{ taskImportanceLabel(detail.root.importance) }}</span>
              </div>
```

**T3.3** 活动流条目改「类型胶囊 + 明细」。
锚点：
```html
              <article v-for="item in activity" :key="item.id" class="activity-item">
                <span class="activity-dot" :class="item.type"></span>
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong>
                      <button type="button" class="activity-task" :title="item.taskTitle" @click="focusTask(item.taskId)">{{ item.taskTitle }}</button>
                      <span class="activity-sep">·</span>{{ item.title }}
                    </strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
                  <p v-if="item.note">{{ item.note }}</p>
                </div>
              </article>
```
替换为：
```html
              <article v-for="item in activity" :key="item.id" class="activity-item">
                <span class="activity-dot" :class="item.type"></span>
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong><span class="task-pill" :class="`type-${item.type}`">{{ item.title }}</span></strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
                  <p class="activity-meta">
                    <button type="button" class="activity-task" :title="item.taskTitle" @click="focusTask(item.taskId)">{{ item.taskTitle }}</button>
                    <span v-if="item.detail" class="activity-detail">{{ item.detail }}</span>
                  </p>
                  <p v-if="item.note" class="activity-note">{{ item.note }}</p>
                </div>
              </article>
```

**T3.4** 抽屉 wiring 补新 props/emit。
锚点：
```html
        <SubtaskDrawer
          ref="drawerRef"
          :node="drawerNode"
          :activity="drawerActivity"
          @close="closeDrawer"
```
替换为：
```html
        <SubtaskDrawer
          ref="drawerRef"
          :node="drawerNode"
          :activity="drawerActivity"
          :children="drawerChildren"
          :parent="drawerParent"
          @close="closeDrawer"
          @select="focusTask"
```

- [ ] **Step 2: 脚本锚点替换（1 组）**

**T3.5** 在 `drawerNode` 之后新增两个 computed。
锚点：
```ts
const drawerNode = computed(() => (drawerNodeId.value ? detail.value?.tasks.find((task) => task.id === drawerNodeId.value) || null : null))
```
替换为：
```ts
const drawerNode = computed(() => (drawerNodeId.value ? detail.value?.tasks.find((task) => task.id === drawerNodeId.value) || null : null))
// Direct children of the drawer's node (drill-down list) and its parent (back row).
const drawerChildren = computed(() => {
  const node = drawerNode.value
  if (!node) return []
  return (detail.value?.tasks ?? [])
    .filter((task) => task.parent_id === node.id)
    .sort((a, b) => a.position - b.position)
})
const drawerParent = computed(() => {
  const parentId = drawerNode.value?.parent_id
  if (!parentId) return null
  return detail.value?.tasks.find((task) => task.id === parentId) ?? null
})
```

- [ ] **Step 3: 样式锚点替换（8 组）**

**T3.6** 页面锁定（任务视图）。
锚点：
```css
.tasks-page { min-height: 100%; max-width: 100%; color: var(--text-primary); }
```
替换为：
```css
.tasks-page { min-height: 100%; max-width: 100%; color: var(--text-primary); }
/* Task view locks the page: only the list/detail/drawer panes scroll (Reader.vue pattern). */
.tasks-page.view-tasks { display: flex; flex-direction: column; height: calc(100vh - 64px); height: calc(100dvh - 64px); overflow: hidden; }
.tasks-page.view-tasks > :not(.task-workspace) { flex-shrink: 0; }
```

**T3.7** workspace 改为 flex 弹性项。
锚点：
```css
.task-workspace { min-height: 620px; height: calc(100dvh - 208px); display: grid; grid-template-columns: minmax(270px, 340px) minmax(0, 1fr); gap: 14px; }
```
替换为：
```css
.task-workspace { flex: 1 1 auto; min-height: 0; display: grid; grid-template-columns: minmax(270px, 340px) minmax(0, 1fr); gap: 14px; }
```

**T3.8** 眉标与状态圆点样式（圆点规则随眉标改胶囊而删除）。
锚点：
```css
.detail-kicker { display: flex; align-items: center; gap: 6px; color: var(--text-muted); font-size: 11px; font-weight: 580; }
.status-dot { width: 7px; height: 7px; border-radius: 50%; background: #8e8e93; box-shadow: 0 0 0 4px color-mix(in srgb, #8e8e93 10%, transparent); }
.status-in_progress { background: var(--accent); box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 11%, transparent); }
.status-blocked { background: #ff9500; }.status-completed { background: #34c759; }.status-cancelled { background: #8e8e93; }
```
替换为：
```css
.detail-kicker { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
```

**T3.9** 下半部分留白（facts/区块/面板底部，对齐阅境轩节奏）。
锚点一：
```css
.task-detail-panel { padding: 24px 26px; }
```
替换为：
```css
.task-detail-panel { padding: 24px 26px 34px; }
```
锚点二：
```css
.detail-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 22px 0; }
```
替换为：
```css
.detail-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 26px 0; }
```
锚点三：
```css
.progress-overview, .detail-section { padding: 16px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.detail-section { margin-top: 12px; }
```
替换为：
```css
.progress-overview, .detail-section { padding: 18px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.detail-section { margin-top: 18px; }
```

**T3.10** 活动流样式块（分隔线双侧呼吸 + 新条目结构）。
锚点：
```css
.activity-list { display: grid; gap: 0; }
.activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; min-height: 56px; }
.activity-copy { min-width: 0; padding: 0 0 14px 3px; border-bottom: 1px solid var(--border-subtle); }
.activity-item:last-child .activity-copy { border-bottom: 0; }
.activity-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
.activity-head strong { min-width: 0; display: flex; align-items: baseline; gap: 0; }
.activity-task { display: inline-block; max-width: 11em; padding: 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-task:hover { text-decoration: underline; }
.activity-sep { flex: none; margin: 0 5px; color: var(--text-faint); font-weight: 450; }
.activity-head time { flex: none; padding-top: 1px; text-align: right; font-variant-numeric: tabular-nums; }
.activity-dot { width: 9px; height: 9px; margin-top: 4px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.activity-dot.audit { border-color: var(--text-faint); }
.activity-item strong { font-size: 12px; }.activity-item p { margin: 5px 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }.activity-item time { color: var(--text-faint); font-size: 10px; }
```
替换为：
```css
.activity-list { display: grid; gap: 0; }
/* Dividers breathe on both sides: the next entry carries the top border plus its own padding. */
.activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 14px 0; }
.activity-item + .activity-item { border-top: 1px solid var(--border-subtle); }
.activity-copy { min-width: 0; }
.activity-head { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.activity-head strong { min-width: 0; display: flex; align-items: center; }
.activity-meta { display: flex; align-items: baseline; flex-wrap: wrap; gap: 7px; margin: 7px 0 0; }
.activity-task { display: inline-block; max-width: 11em; padding: 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-task:hover { text-decoration: underline; }
.activity-detail { color: var(--text-secondary); }
.activity-note { margin: 5px 0 0; color: var(--text-muted); line-height: 1.5; white-space: pre-wrap; }
.activity-head time { flex: none; padding-top: 1px; text-align: right; font-variant-numeric: tabular-nums; }
.activity-dot { width: 9px; height: 9px; margin-top: 5px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.activity-dot.audit { border-color: var(--text-faint); }
.activity-item time { color: var(--text-faint); font-size: 10px; }
```

**T3.11** 字号微调块对应更新（kicker 已胶囊化，删除死规则）。
锚点：
```css
.detail-kicker { font-size: 12px; }
```
替换为：（删除该行——空替换）
锚点二：
```css
.activity-item strong, .activity-item p { font-size: 13px; }
```
替换为：
```css
.activity-meta, .activity-note { font-size: 13px; }
```

**T3.12** 768px 媒体查询：页面锁定高度 + 面板占满（注意该块是一整行长 CSS，锚点为其子串）。
锚点：
```css
.task-workspace { min-height: calc(100dvh - 260px); height: auto; display: block; }.task-list-panel, .task-detail-panel { min-height: calc(100dvh - 260px); border-radius: 20px; }
```
替换为：
```css
.tasks-page.view-tasks { height: calc(100dvh - 96px - var(--safe-top) - var(--safe-bottom)); }.task-workspace { flex: 1 1 auto; min-height: 0; height: auto; display: block; }.task-list-panel, .task-detail-panel { height: 100%; min-height: 0; border-radius: 20px; }
```

- [ ] **Step 4: 门禁**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 类型零错误；31 个测试全部通过。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/views/Tasks.vue
git commit -m "feat(tasks): pill attributes, locked page layout, breathing activity feed"
```

---

### Task 4: 需求与开发文档更新

**Files:**
- Modify: `docs/requirement/09-task-management.md`（3 组锚点）
- Modify: `docs/development/09-task-management.md`（1 组锚点）

**Interfaces:**
- Consumes: 已落地的实现（Task 1-3）。

- [ ] **Step 1: 需求文档 §4.3.1 第 4 条**
锚点：
```md
4. 全树聚合的进展与记录时间线：根任务与所有子任务的进展、审计条目按时间倒序展示，每条标注所属任务名；点击任务名可聚焦该任务（子任务打开详情抽屉，根任务回到总览）。
```
替换为：
```md
4. 全树聚合的进展与记录时间线：根任务与所有子任务的进展、审计条目按时间倒序展示。条目标题为记录类型（如「进展」「状态变更」），明细行标注所属任务名与具体变化（完成度、状态流转），备注另起一行；点击任务名可聚焦该任务（子任务打开详情抽屉，根任务回到总览）。
```

- [ ] **Step 2: 需求文档 §4.3.4 整块**
锚点：
```md
- 详情面板固定展示根任务总览；点击任务树中的子任务，从右侧滑出抽屉展示该子任务的标题、状态、描述、日期、优先级与其专属进展记录。
- 桌面端（≥1150px）抽屉压缩详情面板让位；较窄屏幕与手机端以浮层从右侧滑入，点击遮罩关闭。
- 抽屉内提供记录进展、添加子任务、更改状态、编辑、移动操作，复用统一的表单弹层。
- 抽屉打开时点击其他子任务，抽屉内容直接切换；点击根任务行、关闭按钮或 Esc 收起抽屉。
```
替换为：
```md
- 详情面板固定展示根任务总览；点击任务树中的子任务，从右侧滑出抽屉展示该子任务的标题、状态、描述、日期、优先级、直属子任务列表与其专属进展记录。
- 抽屉头部提供「返回上级」入口；点击直属子任务列表在抽屉内下钻，父任务为根任务时返回即收起抽屉回总览。
- 桌面端（≥1150px）抽屉压缩详情面板让位；较窄屏幕与手机端以浮层从右侧滑入，点击遮罩关闭。抽屉左缘中部常驻收起符号，Esc 亦可收起。
- 抽屉右上角提供编辑、更改状态、移动操作；「记录进展」「添加子任务」分别为进展与子任务栏目标题行的加号按钮，复用统一的表单弹层。
- 关键属性（任务状态、重要程度、记录类型）以软色胶囊标注。
- 任务视图整页不滚动：仅任务列表、详情面板与抽屉内部各自滚动（桌面与移动端一致）；详情面板下半部分保留与阅境轩一致的疏朗留白。
```

- [ ] **Step 3: 需求文档 §10 验收标准追加两条**
锚点：
```md
- [ ] 点击子任务从右侧打开详情抽屉（桌面端主面板被压缩），抽屉内可完成记录进展、更改状态等操作，操作后数据即时刷新。
```
替换为：
```md
- [ ] 点击子任务从右侧打开详情抽屉（桌面端主面板被压缩），抽屉内可完成记录进展、更改状态等操作，操作后数据即时刷新。
- [ ] 抽屉展示直属子任务列表并可逐级下钻；编辑/更改状态/移动位于抽屉右上角，栏目加号触发记录进展与添加子任务；左缘收起符号、Esc、遮罩点击均可收起抽屉。
- [ ] 任务视图下页面本身不出现滚动条，仅面板内部滚动；活动流条目为「类型胶囊 + 任务名/变化明细」结构，分隔线两侧留有呼吸间距。
```

- [ ] **Step 4: 开发文档 §11.3 整块**
锚点：
```md
- 详情面板恒渲染根任务（总览、拆解树、聚合活动流）；`Tasks.vue` 以 `drawerNodeId` ref 驱动子任务抽屉，不再有整面板切换。
- `utils/taskActivity.ts` 的 `buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 负责全树聚合（省略 scope）与单任务过滤（传入 scope，供抽屉），条目携带 `taskTitle` 归因；`taskStatusLabel`/`taskImportanceLabel`/`formatTimestamp` 一并从视图层下沉到 utils。
- `SubtaskDrawer.vue` 常驻挂载：桌面端（≥1150px）以 flex 定宽 slot 实现 push 压缩；<1150px 转为 fixed 浮层 + 遮罩；≤768px 宽度 min(88vw, 400px)。z-index 2300/2301，低于 MotionModal 的 2400。
- 抽屉操作全部 emit 回 `Tasks.vue` 复用 sheet 表单体系；写入成功后子任务目标自动打开其抽屉，根任务目标回到总览。
```
替换为：
```md
- 详情面板恒渲染根任务（总览、拆解树、聚合活动流）；`Tasks.vue` 以 `drawerNodeId` ref 驱动子任务抽屉，不再有整面板切换。
- `utils/taskActivity.ts` 的 `buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 负责全树聚合（省略 scope）与单任务过滤（传入 scope，供抽屉）；条目 `title` 为名词式类型标签（进展/状态变更/移动…），`detail` 携带具体变化（完成度 X%、状态 A → B），`taskTitle` 归因；`taskStatusLabel`/`taskImportanceLabel`/`formatTimestamp` 一并从视图层下沉到 utils。
- `SubtaskDrawer.vue` 常驻挂载：桌面端（≥1150px）以 flex 定宽 slot 实现 push 压缩（slot 总宽 `min(400px, 30vw) + 24px`，其中 24px 为左缘把手槽）；<1150px 转为 fixed 浮层 + 遮罩；≤768px 宽度 min(88vw, 400px)。z-index 2300/2301，低于 MotionModal 的 2400。
- 抽屉结构：头部「‹ 返回上级」行（emit `select`，父级绑 `focusTask`）+ 眉标属性胶囊 + 右上角编辑/更改状态/移动图标按钮（EditPen/Refresh/Rank）；正文为「子任务」栏目（直属子任务列表，点击下钻）与「进展与记录」栏目（栏目标题行「＋」分别为添加子任务/记录进展）；左缘中部「‹」收起把手替代关闭按钮（打开时键盘焦点落于把手）。
- 抽屉操作全部 emit 回 `Tasks.vue` 复用 sheet 表单体系；写入成功后子任务目标自动打开其抽屉，根任务目标回到总览。
- 布局锁定：`.tasks-page.view-tasks` 为 `calc(100dvh - 64px)` 的 flex 列（移动端 `calc(100dvh - 96px - safe-top - safe-bottom)`），workspace `flex: 1 1 auto; min-height: 0`，仅面板内部滚动；属性胶囊 `.task-pill`（status-*/importance-*/type-*）定义于 App.vue 全局样式块，主面板与抽屉共用。
```

- [ ] **Step 5: Commit**

```bash
git add docs/requirement/09-task-management.md docs/development/09-task-management.md
git commit -m "docs(tasks): record adjustment-wave interactions and layout lock"
```

---

### Task 5: 验证与构建产物

**Files:**
- Build: `frontend/dist_new/`（vite 产物）、`backend/target/release/obsidian-brain`（rust-embed 嵌入新前端）

**Interfaces:**
- Consumes: Task 1-4 的全部提交。

- [ ] **Step 1: 完整门禁**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 类型零错误；31 个测试全部通过。

- [ ] **Step 2: 构建前端产物**

Run: `cd frontend && npx vite build --outDir dist_new`
Expected: 构建成功；`dist_new/` 文件数与此前同量级（168 左右，±若 chunk 数变化属正常）。

- [ ] **Step 3: 构建 release 二进制（验证 rust-embed 嵌入新前端可编译）**

Run: `cd backend && cargo build --release`
Expected: 编译成功。**不要安装、不要重启任何进程**（安装由用户执行）。

- [ ] **Step 4: 报告**

报告 `dist_new` 文件数、二进制 `ls -la backend/target/release/obsidian-brain` 的大小与 mtime、门禁结果。无需 commit（构建产物不入库）。

---

## Self-Review 结论

- Spec §12 五小节均有对应任务：12.1→Task 2；12.2→Task 1+3；12.3→Task 2+3；12.4→Task 3（T3.6/7/9/12）；12.5→Task 2+3（T3.4/5）。
- 无占位符；Task 1/2 为整文件替换，Task 3/4 为精确锚点替换。
- 类型一致性：`TaskActivityEntry.detail`（Task 1 定义 → Task 2/3 消费）；`SubtaskDrawer` props `children`/`parent`、emit `select`（Task 2 定义 → Task 3 T3.4/T3.5 绑定）；`.task-pill` 修饰类（Task 2 定义 → Task 3 T3.2/T3.3 消费）。
- 中间态说明：Task 2 完成时 vue-tsc 会报 Tasks.vue 缺 props 的错误，属预期，Task 3 消除。
