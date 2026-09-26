export type KnowledgeInterruptionKind = 'cancelled' | 'credentials' | 'truncated' | 'failed'

/** Launcher availability is not invalidated by a single failed model request. */
export function knowledgeRuntimeMode(launcherAvailable: boolean): 'runtime' | 'evidence_only' {
  return launcherAvailable ? 'runtime' : 'evidence_only'
}

export function interruptedKnowledgeAnswer(displayedText: string, receivedText: string, error: unknown) {
  const detail = error instanceof Error ? error.message : String(error)
  const cancelled = error instanceof Error && error.name === 'AbortError'
  const credentials = /secure storage|凭据|keychain|User canceled the operation|permission denied|401|403/i.test(detail)
  const truncated = /max_tokens|max_turn_requests|max_output|token.{0,12}(limit|上限)|输出.{0,12}(截断|上限)/i.test(detail)
  const kind: KnowledgeInterruptionKind = cancelled ? 'cancelled' : credentials ? 'credentials' : truncated ? 'truncated' : 'failed'
  const notice = kind === 'cancelled'
    ? '已停止生成，内容可能不完整。'
    : kind === 'credentials'
      ? '本次凭据授权未完成，内容可能不完整。检查供应商凭据后可以再次尝试。'
      : kind === 'truncated'
        ? '本次输出达到运行上限，内容不完整。可以缩小问题范围后再次尝试。'
        : '本次回答未完成，已有内容已保留。下一次提问会再次尝试连接模型。'
  return { content: receivedText || displayedText, kind, notice, detail }
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
