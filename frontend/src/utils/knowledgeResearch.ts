export function researchStageStatus(status: string): string {
  return ({ pending: '尚未开始', running: '正在执行', completed: '成果已保存', failed: '本阶段未完成', cancelled: '已取消', stale: '依据变化 · 待复核' } as Record<string, string>)[status] || '未记录状态'
}

export function researchFindingStatus(status: string): string {
  return ({ supported: '模型自报支持', partial: '部分支持', missing: '证据缺失', conflict: '存在分歧' } as Record<string, string>)[status] || '未记录判定'
}

export function researchCoverage(stages: { kind: string; status: string }[]) {
  const chapters = stages.filter(stage => stage.kind === 'section')
  const saved = chapters.filter(stage => stage.status === 'completed').length
  return { saved, planned: chapters.length, unfinished: chapters.length - saved }
}

interface ResearchStageSnapshot {
  stage_key: string
  status: string
  revision: number
  run_id?: string | null
  content_characters: number
}

export function pickResearchStageToOpen<T extends ResearchStageSnapshot>(stages: T[]): T | undefined {
  const active = stages.find(stage => stage.status === 'running')
  if (active) return active
  const attention = stages.find(stage => stage.status === 'failed' || stage.status === 'stale')
  if (attention) return attention
  const report = stages.find(stage => stage.stage_key === 'report' && stage.status === 'completed' && stage.content_characters > 0)
  if (report) return report
  for (let index = stages.length - 1; index >= 0; index--) {
    const stage = stages[index]
    if (stage.status === 'completed' && stage.content_characters > 0) return stage
  }
  return stages[0]
}

export function shouldRefreshResearchStage(
  selected: ResearchStageSnapshot,
  next: ResearchStageSnapshot[],
  followLatest: boolean,
): boolean {
  if (!followLatest) return false
  const after = next.find(stage => stage.stage_key === selected.stage_key)
  if (!after) return false
  return selected.revision !== after.revision
    || selected.status !== after.status
    || selected.run_id !== after.run_id
    || selected.content_characters !== after.content_characters
}
