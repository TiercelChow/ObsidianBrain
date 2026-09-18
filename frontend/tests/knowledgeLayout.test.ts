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
