import api, { callTool } from '@/api'
import type { TopicBookBuildSummary, TopicPage } from '@/api/wikiTopics'
import type { ReaderBook, ToolEnvelope } from '@/api/reader'
import { createSseDataParser } from '@/utils/sseDataParser'
import { buildModelProviderPayload } from '@/utils/modelProviderPayload'

export interface KnowledgeBaseSummary {
  id: string
  book_id: string
  book_name: string
  book_path: string
  book_kind: 'folder' | 'pdf'
  book_description: string
  book_category: string
  source_available: boolean
  lifecycle: 'uninitialized' | 'active' | 'paused' | 'archived'
  sync_state: 'clean' | 'outdated' | 'scanning' | 'extracting' | 'ingesting' | 'failed'
  compile_mode: 'chapter' | 'smart'
  compile_state: 'not_started' | 'outdated' | 'compiling' | 'ready' | 'failed'
  compile_phase: string
  compile_message: string | null
  compile_current_batch: number
  compile_total_batches: number
  compile_active_run_id: string | null
  compile_change_set_id: string | null
  compile_started_at: string | null
  compile_heartbeat_at: string | null
  compile_cancel_requested: boolean
  compile_error?: string | null
  health_state: 'healthy' | 'warning' | 'needs_review'
  last_error?: string | null
  last_synced_at?: string | null
  last_scanned_at?: string | null
  last_compiled_at?: string | null
  compile_processed_sources: number
  compile_total_sources: number
  pending_review_count: number
  source_count: number
  entry_count: number
  claim_count: number
  task_count: number
}

