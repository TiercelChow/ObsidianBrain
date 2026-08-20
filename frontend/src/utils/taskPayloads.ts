import type { TaskFields, TaskKind } from '../api/tasks'

type TaskFormFields = TaskFields & Partial<{ kind: TaskKind }>

/** Keep form-only state out of strict tool payloads. */
export function taskFieldsPayload(fields: TaskFormFields): TaskFields {
  return {
    title: fields.title,
    description: fields.description,
    start_date: fields.start_date,
    end_date: fields.end_date,
    importance: fields.importance,
  }
}
