/** Real browser layout/gesture regression. Never connects to the user's backend. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'
import { verifyIconGeometry } from './helpers/verify-icon-geometry.mjs'
import { mermaidSamples } from '../src/dev/mermaidSamples.ts'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'mermaid-presentation-'))
const fixtureRoot = '/isolated-mermaid-fixture'
const samplePath = sample => `${fixtureRoot}/${sample.id}.md`
const unexpected = []
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const mock = { name: 'isolated-mermaid-verification', configureServer(server) {
  server.middlewares.use(async (request, response, next) => {
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'mock', components: {}, uptime_seconds: 1, tools_count: 0 }))
    let body = ''; for await (const chunk of request) body += chunk
    const { tool, arguments: args = {} } = body ? JSON.parse(body) : {}
    const sample = mermaidSamples.find(sample => samplePath(sample) === args.path)
    const fixtures = {
      get_reader_books: () => ({ books: mermaidSamples.map(sample => ({ id: sample.id, path: fixtureRoot, kind: 'folder', name: sample.title, description: '', category: '', addedAt: 1, progress: { lastFile: samplePath(sample), position: 0, updatedAt: 1 } })) }),
      list_local_dir: () => ({ root: fixtureRoot, entries: mermaidSamples.map(sample => ({ name: `${sample.id}.md`, path: samplePath(sample), is_dir: false, is_markdown: true, is_pdf: false })), total: mermaidSamples.length }),
      read_local_file: () => ({ path: args.path, name: `${sample?.id}.md`, content: `# ${sample?.title}\n\n正文中的图表，需要保持文字和公式完整。\n\n\`\`\`mermaid\n${sample?.source}\n\`\`\`\n`, size: sample?.source.length || 0 }),
      save_reader_progress: () => ({ state: args.state }),
      get_reader_history: () => ({ history: [] }),
    }
    if (!fixtures[tool]) { unexpected.push(tool || request.url); response.statusCode = 501; return response.end(JSON.stringify({ status: 'error', error: { message: 'Real backend forbidden' } })) }
    response.end(JSON.stringify({ status: 'success', result: fixtures[tool]() }))
  })
} }
const server = await createServer({ ...config.config, root, cacheDir: join(screenshots, '.vite-cache'), configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser
const results = []
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.route('**/*', route => new URL(route.request().url()).origin === origin || route.request().url().startsWith('blob:') ? route.continue() : route.abort())
  for (const sample of mermaidSamples.filter(sample => !process.env.MERMAID_SAMPLE || sample.id === process.env.MERMAID_SAMPLE)) {
    await page.goto(`${origin}/dev/mermaid?sample=${sample.id}`)
    const diagram = page.locator('[data-engine="optimized"] .mermaid')
    await diagram.waitFor()
    await page.waitForFunction(() => document.querySelector('[data-engine="optimized"] .mermaid')?.dataset.processed, null, { timeout: 60000 })
    await page.evaluate(() => document.fonts.ready)
    const state = await diagram.getAttribute('data-processed')
    assert.equal(state, sample.invalid ? 'error' : 'true', `${sample.id} renders without a Mermaid error`)
    if (sample.invalid) {
      await diagram.getByText('查看源码', { exact: true }).click()
      assert.equal(await diagram.locator('pre').innerText(), await diagram.getAttribute('data-raw'))
    } else {
      const geometry = await diagram.evaluate(element => {
        const svg = element.querySelector('svg'), viewBox = svg.viewBox.baseVal
        const labels = [...svg.querySelectorAll('foreignObject')].map(label => {
          const text = label.firstElementChild, box = text?.getBoundingClientRect(), container = label.getBoundingClientRect()
          return { text: text?.textContent, width: box?.width, available: container.width, height: box?.height, availableHeight: container.height, font: text && getComputedStyle(text).font, html: label.outerHTML.slice(0, 700) }
        })
        const rect = svg.getBoundingClientRect(), css = getComputedStyle(element)
        return { width: viewBox.width, height: viewBox.height, labels, overflow: document.documentElement.scrollWidth > window.innerWidth + 1,
          displayed: { width: rect.width, height: rect.height }, available: element.clientWidth - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight),
          inlineScrollWidth: element.scrollWidth, inlineClientWidth: element.clientWidth,
        }
      })
      assert.ok(geometry.width > 0 && geometry.height > 0, `${sample.id} has a valid viewBox`)
      assert.equal(geometry.overflow, false, `${sample.id} never widens the app`)
      assert.ok(geometry.displayed.width <= geometry.available + 1, `${sample.id} fits the complete diagram inside the text width`)
      assert.ok(geometry.inlineScrollWidth <= geometry.inlineClientWidth + 1, `${sample.id} needs no horizontal scrolling`)
      assert.ok(Math.abs(geometry.displayed.height - geometry.displayed.width * geometry.height / geometry.width) < 1, `${sample.id} scales the entire viewBox without cropping or distortion: ${JSON.stringify({ displayed: geometry.displayed, width: geometry.width, height: geometry.height })}`)
      for (const label of geometry.labels) {
        assert.ok(label.width <= label.available + 2, `${sample.id}: label width ${JSON.stringify(label)}`)
        assert.ok(label.height <= label.availableHeight + 2, `${sample.id}: label height ${JSON.stringify(label)}`)
      }
      if (sample.id.includes('math') || sample.id === 'matrix') assert.ok(await diagram.locator('.katex').count() > 0, 'math survives rendering')
      if (sample.id === 'matrix') assert.ok((await diagram.locator('math mtable').evaluateAll(tables => tables.map(table => [...table.children].filter(row => row.tagName === 'mtr').length))).includes(2), 'the matrix retains two mathematical rows, not merely a successful KaTeX span')
      if (sample.id === 'overview') await page.screenshot({ path: join(screenshots, 'desktop-comparison.png'), fullPage: true })
      if (sample.id === 'overview') {
        const paths = await diagram.locator('.flowchart-link').evaluateAll(elements => elements.map(element => element.getAttribute('d')))
        assert.ok(paths.length > 0 && paths.every(path => !/[CQST]/i.test(path)), 'default routing never adds spline overshoot to ELK bend points')
        const reportPath = await diagram.locator('.flowchart-link').evaluateAll(elements => elements.find(element => element.id.includes('C_E'))?.getAttribute('d'))
        assert.ok(reportPath, 'the reported decision → research report branch exists')
        const numbers = reportPath.match(/-?\d*\.?\d+(?:e[-+]?\d+)?/gi).map(Number)
        const heights = numbers.filter((_, index) => index % 2 === 1)
        assert.ok(heights.slice(1).every(y => Math.abs(y - heights[heights.length - 1]) < 0.5), `the report branch has no short staircase after leaving the diamond: ${reportPath}`)
      }
    }
    results.push({ sample: sample.id, state })
  }
  for (const theme of ['light', 'dark', 'eye-care']) {
    await page.goto(`${origin}/dev/mermaid?sample=math`)
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; localStorage.setItem('theme', theme) }, theme)
    await page.reload()
    const diagram = page.locator('[data-engine="optimized"] .mermaid[data-processed="true"]')
    await diagram.waitFor({ timeout: 60000 })
    assert.equal(await diagram.getAttribute('data-diagram-theme'), theme)
    await page.screenshot({ path: join(screenshots, `desktop-${theme}.png`), fullPage: true })
  }
  // The lab is not enough: also run inside the actual Reader, where prose CSS,
  // page transitions, reading progress and the mobile dock are all active.
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport)
    for (const id of ['overview', 'dense', 'chinese', 'matrix', 'class', 'sequence-math']) {
      await page.goto(`${origin}/reader?book=${id}&view=read`)
      const graph = page.locator('article.markdown-body .mermaid[data-processed="true"]')
      await graph.waitFor({ timeout: 60000 })
      await page.evaluate(() => document.fonts.ready)
      const clipped = await graph.evaluate(element => [...element.querySelectorAll('foreignObject')].flatMap(label => {
        const content = label.firstElementChild?.getBoundingClientRect(), box = label.getBoundingClientRect()
        return content && (content.width > box.width + 2 || content.height > box.height + 2) ? [{ text: label.textContent, content: { width: content.width, height: content.height }, box: { width: box.width, height: box.height }, html: label.outerHTML.slice(0, 1000), math: [...label.querySelectorAll('.katex-display, .katex, .mtable')].map(node => ({ class: node.className, font: getComputedStyle(node).font, display: getComputedStyle(node).display, width: getComputedStyle(node).width, minWidth: getComputedStyle(node).minWidth, lineHeight: getComputedStyle(node).lineHeight, margin: getComputedStyle(node).margin, padding: getComputedStyle(node).padding })) }] : []
      }))
      assert.deepEqual(clipped, [], `${id}: actual Reader labels are not clipped at ${viewport.width}px`)
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1), false)
      assert.ok(await graph.evaluate(element => element.scrollWidth <= element.clientWidth + 1), `${id}: Reader shows the complete graph without horizontal panning`)
      if (id === 'matrix') await page.screenshot({ path: join(screenshots, `reader-${viewport.width}.png`), fullPage: true })
    }
  }
  const mobileContext = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })
  const mobile = await mobileContext.newPage()
  await mobile.goto(`${origin}/dev/mermaid?sample=dense`)
  const diagram = mobile.locator('[data-engine="optimized"] .mermaid[data-processed="true"]')
  await diagram.waitFor({ timeout: 60000 })
  await diagram.click({ position: { x: 24, y: 50 } })
  const viewer = mobile.getByRole('dialog', { name: '复杂连线', exact: true })
  await viewer.waitFor()
  await mobile.waitForFunction(() => !!document.querySelector('.mv-canvas')?.style.transform)
  await mobile.waitForFunction(() => document.querySelector('.mermaid-viewer').getAnimations().every(animation => animation.playState === 'finished'))
  assert.equal(await viewer.locator('.mv-header, .mv-heading').count(), 0, 'there is no visible title band')
  const fullStage = await viewer.locator('.mv-stage').boundingBox()
  assert.deepEqual(fullStage, { x: 0, y: 0, width: 390, height: 844 }, 'the canvas uses the full viewport beneath floating controls')
  for (const selector of ['.mv-close-surface', '.mv-controls']) {
    const material = await viewer.locator(selector).evaluate(element => ({ glass: element.dataset.glass, filter: getComputedStyle(element).backdropFilter, lensFilter: element.querySelector('.glass-optics') && getComputedStyle(element.querySelector('.glass-optics')).backdropFilter }))
    assert.equal(material.glass, 'floating')
    assert.ok(material.filter.includes('blur(') || material.lensFilter?.includes('blur('), `${selector} reuses the system optical material`)
  }
  const getScale = () => mobile.locator('.mv-zoom-label').innerText()
  assert.ok(parseInt(await getScale()) >= 85, 'mobile opens at readable size')
  await viewer.getByRole('button', { name: '完整查看', exact: true }).click()
  const fitScale = parseInt(await getScale())
  assert.ok(fitScale < 85, 'overview fits a large diagram')
  await viewer.getByRole('button', { name: '可读尺寸', exact: true }).click()
  assert.ok(parseInt(await getScale()) > fitScale)
  await mobile.screenshot({ path: join(screenshots, 'mobile-viewer.png') })
  await verifyIconGeometry(mobile, 'Mermaid close and zoom controls')
  const stage = await viewer.locator('.mv-stage').boundingBox()
  const client = await mobileContext.newCDPSession(mobile)
  const point = { x: stage.x + stage.width / 2, y: stage.y + stage.height / 2 }
  const beforePan = await viewer.locator('.mv-canvas').getAttribute('style')
  await client.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [point] })
  for (let step = 1; step <= 8; step++) await client.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: point.x - step * 12, y: point.y - step * 8 }] })
  await client.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
  assert.notEqual(await viewer.locator('.mv-canvas').getAttribute('style'), beforePan, 'touch drag pans the diagram')
  const beforePinch = parseInt(await getScale())
  await client.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: point.x - 30, y: point.y }, { x: point.x + 30, y: point.y }] })
  for (let step = 1; step <= 8; step++) await client.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: point.x - 30 - step * 5, y: point.y }, { x: point.x + 30 + step * 5, y: point.y }] })
  await client.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
  await mobile.waitForFunction(scale => parseInt(document.querySelector('.mv-zoom-label')?.textContent) > scale, beforePinch)
  for (const viewport of [{ width: 320, height: 568 }, { width: 844, height: 390 }]) {
    await mobile.setViewportSize(viewport)
    await mobile.waitForFunction(size => document.querySelector('.mv-stage').clientWidth === size.width && document.querySelector('.mv-stage').clientHeight === size.height, viewport)
    const controls = await viewer.locator('.mv-controls').boundingBox()
    assert.ok(controls.x >= 0 && controls.x + controls.width <= viewport.width + 1, 'all viewer controls fit')
    assert.ok(controls.y >= 0 && controls.y + controls.height <= viewport.height + 1, 'floating controls remain inside safe viewport edges')
  }
  const ids = await mobile.locator('[id]').evaluateAll(elements => elements.map(element => element.id))
  assert.equal(new Set(ids).size, ids.length, 'fullscreen SVG IDs never collide with the inline graph')
  await viewer.getByRole('button', { name: '关闭图表查看器', exact: true }).click()
  await viewer.waitFor({ state: 'hidden' })
  assert.equal(await mobile.locator('.mermaid-measure-host').count(), 0, 'measurement hosts are released')
  assert.equal(await mobile.evaluate(() => document.documentElement.classList.contains('motion-overlay-open')), false, 'viewer releases scroll locks')
  assert.deepEqual(errors, [])
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, themes: 3, readerDesktopAndMobile: true, mobilePan: true, mobilePinch: true, screenshots }, null, 2))
} finally { await browser?.close(); await server.close() }