export interface BookKnowledgeCard {
  book: ReaderBook
  knowledge_base?: KnowledgeBaseSummary | null
  topic_build?: TopicBookBuildSummary
  topic_draft_count: number
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

export interface KnowledgeClaimSummary {
  id: string
  predicate: string
  object_text?: string | null
  claim_text: string
  confidence?: number | null
  verification_status: string
  citation_count: number
}

export interface KnowledgeRelationSummary {
  id: string
  direction: 'incoming' | 'outgoing'
  relation_type: string
  related_entry_id: string
  related_entry_title: string
  strength?: number | null
  evidence: string
}

export interface KnowledgeEntryVersionSummary {
  revision: number
  title: string
  summary: string
  status: string
  created_at: string
}

export interface KnowledgeEntryDetail extends KnowledgeEntrySummary {
  content_md: string
  aliases: string[]
  edit_policy: string
  revision: number
  citations: KnowledgeCitation[]
  claims: KnowledgeClaimSummary[]
  relations: KnowledgeRelationSummary[]
  versions: KnowledgeEntryVersionSummary[]
  source_impact_count?: number
  source_impacts?: KnowledgeSourceImpact[]
}

export interface KnowledgeSourceImpact {
  source_document_id: string
  source_path: string
  previous_version_id: string
  current_version_id: string | null
  reason: 'source_changed' | 'source_missing' | 'source_reindexed'
  affected_via_entry_id: string | null
  affected_via_entry_title: string | null
  detected_at: string
}

export interface KnowledgeEntryPage {
  entries: KnowledgeEntrySummary[]
  offset: number
  limit: number
  total: number
  has_more: boolean
}

export interface KnowledgeBridgeEntry extends KnowledgeEntrySummary {
  degree: number
}

export interface KnowledgeGraphOverview {
  relation_count: number
  orphan_entries: KnowledgeEntrySummary[]
  bridge_entries: KnowledgeBridgeEntry[]
}

export interface KnowledgeGraphPath {
  entries: KnowledgeEntrySummary[]
}

export interface KnowledgeGraphRelation {
  id: string
  from_entry_id: string
  to_entry_id: string
  relation_type: string
  strength?: number | null
  evidence: string
}

export interface KnowledgeGraphSnapshot {
  entries: KnowledgeEntrySummary[]
  relations: KnowledgeGraphRelation[]
  total_entries: number
  truncated: boolean
}

export type PresentationPagination =
  | { mode: 'auto' }
  | { mode: 'fixed'; total_slides: number }
  | { mode: 'duration'; minutes: number }

export interface ResearchBrief {
  confirmed: boolean
  audience: 'general' | 'specialist' | 'beginner' | 'self'
  purpose: 'understand' | 'decision' | 'teach' | 'reference'
  tone: 'analytical' | 'technical' | 'narrative' | 'concise'
  depth: 'brief' | 'standard' | 'deep'
  presentation_theme: 'editorial' | 'midnight' | 'sage'
  presentation_format: 'narrative' | 'qa'
  presentation_pagination?: PresentationPagination
  emphasis: string
}

export interface ResearchPreflight {
  summary: string
  recommended: ResearchBrief
  focus_decisions: Array<'audience' | 'purpose' | 'tone' | 'depth' | 'presentation_theme' | 'presentation_format'>
  decision_points?: Array<{ field: ResearchPreflight['focus_decisions'][number]; question: string; impact: string }>
  cautions: string[]
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
  deliverable_type: 'report' | 'presentation'
  artifact_state: 'not_requested' | 'pending' | 'ready' | 'failed'
  knowledge_change_state: 'none' | 'proposed' | 'applied' | 'rejected'
  cancel_requested: boolean
  external_research_enabled: boolean
  external_domains: string[]
  external_request_limit: number
  external_requests_used: number
  brief: ResearchBrief
  created_at: string
  updated_at: string
}

export interface ResearchFinding {
  finding: string
  status: 'supported' | 'partial' | 'missing' | 'conflict'
  citation_indices: number[]
  limitations: string[]
  baseline_entry_id?: string | null
  baseline_claim_id?: string | null
}

export interface ResearchStageSummary {
  stage_key: string
  title: string
  kind: 'plan' | 'section' | 'synthesis' | 'report' | 'validation' | 'presentation'
  ordinal: number
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled' | 'stale'
  revision: number
  run_id?: string | null
  summary: string
  content_characters: number
  finding_count: number
  error?: string | null
  updated_at: string
}

export interface ResearchWorkspace {
  task_id: string
  knowledge_base_id: string
  original_request: Record<string, unknown>
  plan?: {
    goal: string; report_title?: string | null; constraints: string[]; acceptance: string[]; depth: string; terminology: string[]
    questions: { id: string; title: string; question: string; required_evidence: string[]; target_entry_ids?: string[]; expected_output_tokens?: number | null }[]
  } | null
  stages: ResearchStageSummary[]
  baselines: { question_id: string; entry_id: string; revision: number; title: string; status: string; content_characters: number; claim_count: number; captured_at: string }[]
  created_at: string
  updated_at: string
}

export interface ResearchStageContent {
  stage: ResearchStageSummary
  content_run_id?: string | null
  content_md: string
  findings: ResearchFinding[]
  evidence: { run_id: string; citation_index: number; kind: string; object_id: string; version_id: string; snapshot_hash: string }[]
}

export interface ResearchUnpublishedOutput {
  run_id: string
  kind: 'interrupted' | 'rejected'
  text: string
  stop_reason: string | null
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
  runtime: 'deepseek_harness'
  executable: string
  model: string
  provider_id?: string | null
  provider_config?: ModelProviderProfile | null
  enabled: boolean
  revision: number
  updated_at: string
}

export interface ModelProviderProfile {
  provider_id: string
  display_name: string
  api_protocol: 'openai-completions' | 'openai-responses' | 'anthropic-messages'
  base_url: string
  model: string
  context_window?: number | null
  max_output_tokens?: number | null
  reasoning_policy?: 'auto' | 'off' | 'minimal' | 'low' | 'medium' | 'high' | 'xhigh' | 'max'
  credential_source: 'local_file' | 'environment'
  api_key_env: string
  api_key_configured: boolean
  enabled: boolean
  revision: number
  updated_at: string
}

export interface SaveModelProviderRequest {
  provider_id?: string
  display_name: string
  api_protocol: ModelProviderProfile['api_protocol']
  base_url: string
  model: string
  context_window?: number | null
  max_output_tokens?: number | null
  reasoning_policy?: ModelProviderProfile['reasoning_policy']
  credential_source: ModelProviderProfile['credential_source']
  api_key_env?: string
  api_key?: string
  clear_api_key?: boolean
  enabled?: boolean
  expected_revision?: number
}

export interface RuntimeHealth {
  profile: RuntimeProfile
  available: boolean
  version?: string | null
  message: string
}

export interface RuntimeVerification {
  profile_id: string
  available: boolean
  message: string
}

export interface KnowledgeAnswer {
  run_id: string
  conversation_id: string
  answer: string
  runtime: 'deepseek_harness'
  evidence: KnowledgeEntrySummary[]
}

export interface KnowledgeConversationSummary {
  id: string
  knowledge_base_id: string
  title: string
  message_count: number
  preview: string
  created_at: string
  updated_at: string
}

export interface UnfinishedQaRun {
  run_id: string
  question: string
  conversation_id?: string | null
  completed_without_history: boolean
  stop_reason?: string | null
  has_partial_answer: boolean
  error: string
  created_at: string
}

export interface KnowledgeChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  run_id?: string | null
  evidence: KnowledgeEntrySummary[]
  created_at: string
}

