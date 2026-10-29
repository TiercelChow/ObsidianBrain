/** Start tests/fixtures/mobileNavigation.mjs first. No real data or model calls. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const origin = 'http://127.0.0.1:5220'
const health = await (await fetch(origin + '/v1/health')).json()
assert.equal(health.version, 'preview')
assert.equal(health.storage.path, '/mock')
const screenshots = await mkdtemp(join(tmpdir(), 'dock-route-morph-'))
const browser = await chromium.launch({ headless: true, channel: 'chromium' })
const results = [], errors = [], warnings = []

async function startCapture(page) {
  await page.evaluate(() => {
    const element = document.querySelector('.mobile-auxiliary-surface')
    const start = performance.now()
    window.__dockMorph = { frames: [], done: false }
    function record(now) {
      const r = element.getBoundingClientRect()
      const style = getComputedStyle(element)
      window.__dockMorph.frames.push({ t: now - start, x: r.x, y: r.y, width: r.width, height: r.height, same: element === document.querySelector('.mobile-auxiliary-surface'), connected: element.isConnected, opacity: style.opacity, mode: element.dataset.mode, moving: element.dataset.morphing })
      if (now - start < 1000) requestAnimationFrame(record)
      else window.__dockMorph.done = true
    }
    requestAnimationFrame(record)
  })
}
async function finishCapture(page, from, to, label) {
  await page.waitForFunction(() => window.__dockMorph?.done)
  const frames = await page.evaluate(() => window.__dockMorph.frames)
  assert.ok(frames.length > 15, `${label}: ${JSON.stringify(frames)}`)
  assert.ok(frames.every(f => f.same && f.connected && f.opacity === '1'), `${label}: one persistent unfaded surface`)
  assert.ok(Math.abs(frames.at(-1).width - to) < .5, `${label}: correct final footprint`)
  assert.ok(frames.some(f => f.width > Math.min(from, to) + 12 && f.width < Math.max(from, to) - 12), `${label}: intermediate capsule widths`)
  const steps = frames.slice(1).map((f, i) => Math.abs(f.width - frames[i].width))
  assert.ok(Math.max(...steps) < Math.max(65, Math.abs(to - from) * .2), `${label}: no endpoint reset (${Math.max(...steps)})`)
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  return { label, frames: frames.length, maxStep: Math.max(...steps), finalWidth: frames.at(-1).width }
}

try {
  for (const width of [320, 390, 667]) {
    const context = await browser.newContext({ viewport: { width, height: width === 667 ? 375 : 844 }, isMobile: true, hasTouch: true })
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    page.on('console', message => { if (message.type() === 'warning' && /Vue warn/.test(message.text())) warnings.push(message.text()) })
    await page.goto(origin)
    await page.getByRole('button', { name: /^切换主题/ }).waitFor()
    const barWidth = width - 40
    const main = page.getByRole('navigation', { name: '主要导航', exact: true })
    for (const [target, name, from, to] of [
      ['tasks', '任务中枢', 56, barWidth], ['timeline', '时光机', barWidth, 56],
      ['wiki', 'Wiki', 56, barWidth], ['reader', '阅境轩', barWidth, 56],
    ]) {
      await startCapture(page)
      await main.getByRole('link', { name, exact: true }).click()
      results.push({ width, ...(await finishCapture(page, from, to, target)) })
      if (width === 390 && target === 'tasks') await page.screenshot({ path: join(screenshots, 'task-sub-dock.png') })
      if (target === 'wiki') {
        // Existing scroll retraction still owns its original spring and footprint.
        await page.evaluate(() => {
          const scroller = [...document.querySelectorAll('.app-main, .app-main *')].find(element => element.scrollHeight > element.clientHeight + 200 && /auto|scroll/.test(getComputedStyle(element).overflowY))
          if (!scroller) throw new Error('Isolated Wiki scroller missing')
          scroller.dispatchEvent(new WheelEvent('wheel', { deltaY: 160, bubbles: true }))
          scroller.scrollTop += 160
        })
        await page.waitForFunction(() => document.querySelector('.mobile-navigation')?.dataset.progress === '100' && document.querySelector('.mobile-navigation')?.dataset.settled === 'true')
        const compact = await page.locator('.mobile-auxiliary-surface').boundingBox()
        assert.ok(Math.abs(compact.height - 62) < .5)
        assert.ok(Math.abs(compact.width - (width - 86)) < .5)
        for (const item of await page.locator('.mobile-sub-dock-item').all()) {
          const box = await item.boundingBox()
          assert.ok(box.width >= 44 && box.height >= 44)
        }
        await page.getByRole('button', { name: '展开主要导航', exact: true }).click()
        await page.waitForFunction(() => document.querySelector('.mobile-navigation')?.dataset.progress === '0' && document.querySelector('.mobile-navigation')?.dataset.settled === 'true')
      }
    }
    // Retarget before expansion settles: don't restart from a circle or full bar.
    await startCapture(page)
    await main.getByRole('link', { name: '任务中枢', exact: true }).click()
    await page.evaluate(() => setTimeout(() => document.querySelector('.mobile-dock-item[href="/timeline"]').click(), 90))
    await page.waitForFunction(() => window.__dockMorph?.done)
    const reversal = await page.evaluate(() => window.__dockMorph.frames)
    assert.ok(reversal.every(f => f.same && f.connected))
    assert.ok(Math.max(...reversal.map(f => f.width)) > 70)
    assert.ok(Math.abs(reversal.at(-1).width - 56) < .5)
    assert.ok(Math.max(...reversal.slice(1).map((f, i) => Math.abs(f.width - reversal[i].width))) < Math.max(65, (barWidth - 56) * .2))
    await page.getByRole('button', { name: '写小记', exact: true }).click()
    const composer = page.getByRole('dialog', { name: '写小记', exact: true })
    await composer.waitFor()
    assert.equal(await page.locator('html').evaluate(e => getComputedStyle(e).visibility), 'visible')
    await page.keyboard.press('Escape')
    await composer.waitFor({ state: 'hidden' })
    if (width === 390) {
      // Simulated visualViewport shrink; actual device keyboard remains manual QA.
      await page.locator('.glass-input').focus()
      await page.evaluate(() => {
        Object.defineProperty(window.visualViewport, 'height', { configurable: true, value: 500 })
        window.visualViewport.dispatchEvent(new Event('resize'))
      })
      await page.locator('.mobile-navigation').waitFor({ state: 'detached' })
      await page.evaluate(() => {
        document.activeElement.blur()
        delete window.visualViewport.height
        window.visualViewport.dispatchEvent(new Event('resize'))
      })
      await page.getByRole('button', { name: '写小记', exact: true }).waitFor()
    }
    await context.close()
  }
  const reduced = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, reducedMotion: 'reduce' })
  await reduced.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
  const page = await reduced.newPage()
  await page.goto(origin)
  await page.getByRole('button', { name: /^切换主题/ }).waitFor()
  await page.getByRole('navigation', { name: '主要导航', exact: true }).getByRole('link', { name: '任务中枢', exact: true }).click()
  await page.locator('.mobile-sub-dock-content').waitFor()
  assert.equal(await page.locator('.mobile-auxiliary-surface').getAttribute('data-morphing'), 'false')
  assert.equal((await page.locator('.mobile-auxiliary-surface').boundingBox()).width, 350)
  await reduced.close()
  assert.deepEqual(errors, [])
  assert.deepEqual(warnings, [])
  console.log(JSON.stringify({ results, screenshots, errors, warnings, reversed: 3, reducedMotion: true }, null, 2))
} finally {
  await browser.close()
}
