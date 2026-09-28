<template>
  <section class="research-workspace" aria-label="研究阶段与成果">
    <div v-if="loading && !workspace" class="workspace-state" role="status">正在读取已保存阶段…</div>
    <div v-if="error" class="workspace-state" role="alert"><span>{{ error }}</span><el-button @click="loadWorkspace()">重新读取</el-button></div>
    <template v-if="workspace">
      <div class="workspace-overview">
        <strong>已保存 {{ coverage.saved }} / {{ coverage.planned }} 个主题</strong>
        <span>{{ coverage.unfinished ? `${coverage.unfinished} 个主题尚未完成` : '主题成果已保存；交付状态以报告、检查与演示阶段为准' }}</span>
        <p>已完成阶段在重试后保留；这些是公开业务成果，不是隐藏思考过程，也不代表全部事实已证明。</p>
      </div>
      <details v-if="workspace.plan" class="research-scope">
        <summary>研究目标、约束与验收条件</summary>
        <strong>{{ workspace.plan.goal }}</strong>
        <p v-if="workspace.plan.report_title" class="research-material-title">材料标题：{{ workspace.plan.report_title }}</p>
        <div v-for="(items, label) in { '范围约束': workspace.plan.constraints, '验收条件': workspace.plan.acceptance, '术语口径': workspace.plan.terminology }" :key="label">
          <h4>{{ label }}</h4><ul v-if="items.length"><li v-for="item in items" :key="item">{{ item }}</li></ul><p v-else>未指定</p>
        </div>
        <div v-if="workspace.baselines?.length" class="baseline-list"><h4>已冻结的核验／刷新基线</h4><p>旧版待核验输入，不是当前证据。模型可通过原生工具分页对照当前知识和原文。</p><article v-for="baseline in workspace.baselines" :key="`${baseline.question_id}-${baseline.entry_id}`"><b>{{ baseline.title }}</b><span>版本 {{ baseline.revision }} · {{ baseline.claim_count }} 条具体主张 · {{ baseline.content_characters.toLocaleString() }} 字符</span></article></div>
      </details>
      <details class="research-stage-directory" :open="!isMobile || stageDirectoryOpen" @toggle="stageDirectoryOpen = ($event.target as HTMLDetailsElement).open">
        <summary>研究阶段 <span>{{ selected?.stage.title || `${workspace.stages.length} 个阶段 · 选择查看成果` }}</span></summary>
      <ol class="research-stages">
        <li v-for="stage in workspace.stages" :key="stage.stage_key">
          <button type="button" :class="{ 'is-selected': selected?.stage.stage_key === stage.stage_key }" :aria-pressed="selected?.stage.stage_key === stage.stage_key" @click="loadStage(stage)">
            <i :class="`is-${stage.status}`"></i><span><b>{{ stage.title }}</b><small>{{ stage.summary || researchStageStatus(stage.status) }}</small></span><em>{{ researchStageStatus(stage.status) }}</em>
          </button>
        </li>
      </ol>
      </details>
      <div v-if="stageLoading" class="workspace-state" role="status">正在读取所选阶段…</div>
      <div v-if="stageError" class="workspace-state" role="alert">{{ stageError }}</div>
      <article v-if="selected" class="selected-stage" aria-label="所选阶段成果">
        <header><h4>{{ selected.stage.title }}</h4><span>{{ researchStageStatus(selected.stage.status) }} · {{ selected.stage.revision > 0 ? `版本 ${selected.stage.revision}` : '尚无保存版本' }}</span></header>
        <div v-if="selected.stage.run_id || selected.content_run_id || (currentStage?.revision || 0) > 0" class="stage-controls">
          <button v-if="selected.stage.run_id" type="button" @click="emit('inspect', selected.stage.run_id)">本次阶段检查器</button>
          <button v-if="selected.content_run_id && selected.content_run_id !== selected.stage.run_id" type="button" @click="emit('inspect', selected.content_run_id)">保留正文的来源运行</button>
          <label v-if="(currentStage?.revision || 0) > 0">历史版本 <el-input-number v-model="revision" :min="1" :max="currentStage?.revision" :precision="0" controls-position="right" aria-label="研究阶段历史版本" /></label>
          <button v-if="(currentStage?.revision || 0) > 0" type="button" :disabled="stageLoading" @click="readRevision">读取版本</button>
        </div>
        <p v-if="currentStage && selected.stage.revision !== currentStage.revision" class="stage-notice">此阶段已有新状态或成果，当前显示保存的历史版本。<button type="button" @click="loadStage(currentStage)">读取当前版本</button></p>
        <p v-if="selected.stage.error" class="stage-notice is-error">{{ selected.stage.error }}</p>
        <KnowledgeAnswerMarkdown v-if="selected.content_md" :content="selected.content_md" :evidence-count="evidenceCount" @citation="openCitation" />
        <p v-else class="stage-notice">此阶段尚未保存完整成果。失败、排队和运行中的内容不会冒充已完成报告。</p>
        <section v-if="selected.findings.length" class="findings"><h4>发现与证据缺口</h4><p>以下是模型的公开判定；实际引用及版本由系统校验，不等于独立事实证明。</p><article v-for="(finding, index) in selected.findings" :key="index"><span :class="`is-${finding.status}`">{{ researchFindingStatus(finding.status) }}</span><strong>{{ finding.finding }}</strong><small v-if="finding.baseline_entry_id">核验对象：{{ finding.baseline_entry_id }}<template v-if="finding.baseline_claim_id"> · 主张 {{ finding.baseline_claim_id }}</template></small><ul v-if="finding.limitations.length"><li v-for="limitation in finding.limitations" :key="limitation">{{ limitation }}</li></ul><div class="finding-citations"><button v-for="index in finding.citation_indices" :key="index" type="button" @click="openCitation(index - 1)">S{{ index }} · 实际读取快照</button></div></article></section>
      </article>
    </template>
    <p v-else-if="!loading && !error" class="workspace-state">此任务尚未建立阶段记录。旧任务不补造规划或核验历史，已有报告仍可回看。</p>
    <KnowledgeCitationPreview v-model="citationVisible" :entry="null" :run-id="citationRun" :source-index="citationIndex" />
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useMediaQuery } from '@vueuse/core'
import { getKnowledgeResearchWorkspace, getKnowledgeResearchStage, type ResearchWorkspace, type ResearchStageContent, type ResearchStageSummary } from '@/api/knowledge'
import { researchCoverage, researchStageStatus, researchFindingStatus, pickResearchStageToOpen, shouldRefreshResearchStage } from '@/utils/knowledgeResearch'
import KnowledgeAnswerMarkdown from './KnowledgeAnswerMarkdown.vue'
import KnowledgeCitationPreview from './KnowledgeCitationPreview.vue'

