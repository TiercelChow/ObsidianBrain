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
