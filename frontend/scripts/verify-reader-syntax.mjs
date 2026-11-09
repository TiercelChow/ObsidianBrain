// Isolated real Reader/Worker/preview verification. All /v1 requests are
// synthetic; a server-side deny guard also prevents forwarding to user data.
// From frontend: PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node scripts/verify-reader-syntax.mjs
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const examples = [
  ['typescript', '// 类型与泛型\ninterface Book { title: string }\nexport function read(book: Book): string {\n  const limit = 128;\n  return `阅读 ${book.title}`;\n}'],
  ['python', '# 中文注释\ndef greet(name: str):\n    return "Hello, " + name\ncount = 128'],
  ['rust', '// 借用与类型\nfn main() {\n    let count: u32 = 128;\n    println!("{}", count);\n}'],
  ['javascript', 'const title = "Reader";\nfunction read() { return true; }'],
  ['json', '{ "title": "阅境轩", "enabled": true, "count": 128 }'],
  ['html', '<section class="reader"><h1>阅读</h1></section>'],
  ['css', '.reader { color: #ffffff; display: grid; }'],
  ['bash', '# 只展示，不执行\necho "$HOME"\nexport READER_MODE="preview"'],
  ['sql', "SELECT title FROM books WHERE id = 128 AND status = 'ready';"],
  ['cpp', '#include <string>\nint main() { const auto count = 128; return count; }'],
  ['diff', '@@ -1 +1 @@\n-old title\n+new title'],
  ['text', 'plain_text $x_i$ <unsafe> \"quotes\" & escapes'],
  ['not-a-language', 'unknown_language <unsafe> $x_i$ & text'],
  ['', 'function greet() { return "Hello"; }'],
]
const fence = (language, code) => `\`\`\`${language}\n${code}\n\`\`\``
const markdown = '# 代码高亮\n\n行内 `npm run_build` 与公式 $x_i^2$ 保持原样。\n\n[打开代码预览](./example.ts)\n\n'
  + examples.map(([language, code]) => `## ${language || 'auto'}\n\n${fence(language, code)}`).join('\n\n')
  + '\n\n## 长代码\n\n' + fence('javascript', `const longLine = "${'reading '.repeat(45)}";`)
