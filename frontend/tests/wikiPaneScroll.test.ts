import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (name: string) => readFile(new URL(`../src/views/knowledge/${name}.vue`, import.meta.url), 'utf8')

test('desktop Wiki directory and reading panel each own scrolling', async () => {
  const wiki = await source('WikiTopics')
  const desktop = wiki.split('@media (min-width: 1101px) {')[1]?.split('@media')[0] || ''
  assert.match(desktop, /\.topic-layout\s*\{[^}]*grid-template-rows:\s*minmax\(0, 1fr\)/)
  assert.match(desktop, /\.topic-directory, \.topic-content\s*\{[^}]*min-height:\s*0;[^}]*overflow:\s*auto;[^}]*overscroll-behavior-y:\s*contain/)
  const mobile = wiki.split('@media (max-width: 768px) {')[1] || ''
  assert.match(mobile, /\.topic-layout\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/)
  assert.doesNotMatch(mobile, /\.topic-content\s*\{[^}]*overflow/)
})

test('new conversations retain the plus icon in shared quiet actions without local geometry overrides', async () => {
  const chat = await source('KnowledgeChat')
  const actions = chat.match(/<UiAction\s+:icon="Plus"\s+label="新对话"[^>]*\/>/g) || []
  assert.equal(actions.length, 2, 'desktop and mobile should share the new-conversation control')
  for (const action of actions) {
    assert.match(action, /appearance="quiet"/)
    assert.match(action, /:disabled="!activeBaseId \|\| searching"/)
  }
  assert.match(actions[0]!, /@click="newConversation\(\)"/)
  assert.match(actions[1]!, /@click="contextVisible = false; newConversation\(\)"/)
  assert.doesNotMatch(chat, /\.conversation-history header > button|\.new-conversation-button|<Plus\s*\//)
  const sheetHead = chat.split('class="knowledge-modal-card chat-context-sheet"')[1]?.split('class="knowledge-modal-body"')[0] || ''
  assert.match(sheetHead, /label="新对话"/)
})

test('all answer actions use quiet shared buttons while preserving their behavior and gating', async () => {
  const chat = await source('KnowledgeChat')
  const footer = chat.split('class="answer-actions"')[1]?.split('</div>')[0] || ''
  const actions = footer.match(/<UiAction[^>]*\/>/g) || []
  assert.equal(actions.length, 5, 'four normal actions plus the recover-history replacement')
  for (const action of actions) assert.match(action, /appearance="quiet"/)
  assert.match(footer, /@click="inspectRun\(message.runId\)"/)
  assert.match(footer, /v-if="!message.interruption && !message.unsavedAnswer && message.evidence\?\.length"[^>]*label="整理回答为主题草稿"[^>]*@click="openAnswerDraft\(message\)"/)
  assert.match(footer, /label="留作主题待验证问题"[^>]*@click="saveAnswer\(message\)"/)
  assert.match(footer, /:disabled="searching \|\| \(!!researchTransferMessageId[^\n]*@click="openResearchTask\(message\)"/)
})

test('completed reports and answers enter the same book-scoped topic draft selection', async () => {
  const [chat, tasks, wiki] = await Promise.all([source('KnowledgeChat'), source('KnowledgeTasks'), source('WikiTopics')])
  assert.match(chat, /query: \{ base: activeBaseId.value, writebackRun: message.runId \}/)
  assert.match(tasks, /v-if="reportVerified && reportRunId && activeEvidence\.length"[\s\S]*?@click="openReportDraft\(activeTask\)"/)
  assert.match(tasks, /query: \{ base: task.knowledge_base_id, writebackRun: reportRunId.value \}/)
  assert.match(wiki, /chosenPassages.value.map\(passage => passage.index\)/)
  assert.match(wiki, /draftTopicFromMaterial\(book, id, runId, revision, passages\), true/)
})
