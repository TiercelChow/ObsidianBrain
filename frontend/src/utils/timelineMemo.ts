export interface TimelineMemo {
  id: string
  timestamp: string
  date?: string
  content: string
  images: string[]
  tags: string[]
  revision: number
}

export interface TimelineStorage {
  directory: string
  originals_bytes: number
  cache_bytes: number
  cache_limit_bytes: number
  missing_images: string[]
  pending_cleanup: number
  legacy_directory: string
}

export function memoImageUrl(path: string, thumbnail = false): string {
  return `/v1/timeline/${thumbnail ? 'thumbnails' : 'images'}/${path.split('/').map(encodeURIComponent).join('/')}`
}

/** Resolve legacy Markdown embeds without restoring any external vault access. */
export function memoImageSource(escapedPath: string): string {
  const path = escapedPath.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').trim()
  if (/^https?:\/\//i.test(path) || /^\/v1\/(?:timeline|vault)\/(?:images|thumbnails)\//.test(path)) return path
  if (!path || path.includes(':') || path.startsWith('/') || path.includes('\\') || path.split('/').includes('..')) return ''
  return memoImageUrl(path.split('|')[0].trim())
}

export function memoLocalDate(memo: { date?: string; timestamp: string }): string {
  if (memo.date) return memo.date
  const date = new Date(memo.timestamp)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

export function memoToolResult<T>(response: unknown): T {
  const envelope = response as { status?: string; result?: T; error?: { message?: string } }
  if (envelope.status === 'error') throw new Error(envelope.error?.message || '操作失败')
  if (envelope.result == null) throw new Error('服务返回了无效结果')
  return envelope.result
}
