import { attachmentValidation } from './taskAttachments.ts'

export interface DraftFile<T> {
  id: string
  file: T
  status: 'pending' | 'uploading' | 'uploaded' | 'failed'
  percent: number
  error: string
}
export interface ProgressDraft<T> {
  progressId: string | null
  files: DraftFile<T>[]
}
let fileSequence = 0
export function createProgressDraft<T>(): ProgressDraft<T> {
  return { progressId: null, files: [] }
}
export function appendProgressFiles<T extends { name: string; size: number }>(
  draft: ProgressDraft<T>,
  files: T[],
): void {
  if (draft.files.length + files.length > 20)
    throw new Error('每条进展最多 20 个附件')
  for (const file of files) {
    const error = attachmentValidation(file)
    if (error) throw new Error(`${file.name}：${error}`)
  }
  draft.files.push(
    ...files.map((file) => ({
      id: `progress-file-${Date.now()}-${++fileSequence}`,
      file,
      status: 'pending' as const,
      percent: 0,
      error: '',
    })),
  )
}
export async function submitProgressDraft<T>(
  draft: ProgressDraft<T>,
  deps: {
    create: () => Promise<string>
    upload: (
      file: T,
      progressId: string,
      onProgress: (percent: number) => void,
      key: string,
    ) => Promise<unknown>
  },
): Promise<boolean> {
  if (!draft.progressId) draft.progressId = await deps.create()
  for (const item of draft.files) {
    if (item.status === 'uploaded') continue
    item.status = 'uploading'
    item.percent = 0
    item.error = ''
    try {
      await deps.upload(
        item.file,
        draft.progressId,
        (percent) => {
          item.percent = percent
        },
        item.id,
      )
      item.status = 'uploaded'
      item.percent = 100
    } catch (error) {
      item.status = 'failed'
      item.error = error instanceof Error ? error.message : '上传失败，请重试'
    }
  }
  return draft.files.every((item) => item.status === 'uploaded')
}
