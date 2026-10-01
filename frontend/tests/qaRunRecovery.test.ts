import assert from 'node:assert/strict'
import test from 'node:test'

import { recoverInterruptedQaRun } from '../src/utils/qaRunRecovery.ts'

const failedRun = {
  id: 'run-1',
  knowledge_base_id: 'book-a',
  task_type: 'knowledge_qa',
  status: 'failed',
  input: { question: '机制是什么？', conversation_id: 'conversation-a' },
  output: { partial_answer: '未完成草稿 [S1]', stop_reason: 'max_tokens' },
  error: 'stop_reason=max_tokens',
}

test('failed QA run can be restored as an unfinished draft with its exact recovery scope', () => {
  assert.deepEqual(recoverInterruptedQaRun(failedRun, 'book-a', 'run-1'), {
    runId: 'run-1',
    question: '机制是什么？',
    conversationId: 'conversation-a',
    draft: '未完成草稿 [S1]',
    error: 'stop_reason=max_tokens',
  })
})

test('restoration refuses cross-book, completed and mismatched run records', () => {
  assert.equal(recoverInterruptedQaRun(failedRun, 'book-b', 'run-1'), null)
  assert.equal(recoverInterruptedQaRun(failedRun, 'book-a', 'another-run'), null)
  assert.equal(recoverInterruptedQaRun({ ...failedRun, status: 'completed' }, 'book-a', 'run-1'), null)
  assert.equal(recoverInterruptedQaRun({ ...failedRun, task_type: 'knowledge_qa_select' }, 'book-a', 'run-1'), null)
  assert.equal(recoverInterruptedQaRun({ ...failedRun, input: { question: '' } }, 'book-a', 'run-1'), null)
})

test('a failed max_tokens run without answer text keeps its original scope without inventing a draft', () => {
  const recovered = recoverInterruptedQaRun({
    ...failedRun,
    input: { question: '机制是什么？', conversation_id: null },
    output: { stop_reason: 'max_tokens' },
    error: null,
  }, 'book-a', 'run-1')
  assert.equal(recovered?.draft, '')
  assert.equal(recovered?.conversationId, undefined)
  assert.equal(recovered?.error, 'stop_reason=max_tokens')
})

test('terminal QA recovery advice survives inspection without replacing the raw run failure', () => {
  const recovered = recoverInterruptedQaRun({
    ...failedRun,
    output: {
      ...failedRun.output,
      qa_recovery_disposition: 'qa_output_retry_exhausted',
      qa_recovery_message: '(qa_output_retry_exhausted) 两次扩容仍未完成',
    },
  }, 'book-a', 'run-1')
  assert.equal(recovered?.error, '(qa_output_retry_exhausted) 两次扩容仍未完成')
  assert.equal(failedRun.error, 'stop_reason=max_tokens')
})
