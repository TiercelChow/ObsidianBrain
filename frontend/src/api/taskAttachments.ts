import api from './index'
export interface TaskAttachment {
  id: string
  task_id: string
  progress_id: string | null
  filename: string
  size_bytes: number
  created_at: string
}
function endpoint(task: string, progress?: string) {
  return `/tasks/${encodeURIComponent(task)}/attachments${progress ? `?progress_id=${encodeURIComponent(progress)}` : ''}`
}
export async function listTaskAttachments(
  task: string,
  progress?: string,
): Promise<TaskAttachment[]> {
  return (await api.get(
    endpoint(task, progress),
  )) as unknown as TaskAttachment[]
}
export async function uploadTaskAttachment(
  task: string,
  progress: string | undefined,
  file: File,
  signal: AbortSignal,
  onProgress: (percent: number) => void,
  uploadKey?: string,
): Promise<TaskAttachment> {
  const form = new FormData()
  form.append('file', file)
  const url =
    endpoint(task, progress) +
    (uploadKey
      ? `${progress ? '&' : '?'}upload_key=${encodeURIComponent(uploadKey)}`
      : '')
  return (await api.post(url, form, {
    headers: { 'Content-Type': 'multipart/form-data' },
    signal,
    timeout: 0,
    onUploadProgress: (event) =>
      onProgress(
        event.total ? Math.round((event.loaded / event.total) * 100) : 0,
      ),
  })) as unknown as TaskAttachment
}
export async function deleteTaskAttachment(id: string) {
  await api.delete(`/tasks/attachments/${encodeURIComponent(id)}`)
}
export function taskAttachmentUrl(id: string) {
  return `/v1/tasks/attachments/${encodeURIComponent(id)}`
}
