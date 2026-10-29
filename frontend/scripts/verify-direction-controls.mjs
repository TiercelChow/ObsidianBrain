/** SVG alignment and direction semantics on isolated task/calendar data. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'
import { verifyIconGeometry } from './helpers/verify-icon-geometry.mjs'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url)), screenshots = await mkdtemp(join(tmpdir(), 'direction-controls-'))
console.log(`Screenshots: ${screenshots}`)
const unexpected = [], results = [], runtimeErrors = []
const task = { id: 'fixture-root', root_id: 'fixture-root', parent_id: null, kind: 'long', role: 'root', title: '安排阅读与校对', description: '隔离的方向按钮检查', start_date: '2026-01-01', end_date: '2026-12-31', importance: 'normal', status: 'planned', position: 0, revision: 1, created_at: '2026-10-26T00:00:00Z', updated_at: '2026-10-26T00:00:00Z', archived_at: null, closed_at: null, closure_note: null }
const longTitle = '核对引用：验证很长的中英文任务标题 Long nested task title can wrap without shifting the heading or overflowing the dialog'
const tasks = [task, ...['章节安排', '校对资料', longTitle].map((title, index) => ({ ...task, id: `fixture-${index}`, parent_id: index ? `fixture-${index - 1}` : task.id, role: 'subtask', title })),
  { ...task, id: 'fixture-completed', parent_id: 'fixture-0', role: 'subtask', title: '已完成的校对', status: 'completed' },
  { ...task, id: 'fixture-cancelled', parent_id: 'fixture-0', role: 'subtask', title: '已取消的校对', status: 'cancelled' }]
const state = { storage_path: 'fixture', document_version: { revision: 1, content_hash: 'fixture' }, progress_percent: 0, effective_leaf_count: 1, completed_leaf_count: 0 }
const mock = { name: 'isolated-direction-controls', configureServer(server) {
  server.middlewares.use(async (request, response, next) => {
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'fixture', components: {}, uptime_seconds: 1, tools_count: 0 }))
    let body = ''; for await (const chunk of request) body += chunk
    const { tool, arguments: args = {} } = body ? JSON.parse(body) : {}
    const fixtures = { list_tasks: () => ({ tasks: [{ ...task, ...state }], next_cursor: null }), get_task: () => ({ root: task, tasks, progress: [], audit: [], ...state }), get_task_calendar: () => tasks.map(node => ({ ...node, ...state, start_date: args.start_date, end_date: args.end_date })) }
    if (request.url !== '/v1/tools/call' || !fixtures[tool]) {
      unexpected.push(tool || request.url); response.statusCode = 501
      return response.end(JSON.stringify({ status: 'error', error: { message: 'Real API and mutations forbidden' } }))
    }
    response.end(JSON.stringify({ status: 'success', result: fixtures[tool]() }))
  })
} }
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, cacheDir: join(screenshots, '.vite-cache'), configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
async function centered(locator, phone, verticalOnly = false) {
  await locator.scrollIntoViewIfNeeded()
  const box = await locator.evaluate(element => {
    const button = element.getBoundingClientRect(), icon = element.querySelector('svg')?.getBoundingClientRect()
    const glyph = element.querySelector('.ui-chevron'), state = element.getAttribute('aria-expanded')
    const transform = glyph && new DOMMatrixReadOnly(getComputedStyle(glyph).transform)
    return { svg: !!icon, width: button.width, height: button.height, x: icon ? Math.abs(icon.x + icon.width / 2 - button.x - button.width / 2) : null,
      y: icon ? Math.abs(icon.y + icon.height / 2 - button.y - button.height / 2) : null, padding: getComputedStyle(element).padding, text: element.textContent.trim(),
      reduced: matchMedia('(prefers-reduced-motion: reduce)').matches, state, angle: transform ? Math.atan2(transform.b, transform.a) * 180 / Math.PI : null }
  })
  assert.ok(box.svg && box.y <= .6 && (verticalOnly || box.x <= .6), JSON.stringify(box))
  if (!verticalOnly) assert.equal(box.text, '', 'icon-only directions contain no font glyphs')
  if (box.reduced && box.state != null && box.angle != null) assert.ok(Math.abs(box.angle - (box.state === 'true' ? 90 : 0)) < .1, 'reduced motion changes direction immediately, without an intermediate diagonal')
  if (phone && !verticalOnly) assert.ok(box.width >= 32 && box.height >= 32)
}
async function settledChild(page, dialog, title, expectedLeft) {
  const phone = await dialog.locator('.mobile-task-detail').count() > 0
  const shell = phone ? '.mobile-task-detail.is-child' : '.child-task-dialog'
  await dialog.locator(phone ? 'h2' : 'h3').filter({ hasText: title }).waitFor()
  await page.waitForFunction(shell => document.querySelectorAll(shell).length === 1, shell)
  await dialog.locator(phone ? 'h2' : '.child-task-title').evaluate(async element => {
    await Promise.all(element.closest('.motion-swap').getAnimations({ subtree: true }).map(animation => animation.finished.catch(() => {})))
  })
  const geometry = await dialog.locator(shell).evaluate(element => {
    const box = element.getBoundingClientRect(), heading = element.querySelector('h2, h3').getBoundingClientRect()
    return { left: heading.left - box.left, headingRight: heading.right - box.left, width: box.width,
      leftGap: box.left, rightGap: innerWidth - box.right,
      overflowing: element.scrollWidth > element.clientWidth + 1,
      scroll: element.querySelector('.child-dialog-scroll, .mobile-task-detail-scroll').getBoundingClientRect().height }
  })
  assert.ok(!geometry.overflowing && geometry.headingRight < geometry.width && geometry.scroll > 30, JSON.stringify(geometry))
  assert.ok(Math.abs(geometry.leftGap - geometry.rightGap) < 1, 'the content fits and centers within the modal shell')
  if (expectedLeft != null) assert.ok(Math.abs(geometry.left - expectedLeft) <= .75, `nested headings share the same left inset: ${JSON.stringify(geometry)}`)
  assert.equal(await dialog.locator('footer').count(), phone ? 1 : 0)
  assert.equal(await dialog.getByRole('button', { name: '编辑任务', exact: true }).count(), 1)
  await verifyIconGeometry(page, `nested task ${title}`)
  return geometry.left
}
async function sectionActions(page, dialog, title) {
  const phone = await dialog.locator('.mobile-task-detail').count() > 0
  for (const [region, label, targetLabel] of [['子任务', '添加子任务', '父任务'], ['进展与记录', '记录进展', '应用于']]) {
    if (phone) await dialog.getByRole('tab', { name: region === '子任务' ? /^子任务/ : /^进展/ }).click()
    const section = phone ? dialog : dialog.getByRole('region', { name: region, exact: true })
    const button = section.getByRole('button', { name: label, exact: true })
    await centered(button, true)
    await button.click()
    const editor = page.getByRole('dialog', { name: label, exact: true })
    await editor.waitFor()
    assert.equal(await editor.locator('.sheet-target').innerText(), `${targetLabel}：${title}`, 'the action targets the current nested task')
    await editor.getByRole('button', { name: phone ? '返回子任务详情' : '关闭', exact: true }).click()
    await editor.waitFor({ state: 'hidden' })
    assert.ok(await dialog.isVisible(), 'closing the form preserves child task context')
  }
  if (phone) await dialog.getByRole('tab', { name: /^子任务/ }).click()
}
async function calendarShift(page, button) {
  const loaded = page.waitForResponse(response => response.url().endsWith('/v1/tools/call') && response.request().postDataJSON()?.tool === 'get_task_calendar')
  await button.click()
  await loaded
  await page.locator('.calendar-grid:not(.loading)').waitFor()
  // A previous month's agenda can still be leaving while the new title is
  // already present. Measure the current controls, not a departing DOM node.
  await page.waitForFunction(() => !document.querySelector('.agenda-item-leave-active'))
  await page.locator('.agenda-list').evaluate(async element => {
    await Promise.all(element.getAnimations({ subtree: true }).map(animation => animation.finished.catch(() => {})))
  })
}
let browser, page
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [320, 390, 900, 1440]) for (const theme of ['light', 'dark', 'eye-care']) {
    const phone = width <= 768
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: phone, hasTouch: phone, reducedMotion: width === 390 && theme === 'dark' ? 'no-preference' : 'reduce' })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    page = await context.newPage(); const errors = []; page.on('pageerror', error => { errors.push(error.message); runtimeErrors.push(error.stack) })
    await page.goto(origin + '/tasks')
    const row = page.locator(phone ? '.mobile-task-row' : '.task-card').first()
    await centered(row, phone, true)
    await row.click()
    await page.locator(phone ? '.mobile-task-hero h2' : '.detail-header h2').waitFor()
    if (!phone) {
      const expand = page.getByRole('button', { name: '展开任务列表', exact: true })
      if (await expand.isVisible()) { await centered(expand, phone); await expand.click() }
      const collapse = page.getByRole('button', { name: '收起任务列表', exact: true })
      await centered(collapse, phone); await collapse.click()
      await centered(expand, phone); await expand.click()
      // Restore the selected-task layout before testing the two detail columns.
      await collapse.click()
    }
    if (!phone) {
      const disclosure = page.locator('.disclosure').first()
      await centered(disclosure, phone)
      await disclosure.click(); assert.equal(await disclosure.getAttribute('aria-expanded'), 'false')
      await centered(disclosure, phone)
      await disclosure.click(); assert.equal(await disclosure.getAttribute('aria-expanded'), 'true')
      await centered(disclosure, phone)
    }
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-tree.png`) })
    await page.getByRole('button', { name: phone ? '章节安排，已计划' : '查看子任务：章节安排', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: '子任务详情', exact: true })
    const titleLeft = await settledChild(page, dialog, '章节安排')
    await sectionActions(page, dialog, '章节安排')
    assert.equal(await dialog.locator('.task-status-mark.is-completed svg').count(), 1)
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-child.png`) })
    const childRows = () => dialog.locator(phone ? '.mobile-child-row' : '.child-list button')
    await centered(childRows().first(), phone, true)
    await childRows().first().click()
    await settledChild(page, dialog, '校对资料', titleLeft)
    await childRows().first().click()
    await settledChild(page, dialog, longTitle, titleLeft)
    assert.equal(await childRows().count(), 0)
    await sectionActions(page, dialog, longTitle)
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-grandchild.png`) })
    const back = dialog.getByRole('button', { name: '返回上一级', exact: true })
    await centered(back, phone); await back.click()
    await settledChild(page, dialog, '校对资料', titleLeft)
    await back.click()
    await dialog.locator(phone ? 'h2' : '.child-dialog-header h3').filter({ hasText: '章节安排' }).waitFor()
    await settledChild(page, dialog, '章节安排', titleLeft)
    await dialog.getByRole('button', { name: '已完成的校对', exact: false }).click()
    await settledChild(page, dialog, '已完成的校对', titleLeft)
    assert.equal(await dialog.getByRole('button', { name: '添加子任务', exact: true }).count(), 0)
    assert.equal(await dialog.getByRole('button', { name: '记录进展', exact: true }).count(), 0)
    await dialog.getByRole('button', { name: '关闭子任务详情', exact: true }).click()
    await dialog.waitFor({ state: 'hidden' })

    if (!phone) {
      await page.getByRole('button', { name: '编辑任务', exact: true }).first().click()
      const editor = page.getByRole('dialog', { name: '编辑任务', exact: true })
      await editor.locator('.task-date-input input').first().click()
      await page.locator('.task-date-popper:visible').waitFor()
      const calendarArrows = page.locator('.task-date-popper:visible .el-picker-panel__icon-btn')
      assert.equal(await calendarArrows.count(), 4)
      for (const arrow of await calendarArrows.all()) await centered(arrow, false)
      await page.keyboard.press('Escape')
      await editor.getByRole('button', { name: '关闭', exact: true }).click()
      await editor.waitFor({ state: 'hidden' })
    }
    await page.goto(origin + '/tasks?view=calendar')
    const title = page.locator('.calendar-heading h2'), initial = await title.innerText()
    const previous = page.getByRole('button', { name: '上个月', exact: true }), next = page.getByRole('button', { name: '下个月', exact: true })
    await centered(previous, phone); await centered(next, phone)
    await calendarShift(page, next); await page.waitForFunction(initial => document.querySelector('.calendar-heading h2')?.textContent !== initial, initial)
    await calendarShift(page, previous); await page.waitForFunction(initial => document.querySelector('.calendar-heading h2')?.textContent === initial, initial)
    const agenda = page.locator('.agenda-expand').first()
    await centered(agenda, phone); await agenda.click()
    assert.equal(await agenda.getAttribute('aria-expanded'), 'true')
    await centered(agenda, phone)
    await page.screenshot({ path: join(screenshots, `${width}-${theme}-calendar.png`) })
    assert.deepEqual(errors, []); assert.deepEqual(unexpected, [])
    results.push({ width, theme, svgCentered: true, treeToggle: true, childBack: true, stableNestedTitles: true, contextualCreation: true, closedTask: true, monthNavigation: true, agendaToggle: true })
    await context.close()
  }
  console.log(JSON.stringify({ passed: true, results, screenshots }, null, 2))
} catch (error) {
  console.error(JSON.stringify({ results, unexpected, runtimeErrors, body: await page?.locator('body').innerText().catch(() => '') }))
  await page?.screenshot({ path: join(screenshots, 'failure.png') }).catch(() => {})
  throw error
} finally { await browser?.close(); await server.close() }
