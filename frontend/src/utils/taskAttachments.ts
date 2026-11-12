export function attachmentValidation(file: {
  size: number
  name: string
}): string | null {
  if (!file.size) return '不能上传空文件'
  if (file.size > 50 * 1024 * 1024) return '单个附件最大 50 MB'
  if (!file.name || /[/\\\x00-\x1f]/.test(file.name)) return '文件名无效'
  return null
}
export function attachmentSize(size: number): string {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${Number((size / 1024).toFixed(1))} KB`
  return `${Number((size / 1024 / 1024).toFixed(1))} MB`
}