export interface KnowledgeConversationDetail extends KnowledgeConversationSummary {
  messages: KnowledgeChatMessage[]
}

export interface KnowledgeTaskExecution {
  task: KnowledgeTask
  run_id: string
  report_verified: boolean
  evidence: KnowledgeEntrySummary[]
  artifacts: KnowledgeArtifact[]
}

export interface KnowledgeArtifact {
  id: string
  knowledge_base_id: string
  knowledge_task_id?: string | null
  agent_run_id?: string | null
  skill_id?: string | null
  artifact_type: 'pptx' | 'pdf' | 'image' | 'report'
  title: string
  relative_path: string
  mime_type: string
  content_hash: string
  size_bytes: number
  validation_state: 'pending' | 'valid' | 'warning' | 'invalid'
  validation_message: string
  validation_details: Partial<PresentationQualityReport>
  created_at: string
}

export interface PresentationQualityCheck {
  code: string
  label: string
  passed: boolean
  detail: string
}

export interface PresentationQualityReport {
  schema_version: string
  slide_count: number
  content_slide_count: number
  theme: 'editorial' | 'midnight' | 'sage'
  layout_count: number
  layout_distribution: Record<string, number>
  cited_slide_count: number
  citation_coverage_percent: number
  package_part_count: number
  editable_text_and_shapes: boolean
  estimated_duration_seconds?: number
  checks: PresentationQualityCheck[]
  summary: string
}

export interface AgentUsageTotals {
  runs: number
  unreported_runs: number
  input_tokens: number
  output_tokens: number
  reasoning_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  total_tokens: number
}

export interface AgentUsagePoint extends AgentUsageTotals {
  date: string
}

export interface AgentUsageCaller extends AgentUsageTotals {
  caller: 'knowledge_qa' | 'knowledge_task' | string
}

export interface AgentUsageStats {
  start_date: string
  end_date: string
  caller?: string | null
  usage_source: 'unavailable' | 'estimated' | 'measured' | 'mixed'
  totals: AgentUsageTotals
  daily: AgentUsagePoint[]
  by_caller: AgentUsageCaller[]
}

export interface WikiSkill {
  id: string
  slug: string
  name: string
  description: string
  source_type: 'builtin' | 'custom'
  status: 'ready' | 'invalid' | 'unavailable'
  permissions: string[]
  requirements: string[]
  revision: number
  instructions: string
  enabled: boolean
  usage_scope: 'qa' | 'research' | 'both' | 'ingest' | 'all'
  updated_at: string
}

