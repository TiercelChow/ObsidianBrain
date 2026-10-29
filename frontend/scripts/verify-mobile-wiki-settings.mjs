/** Mobile and desktop settings acceptance; never connects to the real backend. */
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const root = fileURLToPath(new URL('../', import.meta.url))
const screenshots = await mkdtemp(join(tmpdir(), 'wiki-settings-mobile-'))
console.log(`Screenshots: ${screenshots}`)
const timestamp = '2026-10-16T00:00:00Z'
const books = ['第一本：知识机制与适用边界的完整文档', '第二本：延迟返回的知识库', '第三本：最新选中的知识库'].map((name, i) => ({
  id: `base-${i}`, book_id: `book-${i}`, book_name: name + 'VeryLongBookNameWithoutWhitespace'.repeat(3),
  book_path: `/mock/book-${i}`, book_kind: 'folder', lifecycle: 'active', source_available: true,
}))
const body = '# 完整 Skill 指令\n\n' + Array.from({ length: 110 }, (_, i) => `## 步骤 ${i + 1}\n\n核对证据、条件与完整输出。${'LongContentWithoutWhitespace'.repeat(5)}\n`).join('\n') + '\n完整正文的最后一行：END_OF_SKILL'
const requests = []
const unexpected = []
let empty = false
let switching = false
let candidate = false
const skill = baseId => ({ id: `skill-${baseId}`, slug: 'book-query', name: `知识问答 · ${baseId}`, description: '完整核对书籍证据与边界，不修改原书。', source_type: 'custom', revision: 1, status: 'ready', permissions: ['read_knowledge'], requirements: [], instructions: body, enabled: baseId === 'base-0', usage_scope: 'both', updated_at: timestamp })
const runtime = { available: true, message: '仅隔离布局测试，不启动执行器', profile: { id: 'mock-runtime', name: 'DeepSeek Harness', runtime: 'deepseek_harness', executable: 'never-executed', provider_id: 'mock-provider', model: '', enabled: true, revision: 1, updated_at: timestamp } }
const provider = { provider_id: 'mock-provider', display_name: '模型供应商长名称', api_protocol: 'openai-completions', base_url: 'https://never-contact.example/' + 'long-api-path/'.repeat(8), model: 'never-contact-model', credential_source: 'environment', api_key_env: 'MOCK_NO_CREDENTIALS', api_key_configured: false, enabled: true, revision: 1, updated_at: timestamp }
const fixtures = {
  list_book_knowledge_bases: () => ({ items: empty ? [] : books.map(base => ({ book: { id: base.book_id, kind: 'folder', name: base.book_name, path: base.book_path }, knowledge_base: base })) }),
  get_book_wiki_settings: args => ({ runtime_profiles: [runtime], model_providers: [provider], documents: ['WIKI.md', 'AGENTS.md', 'QUERY.md', 'RESEARCH.md'].map(name => ({ id: `${args.knowledge_base_id}-${name}`, name, knowledge_base_id: args.knowledge_base_id, scope: 'book', revision: 1, content_md: `# ${args.knowledge_base_id}\n\n${'完整配置规则\n'.repeat(50)}`, updated_at: timestamp })) }),
  list_wiki_skills: args => ({ skills: [skill(args.knowledge_base_id)] }),
  get_wiki_skill_detail: args => ({ skill: skill(args.knowledge_base_id), current_version_id: 'version-1', versions: [{ id: 'version-1', revision: 1, content_hash: 'a'.repeat(64), release_state: candidate ? 'candidate' : 'published', changelog: '完整指令，不截断正文', created_at: timestamp, files: [
    { relative_path: 'SKILL.md', content_text: body, content_hash: 'a'.repeat(64), media_type: 'text/markdown', size_bytes: body.length },
    { relative_path: 'references/' + 'very-long-resource-path-'.repeat(6) + '.md', content_text: '# 资源文件\n\n资源正文：RESOURCE_END', content_hash: 'b'.repeat(64), media_type: 'text/markdown', size_bytes: 100 },
  ] }] }),
  get_agent_usage_stats: args => ({ start_date: args.start_date, end_date: args.end_date, usage_source: 'unavailable', totals: { runs: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0, unreported_runs: 0 }, daily: [], by_caller: [] }),
}
const mockApi = {
  name: 'mobile-settings-in-memory-api',
  configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.url?.startsWith('/v1')) return next()
      response.setHeader('Content-Type', 'application/json')
      if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'mock', components: {}, uptime_seconds: 1, tools_count: 0 }))
      if (request.url !== '/v1/tools/call') {
        unexpected.push(request.url)
        response.statusCode = 501
        return response.end(JSON.stringify({ error: 'Real backend forbidden' }))
      }
      let payload = ''
      for await (const part of request) payload += part
      const { tool, arguments: args = {} } = JSON.parse(payload)
      requests.push({ tool, ...args })
      if (!fixtures[tool]) {
        unexpected.push(tool)
        response.statusCode = 501
        return response.end(JSON.stringify({ status: 'error', error: { message: 'Unsupported isolated action' } }))
      }
      const result = fixtures[tool](args)
      const send = () => response.end(JSON.stringify({ status: 'success', result }))
      if (switching && args.knowledge_base_id === 'base-1' && ['get_book_wiki_settings', 'list_wiki_skills'].includes(tool)) setTimeout(send, 600)
      else send()
    })
  },
}
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, logLevel: 'error', plugins: [mockApi, ...config.config.plugins], server: { host: '127.0.0.1', port: 0, strictPort: false, proxy: {} } })
let browser
const results = []
try {
  await server.listen()
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const viewport of [{ width: 320, height: 568 }, { width: 390, height: 844 }, { width: 667, height: 375 }, { width: 769, height: 700 }, { width: 900, height: 700 }, { width: 1100, height: 800 }, { width: 1440, height: 1000 }, { width: 1440, height: 600 }]) {
    const phone = viewport.width <= 768
    candidate = !phone && viewport.height === 600
    const context = await browser.newContext({ viewport, isMobile: phone, hasTouch: phone })
    const page = await context.newPage()
    const errors = []
    page.on('pageerror', error => errors.push(error.message))
    await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort())
    await page.goto(`${origin}/knowledge/settings?section=protection`)
    await page.locator('.runtime-card').waitFor()
    assert.equal(await page.locator('.settings-card').count(), phone ? 5 : 1)
    assert.equal(await page.locator('.settings-nav').count(), phone ? 0 : 1)
    assert.equal(await page.locator('#settings-protection, .restore-modal-card, .wiki-export-card').count(), 0)
    assert.equal(await page.getByRole('button', { name: /数据保护/ }).count(), 0)
    assert.ok(await page.locator('#settings-runtime').isVisible(), 'retired section links fall back to Runtime')
    await page.evaluate(() => document.getAnimations().forEach(animation => animation.finish()))
    for (const theme of ['light', 'dark', 'eye-care']) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme }, theme)
      const runtimeSurface = await page.locator('.runtime-card').evaluate(element => {
        const style = getComputedStyle(element)
        return { fill: style.backgroundColor, shadow: style.boxShadow, filter: style.backdropFilter, border: style.borderWidth, padding: style.padding }
      })
      assert.deepEqual(runtimeSurface, { fill: 'rgba(0, 0, 0, 0)', shadow: 'none', filter: 'none', border: '0px', padding: '0px' }, `${viewport.width}px ${theme}: Runtime shares its section surface, not a second card`)
    }
    if (phone) for (const theme of ['light', 'dark', 'eye-care']) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme }, theme)
      const material = await page.locator('.settings-main').evaluate(element => {
        const shell = getComputedStyle(element)
        const card = getComputedStyle(element.querySelector('.settings-card'))
        const reference = document.createElement('span')
        reference.style.background = 'var(--glass-structural-fill)'
        document.body.append(reference)
        const expected = getComputedStyle(reference).backgroundColor
        reference.remove()
        return { shell: shell.backgroundColor, shadow: shell.boxShadow, filter: shell.backdropFilter, card: card.backgroundColor, cardFilter: card.backdropFilter, expected }
      })
      assert.equal(material.shell, 'rgba(0, 0, 0, 0)', 'a phone feed must not retain a hidden content-colored slab')
      assert.equal(material.shadow, 'none')
      assert.equal(material.filter, 'none')
      assert.equal(material.card, material.expected, `${theme}: only one content material per card`)
      assert.equal(material.cardFilter, 'blur(5px) saturate(1.15)', 'same single background material as My Tasks')
      assert.equal(await page.locator('.runtime-card').evaluate(element => getComputedStyle(element).backdropFilter), 'none', 'nested cards never add another blur pass')
    }
    await page.evaluate(() => { document.documentElement.dataset.theme = localStorage.getItem('theme') || 'light' })
    if (phone) {
      await page.screenshot({ path: join(screenshots, `${viewport.width}-runtime.png`) })
      await page.locator('.runtime-card input').first().fill('unsaved-local-command')
    }
    if (!phone) await page.locator('.settings-nav button').filter({ hasText: '配置文档' }).click()
    const docs = page.locator('#settings-documents')
    await docs.locator('textarea').waitFor()
    const select = docs.locator('.settings-base-select')
    await select.click()
    const menu = page.getByRole('listbox').filter({ visible: true })
    await menu.waitFor()
    if (phone) {
      await page.waitForFunction(() => !document.querySelector('.wiki-settings-base-popper')?.getAnimations({ subtree: true }).some(animation => animation.playState === 'running'))
      await menu.evaluate(element => {
        for (let node = element; node; node = node.parentElement) node.getAnimations().forEach(animation => animation.finish())
      })
      const full = await menu.getByRole('option').first().evaluate(option => {
        const range = document.createRange()
        range.selectNodeContents(option)
        const text = range.getBoundingClientRect(), rect = option.getBoundingClientRect()
        return { fits: text.left >= rect.left - 1 && text.right <= rect.right + 1 && text.bottom <= rect.bottom + 1, height: rect.height, viewport: innerWidth, right: rect.right, label: option.textContent, cssHeight: getComputedStyle(option).height, minHeight: getComputedStyle(option).minHeight, whiteSpace: getComputedStyle(option).whiteSpace }
      })
      assert.ok(full.fits && full.height > 44 && full.right <= full.viewport, `long option must wrap: ${JSON.stringify(full)}`)
      const popup = await menu.evaluate(element => {
        const host = element.closest('.el-popper'), rect = host.getBoundingClientRect(), style = getComputedStyle(host)
        return { top: rect.top, bottom: rect.bottom, viewport: innerHeight, radius: style.borderRadius, overflow: style.overflow }
      })
      assert.ok(popup.top >= -1 && popup.bottom <= popup.viewport + 1, `entire popper fits: ${JSON.stringify(popup)}`)
      assert.equal(popup.radius, '14px'); assert.equal(popup.overflow, 'hidden')
      const wrap = page.locator('.wiki-settings-base-popper:visible .el-select-dropdown__wrap')
      await wrap.evaluate(element => { element.scrollTop = element.scrollHeight })
      const last = await menu.getByRole('option').last().boundingBox()
      assert.ok(last.y + last.height <= popup.bottom + 1, 'last knowledge base can be scrolled fully into view')
      await wrap.evaluate(element => { element.scrollTop = 0 })
      await page.screenshot({ path: join(screenshots, `${viewport.width}-book-selector.png`) })
    }
    await page.keyboard.press('Escape')
    if (phone) {
      switching = true
      for (const index of [1, 2]) {
        await select.click()
        await page.getByRole('option', { name: books[index].book_name, exact: true }).click()
      }
      await page.waitForFunction(() => document.querySelector('.document-editor textarea')?.value.startsWith('# base-2'))
      await page.waitForTimeout(750)
      assert.match(await docs.locator('textarea').inputValue(), /^# base-2/)
      assert.equal(await page.locator('.skill-card h3').innerText(), '知识问答 · base-2')
      assert.equal(await page.locator('.skill-card input[role="switch"]').isChecked(), false)
      assert.equal(await page.locator('.runtime-card input').first().inputValue(), 'unsaved-local-command', 'switching books preserves unsaved Runtime input')
      switching = false
      for (const selector of ['.skills-create-actions', '.skill-card-actions', '.runtime-actions']) {
        const groups = page.locator(selector)
        for (const group of await groups.all()) {
          const geometry = await group.evaluate(element => {
            const parent = element.closest('.settings-card').getBoundingClientRect()
            const buttons = [...element.querySelectorAll('button.ui-action, button.ui-icon-action')].map(button => {
              const rect = button.getBoundingClientRect()
              return { left: rect.left, right: rect.right, width: rect.width, height: rect.height, top: rect.top }
            })
            return { left: parent.left, right: parent.right, buttons }
          })
          for (const button of geometry.buttons) assert.ok(button.width >= 43 && button.height >= 43 && button.left >= geometry.left && button.right <= geometry.right, `44px actions inside card: ${JSON.stringify(geometry)}`)
          if (geometry.buttons.length > 1) assert.ok(Math.abs(geometry.buttons[0].top - geometry.buttons[1].top) <= 1, 'paired icons stay on one line')
        }
      }
      const overflow = await page.evaluate(() => ({ width: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth, cards: [...document.querySelectorAll('.settings-card')].map(card => ({ width: card.clientWidth, scroll: card.scrollWidth })) }))
      assert.ok(overflow.scroll <= overflow.width + 1 && overflow.cards.every(card => card.scroll <= card.width + 1), JSON.stringify(overflow))
      await page.locator('#settings-skills').scrollIntoViewIfNeeded()
      await page.screenshot({ path: join(screenshots, `${viewport.width}-cards.png`) })
    } else await page.locator('.settings-nav button').filter({ hasText: 'Skills' }).click()
    await page.getByRole('button', { name: '查看 Skill 内容', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: 'Skill 内容', exact: true })
    await dialog.locator('.skill-file-content').waitFor()
    if (phone) {
      assert.equal(await dialog.locator('.skill-file-content').innerText(), body.trimEnd())
      await page.waitForFunction(() => !document.querySelector('.motion-modal').classList.contains('is-settling'))
      const reader = dialog.locator('.skill-detail-layout')
      await reader.evaluate(element => { element.scrollTop = element.scrollHeight })
      const end = await dialog.locator('.skill-file-content').evaluate(element => {
        const text = element.firstChild, range = document.createRange()
        range.setStart(text, text.length - 'END_OF_SKILL'.length)
        range.setEnd(text, text.length)
        const rect = range.getBoundingClientRect(), viewport = element.closest('.skill-detail-layout').getBoundingClientRect()
        return { top: rect.top, bottom: rect.bottom, viewportTop: viewport.top, viewportBottom: viewport.bottom, right: rect.right, width: innerWidth }
      })
      assert.ok(end.top >= end.viewportTop && end.bottom <= end.viewportBottom + 1 && end.right <= end.width, `last line readable: ${JSON.stringify(end)}`)
      await dialog.getByRole('button', { name: '权限、版本与来源说明' }).click()
      await reader.evaluate(element => { element.scrollTop = 0 })
      assert.ok(await dialog.locator('.skill-detail-sidebar').isVisible())
      await dialog.getByRole('button', { name: '收起权限与版本说明' }).click()
      const resource = dialog.locator('.skill-file-tabs button').nth(1)
      await resource.click()
      await page.waitForFunction(() => document.querySelector('.skill-file-content')?.textContent.includes('RESOURCE_END'))
      assert.match(await dialog.locator('.skill-file-content').innerText(), /RESOURCE_END/)
      const footer = await dialog.getByRole('button', { name: '完成', exact: true }).boundingBox()
      assert.ok(footer.y >= 0 && footer.y + footer.height <= viewport.height)
      await page.screenshot({ path: join(screenshots, `${viewport.width}-skill.png`) })
      if (viewport.height > 500) {
        await page.evaluate(() => { Object.defineProperty(window.visualViewport, 'height', { value: 350, configurable: true }); window.visualViewport.dispatchEvent(new Event('resize')) })
        await page.waitForFunction(() => document.querySelector('.motion-modal').style.getPropertyValue('--motion-viewport-height') === '350px')
        const keyboardFooter = await dialog.getByRole('button', { name: '完成', exact: true }).boundingBox()
        assert.ok(keyboardFooter.y >= 0 && keyboardFooter.y + keyboardFooter.height <= 350, 'Skill footer clears keyboard')
      }
    } else {
      await page.waitForFunction(() => !document.querySelector('.motion-modal').classList.contains('is-settling'))
      assert.equal(await dialog.locator('.skill-detail-sidebar .skill-file-button').count(), 2)
      assert.equal(await dialog.locator('.desktop-skill-files .skill-file-button').first().evaluate(button => getComputedStyle(button).boxShadow), 'none', 'file navigation is flat, not another raised glass layer')
      assert.ok((await dialog.locator('.desktop-skill-files .skill-file-button').nth(1).boundingBox()).height <= 80, 'long resource paths do not consume the sidebar')
      const reader = dialog.locator('.skill-file-content')
      const readBox = await reader.boundingBox()
      const panel = await dialog.boundingBox()
      assert.ok(panel.x >= 0 && panel.x + panel.width <= viewport.width && panel.y >= 0 && panel.y + panel.height <= viewport.height, 'desktop modal fits viewport')
      assert.ok(Math.abs(panel.width - Math.min(1080, viewport.width - 48)) < 1, 'Skill dialog uses its own wide layout, not the generic 840px panel')
      assert.ok(readBox.height >= 180, `body has meaningful reading area: ${readBox.height}`)
      const scroll = await reader.evaluate(element => {
        element.scrollTop = element.scrollHeight
        const range = document.createRange(), text = element.firstChild
        range.setStart(text, text.length - 'END_OF_SKILL'.length)
        range.setEnd(text, text.length)
        const last = range.getBoundingClientRect(), frame = element.getBoundingClientRect()
        return { top: last.top, bottom: last.bottom, left: last.left, right: last.right, frameTop: frame.top, frameBottom: frame.bottom, frameLeft: frame.left, frameRight: frame.right, width: element.clientWidth, scrollWidth: element.scrollWidth }
      })
      assert.ok(scroll.top >= scroll.frameTop && scroll.bottom <= scroll.frameBottom + 1 && scroll.left >= scroll.frameLeft && scroll.right <= scroll.frameRight && scroll.scrollWidth <= scroll.width + 1, `entire body fits its own reader: ${JSON.stringify(scroll)}`)
      const audit = dialog.locator('.skill-version-audit')
      assert.equal(await audit.getAttribute('open'), null, 'metadata starts collapsed on desktop')
      await reader.evaluate(element => { element.scrollTop = 0 })
      await page.screenshot({ path: join(screenshots, `${viewport.width}-${viewport.height}-skill-reader.png`) })
      const before = await dialog.getByRole('button', { name: '完成', exact: true }).boundingBox()
      await audit.locator('summary.skill-audit-toggle').click()
      await audit.locator('.skill-audit-grid').waitFor()
      const after = await dialog.getByRole('button', { name: '完成', exact: true }).boundingBox()
      assert.ok(Math.abs(before.y - after.y) < 1 && after.y + after.height <= viewport.height, 'metadata never moves the footer off screen')
      await audit.locator('summary.skill-audit-toggle').click()
      await dialog.locator('.desktop-skill-files button').nth(1).click()
      await page.waitForFunction(() => document.querySelector('.skill-file-content')?.textContent.includes('RESOURCE_END'))
      assert.match(await dialog.locator('.skill-file-content').innerText(), /RESOURCE_END/)
      assert.equal(await dialog.locator('.skill-detail-content .skill-file-tabs').count(), 0, 'desktop has no duplicated file controls')
      if (candidate) {
        for (const button of await dialog.locator('.knowledge-modal-actions button').all()) {
          const box = await button.boundingBox()
          assert.ok(box && box.y >= 0 && box.y + box.height <= viewport.height && box.x >= panel.x && box.x + box.width <= panel.x + panel.width, 'all candidate actions remain accessible')
        }
      }
    }
    await dialog.getByRole('button', { name: '完成', exact: true }).click()
    assert.deepEqual(errors, [])
    results.push({ ...viewport, cards: phone ? 5 : 1, retiredSectionFallback: true, longNames: true, independentActions: true, fullSkill: true })
    await context.close()
  }
  empty = true
  candidate = false
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } })
  await page.goto(`${origin}/knowledge/settings`)
  await page.locator('.runtime-card').waitFor()
  assert.equal(await page.locator('.settings-card').count(), 5)
  assert.match(await page.locator('#settings-documents .knowledge-empty').innerText(), /选择一本书/)
  assert.match(await page.locator('#settings-skills .knowledge-empty').innerText(), /选择一本书/)
  assert.ok(requests.some(request => request.tool === 'get_agent_usage_stats'))
  assert.ok(!requests.some(request => /knowledge_backup/.test(request.tool)), 'settings never list, create or restore snapshots')
  await page.close()
  empty = false
  const dark = await browser.newPage({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce', colorScheme: 'dark' })
  await dark.addInitScript(() => localStorage.setItem('theme', 'dark'))
  await dark.goto(`${origin}/knowledge/settings?section=skills`)
  await dark.locator('.skill-card').waitFor()
  await dark.waitForFunction(() => document.querySelector('#settings-skills').getBoundingClientRect().top < innerHeight)
  assert.equal(await dark.locator('.settings-card').count(), 5)
  await dark.screenshot({ path: join(screenshots, '390-dark-reduced-motion.png') })
  await dark.getByRole('button', { name: '查看 Skill 内容', exact: true }).click()
  await dark.getByRole('dialog', { name: 'Skill 内容', exact: true }).locator('.skill-file-content').waitFor()
  assert.equal(await dark.locator('.skill-file-content').innerText(), body.trimEnd())
  await dark.close()
  const desktopDark = await browser.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce', colorScheme: 'dark' })
  await desktopDark.addInitScript(() => localStorage.setItem('theme', 'dark'))
  await desktopDark.goto(`${origin}/knowledge/settings?section=skills`)
  await desktopDark.getByRole('button', { name: '查看 Skill 内容', exact: true }).click()
  const darkDialog = desktopDark.getByRole('dialog', { name: 'Skill 内容', exact: true })
  await darkDialog.locator('.skill-file-content').waitFor()
  await desktopDark.waitForFunction(() => !document.querySelector('.motion-modal').classList.contains('is-settling'))
  await darkDialog.locator('.skill-audit-toggle').click()
  await darkDialog.locator('.skill-audit-grid').waitFor()
  await desktopDark.screenshot({ path: join(screenshots, '1440-dark-skill-metadata.png') })
  await desktopDark.close()
  assert.ok(!requests.some(request => /knowledge_backup/.test(request.tool)), 'all settings interactions avoid retired backup APIs')
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ passed: true, results, emptyLibrary: true, darkReducedMotion: true, requestedSectionLink: true, screenshots }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
