import type { TaskNode } from '../api/tasks'
import { addLocalDays, parseLocalDate } from './taskDates.ts'

export type WorkspaceTask = Pick<TaskNode, 'id' | 'status' | 'importance' | 'start_date' | 'end_date' | 'updated_at'>
export type TaskFocus = 'all' | 'overdue' | 'today' | 'blocked'

export function isTaskClosed(task: Pick<TaskNode, 'status'>): boolean {
  return task.status === 'completed' || task.status === 'cancelled'
}

export function matchesTaskFocus(task: WorkspaceTask, focus: TaskFocus, today: string): boolean {
  if (focus === 'all') return true
  if (isTaskClosed(task)) return false
  if (focus === 'overdue') return task.end_date < today
  if (focus === 'blocked') return task.status === 'blocked'
  return task.end_date === today || task.start_date === today
}

/** A spanning project is ongoing, not a daily deadline. Each root appears once. */
export function groupWorkspaceTasks<T extends WorkspaceTask>(tasks: readonly T[], today: string) {
  const soon = addLocalDays(today, 7)
  const groups = [
    { key: 'overdue', label: '已逾期', tasks: [] as T[] },
    { key: 'blocked', label: '需要解阻', tasks: [] as T[] },
    { key: 'today', label: '今日安排', tasks: [] as T[] },
    { key: 'soon', label: '七天内截止', tasks: [] as T[] },
    { key: 'ongoing', label: '持续推进', tasks: [] as T[] },
    { key: 'later', label: '之后开始', tasks: [] as T[] },
    { key: 'closed', label: '已关闭', tasks: [] as T[] },
  ]
  const importance = { urgent: 0, high: 1, normal: 2, low: 3 }
  for (const task of tasks) {
    const index = isTaskClosed(task) ? 6
      : task.end_date < today ? 0
      : task.status === 'blocked' ? 1
      : matchesTaskFocus(task, 'today', today) ? 2
      : task.end_date <= soon ? 3
      : task.start_date <= today ? 4 : 5
    groups[index].tasks.push(task)
  }
  return groups.filter(group => group.tasks.length).map(group => ({
    ...group,
    tasks: group.tasks.sort((a, b) => group.key === 'closed'
      ? b.updated_at.localeCompare(a.updated_at)
      : importance[a.importance] - importance[b.importance]
        || a.end_date.localeCompare(b.end_date)
        || b.updated_at.localeCompare(a.updated_at)),
  }))
}

/** UTC is used only for calendar-day arithmetic, never for displaying a local date. */
function calendarDayNumber(value: string) {
  const { year, month, day } = parseLocalDate(value)
  return Date.UTC(year, month - 1, day) / 86_400_000
}

export function taskTiming(task: WorkspaceTask, today: string): { label: string; tone: 'danger' | 'accent' | 'quiet' } {
  if (isTaskClosed(task)) return { label: `${task.end_date} 截止`, tone: 'quiet' }
  const days = calendarDayNumber(task.end_date) - calendarDayNumber(today)
  if (days < 0) return { label: `逾期 ${-days} 天`, tone: 'danger' }
  if (days === 0) return { label: '今天截止', tone: 'accent' }
  if (task.start_date === today) return { label: '今天开始', tone: 'accent' }
  if (days <= 7) return { label: `${days} 天后截止`, tone: 'accent' }
  return { label: `${task.end_date} 截止`, tone: 'quiet' }
}
