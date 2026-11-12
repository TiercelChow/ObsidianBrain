// Browser fixture for host path navigation. No request can reach user data.
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const screenshots = await mkdtemp(join(tmpdir(), 'reader-path-picker-'))
const origin = 'http://127.0.0.1:5234'
const vite = await createServer({
  root: fileURLToPath(new URL('../', import.meta.url)),
  server: { host: '127.0.0.1', port: 5234, strictPort: true, open: false },
  plugins: [{
    name: 'deny-real-data',
    configureServer(server) {
      server.middlewares.use('/v1', (_req, res) => {
        res.statusCode = 403
        res.end('Isolated reader path verification')
      })
    },
  }],
})

const drive = 'D:\\'
const share = '\\\\nas\\阅读资料'
const folder = drive + '资料'
const deep = folder + '\\很长很长的项目目录\\当前阅读文档'
const localBook = drive + '机器 学习.epub'
const remoteBook = share + '\\PDF 手册.pdf'
const dir = (name, path) => ({ name, path, is_dir: true })
const file = (name, path) => ({ name, path, is_dir: false, is_pdf: /\.pdf$/i.test(name), is_epub: /\.epub$/i.test(name) })
const dirs = new Map([
  [drive, [dir('资料', folder), file('机器 学习.epub', localBook)]],
  [folder, [dir('很长很长的项目目录', folder + '\\很长很长的项目目录')]],
  [folder + '\\很长很长的项目目录', [dir('当前阅读文档', deep)]],
  [deep, []],
  [share, [file('PDF 手册.pdf', remoteBook)]],
  ['/home/reader', [dir('藏书.v2', '/home/reader/藏书.v2')]],
  ['/home/reader/藏书.v2', [file('测试.pdf', '/home/reader/藏书.v2/测试.pdf')]],
])

let browser
const results = []
try {
  await vite.listen()
  browser = await chromium.launch({ channel: 'chromium', headless: true })
  for (const [platform, width] of [['windows', 320], ['windows', 1440], ['unix', 390]]) {
    const context = await browser.newContext({ viewport: { width, height: 860 }, isMobile: width < 768, hasTouch: width < 768 })
    const errors = [], unexpected = [], requests = []
    let saved = []
    await context.route('**/*', async (route) => {
      const url = new URL(route.request().url())
      if (url.origin !== origin) return route.abort()
      if (!url.pathname.startsWith('/v1')) return route.continue()
      if (url.pathname === '/v1/health') return route.fulfill({ json: { status: 'healthy', version: 'fixture', components: {} } })
      if (url.pathname === '/v1/reader/locations') {
        return route.fulfill({ json: platform === 'windows'
          ? { platform, home: 'C:\\Users\\Reader', roots: ['C:\\', drive] }
          : { platform, home: '/home/reader', roots: ['/'] } })
      }
      const call = route.request().postDataJSON()
      const args = call?.arguments ?? {}
      const ok = (result) => route.fulfill({ json: { status: 'success', tool: call.tool, result } })
      if (call?.tool === 'get_reader_books') return ok({ books: saved })
      if (call?.tool === 'get_reader_history') return ok({ history: [] })
      if (call?.tool === 'save_reader_books') {
        saved = args.books
        return ok({ ok: true })
      }
      if (call?.tool === 'list_local_dir') {
        requests.push(args)
        const entries = dirs.get(args.path)
        if (entries) return ok({ root: args.path, entries, total: entries.length })
        return route.fulfill({ json: { status: 'error', error: { message: '无法读取此文件夹' } } })
      }
      if (call?.tool === 'stat_local_path') {
        const is_dir = dirs.has(args.path)
        const is_file = [localBook, remoteBook, '/home/reader/藏书.v2/测试.pdf'].includes(args.path)
        return ok({ exists: is_dir || is_file, is_dir, is_file, path: args.path })
      }
      unexpected.push(call?.tool || url.pathname)
      return route.fulfill({ status: 400, json: { error: 'Unexpected request' } })
    })
    const page = await context.newPage()
    page.on('pageerror', (error) => errors.push(error.message))
    await page.goto(origin + '/reader?view=shelf')
    await page.getByRole('button', { name: '添加书籍', exact: true }).click()
    const bookForm = page.getByRole('dialog', { name: '添加书籍' })
    const browse = bookForm.getByRole('button', { name: '浏览本地路径' })
    const picker = page.getByRole('dialog', { name: '选择文件夹、PDF 或 EPUB' })
    await browse.click()
    const button = (name) => picker.getByRole('button', { name, exact: true })
    const current = (name) => picker.locator('[aria-current=location]').getByText(name, { exact: true }).waitFor()
    const paste = async (path) => {
      await button('输入路径').click()
      const input = picker.getByRole('textbox', { name: '目录或书籍路径' })
      await input.fill(path)
      await input.press('Enter')
    }
    const confirm = () => button('确定').click()
    if (platform === 'windows') {
      await current('此电脑')
      await button('C:').click()
      await picker.getByRole('alert').getByText('无法读取此文件夹').waitFor()
      await button('D:').click()
      await current('D:')
      await picker.getByRole('button', { name: /机器 学习\.epub/ }).click()
      await picker.getByRole('status').getByText('已选 机器 学习.epub').waitFor()
      await button('上一级').click()
      await current('此电脑')
      await paste(deep)
      await current('当前阅读文档')
      await page.screenshot({ path: join(screenshots, width + '-breadcrumbs.png') })
      const horizontalFit = await picker.locator('.pp-address').evaluate(el => el.scrollWidth <= el.clientWidth + 1)
      assert.equal(horizontalFit, true, 'The address toolbar must fit within the picker')
      await picker.getByRole('button', { name: /^上级目录：/ }).click()
      await page.getByRole('menuitem', { name: '资料', exact: true }).click()
      await current('资料')
      await paste('"' + remoteBook + '"')
      await current(share)
      await confirm()
      assert.equal(await bookForm.getByPlaceholder('输入路径，或点击右侧浏览').inputValue(), remoteBook)
      assert.equal(await bookForm.getByPlaceholder('留空则使用文件名').inputValue(), 'PDF 手册.pdf')
    } else {
      await current('reader')
      await button('藏书.v2').click()
      await current('藏书.v2')
      await button('选此文件夹').click()
      await confirm()
      assert.equal(await bookForm.getByPlaceholder('输入路径，或点击右侧浏览').inputValue(), '/home/reader/藏书.v2')
    }
    await bookForm.getByRole('button', { name: '保存', exact: true }).click()
    await bookForm.waitFor({ state: 'hidden' })
    assert.equal(saved[0]?.path, platform === 'windows' ? remoteBook : '/home/reader/藏书.v2')
    assert.equal(saved[0]?.kind, platform === 'windows' ? 'pdf' : 'folder')
    assert.ok(requests.every(({ depth }) => depth === 0))
    assert.deepEqual(errors, [])
    assert.deepEqual(unexpected, [])
    results.push({ platform, width, directoryRequests: requests.length })
    await context.close()
  }
  console.log(JSON.stringify({ results, screenshots }, null, 2))
} finally {
  await browser?.close()
  await vite.close()
}
