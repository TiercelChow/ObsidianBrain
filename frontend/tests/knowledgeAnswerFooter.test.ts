import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/KnowledgeChat.vue', import.meta.url), 'utf8')

test('answer actions and the source disclosure share one completed-answer footer row', async () => {
  const chat = await source()
  const footer = chat.split('class="answer-footer"')[1]?.split('</footer>')[0] || ''
  const bar = footer.split('class="answer-footer-bar"')[1]?.split('<Transition')[0] || ''
  assert.match(bar, /class="evidence-toggle"/)
  assert.match(bar, /class="answer-actions"/)
  assert.match(chat, /<footer v-if="message.role === 'assistant' && message.id !== streamingMessageId && \(message.runId \|\| message.evidence\?\.length\)" class="answer-footer"/)
  assert.match(chat, /\.answer-footer-bar\s*\{[^}]*display:\s*flex;[^}]*align-items:\s*center/)
  assert.match(chat, /\.answer-actions\s*\{[^}]*margin-left:\s*auto/)
})

test('sources start collapsed per answer and retain an accessible controlled region', async () => {
  const chat = await source()
  assert.match(chat, /evidenceExpanded\?: boolean/)
  assert.doesNotMatch(chat, /evidenceExpanded:\s*true/)
  assert.match(chat, /:aria-expanded="Boolean\(message.evidenceExpanded\)"/)
  assert.match(chat, /:aria-controls="`answer-sources-\$\{message.id\}`"/)
  assert.match(chat, /@click="message.evidenceExpanded = !message.evidenceExpanded"/)
  assert.match(chat, /v-show="Boolean\(message.evidenceExpanded\)"[^>]*:id="`answer-sources-\$\{message.id\}`"[^>]*:inert="!message.evidenceExpanded"/)
  assert.match(chat, /@click="previewEvidence\(message, evidenceIndex\)"/)
})

test('source expansion animates natural content height and has reduced-motion support', async () => {
  const chat = await source()
  assert.match(chat, /<Transition name="evidence-expand">/)
  assert.match(chat, /\.evidence-section\s*\{[^}]*grid-template-rows:\s*1fr/)
  assert.match(chat, /\.evidence-reveal-body\s*\{[^}]*min-height:\s*0;[^}]*overflow:\s*hidden/)
  assert.match(chat, /\.evidence-expand-enter-from,\s*\.evidence-expand-leave-to\s*\{[^}]*grid-template-rows:\s*0fr/)
  assert.match(chat.split('@media (prefers-reduced-motion: reduce) {')[1] || '', /\.evidence-expand-enter-active,\s*\.evidence-expand-leave-active\s*\{[^}]*transition:\s*none/)
})
