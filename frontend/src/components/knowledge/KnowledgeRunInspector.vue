<template>
  <section class="adaptive-inspector" aria-label="目标与取证预算">
    <template v-if="diagnostics.available">
      <header><h4>本轮目标</h4><span>{{ depthLabels[diagnostics.plan.depth] || '按问题规划' }}</span></header>
      <p class="adaptive-goal">{{ diagnostics.plan.goal || '当前运行未记录结构化目标' }}</p>
      <ul v-if="diagnostics.plan.constraints.length" class="adaptive-constraints"><li v-for="item in diagnostics.plan.constraints" :key="item">{{ item }}</li></ul>
      <section v-if="diagnostics.research.stageKey" class="adaptive-coverage">
        <h4>当前研究阶段 · {{ diagnostics.research.title || researchPhase }}</h4>
        <p v-if="diagnostics.research.question">{{ diagnostics.research.question }}</p>
        <ul v-if="diagnostics.research.requirements.length"><li v-for="item in diagnostics.research.requirements" :key="item">{{ item }}</li></ul>
      </section>
      <div class="adaptive-metrics">
        <div v-if="diagnostics.planning.total != null"><small>已浏览编译目录</small><strong>{{ format(diagnostics.planning.seen) }} / {{ format(diagnostics.planning.total) }}</strong></div>
        <div v-if="diagnostics.research.initialTarget != null"><small>初始候选目标 · 非实际覆盖</small><strong>{{ format(diagnostics.research.initialTarget) }}</strong></div>
        <div v-if="diagnostics.research.capacity != null"><small>{{ diagnostics.research.capacityBasis === 'observed_runtime_capacity' ? 'Runtime 上报的模型容量' : diagnostics.research.capacityKnown ? '配置声明的模型容量' : '未知模型容量 · 应用默认值' }}</small><strong>{{ format(diagnostics.research.capacity) }} tokens</strong></div>
        <div><small>工具调用 · 当前软限 / 硬限</small><strong>{{ format(diagnostics.budget.used) }} · {{ format(diagnostics.budget.soft) }} / {{ format(diagnostics.budget.hard) }}</strong></div>
        <div><small>工具返回负载估算 · 硬上限</small><strong>{{ format(diagnostics.budget.tokens) }} / {{ format(diagnostics.budget.hardTokens) }}</strong><small>数据取证软限：{{ format(diagnostics.budget.softTokens) }}</small></div>
        <div><small>本轮输出 / 有效上下文</small><strong>{{ format(runtime.budget.effective_max_output_tokens) }} / {{ format(runtime.budget.context_window) }} tokens</strong></div>
        <div v-if="diagnostics.research.contentTokens != null"><small>正文篇幅估计 · 非目标长度</small><strong>{{ format(diagnostics.research.contentTokens) }} tokens</strong></div>
        <div v-if="diagnostics.research.structureTokens != null"><small>JSON 结构基础预留</small><strong>{{ format(diagnostics.research.structureTokens) }} tokens</strong></div>
        <div v-if="diagnostics.research.reasoningTokens != null"><small>推理基础预留 · 非实测用量</small><strong>{{ format(diagnostics.research.reasoningTokens) }} tokens</strong></div>
        <div v-if="diagnostics.planning.secondsLimit != null"><small>规划耗时 / 上限</small><strong>{{ diagnostics.planning.elapsedMs == null ? '—' : `${(diagnostics.planning.elapsedMs / 1000).toFixed(1)} 秒` }} / {{ format(diagnostics.planning.secondsLimit) }} 秒</strong></div>
        <div v-if="diagnostics.research.secondsLimit != null"><small>研究阶段执行上限 · 非已用时</small><strong>{{ format(diagnostics.research.secondsLimit) }} 秒</strong></div>
      </div>
      <p class="adaptive-note">软预算可按证据缺口扩大；硬上限、时间与授权不变。负载为业务估算，不是模型账单；浏览目录不代表已读正文。</p>
      <p v-if="diagnostics.planning.stopReason === 'planning_budget_reached'" class="adaptive-warning">目录规划已到预算上限；剩余目录需要由 Agent 针对具体缺口补查，不代表全书已覆盖。</p>
      <p v-if="['planner_failed', 'planner_invalid_output'].includes(diagnostics.planning.stopReason)" class="adaptive-warning">目录规划未完成，本轮保留已选候选并使用只读工具补查。</p>
      <section v-if="diagnostics.coverage.length" class="adaptive-coverage">
        <h4>子问题与依据</h4><p class="adaptive-note">以下为 Agent 自报的取证覆盖，不等于独立事实核验。尚未报告的子问题不推定已完成。</p>
        <article v-for="item in diagnostics.coverage" :key="item.index">
          <header><strong>{{ item.index + 1 }}. {{ item.question }}</strong><span>{{ item.finding ? statusLabels[item.status] || '未报告' : '尚未报告' }}</span></header>
          <p v-if="item.finding">{{ item.finding }}</p>
          <small v-if="item.citationIndices.length">已读依据：{{ item.citationIndices.map(index => `S${index}`).join(' · ') }}</small>
        </article>
      </section>
      <section v-if="diagnostics.expansions.length"><h4>预算扩展原因</h4><ul><li v-for="(reason, index) in diagnostics.expansions" :key="index">{{ reason }}</li></ul></section>
      <section v-if="diagnostics.limits.length"><h4>取证限制记录</h4><ul><li v-for="(reason, index) in diagnostics.limits" :key="index">{{ reason }}</li></ul><p class="adaptive-note">这是工具请求被限制的原因，不表示回答已完整完成。</p></section>
    </template>
    <p v-else-if="!diagnostics.presentation.mode" class="adaptive-note">该运行尚未记录自适应规划。历史运行保持原有记录，不补造目标或覆盖结果。</p>
    <section v-if="diagnostics.presentation.mode" class="adaptive-coverage" aria-label="演示选材范围">
      <h4>演示选材范围</h4>
      <p v-if="diagnostics.presentation.mode === 'full_report'">策划输入为完整报告，共 {{ format(diagnostics.presentation.reportCharacters) }} 字符。</p>
      <template v-else-if="diagnostics.presentation.mode === 'whole_structure_projection'">
        <p>报告超出本阶段容量，按整份报告的 {{ format(diagnostics.presentation.sections) }} 个主题选材，保留各主题的发现、限制及可容纳的完整结构片段；不是只截取报告开头。</p>
        <div class="adaptive-metrics"><div><small>完整报告</small><strong>{{ format(diagnostics.presentation.reportCharacters) }} 字符</strong></div><div><small>本次未选正文</small><strong>{{ format(diagnostics.presentation.omittedCharacters) }} 字符</strong></div><div><small>选材负载估算 · 非计费</small><strong>{{ format(diagnostics.presentation.materialTokens) }} tokens</strong></div></div>
        <p class="adaptive-note">完整报告仍保留。选材不等于全文，不允许补造未选内容，原有引用编号不变。</p>
      </template>
      <p v-else class="adaptive-note">该运行记录了选材方式，但此版本不能解释其完整范围，不推定全文已覆盖。</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { AgentRunInspection } from '@/api/knowledge'
