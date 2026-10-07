<template>
  <MotionModal :model-value="modelValue" aria-label="Wiki 编译覆盖报告" size="wide" @update:model-value="emit('update:modelValue', $event)">
    <div class="knowledge-modal-card compile-report">
      <div class="knowledge-modal-head"><div><h3>编译覆盖报告</h3><p>{{ bookName }} · 分析范围、主题归并与当次审核结果</p></div></div>
      <div class="knowledge-modal-body report-body">
      <p v-if="loading && !report" role="status">正在读取报告…</p>
      <p v-if="error" role="alert" class="report-warning">{{ error }}</p>
      <p v-if="waitingForReport" role="status" class="report-warning">新编译已进入队列，正在等待本次报告建立；下方仍是之前的历史报告。</p>
      <template v-if="report">
        <div class="report-status"><strong>{{ compileReportStatus(report.status) }}</strong><small>{{ formatTime(report.created_at) }}</small></div>
        <p class="report-note">已分析不等于知识已完整提取；候选引用只说明本批关联的来源。主题覆盖／冲突由模型报告，不是独立事实核验。当次审核已应用也不代表知识目前仍有效。</p>
        <p v-if="report.error" class="report-warning">{{ report.error }}</p>
        <div class="report-metrics">
          <div><strong>{{ report.selected_sources }} / {{ report.current_sources }}</strong><span>本次分析来源 / 当时当前来源</span></div>
          <div><strong>{{ report.analyzed_fragments }} / {{ report.fragment_total }}</strong><span>{{ report.planned ? '已分析 / 计划片段' : '已分析 / 待规划来源段' }}</span></div>
          <div><strong>{{ report.topic_total }}</strong><span>已记录主题结果</span></div>
          <div><strong>{{ report.no_material_fragments }}</strong><span>无实质结果片段</span></div>
          <div><strong>{{ report.failed_fragments }}</strong><span>本批分析未完成</span></div>
          <div><strong>{{ report.unprocessed_fragments }}</strong><span>尚未完成分析</span></div>
        </div>
        <section aria-label="编译来源片段"><h4>来源分析</h4>
          <article v-for="fragment in report.fragments" :key="fragment.ordinal" class="report-row">
            <header><strong>{{ fragment.source_path }}</strong><span>{{ compileFragmentStatus(fragment.status) }}</span></header>
            <small>第 {{ fragment.batch }} 批<span v-if="fragment.line_start != null"> · 行 {{ fragment.line_start }}–{{ fragment.line_end ?? fragment.line_start }}</span></small>
            <p v-if="headingPath(fragment)">{{ headingPath(fragment) }}</p>
            <p v-if="fragment.reason">{{ fragment.reason }}</p>
            <small v-if="fragment.candidate_slugs.length">本批关联候选：{{ fragment.candidate_slugs.join('、') }}（不证明每个片段已覆盖）</small>
          </article>
          <nav class="report-pagination" aria-label="来源分页"><el-button :disabled="loading || report.fragment_offset === 0" @click="load(report.id, Math.max(0, report.fragment_offset - pageSize), report.topic_offset)">上一页</el-button><span>{{ pageNumber(report.fragment_offset) }} · 共 {{ report.fragment_total }} 片段</span><el-button :disabled="loading || !report.fragment_has_more" @click="load(report.id, report.fragment_offset + pageSize, report.topic_offset)">下一页</el-button></nav>
        </section>
        <section v-if="report.topic_total" aria-label="编译主题结果"><h4>主题归并与冲突</h4>
          <article v-for="(topic, index) in report.topics" :key="`${report.topic_offset}:${index}`" class="report-row">
            <header><strong>{{ topic.title || topic.slug }}</strong><span>{{ compileTopicStatus(topic.outcome, report.status) }}</span></header>
            <p v-if="topic.reason">{{ topic.reason }}</p>
            <small v-if="topic.preserved_claim_count != null">保留 {{ topic.preserved_claim_count }} 条论断<span v-if="topic.retired_claim_count"> · {{ topic.retired_claim_count }} 条失效旧论断保留在历史</span></small>
            <small v-if="topic.body_coverage">正文覆盖为模型自报，不是独立事实核验。</small>
            <ul v-if="topic.conflicts?.length" class="report-conflicts"><li v-for="(conflict, conflictIndex) in topic.conflicts" :key="conflictIndex">{{ conflict.reason }}<small>涉及论断 {{ conflict.claim_indices.map(value => value + 1).join('、') }}；请在变更审核中对照。</small></li></ul>
          </article>
          <nav class="report-pagination" aria-label="主题分页"><el-button :disabled="loading || report.topic_offset === 0" @click="load(report.id, report.fragment_offset, Math.max(0, report.topic_offset - pageSize))">上一页</el-button><span>{{ pageNumber(report.topic_offset) }} · 共 {{ report.topic_total }} 主题</span><el-button :disabled="loading || !report.topic_has_more" @click="load(report.id, report.fragment_offset, report.topic_offset + pageSize)">下一页</el-button></nav>
        </section>
        <nav class="report-history" aria-label="编译历史"><el-button :disabled="loading || !report.previous_report_id" @click="load(report.previous_report_id || undefined)">较早报告</el-button><el-button :disabled="loading || !report.next_report_id" @click="load(report.next_report_id || undefined)">较新报告</el-button></nav>
      </template>
      <p v-else-if="!loading && !error && !waitingForReport" class="report-note">尚未记录覆盖报告。旧编译保留原记录，不补造覆盖结果；下次智能编译会建立报告。</p>
      </div>
      <div class="knowledge-modal-actions"><UiAction :icon="Refresh" label="刷新最新编译报告" :loading="loading" @click="load()" /><el-button v-if="report?.status === 'waiting_review' && report.change_set_id" type="primary" @click="openReview">审核变更</el-button><UiAction :icon="Close" label="关闭编译报告" @click="emit('update:modelValue', false)" /></div>
    </div>
  </MotionModal>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { Close, Refresh } from '@element-plus/icons-vue'