const props = defineProps<{ taskId: string; taskStatus: string; active: boolean }>()
const emit = defineEmits<{ inspect: [runId: string] }>()
const workspace = ref<ResearchWorkspace | null>(null)
const loading = ref(false)
const error = ref('')
const selected = ref<ResearchStageContent | null>(null)
const stageDirectoryOpen = ref(true)
const isMobile = useMediaQuery('(max-width: 768px)')
const stageLoading = ref(false)
const stageError = ref('')
const revision = ref(1)
const followLatest = ref(true)
const citationVisible = ref(false)
const citationRun = ref('')
const citationIndex = ref(0)
const coverage = computed(() => researchCoverage(workspace.value?.stages || []))
const currentStage = computed(() => workspace.value?.stages.find(stage => stage.stage_key === selected.value?.stage.stage_key))
const evidenceCount = computed(() => Math.max(0, ...(selected.value?.evidence.map(item => item.citation_index) || [])))
let workspaceRequest = 0
let stageRequest = 0
let timer: ReturnType<typeof setTimeout> | undefined
let alive = true
function visible() { return alive && props.active && document.visibilityState === 'visible' }
function stop() { clearTimeout(timer); timer = undefined }
function schedule() {
  stop()
  if (visible() && ['queued', 'running'].includes(props.taskStatus)) timer = setTimeout(() => void loadWorkspace(true), 2400)
}
async function loadWorkspace(quiet = false) {
  if (!visible()) return
  const request = ++workspaceRequest
  if (!quiet) loading.value = true
  try {
    const response = await getKnowledgeResearchWorkspace(props.taskId)
    if (request !== workspaceRequest || !visible()) return
    if (response.status !== 'success') throw new Error(response.error?.message || '研究阶段读取失败')
    workspace.value = response.result || null
    error.value = ''
    if (workspace.value && !stageLoading.value && !stageError.value) {
      if (selected.value) {
        const current = workspace.value.stages.find(stage => stage.stage_key === selected.value?.stage.stage_key)
        if (current && shouldRefreshResearchStage(selected.value.stage, workspace.value.stages, followLatest.value)) void loadStage(current)
      } else {
        const focus = pickResearchStageToOpen(workspace.value.stages)
        if (focus) void loadStage(focus)
      }
    }
  } catch (failure) { if (request === workspaceRequest && visible()) error.value = (failure as Error).message }
  finally { if (request === workspaceRequest) { loading.value = false; schedule() } }
}
async function loadStage(stage: ResearchStageSummary, requestedRevision?: number) {
  if (!visible()) return
  const request = ++stageRequest
  stageLoading.value = true
  stageError.value = ''
  if (selected.value?.stage.stage_key !== stage.stage_key) selected.value = null
  try {
    const response = await getKnowledgeResearchStage(props.taskId, stage.stage_key, requestedRevision)
    if (request !== stageRequest || !visible()) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '阶段成果读取失败')
    selected.value = response.result
    revision.value = response.result.stage.revision
    followLatest.value = requestedRevision == null
    if (isMobile.value) stageDirectoryOpen.value = false
  } catch (failure) { if (request === stageRequest && visible()) stageError.value = (failure as Error).message }
  finally { if (request === stageRequest) stageLoading.value = false }
}
function readRevision() { if (currentStage.value) void loadStage(currentStage.value, revision.value) }
function openCitation(index: number) {
  const run = selected.value?.content_run_id
  if (!run || !selected.value?.evidence.some(item => item.citation_index === index + 1)) return
  citationRun.value = run
  citationIndex.value = index
  citationVisible.value = true
}
function visibilityChanged() { if (visible()) void loadWorkspace(true); else { stop(); ++workspaceRequest; ++stageRequest; loading.value = false; stageLoading.value = false } }
watch(() => [props.taskId, props.active], () => {
  ++workspaceRequest; ++stageRequest; stop(); loading.value = false; stageLoading.value = false
  if (!props.active) { citationVisible.value = false; return }
  workspace.value = null; selected.value = null; followLatest.value = true; error.value = ''; stageError.value = ''
  void loadWorkspace()
}, { immediate: true })
watch(() => props.taskStatus, () => { if (visible()) void loadWorkspace(true) })
onMounted(() => document.addEventListener('visibilitychange', visibilityChanged))
onBeforeUnmount(() => { alive = false; stop(); ++workspaceRequest; ++stageRequest; document.removeEventListener('visibilitychange', visibilityChanged) })
</script>

