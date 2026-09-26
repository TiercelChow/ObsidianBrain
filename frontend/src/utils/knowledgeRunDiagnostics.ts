interface DiagnosticEvent { event_type: string; payload: Record<string, unknown> }

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

/** ACP cost is cumulative currency, and UsageContext is occupancy: neither is token billing. */
export function knowledgeRunDiagnostics(events: DiagnosticEvent[], budgetValue: unknown) {
  const values = budgetValue && typeof budgetValue === 'object' ? budgetValue as Record<string, unknown> : {}
  let context: { used: number; size: number } | null = null
  let cost: { amount: number; currency: string } | null = null
  let completion: { stopReason: string; complete: boolean } | null = null
  for (const event of events) {
    const payload = event.payload
    if (event.event_type === 'run.usage') {
      const used = finiteNumber(payload.context_used)
      const size = finiteNumber(payload.context_size)
      if (used != null && size != null) context = { used, size }
    } else if (event.event_type === 'run.usage_cost') {
      const amount = finiteNumber(payload.amount)
      if (amount != null && typeof payload.currency === 'string') cost = { amount, currency: payload.currency }
    } else if (event.event_type === 'run.runtime_completed' && typeof payload.complete === 'boolean') {
      completion = { stopReason: String(payload.stop_reason || 'unknown'), complete: payload.complete }
    }
  }
  return {
    context, cost, completion,
    budget: {
      context_window: finiteNumber(values.context_window),
      max_output_tokens: finiteNumber(values.max_output_tokens),
      effective_max_output_tokens: finiteNumber(values.effective_max_output_tokens),
      timeout_seconds: finiteNumber(values.timeout_seconds),
      tool_call_limit: finiteNumber(values.tool_call_limit),
      reasoning_policy: typeof values.reasoning_policy === 'string' ? values.reasoning_policy : 'auto',
    },
  }
}