import UiAction from '@/components/motion/UiAction'
import { getBookKnowledgeBase, getKnowledgeCompileReport, type KnowledgeBaseSummary, type KnowledgeCompileFragment, type KnowledgeCompileReport } from '@/api/knowledge'
import { compileReportStatus, compileFragmentStatus, compileTopicStatus } from '@/utils/knowledgeCompileReport'

const props = defineProps<{ modelValue: boolean; baseId: string; bookName?: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; review: [changeSetId: string]; 'base-updated': [base: KnowledgeBaseSummary] }>()
const report = ref<KnowledgeCompileReport | null>(null)
const loading = ref(false)
const error = ref('')
const waitingForReport = ref(false)
const pageSize = 20
let generation = 0
let poll: ReturnType<typeof setTimeout> | undefined
function clearPoll() { if (poll != null) clearTimeout(poll); poll = undefined }
function scheduleRefresh(current: number) {
  poll = setTimeout(() => {
    if (current !== generation || !props.modelValue || (!waitingForReport.value && report.value?.status !== 'running')) return
    if (document.hidden) { scheduleRefresh(current); return }
    void load(waitingForReport.value ? undefined : report.value?.id, report.value?.fragment_offset, report.value?.topic_offset)
  }, 5000)
}
async function load(id?: string, fragmentOffset = 0, topicOffset = 0) {
  clearPoll()
  if (!props.modelValue || !props.baseId) return
  const current = ++generation
  loading.value = true
  error.value = ''
  try {
    const response = await getKnowledgeCompileReport(props.baseId, id, fragmentOffset, topicOffset, pageSize)
    if (current !== generation) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '报告读取失败')
    report.value = response.result.report
    const base = await getBookKnowledgeBase(props.baseId)
    if (current !== generation) return
    if (base.status !== 'success' || !base.result) throw new Error(base.error?.message || '编译状态读取失败')
    waitingForReport.value = !id && base.result.compile_state === 'compiling' && report.value?.status !== 'running'
    emit('base-updated', base.result)
  } catch (failure) {
    if (current === generation) error.value = failure instanceof Error ? failure.message : '报告读取失败'
  } finally {
    if (current === generation) {
      loading.value = false
      if ((report.value?.status === 'running' || waitingForReport.value) && props.modelValue) {
        scheduleRefresh(current)
      }
    }
  }
}
function headingPath(fragment: KnowledgeCompileFragment) { const path = fragment.locator.heading_path; return Array.isArray(path) ? path.filter(item => typeof item === 'string').join(' › ') : '' }
function pageNumber(offset: number) { return `第 ${Math.floor(offset / pageSize) + 1} 页` }
function formatTime(value: string) { return new Date(value).toLocaleString() }
function openReview() {
  if (!report.value?.change_set_id) return
  emit('update:modelValue', false)
  emit('review', report.value.change_set_id)
}
watch(() => [props.modelValue, props.baseId], () => { generation++; clearPoll(); report.value = null; error.value = ''; loading.value = false; waitingForReport.value = false; if (props.modelValue) void load() }, { immediate: true })
onBeforeUnmount(() => { generation++; clearPoll() })
</script>

<style scoped>
.compile-report { min-width: 0; font-size: .875rem; line-height: 1.65; overflow-wrap: anywhere; }
.report-body { flex: 1; display: block; }
.report-status, .report-row header { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: baseline; gap: .5rem; }
.report-note, .report-row small, .report-status small { color: var(--text-muted); font-size: .8125rem; }
.report-warning { color: var(--warning, #a66b21); }
.report-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: .625rem; margin: 1rem 0; }
.report-metrics > div { display: grid; gap: .25rem; border-radius: 12px; padding: .75rem; background: var(--bg-glass-subtle); min-width: 0; }
.report-metrics strong { font-size: 1.125rem; font-variant-numeric: tabular-nums; }
.report-metrics span { font-size: .75rem; color: var(--text-muted); }
.compile-report h4 { font-size: .9375rem; font-weight: 650; margin: 1.25rem 0 .5rem; }
.report-row { padding: .875rem; margin-top: .625rem; border-radius: 12px; background: var(--bg-glass-subtle); }
.report-row header > strong { min-width: 0; flex: 1 1 12rem; }
.report-row header > span { color: var(--accent); font-size: .75rem; }
.report-row p { margin: .375rem 0; }
.report-row small { display: block; }
.report-pagination, .report-history { display: flex; flex-wrap: wrap; align-items: center; justify-content: center; gap: .5rem; margin-top: .75rem; }
.report-pagination span { color: var(--text-muted); font-size: .75rem; }
.report-pagination .el-button, .report-history .el-button { margin: 0; }
.report-conflicts { padding-left: 1.25rem; color: var(--warning, #a66b21); }
.report-conflicts li + li { margin-top: .625rem; }
@media (max-width: 640px) { .report-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); } .report-pagination .el-button { min-height: 40px; } }
</style>
