import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, access } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)
const read = (path) => readFile(new URL(path, root), 'utf8')
const chapters = ['intro', 'timeline', 'wiki-dashboard', 'wiki-workbench', 'explore', 'ingest', 'knowledge', 'code-repo', 'config', 'workflow']

test('migration preserves all ten original manual chapters byte for byte', async () => {
  const html = await read('manual/index.html')
  const body = html.match(/<!-- migrated-manual:start -->([\s\S]*?)<!-- migrated-manual:end -->/)?.[1]
  assert.ok(body, 'keep explicit boundaries around the unmodified legacy content')
  // Snapshot of Manual.vue content at migration; update only after the content review.
  assert.equal(createHash('sha256').update(body).digest('hex'), '24b2b8d4584410115062ea04631f3621a5c2d813fee73639289bef34e81e9a16')
  for (const id of chapters) {
    assert.match(html, new RegExp(`id="${id}"`))
    assert.match(html, new RegExp(`href="#${id}"`))
  }
  assert.match(html, /迁移版/)
  assert.match(html, /内容待核实/)
})

test('manual is a standalone Pages entry with onboarding and return navigation', async () => {
  const [home, manual, config] = await Promise.all([read('index.html'), read('manual/index.html'), read('vite.config.js')])
  assert.match(home, /href="\.\/manual\/"/)
  assert.match(manual, /href="\.\.\/"/)
  assert.match(manual, /id="getting-started"/)
  assert.match(manual, /LLM 配置/)
  assert.match(manual, /obsidian-brain config show/)
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
