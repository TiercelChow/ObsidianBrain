import type { KnowledgeBaseSummary } from '../api/knowledge'

type ResearchReadinessBase = Pick<KnowledgeBaseSummary,
  'lifecycle' | 'source_available' | 'sync_state' | 'compile_mode' | 'compile_state' |
  'compile_phase' | 'source_count' | 'entry_count' | 'pending_review_count'>

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

  if (base.compile_mode === 'chapter') {
    notes.push('当前只有章节索引，尚无智能编译的语义知识；研究仍可使用原文。')
  }
  if (base.compile_state === 'failed') {
    notes.push('智能编译失败；可以先处理失败原因，或基于当前资料继续。')
  } else if (base.compile_state === 'outdated') {
    notes.push('智能 Wiki 待更新；已有条目可能落后于原文。')
  } else if (base.compile_state === 'compiling') {
    notes.push('智能编译正在进行；本次研究只能使用当前已保存的正式知识。')
  } else if (base.compile_mode === 'smart' && base.compile_state === 'not_started') {
    notes.push('智能 Wiki 尚未编译；研究会更多依赖章节与原文。')
  }

  if (base.pending_review_count > 0) {
    notes.push(`有 ${base.pending_review_count} 项待审核候选；它们尚未成为正式知识。`)
  } else if (base.compile_phase === 'waiting_review') {
    notes.push('编译结果正在等待审核；候选尚未成为正式知识。')
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
    summary: '来源已同步，智能 Wiki 可检索。',
    notes: ['这只表示资料已准备，不保证足以回答具体问题；研究阶段仍需核查证据。'],
  }
}
