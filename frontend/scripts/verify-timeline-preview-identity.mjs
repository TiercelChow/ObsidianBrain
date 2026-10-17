/** Slow-image regression with in-memory data only; no real backend or photos. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'timeline-preview-identity-'))
console.log(`Screenshots: ${screenshots}`)
const names = ['red.svg', 'blue.svg', 'green.svg', 'yellow.svg']
const colors = ['#ed2530', '#2872ed', '#27c770', '#d69c24']
const blocked = new Set(), failed = new Set(), held = new Map(), unexpected = []
function image(response, name) {
  if (response.destroyed || response.writableEnded) return
  response.setHeader('Cache-Control', 'no-store')
  if (failed.has(name)) { response.statusCode = 404; response.end(); return }
  response.setHeader('Content-Type', 'image/svg+xml')
  response.end(`<svg xmlns="http://www.w3.org/2000/svg" width="600" height="900"><rect width="600" height="900" fill="${colors[names.indexOf(name)]}"/></svg>`)
}
function release(name) {
  blocked.delete(name)
  for (const response of held.get(name) || []) image(response, name)
  held.delete(name)
}
const mock = {
  name: 'isolated-timeline-preview-identity',
  configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.url?.startsWith('/v1')) return next()
      const picture = request.url.match(/^\/v1\/timeline\/(?:images|thumbnails)\/([^/?]+)$/)?.[1]
      if (picture && names.includes(picture)) {
        if (blocked.has(picture)) { held.set(picture, [...(held.get(picture) || []), response]); return }
        return image(response, picture)
      }
      response.setHeader('Content-Type', 'application/json')
      if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'mock', components: {}, uptime_seconds: 1, tools_count: 0 }))
      let body = ''
      for await (const part of request) body += part
      const { tool } = body ? JSON.parse(body) : {}
      if (request.url === '/v1/tools/call' && tool === 'browse_timeline') return response.end(JSON.stringify({ status: 'success', result: { memos: [{ id: 'mock-memo', content: '隔离图片查看测试', timestamp: new Date().toISOString(), images: names, tags: [], revision: 1 }], total: 1, has_more: false } }))
      unexpected.push(tool || request.url)
      response.statusCode = 501
      response.end(JSON.stringify({ status: 'error', error: { message: 'Real backend forbidden' } }))
    })
  },
}
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, logLevel: 'error', plugins: [mock, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, proxy: {} } })
let browser
const results = []
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const viewport of [{ width: 390, height: 844 }, { width: 320, height: 568 }, { width: 1280, height: 800 }]) {
    const context = await browser.newContext({ viewport, isMobile: viewport.width < 769, hasTouch: viewport.width < 769 })
    const page = await context.newPage()
    const errors = []
    page.on('pageerror', error => errors.push(error.message))
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    await page.goto(`${origin}/timeline`)
    await page.locator('.memo-image').first().waitFor()
    await page.waitForFunction(() => [...document.querySelectorAll('.memo-image')].every(image => image.complete && image.naturalWidth > 0))
    await page.locator('.memo-image').first().click()
    const viewer = page.getByRole('dialog', { name: '小记图片', exact: true })
    const current = viewer.locator('.viewer-slide:not([aria-hidden]) .viewer-preview')
    await viewer.locator('.viewer-image-ready').waitFor()
    await current.evaluate(image => image.decode())
    const old = await current.elementHandle()
    blocked.add('blue.svg')
    blocked.add('green.svg')
    await page.evaluate(() => {
      window.previewSamples = []
      window.samplePreviews = true
      function sample() {
        const counter = document.querySelector('.viewer-counter')?.textContent.replace(/\s/g, '')
        const expected = counter?.startsWith('2/') ? 'blue.svg' : counter?.startsWith('3/') ? 'green.svg' : null
        if (expected) {
          const images = [...document.querySelectorAll('.viewer-slide .viewer-preview')]
          const wrong = images.filter(image => image.naturalWidth && image.currentSrc && new URL(image.currentSrc).pathname !== new URL(image.src).pathname)
          window.previewSamples.push({ expected, wrong: wrong.map(image => ({ source: image.src, bitmap: image.currentSrc })) })
        }
        if (window.samplePreviews) requestAnimationFrame(sample)
      }
      requestAnimationFrame(sample)
    })
    await viewer.getByRole('button', { name: '下一张图片', exact: true }).click()
    await page.waitForFunction(() => document.querySelector('.viewer-counter')?.textContent.replace(/\s/g, '') === '2/4')
    await page.waitForFunction(() => window.previewSamples?.length >= 6)
    await page.screenshot({ path: join(screenshots, `${viewport.width}-pending-blue.png`) })
    assert.equal(await current.evaluate((image, previous) => image === previous, old), false, 'a new photo must not reuse the previous preview img/decoded bitmap')
    const preview = await current.evaluate(image => {
      const canvas = document.createElement('canvas'); canvas.width = canvas.height = 1
      const context = canvas.getContext('2d')
      if (image.naturalWidth) context.drawImage(image, 0, 0, 1, 1)
      return { width: image.naturalWidth, source: image.currentSrc, color: [...context.getImageData(0, 0, 1, 1).data].slice(0, 3) }
    })
    assert.ok(preview.width === 0 || (preview.source.endsWith('blue.svg') && preview.color.join(',') === '40,114,237'), `only the correct cached thumbnail or an empty pending preview is allowed: ${JSON.stringify(preview)}`)
    await viewer.getByRole('button', { name: '下一张图片', exact: true }).click()
    await page.waitForFunction(() => document.querySelector('.viewer-counter')?.textContent.replace(/\s/g, '') === '3/4')
    release('blue.svg')
    await page.waitForTimeout(150) // Keep the active request pending while older responses arrive.
    assert.equal(await viewer.locator('.viewer-image-ready').count(), 0, 'late original must not mark the current image ready')
    const samples = await page.evaluate(() => { window.samplePreviews = false; return window.previewSamples })
    assert.ok(samples.length >= 6)
    assert.deepEqual(samples.flatMap(sample => sample.wrong), [], 'no animation frame retains a bitmap from another thumbnail URL')
    release('green.svg')
    await viewer.locator('.viewer-image-ready').waitFor()
    await current.evaluate(image => image.decode())
    assert.match(await current.evaluate(image => image.currentSrc), /green\.svg$/)
    const stage = await viewer.locator('.viewer-image-wrap').boundingBox()
    await page.mouse.move(stage.x + stage.width * .2, stage.y + stage.height * .5)
    await page.mouse.down()
    await page.mouse.move(stage.x + stage.width * .8, stage.y + stage.height * .5, { steps: 9 })
    await page.mouse.up()
    await page.waitForFunction(() => document.querySelector('.viewer-counter')?.textContent.replace(/\s/g, '') === '2/4')
    await viewer.locator('.viewer-image-ready').waitFor()
    await current.evaluate(image => image.decode())
    assert.match(await current.evaluate(image => image.currentSrc), /blue\.svg$/)
    failed.add('yellow.svg')
    await viewer.getByRole('button', { name: '下一张图片', exact: true }).click()
    await page.waitForFunction(() => document.querySelector('.viewer-counter')?.textContent.replace(/\s/g, '') === '3/4')
    await viewer.getByRole('button', { name: '下一张图片', exact: true }).click()
    await viewer.locator('.viewer-status').waitFor()
    failed.delete('yellow.svg')
    await viewer.getByRole('button', { name: '重试', exact: true }).click()
    await viewer.locator('.viewer-image-ready').waitFor()
    await viewer.getByRole('button', { name: '关闭图片', exact: true }).click()
    await viewer.waitFor({ state: 'hidden' })
    await page.locator('.memo-image').first().click()
    await viewer.locator('.viewer-image-ready').waitFor()
    await current.evaluate(image => image.decode())
    assert.match(await current.evaluate(image => image.currentSrc), /red\.svg$/)
    assert.deepEqual(errors, [])
    results.push({ ...viewport, frameSamples: samples.length, noPreviousBitmap: true, staleResponseIgnored: true, swipe: true, retry: true, reopen: true })
    await context.close()
  }
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, screenshots }, null, 2))
} finally {
  for (const name of names) release(name)
  await browser?.close()
  await server.close()
}
