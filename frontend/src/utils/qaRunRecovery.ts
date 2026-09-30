interface QaRunRecord {
  id: string
  knowledge_base_id?: string | null
  task_type: string
  status: string
  input: Record<string, unknown>
  output?: Record<string, unknown> | null
  error?: string | null
}

/** An inspection is data, not authority to resume a different book or turn. */
export function recoverInterruptedQaRun(run: QaRunRecord, baseId: string, runId: string) {
  if (run.id !== runId || run.knowledge_base_id !== baseId || run.task_type !== 'knowledge_qa' || run.status !== 'failed') return null
  const question = typeof run.input.question === 'string' ? run.input.question.trim() : ''
  const conversationId = run.input.conversation_id
  if (!question || Array.from(question).length > 2000 || (conversationId != null && typeof conversationId !== 'string')) return null
  const draft = typeof run.output?.partial_answer === 'string' ? run.output.partial_answer : ''
  const stopReason = typeof run.output?.stop_reason === 'string' ? run.output.stop_reason : ''
  return {
    runId,
    question,
    conversationId: conversationId || undefined,
    draft,
    error: run.error || (stopReason ? `stop_reason=${stopReason}` : '模型运行未完成'),
  }
}
