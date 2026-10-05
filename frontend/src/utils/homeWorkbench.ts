import type { HomeBook, HomeOverview, HomeWikiItem, HomeSection } from '../api/home'

export function readingPosition(book: HomeBook): string {
  if (book.kind === 'pdf' || /\.pdf$/i.test(book.last_file || '')) {
    const page = Math.max(1, Math.floor(book.position || 1))
    return book.page_count && book.page_count > 0 ? `第 ${Math.min(page, book.page_count)} / ${book.page_count} 页` : `第 ${page} 页`
  }
  return book.last_file?.split(/[\\/]/).pop()?.replace(/\.md$/i, '') || '继续上次阅读'
}
export function wikiItemRoute(item: HomeWikiItem) {
  if (item.kind === 'review') return { path: '/knowledge/wiki', query: { base: item.base_id, review: item.id } }
  if (item.kind === 'compile') return { path: '/knowledge', query: { base: item.base_id, report: '1' } }
  return { path: '/knowledge/tasks', query: { base: item.base_id, task: item.id } }
}
export function wikiStatus(item: HomeWikiItem): string {
  if (item.kind === 'review') return item.status === 'conflicted' ? '审核有冲突' : '等待审核'
  if (item.kind === 'compile') return item.status === 'failed' ? '编译失败' : '编译中'
  if (item.status === 'completed') {
    if (item.artifact_state === 'failed') return '报告已完成 · PPT 失败'
    if (item.artifact_state === 'pending') return '报告已完成 · PPT 准备中'
    return item.artifact_state === 'ready' ? '报告与 PPT 已就绪' : '报告已完成'
  }
  return ({ queued: '排队中', running: '研究中', failed: '研究失败', cancelled: '已取消' } as Record<string,string>)[item.status] || '待处理'
}
/** Keep the last successful section visible, but never hide its fresh error. */
export function mergeHomeOverview(old: HomeOverview | null, fresh: HomeOverview): HomeOverview {
  const merged = { ...fresh }
  for (const key of ['tasks', 'reading', 'memos', 'wiki', 'storage'] as const) {
    if (!fresh[key].data && old?.[key]?.data) {
      (merged as unknown as Record<string, HomeSection<unknown>>)[key] = { data: old[key].data, error: fresh[key].error || '暂时无法更新' }
    }
  }
  return merged
}
export function shouldPollHome(data: HomeOverview | null, visible: boolean): boolean {
  return visible && !!data?.wiki.data && !data.wiki.error && data.wiki.data.running_count > 0
}
export function formatHomeBytes(value: number): string {
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GB`
  return `${(value / 1024 ** 2).toFixed(1)} MB`
}
export function formatUptime(seconds: number): string {
  if (seconds < 60) return '不足 1 分钟'
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时 ${Math.floor(seconds % 3600 / 60)} 分钟`
  return `${Math.floor(seconds / 86400)} 天 ${Math.floor(seconds % 86400 / 3600)} 小时`
}
export function recentReadLabel(value: number, now = new Date()): string {
  const read = new Date(value)
  const day = (date:Date) => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86400000
  const days = day(now) - day(read)
  return days <= 0 ? '今天' : days === 1 ? '昨天' : days < 7 ? `${days} 天前` : read.toLocaleDateString('zh-CN',{month:'numeric',day:'numeric'})
}
// Small, session-only cache: no bodies, image bytes, credentials or persistent writes.
let cached: HomeOverview | null = null
let scroll = 0
export function homeSnapshot() { return { data: cached, scroll } }
export function saveHomeSnapshot(data: HomeOverview | null, position?: number) {
  cached = data
  if (position != null) scroll = position
}
