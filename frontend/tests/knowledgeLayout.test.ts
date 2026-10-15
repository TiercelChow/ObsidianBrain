import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)
const source = (path: string) => readFile(new URL(path, root), 'utf8')

test('knowledge pages keep horizontal chrome inside their own width', async () => {
  const [shared, bases, settings] = await Promise.all([
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/views/knowledge/WikiSettings.vue'),
  ])

  assert.match(shared, /\.knowledge-page\s*\{[^}]*min-width:\s*0/)
  assert.match(shared, /\.knowledge-tabs\s*\{[^}]*width:\s*100%/)
  assert.match(bases, /\.book-actions\s*\{[^}]*flex-wrap:\s*wrap/)
  assert.match(bases, /aria-label="刷新书籍知识库"/)
  assert.match(settings, /\.settings-main\s*\{[^}]*container-type:\s*inline-size/)
  assert.match(settings, /@container\s*\(max-width:\s*740px\)/)
})

test('long Wiki dialogs fit their panel and scroll content before actions', async () => {
  const [shared, tasks, settings, workspace] = await Promise.all([
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/views/knowledge/WikiWorkspace.vue'),
  ])

  assert.match(shared, /\.knowledge-modal-card\s*\{[^}]*max-height:\s*calc\(100dvh - 48px\)/)
  assert.match(tasks, /<MotionModal v-model="resultVisible"[^>]*size="wide"/)
  assert.match(tasks, /class="task-result-scroll"/)
  assert.match(tasks, /\.task-result-scroll\s*\{[^}]*overflow-y:\s*auto/)
  assert.match(tasks, /\.task-result-scroll\s*\{[^}]*display:\s*flex;\s*flex-direction:\s*column/)
  assert.match(tasks, /\.task-result-scroll\s*>\s*\*\s*\{[^}]*flex:\s*0 0 auto/)
  assert.match(tasks, /\.task-result-scroll\s*\{[^}]*padding:\s*0 24px 24px/)
  assert.match(settings, /\.skill-detail-modal\s*\{[^}]*width:\s*100%/)
  assert.match(settings, /\.skill-detail-layout\s*\{[^}]*min-height:\s*0/)
  assert.match(workspace, /\.review-content\s*\{[^}]*overflow:\s*auto/)
})

test('narrow Wiki navigation keeps all actions reachable and long content contained', async () => {
  const [workspace, chat, tasks] = await Promise.all([
    source('src/views/knowledge/WikiWorkspace.vue'),
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/views/knowledge/KnowledgeTasks.vue'),
  ])

  assert.match(workspace, /aria-label="Wiki 操作"/)
  assert.match(workspace, /v-model="mobileActionsVisible"/)
  assert.match(workspace, /@media \(max-width: 1100px\)/)
  assert.match(chat, /@media \(min-width: 769px\) and \(max-width: 1150px\)/)
  assert.match(tasks, /\.task-main footer\s*\{[^}]*flex-wrap:\s*wrap/)
  assert.match(tasks, /\.task-topline\s*\{[^}]*flex-wrap:\s*wrap/)
})

test('Wiki pages share the global page width and title rhythm', async () => {
  const [shell, shared, bases, settings] = await Promise.all([
    source('src/components/knowledge/KnowledgePageShell.vue'),
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/views/knowledge/WikiSettings.vue'),
  ])

  assert.doesNotMatch(shell, /knowledge-eyebrow/)
  assert.doesNotMatch(shared, /max-width:\s*1440px/)
  assert.doesNotMatch(shared, /\.knowledge-heading\s+\.page-title/)
  assert.match(bases, /\.book-wiki-grid\s*\{[^}]*repeat\(auto-fit/)
  assert.doesNotMatch(settings, /\.runtime-card\s*\{[^}]*max-width:\s*720px/)
  assert.doesNotMatch(settings, /\.settings-layout\s*\{[^}]*min-height:\s*585px/)
})

test('desktop Wiki headers reserve the same action height even without actions or loaded data', async () => {
  const shared = await source('src/styles/knowledge.css')
  const desktop = shared.split('@media (min-width: 769px) {')[1]?.split('}\n}')[0] || ''
  assert.match(desktop, /\.knowledge-heading\s*\{[^}]*min-height:\s*44px/)
  assert.match(shared, /\.knowledge-tabs\s*\{[^}]*flex-shrink:\s*0/)
  assert.match(shared, /\.knowledge-heading\s*\{[^}]*display:\s*none\s*!important/)
})

test('desktop Wiki reading surfaces fill the available height without clipping their own scroll areas', async () => {
  const [shared, workspace, chat] = await Promise.all([
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/WikiWorkspace.vue'),
    source('src/views/knowledge/KnowledgeChat.vue'),
  ])

  assert.match(shared, /\.knowledge-page:has\(\.wiki-workspace\)/)
  assert.match(shared, /\.knowledge-page:has\(\.chat-layout\)/)
  assert.match(workspace, /\.entry-list\s*\{[^}]*max-height:\s*none/)
  assert.match(workspace, /\.entry-detail\s*\{[^}]*max-height:\s*none/)
  assert.match(chat, /\.message-list\s*\{[^}]*min-height:\s*0;[^}]*max-height:\s*none/)
})

