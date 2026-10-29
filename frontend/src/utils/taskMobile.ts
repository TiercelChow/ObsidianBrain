import type { TaskNode } from '../api/tasks'
import { taskStatusLabel, type TaskActivityEntry } from './taskActivity.ts'
import { isTaskClosed, taskTiming, type WorkspaceTask } from './taskWorkspace.ts'

export function mobileTaskMeta(task: WorkspaceTask & Pick<TaskNode, 'kind'>, today: string): string {
  return `${task.kind === 'long' ? '长期' : '待办'} · ${isTaskClosed(task) ? taskStatusLabel(task.status) : taskTiming(task, today).label}`
}

/** One level at a time on a phone; ordering matches the document's sibling order. */
export function directTaskChildren<T extends Pick<TaskNode, 'id' | 'parent_id' | 'position' | 'created_at'>>(nodes: readonly T[], parentId: string): T[] {
  return nodes.filter(node => node.parent_id === parentId)
    .sort((a, b) => a.position - b.position || a.created_at.localeCompare(b.created_at) || a.id.localeCompare(b.id))
}

/** Keep every record and its attribution; audit traffic must not bury personal progress. */
export function partitionTaskActivity(entries: readonly TaskActivityEntry[]) {
  return { progress: entries.filter(item => item.type === 'progress'), audit: entries.filter(item => item.type === 'audit') }
}
