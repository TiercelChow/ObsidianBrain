import { callTool } from './index'

export type TaskKind = 'short' | 'long'
export type TaskRole = 'root' | 'subtask'
export type TaskImportance = 'low' | 'normal' | 'high' | 'urgent'
export type TaskStatus = 'open' | 'planned' | 'in_progress' | 'blocked' | 'completed' | 'cancelled'

export interface DocumentVersion {
  revision: number
  content_hash: string
}

export interface TaskNode {
  id: string
  root_id: string
  parent_id: string | null
  kind: TaskKind
  role: TaskRole
  title: string
  description: string
  start_date: string
  end_date: string
  importance: TaskImportance
  status: TaskStatus
  position: number
  closure_note: string | null
  closed_at: string | null
  created_at: string
  updated_at: string
  revision: number
  archived_at: string | null
}

export interface ProgressEntry {
  id: string
  root_id: string
  task_id: string
  recorded_at: string
  note: string
  percent_after: number | null
  created_at: string
}

export interface AuditEvent {
  id: string
  root_id: string
  task_id: string
  event_type: string
  from_status: TaskStatus | null
  to_status: TaskStatus | null
  note: string | null
  occurred_at: string
}

export interface TaskDerivedState {
  overdue: boolean
  active_today: boolean
  due_today: boolean
}

export interface TaskSummary extends TaskNode {
  storage_path: string
  document_version: DocumentVersion
  progress_percent: number
  completed_leaf_count: number
  effective_leaf_count: number
  derived: TaskDerivedState
}

export interface TaskDetail {
  root: TaskNode
  tasks: TaskNode[]
  progress: ProgressEntry[]
  audit: AuditEvent[]
  storage_path: string
  document_version: DocumentVersion
  progress_percent: number
  completed_leaf_count: number
  effective_leaf_count: number
  freeform_notes: string
}

export interface TaskListResponse {
  tasks: TaskSummary[]
  next_cursor: string | null
}

export interface TaskWriteResponse {
  task: TaskDetail
  warnings: string[]
}

export interface TaskSyncResult {
  created: number
  updated: number
  unchanged: number
  removed: number
  errors: Array<{ path: string; code: string; message: string }>
}

export interface TaskFields {
  title: string
  description?: string
  start_date: string
  end_date: string
  importance: TaskImportance
}

export interface TaskFilters {
  kinds?: TaskKind[]
  statuses?: TaskStatus[]
  importance?: TaskImportance[]
  start_date?: string
  end_date?: string
  query?: string
  include_archived?: boolean
  include_subtasks?: boolean
  sort?: 'priority' | 'start_date' | 'updated_at' | 'created_at' | 'importance'
  cursor?: string
  limit?: number
}

interface ToolEnvelope<T> {
  status: 'success' | 'error'
  result: T | null
  error: { code: string; message: string; suggestion?: string } | null
}

export class TaskApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly suggestion?: string,
  ) {
    super(message)
    this.name = 'TaskApiError'
  }
}

async function taskCall<T>(tool: string, args: Record<string, unknown>): Promise<T> {
  const envelope = await callTool(tool, args) as unknown as ToolEnvelope<T>
  if (envelope.status !== 'success' || envelope.result == null) {
    throw new TaskApiError(
      envelope.error?.code || 'TASK_REQUEST_FAILED',
      envelope.error?.message || '任务请求失败',
      envelope.error?.suggestion,
    )
  }
  return envelope.result
}

export function createTask(kind: TaskKind, fields: TaskFields) {
  return taskCall<TaskWriteResponse>('create_task', { kind, ...fields })
}

export function listTasks(filters: TaskFilters = {}) {
  return taskCall<TaskListResponse>('list_tasks', filters as Record<string, unknown>)
}

export function getTask(taskId: string) {
  return taskCall<TaskDetail>('get_task', { task_id: taskId })
}

export function updateTask(
  taskId: string,
  patch: Partial<TaskFields>,
  expectedVersion: DocumentVersion,
) {
  return taskCall<TaskWriteResponse>('update_task', {
    task_id: taskId,
    patch,
    expected_version: expectedVersion,
  })
}

export function setTaskStatus(
  taskId: string,
  status: TaskStatus,
  expectedVersion: DocumentVersion,
  closureNote?: string,
  cascade = false,
) {
  return taskCall<TaskWriteResponse>('set_task_status', {
    task_id: taskId,
    status,
    expected_version: expectedVersion,
    cascade,
    ...(closureNote !== undefined ? { closure_note: closureNote } : {}),
  })
}

export function addSubtask(parentId: string, fields: TaskFields, expectedVersion: DocumentVersion) {
  return taskCall<TaskWriteResponse>('add_subtask', {
    parent_id: parentId,
    ...fields,
    expected_version: expectedVersion,
  })
}

export function moveSubtask(
  taskId: string,
  newParentId: string,
  position: number,
  expectedVersion: DocumentVersion,
) {
  return taskCall<TaskWriteResponse>('move_subtask', {
    task_id: taskId,
    new_parent_id: newParentId,
    position,
    expected_version: expectedVersion,
  })
}

export function addTaskProgress(
  taskId: string,
  note: string,
  expectedVersion: DocumentVersion,
  percentAfter?: number,
) {
  return taskCall<TaskWriteResponse>('add_task_progress', {
    task_id: taskId,
    note,
    expected_version: expectedVersion,
    ...(percentAfter !== undefined ? { percent_after: percentAfter } : {}),
  })
}

export function getTaskCalendar(startDate: string, endDate: string, filters: TaskFilters = {}) {
  return taskCall<TaskSummary[]>('get_task_calendar', {
    start_date: startDate,
    end_date: endDate,
    include_subtasks: filters.include_subtasks || false,
    include_archived: filters.include_archived || false,
    ...(filters.kinds?.length ? { kinds: filters.kinds } : {}),
    ...(filters.statuses?.length ? { statuses: filters.statuses } : {}),
    ...(filters.importance?.length ? { importance: filters.importance } : {}),
  })
}

export function archiveTask(taskId: string, archived: boolean, expectedVersion: DocumentVersion) {
  return taskCall<TaskWriteResponse>('archive_task', {
    task_id: taskId,
    archived,
    expected_version: expectedVersion,
  })
}

export function syncTasks(dryRun = false) {
  return taskCall<TaskSyncResult>('sync_tasks', { dry_run: dryRun })
}