<style scoped>
.research-workspace { min-width: 0; display: grid; gap: 12px; font-size: 13px; line-height: 1.65; }
.research-stage-directory > summary { display: none; }
.research-workspace p { margin: 0; color: var(--text-muted); overflow-wrap: anywhere; }
.workspace-state { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 14px; color: var(--text-muted); }
.workspace-overview { display: grid; gap: 4px; padding: 14px; border-radius: 14px; background: var(--accent-light); }
.workspace-overview strong { color: var(--accent); font-size: 15px; }
.workspace-overview > span { color: var(--text-secondary); }
.research-scope, .selected-stage { min-width: 0; padding: 14px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); overflow-wrap: anywhere; }
.research-scope summary { min-height: 32px; color: var(--text-secondary); font-weight: 650; cursor: pointer; }
.research-material-title { margin: 6px 0 0; color: var(--text-muted); }
.research-scope h4, .selected-stage h4 { margin: 10px 0 4px; font-size: 13px; }
.research-scope ul, .findings ul { margin: 4px 0; padding-left: 20px; }
.baseline-list article { display: grid; gap: 2px; margin-top: 8px; }.baseline-list span { color: var(--text-muted); }
.research-stages { display: grid; gap: 6px; padding: 0; margin: 0; list-style: none; }
.research-stages button { width: 100%; display: grid; grid-template-columns: 10px minmax(0,1fr) auto; align-items: center; gap: 10px; text-align: left; padding: 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-primary); cursor: pointer; font: inherit; transition: background var(--motion-fast), border-color var(--motion-fast); }
.research-stages button:active { background: var(--accent-light); }.research-stages button.is-selected { border-color: var(--accent-border); background: var(--accent-light); }
.research-stages i { width: 7px; height: 7px; border-radius: 50%; background: var(--text-faint); }.research-stages i.is-running, .research-stages i.is-completed { background: var(--accent); }.research-stages i.is-failed, .research-stages i.is-stale { background: var(--danger,#d9342b); }
.research-stages span { min-width: 0; display: grid; gap: 2px; }.research-stages small { color: var(--text-muted); overflow-wrap: anywhere; }.research-stages em { font-size: 11px; font-style: normal; color: var(--text-muted); }
.selected-stage header { display: flex; align-items: center; flex-wrap: wrap; justify-content: space-between; gap: 8px; }.selected-stage header h4 { margin: 0; }.selected-stage header span { color: var(--text-muted); }
.stage-controls { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 12px 0; }.stage-controls label { display: flex; align-items: center; gap: 6px; color: var(--text-muted); }.stage-controls :deep(.el-input-number) { width: 90px; }
.stage-controls button, .finding-citations button, .stage-notice button { min-height: 36px; border: 0; border-radius: 9px; padding: 5px 10px; background: var(--accent-light); color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; }
.stage-controls button:disabled { opacity: .5; }.stage-notice { padding: 10px; margin: 10px 0 !important; border-radius: 10px; background: var(--accent-light); }.stage-notice.is-error { color: var(--danger,#d9342b); }
.findings { display: grid; gap: 8px; margin-top: 14px; }.findings article { display: grid; gap: 5px; padding: 12px; border-radius: 12px; background: var(--bg-glass-subtle); }.findings article > span { color: var(--accent); font-size: 11px; }.findings .is-missing, .findings .is-conflict { color: var(--danger,#d9342b); }.findings small { overflow-wrap: anywhere; color: var(--text-muted); }.finding-citations { display: flex; flex-wrap: wrap; gap: 6px; }
@media (max-width:768px) { .research-stages button { grid-template-columns: 10px minmax(0,1fr); }.research-stages em { grid-column: 2; }.research-scope summary { min-height: 44px; display: flex; align-items: center; }.stage-controls button, .finding-citations button, .stage-notice button { min-height: 44px; }.stage-controls label { flex: 1; }.workspace-overview, .research-scope, .selected-stage { padding: 12px; } }
@media (prefers-reduced-motion:reduce) { .research-stages button { transition: none; } }
@media (max-width: 768px) {
  .research-stage-directory > summary { min-height: 48px; display: flex; align-items: center; flex-wrap: wrap; gap: 4px 10px; padding: 8px 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-primary); cursor: pointer; list-style: none; }
  .research-stage-directory > summary::-webkit-details-marker { display: none; }
  .research-stage-directory > summary::after { content: '+'; margin-left: auto; color: var(--accent); }
  .research-stage-directory[open] > summary::after { content: '−'; }
  .research-stage-directory > summary span { min-width: 0; color: var(--text-muted); overflow-wrap: anywhere; font-size: 12px; }
  .research-stage-directory[open] .research-stages { margin-top: 8px; }
  .selected-stage { padding: 0; border: 0; background: transparent; }
}
</style>
