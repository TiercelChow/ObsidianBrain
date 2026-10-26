/** Real Element Plus menus with isolated state: no App, API or saved settings. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url)), screenshots = await mkdtemp(join(tmpdir(), 'select-menu-shape-'))
console.log(`Screenshots: ${screenshots}`)
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, cacheDir: join(screenshots, '.vite-cache'), logLevel: 'error', server: { host: '127.0.0.1', port: 0, proxy: {} } })
const results = [], errors = [], forbidden = []
let browser, page

async function shape(menu, placement) {
  await menu.waitFor()
  await menu.evaluate(async element => {
    await Promise.all(element.getAnimations({ subtree: true }).map(animation => animation.finished.catch(() => {})))
  })
  const geometry = await menu.evaluate(element => {
    const style = getComputedStyle(element), rect = element.getBoundingClientRect()
    const corners = [[1, 1], [rect.width - 1, 1], [1, rect.height - 1], [rect.width - 1, rect.height - 1]]
    return { radii: [style.borderTopLeftRadius, style.borderTopRightRadius, style.borderBottomLeftRadius, style.borderBottomRightRadius],
      overflow: style.overflow, placement: element.getAttribute('data-popper-placement'),
      opticalRadii: getComputedStyle(element, '::before').borderRadius,
      cornersClipped: corners.every(([x, y]) => !element.contains(document.elementFromPoint(rect.left + x, rect.top + y))),
      inside: rect.left >= 0 && rect.right <= innerWidth + 1 && rect.top >= 0 && rect.bottom <= innerHeight + 1 }
  })
  assert.deepEqual(geometry.radii, Array(4).fill('14px'))
  assert.equal(geometry.overflow, 'hidden')
  assert.equal(geometry.opticalRadii, '14px')
  assert.ok(geometry.placement.startsWith(placement) && geometry.inside && geometry.cornersClipped, JSON.stringify(geometry))
}

try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [320, 390, 1440]) for (const theme of ['light', 'dark', 'eye-care']) {
    const context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: width < 768, hasTouch: width < 768 })
    await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
    await context.route('**/*', route => {
      const url = new URL(route.request().url())
      if (url.origin !== origin || url.pathname.startsWith('/v1')) { forbidden.push(url.pathname); return route.abort() }
      return route.continue()
    })
    page = await context.newPage(); page.on('pageerror', error => errors.push(error.message))
    for (const kind of ['system', 'generic', 'search', 'multiple', 'empty']) for (const placement of ['bottom', 'top']) {
      await page.goto(`${origin}/tests/fixtures/select-menus.html?kind=${kind}&placement=${placement}`)
      await page.locator('.el-select__wrapper').click()
      const menu = page.locator('.el-popper:visible').filter({ has: page.locator('.el-select-dropdown') })
      await shape(menu, placement)
      if (kind !== 'empty') {
        const wrap = menu.locator('.el-select-dropdown__wrap')
        await wrap.evaluate(element => { element.scrollTop = element.scrollHeight })
        await shape(menu, placement)
        await page.getByRole('option', { name: '选项 30', exact: true }).click()
        assert.match(await page.locator('#selected-value').innerText(), /选项 30/)
        if (kind === 'multiple') await page.keyboard.press('Escape')
        await menu.waitFor({ state: 'hidden' })
        await page.locator('.el-select__wrapper').click()
        await page.getByRole('option', { name: '选项 30', exact: true }).hover()
        await shape(menu, placement)
      }
      if (kind === 'system' || kind === 'generic') await page.screenshot({ path: join(screenshots, `${width}-${theme}-${kind}-${placement}.png`) })
      if (kind === 'search') {
        await page.getByRole('combobox', { name: '隔离选择框' }).fill('选项 29')
        await page.getByRole('option', { name: '选项 29', exact: true }).waitFor()
        await shape(menu, placement)
        await page.keyboard.press('ArrowDown')
        await page.keyboard.press('Enter')
        assert.match(await page.locator('#selected-value').innerText(), /选项 29/)
      }
      await page.keyboard.press('Escape')
      await menu.waitFor({ state: 'hidden' })
      results.push({ width, theme, kind, placement })
    }
    await context.close()
  }
  assert.deepEqual(errors, []); assert.deepEqual(forbidden, [])
  console.log(JSON.stringify({ passed: true, cases: results.length, screenshots }, null, 2))
} catch (error) {
  await page?.screenshot({ path: join(screenshots, 'failure.png') }).catch(() => {})
  console.error(JSON.stringify({ results, errors, forbidden }))
  throw error
} finally { await browser?.close(); await server.close() }
