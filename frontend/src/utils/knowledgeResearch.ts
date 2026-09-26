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
