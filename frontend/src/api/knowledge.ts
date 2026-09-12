import { callTool } from '@/api'
import type { ReaderBook, ToolEnvelope } from '@/api/reader'

export interface KnowledgeBaseSummary {
  id: string
  book_id: string
  book_name: string
  book_path: string
  book_kind: 'folder' | 'pdf'
  book_description: string
  book_category: string
  lifecycle: 'uninitialized' | 'active' | 'paused' | 'archived'
  sync_state: 'clean' | 'outdated' | 'scanning' | 'extracting' | 'ingesting' | 'failed'
  health_state: 'healthy' | 'warning' | 'needs_review'
  last_error?: string | null
  last_synced_at?: string | null
  last_scanned_at?: string | null
  source_count: number
  entry_count: number
  claim_count: number
  task_count: number
}

export interface BookKnowledgeCard {
  book: ReaderBook
  knowledge_base?: KnowledgeBaseSummary | null
}

export interface SyncKnowledgeBaseResult {
  knowledge_base: KnowledgeBaseSummary
  scanned_sources: number
  indexed_entries: number
  requires_harness: boolean
  message: string
}

export interface KnowledgeEntrySummary {
  id: string
  knowledge_base_id: string
  entry_type: string
  slug: string
  title: string
  summary: string
  status: string
  confidence?: number | null
  source_path?: string | null
  updated_at: string
}

export interface KnowledgeCitation {
  id: string
  source_path: string
  heading?: string | null
  line_start?: number | null
  line_end?: number | null
  quote_text?: string | null
}

export interface KnowledgeEntryDetail extends KnowledgeEntrySummary {
  content_md: string
  citations: KnowledgeCitation[]
}

export interface KnowledgeTask {
  id: string
  knowledge_base_id: string
  book_name: string
  title: string
  description: string
  task_type: 'research' | 'refresh' | 'review'
  status: 'draft' | 'queued' | 'running' | 'completed' | 'failed' | 'cancelled'
  result_summary: string
  created_at: string
  updated_at: string
}

export interface ConfigDocument {
  id: string
  knowledge_base_id?: string | null
  scope: 'global' | 'book'
  name: string
  content_md: string
  revision: number
  updated_at: string
}

export interface RuntimeProfile {
  id: string
  name: string
  runtime: 'deepseek_harness' | 'claude_code'
  executable: string
  model: string
  enabled: boolean
  revision: number
  updated_at: string
}

export interface RuntimeHealth {
  profile: RuntimeProfile
  available: boolean
  version?: string | null
  message: string
}

export function listBookKnowledgeBases() {
  return callTool('list_book_knowledge_bases') as unknown as Promise<
    ToolEnvelope<{ items: BookKnowledgeCard[] }>
  >
}

export function initializeBookKnowledgeBase(bookId: string, sync = true) {
  return callTool('initialize_book_knowledge_base', {
    book_id: bookId,
    sync,
  }) as unknown as Promise<ToolEnvelope<SyncKnowledgeBaseResult>>
}

export function syncBookKnowledgeBase(knowledgeBaseId: string) {
  return callTool('sync_book_knowledge_base', {
    knowledge_base_id: knowledgeBaseId,
  }) as unknown as Promise<ToolEnvelope<SyncKnowledgeBaseResult>>
}

export function getBookKnowledgeBase(knowledgeBaseId: string) {
  return callTool('get_book_knowledge_base', {
    knowledge_base_id: knowledgeBaseId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeBaseSummary>>
}

export function listKnowledgeEntries(
  knowledgeBaseId: string,
  options: { query?: string; entryType?: string; limit?: number } = {},
) {
  return callTool('list_knowledge_entries', {
    knowledge_base_id: knowledgeBaseId,
    ...(options.query ? { query: options.query } : {}),
    ...(options.entryType ? { entry_type: options.entryType } : {}),
    limit: options.limit ?? 100,
  }) as unknown as Promise<ToolEnvelope<{ entries: KnowledgeEntrySummary[] }>>
}

export function getKnowledgeEntry(entryId: string) {
  return callTool('get_knowledge_entry', { entry_id: entryId }) as unknown as Promise<
    ToolEnvelope<KnowledgeEntryDetail>
  >
}

export function listKnowledgeTasks(knowledgeBaseId?: string) {
  return callTool('list_knowledge_tasks', {
    ...(knowledgeBaseId ? { knowledge_base_id: knowledgeBaseId } : {}),
  }) as unknown as Promise<ToolEnvelope<{ tasks: KnowledgeTask[] }>>
}

export function createKnowledgeTask(input: {
  knowledgeBaseId: string
  title: string
  description?: string
  taskType?: KnowledgeTask['task_type']
}) {
  return callTool('create_knowledge_task', {
    knowledge_base_id: input.knowledgeBaseId,
    title: input.title,
    description: input.description ?? '',
    task_type: input.taskType ?? 'research',
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function getBookWikiSettings(knowledgeBaseId?: string) {
  return callTool('get_book_wiki_settings', {
    ...(knowledgeBaseId ? { knowledge_base_id: knowledgeBaseId } : {}),
  }) as unknown as Promise<
    ToolEnvelope<{ runtime_profiles: RuntimeHealth[]; documents: ConfigDocument[] }>
  >
}

export function saveBookWikiConfigDocument(document: ConfigDocument) {
  return callTool('save_book_wiki_config_document', {
    document_id: document.id,
    content_md: document.content_md,
    expected_revision: document.revision,
  }) as unknown as Promise<ToolEnvelope<ConfigDocument>>
}

export function saveAgentRuntimeProfile(profile: RuntimeProfile) {
  return callTool('save_agent_runtime_profile', {
    profile_id: profile.id,
    executable: profile.executable,
    model: profile.model,
    enabled: profile.enabled,
    expected_revision: profile.revision,
  }) as unknown as Promise<ToolEnvelope<RuntimeProfile>>
}
