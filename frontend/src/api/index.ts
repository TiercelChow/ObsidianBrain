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
