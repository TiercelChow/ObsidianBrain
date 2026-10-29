import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (file: string) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8')

test('all four search families share one material and geometry, with no separate Reader/Task lens', async () => {
  for (const [file, count] of [['views/Reader.vue', 3], ['views/Tasks.vue', 2], ['views/Timeline.vue', 1], ['views/knowledge/WikiWorkspace.vue', 1]] as const) {
    const template = (await source(file)).split('<script')[0]
    const searches = template.match(/<[^>]*data-glass="search"[^>]*>/g) || []
    assert.equal(searches.length, count, file)
    for (const search of searches) {
      assert.match(search, /data-glass-rim/)
      assert.doesNotMatch(search, /v-glass-lens/, 'same surface as the reference searches, not a second optic')
    }
  }
  const controls = await source('styles/controls.css')
  assert.match(controls, /\[data-glass="search"\]\s*\{[^}]*height:\s*44px/)
  assert.match(controls, /\[data-glass="search"\]\s*\{[^}]*border-radius:\s*14px/)
})

test('main content cards share the unchanged My Tasks structural material', async () => {
  const css = await source('styles/materials.css')
  assert.match(css, /\[data-glass="card"\][^{}]*\[data-glass="structural"\][^{]*\{[^}]*--material-fill:\s*var\(--glass-structural-fill\)/)
  const block = css.match(/\[data-glass="card"\][^{}]*\[data-glass="structural"\][^{]*\{([^}]+)\}/)?.[1] || ''
  for (const property of ['--material-sheen', '--material-filter', '--material-shadow', '--material-border']) assert.ok(block.includes(property), property)
  assert.doesNotMatch(css, /--glass-dashboard-fill/)
})

test('each product page explicitly marks its principal content surfaces', async () => {
  for (const [file, classes] of [
    ['views/Home.vue', ['attention-tile glass-surface', 'home-system glass-surface']],
    ['components/home/HomePanel.vue', ['home-panel glass-surface']],
    ['views/Reader.vue', ['pane pane-center']],
    ['views/Timeline.vue', ['memo-card-body glass-surface']],
    ['views/Tasks.vue', ['task-detail-zone glass-surface']],
    ['views/CodeRepo.vue', ['repo-card']],
    ['views/knowledge/KnowledgeBases.vue', ['overview-strip knowledge-surface', 'book-wiki-card knowledge-surface']],
    ['views/knowledge/WikiWorkspace.vue', ['entry-detail knowledge-surface']],
    ['views/knowledge/KnowledgeChat.vue', ['chat-panel knowledge-surface']],
    ['views/knowledge/KnowledgeTasks.vue', ['research-task knowledge-surface']],
  ] as const) {
    const template = (await source(file)).split('<script')[0]
    for (const cls of classes) assert.match(template, new RegExp(`class="${cls}"[^>]*data-glass="card"`), `${file}: ${cls}`)
  }
  const calendar = await source('components/tasks/TaskCalendar.vue')
  assert.match(calendar, /class="task-calendar glass-panel"[^>]*:data-glass="compact \? 'flat' : 'card'"/)
  assert.match(calendar, /class="agenda"[^>]*:data-glass="compact \? 'flat' : 'subcard'"/)
})

test('configuration feed and nested cards have explicit weights without adding background observers', async () => {
  const settings = await source('views/knowledge/WikiSettings.vue')
  assert.match(settings, /:data-glass="isMobile \? 'flat' : 'card'"/)
  assert.equal((settings.match(/:data-glass="isMobile \? 'card' : 'flat'"/g) || []).length, 5)
  for (const cls of ['runtime-card', 'provider-card', 'skill-card']) assert.match(settings, new RegExp(`class="${cls}"[^>]*data-glass="subcard"`))
  const css = await source('styles/materials.css')
  assert.match(css, /\[data-glass="subcard"\]\s*\{[^}]*--material-filter:\s*none/)
  assert.match(css, /\[data-glass="card"\] \[data-glass="card"\][^{]*\{[^}]*--material-filter:\s*none/)
})

test('Wiki details, citations, reports, usage metrics and conversation surfaces share nested glass', async () => {
  const css = await source('styles/materials.css')
  const nested = css.match(/:root\[data-theme\] :is\(\s*\.structure-card,[\s\S]*?\)\s*\{([^}]+)\}/)?.[0] || ''
  for (const selector of ['.citation-card', '.review-change', '.artifact-card', '.usage-metrics > article', '.run-inspector-section', '.adaptive-metrics > div', '.research-scope', '.evidence-grid > button']) assert.ok(nested.includes(selector), selector)
  assert.match(nested, /backdrop-filter:\s*none/)
  const chat = await source('views/knowledge/KnowledgeChat.vue')
  assert.match(chat, /<KnowledgeAnswerMarkdown\s+v-else-if="message.content"\s+data-glass="subcard"/)
  assert.match(chat, /\.message\.user p\s*\{[^}]*var\(--glass-action-fill\)[^}]*color:\s*var\(--text-primary\)/)
  assert.doesNotMatch(chat, /radial-gradient/)
})
