# 任务进展归因 + 子任务详情抽屉 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 任务详情主面板固定展示根任务总览，进展与记录聚合全树条目并逐条标注所属任务名；点击子任务改为右侧滑出抽屉展示子任务详情与专属进展流，桌面端压缩主面板让位。

**Architecture:** 纯前端改动。聚合逻辑抽为纯函数 `utils/taskActivity.ts`（node --test 可测）；新组件 `components/tasks/SubtaskDrawer.vue` 承载抽屉（push/overlay 双形态）；`views/Tasks.vue` 移除 activeNode 面板切换体系，改由 `drawerNodeId` 状态驱动抽屉，主面板恒渲染 `detail.root`。后端、API、数据模型零改动。

**Tech Stack:** Vue 3 `<script setup lang="ts">` + scoped CSS、既有设计令牌（`--bg-glass`/`--motion-*`/`--ease-*`）、`node --test --experimental-strip-types`。

**Spec:** `docs/superpowers/specs/2026-08-22-tasks-activity-drawer-design.md`

## Global Constraints

- 后端零改动：不碰 `backend/` 下任何文件、migrations、Tool API。
- 不触碰 live vault（`~/Documents/Obsidian/TiercelChow's Blog/`）与 `~/.obsidian-brain/brain.db`。
- 不修改 `frontend/src/components/tasks/TaskTree.vue`（交互语义变化全部落在父级 Tasks.vue）。
- utils 模块跨模块引用只允许 `import type`（node --test 走类型剥离，运行时 import 别名/无扩展名会失败）；utils 内部遵循现有相对导入风格 `import type { ... } from '../api/tasks'`。
- 抽屉 z-index：backdrop 2300、抽屉 2301，必须低于 MotionModal 的 2400。
- 每个 commit 前必须通过门禁（在 `frontend/` 目录）：`npx vue-tsc -b && npm test`（当前 24 测试 + 新增测试全绿）。
- Commit 遵循 Conventional Commits：`feat(tasks): ...`、`docs(tasks): ...`。
- 样式沿用既有设计令牌与毛玻璃配方，禁止引入新依赖。
- 当前分支：`feature/tasks-subtask-drawer`（基于 `refactor/tasks-decouple-obsidian`，所有工作提交在此分支）。

---

### Task 1: `taskActivity` 纯函数模块 + `formatTimestamp` 入驻 taskDates（TDD）

**Files:**
- Create: `frontend/src/utils/taskActivity.ts`
- Test: `frontend/tests/taskActivity.test.ts`（新建）
- Modify: `frontend/src/utils/taskDates.ts`（新增 `formatTimestamp`）
- Test: `frontend/tests/taskDates.test.ts`（追加用例）

**Interfaces:**
- Consumes: 类型 `TaskNode`/`ProgressEntry`/`AuditEvent`/`TaskStatus`/`TaskImportance`（来自 `frontend/src/api/tasks.ts`，仅 type import）。
- Produces（Task 2/3 依赖，签名逐字使用）:
  ```ts
  export interface TaskActivityEntry {
    id: string                 // 'progress:{uuid}' / 'audit:{uuid}'
    type: 'progress' | 'audit'
    taskId: string
    taskTitle: string
    title: string
    note: string | null
    time: string
  }
  export function taskStatusLabel(status: TaskStatus): string
  export function taskImportanceLabel(importance: TaskImportance): string
  export function buildTaskActivity(
    nodes: readonly TaskNode[],
    progress: readonly ProgressEntry[],
    audit: readonly AuditEvent[],
    scopeTaskId?: string,
  ): TaskActivityEntry[]
  ```
  以及 `taskDates.ts` 新增 `export function formatTimestamp(value: string): string`。

- [ ] **Step 1: 写失败测试 `frontend/tests/taskActivity.test.ts`**

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

