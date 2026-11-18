import type { KnowledgeBaseSummary } from '../api/knowledge'

type ResearchReadinessBase = Pick<KnowledgeBaseSummary,
  'lifecycle' | 'source_available' | 'sync_state' | 'source_count' | 'entry_count'>

export interface ResearchReadiness {
  status: 'ready' | 'attention' | 'blocked'
  canCreate: boolean
  summary: string
  notes: string[]
}

export function researchReadiness(base: ResearchReadinessBase): ResearchReadiness {
  if (base.lifecycle !== 'active') {
    return {
      status: 'blocked',
      canCreate: false,
      summary: '知识库当前不可用于新研究。',
      notes: [base.lifecycle === 'paused'
        ? '知识库已暂停；请先到知识库页面恢复，再分析诉求或创建任务。'
        : base.lifecycle === 'archived'
          ? '知识库已归档；请先到知识库页面恢复，再分析诉求或创建任务。'
          : '知识库尚未启用；请先到知识库页面启用，再分析诉求或创建任务。'],
    }
  }

  const notes: string[] = []
  if (!base.source_available) {
    notes.push('原书目录不可用；历史知识仍可检索，但本轮可能无法核验原文。')
  }

  if (base.sync_state === 'failed') {
    notes.push('来源同步失败；现有数据可能不是最新版本。')
  } else if (base.sync_state === 'outdated') {
    notes.push('来源待同步；研究可能遗漏书内最近的改动。')
  } else if (base.sync_state !== 'clean') {
    notes.push('来源正在同步；研究将基于当前已保存的资料。')
  }

  if (base.source_count === 0) {
    notes.push('当前没有已同步的 Markdown 来源；可先同步书籍。')
  }
  if (base.entry_count === 0) {
    notes.push('当前没有可检索条目；若原文可读，研究仍可尝试原文搜索。')
  }

  if (notes.length > 0) {
    return {
      status: 'attention',
      canCreate: true,
      summary: '材料状态需要确认；可先处理知识库，也可基于当前资料继续。',
      notes,
    }
  }

  return {
    status: 'ready',
    canCreate: true,
    summary: '原文已同步，可以开展研究。',
    notes: ['这只表示资料已准备，不保证足以回答具体问题；研究阶段仍需核查证据。'],
  }
}
