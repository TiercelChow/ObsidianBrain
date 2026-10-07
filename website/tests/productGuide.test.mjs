import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('hero names the four everyday jobs before the decorative preview', async () => {
  const home = await read('index.html')
  const first = home.split('<div class="hero-product"')[0]
  for (const text of ['Markdown', 'PDF', '小记', '任务', '问答', '研究']) assert.ok(first.includes(text), text)
  assert.match(first, /class="hero-feature-map"/)
  for (const chapter of ['reader', 'timeline', 'tasks', 'wiki']) assert.ok(first.includes(`./manual/#${chapter}`), chapter)
})

test('homepage uses functional copy and leaves installation commands in the handbook', async () => {
  const home = await read('index.html')
  for (const phrase of ['继续上次阅读', '编辑和删除', '多级子任务', '研究报告', '下载 PPTX', '最近阅读', '代码仓']) assert.ok(home.includes(phrase), phrase)
  assert.doesNotMatch(home, /git clone|make install|CUSTOM_LLM_API_KEY|真实样例基准/)
  assert.doesNotMatch(home, /把散落的知识|变成自己的力量|API Key 仅从环境变量读取|Vault、文集与 PDF/)
})

test('all homepage manual links land at a real handbook heading', async () => {
  const [home, manual] = await Promise.all([read('index.html'), read('manual/index.html')])
  const links = [...home.matchAll(/href="\.\/manual\/#([^"]+)"/g)].map(m => m[1])
  for (const id of links) assert.ok(manual.includes(`id="${id}"`), id)
  for (const id of ['start', 'wiki-setup', 'wiki-compile', 'wiki-query', 'wiki-research', 'wiki-skills', 'wiki-backup']) {
    assert.ok(manual.includes(`id="${id}"`), id)
    assert.ok(manual.includes(`href="#${id}"`), `navigation to ${id}`)
  }
})

test('handbook follows the current icons, five-tab Dock and service-backed reading state', async () => {
  const manual = await read('manual/index.html')
  for (const phrase of ['从这里开始', '首次使用', '铅笔', '五个', '搜索当前目录文件', '约 1.5 秒', '服务端 SQLite', '系统凭据库', '环境变量模式', '模型供应商', '＋', 'npm ci']) assert.ok(manual.includes(phrase), phrase)
  assert.doesNotMatch(manual, /不会自动同步到另一台设备|直接输入本地文件夹路径|点击「＋ 添加」|点击右上角「写小记」|API Key[^。]*只通过环境变量/)
})

test('guide entry cards fit small screens and anchored steps clear the fixed navigation', async () => {
  const [home, manual] = await Promise.all([read('src/home.css'), read('src/manual.css')])
  assert.match(home, /\.home-page \.hero h1\s*\{[^}]*text-wrap: balance/)
  assert.match(manual, /\.manual-jobs\s*\{[^}]*minmax\(0, 1fr\)/)
  assert.match(manual, /\.manual-subsections\s*\{[^}]*flex-wrap: wrap/)
  assert.match(manual, /\.manual-section h3\s*\{[^}]*scroll-margin-top: calc\(9rem/)
  assert.doesNotMatch(manual, /\.manual-steps strong\s*\{/)
})
