/**
 * Isolated browser acceptance: no production API, SQLite, credentials, or shelf access.
 * Run: PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node scripts/verify-wiki-reliability.mjs
 * If Playwright is installed locally, the environment override is unnecessary.
 * Optional SCREENSHOT_DIR selects the generated-artifact directory.
 * WIKI_MOBILE_LAYOUT_ONLY=1 checks all five pages and settings sections at 320/390/667/1440px.
 */
import assert from 'node:assert/strict'
import { mkdtemp, mkdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const startedAt = Date.now()
const mobileLayoutOnly = process.env.WIKI_MOBILE_LAYOUT_ONLY === '1'
const frontendRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const screenshotDir = process.env.SCREENSHOT_DIR || await mkdtemp(join(tmpdir(), 'wiki-reliability-'))
await mkdir(screenshotDir, { recursive: true })
const timestamp = '2026-09-26T00:00:00Z'
const entry = { id: 'mock-entry', knowledge_base_id: 'mock-base', entry_type: 'concept', slug: 'mechanism', title: '历史引用实体', summary: '编译后的机制', status: 'verified', source_path: 'chapter.md', updated_at: timestamp }
const staleEntry = { ...entry, id: 'mock-stale', slug: 'expired', title: '依据已变化的知识主题', status: 'stale' }
const base = { id: 'mock-base', book_id: 'mock-book', book_name: '隔离测试书籍', book_path: '/mock/not-a-real-book', book_kind: 'folder', lifecycle: 'active', source_available: true, entry_count: 3, sync_state: 'clean', compile_mode: 'smart', compile_state: 'ready', compile_phase: 'completed' }
if (mobileLayoutOnly) Object.assign(base, { book_name: '很长的书籍名称：跨章节知识机制与适用边界完整研究', book_path: `/mock/${'long-folder-without-spaces/'.repeat(8)}book`, source_count: 42, claim_count: 128, task_count: 5, health_state: 'healthy' })
const provider = { provider_id: 'mock-provider', display_name: '隔离模型供应商', api_protocol: 'openai-completions', base_url: 'https://never-contact.example/v1', model: 'mock-model', credential_source: 'environment', api_key_env: 'MOCK_NOT_A_REAL_SECRET', api_key_configured: false, context_window: null, max_output_tokens: null, reasoning_policy: 'auto', enabled: true, revision: 1, updated_at: timestamp }
if (mobileLayoutOnly) Object.assign(provider, { display_name: '供应商长名称：兼容多种模型与上下文容量的隔离测试', base_url: `https://never-contact.example/${'long-api-path/'.repeat(8)}v1`, api_key_env: 'A_VERY_LONG_ENVIRONMENT_VARIABLE_NAME_WITHOUT_WHITESPACE', model: 'long-model-name-without-whitespace-'.repeat(3) })
const layoutSkill = { id: 'mock-skill', slug: 'long-skill-slug-without-whitespace'.repeat(3), name: '基于书籍证据的完整知识问答与研究工作流', description: '按当前证据范围研究并保留版本与引用，不修改原书。', source_type: 'builtin', status: 'ready', permissions: ['read_knowledge'], requirements: [], revision: 1, instructions: '# 隔离测试 Skill\n\n完整指令内容。\n'.repeat(40), enabled: true, usage_scope: 'both', updated_at: timestamp }
const state = { provider: { ...provider }, providerSaves: [], chatAttempts: 0, chatRequests: [], snapshotReads: [], currentReads: 0, fallbackSearches: 0, reportReads: [], reportReviewMode: false, archiveProposal: null, archiveRequests: [], archiveResolutions: [], reviewRetries: [], reviewRetryReportReads: 0, researchStatus: 'failed', researchQueuePolls: 0, researchHold: false, researchExecutions: [], workspaceReads: [], stageReads: [], stageDelay: 0, unsupported: [] }
const researchTask = { id: 'mock-research', knowledge_base_id: base.id, book_name: base.book_name, title: '跨主题研究与失败恢复', description: '隔离测试：保存机制、条件、历史基线和完整报告', task_type: 'research', deliverable_type: 'presentation', artifact_state: 'failed', knowledge_change_state: 'none', status: 'failed', result_summary: '演示阶段未完成；报告仍保留', external_research_enabled: false, external_domains: [], external_request_limit: 0, external_requests_used: 0, created_at: timestamp, updated_at: timestamp }
const runningTask = { ...researchTask, id: 'mock-running', title: '仍在执行的另一个研究', status: 'running', deliverable_type: 'report', artifact_state: 'none' }
const researchPlan = { goal: '比较机制、公式与适用边界', constraints: ['不新增书外事实'], acceptance: ['保留公式与条件差异'], depth: 'deep', terminology: ['使用统一术语但保留不同条件'], questions: [{ id: 'mechanism', title: '机制与公式', question: '机制和完整公式是什么？', required_evidence: ['公式与变量含义'] }, { id: 'boundary', title: '边界与反例', question: '哪些条件下不成立？', required_evidence: ['反例与适用条件'] }] }
const researchStages = [
  { stage_key: 'plan', title: '研究目标规划', kind: 'plan', status: 'completed', run_id: 'research-plan', revision: 2 },
  { stage_key: 'section:mechanism', title: '机制与公式', kind: 'section', status: 'completed', run_id: 'research-mechanism', revision: 2, summary: '完整公式已保存' },
  { stage_key: 'section:boundary', title: '边界与反例', kind: 'section', status: 'failed', run_id: 'research-boundary-new', revision: 3, error: '仍缺少反例证据，不宣称核验完成' },
  { stage_key: 'synthesis', title: '综合结论与交叉核验', kind: 'synthesis', status: 'stale', run_id: 'research-synthesis-new', revision: 3, error: '综合所读依据变化，原章节仍保留' },
  { stage_key: 'report', title: '完整报告', kind: 'report', status: 'stale', run_id: 'research-report-new', revision: 3, error: '依据变化；旧报告与原始引用保留' },
  { stage_key: 'validation', title: '引用与结构校验', kind: 'validation', status: 'pending', revision: 1 },
  { stage_key: 'presentation', title: '演示交付', kind: 'presentation', status: 'failed', run_id: 'research-ppt', revision: 2, error: '演示策划尚未完成，完整报告保留' },
].map((stage, ordinal) => ({ summary: '', content_characters: 0, finding_count: 0, updated_at: timestamp, ...stage, ordinal }))
const savedResearchBody = `## 已保存的完整成果\n\n${'机制、步骤与条件保持完整，不用篇幅冒充事实证明。\n\n'.repeat(80)}$$d_k=d_v=128$$\n\n报告尾部条件与反例完整保留。[S1]`
function researchWorkspace(taskId) {
  return { task_id: taskId, knowledge_base_id: base.id, original_request: {}, plan: researchPlan, stages: researchStages, baselines: [{ question_id: 'boundary', entry_id: entry.id, revision: 2, title: entry.title, status: 'verified', claim_count: 30, content_characters: 32000, captured_at: timestamp }], created_at: timestamp, updated_at: timestamp }
}
function researchStage(args) {
  const stage = researchStages.find(stage => stage.stage_key === args.stage_key)
  assert.ok(stage, 'fixture accepts only actual research phases')
  const historical = args.revision === 1
  const contentRun = args.stage_key === 'validation' ? null : historical ? 'research-mechanism-old' : args.stage_key === 'report' ? 'research-report-old' : args.stage_key === 'synthesis' ? 'research-synthesis-old' : stage.run_id
  return { stage: historical ? { ...stage, status: 'completed', revision: 1, run_id: 'research-mechanism-old' } : stage, content_run_id: contentRun,
    content_md: args.stage_key === 'validation' ? '' : historical ? '## 保存的旧版主题\n\n旧版公式与限制。[S1]' : args.stage_key === 'synthesis' ? '## 跨章节综合判断\n\n不同版本和条件分别成立，不强行合并。\n\n### 跨章节对照记录\n\n这是模型的公开对照判断，不是独立事实证明。' : savedResearchBody,
    findings: args.stage_key === 'validation' ? [] : [{ finding: '机制已有依据，条件尚需复核', status: 'partial', citation_indices: args.stage_key === 'synthesis' ? [] : [1], limitations: ['缺少对称的反例，不能得出无条件结论'], baseline_entry_id: args.stage_key === 'synthesis' ? null : entry.id, baseline_claim_id: args.stage_key === 'synthesis' ? null : 'real-baseline-claim' }],
    evidence: args.stage_key === 'validation' || args.stage_key === 'synthesis' ? [] : [{ citation_index: 1, kind: 'entry', object_id: entry.id, version_id: '2', run_id: contentRun }] }
}
const reportFragments = Array.from({ length: 31 }, (_, index) => ({ ordinal: index, batch: index < 25 ? 1 : 2, source_document_id: 'mock-doc', source_version_id: 'mock-v2', source_span_id: `mock-span-${index}`, source_path: `${'很长的路径/'.repeat(8)}chapter-${index}.md`, line_start: index * 10 + 1, line_end: index * 10 + 10, locator: { heading_path: ['完整主题', `机制与条件 ${index}`] }, status: index < 4 ? 'no_material' : index < 25 ? 'analyzed' : index < 27 ? 'failed' : 'unprocessed', run_id: 'mock-run', candidate_slugs: index >= 4 && index < 25 ? ['paged-attention'] : [], reason: index < 4 ? '纯导航，没有新增事实' : null }))
const reportTopics = Array.from({ length: 25 }, (_, index) => ({ slug: `topic-${index}`, title: `归并主题 ${index}`, outcome: 'reconciled_topic', preserved_claim_count: 5, retired_claim_count: index === 0 ? 2 : 0, body_coverage: 'model_reported_not_independent_fact_verification', conflicts: index === 0 ? [{ claim_indices: [0, 1], reason: '同一测试条件的数字不同，保留分歧等待审核' }] : [] }))
function compileReport(args) {
  const older = args.report_id === 'mock-report-old'
  const fragments = older ? reportFragments.slice(0, 1) : reportFragments
  const topics = older ? [] : reportTopics
  const offset = args.fragment_offset || 0, topicOffset = args.topic_offset || 0, limit = args.limit || 20
  const report = { id: older ? 'mock-report-old' : 'mock-report', knowledge_base_id: base.id, status: older ? 'no_material' : 'failed', created_at: timestamp, updated_at: timestamp, selected_sources: 1, current_sources: 8, selected_spans: 31, planned: true, fragment_total: fragments.length, analyzed_fragments: older ? 1 : 25, no_material_fragments: older ? 1 : 4, failed_fragments: older ? 0 : 2, unprocessed_fragments: older ? 0 : 4, fragment_offset: offset, fragment_has_more: offset + limit < fragments.length, fragments: fragments.slice(offset, offset + limit), topic_total: topics.length, topic_offset: topicOffset, topic_has_more: topicOffset + limit < topics.length, topics: topics.slice(topicOffset, topicOffset + limit), change_set_id: null, error: older ? null : '第二批未完成；前批分析结果保留，未覆盖正式知识', previous_report_id: older ? null : 'mock-report-old', next_report_id: older ? 'mock-report' : null }
  if (!older && state.reportReviewMode) Object.assign(report, { status: 'waiting_review', change_set_id: 'mock-change-report', error: null, analyzed_fragments: 31, failed_fragments: 0, unprocessed_fragments: 0, fragments: report.fragments.map(fragment => fragment.ordinal >= 25 ? { ...fragment, status: 'analyzed' } : fragment) })
  return report
}

function json(response, value) { response.setHeader('Content-Type', 'application/json'); response.end(JSON.stringify(value)) }
function success(tool, result) { return { tool, status: 'success', result } }
function runtimeSettings() {
  return { runtime_profiles: [{ available: true, version: 'mock-harness', message: '隔离启动器', profile: { id: 'mock-runtime', name: 'DeepSeek Harness', runtime: 'deepseek_harness', executable: 'never-executed', model: '', provider_id: provider.provider_id, enabled: true, revision: 1, updated_at: timestamp } }], model_providers: [state.provider], documents: mobileLayoutOnly ? ['WIKI.md', 'AGENTS.md', 'QUERY.md', 'RESEARCH.md'].map(name => ({ id: name, name, scope: 'book', revision: 1, content_md: '# 隔离规则\n\n规则内容。\n'.repeat(30) })) : [] }
}
const interruptedBody = '故障前收到的完整片段。'.repeat(120)
const completedAnswer = '## 本轮回答\n\n失败后仍然连接模型，引用本轮读取的历史证据。[S1]'
const conversation = { id: 'mock-conversation', knowledge_base_id: base.id, title: '隔离历史会话', message_count: 2, preview: '本轮回答', created_at: timestamp, updated_at: timestamp }

const mockApi = {
  name: 'isolated-wiki-acceptance-api',
  configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.url?.startsWith('/v1')) return next()
      // Every /v1 request terminates here. There is no real backend proxy.
      if (request.url === '/v1/health') return json(response, { status: 'healthy', version: 'mock', components: {} })
      let body = ''
      for await (const chunk of request) body += chunk
      const payload = body ? JSON.parse(body) : {}
      if (request.url === '/v1/knowledge/chat/stream') {
        state.chatAttempts += 1
        state.chatRequests.push(payload)
        const runId = `mock-run-${state.chatAttempts}`
        response.setHeader('Content-Type', 'text/event-stream')
        response.setHeader('Cache-Control', 'no-cache')
        const event = value => response.write(`data: ${JSON.stringify(value)}\n\n`)
        event({ type: 'evidence', evidence: [entry] })
        event({ type: 'run_started', run_id: runId })
        if (state.chatAttempts === 1) {
          event({ type: 'text_delta', run_id: runId, delta: interruptedBody })
          setTimeout(() => { event({ type: 'error', message: 'DeepSeek Harness max_tokens: output incomplete' }); response.end() }, 100)
        } else {
          const answer = completedAnswer
          event({ type: 'text_delta', run_id: runId, delta: answer })
          setTimeout(() => { event({ type: 'completed', result: { run_id: runId, conversation_id: 'mock-conversation', answer, runtime: 'deepseek_harness', evidence: [entry] } }); response.end() }, 100)
        }
        return
      }
      if (request.url !== '/v1/tools/call') { state.unsupported.push(request.url); response.statusCode = 501; return json(response, { error: 'Mock API only; production requests forbidden' }) }
      const { tool, arguments: args = {} } = payload
      switch (tool) {
        case 'list_knowledge_tasks': {
          if (state.researchStatus === 'queued' && ++state.researchQueuePolls >= 3 && !state.researchHold) state.researchStatus = 'completed'
          return json(response, success(tool, { tasks: [{ ...researchTask, status: state.researchStatus }, runningTask] }))
        }
        case 'get_knowledge_research_workspace': state.workspaceReads.push(args); return json(response, success(tool, researchWorkspace(args.task_id)))
        case 'get_knowledge_research_stage': {
          state.stageReads.push(args)
          const result = researchStage(args)
          if (state.stageDelay) return setTimeout(() => json(response, success(tool, result)), state.stageDelay)
          return json(response, success(tool, result))
        }
        case 'get_knowledge_task_result': return json(response, success(tool, { task: { ...researchTask, status: state.researchStatus, result_summary: savedResearchBody }, run_id: 'research-report-old', evidence: [entry], artifacts: [] }))
        case 'get_knowledge_task_activity': return json(response, success(tool, { run: { id: 'research-ppt', status: 'failed' }, events: [] }))
        case 'execute_knowledge_task': state.researchExecutions.push(args); state.researchStatus = 'queued'; state.researchQueuePolls = 0; return json(response, success(tool, { ...researchTask, status: 'queued' }))
        case 'retry_knowledge_source_review': state.reviewRetries.push(args); return json(response, success(tool, { knowledge_base: { ...base, compile_state: 'compiling' }, queued: true, message: '来源复核已进入后台队列；真实变更仍需审核' }))
        case 'get_book_knowledge_base': return json(response, success(tool, { ...base, compile_state: state.reviewRetries.length ? 'compiling' : base.compile_state }))
        case 'propose_knowledge_entry_archive': {
          state.archiveRequests.push(args)
          state.archiveProposal = { id: 'mock-archive', knowledge_base_id: base.id, title: '历史归档：依据已变化的知识主题', reason: '仅归档并保留历史，不代表事实校验通过', risk_level: 'high', status: 'proposed', classification_summary: { new: 0, update: 1, disputed: 0 }, citation_audit: { passed: true, historical_archive: true, entry_citations: 0, claim_citations: 0, issues: [] }, impact_summary: { entries: 1, claims: 1, relations: 1, citations: 1 }, changes: [{ id: 'archive-change', ordinal: 0, operation: 'archive', object_type: 'entry', object_id: staleEntry.id, expected_revision: 3, classification: 'update', citation_audit: { inherited_claims: 0 }, impact: { claims: 1, relations: 1 }, before: { title: staleEntry.title, status: 'stale', content_md: '历史正文保留' }, after: { title: staleEntry.title, entry_type: 'concept', status: 'archived', content_md: '历史正文保留', citations: ['historical-span'] } }] }
          return json(response, success(tool, state.archiveProposal))
        }
        case 'resolve_knowledge_change_set': state.archiveResolutions.push(args); state.archiveProposal = null; return json(response, success(tool, { id: args.change_set_id, status: args.decision === 'approve' ? 'applied' : 'rejected' }))
        case 'get_knowledge_compile_report': {
          state.reportReads.push(args)
          const report = compileReport(args)
          if (state.reviewRetries.length && ++state.reviewRetryReportReads >= 2) Object.assign(report, { id: 'mock-new-review-report', status: 'running', error: null })
          return json(response, success(tool, { report }))
        }
        case 'list_book_knowledge_bases': return json(response, success(tool, { items: [{ book: { id: base.book_id, kind: 'folder', name: base.book_name, path: base.book_path }, knowledge_base: base }] }))
        case 'get_book_wiki_settings': return json(response, success(tool, runtimeSettings()))
        case 'list_wiki_skills': return json(response, success(tool, { skills: mobileLayoutOnly ? [layoutSkill] : [] }))
        case 'get_wiki_skill_detail': return json(response, success(tool, { skill: layoutSkill, current_version_id: 'mock-skill-v1', versions: [{ id: 'mock-skill-v1', revision: 1, release_state: 'published', content_hash: 'a'.repeat(64), changelog: '完整的版本与来源说明', created_at: timestamp, files: [{ relative_path: 'SKILL.md', content_text: layoutSkill.instructions, content_hash: 'a'.repeat(64), media_type: 'text/markdown', size_bytes: 2048 }] }] }))
        case 'list_knowledge_backups': return json(response, success(tool, { backups: [{ filename: 'very-long-backup-file-name-for-responsive-ui.sqlite', reason: 'manual', created_at: timestamp, size_bytes: 1024000 }], retention: 7 }))
        case 'get_agent_usage_stats': return json(response, success(tool, { start_date: args.start_date, end_date: args.end_date, usage_source: 'unavailable', totals: { runs: 0, unreported_runs: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0 }, daily: [], by_caller: [] }))
        case 'save_model_provider': state.providerSaves.push(args); state.provider = { ...state.provider, ...args, revision: state.provider.revision + 1 }; return json(response, success(tool, state.provider))
        case 'list_knowledge_conversations': return json(response, success(tool, { conversations: state.chatAttempts >= 2 ? [conversation] : [] }))
        case 'get_knowledge_conversation': return json(response, success(tool, { ...conversation, messages: [{ id: 'persisted-user', role: 'user', content: '第二次：重新提问', evidence: [], created_at: timestamp }, { id: 'persisted-assistant', role: 'assistant', content: completedAnswer, run_id: 'mock-run-2', evidence: [entry], created_at: timestamp }] }))
        case 'get_agent_run_citation': state.snapshotReads.push(args); return json(response, success(tool, { run_id: args.run_id, citation_index: args.source_index + 1, kind: 'entry', object_id: entry.id, version_id: 'mock-revision-2', entry, content_md: '## 历史快照正文\n\n这是运行当时读取的 version-2；当前实体已改为 version-3。', citations: [], historical: true, read_ranges: [{ offset_chars: 0, returned_chars: 100 }] }))
        case 'get_knowledge_entry': state.currentReads += 1; return json(response, success(tool, { ...(args.entry_id === staleEntry.id ? staleEntry : entry), content_md: '## 保留的历史知识正文\n\n当前 version-3（不应偷偷替换历史快照）', citations: [], aliases: [], revision: 3, claims: [], relations: [], versions: [], source_impact_count: args.entry_id === staleEntry.id ? 1 : 0, source_impacts: args.entry_id === staleEntry.id ? [{ source_document_id: 'mock-doc', source_path: `${'长来源路径/'.repeat(10)}chapter.md`, previous_version_id: 'v1', current_version_id: 'v2', reason: 'source_changed', affected_via_entry_id: null, affected_via_entry_title: null, detected_at: timestamp }] : [] }))
        case 'list_knowledge_change_sets': return json(response, success(tool, { change_sets: state.archiveProposal ? [state.archiveProposal] : state.reportReviewMode ? [{ id: 'mock-change-report', knowledge_base_id: base.id, title: '报告关联审核', reason: '完整归并候选', risk_level: 'high', status: 'proposed', classification_summary: { new: 1, update: 0, disputed: 0 }, citation_audit: { passed: true, entry_citations: 1, claim_citations: 1, issues: [] }, impact_summary: { entries: 1, claims: 1, relations: 0, citations: 1 }, changes: [] }] : [] }))
        case 'lint_book_knowledge_base': return json(response, success(tool, { knowledge_base_id: base.id, state: 'warning', semantic_entry_count: 1, source_span_count: 1, pending_review_count: 0, issues: [{ code: 'expired-source-evidence', severity: 'warning', title: '1 个主题的来源或依赖需要复核', detail: '历史正文仍保留，不能作为当前有效知识。', object_ids: [staleEntry.id] }], generated_at: timestamp }))
        case 'get_agent_run_inspection': {
          if (args.run_id.startsWith('research-')) return json(response, success(tool, {
            run: { id: args.run_id, task_type: 'knowledge_task_research', status: 'completed', runtime: 'deepseek_harness', input: {}, created_at: timestamp, started_at: timestamp }, events: [], evidence: [],
            snapshot: { run_id: args.run_id, tool_names: [], skill_snapshots: [], config_snapshots: [], prompt_text: '公开业务目标，不包含隐藏推理', evidence_refs: {
              research_stage_key: args.run_id === 'research-ppt' ? 'presentation' : 'section:boundary', research_plan: researchPlan, research_question: researchPlan.questions[1],
              research_resources: { capacity_tokens: 32768, capacity_basis: 'unknown_model_application_guard', initial_entry_target: 14, policy: { timeout_seconds: 480 } },
              adaptive_state: { used_tool_calls: 5, soft_tool_calls: 18, estimated_tool_payload_tokens: 4000, soft_retrieval_tokens: 8000, policy: { hard_tool_calls: 72, hard_retrieval_tokens: 32000 }, coverage: [{ question_index: 0, question: '反例与适用条件', status: 'partial', citation_indices: [1], finding: '尚缺反例，不把局部依据当全书结论' }] },
              ...(args.run_id === 'research-ppt' ? { presentation_materialization: { mode: 'whole_structure_projection', report_characters: 95000, omitted_characters: 71000, section_count: 10, estimated_material_tokens: 8000, billing_usage: false } } : {}),
            } },
          }))
          return json(response, success(tool, {
          run: { id: args.run_id, task_type: 'knowledge_qa', status: 'completed', runtime: 'deepseek_harness', input: {}, created_at: timestamp, started_at: timestamp },
          events: [{ event_type: 'run.budget_changed', payload: { reason: '需要核对后半书的条件差异' }, message: '扩大取证范围' }], evidence: [],
          snapshot: { run_id: args.run_id, tool_names: [], skill_snapshots: [], config_snapshots: [], prompt_text: '', evidence_refs: {
            qa_plan: { goal: '对比全书的优化机制及适用边界', constraints: ['保留公式和单位'], subquestions: ['机制', '边界'], depth: 'comprehensive' },
            planning_stats: { catalog_seen: 120, catalog_total: 120, elapsed_ms: 1200, time_limit_seconds: 180 },
            adaptive_state: { used_tool_calls: 14, soft_tool_calls: 36, extension_count: 1, estimated_tool_payload_tokens: 14000, soft_retrieval_tokens: 32000, policy: { hard_tool_calls: 120, hard_retrieval_tokens: 64000 }, coverage: [{ question_index: 0, question: '机制', status: 'supported', citation_indices: [1], finding: '从编译知识正文核对机制' }, { question_index: 1, question: '不同版本中的适用边界、条件与例外是否存在冲突', status: 'partial', citation_indices: [1], finding: '仍需核对后半书的具体条件，不冒称全书已覆盖' }] },
          } },
          }))
        }
        case 'list_knowledge_entries': state.fallbackSearches += 1; return json(response, success(tool, { entries: args.include_stale ? [entry, staleEntry] : [entry], offset: 0, limit: args.limit || 6, total: args.include_stale ? 2 : 1, has_more: false }))
        default: state.unsupported.push(tool); response.statusCode = 501; return json(response, { tool, status: 'error', error: { code: 'MOCK_ONLY', message: `Unimplemented mock tool: ${tool}` } })
      }
    })
  },
}
const loadedConfig = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(frontendRoot, 'vite.config.ts'))
assert.ok(loadedConfig, 'Vite config must load')
const server = await createServer({ ...loadedConfig.config, root: frontendRoot, configFile: false, logLevel: 'error', plugins: [mockApi, ...loadedConfig.config.plugins], server: { host: '127.0.0.1', port: 0, strictPort: false, proxy: {} } })
let browser
const results = []
try {
  await server.listen()
  const address = server.httpServer.address()
  assert.ok(address && typeof address === 'object')
  const origin = `http://127.0.0.1:${address.port}`
  // Use the Chromium channel's headless mode, not the separately installed headless-shell package.
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  const viewports = mobileLayoutOnly ? [{ width: 320, height: 568 }, { width: 390, height: 844 }, { width: 667, height: 375 }, { width: 1440, height: 1000 }] : [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]
  for (const viewport of viewports) {
    const mode = mobileLayoutOnly ? `${viewport.width}x${viewport.height}` : viewport.width > 768 ? 'desktop' : 'phone'
    const phone = viewport.width <= 768
    state.chatAttempts = 0
    state.chatRequests = []
    state.provider = { ...provider }
    state.providerSaves = []
    state.snapshotReads = []
    state.currentReads = 0
    state.fallbackSearches = 0
    state.reportReviewMode = false
    state.archiveProposal = null
    state.archiveRequests = []
    state.archiveResolutions = []
    state.reviewRetries = []
    state.reviewRetryReportReads = 0
    state.researchStatus = 'failed'; state.researchQueuePolls = 0; state.researchHold = false; state.researchExecutions = []; state.workspaceReads = []; state.stageReads = []; state.stageDelay = 0
    const context = await browser.newContext({ viewport, reducedMotion: 'reduce' })
    await context.route('**/*', route => {
      const url = route.request().url()
      if (url.startsWith('blob:') || url.startsWith('data:') || new URL(url).origin === origin) return route.continue()
      return route.abort('blockedbyclient')
    })
    const page = await context.newPage()
    const pageErrors = []
    page.on('pageerror', error => pageErrors.push(error.message))
    async function checkLayout(label, dialog) {
      if (dialog) await page.waitForFunction(() => !document.querySelector('.motion-modal-enter-active'))
      const data = await page.evaluate(() => ({ width: window.innerWidth, height: window.innerHeight, scrollWidth: document.documentElement.scrollWidth }))
      assert.ok(data.scrollWidth <= data.width + 1, `${label}: horizontal overflow ${JSON.stringify(data)}`)
      if (dialog) {
        const rect = await dialog.boundingBox()
        assert.ok(rect && rect.x >= -1 && rect.x + rect.width <= data.width + 1 && rect.y >= -1 && rect.y + rect.height <= data.height + 1, `${label}: dialog clipped ${JSON.stringify(rect)}`)
      }
      await page.screenshot({ path: join(screenshotDir, `${mode}-${label}.png`), animations: 'disabled' })
    }
    async function selectSettings(label) {
      if (phone) {
        await page.locator('.mobile-settings-section').click()
        await page.getByRole('option', { name: new RegExp(`^${label}`) }).click()
      } else await page.locator('.settings-nav').getByRole('button', { name: new RegExp(label) }).click()
    }
    async function selectStage(dialog, title) {
      if (phone && await dialog.locator('.research-stage-directory').getAttribute('open') === null) await dialog.locator('.research-stage-directory > summary').click()
      await dialog.locator('.research-stages button').filter({ hasText: title }).click()
    }
    if (mobileLayoutOnly) {
      await page.goto(`${origin}/knowledge`)
      await page.locator('.book-wiki-card').waitFor()
      const tabs = await page.locator('.knowledge-tabs a').all()
      assert.equal(tabs.length, 5)
      for (const tab of tabs) {
        const box = await tab.boundingBox()
        assert.ok(box && box.x >= 0 && box.x + box.width <= viewport.width, 'all Wiki destinations are visible without swiping')
      }
      if (phone) {
        const card = await page.locator('.book-wiki-card').boundingBox()
        const stats = await page.locator('.book-stats').boundingBox()
        assert.ok(stats.width >= card.width - 32, 'book statistics use the complete card width')
        await page.getByRole('button', { name: '知识库操作', exact: true }).click()
        const actions = page.getByRole('dialog', { name: '知识库操作', exact: true })
        await checkLayout('book-actions', actions)
        await actions.getByRole('button', { name: '管理知识库', exact: true }).click()
        const manage = page.getByRole('dialog', { name: '管理知识库', exact: true })
        await checkLayout('book-manage', manage)
        await manage.getByRole('button', { name: '完成', exact: true }).click()
        await manage.waitFor({ state: 'hidden' })
      }
      await page.locator('.app-main').evaluate(main => { main.scrollTop = 0 })
      await checkLayout('knowledge-bases')
      await page.goto(`${origin}/knowledge/wiki?base=${base.id}`)
      await page.locator('.entry-row').first().click()
      await page.locator('.entity-markdown h2').waitFor()
      if (phone) {
        assert.equal(await page.locator('.wiki-toolbar .knowledge-search').isVisible(), false)
        await page.getByRole('button', { name: '返回实体列表' }).click()
        assert.equal(await page.locator('.wiki-toolbar .knowledge-search').isVisible(), true)
        await page.locator('.entry-row').first().click()
      }
      await page.locator('.detail-loading').waitFor({ state: 'hidden' })
      await page.locator('.entity-markdown h2').waitFor()
      await checkLayout('wiki-detail')
      await page.goto(`${origin}/knowledge/chat`)
      await page.locator('.chat-composer textarea').waitFor()
      if (phone) {
        const body = await page.locator('.message-list').boundingBox()
        const chat = await page.locator('.chat-layout').boundingBox()
        const composer = await page.locator('.chat-composer').boundingBox()
        const dock = await page.locator('.mobile-dock').boundingBox()
        assert.ok(body.height >= Math.min(220, chat.height * .5), `usable answer area ${JSON.stringify({ body, chat })}`)
        assert.ok(composer.y + composer.height <= dock.y, 'composer is not behind the dock')
        assert.ok(await page.locator('.chat-composer textarea').evaluate(input => Number.parseFloat(getComputedStyle(input).fontSize)) >= 16, 'composer avoids mobile input zoom')
        await page.getByRole('button', { name: '问答历史与设置' }).filter({ visible: true }).click()
        const history = page.getByRole('dialog', { name: '问答历史与设置' })
        await checkLayout('chat-history', history)
        await history.getByRole('button', { name: '新对话', exact: true }).click()
      }
      await checkLayout('chat')
      await page.goto(`${origin}/knowledge/tasks`)
      await page.locator('.research-task').first().waitFor()
      if (phone) {
        const card = await page.locator('.research-task').first().boundingBox()
        const main = await page.locator('.task-main').first().boundingBox()
        assert.ok(main.width >= card.width - 32, 'research body uses full width')
      }
      await checkLayout('research-tasks')
      await page.locator('.task-stage-link').first().click()
      const result = page.getByRole('dialog', { name: '研究任务结果' })
      await result.locator('.research-stages button').first().waitFor()
      await selectStage(result, '机制与公式')
      await result.locator('.selected-stage').waitFor()
      if (phone) assert.equal(await result.locator('.research-stage-directory').getAttribute('open'), null, 'selecting a stage focuses the saved body')
      await checkLayout('research-stage', result)
      await result.getByRole('button', { name: '完成', exact: true }).click()
      await page.goto(`${origin}/knowledge/settings`)
      await page.locator('.runtime-card').waitFor()
      await checkLayout('settings-runtime')
      for (const [label, name] of [['模型供应商', 'providers'], ['Token 用量', 'usage'], ['数据保护', 'protection'], ['配置文档', 'documents'], ['Skills', 'skills']]) {
        await selectSettings(label)
        await page.locator('.settings-section-head h2').filter({ hasText: label }).waitFor()
        await checkLayout(`settings-${name}`)
        if (name === 'documents' && phone) assert.ok(await page.locator('.document-editor textarea').evaluate(input => Number.parseFloat(getComputedStyle(input).fontSize)) >= 16, 'document editor avoids mobile input zoom')
        if (name === 'providers') {
          await page.getByRole('button', { name: '编辑', exact: true }).click()
          const editor = page.getByRole('dialog', { name: '编辑模型供应商' })
          await checkLayout('provider-editor', editor)
          const cancel = await editor.getByRole('button', { name: '取消', exact: true }).boundingBox()
          assert.ok(cancel.y >= 0 && cancel.y + cancel.height <= viewport.height, 'editor footer is not clipped')
          if (phone && viewport.height > 500) {
            // Emulate the visual viewport shrinking independently of the layout viewport.
            // This is a layout contract, not a replacement for real-device keyboard testing.
            await page.evaluate(() => { Object.defineProperty(window.visualViewport, 'height', { value: 350, configurable: true }); window.visualViewport.dispatchEvent(new Event('resize')) })
            await page.waitForFunction(() => Number.parseFloat(document.querySelector('.motion-modal').style.getPropertyValue('--motion-viewport-height')) === 350)
            const keyboardFooter = await editor.getByRole('button', { name: '取消', exact: true }).boundingBox()
            assert.ok(keyboardFooter.y >= 0 && keyboardFooter.y + keyboardFooter.height <= 350, 'drawer footer fits the keyboard visual viewport')
            await page.evaluate(() => { delete window.visualViewport.height; window.visualViewport.dispatchEvent(new Event('resize')) })
          }
          await editor.getByRole('button', { name: '取消', exact: true }).click()
          await editor.waitFor({ state: 'hidden' })
        }
        if (name === 'usage' && phone) {
          await page.locator('.usage-filters .el-date-editor input').first().click()
          await page.locator('.wiki-usage-picker').waitFor()
          const picker = await page.locator('.wiki-usage-picker').boundingBox()
          assert.ok(picker.x >= -1 && picker.x + picker.width <= viewport.width + 1, 'range picker fits phone')
          await page.keyboard.press('Escape')
        }
        if (name === 'skills') {
          await page.getByRole('button', { name: '查看内容', exact: true }).click()
          const skill = page.getByRole('dialog', { name: 'Skill 内容' })
          await skill.locator('.skill-detail-content > pre').waitFor()
          await checkLayout('skill-body', skill)
          if (phone) {
            assert.equal(await skill.locator('.skill-detail-sidebar').isVisible(), false)
            await skill.getByRole('button', { name: '权限、版本与来源说明' }).click()
            assert.equal(await skill.locator('.skill-detail-sidebar').isVisible(), true)
          }
          await skill.getByRole('button', { name: '完成', exact: true }).click()
        }
      }
      assert.deepEqual(pageErrors, [], `${mode}: runtime errors`)
      results.push({ viewport, fivePages: true, allSettingsSections: true, usableReadingArea: true, reachableActions: true, horizontalOverflow: false, clippedDialogs: false })
      await context.close()
      continue
    }
    await page.goto(`${origin}/knowledge/settings`)
    await selectSettings('模型供应商')
    await page.getByRole('button', { name: '编辑', exact: true }).click()
    const providerDialog = page.getByRole('dialog', { name: '编辑模型供应商' })
    await providerDialog.waitFor()
    // Editing identity is not a pending-request spinner: cancelling must leave Edit usable.
    await providerDialog.getByRole('button', { name: '取消', exact: true }).click()
    await providerDialog.waitFor({ state: 'hidden' })
    await page.getByRole('button', { name: '编辑', exact: true }).click()
    await providerDialog.waitFor()
    await providerDialog.getByLabel('上下文容量（tokens）').fill('1000000')
    await providerDialog.getByLabel('最大输出（tokens）').fill('64000')
    await providerDialog.locator('.provider-capabilities .el-select').click()
    await page.getByRole('option', { name: '高', exact: true }).click()
    await page.getByRole('option', { name: '高', exact: true }).waitFor({ state: 'hidden' })
    await checkLayout('provider-capabilities', providerDialog)
    await providerDialog.getByRole('button', { name: '保存供应商' }).click()
    await providerDialog.waitFor({ state: 'hidden' })
    assert.equal(state.providerSaves[0].context_window, 1000000)
    assert.equal(state.providerSaves[0].max_output_tokens, 64000)
    assert.equal(state.providerSaves[0].reasoning_policy, 'high')
    await page.getByRole('button', { name: '编辑', exact: true }).click()
    await providerDialog.getByLabel('上下文容量（tokens）').fill('')
    await providerDialog.getByLabel('最大输出（tokens）').fill('')
    await providerDialog.locator('.provider-capabilities .el-select').click()
    await page.getByRole('option', { name: '继承 Harness', exact: true }).click()
    await providerDialog.getByRole('button', { name: '保存供应商' }).click()
    await providerDialog.waitFor({ state: 'hidden' })
    assert.equal(state.providerSaves[1].context_window, null)
    assert.equal(state.providerSaves[1].max_output_tokens, null)
    assert.equal(state.providerSaves[1].reasoning_policy, 'auto')

    await page.goto(`${origin}/knowledge/chat`)
    const composer = page.locator('.chat-composer textarea')
    await composer.fill('第一次：模拟截断')
    await page.getByRole('button', { name: '发送问题' }).click()
    await page.locator('.answer-interruption').waitFor()
    await page.waitForFunction(text => document.querySelector('.message.assistant .knowledge-answer-markdown')?.textContent?.includes(text), interruptedBody)
    assert.equal(await page.getByRole('button', { name: '保存到 Wiki', exact: true }).count(), 0)
    await checkLayout('interrupted-answer')
    await page.getByRole('button', { name: '继续完成完整答案', exact: true }).click()
    await page.getByRole('button', { name: '保存到 Wiki', exact: true }).waitFor()
    assert.equal(state.chatAttempts, 2, 'second request must retry Runtime')
    assert.equal(state.chatRequests[1].resume_run_id, 'mock-run-1', 'recovery must explicitly address the truncated run')
    assert.equal(state.chatRequests[1].question, state.chatRequests[0].question, 'recovery must keep the original question')
    assert.equal(state.fallbackSearches, 0, 'runtime failure must not silently replace model output with FTS')
    await page.getByRole('button', { name: '查看本轮目标与取证预算' }).last().click()
    const inspectorDialog = page.getByRole('dialog', { name: '问答运行检查器' })
    await inspectorDialog.getByText('对比全书的优化机制及适用边界', { exact: true }).waitFor()
    await inspectorDialog.getByText('仍需核对后半书的具体条件，不冒称全书已覆盖', { exact: true }).waitFor()
    await checkLayout('adaptive-inspector', inspectorDialog)
    await inspectorDialog.getByRole('button', { name: '关闭', exact: true }).scrollIntoViewIfNeeded()
    await inspectorDialog.getByRole('button', { name: '关闭', exact: true }).click()
    await inspectorDialog.waitFor({ state: 'hidden' })
    await page.locator('.evidence-grid button').last().click()
    const citationDialog = page.getByRole('dialog', { name: '来源预览' })
    await citationDialog.getByText('历史快照正文', { exact: true }).waitFor()
    assert.deepEqual(state.snapshotReads.at(-1), { run_id: 'mock-run-2', source_index: 0 })
    assert.equal(state.currentReads, 0, 'historical citations must not fetch current mutable contents')
    await checkLayout('historical-citation', citationDialog)
    await citationDialog.getByRole('button', { name: '关闭来源预览' }).click()
    await citationDialog.waitFor({ state: 'hidden' })
    await page.reload()
    await page.getByRole('button', { name: '保存到 Wiki', exact: true }).waitFor()
    await page.locator('.evidence-grid button').last().click()
    await citationDialog.getByText('历史快照正文', { exact: true }).waitFor()
    assert.deepEqual(state.snapshotReads.at(-1), { run_id: 'mock-run-2', source_index: 0 })
    assert.equal(state.currentReads, 0, 'restored conversation must preserve historical evidence identity')
    await citationDialog.getByRole('button', { name: '关闭来源预览' }).click()
    await citationDialog.waitFor({ state: 'hidden' })
    await page.goto(`${origin}/knowledge/wiki?base=${base.id}&entry=${staleEntry.id}`)
    await page.getByRole('region', { name: '来源复核提示' }).waitFor()
    await page.getByText('这份知识的依据需要重新复核', { exact: true }).waitFor()
    if (mode === 'phone') {
      await page.getByRole('button', { name: 'Wiki 操作', exact: true }).click()
      await page.getByRole('dialog', { name: 'Wiki 操作' }).getByRole('button', { name: /编译报告/ }).click()
    } else await page.getByRole('button', { name: '编译报告', exact: true }).click()
    const reportDialog = page.getByRole('dialog', { name: 'Wiki 编译覆盖报告' })
    await reportDialog.getByText('25 / 31', { exact: true }).waitFor()
    await reportDialog.getByText('同一测试条件的数字不同，保留分歧等待审核', { exact: false }).waitFor()
    await checkLayout('compile-report', reportDialog)
    await reportDialog.getByRole('navigation', { name: '来源分页' }).getByRole('button', { name: '下一页', exact: true }).click()
    await reportDialog.getByText(/chapter-30.md/).waitFor()
    assert.equal(state.reportReads.at(-1).fragment_offset, 20)
    assert.equal(state.reportReads.at(-1).report_id, 'mock-report')
    await reportDialog.getByRole('navigation', { name: '主题分页' }).getByRole('button', { name: '下一页', exact: true }).click()
    await reportDialog.getByText('归并主题 24', { exact: true }).waitFor()
    assert.equal(state.reportReads.at(-1).topic_offset, 20)
    await checkLayout('compile-report-tail', reportDialog)
    await reportDialog.getByRole('button', { name: '较早报告', exact: true }).click()
    await reportDialog.getByText('分析完成 · 无实质新增', { exact: true }).waitFor()
    assert.equal(state.reportReads.at(-1).report_id, 'mock-report-old')
    const footerBounds = await reportDialog.getByRole('button', { name: '关闭', exact: true }).boundingBox()
    assert.ok(footerBounds && footerBounds.y + footerBounds.height <= viewport.height, 'report actions stay reachable while content scrolls')
    state.reportReviewMode = true
    await reportDialog.getByRole('button', { name: '较新报告', exact: true }).click()
    await reportDialog.getByRole('button', { name: '查看变更审核', exact: true }).click()
    await reportDialog.waitFor({ state: 'hidden' })
    const reportReview = page.getByRole('dialog', { name: '审核 Wiki 变更' })
    await reportReview.getByText('报告关联审核', { exact: true }).waitFor()
    await checkLayout('compile-report-review', reportReview)
    await reportReview.getByRole('button', { name: '稍后处理', exact: true }).click()
    await reportReview.waitFor({ state: 'hidden' })
    state.reportReviewMode = false
    assert.equal(await page.getByRole('button', { name: '调整实体' }).count(), 0, 'expired evidence cannot be restored by a text-only edit')
    await checkLayout('expired-source-detail')
    if (mode === 'phone') {
      await page.getByRole('button', { name: 'Wiki 操作', exact: true }).click()
      await page.getByRole('dialog', { name: 'Wiki 操作' }).getByRole('button', { name: /知识体检/ }).click()
    } else {
      await page.getByRole('button', { name: '知识体检', exact: true }).click()
    }
    const healthDialog = page.getByRole('dialog', { name: '知识库体检结果' })
    await healthDialog.getByText('1 个主题的来源或依赖需要复核', { exact: true }).waitFor()
    await checkLayout('source-impact-health', healthDialog)
    await healthDialog.getByRole('button', { name: staleEntry.title, exact: true }).click()
    await healthDialog.waitFor({ state: 'hidden' })
    await page.getByRole('region', { name: '来源复核提示' }).waitFor()
    await page.getByRole('button', { name: '重新分析当前依据', exact: true }).click()
    await reportDialog.getByText('25 / 31', { exact: true }).waitFor()
    await reportDialog.getByText('新编译已进入队列，正在等待本次报告建立；下方仍是之前的历史报告。', { exact: true }).waitFor()
    assert.deepEqual(state.reviewRetries, [{ knowledge_base_id: base.id }])
    await reportDialog.getByText('正在分析', { exact: true }).waitFor()
    assert.ok(state.reviewRetryReportReads >= 2, 'queued report follows the latest attempt automatically')
    assert.equal(state.reportReads.at(-1).report_id, undefined, 'while queueing, refresh latest rather than repeatedly fetching old report id')
    await reportDialog.getByRole('button', { name: '关闭', exact: true }).click()
    await reportDialog.waitFor({ state: 'hidden' })
    assert.equal(await page.getByRole('button', { name: '重新分析当前依据', exact: true }).isDisabled(), true, 'queued source review cannot be started twice')
    await page.getByRole('button', { name: '提交历史归档', exact: true }).click()
    const archiveDialog = page.getByRole('dialog', { name: '提交历史归档' })
    await archiveDialog.getByText(/不删除正文/).waitFor()
    await checkLayout('historical-archive-confirm', archiveDialog)
    await archiveDialog.getByRole('button', { name: '取消', exact: true }).click()
    await archiveDialog.waitFor({ state: 'hidden' })
    assert.equal(state.archiveRequests.length, 0, 'archive cancellation before submission does not mutate')
    await page.getByRole('button', { name: '提交历史归档', exact: true }).click()
    await archiveDialog.getByRole('button', { name: '生成归档审核', exact: true }).click()
    await reportReview.getByText('历史保留 · 不作事实核验', { exact: true }).waitFor()
    assert.deepEqual(state.archiveRequests, [{ entry_id: staleEntry.id, expected_revision: 3 }])
    await checkLayout('historical-archive-review', reportReview)
    assert.equal(state.archiveResolutions.length, 0, 'archive still requires separate review')
    await reportReview.getByRole('button', { name: '驳回', exact: true }).click()
    await reportReview.waitFor({ state: 'hidden' })
    assert.equal(state.archiveResolutions[0].change_set_id, 'mock-archive')
    assert.equal(state.archiveResolutions[0].decision, 'reject')
    await page.goto(`${origin}/knowledge/tasks`)
    const researchCard = page.locator('.research-task').filter({ has: page.getByRole('heading', { name: researchTask.title, exact: true }) })
    const taskDialog = page.getByRole('dialog', { name: '研究任务结果', exact: true })
    await researchCard.getByRole('button', { name: '阶段与成果', exact: true }).click()
    await taskDialog.getByText('已保存 1 / 2 个主题', { exact: true }).waitFor()
    await taskDialog.getByText('研究目标、约束与验收条件', { exact: true }).click()
    await taskDialog.getByText('版本 2 · 30 条具体主张 · 32,000 字符', { exact: true }).waitFor()
    await selectStage(taskDialog, '综合结论与交叉核验')
    await taskDialog.getByRole('heading', { name: '跨章节对照记录', exact: true }).waitFor()
    await taskDialog.getByText('不同版本和条件分别成立，不强行合并。', { exact: true }).waitFor()
    await taskDialog.getByRole('heading', { name: '跨章节对照记录', exact: true }).scrollIntoViewIfNeeded()
    await checkLayout('research-cross-review', taskDialog)
    await selectStage(taskDialog, '机制与公式')
    await taskDialog.getByText('报告尾部条件与反例完整保留。', { exact: false }).waitFor()
    assert.equal(await taskDialog.locator('.katex').count() > 0, true, 'saved formula is rendered')
    await taskDialog.getByRole('spinbutton', { name: '研究阶段历史版本' }).fill('1')
    await taskDialog.getByRole('button', { name: '读取版本', exact: true }).click()
    await taskDialog.getByRole('heading', { name: '保存的旧版主题', exact: true }).waitFor()
    await checkLayout('research-stage-history', taskDialog)
    const currentReadsBeforeStage = state.currentReads
    await taskDialog.getByRole('button', { name: 'S1 · 实际读取快照', exact: true }).click()
    const stageCitation = page.getByRole('dialog', { name: '来源预览', exact: true })
    await stageCitation.getByText(/运行当时读取的 version-2/).waitFor()
    assert.deepEqual(state.snapshotReads.at(-1), { run_id: 'research-mechanism-old', source_index: 0 }, 'selected revision uses its original content run')
    assert.equal(state.currentReads, currentReadsBeforeStage, 'stage snapshot does not require a live entity or substitute current text')
    await stageCitation.getByRole('button', { name: '关闭', exact: true }).click()
    await stageCitation.waitFor({ state: 'hidden' })
    await taskDialog.getByRole('button', { name: '本次阶段检查器', exact: true }).click()
    await taskDialog.getByText('未知模型容量 · 应用护栏', { exact: true }).waitFor()
    await taskDialog.getByText('尚缺反例，不把局部依据当全书结论', { exact: true }).waitFor()
    await checkLayout('research-stage-inspector', taskDialog)
    await taskDialog.getByRole('button', { name: '研究阶段', exact: true }).click()
    await selectStage(taskDialog, '完整报告')
    await taskDialog.getByRole('button', { name: '保留正文的来源运行', exact: true }).waitFor()
    await taskDialog.getByText('依据变化；旧报告与原始引用保留', { exact: true }).waitFor()
    await checkLayout('research-retained-report', taskDialog)
    await selectStage(taskDialog, '引用与结构校验')
    await taskDialog.getByText('此阶段尚未保存完整成果。失败、排队和运行中的内容不会冒充已完成报告。', { exact: true }).waitFor()
    await selectStage(taskDialog, '演示交付')
    await taskDialog.getByRole('button', { name: '本次阶段检查器', exact: true }).click()
    await taskDialog.getByRole('region', { name: '演示选材范围' }).waitFor()
    await taskDialog.getByText('71,000 字符', { exact: true }).waitFor()
    await checkLayout('research-presentation-material', taskDialog)
    const footerRect = await taskDialog.getByRole('button', { name: '完成', exact: true }).boundingBox()
    assert.ok(footerRect && footerRect.y >= 0 && footerRect.y + footerRect.height <= viewport.height, 'completion action remains in view beneath long inspector')
    state.researchHold = true
    await taskDialog.getByRole('button', { name: '恢复未完成阶段', exact: true }).click()
    await taskDialog.waitFor({ state: 'hidden' })
    await researchCard.getByRole('button', { name: '阶段与成果', exact: true }).click()
    await taskDialog.getByText('已保存 1 / 2 个主题', { exact: true }).waitFor()
    await taskDialog.getByRole('button', { name: '研究报告', exact: true }).click()
    await taskDialog.getByText('报告尾部条件与反例完整保留。', { exact: false }).waitFor()
    await page.waitForTimeout(2800)
    assert.ok(state.researchQueuePolls >= 2, 'queued recovery receives status polling')
    assert.equal(await taskDialog.getByText('报告尾部条件与反例完整保留。', { exact: false }).count() > 0, true, 'status polling cannot replace the saved full report with a queue summary')
    await checkLayout('research-report-during-recovery', taskDialog)
    await taskDialog.getByRole('button', { name: '研究阶段', exact: true }).click()
    state.researchHold = false
    await page.waitForFunction(() => document.querySelector('.task-result-modal .knowledge-status')?.textContent === '已完成')
    assert.deepEqual(state.researchExecutions, [{ task_id: researchTask.id }], 'explicit recovery queues exactly one authorized task')
    assert.equal(await taskDialog.getByRole('region', { name: '研究阶段', exact: true }).isVisible(), true, 'completion does not force-switch the open stage view')
    await taskDialog.getByRole('button', { name: '研究报告', exact: true }).click()
    await taskDialog.getByText(/报告尾部条件与反例完整保留/).waitFor()
    await checkLayout('research-complete-report', taskDialog)
    await taskDialog.getByRole('button', { name: '完成', exact: true }).click()
    await taskDialog.waitFor({ state: 'hidden' })
    const runningCard = page.locator('.research-task').filter({ has: page.getByRole('heading', { name: runningTask.title, exact: true }) })
    await runningCard.getByRole('button', { name: '阶段与成果', exact: true }).click()
    await taskDialog.getByText('已保存 1 / 2 个主题', { exact: true }).waitFor()
    await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true }); document.dispatchEvent(new Event('visibilitychange')) })
    const hiddenWorkspaceReads = state.workspaceReads.length
    await page.waitForTimeout(2700)
    assert.equal(state.workspaceReads.length, hiddenWorkspaceReads, 'hidden window does not poll research bodies or request focus')
    await page.evaluate(() => { delete document.visibilityState; document.dispatchEvent(new Event('visibilitychange')) })
    await page.waitForFunction(count => window.document.visibilityState === 'visible', hiddenWorkspaceReads)
    await taskDialog.getByRole('button', { name: '完成', exact: true }).click()
    await taskDialog.waitFor({ state: 'hidden' })
    const closedWorkspaceReads = state.workspaceReads.length
    await page.waitForTimeout(2700)
    assert.equal(state.workspaceReads.length, closedWorkspaceReads, 'closing stages cancels its poll independently of the task list')
    await runningCard.getByRole('button', { name: '阶段与成果', exact: true }).click()
    await taskDialog.getByText('已保存 1 / 2 个主题', { exact: true }).waitFor()
    state.stageDelay = 500
    const previousStageReads = state.stageReads.length
    await selectStage(taskDialog, '机制与公式')
    await page.waitForFunction(() => document.querySelector('.research-workspace [role="status"]')?.textContent.includes('所选阶段'))
    assert.ok(state.stageReads.length > previousStageReads)
    await taskDialog.getByRole('button', { name: '完成', exact: true }).click()
    await page.waitForTimeout(700)
    assert.equal(await taskDialog.count(), 0, 'late stage body cannot reopen a closed modal')
    state.stageDelay = 0
    assert.deepEqual(pageErrors, [], `${mode}: browser runtime errors`)
    results.push({ viewport, providerRoundtrip: true, nullClearing: true, cancelAndReedit: true, interruptedTextPreserved: true, explicitRecovery: true, adaptiveInspector: true, nextRequestRetried: true, historicalSnapshot: true, restoredConversationSnapshot: true, sourceImpactDetail: true, healthToAffectedEntry: true, compileReportPagination: true, compileReportHistory: true, compileReportToReview: true, explicitSourceReview: true, historicalArchive: true, researchStages: true, frozenBaselineMetadata: true, researchHistoricalCitation: true, retainedReportIdentity: true, researchPhaseInspector: true, presentationProjection: true, explicitResearchRecovery: true, hiddenAndClosedPollStopped: true, lateStageIgnored: true, clippedDialogs: false })
    await context.close()
  }
  assert.deepEqual(state.unsupported, [], 'unexpected API calls must be explicitly mocked')
  console.log(JSON.stringify({ passed: true, results, elapsedSeconds: (Date.now() - startedAt) / 1000, screenshotDir, isolation: 'Only ephemeral Vite mock /v1 endpoints; external HTTP blocked; no production proxy or database' }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
