import axios, { type AxiosRequestConfig } from 'axios'
import { memoToolResult, type TimelineMemo, type TimelineStorage } from '@/utils/timelineMemo'

const api = axios.create({
  baseURL: '/v1',
  timeout: 120000,
  headers: { 'Content-Type': 'application/json' },
})

api.interceptors.response.use(
  (response) => response.data,
  (error) => {
    if (!axios.isCancel(error)) console.error('API 错误:', error.response?.data || error.message)
    return Promise.reject(error)
  }
)

export default api

// ── Health ──
export function getHealth() {
  return api.get('/health')
}

export async function getMemo(id: string): Promise<TimelineMemo> {
  return await api.get(`/timeline/memos/${encodeURIComponent(id)}`) as unknown as TimelineMemo
}

// ── Tools ──
export function listTools() {
  return api.get('/tools')
}

export function callTool(
  tool: string,
  args: Record<string, unknown> = {},
  config?: AxiosRequestConfig,
) {
  return api.post('/tools/call', { tool, arguments: args }, config)
}

// ── System Config ──
export function saveConfig(config: Record<string, unknown>) {
  return callTool('save_config', config)
}

// ── Code Repo ──
export function addCodeRepo(path: string, name: string) {
  return callTool('add_code_repo', { path, name })
}

export function listCodeRepos() {
  return callTool('list_code_repos')
}

export function getRepoDetail(name: string) {
  return callTool('get_repo_detail', { name })
}

export function linkNoteToRepo(notePath: string, repoName: string) {
  return callTool('link_note_to_repo', { note_path: notePath, repo_name: repoName })
}

export function getLinkedNotes(repoName: string) {
  return callTool('get_linked_notes', { repo_name: repoName })
}

export function openInVscode(name: string) {
  return callTool('open_in_vscode', { name })
}

// ── Timeline ──
export function getTimeline(startDate: string, endDate: string) {
  return callTool('get_timeline', { start_date: startDate, end_date: endDate })
}

// ── Time Machine (时光机) ──
export function createMemo(content: string, images?: string[], tags?: string[]) {
  return callTool('create_memo', {
    content,
    ...(images?.length ? { images } : {}),
    ...(tags?.length ? { tags } : {}),
  })
}

export function browseTimeline(startDate?: string, endDate?: string, limit = 20, offset = 0) {
  return callTool('browse_timeline', {
    ...(startDate ? { start_date: startDate } : {}),
    ...(endDate ? { end_date: endDate } : {}),
    limit,
    offset,
  })
}

export async function updateMemo(memoId: string, revision: number, content: string, images: string[], tags: string[]) {
  return memoToolResult<TimelineMemo>(await callTool('update_memo', { memo_id: memoId, expected_revision: revision, content, images, tags }))
}

export async function deleteMemo(memoId: string, revision: number) {
  return memoToolResult<{ deleted: boolean; pending_cleanup: number }>(await callTool('delete_memo', { memo_id: memoId, expected_revision: revision }))
}

export async function getTimelineStorage() {
  return memoToolResult<TimelineStorage>(await callTool('get_timeline_storage'))
}

export async function importTimelineImages(directory: string) {
  return memoToolResult<{ copied: number; missing: string[] }>(await callTool('import_timeline_images', { directory }, { timeout: 600000 }))
}

export async function discardMemoImages(paths: string[]) {
  return memoToolResult<{ pending_cleanup: number }>(await callTool('discard_memo_images', { paths }))
}

export async function clearTimelineImageCache() {
  return memoToolResult<{ cleared: boolean }>(await callTool('clear_timeline_image_cache'))
}

export function getMemoStats() {
  return callTool('get_memo_stats')
}

export function searchMemos(query: string, startDate?: string, endDate?: string, tags?: string[], limit = 20, offset = 0) {
  return callTool('search_memos', {
    query,
    ...(startDate ? { start_date: startDate } : {}),
    ...(endDate ? { end_date: endDate } : {}),
    ...(tags?.length ? { tags } : {}),
    limit,
    offset,
  })
}

export async function uploadImages(files: File[]): Promise<{ paths: string[] }> {
  const formData = new FormData()
  files.forEach(f => formData.append('images', f))
  // Use relative URL so it works on both localhost and LAN access
  const res = await fetch('/v1/upload/images', {
    method: 'POST',
    body: formData,
  })
  if (!res.ok) {
    const error = await res.json().catch(() => null)
    throw new Error(error?.message || `上传失败：${res.status}`)
  }
  return res.json()
}
