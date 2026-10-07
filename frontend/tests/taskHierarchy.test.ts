import assert from 'node:assert/strict'
import test from 'node:test'

import {
  calendarAgendaEntries,
  calendarTopLevelTasks,
  flattenVisibleSubtasks,
} from '../src/utils/taskHierarchy.ts'

const tasks = [
  { id: 'root-a', root_id: 'root-a', parent_id: null, role: 'root' as const, position: 0, created_at: '2026-08-01', start_date: '2026-08-01', end_date: '2026-08-31' },
  { id: 'child-a', root_id: 'root-a', parent_id: 'root-a', role: 'subtask' as const, position: 0, created_at: '2026-08-02', start_date: '2026-08-20', end_date: '2026-08-22' },
  { id: 'grandchild-a', root_id: 'root-a', parent_id: 'child-a', role: 'subtask' as const, position: 0, created_at: '2026-08-03', start_date: '2026-08-20', end_date: '2026-08-20' },
  { id: 'child-later', root_id: 'root-a', parent_id: 'root-a', role: 'subtask' as const, position: 1, created_at: '2026-08-04', start_date: '2026-08-25', end_date: '2026-08-26' },
  { id: 'root-b', root_id: 'root-b', parent_id: null, role: 'root' as const, position: 0, created_at: '2026-08-05', start_date: '2026-09-01', end_date: '2026-09-30' },
]

test('task breakdown omits the root and promotes its direct children to depth zero', () => {
  const rows = flattenVisibleSubtasks(tasks, new Set(['child-a']))

  assert.deepEqual(rows.map(row => [row.node.id, row.depth]), [
    ['child-a', 0],
    ['grandchild-a', 1],
    ['child-later', 0],
  ])
})

test('collapsed descendants do not reappear as orphaned top-level subtasks', () => {
  const rows = flattenVisibleSubtasks(tasks, new Set())
  assert.deepEqual(rows.map(row => row.node.id), ['child-a', 'child-later'])
})

test('genuinely orphaned subtasks remain visible without looping over damaged cycles', () => {
  const orphan = {...tasks[1],id:'orphan',parent_id:'missing'}
  const cycle = [{...orphan,id:'cycle-a',parent_id:'cycle-b'},{...orphan,id:'cycle-b',parent_id:'cycle-a'}]
  const rows = flattenVisibleSubtasks([...tasks,orphan,...cycle], new Set(['cycle-a','cycle-b']))
  assert.deepEqual(rows.map(row => row.node.id), ['child-a','child-later','orphan','cycle-a','cycle-b'])
})

test('calendar grid and collapsed agenda only receive root tasks', () => {
  assert.deepEqual(calendarTopLevelTasks(tasks).map(task => task.id), ['root-a', 'root-b'])

  const entries = calendarAgendaEntries(tasks, '2026-08-20')
  assert.deepEqual(entries.map(entry => [entry.task.id, entry.depth]), [
    ['root-a', 0],
  ])
  assert.equal(entries[0]?.hasChildren, true)
})

test('calendar agenda reveals dated descendants only after their root is expanded', () => {
  const entries = calendarAgendaEntries(tasks, '2026-08-20', new Set(['root-a']))

  assert.deepEqual(entries.map(entry => [entry.task.id, entry.depth]), [
    ['root-a', 0],
    ['child-a', 1],
    ['grandchild-a', 2],
  ])
})
