import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { shouldSendComposerOnEnter } from '../src/utils/chatComposer.ts'

const enter = {
  key: 'Enter',
  shiftKey: false,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  isComposing: false,
  keyCode: 13,
}

test('plain Enter sends, while modified Enter stays in the composer', () => {
  assert.equal(shouldSendComposerOnEnter(enter, false, Infinity), true)
  assert.equal(shouldSendComposerOnEnter({ ...enter, shiftKey: true }, false, Infinity), false)
  assert.equal(shouldSendComposerOnEnter({ ...enter, ctrlKey: true }, false, Infinity), false)
  assert.equal(shouldSendComposerOnEnter({ ...enter, key: 'a' }, false, Infinity), false)
})

test('IME candidate confirmation never sends the message', () => {
  assert.equal(shouldSendComposerOnEnter({ ...enter, isComposing: true }, false, Infinity), false)
  assert.equal(shouldSendComposerOnEnter(enter, true, Infinity), false)
  assert.equal(shouldSendComposerOnEnter({ ...enter, keyCode: 229 }, false, Infinity), false)
  assert.equal(shouldSendComposerOnEnter(enter, false, 40), false)
  assert.equal(shouldSendComposerOnEnter(enter, false, 200), true)
})

test('knowledge chat handles composition before preventing Enter', async () => {
  const chat = await readFile(new URL('../src/views/knowledge/KnowledgeChat.vue', import.meta.url), 'utf8')
  assert.match(chat, /@compositionstart="composerIsComposing = true"/)
  assert.match(chat, /@compositionend="finishComposerComposition"/)
  assert.match(chat, /@keydown="handleComposerKeydown"/)
  assert.doesNotMatch(chat, /@keydown\.enter\.exact\.prevent/)
})
