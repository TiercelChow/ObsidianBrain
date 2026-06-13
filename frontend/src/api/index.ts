import axios from 'axios'

const api = axios.create({
  baseURL: '/v1',
  timeout: 30000,
  headers: { 'Content-Type': 'application/json' },
})

api.interceptors.response.use(
  (response) => response.data,
  (error) => {
    console.error('API 错误:', error.response?.data || error.message)
    return Promise.reject(error)
  }
)

export default api

// ── Health ──
export function getHealth() {
  return api.get('/health')
}

// ── Tools ──
export function listTools() {
  return api.get('/tools')
}

export function callTool(tool: string, args: Record<string, unknown> = {}) {
  return api.post('/tools/call', { tool, arguments: args })
}

// ── Convenience: wrapped tool calls ──
export function searchNotes(query: string, topK = 5, tags?: string[]) {
  return callTool('search_notes', { query, top_k: topK, ...(tags?.length ? { tags } : {}) })
}

export function getNote(path: string) {
  return callTool('get_note', { path })
}

export function listRecentNotes(days?: number, limit?: number) {
  return callTool('list_recent_notes', { ...(days ? { days } : {}), ...(limit ? { limit } : {}) })
}

export function searchMemory(query: string, topK = 5, tags?: string[]) {
  return callTool('search_memory', { query, top_k: topK, ...(tags?.length ? { tags } : {}) })
}

export function addMemory(notePath: string, content: string, tags?: string[]) {
  return callTool('add_memory', { note_path: notePath, content, ...(tags?.length ? { tags } : {}) })
}

export function updateMemory(memoryId: string, content: string) {
  return callTool('update_memory', { memory_id: memoryId, content })
}

export function forgetMemory(memoryId: string) {
  return callTool('forget_memory', { memory_id: memoryId })
}

export function getMemoryStats() {
  return callTool('get_memory_stats')
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

export function searchMemos(query: string, startDate?: string, endDate?: string, tags?: string[], limit = 20) {
  return callTool('search_memos', {
    query,
    ...(startDate ? { start_date: startDate } : {}),
    ...(endDate ? { end_date: endDate } : {}),
    ...(tags?.length ? { tags } : {}),
    limit,
  })
}

// ── Inspiration ──
export function getInspiration(type?: string, notePath?: string) {
  return callTool('get_inspiration', {
    ...(type ? { type } : {}),
    ...(notePath ? { note_path: notePath } : {})
  })
}

// ── Radar ──
export function getRadar(limit?: number) {
  return callTool('get_radar', { ...(limit ? { limit } : {}) })
}

export function addToVault(articleId: string, targetDir?: string) {
  return callTool('add_to_vault', {
    article_id: articleId,
    ...(targetDir ? { target_dir: targetDir } : {})
  })
}

export function dismissRadarItem(articleId: string) {
  return callTool('dismiss_radar_item', { article_id: articleId })
}
