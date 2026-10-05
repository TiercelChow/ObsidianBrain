import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('home is a status dashboard without the retired system configuration cards or requests', async () => {
  const home = await source('views/Home.vue')
  assert.match(home, /今日关注/)
  assert.match(home, /系统概况/)
  for (const retired of ['系统配置', 'TimelineStoragePanel', 'LLM 配置', 'ConfigData', 'getConfig', 'saveConfig', 'verify_llm', 'saveSettings', 'config-grid', 'config-card', 'config-actions']) {
    assert.ok(!home.includes(retired), `home must not keep ${retired}`)
  }
  for (const active of ['getHomeOverview(', 'toggleTheme()', 'formatUptime', '我的任务', '最近阅读', 'Wiki 动态', '最近小记']) {
    assert.ok(home.includes(active), `keep ${active}`)
  }
})

test('photo controls remain available in Timeline and model settings remain in Wiki', async () => {
  const [timeline, panel, wiki, api] = await Promise.all([
    source('views/Timeline.vue'), source('components/timeline/TimelineStoragePanel.vue'),
    source('views/knowledge/WikiSettings.vue'), source('api/index.ts'),
  ])
  assert.match(timeline, /<TimelineStoragePanel/)
  assert.match(panel, /saveConfig\(\{ timeline: \{ cache_limit_mb:/)
  assert.match(panel, /clearTimelineImageCache\(/)
  assert.match(wiki, /Agent Runtime/)
  assert.match(api, /export function saveConfig\(/)
  assert.doesNotMatch(api, /export function getConfig\(/)
})

test('shared styles have no obsolete home configuration card or footer selectors', async () => {
  for (const path of ['App.vue', 'styles/materials.css']) {
    assert.doesNotMatch(await source(path), /\.(?:config-card|config-actions)\b/)
  }
})

test('phone dashboard neither displays code repository stats nor requests their metadata', async () => {
  const home = await source('views/Home.vue')
  assert.doesNotMatch(home, /listCodeRepos|代码仓|已注册工具|getTimelineStorage/)
  assert.match(home, /usePhoneViewport\(/)
  assert.match(home, /grid-template-areas:'tasks' 'reading' 'wiki' 'memos'/)
})

test('appearance remains visible in the left phone header slot without competing with refresh', async () => {
  const home = await source('views/Home.vue')
  assert.match(home, /class="home-theme-action"/)
  assert.match(home, /@media \(max-width: 768px\)[\s\S]*\.home-page \.header-actions \.home-theme-action\s*\{[^}]*display: inline-flex;[^}]*position: fixed;[^}]*left: max\(12px, var\(--safe-left\)\)/)
  assert.match(home, /\.home-theme-action\s*\{[^}]*width: var\(--tap-target\)/)
})

test('desktop dashboard stretches both columns without imposing fixed card heights on phones', async () => {
  const home = await source('views/Home.vue')
  assert.match(home, /\.workbench-grid\s*\{[^}]*align-items:stretch/)
  assert.match(home, /\.workbench-main,\.workbench-side\s*\{[^}]*display:flex;[^}]*flex-direction:column/)
  assert.match(home, /\.workbench-main\s*>\s*\.home-panel,\.workbench-side\s*>\s*\.home-panel\s*\{[^}]*flex:1 1 auto/)
  assert.match(home, /@media \(max-width: 768px\)[\s\S]*\.workbench-main,\.workbench-side\s*\{[^}]*display:contents/)
  assert.doesNotMatch(home, /\.workbench-(?:grid|main|side)\s*\{[^}]*\b(?:min-|max-)?height:/)
})
