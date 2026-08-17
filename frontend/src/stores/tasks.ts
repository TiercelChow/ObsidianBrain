import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  TaskApiError,
  addSubtask as addSubtaskApi,
  addTaskProgress as addTaskProgressApi,
  archiveTask as archiveTaskApi,
  createTask as createTaskApi,
  getTask as getTaskApi,
  getTaskCalendar as getTaskCalendarApi,
  listTasks as listTasksApi,
  moveSubtask as moveSubtaskApi,
  setTaskStatus as setTaskStatusApi,
  syncTasks as syncTasksApi,
  updateTask as updateTaskApi,
  type DocumentVersion,
  type TaskDetail,
  type TaskFields,
  type TaskFilters,
  type TaskKind,
  type TaskStatus,
  type TaskSummary,
  type TaskSyncResult,
} from '@/api/tasks'

export const useTasksStore = defineStore('tasks', () => {
  const tasks = ref<TaskSummary[]>([])
  const calendarTasks = ref<TaskSummary[]>([])
  const details = ref<Record<string, TaskDetail>>({})
  const selectedTaskId = ref<string | null>(null)
  const loading = ref(false)
  const detailLoading = ref(false)
  const calendarLoading = ref(false)
  const saving = ref(false)
  const syncing = ref(false)
  const error = ref<string | null>(null)
  const lastSync = ref<TaskSyncResult | null>(null)
  let listSequence = 0
  let detailSequence = 0
  let calendarSequence = 0

  const selectedDetail = computed(() => (
    selectedTaskId.value ? details.value[selectedTaskId.value] || null : null
  ))

  function describeError(cause: unknown): string {
    if (cause instanceof TaskApiError) {
      return cause.suggestion ? `${cause.message} · ${cause.suggestion}` : cause.message
    }
    return cause instanceof Error ? cause.message : '任务操作失败，请稍后重试'
  }

  function versionOf(detail = selectedDetail.value): DocumentVersion {
    if (!detail) throw new Error('任务详情尚未加载')
    return detail.document_version
  }

  function applyDetail(detail: TaskDetail) {
    details.value = { ...details.value, [detail.root.id]: detail }
    selectedTaskId.value = detail.root.id
  }

  async function loadTasks(filters: TaskFilters = {}) {
    const sequence = ++listSequence
    loading.value = true
    error.value = null
    try {
      const response = await listTasksApi({ ...filters, limit: filters.limit || 200 })
      if (sequence === listSequence) tasks.value = response.tasks
      return response.tasks
    } catch (cause) {
      if (sequence === listSequence) error.value = describeError(cause)
      throw cause
    } finally {
      if (sequence === listSequence) loading.value = false
    }
  }

  async function loadDetail(taskId: string) {
    const sequence = ++detailSequence
    selectedTaskId.value = taskId
    detailLoading.value = true
    error.value = null
    try {
      const detail = await getTaskApi(taskId)
      if (sequence === detailSequence) applyDetail(detail)
      return detail
    } catch (cause) {
      if (sequence === detailSequence) error.value = describeError(cause)
      throw cause
    } finally {
      if (sequence === detailSequence) detailLoading.value = false
    }
  }

  async function loadCalendar(startDate: string, endDate: string, filters: TaskFilters = {}) {
    const sequence = ++calendarSequence
    calendarLoading.value = true
    error.value = null
    try {
      const result = await getTaskCalendarApi(startDate, endDate, filters)
      if (sequence === calendarSequence) calendarTasks.value = result
      return result
    } catch (cause) {
      if (sequence === calendarSequence) error.value = describeError(cause)
      throw cause
    } finally {
      if (sequence === calendarSequence) calendarLoading.value = false
    }
  }

  async function runWrite(operation: () => Promise<{ task: TaskDetail }>) {
    saving.value = true
    error.value = null
    try {
      const response = await operation()
      applyDetail(response.task)
      return response.task
    } catch (cause) {
      error.value = describeError(cause)
      if (cause instanceof TaskApiError && cause.code === 'TASK_VERSION_CONFLICT' && selectedTaskId.value) {
        await loadDetail(selectedTaskId.value).catch(() => undefined)
      }
      throw cause
    } finally {
      saving.value = false
    }
  }

  async function create(kind: TaskKind, fields: TaskFields) {
    return runWrite(() => createTaskApi(kind, fields))
  }

  async function update(taskId: string, patch: Partial<TaskFields>, version = versionOf()) {
    return runWrite(() => updateTaskApi(taskId, patch, version))
  }

  async function setStatus(
    taskId: string,
    status: TaskStatus,
    closureNote?: string,
    cascade = false,
    version = versionOf(),
  ) {
    return runWrite(() => setTaskStatusApi(taskId, status, version, closureNote, cascade))
  }

  async function addSubtask(parentId: string, fields: TaskFields, version = versionOf()) {
    return runWrite(() => addSubtaskApi(parentId, fields, version))
  }

  async function moveSubtask(taskId: string, parentId: string, position = 0, version = versionOf()) {
    return runWrite(() => moveSubtaskApi(taskId, parentId, position, version))
  }

  async function addProgress(taskId: string, note: string, percent?: number, version = versionOf()) {
    return runWrite(() => addTaskProgressApi(taskId, note, version, percent))
  }

  async function archive(taskId: string, archived: boolean, version = versionOf()) {
    return runWrite(() => archiveTaskApi(taskId, archived, version))
  }

  async function sync(dryRun = false) {
    syncing.value = true
    error.value = null
    try {
      const result = await syncTasksApi(dryRun)
      lastSync.value = result
      return result
    } catch (cause) {
      error.value = describeError(cause)
      throw cause
    } finally {
      syncing.value = false
    }
  }

  function clearSelection() {
    selectedTaskId.value = null
  }

  function clearError() {
    error.value = null
  }

  return {
    tasks,
    calendarTasks,
    details,
    selectedTaskId,
    selectedDetail,
    loading,
    detailLoading,
    calendarLoading,
    saving,
    syncing,
    error,
    lastSync,
    loadTasks,
    loadDetail,
    loadCalendar,
    create,
    update,
    setStatus,
    addSubtask,
    moveSubtask,
    addProgress,
    archive,
    sync,
    clearSelection,
    clearError,
  }
})