import { adaptiveKnowledgeDiagnostics } from '@/utils/knowledgeAdaptiveDiagnostics'
import { knowledgeRunDiagnostics } from '@/utils/knowledgeRunDiagnostics'
const props = defineProps<{ inspection: AgentRunInspection | null }>()
const diagnostics = computed(() => adaptiveKnowledgeDiagnostics(props.inspection?.snapshot?.evidence_refs, props.inspection?.events || []))
const runtime = computed(() => knowledgeRunDiagnostics(props.inspection?.events || [], props.inspection?.snapshot?.evidence_refs.runtime_budget))
const depthLabels: Record<string, string> = { brief: '简洁回答', standard: '标准解释', detailed: '深入分析', comprehensive: '综合回答', deep: '深入研究' }
const researchPhase = computed(() => ({ plan: '目标规划', synthesis: '综合结论与交叉核验', report: '报告组装', validation: '引用与结构校验', presentation: '演示交付' })[diagnostics.value.research.stageKey as 'plan' | 'synthesis' | 'report' | 'validation' | 'presentation'] || '主题取证')
const statusLabels: Record<string, string> = { supported: '报告已有依据', partial: '部分覆盖', missing: '仍有缺口', conflict: '依据冲突' }
function format(value: number | null) { return value == null ? '—' : value.toLocaleString() }
</script>

<style scoped>
.adaptive-inspector { min-width: 0; color: var(--text-primary); font-size: .875rem; line-height: 1.65; overflow-wrap: anywhere; }
.adaptive-inspector header { display: flex; align-items: baseline; justify-content: space-between; gap: .75rem; }
.adaptive-inspector h4 { font-size: .9375rem; margin: 0 0 .5rem; font-weight: 650; }
.adaptive-inspector header > span { flex: none; color: var(--text-muted); font-size: .75rem; }
.adaptive-goal { margin: .25rem 0 .75rem; }
.adaptive-constraints { color: var(--text-secondary); padding-left: 1.25rem; }
.adaptive-metrics { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .625rem; margin: 1rem 0; }
.adaptive-metrics > div { min-width: 0; border-radius: 12px; background: var(--bg-glass-subtle); padding: .75rem; display: grid; gap: .25rem; }
.adaptive-metrics small, .adaptive-note, .adaptive-coverage small { color: var(--text-muted); }
.adaptive-note { font-size: .8125rem; }
.adaptive-warning { color: var(--warning, #a66b21); }
.adaptive-coverage { margin-top: 1.5rem; }
.adaptive-coverage article { background: var(--bg-glass-subtle); border-radius: 12px; padding: .875rem; margin-top: .625rem; }
.adaptive-coverage article p { margin: .375rem 0; }
@media (max-width: 640px) { .adaptive-metrics { grid-template-columns: 1fr; } .adaptive-inspector header { flex-wrap: wrap; } }
</style>
