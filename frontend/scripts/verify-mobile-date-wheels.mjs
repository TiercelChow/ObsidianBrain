/** Real page date controls, with all API calls mocked and no production proxy. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'mobile-date-wheels-'))
const requests = [], unexpected = []
const timestamp = '2026-10-18T00:00:00Z'
const task = { id: 'mock-task', root_id: 'mock-task', parent_id: null, kind: 'long', role: 'root', title: '隔离任务日期测试', description: '', status: 'planned', importance: 'normal', start_date: '2024-01-31', end_date: '2024-03-31', position: 0, revision: 1, created_at: timestamp, updated_at: timestamp, archived_at: null, closed_at: null, closure_note: null }
const summary = { ...task, storage_path: 'mock', document_version: { revision: 1, content_hash: 'mock' }, progress_percent: 0, effective_leaf_count: 1, completed_leaf_count: 0, derived: { overdue: true, active_today: false, due_today: false } }
const fixtures = {
  list_tasks: () => ({ tasks: [summary], next_cursor: null }),
  get_task: () => ({ root: task, tasks: [task], progress: [], audit: [], ...Object.fromEntries(['storage_path', 'document_version', 'progress_percent', 'effective_leaf_count', 'completed_leaf_count'].map(key => [key, summary[key]])) }),
  get_task_calendar: () => [],
  browse_timeline: () => ({ memos: [], total: 0, has_more: false }),
  list_book_knowledge_bases: () => ({ items: [] }),
  get_book_wiki_settings: () => ({ runtime_profiles: [], model_providers: [], documents: [] }),
  list_wiki_skills: () => ({ skills: [] }),
  get_agent_usage_stats: args => ({ ...args, usage_source: 'unavailable', totals: { runs: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0, unreported_runs: 0 }, daily: [], by_caller: [] }),
}
const mock = { name: 'isolated-date-wheels', configureServer(server) {
  server.middlewares.use(async (request, response, next) => {
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'mock', components: {}, uptime_seconds: 1, tools_count: 0 }))
    let body = ''; for await (const chunk of request) body += chunk
    const { tool, arguments: args = {} } = body ? JSON.parse(body) : {}
    if (request.url !== '/v1/tools/call' || !fixtures[tool]) {
      unexpected.push(tool || request.url); response.statusCode = 501
      return response.end(JSON.stringify({ status: 'error', error: { message: 'Real backend forbidden' } }))
    }
    requests.push({ tool, ...args })
    response.end(JSON.stringify({ status: 'success', result: fixtures[tool](args) }))
  })
} }
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser
const results = []
async function choose(dialog, label, value) {
  const wheel = dialog.getByRole('listbox', { name: label, exact: true })
  await wheel.getByRole('option', { name: `${value}${label}`, exact: true }).evaluate(option => option.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 0 })))
  await wheel.page().waitForFunction(element => element.dataset.moving === 'false', await wheel.elementHandle())
  assert.equal(await wheel.locator('[aria-selected="true"]').innerText(), `${value}${label}`)
}
async function date(dialog, year, month, day) { await choose(dialog, '年', year); await choose(dialog, '月', month); await choose(dialog, '日', day) }
async function closed(dialog) { await dialog.waitFor({ state: 'hidden' }) }
async function geometry(dialog, viewport) {
  await dialog.page().waitForFunction(element => !element.closest('.motion-modal').classList.contains('is-settling'), await dialog.elementHandle())
  const panel = await dialog.locator('.date-wheel-sheet').boundingBox()
  assert.ok(panel.x >= -1 && panel.x + panel.width <= viewport.width + 1 && panel.y >= -1 && panel.y + panel.height <= viewport.height + 1, `date sheet fits: ${JSON.stringify({ viewport, panel })}`)
  for (const wheel of await dialog.getByRole('listbox').all()) {
    const selected = await wheel.locator('[aria-selected="true"]').boundingBox(), column = await wheel.boundingBox()
    assert.ok(Math.abs(selected.y + selected.height / 2 - column.y - column.height / 2) < 1, 'selection settles exactly on the center row')
    assert.ok(selected.height >= 44)
  }
}
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const viewport of [{ width: 390, height: 844 }, { width: 320, height: 568 }, { width: 667, height: 375 }]) for (const theme of ['light', 'dark', 'eye-care']) {
    const physical = viewport.width === 390 && theme === 'light'
    const context = await browser.newContext({ viewport, isMobile: true, hasTouch: true, reducedMotion: physical ? 'no-preference' : 'reduce' })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    const page = await context.newPage(), errors = []
    const requestStart = requests.length
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(`${origin}/tasks?task=mock-task`)
    await page.getByRole('button', { name: '编辑任务', exact: true }).filter({ visible: true }).first().click()
    const form = page.getByRole('dialog', { name: '编辑任务', exact: true })
    const start = form.getByRole('button', { name: '开始日期', exact: true })
    await start.click()
    let picker = page.getByRole('dialog', { name: '开始日期', exact: true })
    await choose(picker, '月', 2)
    assert.equal(await picker.getByRole('listbox', { name: '日', exact: true }).locator('[aria-selected="true"]').innerText(), '29日')
    await geometry(picker, viewport)
    await picker.getByRole('button', { name: '取消', exact: true }).click(); await closed(picker)
    assert.match(await start.innerText(), /2024\/01\/31/)
    assert.ok(await form.isVisible(), 'cancelling the child picker keeps its parent sheet open')
    await start.click(); picker = page.getByRole('dialog', { name: '开始日期', exact: true })
    await choose(picker, '月', 2)
    await picker.getByRole('button', { name: '完成', exact: true }).click(); await closed(picker)
    assert.match(await start.innerText(), /2024\/02\/29/)
    assert.equal(await form.getByRole('button', { name: '清除日期', exact: true }).count(), 0, 'required task dates cannot be cleared')
    await form.getByRole('button', { name: '关闭', exact: true }).click(); await closed(form)
    await page.getByRole('button', { name: '添加子任务', exact: true }).filter({ visible: true }).first().click()
    const child = page.getByRole('dialog', { name: '添加子任务', exact: true })
    await child.getByRole('button', { name: '结束日期', exact: true }).click()
    picker = page.getByRole('dialog', { name: '结束日期', exact: true })
    await geometry(picker, viewport)
    await page.keyboard.press('Escape'); await closed(picker)
    assert.ok(await child.isVisible())
    await child.getByRole('button', { name: '关闭', exact: true }).click(); await closed(child)

    await page.goto(`${origin}/timeline`)
    await page.getByRole('button', { name: /^时间范围：/ }).click()
    const filter = page.getByRole('dialog', { name: '时间筛选', exact: true })
    await filter.getByRole('button', { name: '开始日期', exact: true }).click()
    picker = page.getByRole('dialog', { name: '开始日期', exact: true })
    await date(picker, 2024, 2, 29)
    await picker.getByRole('button', { name: '完成', exact: true }).click(); await closed(picker)
    await filter.getByRole('button', { name: '结束日期', exact: true }).click()
    picker = page.getByRole('dialog', { name: '结束日期', exact: true })
    await date(picker, 2024, 3, 5)
    await picker.getByRole('button', { name: '完成', exact: true }).click(); await closed(picker)
    assert.ok(requests.slice(requestStart).some(request => request.tool === 'browse_timeline' && request.start_date === '2024-02-29' && request.end_date === '2024-03-05'))
    await filter.getByRole('button', { name: '结束日期', exact: true }).click()
    picker = page.getByRole('dialog', { name: '结束日期', exact: true })
    await picker.getByRole('button', { name: '清除日期', exact: true }).click(); await closed(picker)
    assert.match(await filter.getByRole('button', { name: '结束日期', exact: true }).innerText(), /选择日期/)
    await filter.getByRole('button', { name: '完成', exact: true }).click(); await closed(filter)

    await page.goto(`${origin}/knowledge/settings?section=usage`)
    await page.locator('.usage-dashboard').waitFor()
    const trigger = page.getByRole('button', { name: 'Token 用量日期范围', exact: true })
    const before = await trigger.innerText(), count = requests.filter(request => request.tool === 'get_agent_usage_stats').length
    await trigger.click()
    picker = page.getByRole('dialog', { name: 'Token 用量日期范围', exact: true })
    await date(picker, 2024, 2, 29)
    assert.equal(requests.filter(request => request.tool === 'get_agent_usage_stats').length, count, 'draft wheel changes never trigger API requests')
    await picker.getByRole('button', { name: '取消', exact: true }).click(); await closed(picker)
    assert.equal(await trigger.innerText(), before)
    await trigger.click(); picker = page.getByRole('dialog', { name: 'Token 用量日期范围', exact: true })
    await date(picker, 2024, 2, 29)
    await picker.getByRole('button', { name: /结束日期/ }).click()
    await date(picker, 2024, 3, 5)
    if (physical) {
      const wheel = picker.getByRole('listbox', { name: '日', exact: true }), box = await wheel.boundingBox()
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.down()
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2 - 46, { steps: 8 })
      await page.mouse.up()
      await page.waitForFunction(element => element.dataset.moving === 'false', await wheel.elementHandle())
      assert.notEqual(await wheel.locator('[aria-selected="true"]').innerText(), '5日', 'actual drag carries momentum')
      await choose(picker, '日', 5)
      const client = await context.newCDPSession(page)
      await client.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: box.x + box.width / 2, y: box.y + box.height / 2 }] })
      for (let step = 1; step <= 6; step++) await client.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: box.x + box.width / 2, y: box.y + box.height / 2 - step * 10 }] })
      await client.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
      await page.waitForFunction(element => element.dataset.moving === 'false', await wheel.elementHandle())
      assert.notEqual(await wheel.locator('[aria-selected="true"]').innerText(), '5日', 'touch input also carries momentum without scrolling the page')
      await choose(picker, '日', 5)
      await client.detach()
    }
    await geometry(picker, viewport)
    await picker.screenshot({ path: join(screenshots, `${viewport.width}-${theme}-range.png`) })
    await picker.getByRole('button', { name: '完成', exact: true }).click(); await closed(picker)
    assert.match(await trigger.innerText(), /2024\/02\/29 至 2024\/03\/05/)
    assert.ok(requests.slice(requestStart).some(request => request.tool === 'get_agent_usage_stats' && request.start_date === '2024-02-29' && request.end_date === '2024-03-05'))
    assert.equal(requests.filter(request => request.tool === 'get_agent_usage_stats').length, count + 1, 'Confirm applies the whole range with one request')
    assert.equal(await page.locator('.glass-picker:visible').count(), 0)
    if (physical) {
      await trigger.click()
      await page.setViewportSize({ width: 1440, height: 900 })
      await page.locator('.usage-filters .el-date-editor').waitFor()
      assert.equal(await page.locator('.date-wheel-sheet').count(), 0, 'desktop resize removes the wheel sheet')
      assert.equal(await page.evaluate(() => document.documentElement.classList.contains('motion-overlay-open')), false, 'responsive cleanup releases page locks')
    }
    assert.deepEqual(errors, [])
    results.push({ ...viewport, theme, nestedSheets: true, leapDate: true, cancel: true, clear: true, range: true, physicalDrag: physical })
    await context.close()
  }
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  await desktop.goto(`${origin}/knowledge/settings?section=usage`)
  await desktop.locator('.usage-filters .el-date-editor').click()
  await desktop.locator('.glass-picker:visible .el-date-table').first().waitFor()
  assert.equal(await desktop.locator('.mobile-date-trigger').count(), 0, 'desktop still uses its existing calendar')
  await desktop.close()
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, desktopFallback: true, screenshots }, null, 2))
} finally { await browser?.close(); await server.close() }
