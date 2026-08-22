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
