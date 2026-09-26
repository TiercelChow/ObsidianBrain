/**
 * Isolated browser acceptance: no production API, SQLite, credentials, or shelf access.
 * Run: PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node scripts/verify-wiki-reliability.mjs
 * If Playwright is installed locally, the environment override is unnecessary.
 * Optional SCREENSHOT_DIR selects the generated-artifact directory.
 */
import assert from 'node:assert/strict'
import { mkdtemp, mkdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const startedAt = Date.now()
const frontendRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const screenshotDir = process.env.SCREENSHOT_DIR || await mkdtemp(join(tmpdir(), 'wiki-reliability-'))
await mkdir(screenshotDir, { recursive: true })
const timestamp = '2026-09-26T00:00:00Z'
const entry = { id: 'mock-entry', knowledge_base_id: 'mock-base', entry_type: 'concept', slug: 'mechanism', title: '历史引用实体', summary: '编译后的机制', status: 'verified', source_path: 'chapter.md', updated_at: timestamp }
const staleEntry = { ...entry, id: 'mock-stale', slug: 'expired', title: '依据已变化的知识主题', status: 'stale' }
const base = { id: 'mock-base', book_id: 'mock-book', book_name: '隔离测试书籍', book_path: '/mock/not-a-real-book', book_kind: 'folder', lifecycle: 'active', source_available: true, entry_count: 3, sync_state: 'clean', compile_mode: 'smart', compile_state: 'ready', compile_phase: 'completed' }
const provider = { provider_id: 'mock-provider', display_name: '隔离模型供应商', api_protocol: 'openai-completions', base_url: 'https://never-contact.example/v1', model: 'mock-model', credential_source: 'environment', api_key_env: 'MOCK_NOT_A_REAL_SECRET', api_key_configured: false, context_window: null, max_output_tokens: null, reasoning_policy: 'auto', enabled: true, revision: 1, updated_at: timestamp }
const state = { provider: { ...provider }, providerSaves: [], chatAttempts: 0, chatRequests: [], snapshotReads: [], currentReads: 0, fallbackSearches: 0, reportReads: [], reportReviewMode: false, archiveProposal: null, archiveRequests: [], archiveResolutions: [], reviewRetries: [], reviewRetryReportReads: 0, unsupported: [] }
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
  return { runtime_profiles: [{ available: true, version: 'mock-harness', message: '隔离启动器', profile: { id: 'mock-runtime', name: 'DeepSeek Harness', runtime: 'deepseek_harness', executable: 'never-executed', model: '', provider_id: provider.provider_id, enabled: true, revision: 1, updated_at: timestamp } }], model_providers: [state.provider], documents: [] }
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
        case 'list_wiki_skills': return json(response, success(tool, { skills: [] }))
        case 'save_model_provider': state.providerSaves.push(args); state.provider = { ...state.provider, ...args, revision: state.provider.revision + 1 }; return json(response, success(tool, state.provider))
        case 'list_knowledge_conversations': return json(response, success(tool, { conversations: state.chatAttempts >= 2 ? [conversation] : [] }))
        case 'get_knowledge_conversation': return json(response, success(tool, { ...conversation, messages: [{ id: 'persisted-user', role: 'user', content: '第二次：重新提问', evidence: [], created_at: timestamp }, { id: 'persisted-assistant', role: 'assistant', content: completedAnswer, run_id: 'mock-run-2', evidence: [entry], created_at: timestamp }] }))
        case 'get_agent_run_citation': state.snapshotReads.push(args); return json(response, success(tool, { run_id: args.run_id, citation_index: args.source_index + 1, kind: 'entry', object_id: entry.id, version_id: 'mock-revision-2', entry, content_md: '## 历史快照正文\n\n这是运行当时读取的 version-2；当前实体已改为 version-3。', citations: [], historical: true, read_ranges: [{ offset_chars: 0, returned_chars: 100 }] }))
        case 'get_knowledge_entry': state.currentReads += 1; return json(response, success(tool, { ...(args.entry_id === staleEntry.id ? staleEntry : entry), content_md: '## 保留的历史知识正文\n\n当前 version-3（不应偷偷替换历史快照）', citations: [], aliases: [], revision: 3, claims: [], relations: [], versions: [], source_impact_count: args.entry_id === staleEntry.id ? 1 : 0, source_impacts: args.entry_id === staleEntry.id ? [{ source_document_id: 'mock-doc', source_path: `${'长来源路径/'.repeat(10)}chapter.md`, previous_version_id: 'v1', current_version_id: 'v2', reason: 'source_changed', affected_via_entry_id: null, affected_via_entry_title: null, detected_at: timestamp }] : [] }))
        case 'list_knowledge_change_sets': return json(response, success(tool, { change_sets: state.archiveProposal ? [state.archiveProposal] : state.reportReviewMode ? [{ id: 'mock-change-report', knowledge_base_id: base.id, title: '报告关联审核', reason: '完整归并候选', risk_level: 'high', status: 'proposed', classification_summary: { new: 1, update: 0, disputed: 0 }, citation_audit: { passed: true, entry_citations: 1, claim_citations: 1, issues: [] }, impact_summary: { entries: 1, claims: 1, relations: 0, citations: 1 }, changes: [] }] : [] }))
        case 'lint_book_knowledge_base': return json(response, success(tool, { knowledge_base_id: base.id, state: 'warning', semantic_entry_count: 1, source_span_count: 1, pending_review_count: 0, issues: [{ code: 'expired-source-evidence', severity: 'warning', title: '1 个主题的来源或依赖需要复核', detail: '历史正文仍保留，不能作为当前有效知识。', object_ids: [staleEntry.id] }], generated_at: timestamp }))
        case 'get_agent_run_inspection': return json(response, success(tool, {
          run: { id: args.run_id, task_type: 'knowledge_qa', status: 'completed', runtime: 'deepseek_harness', input: {}, created_at: timestamp, started_at: timestamp },
          events: [{ event_type: 'run.budget_changed', payload: { reason: '需要核对后半书的条件差异' }, message: '扩大取证范围' }], evidence: [],
          snapshot: { run_id: args.run_id, tool_names: [], skill_snapshots: [], config_snapshots: [], prompt_text: '', evidence_refs: {
            qa_plan: { goal: '对比全书的优化机制及适用边界', constraints: ['保留公式和单位'], subquestions: ['机制', '边界'], depth: 'comprehensive' },
            planning_stats: { catalog_seen: 120, catalog_total: 120, elapsed_ms: 1200, time_limit_seconds: 180 },
            adaptive_state: { used_tool_calls: 14, soft_tool_calls: 36, extension_count: 1, estimated_tool_payload_tokens: 14000, soft_retrieval_tokens: 32000, policy: { hard_tool_calls: 120, hard_retrieval_tokens: 64000 }, coverage: [{ question_index: 0, question: '机制', status: 'supported', citation_indices: [1], finding: '从编译知识正文核对机制' }, { question_index: 1, question: '不同版本中的适用边界、条件与例外是否存在冲突', status: 'partial', citation_indices: [1], finding: '仍需核对后半书的具体条件，不冒称全书已覆盖' }] },
          } },
        }))
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
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
    const mode = viewport.width > 700 ? 'desktop' : 'phone'
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
    await page.goto(`${origin}/knowledge/settings`)
    await page.getByRole('button', { name: /模型供应商/ }).click()
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
    assert.deepEqual(pageErrors, [], `${mode}: browser runtime errors`)
    results.push({ viewport, providerRoundtrip: true, nullClearing: true, cancelAndReedit: true, interruptedTextPreserved: true, explicitRecovery: true, adaptiveInspector: true, nextRequestRetried: true, historicalSnapshot: true, restoredConversationSnapshot: true, sourceImpactDetail: true, healthToAffectedEntry: true, compileReportPagination: true, compileReportHistory: true, compileReportToReview: true, explicitSourceReview: true, historicalArchive: true, clippedDialogs: false })
    await context.close()
  }
  assert.deepEqual(state.unsupported, [], 'unexpected API calls must be explicitly mocked')
  console.log(JSON.stringify({ passed: true, results, elapsedSeconds: (Date.now() - startedAt) / 1000, screenshotDir, isolation: 'Only ephemeral Vite mock /v1 endpoints; external HTTP blocked; no production proxy or database' }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
