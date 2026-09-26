export function compileReportStatus(status: string): string {
  return ({ running: '正在分析', waiting_review: '候选待审核', applied: '当次审核已应用', rejected: '候选已驳回', conflicted: '候选版本冲突', no_material: '分析完成 · 无实质新增', failed: '编译失败', cancelled: '编译已取消' } as Record<string, string>)[status] || '未记录状态'
}
export function compileFragmentStatus(status: string): string {
  return ({ unprocessed: '尚未分析', analyzing: '正在分析', analyzed: '已分析', no_material: '已分析 · 无实质结果', failed: '本批分析未完成' } as Record<string, string>)[status] || '未记录状态'
}
export function compileTopicStatus(outcome: string, reportStatus = 'waiting_review'): string {
  if (outcome === 'reconciliation_failed') return '主题归并未完成'
  if (outcome !== 'excluded_by_archive') {
    if (reportStatus === 'applied') return '当次审核已应用'
    if (reportStatus === 'rejected') return '当次候选已驳回'
    if (reportStatus === 'conflicted') return '候选版本冲突'
    if (reportStatus === 'failed' || reportStatus === 'cancelled') return '主题结果已记录 · 本次未完成'
  }
  return ({ new_topic: '新增候选 · 尚需审核', reconciled_topic: '已归并 · 尚需审核', excluded_by_archive: '保留归档 · 未恢复' } as Record<string, string>)[outcome] || '未记录结果'
}
