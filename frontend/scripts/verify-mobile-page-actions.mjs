/** Run against tests/fixtures/mobileNavigation.mjs only. Never saves user data. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const origin = 'http://127.0.0.1:5220'
const health = await (await fetch(`${origin}/v1/health`)).json()
assert.equal(health.version, 'preview', 'Only the isolated navigation fixture is allowed')
assert.equal(health.storage.path, '/mock', 'Never test against user data')
const screenshots = await mkdtemp(join(tmpdir(), 'mobile-page-actions-'))
const browser = await chromium.launch({ headless: true, channel: 'chromium' })
const errors = [], results = []

async function settle(page) {
  await page.evaluate(async () => {
    await Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => {})))
  })
}

async function geometry(page, label) {
  const button = page.getByRole('button', { name: label, exact: true })
  await button.waitFor()
  await page.locator('.mobile-auxiliary-surface[data-morphing="false"]').waitFor()
  const fab = await button.boundingBox()
  const dock = await page.locator('.mobile-navigation').boundingBox()
  assert.equal(fab.width, 56)
  assert.equal(fab.height, 56)
  assert.ok(Math.abs(dock.y - (fab.y + fab.height) - 12) < 1, '12px clear of Dock')
  assert.ok(Math.abs(page.viewportSize().width - fab.x - fab.width - 16) < 1, 'Same right anchor')
  const icon = await button.locator('svg').boundingBox()
  assert.ok(Math.abs(icon.x + icon.width / 2 - fab.x - fab.width / 2) < .5)
  assert.ok(Math.abs(icon.y + icon.height / 2 - fab.y - fab.height / 2) < .5)
  const material = await button.evaluate(button => {
    const element = button.closest('.mobile-auxiliary-surface')
    const style = getComputedStyle(element)
    const optics = element.querySelector('.glass-optics')
    return { radius: style.borderRadius, optics: !!optics, filter: optics && getComputedStyle(optics).backdropFilter }
  })
  assert.equal(material.radius, '999px')
  assert.ok(material.optics && material.filter !== 'none', 'Shared floating glass, not a flat glyph')
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
  return fab
}

try {
  for (const [width, height] of [[320, 568], [390, 844], [667, 375], [768, 960], [769, 900], [1440, 1000]]) {
    const phone = width <= 768
    for (const theme of ['light', 'dark', 'eye-care']) {
      const context = await browser.newContext({ viewport: { width, height }, isMobile: phone, hasTouch: phone })
      await context.addInitScript(theme => localStorage.setItem('theme', theme), theme)
      await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
      const page = await context.newPage()
      page.on('pageerror', error => errors.push(error.message))
      await page.goto(origin)
      await page.locator('.attention-tile').first().waitFor()
      await settle(page)
      if (phone) {
        const label = `切换主题，当前${{ light: '浅色', dark: '深色', 'eye-care': '护眼' }[theme]}`
        const before = await geometry(page, label)
        assert.equal(await page.getByRole('button', { name: '刷新工作台', exact: true }).count(), 0)
        await page.getByRole('button', { name: label, exact: true }).click()
        const nextTheme = { light: 'dark', dark: 'eye-care', 'eye-care': 'light' }[theme]
        assert.equal(await page.locator('html').getAttribute('data-theme'), nextTheme)
        // Restore palette without remounting; scrolling must not move the action.
        await page.getByRole('button', { name: /^切换主题/ }).click()
        await page.getByRole('button', { name: /^切换主题/ }).click()
        await page.locator('.app-main').evaluate(element => { element.scrollTop = element.scrollHeight })
        const after = await page.locator('.mobile-page-action').boundingBox()
        assert.deepEqual(after, before)
        const last = await page.locator('.home-system footer').boundingBox()
        assert.ok(last.y + last.height <= after.y + 1, 'Trailing information can scroll above action')
        if (width === 390 && theme === 'dark') await page.screenshot({ path: join(screenshots, 'home-dark.png') })
      } else {
        assert.equal(await page.locator('.mobile-page-action').count(), 0)
        assert.ok(await page.getByRole('button', { name: '刷新工作台', exact: true }).isVisible())
        assert.ok(await page.locator('.home-theme-action').isVisible())
      }

      await page.goto(`${origin}/reader?view=shelf`)
      await page.locator('.book-cover').first().waitFor()
      await settle(page)
      const add = page.getByRole('button', { name: '添加书籍', exact: true })
      assert.equal(await add.count(), 1)
      if (phone) {
        await geometry(page, '添加书籍')
        assert.equal(await page.locator('.reader-shelf-add').count(), 0)
        await page.locator('.bookshelf-root').evaluate(element => { element.scrollTop = element.scrollHeight })
        const last = await page.locator('.book-cover').last().boundingBox()
        const fab = await add.boundingBox()
        assert.ok(last.y + last.height <= fab.y + 1, 'Last row is not covered')
        if (width === 390 && theme === 'light') await page.screenshot({ path: join(screenshots, 'bookshelf-light.png') })
      }
      await add.click()
      const bookDialog = page.locator('.book-form-dialog:visible')
      await bookDialog.waitFor()
      await bookDialog.getByRole('button', { name: '取消', exact: true }).click()
      await bookDialog.waitFor({ state: 'hidden' })
      if (phone) {
        await page.locator('.view-switch').getByRole('button', { name: '阅读', exact: true }).click()
        await page.waitForURL('**/reader?view=read')
        assert.equal(await page.locator('.mobile-page-action').count(), 0, 'Never cover reading controls')
        await page.locator('.view-switch').getByRole('button', { name: '书架', exact: true }).click()
        await geometry(page, '添加书籍')
      }

      await page.goto(`${origin}/timeline`)
      await page.locator('.memo-card').first().waitFor()
      await settle(page)
      const compose = page.getByRole('button', { name: '写小记', exact: true })
      assert.equal(await compose.count(), 1)
      if (phone) {
        await geometry(page, '写小记')
        const search = await page.locator('.search-box').boundingBox()
        const filter = await page.locator('.mobile-filter-summary').boundingBox()
        assert.ok(search.x + search.width < filter.x && Math.abs(search.y - filter.y) < 1)
        await page.locator('.app-main').evaluate(element => { element.scrollTop = element.scrollHeight })
        const last = await page.locator('.memo-card').last().boundingBox()
        const fab = await compose.boundingBox()
        assert.ok(last.y + last.height <= fab.y + 1, 'Last memo can scroll clear of action')
        if (width === 390 && theme === 'eye-care') {
          await page.locator('.app-main').evaluate(element => { element.scrollTop = 0 })
          await page.screenshot({ path: join(screenshots, 'timeline-eye-care.png') })
        }
      }
      await compose.click()
      const composer = page.getByRole('dialog', { name: '写小记', exact: true })
      await composer.waitFor()
      assert.equal(await page.locator('html').evaluate(element => getComputedStyle(element).visibility), 'visible', 'Document remains visible with sheet open')
      assert.equal(await composer.evaluate(element => getComputedStyle(element).visibility), 'visible')
      if (phone) assert.equal(await page.locator('.mobile-page-action').isVisible(), false, 'No floating action above an open sheet')
      if (width === 390 && theme === 'light') {
        await settle(page)
        await page.screenshot({ path: join(screenshots, 'compose-light.png') })
      }
      await page.keyboard.press('Escape')
      await composer.waitFor({ state: 'hidden' })
      if (phone) {
        await geometry(page, '写小记')
        await page.locator('.app-main').evaluate(element => { element.scrollTop = 0 })
        await page.locator('.mobile-filter-summary').click()
        const filters = page.getByRole('dialog', { name: '时间筛选', exact: true })
        await filters.waitFor()
        assert.equal(await filters.evaluate(element => getComputedStyle(element).visibility), 'visible')
        await page.keyboard.press('Escape')
        await filters.waitFor({ state: 'hidden' })
        await page.locator('.memo-image').first().click()
        const photo = page.getByRole('dialog', { name: '小记图片', exact: true })
        await photo.waitFor()
        assert.equal(await photo.evaluate(element => getComputedStyle(element).visibility), 'visible')
        await page.getByRole('button', { name: '关闭图片', exact: true }).click()
        await photo.waitFor({ state: 'hidden' })
        await geometry(page, '写小记')
      }
      results.push({ width, height, theme, phone })
      await context.close()
    }
  }
  assert.deepEqual(errors, [])
  console.log(JSON.stringify({ passed: results.length, screenshots, errors, results }, null, 2))
} finally {
  await browser.close()
}
