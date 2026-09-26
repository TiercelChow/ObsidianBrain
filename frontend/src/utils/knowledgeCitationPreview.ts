import type { AgentRunCitationPreview, KnowledgeEntryDetail, KnowledgeEntrySummary } from '../api/knowledge'

interface PreviewResponse<T> { status: string; result?: T; error?: { message: string } }

/** A failed historical lookup must never silently substitute today's mutable entity. */
export async function loadKnowledgeCitationPreview(
  entry: KnowledgeEntrySummary,
  runId: string | undefined,
  sourceIndex: number,
  loaders: {
    snapshot(runId: string, sourceIndex: number): Promise<PreviewResponse<AgentRunCitationPreview>>
    current(entryId: string): Promise<PreviewResponse<KnowledgeEntryDetail>>
  },
): Promise<AgentRunCitationPreview> {
  if (runId) {
    const response = await loaders.snapshot(runId, sourceIndex)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '历史证据快照读取失败')
    return response.result
  }
  const response = await loaders.current(entry.id)
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '来源读取失败')
  const detail = response.result
  return {
    run_id: '', citation_index: sourceIndex + 1, kind: 'entry', object_id: detail.id,
    version_id: String(detail.revision), entry: detail, content_md: detail.content_md,
    citations: detail.citations, historical: false, read_ranges: [],
  }
}
