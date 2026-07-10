import { callTool } from '@/api'

/** A single node in the directory tree returned by list_local_dir. */
export interface DirEntry {
  name: string
  path: string
  is_dir: boolean
  is_markdown: boolean
  children?: DirEntry[]
}

export interface ListLocalDirResult {
  root: string
  entries: DirEntry[]
  total: number
}

export interface ReadLocalFileResult {
  path: string
  name: string
  content: string
  size: number
}

/** Envelope returned by /v1/tools/call — the interceptor unwraps response.data. */
export interface ToolEnvelope<T> {
  tool: string
  status: string
  result?: T
  error?: { code: string; message: string; suggestion?: string }
}

/** List a local directory as a recursive file tree (filesystem-scoped, not vault). */
export function listLocalDir(path: string, depth = 3): Promise<ToolEnvelope<ListLocalDirResult>> {
  return callTool('list_local_dir', { path, depth }) as unknown as Promise<ToolEnvelope<ListLocalDirResult>>
}

/** Read a local file's text content (5MB cap). */
export function readLocalFile(path: string): Promise<ToolEnvelope<ReadLocalFileResult>> {
  return callTool('read_local_file', { path }) as unknown as Promise<ToolEnvelope<ReadLocalFileResult>>
}

/** A reader history entry (server-stored, shared across all users). */
export interface HistoryItem {
  path: string
  pinned: boolean
  lastUsed: number
}

/** Get the shared reader history from the server. */
export function getReaderHistory(): Promise<ToolEnvelope<{ history: HistoryItem[] }>> {
  return callTool('get_reader_history', {}) as unknown as Promise<
    ToolEnvelope<{ history: HistoryItem[] }>
  >
}

/** Save the full reader history list to the server (replaces existing). */
export function saveReaderHistory(
  history: HistoryItem[],
): Promise<ToolEnvelope<{ ok: boolean; count: number }>> {
  return callTool('save_reader_history', { history }) as unknown as Promise<
    ToolEnvelope<{ ok: boolean; count: number }>
  >
}
