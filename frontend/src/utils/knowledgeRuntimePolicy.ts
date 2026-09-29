export type KnowledgeInterruptionKind = 'cancelled' | 'credentials' | 'truncated' | 'turn_limit' | 'failed'

/** Launcher availability is not invalidated by a single failed model request. */
export function knowledgeRuntimeMode(launcherAvailable: boolean): 'runtime' | 'evidence_only' {
  return launcherAvailable ? 'runtime' : 'evidence_only'
}

export function labelForRuntimePhase(message: string): string | null {
  if (/目录规划未完成|目录规划结果无效/.test(message)) return '正在调整检索方式'
  if (/理解追问|编译知识目录/.test(message)) return '正在理解问题与选择知识'
  if (/扩容|重试/.test(message)) return '正在调整输出预算'
  if (/结束|校验/.test(message)) return '正在整理回答'
  if (/启动|连接|会话/.test(message)) return '正在连接模型'
  if (/请求|等待|分析|思考/.test(message)) return '正在分析书内证据'
  return null
}

export function interruptedKnowledgeAnswer(displayedText: string, receivedText: string, error: unknown) {
  const detail = error instanceof Error ? error.message : String(error)
  const cancelled = error instanceof Error && error.name === 'AbortError'
  const credentials = /secure storage|凭据|keychain|User canceled the operation|permission denied|401|403/i.test(detail)
  const turnLimit = /max_turn_requests|qa_turn_limit/i.test(detail)
  const truncated = /max_tokens|max_turn_requests|max_output|token.{0,12}(limit|上限)|输出.{0,12}(截断|上限)/i.test(detail)
  const kind: KnowledgeInterruptionKind = cancelled ? 'cancelled' : credentials ? 'credentials' : turnLimit ? 'turn_limit' : truncated ? 'truncated' : 'failed'
  const hardOutputLimit = detail.includes('qa_output_hard_limit')
  const canRetry = kind === 'truncated' && !hardOutputLimit
  const content = receivedText || displayedText
  const notice = kind === 'cancelled'
    ? '已停止生成，内容可能不完整。'
    : kind === 'credentials'
      ? '本次凭据授权未完成，内容可能不完整。检查供应商凭据后可以再次尝试。'
      : kind === 'turn_limit'
        ? 'Harness 请求轮次已用尽，内容不完整。增加输出 token 无法解决；复杂问题建议转为研究任务，按子问题独立分析后综合。'
      : kind === 'truncated'
        ? hardOutputLimit
          ? '模型单次输出已达到硬上限，未完成内容已保留。请先调整模型最大输出或推理策略，或缩小问题范围。'
          : content
          ? '本次输出达到运行上限，内容不完整。可继续完成完整答案，或缩小问题范围后重新提问。'
          : '本次输出达到运行上限且没有收到正文。请核对模型最大输出与推理策略，或缩小问题范围后重新提问。'
        : '本次回答未完成，已有内容已保留。下一次提问会再次尝试连接模型。'
  return { content, kind, notice, detail, canRetry }
}

export function researchTaskRouteFromQuestion(baseId: string, question: string) {
  const characters = Array.from(question.trim())
  return {
    path: '/knowledge/tasks',
    query: {
      create: '1',
      base: baseId,
      title: characters.slice(0, 200).join(''),
      description: characters.length > 200 ? characters.slice(0, 4000).join('') : '',
    },
  }
}

/** Only reuse a planner rewrite belonging to this exact book and user turn. */
export function researchTaskQuestionFromRun(
  originalQuestion: string,
  baseId: string,
  run: { knowledge_base_id?: string | null; task_type: string; input: Record<string, unknown> } | null,
) {
  const original = originalQuestion.trim()
  const input = run?.input
  const standalone = typeof input?.standalone_question === 'string' ? input.standalone_question.trim() : ''
  if (run?.knowledge_base_id === baseId
    && run.task_type === 'knowledge_qa'
    && input?.question === original
    && standalone
    && Array.from(standalone).length <= 2000) {
    return { question: standalone, contextRecovered: true }
  }
  return { question: original, contextRecovered: false }
}

/** The task summary can append an entire report; inspect only its failure header. */
export function researchTaskTurnLimitFailure(task: { status: string; result_summary: string } | null): boolean {
  if (task?.status !== 'failed') return false
  const failureHeader = task.result_summary.split(/\n\s*\n/, 1)[0]
  return /\(research_turn_limit\)|stop_reason=max_turn_requests/.test(failureHeader)
}

export const modelReasoningPolicies = ['auto', 'off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'] as const

export function validateModelCapabilities(capabilities: {
  context_window?: number | null
  max_output_tokens?: number | null
  reasoning_policy?: string
}): string {
  for (const [name, value] of [['上下文容量', capabilities.context_window], ['最大输出', capabilities.max_output_tokens]] as const) {
    if (value != null && (!Number.isInteger(value) || value < 1 || value > 4294967295)) return `${name}必须是 1–4294967295 的整数；未知时请留空。`
  }
  if (capabilities.context_window != null && capabilities.max_output_tokens != null && capabilities.max_output_tokens >= capabilities.context_window) {
    return '最大输出必须小于上下文容量，为输入材料保留空间。'
  }
  if (capabilities.reasoning_policy && !modelReasoningPolicies.some(policy => policy === capabilities.reasoning_policy)) return '请选择有效的推理策略。'
  return ''
}