test('aggregates progress and audit across the whole tree with task attribution', () => {
  const entries = buildTaskActivity(nodes, progress, audit)

  assert.deepEqual(entries.map((entry) => [entry.id, entry.taskTitle, entry.title]), [
    ['progress:p2', '子任务甲', '记录了新进展'],
    ['audit:a2', '子任务甲', '移动了任务'],
    ['audit:a1', '根任务', '状态变为进行中'],
    ['progress:p1', '根任务', '进展更新为 35%'],
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

test('status and importance labels cover enum values', () => {
  assert.equal(taskStatusLabel('open'), '待处理')
  assert.equal(taskStatusLabel('in_progress'), '进行中')
  assert.equal(taskStatusLabel('cancelled'), '已取消')
  assert.equal(taskImportanceLabel('urgent'), '紧急')
  assert.equal(taskImportanceLabel('normal'), '普通')
})
```

- [ ] **Step 2: 追加失败测试到 `frontend/tests/taskDates.test.ts`**

在文件顶部现有 `import { ... } from '../src/utils/taskDates.ts'` 中加入 `formatTimestamp`，文件末尾追加：

```ts
test('formatTimestamp keeps unparseable values verbatim', () => {
  assert.equal(formatTimestamp('not-a-date'), 'not-a-date')
})

test('formatTimestamp renders month/day and hh:mm', () => {
  assert.match(formatTimestamp('2026-08-21T09:52:16Z'), /^\d{1,2}\/\d{1,2},? \d{2}:\d{2}$/)
})
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend && npm test`
Expected: FAIL —— `Cannot find module .../src/utils/taskActivity.ts`，以及 taskDates 测试中 `formatTimestamp is not a function`。

- [ ] **Step 4: 实现 `frontend/src/utils/taskActivity.ts`**

```ts
import type { AuditEvent, ProgressEntry, TaskImportance, TaskNode, TaskStatus } from '../api/tasks'

/** A single attributed row in the task activity feed (progress or audit). */
export interface TaskActivityEntry {
  id: string
  type: 'progress' | 'audit'
  taskId: string
  taskTitle: string
  title: string
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

function auditTitle(item: Pick<AuditEvent, 'event_type' | 'to_status'>): string {
  if (item.event_type === 'status_changed' && item.to_status) return `状态变为${taskStatusLabel(item.to_status)}`
  return ({ created: '创建了任务', updated: '更新了任务', moved: '移动了任务', archived: '归档了任务', reopened: '重新打开任务' } as Record<string, string>)[item.event_type] || '任务发生变化'
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
      title: item.percent_after == null ? '记录了新进展' : `进展更新为 ${item.percent_after}%`,
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
      note: item.note,
      time: item.occurred_at,
    }))
  return [...progressEntries, ...auditEntries].sort((a, b) => b.time.localeCompare(a.time))
}
```

- [ ] **Step 5: 在 `frontend/src/utils/taskDates.ts` 末尾追加 `formatTimestamp`**

（从 `views/Tasks.vue` 的本地函数原样迁移——Task 3 会删除原副本。）

```ts
/** Format an ISO timestamp as "M/D HH:mm" for activity feeds; unparseable values pass through. */
export function formatTimestamp(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend && npx vue-tsc -b && npm test`
Expected: PASS —— 新增 6 个测试 + 原 24 个测试全绿（30 tests）。

- [ ] **Step 7: Commit**

```bash
git add frontend/src/utils/taskActivity.ts frontend/src/utils/taskDates.ts frontend/tests/taskActivity.test.ts frontend/tests/taskDates.test.ts
git commit -m "feat(tasks): add attributed activity builder util"
```

---

### Task 2: `SubtaskDrawer` 组件

**Files:**
- Create: `frontend/src/components/tasks/SubtaskDrawer.vue`

**Interfaces:**
- Consumes（Task 1 产出）: `taskStatusLabel` / `taskImportanceLabel`（`@/utils/taskActivity`）、`formatTimestamp`（`@/utils/taskDates`）、类型 `TaskNode`（`@/api/tasks`）、`TaskActivityEntry`（`@/utils/taskActivity`）。
- Produces（Task 3 依赖，逐字使用）:
  ```ts
  props: { node: TaskNode | null; activity: TaskActivityEntry[] }   // node=null 为关闭态；组件常驻挂载（非 v-if）
  emits: { close: []; progress: [task: TaskNode]; add: [task: TaskNode]; status: [task: TaskNode]; edit: [task: TaskNode]; move: [task: TaskNode] }
  expose: { revealActivity(): void }
  ```
  组件根为两个兄弟节点（backdrop + slot），作为 `.task-detail-zone`（flex）的子项参与桌面端 push 布局。

说明：SFC 无组件级测试设施（项目测试策略为纯逻辑入 utils + node --test），本任务以 `vue-tsc -b` 类型检查为自动验证，交互在 Task 5 后由用户手动验收。

- [ ] **Step 1: 创建 `frontend/src/components/tasks/SubtaskDrawer.vue`（完整内容如下）**

```vue
<template>
  <div class="drawer-backdrop" :class="{ open: !!node }" @click="emit('close')"></div>
  <div class="subtask-drawer-slot" :class="{ open: !!node }">
    <aside v-if="node" class="subtask-drawer" role="dialog" aria-label="子任务详情">
      <header class="drawer-header">
        <button ref="closeButtonRef" type="button" aria-label="关闭子任务详情" @click="emit('close')">✕</button>
        <div class="drawer-title-group">
          <div class="drawer-kicker">
            <span class="status-dot" :class="`status-${node.status}`"></span>
            子任务 · {{ taskStatusLabel(node.status) }} · {{ taskImportanceLabel(node.importance) }}
          </div>
          <h3>{{ node.title }}</h3>
        </div>
      </header>

      <div class="drawer-scroll">
        <p v-if="node.description" class="drawer-description">{{ node.description }}</p>

        <div class="drawer-facts">
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
          <div><span>优先级</span><strong>{{ taskImportanceLabel(node.importance) }}</strong></div>
        </div>

        <div class="drawer-actions">
          <button type="button" @click="emit('progress', node)">记录进展</button>
          <button type="button" @click="emit('add', node)">添加子任务</button>
          <button type="button" @click="emit('status', node)">更改状态</button>
          <button type="button" @click="emit('edit', node)">编辑</button>
          <button type="button" @click="emit('move', node)">移动</button>
        </div>

        <section ref="activityRef" class="drawer-section">
          <div class="drawer-section-heading">
            <span>进展与记录</span>
            <strong>{{ activity.length }} 条</strong>
          </div>
          <div v-if="activity.length" class="drawer-activity">
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item">
              <span class="activity-dot" :class="item.type"></span>
              <div class="drawer-activity-copy">
                <div class="drawer-activity-head">
                  <strong>{{ item.title }}</strong>
                  <time>{{ formatTimestamp(item.time) }}</time>
                </div>
                <p v-if="item.note">{{ item.note }}</p>
              </div>
            </article>
          </div>
          <p v-else class="drawer-activity-empty">暂无进展记录</p>
        </section>
      </div>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import type { TaskNode } from '@/api/tasks'
import { taskImportanceLabel, taskStatusLabel, type TaskActivityEntry } from '@/utils/taskActivity'
import { formatTimestamp } from '@/utils/taskDates'

const props = defineProps<{
  node: TaskNode | null
  activity: TaskActivityEntry[]
}>()

const emit = defineEmits<{
  close: []
  progress: [task: TaskNode]
  add: [task: TaskNode]
  status: [task: TaskNode]
  edit: [task: TaskNode]
  move: [task: TaskNode]
}>()

const activityRef = ref<HTMLElement | null>(null)
const closeButtonRef = ref<HTMLButtonElement | null>(null)

// Move focus into the drawer when it opens so keyboard users land on the close affordance.
watch(() => props.node, async (node) => {
  await nextTick()
  if (node) closeButtonRef.value?.focus()
})

function revealActivity() {
  activityRef.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ revealActivity })
</script>

<style scoped>
.drawer-backdrop { display: none; }

/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone. */
.subtask-drawer-slot { flex: 0 0 auto; width: 0; overflow: hidden; transition: width var(--motion-normal) var(--ease-emphasized), margin var(--motion-normal) var(--ease-emphasized); }
.subtask-drawer-slot.open { width: min(400px, 30vw); margin-left: 14px; }
.subtask-drawer {
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

.drawer-header { display: flex; align-items: flex-start; gap: 12px; padding: 18px 18px 0; }
.drawer-header > button { flex: none; width: 34px; height: 34px; border: 0; border-radius: 11px; background: transparent; color: var(--text-faint); font-size: 15px; cursor: pointer; }
.drawer-header > button:hover { background: var(--bg-glass-strong); color: var(--text-primary); }
.drawer-title-group { min-width: 0; }
.drawer-kicker { display: flex; align-items: center; gap: 6px; color: var(--text-muted); font-size: 11px; font-weight: 580; }
.drawer-kicker .status-dot { flex: none; width: 7px; height: 7px; border-radius: 50%; background: #8e8e93; box-shadow: 0 0 0 4px color-mix(in srgb, #8e8e93 10%, transparent); }
.drawer-kicker .status-dot.status-in_progress { background: var(--accent); box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 11%, transparent); }
.drawer-kicker .status-dot.status-blocked { background: #ff9500; }
.drawer-kicker .status-dot.status-completed { background: #34c759; }
.drawer-kicker .status-dot.status-cancelled { background: #8e8e93; }
.drawer-title-group h3 { margin: 7px 0 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }

.drawer-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 0 18px max(18px, env(safe-area-inset-bottom)); }
.drawer-description { margin: 14px 0 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.drawer-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 16px 0 0; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
.drawer-actions { display: flex; flex-wrap: wrap; gap: 7px; margin: 16px 0 0; }
.drawer-actions button { min-height: 38px; padding: 0 11px; border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--text-secondary); font-weight: 580; cursor: pointer; }
.drawer-actions button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

.drawer-section { margin: 18px 0 0; padding: 16px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.drawer-section-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 6px; }
.drawer-section-heading span { font-size: 12px; font-weight: 650; }
.drawer-section-heading strong { color: var(--text-faint); font-size: 11px; font-weight: 600; }
.drawer-activity { display: grid; gap: 0; }
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; min-height: 56px; }
.drawer-activity-copy { min-width: 0; padding: 0 0 14px 3px; border-bottom: 1px solid var(--border-subtle); }
.drawer-activity-item:last-child .drawer-activity-copy { border-bottom: 0; }
.drawer-activity-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
.drawer-activity-head strong { min-width: 0; font-size: 12px; }
.drawer-activity-head time { flex: none; padding-top: 1px; text-align: right; color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }
.drawer-activity-item .activity-dot { width: 9px; height: 9px; margin-top: 4px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.drawer-activity-item .activity-dot.audit { border-color: var(--text-faint); }
.drawer-activity-item p { margin: 5px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }
.drawer-activity-empty { margin: 6px 0 0; color: var(--text-faint); font-size: 12px; }

/* Below 1150px: no room to push — overlay from the right with a backdrop. */
@media (max-width: 1149px) {
  .drawer-backdrop { display: block; position: fixed; inset: 0; z-index: 2300; background: color-mix(in srgb, #0f121a 32%, transparent); opacity: 0; pointer-events: none; transition: opacity var(--motion-normal) ease; }
  .drawer-backdrop.open { opacity: 1; pointer-events: auto; }
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
  .subtask-drawer-slot.open { transform: translateX(0); }
  .subtask-drawer { width: 100%; height: 100%; border-radius: 22px 0 0 22px; }
}

@media (max-width: 768px) {
  .subtask-drawer-slot { width: min(88vw, 400px); }
  .drawer-header, .drawer-scroll { padding-left: 16px; padding-right: 16px; }
  .drawer-actions button { min-height: 44px; }
  .drawer-facts { grid-template-columns: 1fr 1fr; }
  .drawer-facts div:last-child { grid-column: 1 / -1; }
}

@media (prefers-reduced-motion: reduce) {
  .subtask-drawer-slot, .drawer-backdrop { transition-duration: 1ms !important; }
}
</style>
```

- [ ] **Step 2: 类型检查通过**

Run: `cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend && npx vue-tsc -b && npm test`
Expected: PASS（30 tests，组件未被引用但须类型无误；`noUnusedLocals` 不应报错——组件内所有绑定均被模板使用）。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/tasks/SubtaskDrawer.vue
git commit -m "feat(tasks): add subtask detail drawer component"
```

---

### Task 3: Tasks.vue 集成 —— 主面板固定根任务 + 抽屉接入 + 活动流归因

**Files:**
- Modify: `frontend/src/views/Tasks.vue`

**Interfaces:**
- Consumes（Task 1/2 产出，签名见上）: `buildTaskActivity`/`taskStatusLabel`/`taskImportanceLabel`（`@/utils/taskActivity`）、`formatTimestamp`（`@/utils/taskDates`）、`SubtaskDrawer` 组件（props/emits/expose 如上）。
- Produces: 无（终端集成任务）。行为契约：主面板恒渲染 `detail.root`；`drawerNodeId` 驱动抽屉；活动流为全树聚合 + 可点击任务名标签。

本任务在一个文件内完成模板、脚本、样式三段修改。以下每处修改以「锚点原文 → 替换后」给出，锚点为当前文件中的唯一原文。

- [ ] **Step 1: 模板 —— 包裹 detail zone 并接入抽屉**

锚点（第 117 行起）：
```html
      <main class="task-detail-panel glass-surface">
```
替换为：
```html
      <div class="task-detail-zone">
        <main class="task-detail-panel glass-surface">
```

锚点（第 202-203 行，detail panel 结束处）：
```html
        </div>
      </main>
    </div>

    <TaskCalendar
```
替换为：
```html
        </div>
        </main>
        <SubtaskDrawer
          ref="drawerRef"
          :node="drawerNode"
          :activity="drawerActivity"
          @close="closeDrawer"
          @progress="openProgress"
          @add="openSubtask"
          @status="openStatus"
          @edit="openEdit"
          @move="openMove"
        />
      </div>
    </div>

    <TaskCalendar
```
（内层 `</div>` 为原 empty-detail 的闭合，顺序保持；main 的内部内容整体缩进可交给编辑器格式化，不强求。）

- [ ] **Step 2: 模板 —— 主面板全部 `activeNode` 引用改为 `detail.root`**

逐处替换：

```html
        <template v-else-if="detail && activeNode">
```
→
```html
        <template v-else-if="detail">
```

```html
                <span class="status-dot" :class="`status-${activeNode.status}`"></span>
                {{ statusLabel(activeNode.status) }} · {{ importanceLabel(activeNode.importance) }}
              </div>
              <h2>{{ activeNode.title }}</h2>
              <p>{{ activeNode.description || '这个任务还没有描述。' }}</p>
```
→
```html
                <span class="status-dot" :class="`status-${detail.root.status}`"></span>
                {{ taskStatusLabel(detail.root.status) }} · {{ taskImportanceLabel(detail.root.importance) }}
              </div>
              <h2>{{ detail.root.title }}</h2>
              <p>{{ detail.root.description || '这个任务还没有描述。' }}</p>
```

```html
              <button type="button" @click="openStatus(activeNode)">更改状态</button>
              <button type="button" @click="openEdit(activeNode)">编辑</button>
```
→
```html
              <button type="button" @click="openStatus(detail.root)">更改状态</button>
              <button type="button" @click="openEdit(detail.root)">编辑</button>
```

```html
          <div class="detail-facts">
            <div><span>开始</span><strong>{{ activeNode.start_date }}</strong></div>
            <div><span>结束</span><strong>{{ activeNode.end_date }}</strong></div>
            <div><span>优先级</span><strong>{{ importanceLabel(activeNode.importance) }}</strong></div>
          </div>
```
→
```html
          <div class="detail-facts">
            <div><span>开始</span><strong>{{ detail.root.start_date }}</strong></div>
            <div><span>结束</span><strong>{{ detail.root.end_date }}</strong></div>
            <div><span>优先级</span><strong>{{ taskImportanceLabel(detail.root.importance) }}</strong></div>
          </div>
```

```html
              <button type="button" @click="openSubtask(activeNode)">＋ 添加子任务</button>
```
→
```html
              <button type="button" @click="openSubtask(detail.root)">＋ 添加子任务</button>
```

```html
            <TaskTree
              :tasks="detail.tasks"
              :selected-id="activeNodeId"
              @select="selectNode"
```
→
```html
            <TaskTree
              :tasks="detail.tasks"
              :selected-id="drawerNodeId || detail.root.id"
              @select="focusTask"
```

```html
              <button type="button" @click="openProgress(activeNode)">＋ 添加进展</button>
```
→
```html
              <button type="button" @click="openProgress(detail.root)">＋ 添加进展</button>
```

```html
            <button v-else type="button" class="activity-empty" @click="openProgress(activeNode)">记录第一条进展</button>
```
→
```html
            <button v-else type="button" class="activity-empty" @click="openProgress(detail.root)">记录第一条进展</button>
```

- [ ] **Step 3: 模板 —— 活动流条目加任务名标签**

锚点：
```html
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong>{{ item.title }}</strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
```
替换为：
```html
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong>
                      <button type="button" class="activity-task" :title="item.taskTitle" @click="focusTask(item.taskId)">{{ item.taskTitle }}</button>
                      <span class="activity-sep">·</span>{{ item.title }}
                    </strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
```

- [ ] **Step 4: 脚本 —— imports**

锚点：
```ts
import MotionModal from '@/components/motion/MotionModal.vue'
import TaskCalendar from '@/components/tasks/TaskCalendar.vue'
import TaskTree from '@/components/tasks/TaskTree.vue'
```
→
```ts
import MotionModal from '@/components/motion/MotionModal.vue'
import SubtaskDrawer from '@/components/tasks/SubtaskDrawer.vue'
import TaskCalendar from '@/components/tasks/TaskCalendar.vue'
import TaskTree from '@/components/tasks/TaskTree.vue'
```

锚点：
```ts
import { computed, nextTick, onMounted, ref, watch } from 'vue'
```
→
```ts
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
```

锚点：
```ts
import {
  addLocalDays,
  buildMonthGrid,
  formatTaskDateRange,
  shiftMonth,
  todayLocal,
} from '@/utils/taskDates'
import { taskFieldsPayload } from '@/utils/taskPayloads'
```
→
```ts
import {
  addLocalDays,
  buildMonthGrid,
  formatTaskDateRange,
  formatTimestamp,
  shiftMonth,
  todayLocal,
} from '@/utils/taskDates'
import { buildTaskActivity, taskImportanceLabel, taskStatusLabel } from '@/utils/taskActivity'
import { taskFieldsPayload } from '@/utils/taskPayloads'
```

- [ ] **Step 5: 脚本 —— 状态替换（activeNodeId → drawerNodeId）**

锚点：
```ts
const activeNodeId = ref<string | null>(null)
const detailHeaderRef = ref<HTMLElement | null>(null)
```
→
```ts
const drawerNodeId = ref<string | null>(null)
const drawerRef = ref<InstanceType<typeof SubtaskDrawer> | null>(null)
const detailHeaderRef = ref<HTMLElement | null>(null)
```

- [ ] **Step 6: 脚本 —— computeds 替换**

锚点：
```ts
const detail = computed(() => store.selectedDetail)
const activeNode = computed(() => detail.value?.tasks.find((task) => task.id === activeNodeId.value) || detail.value?.root || null)
```
→
```ts
const detail = computed(() => store.selectedDetail)
const drawerNode = computed(() => (drawerNodeId.value ? detail.value?.tasks.find((task) => task.id === drawerNodeId.value) || null : null))
```

锚点（整个 activity computed）：
```ts
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
```
→
```ts
// Root overview shows the whole tree's history with per-task attribution;
// the drawer gets the focused feed for its own node.
const activity = computed(() => buildTaskActivity(detail.value?.tasks ?? [], detail.value?.progress ?? [], detail.value?.audit ?? []))
const drawerActivity = computed(() => (drawerNode.value ? buildTaskActivity(detail.value?.tasks ?? [], detail.value?.progress ?? [], detail.value?.audit ?? [], drawerNode.value.id) : []))
```

- [ ] **Step 7: 脚本 —— openTask / closeMobileDetail**

锚点：
```ts
  const loaded = await store.loadDetail(id).catch(() => null)
  if (!loaded) return
  activeNodeId.value = loaded.tasks.some(task => task.id === id) ? id : loaded.root.id
```
→
```ts
  const loaded = await store.loadDetail(id).catch(() => null)
  if (!loaded) return
  drawerNodeId.value = null
```

锚点：
```ts
function closeMobileDetail() {
  store.clearSelection()
  activeNodeId.value = null
```
→
```ts
function closeMobileDetail() {
  store.clearSelection()
  drawerNodeId.value = null
```

- [ ] **Step 8: 脚本 —— selectNode 替换为 focusTask + closeDrawer + 抽屉 watch + Esc**

锚点：
```ts
async function selectNode(id: string) {
  activeNodeId.value = id
  await revealDetailEl(detailHeaderRef.value)
}
```
→
```ts
/** Focus a task: subtasks surface in the right drawer, the root closes it back to the overview. */
function focusTask(id: string) {
  if (!detail.value || id === detail.value.root.id) {
    closeDrawer()
    return
  }
  drawerNodeId.value = id
}

function closeDrawer() {
  drawerNodeId.value = null
}

// Switching to another task in the list must not keep a stale drawer open.
watch(() => store.selectedDetail?.root.id, () => {
  drawerNodeId.value = null
})

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !drawerNodeId.value || sheetOpen.value || archiveConfirmOpen.value) return
  closeDrawer()
}
```

- [ ] **Step 9: 脚本 —— submitSheet 写入后聚焦受影响任务**

锚点（整个 submitSheet 函数体的 try 块）：
```ts
    const fields = taskFieldsPayload(form.value)
    const mode = sheetMode.value
    if (mode === 'create') await store.create(form.value.kind, fields)
    else if (mode === 'edit' && targetNode.value) await store.update(targetNode.value.id, fields)
    else if (mode === 'subtask' && targetNode.value) await store.addSubtask(targetNode.value.id, fields)
    else if (mode === 'progress' && targetNode.value) await store.addProgress(targetNode.value.id, progressForm.value.note, progressForm.value.includePercent ? progressForm.value.percent : undefined)
    else if (mode === 'status' && targetNode.value) await store.setStatus(targetNode.value.id, statusForm.value.status, statusForm.value.note || undefined, statusForm.value.cascade)
    else if (mode === 'move' && targetNode.value) await store.moveSubtask(targetNode.value.id, moveForm.value.parentId, 9999)
    sheetOpen.value = false
    activeNodeId.value = targetNode.value?.id || store.selectedDetail?.root.id || null
    await refreshCurrent()
    ElMessage.success(`${sheetAction.value}成功`)
    // Show the user what changed: the new activity entry for progress writes,
    // the refreshed header for everything else.
    await revealDetailEl(mode === 'progress' ? activitySectionRef.value : detailHeaderRef.value)
```
→
```ts
    const fields = taskFieldsPayload(form.value)
    const mode = sheetMode.value
    const writeTarget = targetNode.value
    if (mode === 'create') await store.create(form.value.kind, fields)
    else if (mode === 'edit' && writeTarget) await store.update(writeTarget.id, fields)
    else if (mode === 'subtask' && writeTarget) await store.addSubtask(writeTarget.id, fields)
    else if (mode === 'progress' && writeTarget) await store.addProgress(writeTarget.id, progressForm.value.note, progressForm.value.includePercent ? progressForm.value.percent : undefined)
    else if (mode === 'status' && writeTarget) await store.setStatus(writeTarget.id, statusForm.value.status, statusForm.value.note || undefined, statusForm.value.cascade)
    else if (mode === 'move' && writeTarget) await store.moveSubtask(writeTarget.id, moveForm.value.parentId, 9999)
    sheetOpen.value = false
    // Surface the affected task: subtask writes open its drawer, root writes stay on the overview.
    drawerNodeId.value = writeTarget && writeTarget.role === 'subtask' ? writeTarget.id : null
    await refreshCurrent()
    ElMessage.success(`${sheetAction.value}成功`)
    // Show the user what changed: progress writes reveal the new entry (in the
    // drawer when one is open, otherwise the panel's activity section); other
    // writes reveal the refreshed header when no drawer is open.
    if (mode === 'progress' && drawerNodeId.value) {
      await nextTick()
      drawerRef.value?.revealActivity()
    } else if (!drawerNodeId.value) {
      await revealDetailEl(mode === 'progress' ? activitySectionRef.value : detailHeaderRef.value)
    }
```

- [ ] **Step 10: 脚本 —— 删除本地重复函数，挂载 Esc 监听**

删除以下三个函数（锚点整段删除）：
```ts
function statusLabel(status: TaskStatus) {
  return statusOptions.find((item) => item.value === status)?.label || status
}

function importanceLabel(importance: TaskImportance) {
  return importanceOptions.find((item) => item.value === importance)?.label || importance
}
```
```ts
function formatTimestamp(value: string) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}
```
```ts
function auditTitle(item: { event_type: string; from_status: TaskStatus | null; to_status: TaskStatus | null }) {
  if (item.event_type === 'status_changed' && item.to_status) return `状态变为${statusLabel(item.to_status)}`
  return ({ created: '创建了任务', updated: '更新了任务', moved: '移动了任务', archived: '归档了任务', reopened: '重新打开任务' } as Record<string, string>)[item.event_type] || '任务发生变化'
}
```

模板中仅剩的 `statusLabel(` 引用（任务卡片 footer）：
```html
                    <span>{{ statusLabel(task.status) }}</span>
```
→
```html
                    <span>{{ taskStatusLabel(task.status) }}</span>
```

锚点（onMounted 处挂载/卸载 Esc 监听）：
```ts
onMounted(async () => {
  await store.loadTasks(taskFilters()).catch(() => [])
```
→
```ts
onMounted(() => {
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})

onMounted(async () => {
  await store.loadTasks(taskFilters()).catch(() => [])
```

注意：删除本地函数后，`TaskStatus`/`TaskImportance` 的 type import 仍被 `statusForm`/`statusOptions`/`importanceOptions` 等使用，import 保持不变。

- [ ] **Step 11: 样式 —— detail zone 与任务名标签**

锚点：
```css
.task-list-panel, .task-detail-panel { border-radius: 22px; min-height: 0; overflow: auto; }
```
→
```css
.task-detail-zone { min-width: 0; min-height: 0; display: flex; }
.task-detail-zone .task-detail-panel { flex: 1 1 0; }
.task-list-panel, .task-detail-panel { border-radius: 22px; min-height: 0; overflow: auto; }
```

锚点（activity-head strong 一组样式之后追加）：
```css
.activity-head strong { min-width: 0; }
```
→
```css
.activity-head strong { min-width: 0; display: flex; align-items: baseline; gap: 0; }
.activity-task { display: inline-block; max-width: 11em; padding: 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-task:hover { text-decoration: underline; }
.activity-sep { flex: none; margin: 0 5px; color: var(--text-faint); font-weight: 450; }
```

在 `@media (max-width: 768px)` 大块的末尾（`.sheet-footer button { min-height: 48px; }` 之后）追加：
```css
  .task-detail-zone { display: block; }
```

- [ ] **Step 12: 门禁通过**

Run: `cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend && npx vue-tsc -b && npm test`
Expected: PASS —— vue-tsc 零错误（重点确认：无 `activeNode` 残留引用、`selectNode`/`auditTitle` 无残留、`drawerRef.revealActivity` 类型可解析）、30 tests 全绿。

再跑残留检查：
```bash
grep -n "activeNode\|activeNodeId\|selectNode\|auditTitle" frontend/src/views/Tasks.vue
```
Expected: 无输出（零残留）。

- [ ] **Step 13: Commit**

```bash
git add frontend/src/views/Tasks.vue
git commit -m "feat(tasks): root-scoped detail panel with subtask drawer and attributed activity"
```

---

### Task 4: 需求与开发文档更新

**Files:**
- Modify: `docs/requirement/09-task-management.md`
- Modify: `docs/development/09-task-management.md`

**Interfaces:**
- Consumes: Task 1-3 已落地的交互行为。
- Produces: 无。

- [ ] **Step 1: 更新 `docs/requirement/09-task-management.md`**

1. §4.3.1「任务详情」第 4 条：
   原：`4. 当前选中节点的详情和进展时间线。`
   改为：`4. 全树聚合的进展与记录时间线：根任务与所有子任务的进展、审计条目按时间倒序展示，每条标注所属任务名；点击任务名可聚焦该任务（子任务打开详情抽屉，根任务回到总览）。`

2. §4.3.3「添加进展」中：
   原：`- 进展按时间倒序展示，卡片标识所属节点。`
   改为：`- 进展按时间倒序展示，每条标注所属任务名。`

3. 在 §4.3.3 之后新增小节：
```markdown
#### 4.3.4 子任务详情抽屉

- 详情面板固定展示根任务总览；点击任务树中的子任务，从右侧滑出抽屉展示该子任务的标题、状态、描述、日期、优先级与其专属进展记录。
- 桌面端（≥1150px）抽屉压缩详情面板让位；较窄屏幕与手机端以浮层从右侧滑入，点击遮罩关闭。
- 抽屉内提供记录进展、添加子任务、更改状态、编辑、移动操作，复用统一的表单弹层。
- 抽屉打开时点击其他子任务，抽屉内容直接切换；点击根任务行、关闭按钮或 Esc 收起抽屉。
```

4. §10 验收标准末尾追加两条：
```markdown
- 查看根任务详情时，进展与记录时间线能看到所有子任务的条目，且每条标注所属任务名；点击任务名能聚焦对应任务。
- 点击子任务从右侧打开详情抽屉（桌面端主面板被压缩），抽屉内可完成记录进展、更改状态等操作，操作后数据即时刷新。
```

- [ ] **Step 2: 更新 `docs/development/09-task-management.md`**

1. §3.2 前端目录树中：
```text
├── components/tasks/
│   ├── TaskTree.vue
│   └── TaskCalendar.vue
```
改为：
```text
├── components/tasks/
│   ├── TaskTree.vue
│   ├── TaskCalendar.vue
│   └── SubtaskDrawer.vue
```
以及：
```text
└── utils/
    ├── taskDates.ts
    ├── taskHierarchy.ts
    └── taskPayloads.ts
```
改为：
```text
└── utils/
    ├── taskDates.ts
    ├── taskHierarchy.ts
    ├── taskPayloads.ts
    └── taskActivity.ts
```

2. §11「前端状态与数据流」下新增小节（置于 §11.2 之后）：
```markdown
### 11.3 详情交互与子任务抽屉

- 详情面板恒渲染根任务（总览、拆解树、聚合活动流）；`Tasks.vue` 以 `drawerNodeId` ref 驱动子任务抽屉，不再有整面板切换。
- `utils/taskActivity.ts` 的 `buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 负责全树聚合（省略 scope）与单任务过滤（传入 scope，供抽屉），条目携带 `taskTitle` 归因；`taskStatusLabel`/`taskImportanceLabel`/`formatTimestamp` 一并从视图层下沉到 utils。
- `SubtaskDrawer.vue` 常驻挂载：桌面端（≥1150px）以 flex 定宽 slot 实现 push 压缩；<1150px 转为 fixed 浮层 + 遮罩；≤768px 宽度 min(88vw, 400px)。z-index 2300/2301，低于 MotionModal 的 2400。
- 抽屉操作全部 emit 回 `Tasks.vue` 复用 sheet 表单体系；写入成功后子任务目标自动打开其抽屉，根任务目标回到总览。
```

- [ ] **Step 3: Commit**

```bash
git add docs/requirement/09-task-management.md docs/development/09-task-management.md
git commit -m "docs(tasks): describe activity attribution and subtask drawer"
```

---

### Task 5: 整体验证与构建产物

**Files:**
- 无源码修改（验证任务；若发现缺陷，修复后重新过门禁再 commit）。

**Interfaces:**
- Consumes: Task 1-4 全部产出。
- Produces: `frontend/dist_new/` 与 `backend/target/release/obsidian-brain` 构建产物，供与 decoupling 分支合并部署（同一次 sudo 安装）。

- [ ] **Step 1: 全量门禁**

```bash
cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend
npx vue-tsc -b && npm test
```
Expected: PASS（30 tests）。

- [ ] **Step 2: 残留与一致性检查**

```bash
cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain
grep -rn "activeNode\|selectNode" frontend/src/views/Tasks.vue          # 期望：无输出
grep -c "activity-task\|drawerNodeId\|task-detail-zone" frontend/src/views/Tasks.vue  # 期望：≥3 处命中
git log --oneline refactor/tasks-decouple-obsidian..HEAD               # 期望：Task 1-4 的 commit 序列
```

- [ ] **Step 3: 前端构建**

```bash
cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/frontend
npx vite build --outDir dist_new
```
Expected: 构建成功零错误。

- [ ] **Step 4: Release 二进制构建（内嵌新前端）**

```bash
cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain/backend
cargo build --release
```
Expected: 编译成功。产物 `backend/target/release/obsidian-brain` 待用户与 decoupling 改动**合并一次** sudo 安装（`sudo cp backend/target/release/obsidian-brain /usr/local/bin/obsidian-brain` 并重启），不在本任务内执行安装。

- [ ] **Step 5: 汇报**

在任务报告中记录：门禁结果、测试数、构建产物路径、sudo 安装提示语。无新增 commit（除非修复缺陷）。
