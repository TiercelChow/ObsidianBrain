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

test('mobile Wiki chat keeps its composer above the global dock', async () => {
  const [shared, chat] = await Promise.all([
    source('src/styles/knowledge.css'),
    source('src/views/knowledge/KnowledgeChat.vue'),
  ])

  assert.match(shared, /\.app-main \.route-stage:has\(\.chat-layout\)\s*\{[^}]*height:\s*calc\(100dvh[^}]*var\(--mobile-navigation-height\)/)
  assert.match(shared, /\.knowledge-page:has\(\.chat-layout\)\s+\.knowledge-content\s*\{[^}]*flex:\s*1/)
  assert.match(chat, /\.chat-panel\s*\{[^}]*min-height:\s*0;[^}]*flex:\s*1/)
  assert.match(chat, /\.chat-composer\s*\{[^}]*flex:\s*none/)
})
