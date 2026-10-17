/** Home Wiki layout regression; all API responses stay in memory, never uses user data. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'home-wiki-density-'))
const unexpected = []
const timestamp = '2026-10-18T00:00:00Z'
const items = [
  { id: 'compile', kind: 'compile', title: '知识编译', status: 'compiling', detail: '', artifact_state: null },
  { id: 'research', kind: 'research', title: '主题研究', status: 'running', detail: '', artifact_state: null },
  { id: 'review', kind: 'review', title: '跨章知识归并', status: 'proposed', detail: '新增论断与关系，等待审核后应用。', artifact_state: null },
  { id: 'presentation', kind: 'research', title: '跨章节研究：机制、证据与适用边界的综合梳理', status: 'completed', detail: '', artifact_state: 'failed' },
  { id: 'failed', kind: 'compile', title: '知识编译', status: 'failed', detail: '需要核对长章节中的引用与条件，保留已完成的知识候选，并在知识库页面查看失败原因。'.repeat(2), artifact_state: null },
].map((item, index) => ({ ...item, base_id: `base-${index}`, book_name: index === 4 ? '很长的书籍名称：机制与条件'.repeat(8) : '学习笔记', updated_at: timestamp }))
const overview = {
  today: '2026-10-18', week_start: '2026-10-12', generated_at: timestamp,
  system: { version: 'mock', uptime_seconds: 300, components: { server: 'ok', sqlite: 'ok' } },
  tasks: { data: { active_count: 0, today_count: 0, overdue_count: 0, items: [] }, error: null },
  reading: { data: [], error: null }, memos: { data: { total_count: 0, week_count: 0, items: [] }, error: null },
  wiki: { data: { running_count: 2, review_count: 1, failed_count: 2, items }, error: null },
  storage: { data: { originals_bytes: 0, cache_bytes: 0, cache_limit_bytes: 268435456, pending_cleanup: 0 }, error: null },
}
const mockApi = {
  name: 'isolated-home-wiki-density',
  configureServer(server) {
    server.middlewares.use((request, response, next) => {
      if (!request.url?.startsWith('/v1')) return next()
      response.setHeader('Content-Type', 'application/json')
      if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'mock', components: {}, uptime_seconds: 300, tools_count: 0 }))
      if (request.url.startsWith('/v1/home/overview?')) return response.end(JSON.stringify(overview))
      unexpected.push(request.url)
      response.statusCode = 501
      response.end(JSON.stringify({ error: 'Real backend forbidden' }))
    })
  },
}
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, logLevel: 'error', plugins: [mockApi, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser
const results = []
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const theme of ['light', 'dark', 'eye-care']) for (const width of [320, 390, 900, 1440]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: width <= 768, hasTouch: width <= 768, reducedMotion: 'reduce' })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    const page = await context.newPage(), errors = []
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(origin)
    await page.locator('.wiki-row').last().waitFor()
    await page.evaluate(() => document.getAnimations().forEach(animation => animation.finish()))
    const geometry = await page.locator('.wiki-row').evaluateAll(rows => rows.map(row => {
      const title = row.querySelector('.wiki-row-heading strong'), status = row.querySelector('.wiki-status')
      const rect = row.getBoundingClientRect(), heading = title.getBoundingClientRect(), pill = status.getBoundingClientRect()
      const content = row.querySelector('.row-content').getBoundingClientRect()
      // Reconstruct the previous three-line hierarchy at exactly the same width.
      const legacy = row.cloneNode(true)
      legacy.classList.remove('wiki-row')
      legacy.setAttribute('aria-hidden', 'true')
      const legacyBody = legacy.querySelector('.row-content'), legacyHeading = legacy.querySelector('.wiki-row-heading')
      const legacyTitle = legacyHeading.querySelector('strong'), legacyStatus = legacyHeading.querySelector('.wiki-status')
      legacyStatus.classList.remove('wiki-status')
      legacyBody.insertBefore(legacyTitle, legacyHeading)
      legacyBody.querySelector('.row-meta').after(legacyStatus)
      legacyHeading.remove()
      row.parentElement.append(legacy)
      const oldHeight = legacy.getBoundingClientRect().height
      legacy.remove()
      return { height: rect.height, oldHeight, inline: heading.right <= pill.left + 1 && Math.max(heading.top, pill.top) < Math.min(heading.bottom, pill.bottom),
        fits: content.left >= rect.left && pill.right <= content.right + 1 && status.scrollWidth <= status.clientWidth + 1,
        radius: getComputedStyle(status).borderRadius, text: status.textContent, book: row.querySelector('.row-meta').textContent,
        href: row.getAttribute('href'), title: title.textContent, titleHint: title.getAttribute('title') }
    }))
    for (const [index, row] of geometry.entries()) {
      assert.ok(row.height >= 44 && row.height < row.oldHeight, `Wiki row should shrink without losing its tap target: ${JSON.stringify({ width, theme, index, ...row })}`)
      assert.ok(row.inline && row.fits, `title and full pill stay side by side: ${JSON.stringify({ width, index, ...row })}`)
      assert.equal(row.radius, '999px')
      assert.equal(row.title, items[index].title)
      assert.equal(row.titleHint, items[index].title)
      assert.equal(row.book, items[index].book_name)
      assert.ok(row.href.includes(`base=base-${index}`), 'read-only base deep link survives')
    }
    assert.equal(geometry[3].text, '报告已完成 · PPT 失败', 'partial delivery is never hidden or abbreviated away')
    assert.ok(geometry[0].height <= 66 && geometry[1].height <= 66, 'ordinary Wiki rows fit within 64px at all viewport sizes')
    const overflow = await page.locator('.wiki-panel').evaluate(panel => panel.scrollWidth > panel.clientWidth + 1)
    assert.equal(overflow, false, 'long names and descriptions do not widen the panel')
    await page.locator('.wiki-panel').screenshot({ path: join(screenshots, `${width}-${theme}.png`) })
    assert.deepEqual(errors, [])
    results.push({ width, theme, heights: geometry.map(row => row.height), previousHeights: geometry.map(row => row.oldHeight) })
    await context.close()
  }
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, screenshots }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
