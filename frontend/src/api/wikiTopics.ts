import api from '@/api'
import { isAxiosError } from 'axios'

export interface TopicSourceSelection {
  document_id: string
  span_id?: string
  range?: { offset: number; length: number }
}

export interface TopicCompileRequest {
  page_id?: string | null
  title: string
  guidance: string
  sources: TopicSourceSelection[]
}

export interface TopicSourceDocument {
  id: string
  title: string
  relative_path: string
  current_version_id: string
  span_count: number
}

export interface TopicSourceSection {
  id: string
  heading: string | null
  line_start: number | null
  total_chars: number
}

export interface TopicPageSummary {
  id: string
  title: string
  has_publication: boolean
  has_draft: boolean
  draft_revision_id?: string | null
  needs_review: boolean
  compiling: boolean
  last_error: string | null
}

export interface TopicRevision {
  id: string
  markdown: string
  run_id: string
  evidence_count: number
  created_at: string
  links: Array<TopicLink & { published: boolean }>
}

export interface TopicLink {
  page_id: string
  title: string
  reason: string
}

export interface TopicPage {
  page: TopicPageSummary
  published: TopicRevision | null
  draft: TopicRevision | null
  last_request: TopicCompileRequest | null
  revisions: Array<{ id: string; created_at: string; published_at: string | null }>
  questions: Array<{ id: string; question: string; created_at: string; resolved_at: string | null }>
  backlinks: TopicLink[]
}

export interface TopicWritebackMaterial {
  title: string
  kind: 'answer' | 'report'
  evidence_count: number
  passages: Array<{ index: number; title: string; markdown: string }>
}

export interface TopicWikiIndex {
  overview: string | null
  pages: Array<{ page_id: string; title: string; summary: string; scope: string | null; needs_review: boolean }>
  issues: Array<{ page_id: string; code: string; detail: string }>
  builds: Array<{
    attempt_id: string; status: string; source_count: number; linked_sources: number;
    written_pages: number; planned_pages: number; recorded_at: string
    maintenance?: TopicMaintenance
  }>
  publications: Array<{ page_id: string; title: string; revision_id: string; published_at: string }>
}

export interface TopicBookBuildSummary {
  status: 'running' | 'ready' | 'partial' | 'failed'
  phase: string
  outdated: boolean
  source_count: number
  ready_pages: number
  total_pages: number
}

export interface TopicMaintenance {
  reused_pages: number
  changes: Array<{
    path: string
    change: 'added' | 'modified' | 'removed' | 'unreadable' | 'empty'
    disposition: 'update' | 'add' | 'no_change' | 'review'
    reason: string
    page_ids: string[]
  }>
}

export interface TopicBookBuild extends Pick<TopicBookBuildSummary, 'status' | 'phase' | 'outdated'> {
  id: string
  stop_requested: boolean
  reviewed_groups: number
  total_groups: number
  error: string | null
  sources: Array<{
    path: string
    page_ids: string[]
    total_sections: number
    linked_sections: number
    status: 'indexed' | 'linked' | 'empty' | 'unreadable'
    error: string | null
  }>
  pages: Array<{ page_id: string; title: string; focus?: string; revision_id: string | null; error: string | null }>
  failed_groups: Array<{ group: number; error: string }>
  maintenance?: TopicMaintenance
}

const root = (base: string) => `/knowledge/bases/${encodeURIComponent(base)}/topics`
const page = (base: string, id: string) => `${root(base)}/${encodeURIComponent(id)}`
const revision = (base: string, id: string, version: string) => `${page(base, id)}/revisions/${encodeURIComponent(version)}`

export function topicErrorMessage(error: unknown) {
  if (isAxiosError<{ message?: string }>(error)) return error.response?.data?.message || error.message
  return error instanceof Error ? error.message : '操作未完成，请重试'
}

export const listTopicPages = (base: string) => api.get<unknown, TopicPageSummary[]>(root(base))
export const getTopicWikiIndex = (base: string) => api.get<unknown, TopicWikiIndex>(`${root(base)}/index`)
export const getTopicBookBuild = (base: string) => api.get<unknown, TopicBookBuild | null>(`${root(base)}/build`)
export const startTopicBookBuild = (base: string) => api.post<unknown, TopicBookBuild>(`${root(base)}/build`)
export const stopTopicBookBuild = (base: string, attempt: string) =>
  api.post<unknown, TopicBookBuild>(`${root(base)}/build/${encodeURIComponent(attempt)}/stop`)
export const publishTopicBookBuild = (base: string, attempt: string) =>
  api.post<unknown, number>(`${root(base)}/build/${encodeURIComponent(attempt)}/publish`)
export const listTopicSources = (base: string) => api.get<unknown, TopicSourceDocument[]>(`${root(base)}/sources`)
export const listTopicSourceSections = (base: string, doc: string, offset = 0) =>
  api.get<unknown, TopicSourceSection[]>(`${root(base)}/sources/${encodeURIComponent(doc)}`, { params: { offset } })
export const getTopicPage = (base: string, id: string) => api.get<unknown, TopicPage>(page(base, id))
export const getTopicRevision = (base: string, id: string, version: string) => api.get<unknown, TopicRevision>(revision(base, id, version))
export const compileTopicPage = (base: string, request: TopicCompileRequest) =>
  api.post<unknown, TopicPage>(root(base), request, { timeout: 185_000 })
export const publishTopicPage = (base: string, id: string, version: string) =>
  api.post<unknown, TopicPage>(`${revision(base, id, version)}/publish`)
export const editTopicMarkdown = (base: string, id: string, version: string, markdown: string) =>
  api.put<unknown, TopicPage>(revision(base, id, version), { markdown })
export const getTopicWritebackMaterial = (base: string, runId: string) =>
  api.get<unknown, TopicWritebackMaterial>(`${root(base)}/writeback-material/${encodeURIComponent(runId)}`)
export const draftTopicFromMaterial = (base: string, id: string, runId: string, version: string, passages: number[]) =>
  api.post<unknown, TopicPage>(`${page(base, id)}/writeback-draft`, { run_id: runId, expected_revision_id: version, passages })
export const saveTopicQuestion = (base: string, id: string, runId: string) =>
  api.post<unknown, TopicPage>(`${page(base, id)}/questions`, { run_id: runId })
export const resolveTopicQuestion = (base: string, id: string, questionId: string) =>
  api.post<unknown, TopicPage>(`${page(base, id)}/questions/${encodeURIComponent(questionId)}/resolve`)
