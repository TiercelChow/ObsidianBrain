import assert from 'node:assert/strict'
import test from 'node:test'
import { groupWorkspaceTasks, matchesTaskFocus, taskTiming, type WorkspaceTask } from '../src/utils/taskWorkspace.ts'

const today = '2026-09-06'
function task(id: string, patch: Partial<WorkspaceTask> = {}): WorkspaceTask {
  return { id, status: 'in_progress', importance: 'normal', start_date: '2026-09-01', end_date: '2026-09-30', updated_at: '2026-09-01T00:00:00Z', ...patch }
}

test('a month-long active task does not crowd out tasks due today or this week', () => {
  const tasks = [task('month'), task('week', { end_date: '2026-09-10' }), task('today', { end_date: today })]
  assert.deepEqual(groupWorkspaceTasks(tasks, today).map(g => [g.key, g.tasks.map(t => t.id)]), [
    ['today', ['today']], ['soon', ['week']], ['ongoing', ['month']],
  ])
  assert.equal(matchesTaskFocus(tasks[0], 'today', today), false)
})

test('overdue and blocked are actionable groups, but closed tasks never enter them', () => {
  const tasks = [
    task('done', { status: 'completed', end_date: '2026-09-01' }),
    task('late-blocked', { status: 'blocked', end_date: '2026-09-05' }),
    task('blocked', { status: 'blocked' }),
    task('cancelled', { status: 'cancelled', end_date: today }),
  ]
  assert.deepEqual(groupWorkspaceTasks(tasks, today).map(g => [g.key, g.tasks.map(t => t.id)]), [
    ['overdue', ['late-blocked']], ['blocked', ['blocked']], ['closed', ['done', 'cancelled']],
  ])
  assert.equal(matchesTaskFocus(tasks[0], 'overdue', today), false)
  assert.equal(matchesTaskFocus(tasks[1], 'blocked', today), true)
  assert.equal(matchesTaskFocus(tasks[3], 'today', today), false)
})

test('today includes starts and deadlines; soon includes exactly the next seven calendar days', () => {
  const tasks = [task('start', { start_date: today }), task('seventh', { end_date: '2026-09-13' }), task('eighth', { end_date: '2026-09-14' })]
  assert.deepEqual(groupWorkspaceTasks(tasks, today).map(g => [g.key, g.tasks.map(t => t.id)]), [
    ['today', ['start']], ['soon', ['seventh']], ['ongoing', ['eighth']],
  ])
  assert.equal(taskTiming(tasks[0], today).label, '今天开始')
  assert.equal(taskTiming(task('due', { start_date: today, end_date: today }), today).label, '今天截止')
})

test('within a group urgency wins, then the nearest deadline; sorting does not mutate the input', () => {
  const tasks = [task('normal', { end_date: '2026-09-08' }), task('urgent', { importance: 'urgent', end_date: '2026-09-12' }), task('earlier', { end_date: '2026-09-07' })]
  assert.deepEqual(groupWorkspaceTasks(tasks, today)[0].tasks.map(t => t.id), ['urgent', 'earlier', 'normal'])
  assert.deepEqual(tasks.map(t => t.id), ['normal', 'urgent', 'earlier'])
  assert.deepEqual(groupWorkspaceTasks([], today), [])
})

test('deadline labels count calendar days across a year boundary without local timezone shifts', () => {
  assert.equal(taskTiming(task('late', { end_date: '2025-12-31' }), '2026-01-02').label, '逾期 2 天')
  assert.equal(taskTiming(task('next', { end_date: '2026-01-02' }), '2025-12-31').label, '2 天后截止')
  assert.equal(taskTiming(task('closed', { status: 'completed', end_date: '2025-12-31' }), today).tone, 'quiet')
})

test('focus chips can overlap but a blocked task due today is listed only once', () => {
  const blocked = task('blocked-today', { status: 'blocked', end_date: today })
  assert.equal(matchesTaskFocus(blocked, 'today', today), true)
  assert.equal(matchesTaskFocus(blocked, 'blocked', today), true)
  assert.equal(matchesTaskFocus(blocked, 'all', today), true)
  assert.deepEqual(groupWorkspaceTasks([blocked], today).map(g => [g.key, g.tasks.length]), [['blocked', 1]])
})

test('future starts remain separate and closed history is ordered by most recent update', () => {
  const tasks = [
    task('future', { status: 'planned', start_date: '2026-09-20' }),
    task('old', { status: 'completed' }),
    task('recent', { status: 'cancelled', updated_at: '2026-09-06T00:00:00Z' }),
  ]
  assert.deepEqual(groupWorkspaceTasks(tasks, today).map(g => [g.key, g.tasks.map(t => t.id)]), [
    ['later', ['future']], ['closed', ['recent', 'old']],
  ])
})
