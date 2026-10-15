import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (name: string) => readFile(new URL(`../src/views/knowledge/${name}.vue`, import.meta.url), 'utf8')

test('desktop workspace carries its bounded height through both animation wrappers', async () => {
  const workspace = await source('WikiWorkspace')
  assert.match(workspace, /<MotionSwap\s+v-else[^>]*class="wiki-workspace-stage"[^>]*:contained="!singleColumn"/)
  const desktop = workspace.split('@media (min-width: 1101px) {')[1]?.split('}\n}')[0] || ''
  assert.match(desktop, /\.wiki-workspace-stage\s*\{[^}]*min-height:\s*0;[^}]*display:\s*flex;[^}]*flex-direction:\s*column/)
  assert.match(desktop, /\.wiki-workspace-stage\s*>\s*:deep\(\.motion-swap__panel\)\s*\{[^}]*min-height:\s*0;[^}]*flex:\s*1;[^}]*display:\s*flex;[^}]*flex-direction:\s*column/)
  assert.match(workspace, /\.entry-pane-head\s*\{[^}]*flex:\s*0 0 auto/)
})

test('desktop pane scroll boundaries do not chain to the page while mobile keeps natural flow', async () => {
  const workspace = await source('WikiWorkspace')
  const desktop = workspace.split('@media (min-width: 1101px) {')[1]?.split('}\n}')[0] || ''
  assert.match(desktop, /\.entry-list\s*\{[^}]*min-height:\s*0;[^}]*max-height:\s*none;[^}]*overscroll-behavior-y:\s*contain/)
  assert.match(desktop, /\.entry-detail\s*\{[^}]*max-height:\s*none;[^}]*overscroll-behavior-y:\s*contain/)
  const mobile = workspace.split('@media (max-width: 1100px) {')[1] || ''
  assert.doesNotMatch(mobile, /overscroll-behavior[^;]*contain/)
  assert.match(workspace, /class="mobile-back"[\s\S]*@click="returnToEntryList"/)
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
  assert.equal(actions.length, 4, 'three normal actions plus the recover-history replacement')
  for (const action of actions) assert.match(action, /appearance="quiet"/)
  assert.match(footer, /@click="inspectRun\(message.runId\)"/)
  assert.match(footer, /:loading="savingRunId === message.runId" @click="saveAnswer\(message\)"/)
  assert.match(footer, /:disabled="searching \|\| \(!!researchTransferMessageId[^\n]*@click="openResearchTask\(message\)"/)
})
