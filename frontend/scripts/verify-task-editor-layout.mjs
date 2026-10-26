/** Isolated create/edit/subtask date geometry; no real API or saved changes. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'
import { verifyIconGeometry } from './helpers/verify-icon-geometry.mjs'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'task-editor-layout-'))
console.log(`Screenshots: ${screenshots}`)
const unexpected = [], results = []
const task = { id: 'fixture-task', root_id: 'fixture-task', parent_id: null, kind: 'long', role: 'root', title: '任务日期排版测试', description: '只读内存夹具，不保存任何真实任务。', start_date: '2026-10-26', end_date: '2026-12-31', importance: 'normal', status: 'planned', position: 0, revision: 1, archived_at: null, closed_at: null, closure_note: null }
const state = { storage_path: 'fixture', document_version: { revision: 1, content_hash: 'fixture' }, progress_percent: 0, effective_leaf_count: 1, completed_leaf_count: 0 }
const mock = { name: 'isolated-task-editor-layout', configureServer(server) {
  server.middlewares.use(async (request, response, next) => {
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'fixture', components: {}, uptime_seconds: 1, tools_count: 0 }))
    let body = ''; for await (const chunk of request) body += chunk
    const { tool } = body ? JSON.parse(body) : {}
    const fixtures = { list_tasks: { tasks: [{ ...task, ...state }], next_cursor: null }, get_task: { root: task, tasks: [task], progress: [], audit: [], ...state } }
    if (request.url !== '/v1/tools/call' || !fixtures[tool]) {
      unexpected.push(tool || request.url); response.statusCode = 501
      return response.end(JSON.stringify({ status: 'error', error: { message: 'Real API and writes forbidden' } }))
    }
    response.end(JSON.stringify({ status: 'success', result: fixtures[tool] }))
  })
} }
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, cacheDir: join(screenshots, '.vite-cache'), configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser, page
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [1440, 1024, 769, 390, 320, 667]) for (const theme of ['light', 'dark', 'eye-care']) {
    const phone = width <= 768, height = width === 667 ? 375 : 900
    const context = await browser.newContext({ viewport: { width, height }, isMobile: phone, hasTouch: phone, reducedMotion: 'reduce' })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    page = await context.newPage()
    const errors = []; page.on('pageerror', error => errors.push(error.message))
    for (const mode of ['create', 'edit', 'subtask']) {
      await page.goto(origin + (mode === 'create' ? '/tasks' : '/tasks?task=' + task.id))
      const action = mode === 'create' ? '新建任务' : mode === 'edit' ? '编辑任务' : '添加子任务'
      await page.getByRole('button', { name: action, exact: true }).filter({ visible: true }).first().click()
      const dialog = page.getByRole('dialog', { name: mode === 'create' ? '新建任务' : action, exact: true })
      await dialog.locator('.task-date-input').first().waitFor()
      await dialog.locator('.task-date-input').first().scrollIntoViewIfNeeded()
      await page.screenshot({ path: join(screenshots, `${width}-${theme}-${mode}.png`) })
      await verifyIconGeometry(page, `${width}-${theme}-${mode} task editor`)
      const dates = await dialog.locator('.task-date-input').evaluateAll(elements => elements.map(element => {
        const trigger = element.querySelector('.mobile-date-trigger') || element.querySelector('.el-input__wrapper') || element
        const value = element.querySelector('.mobile-date-value') || element.querySelector('input')
        const icon = element.querySelector('.el-icon'), rect = trigger.getBoundingClientRect(), text = value.getBoundingClientRect(), symbol = icon.getBoundingClientRect()
        const style = getComputedStyle(value), fieldElement = element.closest('.field'), field = fieldElement.getBoundingClientRect()
        const next = [...fieldElement.parentElement.children].filter(node => node.matches('.field')).map(node => node.getBoundingClientRect()).filter(box => box.top > field.top + 1).sort((a, b) => a.top - b.top)[0]
        const canvas = document.createElement('canvas'), painter = canvas.getContext('2d'); painter.font = style.font
        return { leftInset: symbol.left - rect.left, iconGap: text.left - symbol.right, rightInset: rect.right - text.right, width: rect.width,
          height: rect.height, inside: rect.left >= field.left - 1 && rect.right <= field.right + 1 && rect.bottom <= field.bottom + 1, nextGap: next ? next.top - rect.bottom : null,
          innerBorder: style.borderLeftWidth, innerBackground: style.backgroundColor, available: value.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight),
          textWidth: painter.measureText(value.value || value.textContent).width, viewport: document.documentElement.scrollWidth <= innerWidth + 1 }
      }))
      assert.equal(dates.length, 2)
      for (const date of dates) {
        assert.ok(date.leftInset >= 12 && date.iconGap >= 9, `separate edge/icon/value: ${JSON.stringify({ width, mode, date })}`)
        assert.ok(date.inside && date.viewport && date.available + 1 >= date.textWidth, `unclipped date: ${JSON.stringify(date)}`)
        assert.ok(date.nextGap == null || date.nextGap >= 13, `preserve the full next-row gap: ${JSON.stringify(date)}`)
        assert.equal(date.innerBorder, '0px', 'the inner date input is not another bordered field')
        assert.equal(date.innerBackground, 'rgba(0, 0, 0, 0)', 'only the outer field owns the material')
        assert.ok(date.height >= (phone ? 48 : 44))
      }
      assert.ok(Math.abs(dates[0].width - dates[1].width) <= 1, 'start/end fields have equal widths')
      if (width === 1440 && theme === 'light') {
        const input = dialog.locator('.task-date-input input').first(), before = await input.inputValue()
        await input.click()
        await page.locator('.task-date-popper:visible').waitFor()
        assert.equal(await input.evaluate(element => getComputedStyle(element).borderLeftWidth), '0px', 'focus never recreates an inner field')
        await page.keyboard.press('Escape')
        await page.locator('.task-date-popper:visible').waitFor({ state: 'hidden' })
        assert.equal(await input.inputValue(), before)
        assert.ok(await dialog.isVisible(), 'calendar dismissal preserves the editor')
      }
      if (width === 390 && theme === 'light') {
        await dialog.getByRole('button', { name: '开始日期', exact: true }).click()
        const picker = page.getByRole('dialog', { name: '开始日期', exact: true })
        await picker.getByRole('listbox', { name: '月', exact: true }).waitFor()
        await picker.getByRole('button', { name: '取消', exact: true }).click()
        await picker.waitFor({ state: 'hidden' })
        assert.ok(await dialog.isVisible(), 'nested wheel cancel preserves editor')
      }
      await dialog.getByRole('button', { name: '关闭', exact: true }).click()
      await dialog.waitFor({ state: 'hidden' })
    }
    assert.deepEqual(errors, []); results.push({ width, height, theme, create: true, edit: true, subtask: true })
    await context.close()
  }
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, screenshots }, null, 2))
} catch (error) {
  console.error(JSON.stringify({ unexpected, url: page?.url(), body: await page?.locator('body').innerText().catch(() => '') }))
  throw error
} finally { await browser?.close(); await server.close() }
