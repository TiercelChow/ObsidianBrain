type ObjectValue = Record<string, unknown>
interface BudgetEvent { event_type: string; payload: ObjectValue; message?: string }
function object(value: unknown): ObjectValue { return value && typeof value === 'object' && !Array.isArray(value) ? value as ObjectValue : {} }
function number(value: unknown): number | null { return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null }
function text(value: unknown): string { return typeof value === 'string' ? value : '' }
function strings(value: unknown): string[] { return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [] }

/** Business diagnostics only. No thought payloads and no invented billing or semantic proof. */
export function adaptiveKnowledgeDiagnostics(refsValue: unknown, events: BudgetEvent[]) {
  const refs = object(refsValue)
  const plan = object(refs.qa_plan)
  const stats = object(refs.planning_stats)
  let state: ObjectValue = {}
  const reports = new Map<number, ObjectValue>()
  const expansions: string[] = []
  const limits: string[] = []
  for (const event of events) {
    const payload = object(event.payload)
    if (event.event_type === 'run.budget_limited') limits.push(text(payload.reason) || text(event.message))
    if (event.event_type === 'run.budget_changed') {
      state = Object.keys(object(payload.budget)).length ? object(payload.budget) : payload
      if (text(payload.reason)) expansions.push(text(payload.reason))
    }
    if (event.event_type === 'run.evidence_coverage') {
      const coverage = object(payload.coverage)
      const index = number(coverage.question_index)
      if (index != null) reports.set(index, coverage)
    }
  }
  if (Object.keys(object(refs.adaptive_state)).length) state = object(refs.adaptive_state)
  if (Array.isArray(state.coverage)) {
    for (const raw of state.coverage) {
      const item = object(raw)
      const index = number(item.question_index)
      if (index != null) reports.set(index, item)
    }
  }
  const policy = object(state.policy)
  return {
    available: Object.keys(plan).length > 0 || Object.keys(state).length > 0,
    plan: { goal: text(plan.goal), constraints: strings(plan.constraints), subquestions: strings(plan.subquestions), depth: text(plan.depth), scope: text(plan.scope) },
    planning: { seen: number(stats.catalog_seen), total: number(stats.catalog_total), tokens: number(stats.estimated_payload_tokens), tokenLimit: number(stats.token_limit), elapsedMs: number(stats.elapsed_ms), secondsLimit: number(stats.time_limit_seconds), stopReason: text(stats.stop_reason) },
    budget: { used: number(state.used_tool_calls), soft: number(state.soft_tool_calls), hard: number(policy.hard_tool_calls), tokens: number(state.estimated_tool_payload_tokens), softTokens: number(state.soft_retrieval_tokens), hardTokens: number(policy.hard_retrieval_tokens), extensions: number(state.extension_count), deadline: text(state.deadline) },
    coverage: [...reports.entries()].sort(([a], [b]) => a - b).map(([index, item]) => ({ index, question: text(item.question), status: text(item.status), finding: text(item.finding), citationIndices: Array.isArray(item.citation_indices) ? item.citation_indices.filter((v): v is number => typeof v === 'number' && Number.isInteger(v) && v > 0) : [] })),
    expansions, limits,
  }
}
