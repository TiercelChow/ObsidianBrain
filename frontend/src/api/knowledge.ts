import api, { callTool } from '@/api'
import type { ReaderBook, ToolEnvelope } from '@/api/reader'

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
  runtime: 'deepseek_harness'
  executable: string
  model: string
  provider_config?: RuntimeProviderConfig | null
  enabled: boolean
  revision: number
  updated_at: string
}

export interface RuntimeProviderConfig {
  provider_id: string
  display_name: string
  api_protocol: 'openai-completions' | 'openai-responses' | 'anthropic-messages'
  base_url: string
  api_key_env: string
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
  created_at: string
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

export interface DatabaseBackup {
  filename: string
  reason: string
  created_at: string
  size_bytes: number
}

export interface DatabaseValidationReport {
  integrity_ok: boolean
  integrity_message: string
  foreign_key_violations: number
  migration_version: number
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
  application_mode?: 'prompt_injected' | 'declared_only'
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
}

export type KnowledgeChatStreamEvent =
  | { type: 'evidence'; evidence: KnowledgeEntrySummary[] }
  | { type: 'run_started'; run_id: string }
  | { type: 'text_delta'; run_id: string; delta: string }
  | { type: 'phase'; run_id: string; message: string }
  | { type: 'tool_started'; run_id: string; title: string; kind: string }
  | { type: 'tool_finished'; run_id: string; title?: string | null; status: string }
  | { type: 'usage'; run_id: string; context_used: number; context_size: number }
  | { type: 'completed'; result: KnowledgeAnswer }
  | { type: 'error'; message: string }

export interface KnowledgeChange {
  id: string
  ordinal: number
  operation: 'create' | 'update' | 'merge' | 'split' | 'archive' | 'restore'
  object_type: 'entry' | 'claim' | 'relation'
  object_id: string
  expected_revision?: number | null
  classification: 'new' | 'update' | 'disputed'
  citation_audit: {
    passed: boolean
    entry_citations: number
    explicit_claim_citations: number
    inherited_claims: number
    effective_claim_citations: number
    issues: string[]
  }
  impact: {
    entries: number
    claims: number
    relations: number
    citations: number
    relation_entry_ids: string[]
  }
  before?: Record<string, unknown> | null
  after: Record<string, unknown>
}

export interface KnowledgeChangeSet {
  id: string
  knowledge_base_id: string
  agent_run_id?: string | null
  title: string
  reason: string
  risk_level: 'low' | 'medium' | 'high'
  status: 'proposed' | 'approved' | 'rejected' | 'applied' | 'conflicted'
  created_at: string
  resolved_at?: string | null
  resolved_by?: string | null
  classification_summary: { new: number; update: number; disputed: number; no_material: number }
  citation_audit: { passed: boolean; entry_citations: number; claim_citations: number; issues: string[] }
  impact_summary: { entries: number; claims: number; relations: number; citations: number }
  changes: KnowledgeChange[]
}

export interface SemanticCompileResult {
  knowledge_base: KnowledgeBaseSummary
  change_set: KnowledgeChangeSet
  processed_sources: number
  total_sources: number
}

export interface SemanticCompileQueueResult {
  knowledge_base: KnowledgeBaseSummary
  queued: boolean
  message: string
}

export interface SemanticCompileCancelResult {
  knowledge_base: KnowledgeBaseSummary
  cancel_requested: boolean
}

export interface KnowledgeHealthIssue {
  code: string
  severity: 'warning' | 'error'
  title: string
  detail: string
  object_ids: string[]
}

export interface KnowledgeHealthReport {
  knowledge_base_id: string
  state: 'healthy' | 'warning' | 'error'
  semantic_entry_count: number
  source_span_count: number
  pending_review_count: number
  issues: KnowledgeHealthIssue[]
  generated_at: string
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

export function compileBookKnowledgeBase(knowledgeBaseId: string) {
  return callTool('compile_book_knowledge_base', {
    knowledge_base_id: knowledgeBaseId,
  }) as unknown as Promise<ToolEnvelope<SemanticCompileQueueResult>>
}

export function cancelBookKnowledgeCompile(knowledgeBaseId: string) {
  return callTool('cancel_book_knowledge_compile', {
    knowledge_base_id: knowledgeBaseId,
  }) as unknown as Promise<ToolEnvelope<SemanticCompileCancelResult>>
}

export function listKnowledgeChangeSets(knowledgeBaseId: string, status?: KnowledgeChangeSet['status']) {
  return callTool('list_knowledge_change_sets', {
    knowledge_base_id: knowledgeBaseId,
    ...(status ? { status } : {}),
  }) as unknown as Promise<ToolEnvelope<{ change_sets: KnowledgeChangeSet[] }>>
}

export function resolveKnowledgeChangeSet(
  changeSetId: string,
  decision: 'approve' | 'reject',
  note = '',
) {
  return callTool('resolve_knowledge_change_set', {
    change_set_id: changeSetId,
    decision,
    note,
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
}

export function listKnowledgeEntries(
  knowledgeBaseId: string,
  options: { query?: string; entryType?: string; offset?: number; limit?: number } = {},
) {
  return callTool('list_knowledge_entries', {
    knowledge_base_id: knowledgeBaseId,
    ...(options.query ? { query: options.query } : {}),
    ...(options.entryType ? { entry_type: options.entryType } : {}),
    offset: options.offset ?? 0,
    limit: options.limit ?? 60,
  }) as unknown as Promise<ToolEnvelope<KnowledgeEntryPage>>
}

export function getKnowledgeEntry(entryId: string) {
  return callTool('get_knowledge_entry', { entry_id: entryId }) as unknown as Promise<
    ToolEnvelope<KnowledgeEntryDetail>
  >
}

export function proposeKnowledgeEntryEdit(input: {
  entryId: string
  title: string
  summary: string
  contentMd: string
  aliases: string[]
  status: 'draft' | 'verified' | 'archived'
  expectedRevision: number
}) {
  return callTool('propose_knowledge_entry_edit', {
    entry_id: input.entryId,
    title: input.title,
    summary: input.summary,
    content_md: input.contentMd,
    aliases: input.aliases,
    status: input.status,
    expected_revision: input.expectedRevision,
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
}

export function proposeKnowledgeEntryMerge(input: {
  targetEntryId: string
  title: string
  summary: string
  contentMd: string
  aliases: string[]
  status: 'draft' | 'verified'
  expectedRevision: number
  sources: Array<{ entryId: string; expectedRevision: number }>
}) {
  return callTool('propose_knowledge_entry_merge', {
    target_entry_id: input.targetEntryId,
    title: input.title,
    summary: input.summary,
    content_md: input.contentMd,
    aliases: input.aliases,
    status: input.status,
    expected_revision: input.expectedRevision,
    sources: input.sources.map(source => ({
      entry_id: source.entryId,
      expected_revision: source.expectedRevision,
    })),
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
}

export function proposeKnowledgeEntrySplit(input: {
  entryId: string
  expectedRevision: number
  parts: Array<{ title: string; summary: string; contentMd: string; aliases: string[] }>
}) {
  return callTool('propose_knowledge_entry_split', {
    entry_id: input.entryId,
    expected_revision: input.expectedRevision,
    parts: input.parts.map(part => ({
      title: part.title,
      summary: part.summary,
      content_md: part.contentMd,
      aliases: part.aliases,
    })),
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
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
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
}

export function getKnowledgeGraphOverview(knowledgeBaseId: string, limit = 20) {
  return callTool('get_knowledge_graph_overview', {
    knowledge_base_id: knowledgeBaseId,
    limit,
  }) as unknown as Promise<ToolEnvelope<KnowledgeGraphOverview>>
}

export function findKnowledgeGraphPath(
  knowledgeBaseId: string,
  fromEntryId: string,
  toEntryId: string,
  maxDepth = 5,
) {
  return callTool('find_knowledge_graph_path', {
    knowledge_base_id: knowledgeBaseId,
    from_entry_id: fromEntryId,
    to_entry_id: toEntryId,
    max_depth: maxDepth,
  }) as unknown as Promise<ToolEnvelope<KnowledgeGraphPath>>
}

export function getKnowledgeGraphSnapshot(knowledgeBaseId: string, limit = 120) {
  return callTool('get_knowledge_graph_snapshot', {
    knowledge_base_id: knowledgeBaseId,
    limit,
  }) as unknown as Promise<ToolEnvelope<KnowledgeGraphSnapshot>>
}

export function lintBookKnowledgeBase(knowledgeBaseId: string) {
  return callTool('lint_book_knowledge_base', {
    knowledge_base_id: knowledgeBaseId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeHealthReport>>
}

export function askBookKnowledge(knowledgeBaseId: string, question: string, conversationId?: string) {
  return callTool('ask_book_knowledge', {
    knowledge_base_id: knowledgeBaseId,
    question,
    ...(conversationId ? { conversation_id: conversationId } : {}),
  }, { timeout: 190_000 }) as unknown as Promise<ToolEnvelope<KnowledgeAnswer>>
}

export async function streamBookKnowledge(
  knowledgeBaseId: string,
  question: string,
  conversationId: string | undefined,
  onEvent: (event: KnowledgeChatStreamEvent) => void,
  signal?: AbortSignal,
) {
  const response = await fetch('/v1/knowledge/chat/stream', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'text/event-stream' },
    body: JSON.stringify({
      knowledge_base_id: knowledgeBaseId,
      question,
      ...(conversationId ? { conversation_id: conversationId } : {}),
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
  let buffer = ''
  let completed: KnowledgeAnswer | undefined

  const consumeBlock = (block: string) => {
    const data = block
      .split('\n')
      .filter(line => line.startsWith('data:'))
      .map(line => line.slice(5).trimStart())
      .join('\n')
    if (!data) return
    const event = JSON.parse(data) as KnowledgeChatStreamEvent
    onEvent(event)
    if (event.type === 'completed') completed = event.result
    if (event.type === 'error') throw new Error(event.message)
  }

  while (true) {
    const { value, done } = await reader.read()
    buffer += decoder.decode(value, { stream: !done }).replace(/\r\n/g, '\n')
    let boundary = buffer.indexOf('\n\n')
    while (boundary >= 0) {
      consumeBlock(buffer.slice(0, boundary))
      buffer = buffer.slice(boundary + 2)
      boundary = buffer.indexOf('\n\n')
    }
    if (done) break
  }
  if (buffer.trim()) consumeBlock(buffer)
  if (!completed) throw new Error('问答流在完成前意外关闭')
  return completed
}

export function saveKnowledgeAnswer(knowledgeBaseId: string, runId: string) {
  return callTool('save_knowledge_answer', {
    knowledge_base_id: knowledgeBaseId,
    run_id: runId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeChangeSet>>
}

export function listKnowledgeConversations(knowledgeBaseId: string, limit = 30) {
  return callTool('list_knowledge_conversations', {
    knowledge_base_id: knowledgeBaseId,
    limit,
  }) as unknown as Promise<ToolEnvelope<{ conversations: KnowledgeConversationSummary[] }>>
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
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function executeKnowledgeTask(taskId: string) {
  return callTool('execute_knowledge_task', {
    task_id: taskId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTask>>
}

export function getKnowledgeTaskResult(taskId: string) {
  return callTool('get_knowledge_task_result', {
    task_id: taskId,
  }) as unknown as Promise<ToolEnvelope<KnowledgeTaskExecution>>
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
    ToolEnvelope<{ runtime_profiles: RuntimeHealth[]; documents: ConfigDocument[] }>
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

export function listKnowledgeBackups() {
  return callTool('list_knowledge_backups') as unknown as Promise<
    ToolEnvelope<{ backups: DatabaseBackup[]; retention: number }>
  >
}

export function createKnowledgeBackup(reason = 'manual') {
  return callTool('create_knowledge_backup', { reason }) as unknown as Promise<
    ToolEnvelope<DatabaseBackup>
  >
}

export function restoreKnowledgeBackup(filename: string, confirmation: string) {
  return callTool('restore_knowledge_backup', { filename, confirmation }, {
    timeout: 10 * 60_000,
  }) as unknown as Promise<ToolEnvelope<{ validation: DatabaseValidationReport }>>
}

export function knowledgeBackupDownloadUrl(filename: string) {
  return `/v1/knowledge/backups/${encodeURIComponent(filename)}`
}

export function bookWikiExportDownloadUrl(
  knowledgeBaseId: string,
  format: 'json' | 'markdown',
) {
  return `/v1/knowledge/bases/${encodeURIComponent(knowledgeBaseId)}/export/${format}`
}

export function uploadAndRestoreKnowledgeBackup(file: File, confirmation: string) {
  const form = new FormData()
  form.append('backup', file)
  form.append('confirmation', confirmation)
  return api.post('/knowledge/backups/restore/upload', form, {
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 10 * 60_000,
  }) as unknown as Promise<{ validation: DatabaseValidationReport }>
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
    provider_config: profile.provider_config ?? null,
    enabled: profile.enabled,
    expected_revision: profile.revision,
  }) as unknown as Promise<ToolEnvelope<RuntimeProfile>>
}


export function verifyAgentRuntime(profileId: string) {
  return callTool('verify_agent_runtime', {
    profile_id: profileId,
  }, { timeout: 190_000 }) as unknown as Promise<ToolEnvelope<RuntimeVerification>>
}
