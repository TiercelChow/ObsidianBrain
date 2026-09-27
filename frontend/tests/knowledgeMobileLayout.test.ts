import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('phone Wiki navigation fits all five destinations without hidden tabs', async () => {
  const css = await source('styles/knowledge.css')
  assert.match(css, /\.knowledge-tabs\s*\{[^}]*grid-template-columns:\s*repeat\(5,\s*minmax\(0,\s*1fr\)\)/)
  assert.match(css, /\.knowledge-page[^}]*\.el-input__inner[^}]*font-size:\s*16px/s)
})

test('phone chat moves history into a sheet instead of consuming answer height', async () => {
  const chat = await source('views/knowledge/KnowledgeChat.vue')
  assert.match(chat, /aria-label="问答历史与设置"/)
  assert.match(chat, /<MotionModal v-model="contextVisible"/)
  assert.match(chat, /\.chat-context\s*\{[^}]*display:\s*none/)
  assert.match(chat, /\.chat-mobile-context\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\) 44px/)
})

test('phone book and research cards give the body full width', async () => {
  const [books, tasks] = await Promise.all([source('views/knowledge/KnowledgeBases.vue'), source('views/knowledge/KnowledgeTasks.vue')])
  assert.match(books, /\.book-card-main\s*\{[^}]*display:\s*contents/)
  assert.match(books, /aria-label="知识库操作"/)
  assert.match(tasks, /class="task-card-actions"/)
  assert.match(tasks, /\.task-main\s*\{[^}]*grid-column:\s*1 \/ -1/)
})

test('phone settings and entity reading avoid redundant persistent chrome', async () => {
  const [settings, workspace, research] = await Promise.all([
    source('views/knowledge/WikiSettings.vue'), source('views/knowledge/WikiWorkspace.vue'),
    source('components/knowledge/KnowledgeResearchWorkspace.vue'),
  ])
  assert.match(settings, /aria-label="配置分区"/)
  assert.match(settings, /\.settings-main\s*\{[^}]*padding:\s*14px/)
  assert.match(workspace, /'is-reading': Boolean\(selectedEntry\)/)
  assert.match(workspace, /\.wiki-toolbar\.is-reading \.knowledge-search\s*\{[^}]*display:\s*none/)
  assert.match(research, /class="research-stage-directory"/)
  assert.match(research, /stageDirectoryOpen\.value = false/)
  assert.match(settings, /class="mobile-skill-metadata-toggle"/)
  assert.match(settings, /\.skill-detail-content > pre\s*\{[^}]*max-height:\s*none/)
})

test('phone Wiki drawers respect both the panel and keyboard viewport', async () => {
  const shared = await source('styles/knowledge.css')
  assert.match(shared, /max-height:\s*min\(88dvh, 760px, calc\(var\(--motion-viewport-height/)
  assert.match(shared, /\.knowledge-modal-body\s*\{[^}]*flex:\s*1 1 auto/)
  assert.match(shared, /\.wiki-usage-picker \.el-date-range-picker__content\s*\{[^}]*width:\s*100%/)
})
