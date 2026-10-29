/** Real Home/Reader/Task controls; isolated in-memory API, no user data. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'
import { verifyIconGeometry } from './helpers/verify-icon-geometry.mjs'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'glass-coverage-'))
console.log(`Screenshots: ${screenshots}`)
const unexpected = [], results = []
const timestamp = '2026-10-26T00:00:00Z'
const book = { id: 'fixture-book', kind: 'folder', name: '缓存策略学习手册', description: '', category: '技术', path: '/isolated-glass-book', addedAt: 1 }
const task = { id: 'fixture-task', root_id: 'fixture-task', parent_id: null, role: 'root', kind: 'long', title: '整理学习计划', description: '', status: 'planned', importance: 'normal', start_date: '2026-10-01', end_date: '2026-12-01', position: 0, revision: 1, created_at: timestamp, updated_at: timestamp, progress_percent: 0, effective_leaf_count: 1, completed_leaf_count: 0 }
const base = { id: 'fixture-base', book_id: book.id, book_name: book.name, book_path: book.path, book_kind: 'folder', book_description: '', book_category: '技术', source_available: true, lifecycle: 'active', sync_state: 'clean', compile_mode: 'smart', compile_state: 'ready', compile_phase: 'completed', compile_current_batch: 1, compile_total_batches: 1, compile_cancel_requested: false, compile_processed_sources: 1, compile_total_sources: 1, health_state: 'healthy', pending_review_count: 0, source_count: 1, entry_count: 1, claim_count: 1, task_count: 1 }
const entry = { id: 'fixture-entry', knowledge_base_id: base.id, entry_type: 'concept', slug: 'cache', title: '缓存与一致性', summary: '在不同约束下理解缓存的适用边界。', status: 'verified', confidence: 0.9, source_path: 'chapter.md', updated_at: timestamp,
  content_md: '# 缓存与一致性\n\n这是只用于页面材质验收的正文。\n\n| 机制 | 边界 |\n|---|---|\n| 缓存 | 需要明确失效策略 |', aliases: [], edit_policy: 'generated', revision: 1,
  claims: [{ id: 'fixture-claim', predicate: '需要', claim_text: '缓存需要明确失效条件。', verification_status: 'verified', citation_count: 1 }], relations: [],
  citations: [{ id: 'fixture-citation', source_path: 'chapter.md', line_start: 1, line_end: 3, quote_text: '缓存有适用条件。' }], versions: [] }
const runtime = { available: false, message: '隔离验收，不启动模型', profile: { id: 'fixture-runtime', name: 'DeepSeek Harness', runtime: 'deepseek_harness', executable: 'never-run', enabled: false, revision: 1, updated_at: timestamp } }
const provider = { provider_id: 'fixture-provider', display_name: '隔离供应商', api_protocol: 'openai-completions', base_url: 'https://never-contact.example/v1', model: 'fixture-model', credential_source: 'environment', api_key_env: 'FIXTURE_NO_KEY', api_key_configured: false, enabled: false, revision: 1, updated_at: timestamp }
const research = { id: 'fixture-research', knowledge_base_id: base.id, book_name: book.name, title: '分析缓存的一致性边界', description: '比较不同策略，说明依据与限制。', task_type: 'research', status: 'completed', result_summary: '# 缓存研究报告\n\n结论必须说明条件和证据边界。', deliverable_type: 'report', artifact_state: 'not_requested', knowledge_change_state: 'none', external_research_enabled: false, external_requests_used: 0, external_request_limit: 0, created_at: timestamp, updated_at: timestamp }
const overview = {
  today: '2026-10-26', week_start: '2026-10-26', generated_at: timestamp,
  system: { version: 'fixture', uptime_seconds: 300, components: { server: 'ok', sqlite: 'ok' } },
  tasks: { data: { active_count: 1, today_count: 0, overdue_count: 0, items: [task] }, error: null },
  reading: { data: [], error: null }, memos: { data: { total_count: 0, week_count: 0, items: [] }, error: null },
  wiki: { data: { running_count: 0, review_count: 0, failed_count: 0, items: [] }, error: null },
  storage: { data: { originals_bytes: 0, cache_bytes: 0, cache_limit_bytes: 268435456, pending_cleanup: 0 }, error: null },
}
const fixtures = {
  get_reader_books: () => ({ books: [book] }),
  get_reader_history: () => ({ history: [] }),
  save_reader_history: args => ({ ok: true, count: args.history.length }),
  save_reader_progress: args => ({ state: args.state }),
  list_local_dir: () => ({ root: book.path, entries: [{ name: 'chapter.md', path: book.path + '/chapter.md', is_dir: false, is_markdown: true, is_pdf: false }], total: 1 }),
  read_local_file: args => ({ path: args.path, name: 'chapter.md', content: '# 缓存策略\n\n隔离阅读与搜索测试，不读取任何真实文件。', size: 80 }),
  list_tasks: args => ({ tasks: args.kind === 'short' ? [] : [task], next_cursor: null }),
  get_task: () => ({ root: task, tasks: [task], progress: [{ id: 'fixture-progress', task_id: task.id, root_id: task.id, recorded_at: timestamp, note: '整理已有的学习资料', percent_after: 0, created_at: timestamp }], audit: [], storage_path: 'db:fixture', document_version: { revision: 1, content_hash: 'fixture' }, progress_percent: 0, completed_leaf_count: 0, effective_leaf_count: 1 }),
  get_task_calendar: () => [task],
  browse_timeline: () => ({ memos: [{ id: 'fixture-memo', content: '记录一次阅读中的发现。\n\n记录与证据都保存在本机。', timestamp, images: [], tags: ['阅读'], revision: 1 }], total: 1, has_more: false }),
  list_code_repos: () => ({ repos: [{ name: 'isolated-project', path: '/isolated/project', current_branch: 'main', is_dirty: false, languages: { Rust: 0.7, TypeScript: 0.3 } }] }),
  get_repo_detail: () => ({ name: 'isolated-project', path: '/isolated/project', current_branch: 'main', is_dirty: false, contributors: ['Fixture'], recent_commits: [], languages: { Rust: 0.7 }, total_commits: 1 }),
  list_book_knowledge_bases: () => ({ items: [{ book, knowledge_base: base }] }),
  list_knowledge_entries: () => ({ entries: [entry], offset: 0, limit: 60, total: 1, has_more: false }),
  get_knowledge_entry: () => entry,
  list_knowledge_change_sets: () => ({ change_sets: [] }),
  get_book_wiki_settings: () => ({ runtime_profiles: [runtime], model_providers: [provider], documents: ['WIKI.md', 'QUERY.md', 'RESEARCH.md'].map(name => ({ id: name, name, knowledge_base_id: base.id, scope: 'book', revision: 1, content_md: '# 隔离配置\n\n仅验收布局和材质，不保存配置。', updated_at: timestamp })) }),
  list_wiki_skills: () => ({ skills: [{ id: 'fixture-skill', slug: 'book-query', name: '知识问答', description: '使用证据回答问题', source_type: 'custom', revision: 1, status: 'ready', permissions: [], requirements: [], instructions: '# 证据规则', enabled: false, usage_scope: 'both', updated_at: timestamp }] }),
  get_agent_usage_stats: args => ({ start_date: args.start_date, end_date: args.end_date, usage_source: 'unavailable', totals: { runs: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0, unreported_runs: 0 }, daily: [], by_caller: [] }),
  list_knowledge_conversations: () => ({ conversations: [{ id: 'fixture-conversation', knowledge_base_id: base.id, title: '缓存的一致性条件', created_at: timestamp, updated_at: timestamp }] }),
  get_knowledge_conversation: () => ({ id: 'fixture-conversation', knowledge_base_id: base.id, title: '缓存的一致性条件', created_at: timestamp, updated_at: timestamp, messages: [
    { id: 'fixture-user', role: 'user', content: '缓存有哪些适用条件？', evidence: [], created_at: timestamp },
    { id: 'fixture-answer', role: 'assistant', content: '# 缓存的适用条件\n\n需要根据读取模式、失效策略和一致性要求作出选择。', evidence: [entry], created_at: timestamp },
  ] }),
  list_unfinished_qa_runs: () => ({ runs: [] }),
  list_knowledge_tasks: () => ({ tasks: [research] }),
  get_knowledge_task_result: () => ({ task: research, run_id: '', evidence: [], artifacts: [] }),
}
const mock = { name: 'isolated-glass-coverage', configureServer(server) {
  server.middlewares.use(async (request, response, next) => {
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url.startsWith('/v1/home/overview?')) return response.end(JSON.stringify(overview))
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'fixture', components: {}, uptime_seconds: 300, tools_count: 0 }))
    let body = ''; for await (const part of request) body += part
    const { tool, arguments: args = {} } = body ? JSON.parse(body) : {}
    if (request.url !== '/v1/tools/call' || !fixtures[tool]) {
      unexpected.push(tool || request.url); response.statusCode = 501
      return response.end(JSON.stringify({ status: 'error', error: { message: 'Real backend forbidden' } }))
    }
    response.end(JSON.stringify({ status: 'success', result: fixtures[tool](args) }))
  })
} }
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, cacheDir: join(screenshots, '.vite-cache'), configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser, activePage
async function glass(locator, token) {
  const actual = await locator.evaluate((element, token) => {
    const host = getComputedStyle(element)
    const reference = document.createElement('span'); reference.style.background = `var(--${token})`; document.body.append(reference)
    const expected = getComputedStyle(reference).backgroundColor; reference.remove()
    return { fill: host.backgroundColor, expected, filter: host.backdropFilter, shadow: host.boxShadow,
      foreground: getComputedStyle(element.querySelector('input, strong, h2, span:not(.glass-optics)') || element).filter,
      count: element.querySelectorAll(':scope > .glass-optics, :scope > .glass-optics-defs').length,
      height: element.getBoundingClientRect().height, radius: host.borderRadius, padding: host.paddingInline, fontSize: host.fontSize,
      pageOverflow: document.documentElement.scrollWidth > innerWidth + 1 }
  }, token)
  assert.equal(actual.fill, actual.expected, `same reference material, not the old opaque plane: ${JSON.stringify(actual)}`)
  assert.ok(actual.filter && actual.filter !== 'none')
  assert.notEqual(actual.shadow, 'none')
  assert.equal(actual.foreground, 'none'); assert.equal(actual.count, 0, 'no independent lenses or observers per card/search')
  assert.equal(actual.pageOverflow, false, 'material changes cannot hide a new page overflow')
  await verifyIconGeometry(locator.page())
  if (token === 'glass-floating-fill') { assert.equal(actual.height, 44); assert.equal(actual.radius, '14px'); assert.equal(actual.padding, '14px') }
  return actual
}
async function selectGlass(locator, fallback = false) {
  const material = await locator.evaluate(element => {
    const before = getComputedStyle(element, '::before'), after = getComputedStyle(element, '::after')
    return { fill: before.backgroundColor, filter: before.backdropFilter, pointer: before.pointerEvents, content: before.content,
      edge: after.maskImage, edgeDisplay: after.display, host: getComputedStyle(element).backdropFilter,
      menuRadius: element.matches('.el-popper') ? getComputedStyle(element).borderRadius : null,
      menuClip: element.matches('.el-popper') ? getComputedStyle(element).overflow : null }
  })
  assert.equal(material.content, '""'); assert.equal(material.pointer, 'none'); assert.equal(material.host, 'none')
  assert.notEqual(material.fill, 'rgba(0, 0, 0, 0)')
  assert.equal(material.filter === 'none', fallback, JSON.stringify(material))
  if (fallback) assert.equal(material.edgeDisplay, 'none')
  else assert.notEqual(material.edge, 'none')
  if (material.menuRadius !== null) { assert.equal(material.menuRadius, '14px'); assert.equal(material.menuClip, 'hidden') }
}
async function settledMenu(menu) {
  await menu.waitFor()
  await menu.evaluate(async element => {
    await Promise.all(element.getAnimations({ subtree: true }).map(animation => animation.finished.catch(() => {})))
  })
  const bounds = await menu.boundingBox()
  assert.ok(bounds && bounds.y >= 0 && bounds.y + bounds.height <= 901, 'open menu stays within the viewport')
  for (const option of await menu.getByRole('option').all()) {
    const rect = await option.boundingBox()
    assert.ok(rect && rect.height >= 36, 'settled options retain readable unscaled touch targets')
  }
}
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [320, 390, 1440]) for (const theme of ['light', 'dark', 'eye-care']) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: width < 769, hasTouch: width < 769, reducedMotion: 'reduce' })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    const page = await context.newPage(), errors = []
    activePage = page
    page.setDefaultTimeout(20000)
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(origin)
    await page.getByText('vfixture', { exact: true }).waitFor()
    assert.equal(await page.locator('[data-glass="card"]').count(), 9)
    for (const card of await page.locator('[data-glass="card"]').all()) await glass(card, 'glass-structural-fill')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-home.png`) })

    await page.goto(origin + '/reader')
    const shelf = page.locator('.shelf-search'), search = page.getByRole('searchbox', { name: '搜索书籍' })
    const searchReference = await glass(shelf, 'glass-floating-fill')
    const before = await shelf.boundingBox()
    await search.fill('不存在的书名'); assert.equal(await page.locator('.book-cover').count(), 0)
    await page.getByRole('button', { name: '清除搜索', exact: true }).click()
    await page.locator('.book-cover').first().waitFor()
    assert.equal(Math.round((await shelf.boundingBox()).width), Math.round(before.width), 'focusing never changes search geometry')
    const addBook = page.getByRole('button', { name: '添加书籍', exact: true })
    assert.equal(await addBook.count(), 1, 'one shared shelf toolbar action, not separate desktop/phone duplicates')
    await addBook.click()
    const bookForm = page.getByRole('dialog', { name: '添加书籍', exact: true })
    await bookForm.waitFor()
    await bookForm.getByRole('button', { name: '取消', exact: true }).click()
    await bookForm.waitFor({ state: 'hidden' })
    await page.locator('.book-cover').first().waitFor()
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-reader.png`) })
    await page.locator('.book-cover').first().click()
    if (width > 768) await page.locator('.ft-row:visible').first().click()
    await page.locator('.markdown-body h1').waitFor()
    await glass(page.locator('.pane-center'), 'glass-structural-fill')
    if (width > 768) {
      await glass(page.locator('.reader-file-search-trigger'), 'glass-floating-fill')
      await page.locator('.reader-file-search-trigger').click()
    } else await page.getByRole('button', { name: '搜索当前目录文件', exact: true }).click()
    await glass(page.locator('.reader-file-search'), 'glass-floating-fill')
    await page.getByRole('searchbox', { name: '搜索当前目录文件', exact: true }).fill('chapter')
    assert.ok(await page.locator('.reader-search-results li').count() > 0)
    await page.keyboard.press('Escape')

    await page.goto(origin + '/tasks')
    if (width > 768) {
      const reference = await glass(page.locator('.task-list-panel'), 'glass-structural-fill')
      assert.equal(reference.filter, 'blur(5px) saturate(1.15)')
      assert.equal((await glass(page.locator('.task-detail-zone'), 'glass-structural-fill')).shadow, reference.shadow)
    } else await page.locator('.mobile-task-row').first().waitFor()
    if (width <= 768) await page.getByRole('button', { name: '搜索任务', exact: true }).click()
    assert.deepEqual(await glass(page.locator('.task-search:visible').first(), 'glass-floating-fill'), searchReference, 'Reader and Tasks use the identical search material and geometry')
    await page.getByRole('searchbox', { name: '搜索任务', exact: true }).filter({ visible: true }).fill('学习')
    if (width <= 768) await page.keyboard.press('Escape')
    if (width <= 768) await page.getByRole('button', { name: '筛选任务', exact: true }).click()
    const select = page.locator(width <= 768 ? '.query-filter-fields .el-select' : '.filters .el-select').first()
    await selectGlass(select.locator('.el-select__wrapper'))
    await select.click()
    const menu = page.locator('.el-popper:visible').filter({ has: page.locator('.el-select-dropdown') })
    await settledMenu(menu); await selectGlass(menu)
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-select.png`) })
    await page.getByRole('option', { name: '短期待办', exact: true }).click()
    assert.match(await select.innerText(), /短期待办/)
    await select.click()
    const client = await context.newCDPSession(page)
    for (const [name, value] of [['prefers-reduced-transparency', 'reduce'], ['prefers-contrast', 'more'], ['forced-colors', 'active']]) {
      await client.send('Emulation.setEmulatedMedia', { features: [{ name, value }] })
      await selectGlass(select.locator('.el-select__wrapper'), true); await selectGlass(menu, true)
    }
    await client.send('Emulation.setEmulatedMedia', { features: [] }); await client.detach()
    await page.keyboard.press('Escape')

    // Real detail content, not just the empty task placeholder.
    await page.goto(origin + '/tasks?task=' + task.id)
    await page.locator(width > 768 ? '.detail-header h2' : '.mobile-task-hero h2').waitFor()
    if (width > 768) {
      await page.waitForFunction(() => document.querySelectorAll('.task-detail-zone').length === 1)
      await glass(page.locator('.task-detail-zone'), 'glass-structural-fill')
    } else assert.equal(await page.locator('.mobile-task-detail').count(), 1)
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-task-detail.png`) })
    await page.goto(origin + '/tasks?view=calendar')
    await page.locator('.task-calendar').waitFor()
    if (width > 768) await glass(page.locator('.task-calendar'), 'glass-structural-fill')
    else assert.equal(await page.locator('.task-calendar').getAttribute('data-glass'), 'flat')
    assert.equal(await page.locator('.agenda').evaluate(element => getComputedStyle(element).backdropFilter), 'none')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-calendar.png`) })

    await page.goto(origin + '/timeline')
    await page.locator('.memo-card-body').waitFor()
    await glass(page.locator('.memo-card-body'), 'glass-structural-fill')
    assert.deepEqual(await glass(page.locator('.search-box'), 'glass-floating-fill'), searchReference, 'Timeline is the shared search reference')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-timeline.png`) })

    // Mobile intentionally has no repository page; do not change that policy.
    if (width > 768) {
      await page.goto(origin + '/code-repo')
      await glass(page.locator('.repo-card'), 'glass-structural-fill')
      await page.getByRole('button', { name: '查看仓库详情', exact: true }).click()
      await page.getByRole('region', { name: '仓库详情内容' }).waitFor()
      await page.keyboard.press('Escape')
    }
    await page.goto(origin + '/knowledge')
    await page.locator('.book-wiki-card').waitFor()
    await glass(page.locator('.book-wiki-card'), 'glass-structural-fill')
    await glass(page.locator('.overview-strip'), 'glass-structural-fill')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-knowledge.png`) })

    await page.goto(origin + '/knowledge/wiki')
    assert.deepEqual(await glass(page.locator('.knowledge-search'), 'glass-floating-fill'), searchReference, 'Wiki uses the same search reference, including phone typography')
    await page.locator('.entry-row').click()
    await page.locator('.entity-markdown h1').waitFor()
    await page.waitForFunction(() => document.querySelectorAll('.entry-detail').length === 1)
    await glass(page.locator('.entry-detail'), 'glass-structural-fill')
    assert.equal(await page.locator('.structure-card').evaluate(element => getComputedStyle(element).backdropFilter), 'none')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-wiki.png`) })
    if (width > 768) await glass(page.locator('.knowledge-search'), 'glass-floating-fill')

    await page.goto(origin + '/knowledge/chat')
    await page.locator('.message.assistant').waitFor()
    await glass(page.locator('.chat-panel'), 'glass-structural-fill')
    const answer = await page.locator('.message.assistant .knowledge-answer-markdown').evaluate(element => {
      const style = getComputedStyle(element)
      return { role: element.getAttribute('data-glass'), background: style.backgroundImage, backdrop: style.backdropFilter, foreground: style.filter }
    })
    assert.equal(answer.role, 'subcard'); assert.match(answer.background, /linear-gradient/)
    assert.equal(answer.backdrop, 'none'); assert.equal(answer.foreground, 'none')
    await page.getByRole('button', { name: /参考来源/ }).click()
    await page.locator('.evidence-grid > button').waitFor()
    assert.equal(await page.locator('.evidence-grid > button').evaluate(element => getComputedStyle(element).backdropFilter), 'none')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-chat.png`) })

    await page.goto(origin + '/knowledge/tasks')
    await page.locator('.research-task').waitFor()
    await glass(page.locator('.research-task'), 'glass-structural-fill')
    await page.getByRole('button', { name: '查看阶段与成果', exact: true }).click()
    await page.locator('.task-result-modal').waitFor()
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-research.png`) })
    await page.keyboard.press('Escape')

    await page.goto(origin + '/knowledge/settings')
    await page.locator('.runtime-card').waitFor()
    if (width <= 768) {
      for (const card of await page.locator('.settings-card').all()) await glass(card, 'glass-structural-fill')
      assert.equal(await page.locator('.settings-main').evaluate(element => getComputedStyle(element).backdropFilter), 'none')
    } else await glass(page.locator('.settings-main'), 'glass-structural-fill')
    assert.equal(await page.locator('.runtime-card').evaluate(element => getComputedStyle(element).backdropFilter), 'none')
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-settings.png`) })
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
    await page.goto(origin + '/tests/fixtures/icon-controls.html')
    await page.locator('body[data-ready="true"]').waitFor()
    assert.equal(await verifyIconGeometry(page, 'native/shared loading, disabled and close states'), 6)
    assert.ok(await page.getByRole('button', { name: '原生加载状态' }).isDisabled())
    assert.ok(await page.getByRole('button', { name: '共享加载状态' }).isDisabled())
    assert.deepEqual(errors, [])
    assert.deepEqual(unexpected, [])
    results.push({ width, theme, homeReaderTasks: true, timeline: true, wikiFivePages: true, calendarDetail: true, repository: width > 768, searchesSelects: true, accessibility: true })
    await context.close()
  }
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, screenshots }, null, 2))
} catch (error) {
  console.error(JSON.stringify({ completed: results, unexpected, url: activePage?.url(), body: await activePage?.locator('body').innerText().catch(() => '') }))
  await activePage?.screenshot({ path: join(screenshots, 'failure.png') }).catch(() => {})
  throw error
} finally { await browser?.close(); await server.close() }
