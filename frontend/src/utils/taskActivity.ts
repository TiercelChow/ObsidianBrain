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
