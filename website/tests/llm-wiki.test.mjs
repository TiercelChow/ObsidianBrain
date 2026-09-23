import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)
const read = (path) => readFile(new URL(path, root), 'utf8')

const chapters = ['overview', 'sources', 'compile', 'review', 'workspace', 'qa', 'tasks', 'config', 'security']

test('llm wiki page contains every architecture chapter with directory links', async () => {
  const html = await read('llm-wiki/index.html')
  for (const id of chapters) {
    assert.match(html, new RegExp(`id="${id}"`), `missing chapter #${id}`)
    assert.match(html, new RegExp(`href="#${id}"`), `missing directory link to #${id}`)
  }
})

test('llm wiki page documents the four knowledge pages as one pipeline', async () => {
  const html = await read('llm-wiki/index.html')
  for (const page of ['书籍知识库', 'Wiki 工作台', '知识问答', '研究任务']) {
    assert.match(html, new RegExp(page), `missing knowledge page: ${page}`)
  }
  for (const phrase of ['proposal', '审核', '单事务', '租约']) {
    assert.match(html, new RegExp(phrase), `missing pipeline concept: ${phrase}`)
  }
})

test('llm wiki page cites verified backend mechanisms and storage contracts', async () => {
  const html = await read('llm-wiki/index.html')
  for (const phrase of [
    'source_versions', 'source_spans', 'knowledge_compile_checkpoints', 'compile_fingerprint',
    'knowledge_change_sets', 'knowledge_entries', 'knowledge_claims', 'knowledge_relations',
    'knowledge_citations', 'agent_runs', 'agent_run_capabilities', 'agent_run_inspections',
    'knowledge_tasks', 'FTS5', 'cjk_terms', 'ACP', 'SSE', 'MCP', 'PPTX', 'OOXML',
    'knowledge_propose_changes', 'book_fetch_external', 'BEGIN IMMEDIATE',
  ]) {
    assert.match(html, new RegExp(phrase), `missing mechanism fact: ${phrase}`)
  }
  assert.match(html, /驳回[^。]*检查点/)
  assert.match(html, /外部研究[^。]*默认关闭/)
  assert.match(html, /环境变量名/)
  assert.match(html, /自动应答[^。]*Cancelled|Cancelled[^。]*自动应答/)
  assert.doesNotMatch(html, /Claude Code/)
})

test('llm wiki page is a standalone Pages entry reachable from every page nav', async () => {
  const [home, manual, page, config] = await Promise.all([
    read('index.html'), read('manual/index.html'), read('llm-wiki/index.html'), read('vite.config.js'),
  ])
  assert.match(home, /href="\.\/llm-wiki\/"/)
  assert.match(manual, /href="\.\.\/llm-wiki\/"/)
  assert.match(page, /href="\.\.\/"[^>]*aria-label="ObsidianBrain 首页"|aria-label="ObsidianBrain 首页"[^>]*href="\.\.\/"/)
  assert.match(page, /href="\.\.\/manual\/"/)
  assert.match(page, /aria-current="page"/)
  assert.match(config, /llm-wiki\/index\.html/)
})

test('llm wiki page uses repository-relative paths and never calls the local API', async () => {
  const html = await read('llm-wiki/index.html')
  assert.doesNotMatch(html, /(?:src|href)=["']\//)
  assert.doesNotMatch(html, /(?:fetch|axios)\s*\(/)
  assert.doesNotMatch(html, /\/v1\//)
})

test('llm wiki page has valid same-page anchors and unique ids', async () => {
  const html = await read('llm-wiki/index.html')
  const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1])
  assert.equal(new Set(ids).size, ids.length, 'duplicate ids')
  for (const [, id] of html.matchAll(/\bhref="#([^"]+)"/g)) {
    assert.ok(ids.includes(id), `missing anchor target #${id}`)
  }
})

test('llm wiki page reuses the shared docs shell scripts and styles', async () => {
  const html = await read('llm-wiki/index.html')
  assert.match(html, /src="\.\.\/src\/main\.js"/)
  assert.match(html, /class="manual-page/)
  assert.match(html, /data-chapter-link/)
  assert.match(html, /data-theme-toggle/)
})