export interface WikiSkillFile {
  relative_path: string
  media_type: string
  content_text: string
  content_hash: string
  size_bytes: number
}

export interface WikiSkillVersion {
  id: string
  revision: number
  content_hash: string
  release_state: 'candidate' | 'published' | 'retired'
  parent_version_id?: string | null
  changelog: string
  created_at: string
  files: WikiSkillFile[]
  origin?: WikiSkillOrigin | null
  latest_evaluation?: WikiSkillEvaluationRun | null
  latest_benchmark?: WikiSkillBenchmarkRun | null
}

export interface WikiSkillOrigin {
  repository_url: string
  source_path: string
  source_ref: string
  license_spdx: string
  attribution: string
  adaptation_notes: string
  reviewed_at: string
}

export interface WikiSkillEvaluationFinding {
  case_id: string
  name: string
  passed: boolean
  missing_concepts: string[]
  forbidden_concepts: string[]
  weight: number
}

export interface WikiSkillEvaluationRun {
  id: string
  skill_version_id: string
  suite_id: string
  suite_name: string
  score: number
  baseline_score?: number | null
  passed: boolean
  findings: WikiSkillEvaluationFinding[]
  created_at: string
}

export interface WikiSkillBenchmarkCaseResult {
  case_id: string
  name: string
  variant: 'baseline' | 'candidate'
  agent_run_id?: string | null
  response_text: string
  citations: string[]
  metrics: Record<string, unknown>
  score: number
  passed: boolean
  error?: string | null
}

export interface WikiSkillBenchmarkRun {
  id: string
  skill_id: string
  skill_version_id: string
  baseline_version_id: string
  suite_id: string
  suite_name: string
  knowledge_base_id: string
  runtime_profile_id: string
  model: string
  status: 'queued' | 'running' | 'completed' | 'failed'
  total_cases: number
  completed_cases: number
  candidate_score?: number | null
  baseline_score?: number | null
  score_delta?: number | null
  passed: boolean
  metrics: Record<string, unknown>
  error?: string | null
  results: WikiSkillBenchmarkCaseResult[]
  started_at?: string | null
  completed_at?: string | null
  created_at: string
}

export interface WikiSkillDetail {
  skill: WikiSkill
  current_version_id: string
  versions: WikiSkillVersion[]
}

export interface AgentRunEvent {
  run_id: string
  sequence: number
  event_type: string
  phase?: string | null
  message: string
  payload: Record<string, unknown>
  created_at: string
}

export interface AgentRun {
  id: string
  knowledge_base_id?: string | null
  runtime: 'deepseek_harness'
  task_type: string
  status: 'running' | 'completed' | 'failed' | 'cancelled'
  input: Record<string, unknown>
  output?: Record<string, unknown> | null
  error?: string | null
  started_at?: string | null
  finished_at?: string | null
  created_at: string
}

export interface AgentRunSkillSnapshot {
  id: string
  slug: string
  name: string
  revision: number
  instructions: string
  permissions: string[]
  requirements: string[]
  application_mode?: 'prompt_injected' | 'declared_only' | 'harness_native_available' | 'benchmark_prompt_injected'
}

export interface AgentRunConfigSnapshot {
  id: string
  scope: string
  name: string
  revision: number
  content_md: string
}

export interface AgentRunInspectionSnapshot {
  run_id: string
  prompt_text: string
  prompt_hash: string
  prompt_characters: number
  skill_snapshots: AgentRunSkillSnapshot[]
  config_snapshots: AgentRunConfigSnapshot[]
  tool_names: string[]
  evidence_refs: Record<string, unknown>
  created_at: string
}

export interface AgentRunInspection {
  run: AgentRun
  events: AgentRunEvent[]
  snapshot?: AgentRunInspectionSnapshot | null
  evidence: Array<{
    kind: 'entry' | 'source_span' | 'external'
    object_id: string
    version_id: string
    snapshot: Record<string, unknown>
  }>
}

