/** Built Pages acceptance, isolated from production APIs, data and accounts.
 * Run npm run build first. Optional PLAYWRIGHT_MODULE and SCREENSHOT_DIR.
 */
import assert from 'node:assert/strict'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { preview } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const output = process.env.SCREENSHOT_DIR || await mkdtemp(join(tmpdir(), 'website-glass-'))
await mkdir(output, { recursive: true })
const server = await preview({ root, configFile: join(root, 'vite.config.js'), preview: { host: '127.0.0.1', port: 0, strictPort: true, open: false } })
const address = server.httpServer.address()
assert.ok(address && typeof address !== 'string')
const origin = `http://127.0.0.1:${address.port}`
const base = `${origin}/ObsidianBrain/`
const results = []
let browser
try {
  browser = await chromium.launch({ headless: true, channel: process.env.PLAYWRIGHT_CHANNEL || 'chromium' })
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 1024, height: 900 }, { width: 390, height: 844 }, { width: 667, height: 375 }, { width: 320, height: 740 }]) {
    const context = await browser.newContext({ viewport, reducedMotion: 'reduce' })
    const page = await context.newPage()
    const errors = [], external = [], badAssets = []
    page.on('pageerror', error => errors.push(error.message))
    page.on('response', response => { if (response.status() >= 400) badAssets.push(response.url()) })
    await context.route('**/*', route => {
      const url = new URL(route.request().url())
      if (url.origin === origin || ['data:', 'about:'].includes(url.protocol)) return route.continue()
      external.push(url.href)
      return route.abort()
    })
    const probe = async (selector, role) => page.locator(selector).first().evaluate(async (element, role) => {
      getComputedStyle(element).backgroundColor
      await Promise.all(element.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => {})))
      const expected = document.createElement('span')
      expected.style.cssText = `background:var(--glass-${role}-fill);color:var(--text-primary);backdrop-filter:var(--glass-${role}-filter)`
      document.body.append(expected)
      const actual = getComputedStyle(element), reference = getComputedStyle(expected)
      const result = { fill: actual.backgroundColor, expectedFill: reference.backgroundColor, filter: actual.backdropFilter, expectedFilter: reference.backdropFilter }
      expected.remove()
      return result
    }, role)
    const bounded = async locator => {
      const rect = await locator.boundingBox()
      assert.ok(rect && rect.x >= -1 && rect.x + rect.width <= viewport.width + 1, `control fits ${viewport.width}: ${JSON.stringify(rect)}`)
    }
    const screenshot = name => page.screenshot({ path: join(output, `${viewport.width}x${viewport.height}-${name}.png`), animations: 'disabled' })
    for (const path of ['', 'manual/', 'llm-wiki/']) {
      await page.goto(`${base}${path}`, { waitUntil: 'networkidle' })
      for (const theme of ['light', 'dark', 'eye-care']) {
        for (let index = 0; index < 3 && await page.locator('html').getAttribute('data-theme') !== theme; index++) await page.locator('[data-theme-toggle]').click()
        assert.equal(await page.locator('html').getAttribute('data-theme'), theme)
        const header = await probe('.header-inner', 'floating')
        assert.equal(header.fill, header.expectedFill, 'header uses application floating tint')
        assert.equal(header.filter, header.expectedFilter, 'header uses application floating optics')
        assert.equal(await page.locator('.header-inner').evaluate(element => getComputedStyle(element, '::before').pointerEvents), 'none', 'rim cannot intercept pointer input')
        await bounded(page.locator('.header-inner'))
        for (const button of await page.locator('.header-actions > *:visible').all()) await bounded(button)
        const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
        assert.ok(overflow <= 1, 'page stays inside viewport')
        assert.equal(await page.evaluate(() => getComputedStyle(document.body, '::before').content), 'none', 'no background light pools')
        if (viewport.width <= 820) {
          await page.locator('[data-menu-toggle]').click()
          await page.locator('.site-nav[data-open="true"]').waitFor()
          await bounded(page.locator('.site-nav'))
          const menu = await probe('.site-nav', 'panel')
          assert.equal(menu.fill, menu.expectedFill)
          assert.equal(menu.filter, menu.expectedFilter)
          assert.equal((await probe('.header-inner', 'panel')).filter, 'none', 'open menu never samples through another filtered parent')
          await screenshot(`${path ? path.slice(0, -1) : 'home'}-${theme}-menu`)
          await page.keyboard.press('Escape')
          assert.equal(await page.locator('[data-menu-toggle]').getAttribute('aria-expanded'), 'false')
        }
        if (!path) {
          const feature = await probe('.feature-card', 'content')
          assert.equal(feature.fill, feature.expectedFill)
          assert.equal(feature.filter, 'none', 'content cards do not stack filters')
          assert.equal((await probe('.app-window', 'content')).filter, 'none')
          const action = await page.locator('.hero-actions .button-primary').evaluate(element => {
            const expected = document.createElement('span')
            expected.style.cssText = 'background:var(--glass-action-fill);color:var(--glass-action-label)'
            document.body.append(expected)
            const actual = getComputedStyle(element), reference = getComputedStyle(expected)
            const result = { fill: actual.backgroundColor, expectedFill: reference.backgroundColor, label: actual.color, expectedLabel: reference.color, filter: actual.backdropFilter }
            expected.remove()
            return result
          })
          assert.equal(action.fill, action.expectedFill, 'primary action is neutral optical glass')
          assert.equal(action.label, action.expectedLabel)
          assert.equal(action.filter, 'none', 'buttons do not sample backdrop individually')
          await screenshot(`home-${theme}`)
          await page.locator('#features').scrollIntoViewIfNeeded()
          await screenshot(`home-${theme}-features`)
          await page.locator('[data-feature-target="timeline"]').click()
          assert.equal(await page.locator('[data-feature-panel="timeline"]').isVisible(), true)
        } else {
          if (viewport.width > 820) {
            const directory = await probe('.docs-sidebar', 'structural')
            assert.equal(directory.fill, directory.expectedFill)
            assert.equal(directory.filter, directory.expectedFilter)
          } else {
            await page.locator('.mobile-chapters summary').click()
            await page.waitForFunction(() => document.querySelector('.mobile-chapters').open)
            const menu = await probe('.mobile-chapters', 'panel')
            assert.equal(menu.fill, menu.expectedFill, 'expanded directory gains readable panel material')
            assert.equal(menu.filter, menu.expectedFilter)
            await bounded(page.locator('.mobile-chapters'))
            await page.locator(`.mobile-chapters a[href="${path === 'manual/' ? '#wiki' : '#qa'}"]`).click()
            assert.equal(await page.locator('.mobile-chapters').getAttribute('open'), null)
          }
          await screenshot(`${path.slice(0, -1)}-${theme}`)
          if (path === 'llm-wiki/') {
            await page.locator('#qa').scrollIntoViewIfNeeded()
            assert.equal((await probe('#qa .diagram', 'content')).filter, 'none', 'diagram text stays unfiltered')
            await screenshot(`llm-wiki-${theme}-qa`)
          }
        }
        await page.evaluate(() => scrollTo(0, 0))
      }
      const cdp = await context.newCDPSession(page)
      for (const [name, value] of [['prefers-reduced-transparency', 'reduce'], ['prefers-contrast', 'more']]) {
        await cdp.send('Emulation.setEmulatedMedia', { features: [{ name, value }, { name: 'prefers-reduced-motion', value: 'reduce' }] })
        assert.equal(await page.evaluate(({ name, value }) => matchMedia(`(${name}: ${value})`).matches, { name, value }), true)
        const header = await probe('.header-inner', 'floating')
        assert.equal(header.filter, 'none')
        assert.equal(header.fill, header.expectedFill)
        assert.match(header.fill, /^rgb\(/, 'accessibility fallback is opaque')
      }
      await cdp.send('Emulation.setEmulatedMedia', { features: [] })
      await cdp.detach()
      await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' })
      assert.equal((await probe('.header-inner', 'floating')).filter, 'none')
      await page.emulateMedia({ forcedColors: 'none', reducedMotion: 'reduce' })
    }
    assert.deepEqual(errors, [])
    assert.deepEqual(external, [])
    assert.deepEqual(badAssets, [])
    results.push({ viewport, pages: 3, themes: 3, sharedOptics: true, menus: true, fallbacks: true, assets: true })
    await context.close()
  }
  for (const width of [1440, 390]) {
    const motionContext = await browser.newContext({ viewport: { width, height: 900 }, reducedMotion: 'no-preference' })
    const motionPage = await motionContext.newPage()
    await motionContext.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    await motionPage.goto(base)
    const feature = motionPage.locator('#features .feature-card[data-reveal]').first()
    const visible = state => motionPage.waitForFunction(state => document.querySelector('#features .feature-card[data-reveal]').dataset.visible === String(state), state)
    await feature.scrollIntoViewIfNeeded()
    await visible(true)
    await motionPage.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }))
    await visible(false)
    await feature.scrollIntoViewIfNeeded()
    await visible(true)
    await motionContext.close()
    const context = await browser.newContext({ viewport: { width, height: 900 }, javaScriptEnabled: false })
    const page = await context.newPage()
    for (const path of ['', 'manual/', 'llm-wiki/']) {
      await page.goto(`${base}${path}`)
      assert.ok(await page.locator('h1').isVisible())
      assert.ok(await page.locator('.site-nav').isVisible(), 'static navigation stays available')
    }
    await context.close()
  }
  console.log(JSON.stringify({ passed: true, results, repeatableMotion: true, noScript: true, screenshots: output, isolation: 'Built static Pages only; no API, data or credentials' }, null, 2))
} finally {
  await browser?.close()
  await new Promise((resolve, reject) => server.httpServer.close(error => error ? reject(error) : resolve()))
}
