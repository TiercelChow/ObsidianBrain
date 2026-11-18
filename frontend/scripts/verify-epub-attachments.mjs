// Isolated UI fixtures. Every /v1 request is intercepted or denied.
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const directory = await mkdtemp(join(tmpdir(), 'epub-attachments-'))
const root = fileURLToPath(new URL('../', import.meta.url)),
  origin = 'http://127.0.0.1:5233'
const server = await createServer({
  root,
  configFile: join(root, 'vite.config.ts'),
  server: { host: '127.0.0.1', port: 5233, strictPort: true, open: false },
  plugins: [
    {
      name: 'deny-real-data',
      configureServer(vite) {
        vite.middlewares.use('/v1', (_req, res) => {
          res.statusCode = 403
          res.end('No real data in UI verification')
        })
      },
    },
  ],
})
const book = {
  id: 'epub',
  path: '/mock/test.epub',
  kind: 'epub',
  name: 'EPUB 阅读验证',
  description: '',
  category: '',
  addedAt: 1,
}
const node = {
  id: 'task',
  root_id: 'task',
  parent_id: null,
  kind: 'long',
  role: 'root',
  title: '附件任务',
  description: '任务说明',
  status: 'in_progress',
  importance: 'normal',
  start_date: '2026-11-01',
  end_date: '2026-12-01',
  position: 0,
  closure_note: null,
  closed_at: null,
  created_at: '2026-11-01T10:00:00Z',
  updated_at: '2026-11-01T10:00:00Z',
  revision: 1,
  archived_at: null,
}
const progress = {
  id: 'progress',
  root_id: 'task',
  task_id: 'task',
  note: '已有进展记录',
  percent_after: 20,
  recorded_at: '2026-11-02T10:00:00Z',
  created_at: '2026-11-02T10:00:00Z',
}
const detail = {
  root: node,
  tasks: [node],
  progress: [progress],
  audit: [],
  storage_path: 'db:long:task',
  document_version: { revision: 1, content_hash: 'test' },
  progress_percent: 20,
  completed_leaf_count: 0,
  effective_leaf_count: 1,
}
const childNode = {
  ...node,
  id: 'child',
  parent_id: 'task',
  role: 'subtask',
  title: '附件子任务',
  description: '子任务说明。',
}
let browser
async function assertEpubPalette(page, expectedTheme) {
  await page.waitForFunction((theme) => {
    const frame = document.querySelector('.epub-frame')
    const chapter = frame?.contentDocument
    const view = frame?.contentWindow
    const probe = chapter?.querySelector('#publisher-surface')
    const svg = chapter?.querySelector('#publisher-illustration rect')
    if (!view || !probe || !svg || document.documentElement.dataset.theme !== theme) return false
    const app = getComputedStyle(document.documentElement)
    const paint = (selector) => view.getComputedStyle(chapter.querySelector(selector))
    const token = (name) => {
      const test = document.createElement('span')
      test.style.color = `var(${name})`
      document.body.append(test)
      const color = getComputedStyle(test).color
      test.remove()
      return color
    }
    const clear = 'rgba(0, 0, 0, 0)'
    return (
      getComputedStyle(frame).colorScheme === app.colorScheme &&
      paint('html').colorScheme === app.colorScheme &&
      paint('html').backgroundColor === clear &&
      paint('body').backgroundColor === clear &&
      paint('#publisher-surface').backgroundColor === clear &&
      paint('#publisher-surface').backgroundImage === 'none' &&
      paint('#publisher-text').color === token('--text-primary') &&
      paint('#publisher-link span').color === token('--accent') &&
      paint('#publisher-code').backgroundColor === token('--code-block-bg') &&
      paint('#publisher-code span').color === token('--code-block-text') &&
      paint('#publisher-table th').backgroundColor === token('--bg-hover') &&
      paint('#publisher-note').color === token('--text-secondary') &&
      view.getComputedStyle(svg).fill === 'rgb(226, 73, 85)'
    )
  }, expectedTheme, { timeout: 2500 })
}
async function settled(page) {
  await page.waitForFunction(
    () =>
      !document.querySelector('.motion-modal.is-settling') &&
      Array.from(document.querySelectorAll('.motion-modal__panel')).every(
        (el) =>
          Math.abs(new DOMMatrixReadOnly(getComputedStyle(el).transform).m42) <
          1,
      ),
  )
  await page.evaluate(() =>
    Promise.all(
      document
        .getAnimations()
        .filter((a) => a.effect?.getTiming().iterations !== Infinity)
        .map((a) => a.finished.catch(() => {})),
    ),
  )
}
const errors = [],
  unexpected = [],
  results = []
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [320, 390, 1024, 1440]) {
    let bookmarks = [],
      attachments = [],
      sequence = 0
    let failNextUpload = false,
      progressWrites = 0
    const taskDetail = structuredClone(detail)
    taskDetail.tasks.push(childNode)
    const saved = []
    const context = await browser.newContext({
      viewport: { width, height: 950 },
      isMobile: width < 768,
      hasTouch: width < 768,
    })
    await context.route('**/*', async (route) => {
      const url = new URL(route.request().url())
      if (url.origin !== origin) return route.abort()
      if (!url.pathname.startsWith('/v1')) return route.continue()
      if (url.pathname === '/v1/health')
        return route.fulfill({
          json: { status: 'healthy', version: 'fixture', components: {} },
        })
      if (url.pathname.endsWith('/epub/epub/metadata'))
        return route.fulfill({
          json: {
            book: {
              title: 'EPUB 阅读验证',
              chapters: [
                { href: 'OPS/one.xhtml', title: '第一章' },
                { href: 'OPS/two.xhtml', title: '第二章' },
              ],
              toc: [
                { href: 'OPS/one.xhtml', title: '第一章' },
                { href: 'OPS/two.xhtml', title: '第二章' },
              ],
            },
            resource_base: '/v1/reader/epub/epub/token/',
          },
        })
      if (url.pathname.startsWith('/v1/reader/epub/epub/token/')) {
        if (url.pathname.endsWith('publisher.css')) return route.fulfill({
          contentType: 'text/css',
          body: `@layer publisher {
            html, body { background: #fff !important; color-scheme: light !important; }
            #publisher-text, #publisher-code span { color: #101010 !important; }
            #publisher-table th { background: #fff !important; }
          }
          #publisher-surface { background-image: linear-gradient(white, #eee) !important; }
          p { margin: 30px 0; }`,
        })
        const title = url.pathname.endsWith('two.xhtml') ? '第二章' : '第一章'
        return route.fulfill({
          contentType: 'text/html; charset=utf-8',
          headers: {
            'Content-Security-Policy':
              "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; sandbox allow-same-origin",
          },
          body: `<!doctype html><html><head><link rel="stylesheet" href="publisher.css"></head><body><h1>${title}</h1>
          <article id="publisher-surface" style="background-color:white !important;color:black !important;">
            <p id="publisher-text" style="color:#222 !important;">出版社自定义文字</p>
            <a id="publisher-link" href="#p12"><span style="color:#000 !important;">书内链接</span></a>
            <blockquote id="publisher-note">引文</blockquote>
            <pre id="publisher-code" style="background: #fff !important;"><code><span>const answer = 42</span></code></pre>
            <table id="publisher-table"><tr><th>表头</th></tr><tr><td>数据</td></tr></table>
            <svg id="publisher-illustration" width="32" height="20" viewBox="0 0 32 20"><rect width="32" height="20" style="fill:#e24955 !important;"/></svg>
          </article>${Array.from({ length: 42 }, (_, i) => `<p id="p${i}">${title} 第 ${i + 1} 段：阅读排版验证，手机上也应清晰显示，段落不得横向溢出。</p>`).join('')}</body></html>`,
        })
      }
      if (/^\/v1\/tasks\/(task|child)\/attachments$/.test(url.pathname)) {
        const taskId = url.pathname.split('/')[3]
        const scope = url.searchParams.get('progress_id')
        if (route.request().method() === 'POST') {
          if (failNextUpload) {
            failNextUpload = false
            return route.fulfill({
              status: 400,
              json: { message: '上传失败，请重试（隔离用例）' },
            })
          }
          const item = {
            id: `attachment-${++sequence}`,
            task_id: taskId,
            progress_id: scope,
            filename: scope ? '进展附件.txt' : '任务附件.txt',
            size_bytes: 128,
            created_at: new Date().toISOString(),
          }
          attachments.push(item)
          return route.fulfill({ json: item })
        }
        return route.fulfill({
          json: attachments.filter(
            (item) => item.progress_id === scope && item.task_id === taskId,
          ),
        })
      }
      if (url.pathname.startsWith('/v1/tasks/attachments/')) {
        const id = url.pathname.split('/').pop()
        if (route.request().method() === 'DELETE') {
          attachments = attachments.filter((item) => item.id !== id)
          return route.fulfill({ status: 204 })
        }
        return route.fulfill({
          headers: {
            'Content-Disposition': 'attachment; filename="attachment.txt"',
          },
          body: 'fixture file',
        })
      }
      if (url.pathname !== '/v1/tools/call') {
        unexpected.push(url.pathname)
        return route.fulfill({
          status: 400,
          json: { message: 'Unexpected mock API' },
        })
      }
      const call = route.request().postDataJSON(),
        args = call.arguments || {}
      let result
      switch (call.tool) {
        case 'get_reader_books':
          result = { books: [book] }
          break
        case 'get_reader_history':
          result = { history: [] }
          break
        case 'save_reader_history':
          result = { ok: true }
          break
        case 'list_local_dir':
          result = {
            root: args.path,
            total: 1,
            entries: [
              {
                name: 'test.epub',
                path: book.path,
                is_dir: false,
                is_markdown: false,
                is_pdf: false,
                is_epub: true,
              },
            ],
          }
          break
        case 'save_reader_progress':
          saved.push(args.state)
          result = { state: args.state }
          break
        case 'list_reader_bookmarks':
          result = { bookmarks }
          break
        case 'save_reader_bookmark': {
          const item = {
            ...args,
            id: `bookmark-${++sequence}`,
            revision: 1,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          }
          bookmarks.push(item)
          result = { bookmark: item }
          break
        }
        case 'list_tasks':
          result = {
            tasks: [
              {
                ...node,
                document_version: detail.document_version,
                storage_path: detail.storage_path,
                progress_percent: 20,
                completed_leaf_count: 0,
                effective_leaf_count: 1,
                derived: {
                  overdue: false,
                  active_today: true,
                  due_today: false,
                },
              },
            ],
            next_cursor: null,
          }
          break
        case 'get_task':
          result = taskDetail
          break
        case 'add_task_progress':
          progressWrites++
          taskDetail.progress.push({
            ...progress,
            id: `progress-${++sequence}`,
            note: args.note,
            recorded_at: new Date().toISOString(),
          })
          taskDetail.document_version.revision++
          result = { task: taskDetail }
          break
        case 'get_task_calendar':
          result = []
          break
        default:
          unexpected.push(call.tool)
          return route.fulfill({
            status: 400,
            json: { message: 'Unexpected tool' },
          })
      }
      return route.fulfill({
        json: { tool: call.tool, status: 'success', result },
      })
    })
    const page = await context.newPage()
    page.on('pageerror', (e) => errors.push(e.message))
    await page.goto(`${origin}/reader?book=epub&view=read`)
    const frame = page.frameLocator('.epub-frame')
    await frame.getByRole('heading', { name: '第一章', exact: true }).waitFor()
    await page.waitForFunction(
      () => document.querySelector('.epub-frame')?.offsetHeight > 1500,
    )
    assert.equal(
      await page.locator('html').evaluate((el) => el.scrollWidth > innerWidth),
      false,
    )
    for (const theme of ['light', 'dark', 'eye-care']) {
      await page.evaluate(
        async (theme) =>
          (await import('/src/stores/app.ts')).useAppStore().setTheme(theme),
        theme,
      )
      await assertEpubPalette(page, theme)
      await page.evaluate(() =>
        Promise.all(
          document
            .getAnimations()
            .filter((a) => a.effect?.getTiming().iterations !== Infinity)
            .map((a) => a.finished.catch(() => {})),
        ),
      )
      await page.screenshot({
        path: join(directory, `epub-${width}-${theme}.png`),
      })
    }
    await frame.locator('#p12').scrollIntoViewIfNeeded()
    await frame.locator('#p12').click()
    await page
      .getByRole('button', { name: width < 768 ? /，本书书签$/ : /^本书书签$/ })
      .first()
      .click()
    await page.getByRole('button', { name: '添加当前位置书签' }).click()
    assert.equal(
      await page
        .getByRole('textbox', { name: '笔记', exact: true })
        .inputValue(),
      '',
    )
    await page.getByLabel('标题', { exact: true }).fill('EPUB 重点')
    await page.getByRole('button', { name: '保存', exact: true }).click()
    assert.equal(bookmarks[0].location.kind, 'epub')
    assert.equal(bookmarks[0].location.chapter, 'OPS/one.xhtml')
    await page.getByRole('button', { name: '关闭书签' }).click()
    const ribbon = page.getByRole('button', { name: '查看书签：EPUB 重点', exact: true })
    await ribbon.locator('.reader-bookmark-tail').waitFor()
    await ribbon.click()
    await page.getByRole('button', { name: '编辑书签', exact: true }).waitFor()
    await page.getByRole('button', { name: '关闭书签' }).click()
    await page
      .locator('.epub-chapter-bar')
      .getByRole('button', { name: '下一章' })
      .click()
    await frame.getByRole('heading', { name: '第二章', exact: true }).waitFor()
    await ribbon.waitFor({ state: 'detached' })
    await assertEpubPalette(page, 'eye-care')
    await page
      .getByRole('button', { name: width < 768 ? /，本书书签$/ : /^本书书签$/ })
      .first()
      .click()
    await page
      .getByRole('button', { name: '查看笔记：EPUB 重点', exact: true })
      .click()
    await page.getByRole('button', { name: '转到原文' }).click()
    await frame.getByRole('heading', { name: '第一章', exact: true }).waitFor()
    await ribbon.locator('.reader-bookmark-tail').waitFor()
    await page.waitForFunction(
      () => document.querySelector('.pane-center').scrollTop > 500,
    )
    await page.locator('.pane-center').dispatchEvent('wheel', { deltaY: 20 })
    await page.locator('.pane-center').evaluate((el) => {
      el.scrollTop = (el.scrollHeight - el.clientHeight) * 0.65
    })
    await page.waitForFunction(() => {
      const states = JSON.parse(
        localStorage.getItem('obsidian-brain:reader-progress') || '{}',
      )
      return (
        Math.abs(
          (states.epub?.byFile?.['/mock/test.epub']?.position ?? -1) - 0.325,
        ) < 0.03
      )
    })
    const readingSurface = await page.locator('.epub-frame').elementHandle()
    const readingScrollTop = await page.locator('.pane-center').evaluate((el) => el.scrollTop)
    await page.evaluate(async () => (await import('/src/stores/app.ts')).useAppStore().setTheme('dark'))
    await assertEpubPalette(page, 'dark')
    assert.equal(await readingSurface.evaluate((el) => el.isConnected), true)
    assert.equal(await page.locator('.pane-center').evaluate((el) => el.scrollTop), readingScrollTop)
    await readingSurface.dispose()
    await page.reload()
    await frame.getByRole('heading', { name: '第一章', exact: true }).waitFor()
    await assertEpubPalette(page, 'dark')
    await page.waitForFunction(() => {
      const pane = document.querySelector('.pane-center')
      return (
        pane &&
        Math.abs(
          pane.scrollTop / Math.max(1, pane.scrollHeight - pane.clientHeight) -
            0.65,
        ) < 0.05
      )
    })
    await page.goto(`${origin}/tasks?task=task`)
    await page.getByRole('heading', { name: '附件任务', exact: true }).waitFor()
    await settled(page)
    const overview = page
      .locator(width < 768 ? '.mobile-task-overview' : '.task-overview')
      .first()
    const info = await overview
      .locator(width < 768 ? '.mobile-task-info' : '.task-overview-info')
      .boundingBox()
    const files = await overview.locator('.task-attachments').boundingBox()
    if (width < 768) {
      assert.ok(
        files.y >= info.y + info.height - 1,
        'Phone attachments follow task information vertically',
      )
      assert.ok(
        Math.abs(files.x - info.x) < 1 &&
          Math.abs(files.width - info.width) < 1,
        `Phone attachments use the full detail width: ${JSON.stringify({ info, files })}`,
      )
    } else {
      const breakdown = await page.locator('.task-breakdown').boundingBox()
      const progress = await page.locator('.task-progress-panel').boundingBox()
      assert.ok(
        Math.abs(info.x - breakdown.x) < 1 &&
          Math.abs(info.width - breakdown.width) < 1,
        'Task information shares the breakdown column',
      )
      assert.ok(
        Math.abs(files.x - progress.x) < 1 &&
          Math.abs(files.width - progress.width) < 1,
        'Attachments share the progress column',
      )
      const title = await overview.locator('h2').boundingBox()
      const breakdownHeading = await page
        .locator('.task-breakdown .column-heading > div')
        .boundingBox()
      const attachmentHeading = await overview
        .locator('.task-attachments > header > span')
        .boundingBox()
      const progressHeading = await page
        .locator('.progress-column-header > div')
        .boundingBox()
      assert.ok(
        Math.abs(title.x - breakdownHeading.x) < 1,
        'Left headings align',
      )
      assert.ok(
        Math.abs(attachmentHeading.x - progressHeading.x) < 1,
        'Right headings align',
      )
      const attachButton = await overview
        .getByRole('button', { name: '上传附件', exact: true })
        .boundingBox()
      const progressButton = await page
        .locator('.progress-column-header')
        .getByRole('button', { name: '记录进展', exact: true })
        .boundingBox()
      assert.ok(
        Math.abs(attachButton.x - progressButton.x) < 1,
        'Right actions align',
      )
    }
    for (const button of await page
      .getByRole('button', {
        name: width < 768 ? '添加子任务' : /^(添加子任务|记录进展)$/,
        exact: true,
      })
      .all()) {
      if (!(await button.isVisible())) continue
      const rect = await button.boundingBox()
      assert.ok(
        Math.abs(rect.width - rect.height) < 1,
        'Task add feedback is circular',
      )
      await button.hover()
      assert.equal(
        await button.evaluate((el) => getComputedStyle(el).borderRadius),
        '50%',
      )
    }
    await page.screenshot({
      path: join(directory, `task-overview-${width}.png`),
    })
    await page.getByText('附件子任务', { exact: true }).first().click()
    const childPanel = page.locator(
      width < 768 ? '.mobile-task-detail.is-child' : '.child-task-dialog',
    )
    await childPanel
      .getByRole('heading', { name: '附件子任务', exact: true })
      .waitFor()
    await settled(page)
    const childOverview = childPanel.locator(
      width < 768 ? '.mobile-task-overview' : '.child-overview',
    )
    const childInfo = await childOverview
      .locator(width < 768 ? '.mobile-task-info' : '.child-task-info')
      .boundingBox()
    const childFiles = await childOverview
      .locator('.task-attachments')
      .boundingBox()
    assert.ok(
      childFiles.y >= childInfo.y + childInfo.height - 1,
      'Subtask attachments stack below the information at every width',
    )
    assert.ok(
      Math.abs(childFiles.x - childInfo.x) < 1 &&
        Math.abs(childFiles.width - childInfo.width) < 1,
    )
    assert.ok(
      childFiles.y >= 0 && childFiles.y + childFiles.height < 950,
      'Child attachments are inside the visible sheet',
    )
    for (const action of await childPanel
      .getByRole('button', { name: /^(添加子任务|记录进展)$/ })
      .all()) {
      const rect = await action.boundingBox()
      assert.ok(Math.abs(rect.width - rect.height) < 1)
    }
    assert.equal(
      await page.locator('html').evaluate((el) => el.scrollWidth > innerWidth),
      false,
    )
    await page.evaluate(() =>
      Promise.all(
        document
          .getAnimations()
          .filter((a) => a.effect?.getTiming().iterations !== Infinity)
          .map((a) => a.finished.catch(() => {})),
      ),
    )
    await page.screenshot({
      path: join(directory, `child-overview-${width}.png`),
    })
    await childPanel
      .getByRole('button', { name: '关闭子任务详情', exact: true })
      .click()
    await childPanel.waitFor({ state: 'hidden' })
    const area = page.locator('.task-attachments').first()
    await area.locator('input[type=file]').setInputFiles({
      name: 'example.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('fixture'),
    })
    await area
      .getByRole('link', { name: '下载 任务附件.txt', exact: true })
      .waitFor()
    const download = page.waitForEvent('download')
    await area
      .getByRole('link', { name: '下载 任务附件.txt', exact: true })
      .click()
    assert.equal((await download).suggestedFilename(), '任务附件.txt')
    if (width < 768) {
      await page.getByRole('tab', { name: /进展/ }).click()
      await page.evaluate(() =>
        document.documentElement.style.setProperty('--safe-bottom', '20px'),
      )
    }
    await page.getByText('已有进展记录', { exact: true }).first().click()
    const activity = page.locator('.activity-dialog')
    await activity.waitFor()
    await settled(page)
    if (width < 768) {
      const bounds = await activity.boundingBox()
      assert.ok(
        Math.abs(bounds.x) < 1 && Math.abs(bounds.width - width) < 1,
        'Progress detail surface covers the entire width',
      )
      assert.ok(
        Math.abs(bounds.y + bounds.height - 950) < 1,
        'Progress detail surface reaches the bottom edge',
      )
      assert.equal(
        await activity.evaluate((el) => getComputedStyle(el).paddingBottom),
        '40px',
        'The bottom safe area stays inside the progress surface',
      )
    }
    await activity.locator('input[type=file]').setInputFiles({
      name: 'progress.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('progress'),
    })
    await activity
      .getByRole('link', { name: '下载 进展附件.txt', exact: true })
      .waitFor()
    assert.equal(
      attachments.filter((item) => item.progress_id === 'progress').length,
      1,
    )
    await activity.getByRole('button', { name: '移除 进展附件.txt' }).click()
    await page.getByRole('button', { name: '移除', exact: true }).click()
    await activity.getByText('暂无附件', { exact: true }).waitFor()
    assert.equal(
      attachments.filter((item) => item.progress_id === null).length,
      1,
    )
    await page
      .getByRole('dialog', { name: '移除附件', exact: true })
      .waitFor({ state: 'hidden' })
    await page.screenshot({ path: join(directory, `attachments-${width}.png`) })
    await page.keyboard.press('Escape')
    await activity.waitFor({ state: 'hidden' })
    if (width < 768)
      await page.evaluate(() =>
        document.documentElement.style.removeProperty('--safe-bottom'),
      )
    await page
      .getByRole('button', { name: '记录进展', exact: true })
      .first()
      .click()
    const draftNote =
      width === 320
        ? [
            '新增进展并上传附件',
            ...Array.from(
              { length: 40 },
              (_, i) => `第 ${i + 1} 行，验证长记录可滚动到附件。`,
            ),
          ].join('\n')
        : '新增进展并上传附件'
    await page
      .getByRole('textbox', { name: '进展说明', exact: true })
      .fill(draftNote)
    const pendingFile = {
      name: 'new-progress.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('retryable attachment'),
    }
    const editor = page.locator('.task-sheet')
    await editor
      .locator('.progress-attachment-picker input[type=file]')
      .setInputFiles(pendingFile)
    await editor.getByText('new-progress.txt', { exact: true }).waitFor()
    await settled(page)
    const pickerBox = await editor
      .locator('.progress-attachment-picker')
      .boundingBox()
    assert.ok(
      pickerBox.y >= 0 && pickerBox.y + pickerBox.height < 950,
      'Attachments are visible inside the progress editor',
    )
    await page.screenshot({
      path: join(directory, `progress-editor-${width}.png`),
    })
    assert.equal(
      progressWrites,
      0,
      'Attachment is selected before creating the progress',
    )
    failNextUpload = true
    await editor.getByRole('button', { name: '记录', exact: true }).click()
    await editor
      .getByRole('alert')
      .filter({ hasText: '部分附件上传失败' })
      .waitFor()
    assert.equal(
      await editor
        .getByRole('textbox', { name: '进展说明', exact: true })
        .isDisabled(),
      true,
    )
    assert.equal(
      await activity.count(),
      0,
      'No post-save attachment dialog is opened',
    )
    assert.equal(progressWrites, 1)
    await editor.getByRole('button', { name: '重试上传', exact: true }).click()
    await editor.waitFor({ state: 'hidden' })
    assert.equal(
      progressWrites,
      1,
      'Retrying an attachment must not create another progress entry',
    )
    await page.getByText(draftNote, { exact: true }).first().click()
    await activity
      .getByRole('link', { name: '下载 进展附件.txt', exact: true })
      .waitFor()
    await settled(page)
    if (width === 320) {
      const head = await activity.locator('.activity-dialog-head').boundingBox()
      await activity
        .getByRole('link', { name: '下载 进展附件.txt', exact: true })
        .scrollIntoViewIfNeeded()
      assert.ok(
        await activity
          .locator('.activity-dialog-scroll')
          .evaluate(
            (el) => el.scrollHeight > el.clientHeight && el.scrollTop > 0,
          ),
        'Long progress notes scroll inside the sheet to reveal the attachment',
      )
      assert.equal(
        (await activity.locator('.activity-dialog-head').boundingBox()).y,
        head.y,
        'Record heading stays above the scroll area',
      )
    }
    await page.evaluate(() =>
      Promise.all(
        document
          .getAnimations()
          .filter((a) => a.effect?.getTiming().iterations !== Infinity)
          .map((a) => a.finished.catch(() => {})),
      ),
    )
    await page.screenshot({
      path: join(directory, `progress-attachment-${width}.png`),
    })
    results.push({
      width,
      bookmarks: bookmarks.length,
      taskAttachments: attachments.filter((item) => item.progress_id === null)
        .length,
      progressAttachments: attachments.filter(
        (item) => item.progress_id !== null,
      ).length,
      readingProgressWrites: saved.length,
      progressWrites,
    })
    await context.close()
  }
  assert.deepEqual(errors, [])
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ results, screenshots: directory }, null, 2))
} finally {
  await browser?.close()
  await server.close()
}