const book = { id: 'syntax-preview', name: '代码高亮', path: '/mock/syntax', kind: 'folder', description: '', category: '', addedAt: 1 }
const mockResults = {
  get_reader_books: { books: [book] }, get_reader_history: { history: [] },
  list_reader_bookmarks: { bookmarks: [] },
  list_local_dir: { root: book.path, total: 1, entries: [{ name: 'code.md', path: `${book.path}/code.md`, is_dir: false, is_markdown: true, is_pdf: false }] },
  save_reader_history: { ok: true, count: 1 },
  get_memo_stats: { total_memos: 1 },
  browse_timeline: { memos: [{ id: 'syntax-memo', content: fence('typescript', examples[0][1]), timestamp: '2026-10-31T08:00:00Z', tags: [], images: [], revision: 1 }], total: 1, has_more: false },
  get_timeline_storage: { cache_limit_bytes: 268435456, cache_bytes: 0, originals_bytes: 0, originals_count: 0, pending_cleanup: 0, missing_images: [] },
}
const vite = await createServer({
  root: fileURLToPath(new URL('../', import.meta.url)),
  configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)),
  plugins: [{ name: 'deny-real-api', configureServer(server) {
    server.middlewares.use('/v1', (_request, response) => {
      response.statusCode = 403
      response.end('Isolated syntax test: real API access denied')
    })
  } }],
  server: { host: '127.0.0.1', port: 5231, strictPort: true, open: false },
})
let browser
const errors = [], unexpected = [], results = []
const screenshots = await mkdtemp(join(tmpdir(), 'reader-syntax-'))
try {
  await vite.listen()
  const origin = `http://127.0.0.1:${vite.httpServer.address().port}`
  browser = await chromium.launch({ headless: true, channel: 'chromium' })
  for (const width of [390, 1440]) {
    const context = await browser.newContext({ viewport: { width, height: 1000 }, isMobile: width < 768, hasTouch: width < 768 })
    await context.addInitScript(() => localStorage.setItem('theme', 'light'))
    await context.route('**/*', async route => {
      const url = new URL(route.request().url())
      if (url.origin !== origin) return route.abort()
      if (!url.pathname.startsWith('/v1')) return route.continue()
      if (url.pathname === '/v1/health') return route.fulfill({ json: { version: 'syntax-preview', status: 'healthy', components: {} } })
      const call = route.request().postDataJSON()
      let result = mockResults[call?.tool]
      if (call?.tool === 'read_local_file') result = { path: call.arguments.path, name: call.arguments.path.split('/').pop(), content: call.arguments.path.endsWith('.ts') ? examples[0][1] : markdown, size: markdown.length }
      if (call?.tool === 'stat_local_path') result = { path: call.arguments.path, name: 'example.ts', exists: true, is_dir: false, is_file: true, ext: 'ts', size: 200 }
      if (call?.tool === 'save_reader_progress') result = { state: call.arguments.state }
      if (!result) { unexpected.push(url.pathname + ' ' + call?.tool); return route.fulfill({ status: 400, json: { error: 'Unexpected mock request' } }) }
      return route.fulfill({ json: { tool: call.tool, status: 'success', result } })
    })
    const page = await context.newPage()
    page.on('pageerror', error => errors.push(error.message))
    const openReader = () => page.goto(`${origin}/reader?book=${book.id}&view=read`)
    await openReader()
    const codes = page.locator('article.markdown-body .code-block code')
    await codes.first().waitFor()
    assert.equal(await codes.count(), examples.length + 1)
    await page.waitForFunction(() => document.querySelector('article.markdown-body code.language-typescript')?.dataset.highlighted === 'yes')
    assert.equal(await codes.last().getAttribute('data-highlighted'), null, 'Offscreen code remains lazy')
    for (let index = 0; index < examples.length; index++) {
      const code = codes.nth(index)
      await code.scrollIntoViewIfNeeded()
      const [language, raw] = examples[index]
      await page.waitForFunction(index => !!document.querySelectorAll('article.markdown-body .code-block code')[index]?.dataset.highlighted, index)
      assert.equal(await code.getAttribute('data-highlighted'), language === 'not-a-language' ? 'skipped' : 'yes')
      assert.equal(await code.textContent(), raw + '\n', `${language} source preserved`)
      if (!['text', 'not-a-language'].includes(language)) assert.ok(await code.locator('span[class*="hljs-"]').count(), `${language} tokenized`)
      else assert.equal(await code.locator('span').count(), 0, 'Plain/unsupported languages remain literal')
    }
    await codes.last().scrollIntoViewIfNeeded()
    const wide = codes.last().locator('..')
    assert.equal(await wide.evaluate(element => element.scrollWidth > element.clientWidth), true)
    await wide.evaluate(element => { element.scrollLeft = element.scrollWidth })
    assert.ok(await wide.evaluate(element => element.scrollLeft > 0), 'Long code scrolls to the right')
    const article = page.locator('article.markdown-body')
    assert.ok(await article.locator('.katex').count())
    assert.equal(await article.locator('p > code').textContent(), 'npm run_build')
    await article.evaluate(element => { element.closest('.pane-center').scrollTop = 0 })
    await codes.first().scrollIntoViewIfNeeded()
    await page.evaluate(() => { window.__syntaxToken = document.querySelector('article .hljs-keyword') })
    const palettes = []
    for (const theme of ['light', 'dark', 'eye-care']) {
      await page.evaluate(async theme => { const { useAppStore } = await import('/src/stores/app.ts'); useAppStore().setTheme(theme) }, theme)
      await page.waitForFunction(theme => document.documentElement.dataset.theme === theme, theme)
      await page.evaluate(async () => {
        await Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => {})))
      })
      palettes.push(await codes.first().evaluate(code => ({ background: getComputedStyle(code.closest('.code-block')).backgroundColor, keyword: getComputedStyle(code.querySelector('.hljs-keyword')).color, string: getComputedStyle(code.querySelector('.hljs-string')).color })))
      assert.equal(await page.evaluate(() => window.__syntaxToken === document.querySelector('article .hljs-keyword')), true, 'Theme changes reuse token DOM')
      assert.equal(await article.locator('.hljs-keyword .hljs-keyword').count(), 0, 'No duplicate highlighting')
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false)
      await page.screenshot({ path: join(screenshots, `${width}-${theme}.png`) })
    }
    assert.equal(new Set(palettes.map(palette => palette.background)).size, 3)
    assert.equal(new Set(palettes.map(palette => palette.keyword)).size, 3)
    for (const palette of palettes) assert.notEqual(palette.keyword, palette.string)
    await article.getByRole('link', { name: '打开代码预览' }).click()
    const preview = page.locator('.ppm-code-block code')
    await preview.waitFor()
    await page.waitForFunction(() => document.querySelector('.ppm-code-block code')?.dataset.highlighted === 'yes')
    assert.ok(await preview.locator('.hljs-keyword').count())
    assert.equal(await preview.textContent(), examples[0][1])
    await page.getByRole('button', { name: '关闭', exact: true }).click()
    // Navigate inside the SPA so Timeline's CSS stays loaded on return.
    await page.evaluate(async () => (await import('/src/router/index.ts')).default.push('/timeline'))
    await page.locator('.memo-code .hljs-keyword').first().waitFor()
    await page.evaluate(async () => (await import('/src/router/index.ts')).default.push('/reader?book=syntax-preview&view=read'))
    await codes.first().waitFor()
    await page.waitForFunction(() => !!document.querySelector('article .hljs-keyword'))
    const afterTimeline = await codes.first().evaluate(code => getComputedStyle(code.querySelector('.hljs-keyword')).color)
    assert.equal(afterTimeline, palettes[2].keyword, 'Visiting Timeline does not overwrite Reader colors')
    results.push({ width, themes: palettes.length, languages: examples.length, lazy: true, preview: true, routeOrder: true })
    await context.close()
  }
  assert.deepEqual(errors, [])
  assert.deepEqual(unexpected, [])
  console.log(JSON.stringify({ results, screenshots, errors, unexpected }, null, 2))
} finally {
  await browser?.close()
  await vite.close()
}
