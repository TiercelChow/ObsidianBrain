import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)
const read = (path) => readFile(new URL(path, root), 'utf8')
const productChapters = ['reader', 'timeline', 'tasks']
const chapters = ['install', ...productChapters]
const removedChapters = ['intro', 'wiki-dashboard', 'wiki-workbench', 'explore', 'ingest', 'knowledge', 'code-repo', 'config', 'workflow', 'getting-started']

test('manual contains installation plus only the three verified product chapters', async () => {
  const html = await read('manual/index.html')
  const body = html.match(/<!-- current-manual:start -->([\s\S]*?)<!-- current-manual:end -->/)?.[1]
  assert.ok(body, 'keep explicit boundaries around the current manual content')
  for (const id of chapters) {
    assert.match(html, new RegExp(`id="${id}"`))
    assert.match(html, new RegExp(`href="#${id}"`))
  }
  for (const id of removedChapters) {
    assert.doesNotMatch(body, new RegExp(`id="${id}"`))
    assert.doesNotMatch(html, new RegExp(`href="#${id}"`))
  }
  assert.match(html, /功能部分只介绍阅境轩、时光机和任务中枢/)
  assert.doesNotMatch(html, /迁移版|内容待核实/)
})

test('manual records the current storage and interaction boundaries', async () => {
  const html = await read('manual/index.html')
  assert.match(html, /阅读进度保存在当前浏览器的 <code>localStorage<\/code>/)
  assert.match(html, /Timeline\/YYYY-MM\.md/)
  assert.match(html, /每次加载 20 条/)
  assert.match(html, /最近 3 个月/)
  assert.match(html, /暂不提供已发布小记的编辑和删除操作/)
  assert.match(html, /task_documents/)
  assert.match(html, /不会写入 Obsidian 的 <code>Tasks\/<\/code> 文件夹/)
  assert.match(html, /任务数据不能从 Vault 重建/)
})

test('manual covers native macOS architectures and Windows x86_64 installation', async () => {
  const html = await read('manual/index.html')
  for (const phrase of ['Apple Silicon（arm64）', 'Intel（x86_64）', 'Windows 10 / 11', 'x86_64-pc-windows-msvc', 'make install', 'frontend/dist_new/', 'obsidian-brain.exe', 'http://localhost:9876']) {
    assert.match(html, new RegExp(phrase.replace(/[/.]/g, '\\$&')), `missing installation fact: ${phrase}`)
  }
  assert.match(html, /尚未在 GitHub Releases 提供预编译安装包/)
  assert.match(html, /Windows ARM64 当前没有经过构建和运行验证/)
})

test('manual is a standalone Pages entry with current homepage navigation', async () => {
  const [home, manual, config] = await Promise.all([read('index.html'), read('manual/index.html'), read('vite.config.js')])
  assert.match(home, /href="\.\/manual\/"/)
  for (const id of productChapters) assert.match(home, new RegExp(`href="\\.\\/manual\\/#${id}"`))
  assert.match(manual, /href="\.\.\/"/)
  assert.match(config, /manual\/index\.html/)
  assert.doesNotMatch(manual, /(?:src|href)=["']\//)
})

test('application no longer bundles the manual view or advertises its route', async () => {
  const [router, sidebar] = await Promise.all([
    read('../frontend/src/router/index.ts'), read('../frontend/src/components/Sidebar.vue'),
  ])
  assert.doesNotMatch(`${router}\n${sidebar}`, /\/manual|views\/Manual\.vue/)
  await assert.rejects(access(new URL('../frontend/src/views/Manual.vue', root)), { code: 'ENOENT' })
})

test('manual and homepage have valid same-page anchors and unique ids', async () => {
  for (const page of ['index.html', 'manual/index.html']) {
    const html = await read(page)
    const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1])
    assert.equal(new Set(ids).size, ids.length, `${page}: duplicate ids`)
    for (const [, id] of html.matchAll(/\bhref="#([^"]+)"/g)) {
      assert.ok(ids.includes(id), `${page}: missing #${id}`)
    }
  }
})
