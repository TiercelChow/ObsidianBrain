import api from './index'
export interface HomeSection<T> { data: T | null; error: string | null }
export interface HomeTask {
  id: string; title: string; kind: 'short' | 'long'; status: string; importance: string
  start_date: string; end_date: string; progress_percent: number
  child_risk_id: string | null; child_risk_title: string | null
}
export interface HomeBook {
  id: string; name: string; kind: 'folder' | 'pdf'; last_file: string | null
  position: number; page_count: number | null; read_at: number
}
export interface HomeMemo { id: string; date: string; timestamp: string; excerpt: string; thumbnail: string | null; tags: string[] }
export interface HomeWikiItem {
  id: string; base_id: string; book_name: string; kind: 'compile' | 'research' | 'review'
  title: string; status: string; detail: string; artifact_state: string | null; updated_at: string
}
export interface HomeOverview {
  today: string; week_start: string; generated_at: string
  tasks: HomeSection<{ active_count: number; overdue_count: number; today_count: number; items: HomeTask[] }>
  reading: HomeSection<HomeBook[]>
  memos: HomeSection<{ total_count: number; week_count: number; items: HomeMemo[] }>
  wiki: HomeSection<{ running_count: number; review_count: number; failed_count: number; items: HomeWikiItem[] }>
  storage: HomeSection<{ originals_bytes: number; cache_bytes: number; cache_limit_bytes: number; pending_cleanup: number }>
  system: { version: string; uptime_seconds: number; components: Record<string, string> }
}
export async function getHomeOverview(today: string, signal?: AbortSignal): Promise<HomeOverview> {
  return await api.get('/home/overview', { params: { today }, signal, timeout: 15000 }) as unknown as HomeOverview
}
