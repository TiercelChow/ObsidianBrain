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
const base = { id: 'mock-base', book_id: 'mock-book', book_name: '隔离测试书籍', book_path: '/mock/not-a-real-book', book_kind: 'folder', lifecycle: 'active', source_available: true, entry_count: 3 }
const provider = { provider_id: 'mock-provider', display_name: '隔离模型供应商', api_protocol: 'openai-completions', base_url: 'https://never-contact.example/v1', model: 'mock-model', credential_source: 'environment', api_key_env: 'MOCK_NOT_A_REAL_SECRET', api_key_configured: false, context_window: null, max_output_tokens: null, reasoning_policy: 'auto', enabled: true, revision: 1, updated_at: timestamp }
const state = { provider: { ...provider }, providerSaves: [], chatAttempts: 0, snapshotReads: [], currentReads: 0, fallbackSearches: 0, unsupported: [] }

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
        case 'list_book_knowledge_bases': return json(response, success(tool, { items: [{ book: { id: base.book_id, kind: 'folder', name: base.book_name, path: base.book_path }, knowledge_base: base }] }))
        case 'get_book_wiki_settings': return json(response, success(tool, runtimeSettings()))
        case 'list_wiki_skills': return json(response, success(tool, { skills: [] }))
        case 'save_model_provider': state.providerSaves.push(args); state.provider = { ...state.provider, ...args, revision: state.provider.revision + 1 }; return json(response, success(tool, state.provider))
        case 'list_knowledge_conversations': return json(response, success(tool, { conversations: state.chatAttempts >= 2 ? [conversation] : [] }))
        case 'get_knowledge_conversation': return json(response, success(tool, { ...conversation, messages: [{ id: 'persisted-user', role: 'user', content: '第二次：重新提问', evidence: [], created_at: timestamp }, { id: 'persisted-assistant', role: 'assistant', content: completedAnswer, run_id: 'mock-run-2', evidence: [entry], created_at: timestamp }] }))
        case 'get_agent_run_citation': state.snapshotReads.push(args); return json(response, success(tool, { run_id: args.run_id, citation_index: args.source_index + 1, kind: 'entry', object_id: entry.id, version_id: 'mock-revision-2', entry, content_md: '## 历史快照正文\n\n这是运行当时读取的 version-2；当前实体已改为 version-3。', citations: [], historical: true, read_ranges: [{ offset_chars: 0, returned_chars: 100 }] }))
        case 'get_knowledge_entry': state.currentReads += 1; return json(response, success(tool, { ...entry, content_md: '当前 version-3（不应偷偷替换历史快照）', citations: [], aliases: [], revision: 3, claims: [], relations: [], versions: [] }))
        case 'list_knowledge_entries': state.fallbackSearches += 1; return json(response, success(tool, { entries: [entry], offset: 0, limit: 6, total: 1, has_more: false }))
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
    state.provider = { ...provider }
    state.providerSaves = []
    state.snapshotReads = []
    state.currentReads = 0
    state.fallbackSearches = 0
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
      const data = await page.evaluate(() => ({ width: window.innerWidth, height: window.innerHeight, scrollWidth: document.documentElement.scrollWidth }))
      assert.ok(data.scrollWidth <= data.width + 1, `${label}: horizontal overflow ${JSON.stringify(data)}`)
      if (dialog) {
        const rect = await dialog.boundingBox()
        assert.ok(rect && rect.x >= -1 && rect.x + rect.width <= data.width + 1 && rect.y >= -1 && rect.y + rect.height <= data.height + 1, `${label}: dialog clipped ${JSON.stringify(rect)}`)
      }
      await page.screenshot({ path: join(screenshotDir, `${mode}-${label}.png`) })
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
    await composer.fill('第二次：重新提问')
    await page.getByRole('button', { name: '发送问题' }).click()
    await page.getByRole('button', { name: '保存到 Wiki', exact: true }).waitFor()
    assert.equal(state.chatAttempts, 2, 'second request must retry Runtime')
    assert.equal(state.fallbackSearches, 0, 'runtime failure must not silently replace model output with FTS')
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
    assert.deepEqual(pageErrors, [], `${mode}: browser runtime errors`)
    results.push({ viewport, providerRoundtrip: true, nullClearing: true, cancelAndReedit: true, interruptedTextPreserved: true, nextRequestRetried: true, historicalSnapshot: true, restoredConversationSnapshot: true, clippedDialogs: false })
    await context.close()
  }
  assert.deepEqual(state.unsupported, [], 'unexpected API calls must be explicitly mocked')
  console.log(JSON.stringify({ passed: true, results, elapsedSeconds: (Date.now() - startedAt) / 1000, screenshotDir, isolation: 'Only ephemeral Vite mock /v1 endpoints; external HTTP blocked; no production proxy or database' }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
