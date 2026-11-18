import api from '@/api'
import type { KnowledgeArtifact, PresentationPagination } from '@/api/knowledge'

export interface PresentationSourceReference {
  index: number
  title: string
  source_path: string | null
}

interface SavedPresentationPage {
  page: number
  title: string
  layout: string
  role: string | null
  estimated_seconds: number | null
  citations: string[]
  fields: Array<{ path: string; label: string; text: string }>
  fixed_text: string[]
  used_characters: number
  character_limit: number
}

export interface SavedPresentationOutline {
  artifact_id: string
  report_run_id: string
  cover_title: string
  cover_subtitle: string
  core_message: string
  pages: SavedPresentationPage[]
  sources: PresentationSourceReference[]
  read_only_reason: string | null
}

export interface PresentationOutlinePreview {
  report_run_id: string
  paging: {
    requested: PresentationPagination
    min_total: number
    preferred_total: number
    max_total: number
    speaking_seconds_limit: number | null
  }
  reference_total: number
  tight: boolean
  sections: Array<{ title: string; finding_start: number; finding_end: number }>
  findings: Array<{ finding: string; status: string; citation_indices: number[]; limitations: string[] }>
  sources: PresentationSourceReference[]
}

const presentationUrl = (artifactId: string) => `/knowledge/artifacts/${encodeURIComponent(artifactId)}/presentation`

export function getSavedPresentationOutline(artifactId: string) {
  return api.get<unknown, SavedPresentationOutline>(presentationUrl(artifactId))
}

export function editPresentationSlide(artifactId: string, reportRunId: string, page: number, changes: Record<string, string>) {
  return api.post<unknown, KnowledgeArtifact>(presentationUrl(artifactId), {
    report_run_id: reportRunId, page, changes,
  })
}

export function previewPresentationOutline(taskId: string, reportRunId: string, pagination: PresentationPagination) {
  return api.post<unknown, PresentationOutlinePreview>(
    `/knowledge/tasks/${encodeURIComponent(taskId)}/presentation-outline`,
    { report_run_id: reportRunId, pagination },
  )
}
