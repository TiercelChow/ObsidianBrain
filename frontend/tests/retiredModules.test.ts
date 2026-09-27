import assert from 'node:assert/strict'
import { access, readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../../', import.meta.url)
const source = (path: string) => readFile(new URL(path, root), 'utf8')

test('code repositories belong to daily navigation with no management group', async () => {
  const sidebar = await source('frontend/src/components/Sidebar.vue')
  const daily = sidebar.match(/label: '日常',\s*items: \[([\s\S]*?)\]/)?.[1] || ''
  assert.match(daily, /path: '\/code-repo', label: '代码仓'/)
  assert.doesNotMatch(sidebar, /label: '管理'|灵感熔炉|智识雷达|MagicStick|DataLine/)
})

test('retired modules have no pages or feature routes and unknown URLs return home', async () => {
  for (const page of ['Inspiration.vue', 'Radar.vue']) {
    await assert.rejects(access(new URL(`frontend/src/views/${page}`, root)), { code: 'ENOENT' })
  }
  const router = await source('frontend/src/router/index.ts')
  assert.doesNotMatch(router, /Inspiration|Radar|\/inspiration|\/radar/)
  assert.match(router, /path: '\/:pathMatch\(\.\*\)\*', redirect: '\/'/)
  assert.match(router, /name: 'CodeRepo'/)
})

test('retired tools and services are not exposed or initialized', async () => {
  for (const path of ['frontend/src/api/index.ts', 'backend/src/main.rs', 'backend/src/tools/handlers/config_handlers.rs', 'backend/src/tools/handlers/mod.rs', 'backend/src/tools/definitions.rs', 'backend/src/core/mod.rs', 'backend/src/models/mod.rs']) {
    assert.doesNotMatch(await source(path), /inspiration|radar|Inspiration|Radar|add_to_vault/i, path)
  }
  const store = await source('backend/src/infra/sqlite_store.rs')
  assert.doesNotMatch(store, /pub fn (?:insert_inspiration|get_recent_inspirations|insert_radar_item|get_radar_items|update_radar_status|radar_url_exists)\b/)
})

test('shared material and motion styles no longer carry retired selectors', async () => {
  for (const path of ['frontend/src/App.vue', 'frontend/index.html', 'frontend/src/styles/materials.css', 'frontend/src/styles/motion.css']) {
    assert.doesNotMatch(await source(path), /\.(?:radar-card|mode-option|combo-result|question-result|counterpoint-result|insight-card)\b/, path)
  }
})

test('retired implementations and dedicated design documents are deleted, not just hidden', async () => {
  for (const path of [
    'backend/src/core/inspiration/service.rs', 'backend/src/core/radar/service.rs',
    'backend/src/models/inspiration.rs', 'backend/src/models/radar.rs',
    'backend/src/tools/handlers/inspiration_handlers.rs', 'backend/src/tools/handlers/radar_handlers.rs',
    'backend/config/radar_sources.toml',
    'docs/requirement/06-inspiration.md', 'docs/requirement/07-radar.md',
    'docs/development/06-inspiration.md', 'docs/development/07-radar.md',
  ]) {
    await assert.rejects(access(new URL(path, root)), { code: 'ENOENT' }, path)
  }
  for (const path of ['README.md', 'AGENTS.md', 'docs/top_design.md', 'docs/requirement/02-tool-protocol.md', 'docs/development/02-tool-protocol.md']) {
    assert.doesNotMatch(await source(path), /灵感熔炉|智识雷达|get_inspiration|get_radar|add_to_vault|dismiss_radar_item/, path)
  }
})