export interface AgentRunCitationPreview {
  run_id: string
  citation_index: number
  kind: 'entry' | 'source_span' | 'external'
  object_id: string
  version_id: string
  entry: KnowledgeEntrySummary
  content_md: string
  citations: KnowledgeCitation[]
  historical: boolean
  read_ranges: Array<{ offset_chars: number; returned_chars: number }>
}

export type KnowledgeChatStreamEvent =
  | { type: 'evidence'; evidence: KnowledgeEntrySummary[] }
  | { type: 'planning_ready'; knowledge_base_id: string; question: string; standalone_question: string }
  | { type: 'run_started'; run_id: string }
  | { type: 'text_delta'; run_id: string; delta: string }
  | { type: 'text_replace'; run_id: string; text: string }
  | { type: 'phase'; run_id: string; message: string }
  | { type: 'tool_started'; run_id: string; title: string; kind: string }
  | { type: 'tool_finished'; run_id: string; title?: string | null; status: string }
  | { type: 'usage'; run_id: string; context_used: number; context_size: number }
  | { type: 'completed'; result: KnowledgeAnswer }
  | { type: 'error'; message: string }

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

export function setBookKnowledgeBaseLifecycle(
  knowledgeBaseId: string,
  lifecycle: 'active' | 'paused' | 'archived',
) {
  return callTool('set_book_knowledge_base_lifecycle', {
    knowledge_base_id: knowledgeBaseId,
    lifecycle,
  }) as unknown as Promise<ToolEnvelope<KnowledgeBaseSummary>>
}

export function deleteBookKnowledgeBase(knowledgeBaseId: string, confirmation: string) {
  return callTool('delete_book_knowledge_base', {
    knowledge_base_id: knowledgeBaseId,
    confirmation,
  }) as unknown as Promise<ToolEnvelope<{ deleted: boolean }>>
}

export function listKnowledgeEntries(
  knowledgeBaseId: string,
  options: { query?: string; entryType?: string; offset?: number; limit?: number; includeStale?: boolean } = {},
) {
  return callTool('list_knowledge_entries', {
    knowledge_base_id: knowledgeBaseId,
    ...(options.query ? { query: options.query } : {}),
    ...(options.entryType ? { entry_type: options.entryType } : {}),
    offset: options.offset ?? 0,
    limit: options.limit ?? 60,
    ...(options.includeStale ? { include_stale: true } : {}),
  }) as unknown as Promise<ToolEnvelope<KnowledgeEntryPage>>
}

export function getKnowledgeEntry(entryId: string) {
  return callTool('get_knowledge_entry', { entry_id: entryId }) as unknown as Promise<
    ToolEnvelope<KnowledgeEntryDetail>
  >
}

export function proposeReaderSelection(input: {
  knowledgeBaseId: string
  sourcePath: string
  selection: string
  title?: string
}) {
  return callTool('propose_reader_selection', {
    knowledge_base_id: input.knowledgeBaseId,
    source_path: input.sourcePath,
    selection: input.selection,
    ...(input.title ? { title: input.title } : {}),
  }) as unknown as Promise<ToolEnvelope<TopicPage>>
}

export function askBookKnowledge(knowledgeBaseId: string, question: string, conversationId?: string) {
  return callTool('ask_book_knowledge', {
    knowledge_base_id: knowledgeBaseId,
    question,
    ...(conversationId ? { conversation_id: conversationId } : {}),
  }, { timeout: 850_000 }) as unknown as Promise<ToolEnvelope<KnowledgeAnswer>>
}

