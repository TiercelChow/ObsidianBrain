/**
 * Validate the built Pages entry in isolated desktop/mobile browser contexts.
 * No production API, shelves, databases, credentials or model calls.
 * Run npm run build first, then:
 * PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node scripts/verify-wiki-architecture.mjs
 * Optional PLAYWRIGHT_CHANNEL=chromium uses an already installed full Chromium.
 */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { preview } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const output = await mkdtemp(join(tmpdir(), 'wiki-architecture-'))
const server = await preview({
  root,
  configFile: join(root, 'vite.config.js'),
  preview: { host: '127.0.0.1', port: 0, strictPort: true, open: false },
})
const address = server.httpServer.address()
assert.ok(address && typeof address !== 'string')
const origin = `http://127.0.0.1:${address.port}`
const pageUrl = `${origin}/ObsidianBrain/llm-wiki/`
let browser
try {
  browser = await chromium.launch({
    headless: true,
    ...(process.env.PLAYWRIGHT_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHANNEL } : {}),
  })
  for (const viewport of [
    { width: 1440, height: 1000 }, { width: 1024, height: 900 },
    { width: 390, height: 844 }, { width: 320, height: 740 },
  ]) {
    const context = await browser.newContext({ viewport, reducedMotion: 'reduce' })
    const page = await context.newPage()
    const errors = [], external = [], responses = []
    page.on('pageerror', error => errors.push(error.message))
    page.on('response', response => {
      if (response.status() >= 400) responses.push(`${response.status()} ${response.url()}`)
    })
    await context.route('**/*', route => {
      const url = new URL(route.request().url())
      if (url.origin === origin || ['data:', 'about:'].includes(url.protocol)) return route.continue()
      external.push(url.href)
      return route.abort()
    })
    await page.goto(pageUrl, { waitUntil: 'networkidle' })
    await page.evaluate(() => document.fonts.ready)
    assert.equal(await page.locator('.manual-section').count(), 11)
    assert.equal(await page.locator('.diagram').count(), 10)
    assert.equal(await page.locator('.site-nav a[aria-current="page"]').textContent(), 'LLM Wiki')

    for (const theme of ['light', 'dark', 'eye-care']) {
      for (let index = 0; index < 3 && await page.locator('html').getAttribute('data-theme') !== theme; index++) {
        await page.locator('[data-theme-toggle]').click()
      }
      assert.equal(await page.locator('html').getAttribute('data-theme'), theme)
      const overflow = await page.evaluate(() => ({
        width: document.documentElement.clientWidth,
        scroll: document.documentElement.scrollWidth,
      }))
      assert.ok(overflow.scroll <= overflow.width + 1, `page overflow: ${viewport.width}px ${theme}`)
    }
    const outsideDiagrams = await page.locator('.diagram svg').evaluateAll(svgs => svgs.flatMap((svg, index) => {
      const box = svg.viewBox.baseVal
      return [...svg.querySelectorAll('text')].flatMap(text => {
        const bounds = text.getBBox()
        return bounds.x < -1 || bounds.y < -1 || bounds.x + bounds.width > box.width + 1 || bounds.y + bounds.height > box.height + 1
          ? [{ diagram: index, text: text.textContent }] : []
      })
    }))
    assert.deepEqual(outsideDiagrams, [], 'diagram labels must fit their viewBox')
    assert.deepEqual(errors, [], 'no browser exceptions')
    assert.deepEqual(external, [], 'the documentation must not depend on external requests')
    assert.deepEqual(responses, [], 'all built assets must resolve under the Pages base')

    if (viewport.width <= 390) {
      await page.locator('.mobile-chapters summary').click()
      await page.locator('.mobile-chapters a[href="#budgets"]').click()
      await page.waitForFunction(() => location.hash === '#budgets')
      assert.equal(await page.locator('.mobile-chapters').getAttribute('open'), null)
      assert.match(await page.locator('[data-current-chapter]').textContent(), /预算/)
      const figure = page.locator('#budgets .diagram')
      await figure.evaluate(element => { element.scrollLeft = element.scrollWidth })
      assert.ok(await figure.evaluate(element => element.scrollLeft > 0), 'wide diagrams can be read to the right')
      await figure.evaluate(element => { element.scrollLeft = 0 })
      await page.screenshot({ path: join(output, `mobile-${viewport.width}-budgets.png`) })
    } else {
      await page.locator('.docs-sidebar a[href="#qa"]').click()
      await page.waitForFunction(() => location.hash === '#qa')
      assert.equal(await page.locator('.docs-sidebar a[href="#qa"]').getAttribute('aria-current'), 'location')
      await page.screenshot({ path: join(output, `desktop-${viewport.width}-qa.png`) })
    }
    await page.goto(`${pageUrl}#tasks`, { waitUntil: 'networkidle' })
    await page.screenshot({ path: join(output, `research-${viewport.width}.png`) })
    console.log(`PASS ${viewport.width}×${viewport.height}: themes, anchors, diagrams, assets and overflow`)
    await context.close()
  }
  console.log(`Screenshots: ${output}`)
} finally {
  await browser?.close()
  await new Promise((resolve, reject) => server.httpServer.close(error => error ? reject(error) : resolve()))
}
