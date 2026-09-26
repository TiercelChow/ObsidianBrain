export function needsSourceReview(entry: { status: string; source_impact_count?: number }) {
  return entry.status === 'stale' || (entry.source_impact_count ?? 0) > 0
}

export function knowledgeStatusLabel(status: string) {
  return ({ verified: '可追溯', draft: '草稿', stale: '来源需复核', archived: '已归档' } as Record<string, string>)[status] || status
}

export function sourceImpactLabel(reason: string) {
  return ({ source_changed: '原文已更新', source_missing: '原文缺失或不可读', source_reindexed: '来源定位已更新' } as Record<string, string>)[reason] || '来源需要复核'
}