export async function streamBookKnowledge(
  knowledgeBaseId: string,
  question: string,
  conversationId: string | undefined,
  onEvent: (event: KnowledgeChatStreamEvent) => void,
  signal?: AbortSignal,
  resumeRunId?: string,
) {
  const response = await fetch('/v1/knowledge/chat/stream', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'text/event-stream' },
    body: JSON.stringify({
      knowledge_base_id: knowledgeBaseId,
      question,
      ...(conversationId ? { conversation_id: conversationId } : {}),
      ...(resumeRunId ? { resume_run_id: resumeRunId } : {}),
    }),
    signal,
  })
  if (!response.ok) {
    const detail = await response.text()
    throw new Error(detail || `问答流连接失败 (${response.status})`)
  }
  if (!response.body) throw new Error('当前环境不支持流式响应')

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let completed: KnowledgeAnswer | undefined
  const parser = createSseDataParser((data) => {
    const event = JSON.parse(data) as KnowledgeChatStreamEvent
    onEvent(event)
    if (event.type === 'completed') completed = event.result
    if (event.type === 'error') throw new Error(event.message)
  })

  try {
    while (true) {
      const { value, done } = await reader.read()
      parser.push(decoder.decode(value, { stream: !done }))
      if (done) break
    }
    parser.finish()
  } finally {
    reader.releaseLock()
  }
  if (!completed) throw new Error('问答流在完成前意外关闭')
  return completed
}