test('short desktop windows bound chat tracks so history cannot push the composer below the viewport', async () => {
  const chat = await source('src/views/knowledge/KnowledgeChat.vue')
  const desktop = chat.split('@media (min-width: 1151px) {')[1]?.split('}\n}')[0] || ''
  assert.match(desktop, /\.chat-layout\s*\{[^}]*grid-template-rows:\s*minmax\(0,\s*1fr\)/)
  assert.match(desktop, /\.chat-context\s*\{[^}]*min-height:\s*0;[^}]*max-height:\s*100%;[^}]*overflow:\s*auto/)
})

test('mobile Wiki chat keeps its composer above the global dock', async () => {
  const [shared, chat] = await Promise.all([
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/KnowledgeChat.vue'),
  ])

  assert.match(shared, /\.app-main \.route-stage:has\(\.chat-layout\)\s*\{[^}]*height:\s*calc\(var\(--visual-viewport-height, 100dvh\)[^}]*var\(--chat-dock-height\)/)
  assert.match(shared, /\.knowledge-page:has\(\.chat-layout\)\s+\.knowledge-content\s*\{[^}]*flex:\s*1/)
  assert.match(chat, /\.chat-panel\s*\{[^}]*min-height:\s*0;[^}]*flex:\s*1/)
  assert.match(chat, /\.chat-composer\s*\{[^}]*flex:\s*none/)
})

test('only mobile chat follows the actual dock arc while outer lists retain stable reserves', async () => {
  const [app, shared, chat, motion] = await Promise.all([
    source('src/App.vue'), source('src/styles/knowledge.css'), source('src/views/knowledge/KnowledgeChat.vue'), source('src/composables/useMobileDockMotion.ts'),
  ])
  assert.match(app, /:style="mobileChatDockStyle"/)
  assert.match(app, /route\.path !== '\/knowledge\/chat'/)
  assert.match(app, /'--chat-dock-sink': geometry\.sink/)
  assert.match(app, /'--chat-dock-travel-y': geometry\.travelY/)
  assert.match(app, /'--chat-dock-visible': dockKeyboardOpen\.value \? 0 : 1/)
  assert.match(app, /\.has-mobile-subnav \{ --mobile-navigation-height: calc\(var\(--mobile-dock-height\) \+ var\(--mobile-sub-dock-height\) \+ 8px\)/)
  const container = shared.match(/\.app-shell \.app-main\.mobile-full:has\(\.chat-layout\)\s*\{([^}]+)\}/)?.[1] ?? ''
  assert.match(container, /padding-bottom: calc\(var\(--chat-dock-height\)/)
  assert.match(container, /overflow-y: hidden/)
  assert.match(container, /var\(--chat-dock-sink, 0\)/)
  assert.match(container, /var\(--chat-dock-travel-y, 0\)/)
  assert.doesNotMatch(container, /transition:/)
  assert.match(chat, /new ResizeObserver/)
  assert.match(chat, /messageEndPinned/)
  assert.match(chat, /messageResizeObserver\?\.disconnect\(\)/)
  assert.match(chat, /overflow-anchor: none/)
  assert.match(motion, /resizeAwareScrollDelta\(/)
})
