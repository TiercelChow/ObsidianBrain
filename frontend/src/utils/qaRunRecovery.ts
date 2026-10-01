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
export function recoverInterruptedQaRun(run: QaRunRecord, baseId: string, runId: string, completedWithoutHistory = false) {
  const completed = run.status === 'completed' && completedWithoutHistory
  if (run.id !== runId || run.knowledge_base_id !== baseId || run.task_type !== 'knowledge_qa' || (!completed && run.status !== 'failed')) return null
  const question = typeof run.input.question === 'string' ? run.input.question.trim() : ''
  const conversationId = run.input.conversation_id
  if (!question || Array.from(question).length > 2000 || (conversationId != null && typeof conversationId !== 'string')) return null
  const draft = completed
    ? typeof run.output?.answer === 'string' ? run.output.answer : ''
    : typeof run.output?.partial_answer === 'string' ? run.output.partial_answer : ''
  if (completed && !draft.trim()) return null
  const stopReason = typeof run.output?.stop_reason === 'string' ? run.output.stop_reason : ''
  const recoveryMessage = typeof run.output?.qa_recovery_message === 'string' ? run.output.qa_recovery_message : ''
  return {
    runId,
    question,
    conversationId: conversationId || undefined,
    draft,
    error: completed
      ? '模型已生成完整回答，但没有写入会话历史；此处可以找回正文，后续追问暂不继承它。'
      : recoveryMessage || run.error || (stopReason ? `stop_reason=${stopReason}` : '模型运行未完成'),
    ...(completed ? { completedWithoutHistory: true } : {}),
  }
}
