import assert from 'node:assert/strict'
import test from 'node:test'

import { parseKnowledgeCitations } from '../src/utils/knowledgeCitations.ts'

test('parseKnowledgeCitations separates valid source markers from answer text', () => {
  assert.deepEqual(parseKnowledgeCitations('结论一。[S1] 结论二 [S12]。', 12), [
    { text: '结论一。' },
    { text: '[S1]', sourceIndex: 0 },
    { text: ' 结论二 ' },
    { text: '[S12]', sourceIndex: 11 },
    { text: '。' },
  ])
})

test('parseKnowledgeCitations keeps unavailable and malformed markers as plain text', () => {
  assert.deepEqual(parseKnowledgeCitations('证据 [S0] [S3] [Sx]', 2), [
    { text: '证据 [S0] [S3] [Sx]' },
  ])
})