export function recoverCompletedQaAnswer(knowledgeBaseId: string, runId: string) {
  return callTool('recover_completed_qa_answer', {
    knowledge_base_id: knowledgeBaseId,
    run_id: runId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeAnswer>>
}

export function listKnowledgeConversations(knowledgeBaseId: string, limit = 30) {
  return callTool('list_knowledge_conversations', {
    knowledge_base_id: knowledgeBaseId,
    limit,
  }) as unknown as Promise<ToolEnvelope<{ conversations: KnowledgeConversationSummary[] }>>
}

export function listUnfinishedQaRuns(knowledgeBaseId: string, limit = 10) {
  return callTool('list_unfinished_qa_runs', {
    knowledge_base_id: knowledgeBaseId,
    limit,
  }) as unknown as Promise<ToolEnvelope<{ runs: UnfinishedQaRun[] }>>
}

export function getKnowledgeConversation(conversationId: string) {
  return callTool('get_knowledge_conversation', {
    conversation_id: conversationId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeConversationDetail>>
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
  deliverableType?: KnowledgeTask['deliverable_type']
  externalResearchEnabled?: boolean
  externalDomains?: string[]
  externalRequestLimit?: number
  brief?: ResearchBrief
}) {
  return callTool('create_knowledge_task', {
    knowledge_base_id: input.knowledgeBaseId,
    title: input.title,
    description: input.description ?? '',
    task_type: input.taskType ?? 'research',
    deliverable_type: input.deliverableType ?? 'report',
    external_research_enabled: input.externalResearchEnabled ?? false,
    external_domains: input.externalDomains ?? [],
    external_request_limit: input.externalRequestLimit ?? 0,
    ...(input.brief ? { brief: input.brief } : {}),
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function updateKnowledgeTaskBrief(taskId: string, expectedUpdatedAt: string, brief: ResearchBrief) {
  return callTool('update_knowledge_task_brief', {
    task_id: taskId,
    expected_updated_at: expectedUpdatedAt,
    brief,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function previewKnowledgeTaskBrief(input: {
  knowledgeBaseId: string
  title: string
  description: string
  taskType: KnowledgeTask['task_type']
  deliverableType: KnowledgeTask['deliverable_type']
}) {
  return callTool('preview_knowledge_task_brief', {
    knowledge_base_id: input.knowledgeBaseId,
    title: input.title,
    description: input.description,
    task_type: input.taskType,
    deliverable_type: input.deliverableType,
  }, {
    timeout: 180_000,
  }) as unknown as Promise<ToolEnvelope<ResearchPreflight>>
}

export function executeKnowledgeTask(taskId: string) {
  return callTool('execute_knowledge_task', {
    task_id: taskId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function regenerateKnowledgePresentation(taskId: string, expectedUpdatedAt: string, pagination: PresentationPagination) {
  return callTool('regenerate_knowledge_presentation', {
    task_id: taskId,
    expected_updated_at: expectedUpdatedAt,
    pagination,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function getKnowledgeTaskResult(taskId: string) {
  return callTool('get_knowledge_task_result', {
    task_id: taskId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTaskExecution>>
}

export function getKnowledgeResearchWorkspace(taskId: string) {
  return callTool('get_knowledge_research_workspace', { task_id: taskId }) as unknown as Promise<ToolEnvelope<ResearchWorkspace | null>>
}

export function getKnowledgeResearchStage(taskId: string, stageKey: string, revision?: number) {
  return callTool('get_knowledge_research_stage', { task_id: taskId, stage_key: stageKey, ...(revision == null ? {} : { revision }) }) as unknown as Promise<ToolEnvelope<ResearchStageContent>>
}

export function getKnowledgeResearchUnpublishedOutput(taskId: string, stageKey: string, runId: string, revision: number) {
  return callTool('get_knowledge_research_unpublished_output', { task_id: taskId, stage_key: stageKey, run_id: runId, revision }) as unknown as Promise<ToolEnvelope<ResearchUnpublishedOutput | null>>
}

export function cancelKnowledgeTask(taskId: string) {
  return callTool('cancel_knowledge_task', { task_id: taskId }) as unknown as Promise<
    ToolEnvelope<KnowledgeTask>
  >
}

export function knowledgeArtifactDownloadUrl(artifactId: string) {
  return `/v1/knowledge/artifacts/${encodeURIComponent(artifactId)}`
}

export function getBookWikiSettings(knowledgeBaseId?: string) {
  return callTool('get_book_wiki_settings', {
    ...(knowledgeBaseId ? { knowledge_base_id: knowledgeBaseId } : {}),
  }) as unknown as Promise<
    ToolEnvelope<{ runtime_profiles: RuntimeHealth[]; model_providers: ModelProviderProfile[]; documents: ConfigDocument[] }>
  >
}

export function getAgentUsageStats(options: {
  startDate: string
  endDate: string
  caller?: 'knowledge_qa' | 'knowledge_task'
}) {
  return callTool('get_agent_usage_stats', {
    start_date: options.startDate,
    end_date: options.endDate,
    ...(options.caller ? { caller: options.caller } : {}),
  }) as unknown as Promise<ToolEnvelope<AgentUsageStats>>
}

export function listWikiSkills(knowledgeBaseId?: string) {
  return callTool('list_wiki_skills', {
    ...(knowledgeBaseId ? { knowledge_base_id: knowledgeBaseId } : {}),
  }) as unknown as Promise<ToolEnvelope<{ skills: WikiSkill[] }>>
}

export function getWikiSkillDetail(skillId: string, knowledgeBaseId?: string) {
  return callTool('get_wiki_skill_detail', {
    skill_id: skillId,
    ...(knowledgeBaseId ? { knowledge_base_id: knowledgeBaseId } : {}),
  }) as unknown as Promise<ToolEnvelope<WikiSkillDetail>>
}

export function saveCustomWikiSkill(input: {
  skillId?: string
  slug: string
  name: string
  description: string
  instructions: string
  expectedRevision?: number
}) {
  return callTool('save_custom_wiki_skill', {
    ...(input.skillId ? { skill_id: input.skillId } : {}),
    slug: input.slug,
    name: input.name,
    description: input.description,
    instructions: input.instructions,
    ...(input.expectedRevision ? { expected_revision: input.expectedRevision } : {}),
  }) as unknown as Promise<ToolEnvelope<WikiSkill>>
}

export function evaluateWikiSkillVersion(skillId: string, versionId: string, suiteSlug: string) {
  return callTool('evaluate_wiki_skill_version', {
    skill_id: skillId,
    version_id: versionId,
    suite_slug: suiteSlug,
  }) as unknown as Promise<ToolEnvelope<WikiSkillEvaluationRun>>
}

export function startWikiSkillBenchmark(
  knowledgeBaseId: string,
  skillId: string,
  versionId: string,
  suiteSlug: string,
) {
  return callTool('start_wiki_skill_benchmark', {
    knowledge_base_id: knowledgeBaseId,
    skill_id: skillId,
    version_id: versionId,
    suite_slug: suiteSlug,
  }) as unknown as Promise<ToolEnvelope<WikiSkillBenchmarkRun>>
}

export function getWikiSkillBenchmark(runId: string) {
  return callTool('get_wiki_skill_benchmark', {
    run_id: runId,
  }) as unknown as Promise<ToolEnvelope<WikiSkillBenchmarkRun>>
}

export function publishWikiSkillVersion(skillId: string, versionId: string) {
  return callTool('publish_wiki_skill_version', {
    skill_id: skillId,
    version_id: versionId,
  }) as unknown as Promise<ToolEnvelope<WikiSkillDetail>>
}

export function rollbackWikiSkillVersion(skillId: string, versionId: string) {
  return callTool('rollback_wiki_skill_version', {
    skill_id: skillId,
    version_id: versionId,
  }) as unknown as Promise<ToolEnvelope<WikiSkillDetail>>
}

export function importWikiSkillArchive(file: File) {
  const form = new FormData()
  form.append('archive', file)
  return api.post('/knowledge/skills/import', form, {
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 120_000,
  }) as unknown as Promise<{ skill: WikiSkill }>
}

export function setWikiSkillBinding(input: {
  knowledgeBaseId: string
  skillId: string
  enabled: boolean
  usageScope: WikiSkill['usage_scope']
}) {
  return callTool('set_wiki_skill_binding', {
    knowledge_base_id: input.knowledgeBaseId,
    skill_id: input.skillId,
    enabled: input.enabled,
    usage_scope: input.usageScope,
  }) as unknown as Promise<ToolEnvelope<WikiSkill>>
}

export function getAgentRunEvents(runId: string) {
  return callTool('get_agent_run_events', { run_id: runId }) as unknown as Promise<
    ToolEnvelope<{ events: AgentRunEvent[] }>
  >
}

export function getAgentRunInspection(runId: string) {
  return callTool('get_agent_run_inspection', { run_id: runId }) as unknown as Promise<
    ToolEnvelope<AgentRunInspection>
  >
}

export function getAgentRunCitation(runId: string, sourceIndex: number) {
  return callTool('get_agent_run_citation', {
    run_id: runId,
    source_index: sourceIndex,
  }) as unknown as Promise<ToolEnvelope<AgentRunCitationPreview>>
}

export function getKnowledgeTaskActivity(taskId: string) {
  return callTool('get_knowledge_task_activity', { task_id: taskId }) as unknown as Promise<
    ToolEnvelope<{ run?: AgentRun | null; events: AgentRunEvent[] }>
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
    provider_id: profile.provider_id ?? '',
    enabled: profile.enabled,
    expected_revision: profile.revision,
  }) as unknown as Promise<ToolEnvelope<RuntimeProfile>>
}

export function listModelProviders() {
  return callTool('list_model_providers', {}) as unknown as Promise<
    ToolEnvelope<{ model_providers: ModelProviderProfile[] }>
  >
}

export function saveModelProvider(provider: SaveModelProviderRequest) {
  return callTool('save_model_provider', buildModelProviderPayload(provider)) as unknown as Promise<
    ToolEnvelope<ModelProviderProfile>
  >
}

export function deleteModelProvider(providerId: string, expectedRevision: number) {
  return callTool('delete_model_provider', {
    provider_id: providerId,
    expected_revision: expectedRevision,
  }) as unknown as Promise<ToolEnvelope<{ deleted: boolean }>>
}

export function verifyAgentRuntime(profileId: string) {
  return callTool('verify_agent_runtime', {
    profile_id: profileId,
  }, { timeout: 190_000 }) as unknown as Promise<ToolEnvelope<RuntimeVerification>>
}
