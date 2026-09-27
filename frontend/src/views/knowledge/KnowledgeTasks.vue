<template>
  <KnowledgePageShell title="研究任务" subtitle="让 Agent 围绕一本书持续研究、刷新与审核知识">
    <template #actions>
      <el-button type="primary" :disabled="!bases.length" @click="openCreate"><el-icon><Plus /></el-icon>新建任务</el-button>
    </template>

    <div class="task-filter knowledge-toolbar">
      <el-select v-model="filterBaseId" class="knowledge-select is-responsive" popper-class="system-select-popper system-toolbar-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="全部知识库" clearable @change="loadTasks()">
        <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
      </el-select>
      <button class="mobile-create-task" type="button" aria-label="新建研究任务" :disabled="!bases.length" @click="openCreate"><el-icon><Plus /></el-icon></button>
      <div class="task-summary">{{ tasks.length }} 项任务 · 报告仅依据当前书籍的数据库证据生成</div>
    </div>

    <div v-if="loading" class="knowledge-empty knowledge-surface"><el-icon class="is-loading"><Loading /></el-icon>正在加载任务…</div>
    <section v-else-if="tasks.length" class="research-task-list">
      <article v-for="(task, index) in tasks" :key="task.id" class="research-task knowledge-surface" :style="{ '--order': index }">
        <div class="task-kind" :class="`is-${task.task_type}`"><el-icon><component :is="taskIcon(task.task_type)" /></el-icon></div>
        <div class="task-main">
          <div class="task-topline"><span>{{ task.book_name }}</span><span class="knowledge-status" :class="`is-${task.status}`">{{ statusLabel(task.status) }}</span></div>
          <h2>{{ task.title }}</h2>
          <p>{{ task.description || '没有补充任务说明' }}</p>
          <div v-if="task.status === 'running' && taskActivity[task.id]" class="task-live" role="status">
            <i></i><span>{{ taskActivity[task.id] }}</span>
          </div>
          <p v-if="task.result_summary" class="task-result-preview">{{ task.result_summary }}</p>
          <footer><span>{{ typeLabel(task.task_type) }}</span><span>{{ task.deliverable_type === 'presentation' ? 'PPTX 演示文稿' : '研究报告' }}</span><span v-if="task.external_research_enabled">外部资料 {{ task.external_requests_used }}/{{ task.external_request_limit }}</span><span v-if="task.knowledge_change_state === 'proposed'">待知识审核</span><time>{{ formatDate(task.updated_at) }}</time></footer>
        </div>
        <div class="task-card-actions">
          <button class="task-stage-link" type="button" @click="openStages(task)">阶段与成果</button>
          <button class="task-action" type="button" :class="{ 'is-cancel': task.status === 'running' || task.status === 'queued' }" :aria-label="taskActionLabel(task)" :disabled="(Boolean(executingTaskId) && executingTaskId !== task.id) || Boolean(loadingResultId)" @click="runOrOpen(task)">
            <el-icon :class="{ 'is-loading': task.status === 'running' || executingTaskId === task.id || loadingResultId === task.id }">
              <Loading v-if="task.status === 'running' || executingTaskId === task.id || loadingResultId === task.id" />
              <View v-else-if="task.status === 'completed'" />
              <VideoPlay v-else />
            </el-icon>
            <span>{{ taskActionLabel(task) }}</span>
          </button>
        </div>
      </article>
    </section>
    <div v-else class="knowledge-empty knowledge-surface">
      <div class="task-empty-symbol"><el-icon><Operation /></el-icon></div>
      <strong>还没有研究任务</strong>
      <span>创建任务后可交给 Harness 执行，结果和运行状态会保存到数据库。</span>
      <el-button type="primary" :disabled="!bases.length" @click="openCreate">创建第一项任务</el-button>
    </div>

    <MotionModal v-model="createVisible" aria-label="新建研究任务">
      <div class="knowledge-modal-card">
        <div class="knowledge-modal-head"><h3>新建研究任务</h3><p>任务严格绑定一本书，不会跨库读取。</p></div>
        <div class="knowledge-modal-body">
          <el-select v-model="draft.knowledgeBaseId" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库">
            <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
          </el-select>
          <el-input v-model="draft.title" :maxlength="200" show-word-limit placeholder="要研究或核实的问题" />
          <el-input v-model="draft.description" type="textarea" :rows="4" :maxlength="4000" show-word-limit placeholder="补充目标、范围和期望结果" />
          <el-select v-model="draft.taskType" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true">
            <el-option label="专题研究" value="research" />
            <el-option label="知识刷新" value="refresh" />
            <el-option label="事实审核" value="review" />
          </el-select>
          <el-select v-model="draft.deliverableType" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true">
            <el-option label="研究报告" value="report" />
            <el-option label="PPTX 演示文稿" value="presentation" />
          </el-select>
          <section class="external-grant" :class="{ 'is-enabled': draft.externalResearchEnabled }">
            <div class="external-grant-head">
              <div><strong>授权外部资料研究</strong><span>仅本次任务有效，默认关闭</span></div>
              <el-switch v-model="draft.externalResearchEnabled" :disabled="draft.taskType !== 'research'" />
            </div>
            <template v-if="draft.externalResearchEnabled">
              <el-input v-model="draft.externalDomains" type="textarea" :rows="2" placeholder="允许访问的域名，用逗号分隔，例如 docs.example.com, arxiv.org" />
              <div class="external-limit-row">
                <span>最多读取</span>
                <el-input-number v-model="draft.externalRequestLimit" :min="1" :max="50" controls-position="right" />
                <span>个网页</span>
              </div>
              <p>只允许 HTTPS 文本、不会跟随重定向，并拒绝本机和内网地址。外部内容只作为参考，不会自动写入正式知识。</p>
            </template>
            <p v-else-if="draft.taskType !== 'research'">知识刷新和事实审核保持纯书内证据模式，不开放外部网络。</p>
          </section>
        </div>
        <div class="knowledge-modal-actions">
          <el-button @click="createVisible = false">取消</el-button>
          <el-button type="primary" :loading="creating" :disabled="!draft.knowledgeBaseId || !draft.title.trim()" @click="createTask">创建任务</el-button>
        </div>
      </div>
    </MotionModal>

    <MotionModal v-model="resultVisible" aria-label="研究任务结果" size="wide">
      <div v-if="activeTask" class="knowledge-modal-card task-result-modal">
        <div class="knowledge-modal-head">
          <div class="task-result-heading"><span>{{ activeTask.book_name }} · {{ typeLabel(activeTask.task_type) }}</span><h3>{{ activeTask.title }}</h3></div>
          <span class="knowledge-status" :class="`is-${activeTask.status}`">{{ statusLabel(activeTask.status) }}</span>
        </div>
        <div class="task-result-tabs" role="group" aria-label="研究任务结果内容">
          <button type="button" :aria-pressed="resultTab === 'stages'" :class="{ 'is-active': resultTab === 'stages' }" @click="resultTab = 'stages'">研究阶段</button>
          <button type="button" :aria-pressed="resultTab === 'report'" :class="{ 'is-active': resultTab === 'report' }" @click="openResult(activeTask)">研究报告</button>
          <button type="button" :aria-pressed="resultTab === 'inspector'" :class="{ 'is-active': resultTab === 'inspector' }" @click="resultTab = 'inspector'"><el-icon><View /></el-icon>运行检查器</button>
        </div>
        <div v-if="resultTab === 'report'" class="task-result-scroll" role="region" aria-label="研究报告">
          <div v-if="activeTask.artifact_state === 'failed'" class="artifact-failure" role="status"><strong>PPTX 生成失败，研究报告已保留</strong><span>可在下方阅读完整报告，或打开运行检查器定位原因后重新运行。</span></div>
          <div class="task-result-content">
            <KnowledgeAnswerMarkdown :content="reportContent" :evidence-count="activeEvidence.length" @citation="openEvidence" />
          </div>
          <div v-if="activeArtifacts.length" class="task-artifacts">
            <article v-for="artifact in activeArtifacts" :key="artifact.id" class="artifact-card">
              <div class="artifact-main-row">
                <span class="artifact-icon"><el-icon><Download /></el-icon></span>
                <span class="artifact-copy"><b>{{ artifact.title }}</b><small>{{ formatBytes(artifact.size_bytes) }} · {{ artifact.validation_message }}</small></span>
                <button v-if="artifact.agent_run_id" type="button" @click="inspectArtifact(artifact)"><el-icon><View /></el-icon>策划过程</button>
                <a :href="knowledgeArtifactDownloadUrl(artifact.id)" download>下载</a>
              </div>
              <div v-if="artifactQuality(artifact)" class="artifact-quality" aria-label="演示文稿质量摘要">
                <span><b>{{ artifactQuality(artifact)?.slide_count }}</b><small>总页数</small></span>
                <span><b>{{ artifactQuality(artifact)?.layout_count }}</b><small>构图</small></span>
                <span><b>{{ artifactQuality(artifact)?.citation_coverage_percent }}%</b><small>引用覆盖</small></span>
                <span><b>{{ themeLabel(artifactQuality(artifact)?.theme) }}</b><small>视觉主题</small></span>
              </div>
              <details v-if="artifactQuality(artifact)?.checks?.length" class="artifact-checks">
                <summary>查看质量检查</summary>
                <ul><li v-for="check in artifactQuality(artifact)?.checks" :key="check.code"><i :class="{ 'is-pass': check.passed }"></i><span><b>{{ check.label }}</b><small>{{ check.detail }}</small></span></li></ul>
              </details>
            </article>
          </div>
          <button v-if="activeTask.knowledge_change_state === 'proposed'" class="review-result-link" type="button" @click="openTaskReview(activeTask)">研究结论已作为候选保存，前往 Wiki 工作台审核</button>
          <div v-if="activeEvidence.length" class="task-result-evidence">
            <button v-for="(entry, index) in activeEvidence" :key="`${index}-${entry.id}`" type="button" @click="openEvidence(index)">
              <b>S{{ index + 1 }}</b><span>{{ entry.title }}</span><small>{{ entry.source_path || '数据库实体' }}</small>
            </button>
          </div>
        </div>
        <div v-else-if="resultTab === 'stages'" class="task-result-scroll" role="region" aria-label="研究阶段">
          <KnowledgeResearchWorkspace :task-id="activeTask.id" :task-status="activeTask.status" :active="resultVisible && resultTab === 'stages'" @inspect="inspectStage" />
        </div>
        <div v-else class="task-result-scroll" role="region" aria-label="运行检查器">
          <div v-if="inspectionLoading" class="task-inspection-state"><el-icon class="is-loading"><Loading /></el-icon>正在读取运行记录…</div>
          <div v-else-if="inspectionError" class="task-inspection-state is-error"><span>{{ inspectionError }}</span><el-button @click="loadRunInspection(activeRunId)">重新加载检查器</el-button></div>
          <div v-else-if="activeInspection" class="run-inspector">
            <div class="run-inspector-title"><span><el-icon><View /></el-icon><b>运行检查器</b></span><small>{{ activeInspection.run.runtime }} · {{ activeInspection.run.status }}</small></div>
            <div class="run-inspector-body">
              <div class="run-inspector-metrics">
                <span><small>Run</small><code>{{ activeInspection.run.id }}</code></span>
                <span><small>任务类型</small><strong>{{ activeInspection.run.task_type }}</strong></span>
                <span><small>Prompt</small><strong>{{ activeInspection.snapshot?.prompt_characters || 0 }} 字符</strong></span>
                <span><small>事件</small><strong>{{ inspectionEvents.length }} 个阶段</strong></span>
              </div>
              <section v-if="activeInspection.snapshot?.evidence_refs.runtime_budget" class="run-inspector-section">
                <h4>本轮模型能力与运行上限</h4>
                <div class="run-inspector-metrics">
                  <span><small>声明上下文</small><strong>{{ runtimeDiagnostics.budget.context_window == null ? '未声明' : `${runtimeDiagnostics.budget.context_window.toLocaleString()} tokens` }}</strong></span>
                  <span><small>有效输出上限</small><strong>{{ runtimeDiagnostics.budget.effective_max_output_tokens == null ? '继承 Harness' : `${runtimeDiagnostics.budget.effective_max_output_tokens.toLocaleString()} tokens` }}</strong></span>
                  <span><small>推理策略</small><strong>{{ runtimeDiagnostics.budget.reasoning_policy }}</strong></span>
                  <span><small>执行上限</small><strong>{{ runtimeDiagnostics.budget.timeout_seconds ?? '—' }} 秒 · {{ runtimeDiagnostics.budget.tool_call_limit ?? '—' }} 次工具</strong></span>
                </div>
                <p class="run-budget-note">这里显示能力声明与运行上限，不代表已消耗的 token。</p>
              </section>
              <section v-if="runtimeDiagnostics.completion || runtimeDiagnostics.context || runtimeDiagnostics.cost" class="run-inspector-section">
                <h4>运行完成与用量性质</h4>
                <p v-if="runtimeDiagnostics.completion" class="run-budget-note" :class="{ 'is-incomplete': !runtimeDiagnostics.completion.complete }">{{ runtimeDiagnostics.completion.complete ? 'ACP 正常结束' : 'ACP 未完整结束，已有内容不能视为完整报告' }} · {{ runtimeDiagnostics.completion.stopReason }}</p>
                <p v-if="runtimeDiagnostics.context" class="run-budget-note">最近上下文占用：{{ runtimeDiagnostics.context.used.toLocaleString() }} / {{ runtimeDiagnostics.context.size.toLocaleString() }}（不是计费用量）</p>
                <p v-if="runtimeDiagnostics.cost" class="run-budget-note">ACP 会话累计费用：{{ runtimeDiagnostics.cost.amount }} {{ runtimeDiagnostics.cost.currency }}（不换算为 token，不叠加累计快照）</p>
                <p class="run-budget-note">Token 估算仅覆盖初始 Prompt 与最终输出，不含工具历史、内部多轮重发、压缩或推理消耗，不是完整供应商账单。</p>
              </section>
              <KnowledgeRunInspector :inspection="activeInspection" />
              <section v-if="activeInspection.snapshot?.skill_snapshots.length" class="run-inspector-section">
                <h4>Skill 快照</h4>
                <div class="run-skill-list"><span v-for="skill in activeInspection.snapshot.skill_snapshots" :key="skill.id"><b>{{ skill.name }}</b><small>{{ skill.slug }} · Revision {{ skill.revision }} · {{ skill.application_mode === 'harness_native_available' ? 'Harness 可按需读取' : skill.application_mode === 'declared_only' ? '仅声明' : '已注入 Prompt' }}</small></span></div>
              </section>
              <section v-if="activeInspection.snapshot?.config_snapshots.length" class="run-inspector-section">
                <h4>配置快照</h4>
                <div class="run-config-list">
                  <details v-for="config in activeInspection.snapshot.config_snapshots" :key="config.id">
                    <summary><span><b>{{ config.name }}</b><small>{{ config.scope }} · Revision {{ config.revision }}</small></span><em>查看</em></summary>
                    <pre>{{ config.content_md }}</pre>
                  </details>
                </div>
              </section>
              <section class="run-inspector-section">
                <h4>工具白名单</h4>
                <div class="run-tool-list"><code v-for="tool in activeInspection.snapshot?.tool_names || []" :key="tool">{{ tool }}</code><span v-if="!activeInspection.snapshot?.tool_names.length">本次运行没有挂载工具</span></div>
              </section>
              <section v-if="activeInspection.evidence?.length" class="run-inspector-section">
                <h4>实际读取的证据</h4>
                <div class="run-evidence-list">
                  <div v-for="(evidence, index) in activeInspection.evidence" :key="`${evidence.kind}-${evidence.object_id}-${index}`">
                    <strong>{{ evidence.kind === 'entry' ? '知识实体' : evidence.kind === 'source_span' ? '原文片段' : '外部资料' }} · {{ String(evidence.snapshot.title || evidence.snapshot.heading || evidence.snapshot.url || evidence.object_id) }}</strong>
                    <small>{{ evidence.object_id }} · 版本 {{ evidence.version_id }}<template v-if="evidence.snapshot.offset_chars != null"> · 字符 {{ evidence.snapshot.offset_chars }} 起</template></small>
                  </div>
                </div>
              </section>
              <section v-if="inspectionEvents.length" class="run-inspector-section">
                <h4>运行时间线</h4>
                <ol class="run-event-list"><li v-for="event in inspectionEvents" :key="event.sequence"><i></i><div><strong>{{ event.message || event.event_type }}</strong><small>{{ event.phase || event.event_type }} · {{ formatDate(event.created_at) }}</small></div></li></ol>
              </section>
              <details v-if="activeInspection.snapshot" class="effective-prompt">
                <summary><b>有效 Prompt</b><span>{{ activeInspection.snapshot.prompt_hash }}</span></summary>
                <pre>{{ activeInspection.snapshot.prompt_text }}</pre>
              </details>
              <p v-else class="run-inspector-legacy">这是透明化功能上线前的历史运行，没有保存 Prompt 与 Skill 快照。</p>
            </div>
          </div>
        </div>
        <div class="knowledge-modal-actions">
          <el-button v-if="['failed', 'cancelled'].includes(activeTask.status)" :loading="executingTaskId === activeTask.id" @click="runTask(activeTask)">恢复未完成阶段</el-button>
          <el-button v-if="['queued', 'running'].includes(activeTask.status)" @click="cancelTask(activeTask)">取消任务</el-button>
          <el-button type="primary" @click="resultVisible = false">完成</el-button>
        </div>
      </div>
    </MotionModal>
    <KnowledgeCitationPreview v-model="sourcePreviewVisible" :entry="sourcePreviewEntry" :run-id="reportRunId || undefined" :source-index="sourcePreviewIndex" />
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { DataAnalysis, Download, Loading, Operation, Plus, Refresh, Select, VideoPlay, View } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import KnowledgeRunInspector from '@/components/knowledge/KnowledgeRunInspector.vue'
import KnowledgeResearchWorkspace from '@/components/knowledge/KnowledgeResearchWorkspace.vue'
import KnowledgeCitationPreview from '@/components/knowledge/KnowledgeCitationPreview.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import { canFocusDocument } from '@/utils/modalFocusPolicy'
import { knowledgeRunDiagnostics } from '@/utils/knowledgeRunDiagnostics'
import {
  createKnowledgeTask,
  cancelKnowledgeTask,
  executeKnowledgeTask,
  getAgentRunInspection,
  getKnowledgeTaskResult,
  getKnowledgeTaskActivity,
  listBookKnowledgeBases,
  listKnowledgeTasks,
  knowledgeArtifactDownloadUrl,
  type KnowledgeArtifact,
  type AgentRunInspection,
  type PresentationQualityReport,
  type KnowledgeTaskExecution,
  type KnowledgeBaseSummary,
  type KnowledgeEntrySummary,
  type KnowledgeTask,
} from '@/api/knowledge'

const router = useRouter()
const route = useRoute()
const bases = ref<KnowledgeBaseSummary[]>([])
const tasks = ref<KnowledgeTask[]>([])
const filterBaseId = ref('')
const loading = ref(false)
const creating = ref(false)
const executingTaskId = ref('')
const loadingResultId = ref('')
const createVisible = ref(false)
const resultVisible = ref(false)
const activeTask = ref<KnowledgeTask | null>(null)
const activeEvidence = ref<KnowledgeEntrySummary[]>([])
const activeArtifacts = ref<KnowledgeArtifact[]>([])
const activeInspection = ref<AgentRunInspection | null>(null)
const activeRunId = ref('')
const reportRunId = ref('')
const reportContent = ref('')
const sourcePreviewVisible = ref(false)
const sourcePreviewEntry = ref<KnowledgeEntrySummary | null>(null)
const sourcePreviewIndex = ref(0)
const resultTab = ref<'report' | 'inspector' | 'stages'>('report')
const inspectionLoading = ref(false)
const inspectionError = ref('')
const taskActivity = ref<Record<string, string>>({})
const inspectionEvents = computed(() => (activeInspection.value?.events || []).filter(event => event.event_type !== 'run.text_delta'))
const runtimeDiagnostics = computed(() => knowledgeRunDiagnostics(activeInspection.value?.events || [], activeInspection.value?.snapshot?.evidence_refs.runtime_budget))
const draft = reactive({ knowledgeBaseId: '', title: '', description: '', taskType: 'research' as KnowledgeTask['task_type'], deliverableType: 'report' as KnowledgeTask['deliverable_type'], externalResearchEnabled: false, externalDomains: '', externalRequestLimit: 6 })
let viewActive = true
let inspectionRequestId = 0
let resultRequestId = 0
let taskPollTimer: ReturnType<typeof setTimeout> | undefined
let tasksRequestId = 0
watch(resultVisible, visible => { if (!visible) { ++resultRequestId; ++inspectionRequestId; loadingResultId.value = '' } })

watch(() => draft.taskType, taskType => {
  if (taskType !== 'research') draft.externalResearchEnabled = false
})

async function loadData() {
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    bases.value = response.result.items.flatMap(card => (
      card.book.kind === 'folder' && card.knowledge_base ? [card.knowledge_base] : []
    ))
    await loadTasks()
    if (route.query.create === '1') {
      openCreate()
      const requestedBase = String(route.query.base || '')
      if (bases.value.some(base => base.id === requestedBase)) draft.knowledgeBaseId = requestedBase
      draft.title = String(route.query.title || '')
      draft.description = String(route.query.description || '')
    }
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function loadTasks(quiet = false) {
  const request = ++tasksRequestId
  if (!quiet) loading.value = true
  try {
    const response = await listKnowledgeTasks(filterBaseId.value || undefined)
    if (request !== tasksRequestId || !viewActive) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务加载失败')
    tasks.value = response.result.tasks
    const updated = tasks.value.find(task => task.id === activeTask.value?.id)
    if (updated) activeTask.value = updated
  } catch (error) {
    if (!quiet && request === tasksRequestId && viewActive) ElMessage.error((error as Error).message)
  } finally {
    if (request === tasksRequestId) { loading.value = false; scheduleTaskPoll() }
  }
}

function scheduleTaskPoll() {
  clearTimeout(taskPollTimer)
  if (viewActive && !executingTaskId.value && document.visibilityState === 'visible' && tasks.value.some(task => ['queued', 'running'].includes(task.status))) taskPollTimer = setTimeout(() => void loadTasks(true), 2400)
}
function taskVisibilityChanged() { if (document.visibilityState === 'visible') void loadTasks(true); else { clearTimeout(taskPollTimer); ++tasksRequestId; loading.value = false } }

function openStages(task: KnowledgeTask) {
  ++resultRequestId; ++inspectionRequestId
  loadingResultId.value = ''
  activeTask.value = task
  activeEvidence.value = []; activeArtifacts.value = []; activeInspection.value = null
  activeRunId.value = ''; reportRunId.value = ''; reportContent.value = ''; inspectionError.value = ''; inspectionLoading.value = false
  resultTab.value = 'stages'
  resultVisible.value = true
}
function inspectStage(runId: string) {
  activeRunId.value = runId
  resultTab.value = 'inspector'
  void loadRunInspection(runId)
}

function openCreate() {
  draft.knowledgeBaseId = filterBaseId.value || bases.value[0]?.id || ''
  draft.title = ''
  draft.description = ''
  draft.taskType = 'research'
  draft.deliverableType = 'report'
  draft.externalResearchEnabled = false
  draft.externalDomains = ''
  draft.externalRequestLimit = 6
  createVisible.value = true
}

async function createTask() {
  creating.value = true
  try {
    const externalDomains = draft.externalDomains
      .split(/[，,\n]/)
      .map(domain => domain.trim())
      .filter(Boolean)
    if (draft.externalResearchEnabled && externalDomains.length === 0) throw new Error('请填写至少一个允许访问的域名')
    const response = await createKnowledgeTask({
      ...draft,
      externalDomains,
      externalRequestLimit: draft.externalResearchEnabled ? draft.externalRequestLimit : 0,
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '创建失败')
    createVisible.value = false
    ElMessage.success('研究任务已创建')
    await loadTasks()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    creating.value = false
  }
}

function runOrOpen(task: KnowledgeTask) {
  if (task.status === 'running' || task.status === 'queued') {
    void cancelTask(task)
    return
  }
  if (task.status === 'completed' || (task.status === 'failed' && task.artifact_state === 'failed')) openResult(task)
  else if (task.status === 'failed') openFailedInspection(task)
  else runTask(task)
}

async function runTask(task: KnowledgeTask) {
  executingTaskId.value = task.id
  resultVisible.value = false
  try {
    const response = await executeKnowledgeTask(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务执行失败')
    const index = tasks.value.findIndex(item => item.id === task.id)
    if (index >= 0) tasks.value[index] = response.result
    ElMessage.info('任务已进入后台队列，可以离开此页面')
    const completed = await waitForTask(task.id)
    if (!completed) return
    if (completed.status === 'cancelled') {
      ElMessage.info('任务已取消')
      return
    }
    if (completed.status === 'failed') throw new Error(completed.result_summary || '任务执行失败')
    if (canFocusDocument(document) && !resultVisible.value) {
      const result = await getKnowledgeTaskResult(task.id)
      if (result.status !== 'success' || !result.result) throw new Error(result.error?.message || '报告加载失败')
      if (canFocusDocument(document) && !resultVisible.value) {
        showResult(result.result)
        ElMessage.success(result.result.artifacts.length ? '报告与演示文稿已生成' : '研究报告已生成')
      }
    }
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadTasks()
  } finally {
    executingTaskId.value = ''
    scheduleTaskPoll()
  }
}

async function waitForTask(taskId: string) {
  while (viewActive) {
    await new Promise(resolve => window.setTimeout(resolve, 1200))
    if (!viewActive) return null
    if (!canFocusDocument(document)) continue
    const response = await listKnowledgeTasks()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务状态读取失败')
    tasks.value = response.result.tasks.filter(task => !filterBaseId.value || task.knowledge_base_id === filterBaseId.value)
    const current = response.result.tasks.find(item => item.id === taskId)
    if (!current) throw new Error('任务已不存在')
    if (activeTask.value?.id === current.id) activeTask.value = current
    if (current.status === 'running') await refreshTaskActivity(taskId)
    if (['completed', 'failed', 'cancelled'].includes(current.status)) return current
  }
  return null
}

async function refreshTaskActivity(taskId: string) {
  const response = await getKnowledgeTaskActivity(taskId)
  if (response.status !== 'success' || !response.result) return
  const latest = [...response.result.events]
    .reverse()
    .find(event => event.message && event.event_type !== 'run.text_delta')
  if (latest) taskActivity.value[taskId] = latest.message
}

async function cancelTask(task: KnowledgeTask) {
  try {
    const response = await cancelKnowledgeTask(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '取消失败')
    const index = tasks.value.findIndex(item => item.id === task.id)
    if (index >= 0) tasks.value[index] = response.result
    if (activeTask.value?.id === task.id) activeTask.value = response.result
    ElMessage.info(response.result.status === 'cancelled' ? '任务已取消' : '已发送中断请求，Harness 正在停止')
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function openResult(task: KnowledgeTask) {
  const requestId = ++resultRequestId
  loadingResultId.value = task.id
  try {
    const response = await getKnowledgeTaskResult(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '报告加载失败')
    if (requestId === resultRequestId && viewActive && canFocusDocument(document)) showResult(response.result)
  } catch (error) {
    if (requestId === resultRequestId) ElMessage.error((error as Error).message)
  } finally {
    if (requestId === resultRequestId) loadingResultId.value = ''
  }
}

function showResult(result: KnowledgeTaskExecution) {
  activeTask.value = result.task
  activeEvidence.value = result.evidence
  activeArtifacts.value = result.artifacts
  activeRunId.value = result.run_id
  reportRunId.value = result.run_id
  reportContent.value = result.task.result_summary
  resultTab.value = 'report'
  resultVisible.value = true
  void loadRunInspection(result.run_id)
}

function inspectArtifact(artifact: KnowledgeArtifact) {
  if (!artifact.agent_run_id) return
  activeRunId.value = artifact.agent_run_id
  resultTab.value = 'inspector'
  void loadRunInspection(artifact.agent_run_id)
}

function artifactQuality(artifact: KnowledgeArtifact) {
  const details = artifact.validation_details
  return typeof details?.slide_count === 'number'
    ? details as PresentationQualityReport
    : null
}

function themeLabel(theme?: PresentationQualityReport['theme']) {
  return ({ editorial: '编辑', midnight: '深夜', sage: '鼠尾草' })[theme || 'editorial']
}

async function openFailedInspection(task: KnowledgeTask) {
  const requestId = ++resultRequestId
  loadingResultId.value = task.id
  try {
    const response = await getKnowledgeTaskActivity(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行记录加载失败')
    if (requestId !== resultRequestId || !viewActive || !canFocusDocument(document)) return
    activeTask.value = task
    reportContent.value = ''
    reportRunId.value = ''
    activeEvidence.value = []
    activeArtifacts.value = []
    activeRunId.value = response.result.run?.id || ''
    resultTab.value = 'inspector'
    resultVisible.value = true
    void loadRunInspection(activeRunId.value)
  } catch (error) {
    if (requestId === resultRequestId && viewActive) ElMessage.error((error as Error).message)
  } finally {
    if (requestId === resultRequestId) loadingResultId.value = ''
  }
}

async function loadRunInspection(runId: string) {
  const requestId = ++inspectionRequestId
  activeInspection.value = null
  inspectionError.value = ''
  inspectionLoading.value = Boolean(runId)
  if (!runId) {
    inspectionError.value = '该任务还没有可查看的运行记录'
    return
  }
  try {
    const response = await getAgentRunInspection(runId)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '检查器读取失败')
    if (requestId === inspectionRequestId) activeInspection.value = response.result
  } catch (error) {
    if (requestId === inspectionRequestId) inspectionError.value = (error as Error).message
  } finally {
    if (requestId === inspectionRequestId) inspectionLoading.value = false
  }
}

function openEvidence(sourceIndex: number) {
  const entry = activeEvidence.value[sourceIndex]
  if (!entry) return
  sourcePreviewEntry.value = entry
  sourcePreviewIndex.value = sourceIndex
  sourcePreviewVisible.value = true
}

function openTaskReview(task: KnowledgeTask) {
  resultVisible.value = false
  router.push({ path: '/knowledge/wiki', query: { base: task.knowledge_base_id } })
}

function taskIcon(type: KnowledgeTask['task_type']) { return type === 'refresh' ? Refresh : type === 'review' ? Select : DataAnalysis }
function typeLabel(type: KnowledgeTask['task_type']) { return ({ research: '专题研究', refresh: '知识刷新', review: '事实审核' })[type] }
function statusLabel(status: KnowledgeTask['status']) { return ({ draft: '草稿', queued: '排队中', running: '执行中', completed: '已完成', failed: '失败', cancelled: '已取消' })[status] }
function taskActionLabel(task: KnowledgeTask) {
  if (task.status === 'running' || task.status === 'queued') return '取消'
  if (loadingResultId.value === task.id) return '加载中'
  if (task.status === 'cancelled') return '重试'
  if (task.status === 'completed') return '查看'
  if (task.status === 'failed' && task.artifact_state === 'failed') return '查看报告'
  if (task.status === 'failed') return '检查'
  return '运行'
}
function formatBytes(bytes: number) { return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB` }
function formatDate(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }

onMounted(() => { viewActive = true; void loadData(); document.addEventListener('visibilitychange', taskVisibilityChanged) })
onBeforeUnmount(() => { viewActive = false; ++inspectionRequestId; ++resultRequestId; ++tasksRequestId; clearTimeout(taskPollTimer); document.removeEventListener('visibilitychange', taskVisibilityChanged) })
</script>

<style scoped>
.task-filter { margin-bottom: 12px; }
.task-summary { color: var(--text-faint); font-size: 12px; }
.task-card-actions { min-width: 0; display: grid; gap: 8px; }
.task-stage-link { min-height: 34px; margin-top: 6px; padding: 3px 9px; border: 0; border-radius: 9px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.research-task-list { display: grid; gap: 9px; }
.research-task { display: grid; grid-template-columns: 52px minmax(0, 1fr) minmax(96px, auto); align-items: center; gap: 15px; padding: 16px 17px; animation: task-in var(--motion-normal) var(--ease-spring-gentle) both; animation-delay: calc(var(--order) * 30ms); }
.task-kind { width: 52px; height: 52px; display: grid; place-items: center; border-radius: 16px; background: var(--accent-light); color: var(--accent); font-size: 22px; }
.task-kind.is-refresh { background: color-mix(in srgb, #32ade6 13%, transparent); color: #1685b8; }
.task-kind.is-review { background: color-mix(in srgb, #34c759 13%, transparent); color: #248a3d; }
.task-main { min-width: 0; }
.task-topline { display: flex; align-items: center; flex-wrap: wrap; gap: 5px 9px; color: var(--accent); font-size: 10px; font-weight: 650; }
.task-topline > span:first-child { min-width: 0; overflow-wrap: anywhere; }
.task-main h2 { margin: 5px 0 4px; overflow-wrap: anywhere; font-size: 16px; }
.task-main > p { display: -webkit-box; overflow: hidden; overflow-wrap: anywhere; color: var(--text-muted); font-size: 12px; line-height: 1.55; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.task-main > p.task-result-preview { margin-top: 6px; color: var(--text-faint); font-size: 11px; }
.task-live { display: flex; align-items: center; gap: 7px; margin-top: 7px; color: var(--accent); font-size: 11px; font-weight: 620; }
.task-live span { min-width: 0; overflow-wrap: anywhere; }
.task-live i { width: 6px; height: 6px; flex: none; border-radius: 50%; background: currentColor; box-shadow: 0 0 0 0 color-mix(in srgb, currentColor 24%, transparent); animation: task-live-pulse 1.4s ease-out infinite; }
.task-main footer { display: flex; flex-wrap: wrap; gap: 4px 12px; margin-top: 9px; color: var(--text-faint); font-size: 10px; }
.task-main footer time { margin-left: auto; white-space: nowrap; }
.task-action { min-height: 36px; display: flex; align-items: center; justify-content: center; gap: 5px; padding: 0 10px; border: 1px solid var(--accent-border); border-radius: 11px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.task-action:disabled { opacity: .48; cursor: default; }
.task-action.is-cancel { border-color: color-mix(in srgb, var(--danger, #ff3b30) 25%, transparent); background: color-mix(in srgb, var(--danger, #ff3b30) 9%, transparent); color: var(--danger, #d9342b); }
.task-empty-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.mobile-create-task { display: none; }
.external-grant { display: grid; gap: 10px; padding: 12px; border: 1px solid var(--border-faint); border-radius: 14px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); transition: border-color var(--motion-fast) var(--ease-emphasized), background var(--motion-fast) var(--ease-emphasized); }
.external-grant.is-enabled { border-color: var(--accent-border); background: var(--accent-light); }
.external-grant-head { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.external-grant-head > div { display: grid; gap: 2px; }
.external-grant-head strong { color: var(--text-primary); font-size: 12px; }
.external-grant-head span, .external-grant > p { color: var(--text-faint); font-size: 10px; line-height: 1.55; }
.external-limit-row { display: flex; align-items: center; gap: 9px; color: var(--text-muted); font-size: 11px; }
.external-limit-row :deep(.el-input-number) { width: 116px; }
.task-result-modal { width: 100%; height: min(760px, calc(100dvh - 48px)); }
.task-result-modal .knowledge-modal-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.task-result-heading { min-width: 0; }
.task-result-heading > span { color: var(--accent); font-size: 10px; font-weight: 680; }
.task-result-heading h3 { margin-top: 5px; overflow-wrap: anywhere; }
.task-result-tabs { flex: none; display: flex; gap: 5px; margin: 0 24px 12px; padding: 4px; border-radius: 12px; background: color-mix(in srgb, var(--text-primary) 5%, transparent); }
.task-result-tabs button { min-height: 38px; flex: 1; display: flex; align-items: center; justify-content: center; gap: 6px; border: 0; border-radius: 9px; background: transparent; color: var(--text-muted); font: inherit; font-size: 12px; font-weight: 650; cursor: pointer; }
.task-result-tabs button.is-active { background: var(--bg-glass-strong); color: var(--accent); box-shadow: var(--shadow-sm); }
.task-result-scroll { min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 12px; flex: 1 1 auto; overflow-x: hidden; overflow-y: auto; padding: 0 24px 24px; overscroll-behavior: contain; }
.task-result-scroll > * { min-width: 0; flex: 0 0 auto; }
.task-inspection-state { min-height: 140px; display: flex; align-items: center; justify-content: center; flex-wrap: wrap; gap: 10px; color: var(--text-muted); font-size: 12px; text-align: center; }
.task-inspection-state.is-error { color: var(--danger, #d9342b); }
.task-result-content { min-width: 0; }
.run-budget-note { margin: 6px 0 0; color: var(--text-muted); font-size: 11px; line-height: 1.7; overflow-wrap: anywhere; }
.run-budget-note.is-incomplete { color: var(--danger, #d9342b); }
.task-result-content :deep(.knowledge-answer-markdown) { border-radius: 14px; }
.artifact-failure { display: grid; gap: 3px; padding: 11px 12px; border: 1px solid color-mix(in srgb, var(--danger, #d9342b) 24%, transparent); border-radius: 12px; background: color-mix(in srgb, var(--danger, #d9342b) 8%, transparent); }
.artifact-failure strong { color: var(--danger, #d9342b); font-size: 11px; }
.artifact-failure span { color: var(--text-muted); font-size: 9px; line-height: 1.5; }
.task-artifacts { display: grid; gap: 8px; }
.artifact-card { overflow: hidden; border: 1px solid var(--accent-border); border-radius: 14px; background: var(--accent-light); }
.artifact-main-row { display: grid; grid-template-columns: 38px minmax(0, 1fr) auto auto; align-items: center; gap: 10px; padding: 10px 12px; }
.artifact-icon { width: 38px; height: 38px; display: grid; place-items: center; border-radius: 11px; background: var(--accent); color: white; font-size: 18px; }
.artifact-copy { min-width: 0; display: grid; gap: 3px; }
.artifact-copy b, .artifact-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.artifact-copy b { font-size: 12px; }
.artifact-copy small { color: var(--text-faint); font-size: 9px; }
.artifact-main-row > button, .artifact-main-row > a { min-height: 32px; display: inline-flex; align-items: center; justify-content: center; gap: 4px; padding: 0 9px; border: 0; border-radius: 9px; background: color-mix(in srgb, var(--bg-base) 48%, transparent); color: var(--accent); font: inherit; font-size: 10px; font-weight: 700; text-decoration: none; cursor: pointer; }
.artifact-main-row > a { background: var(--accent); color: white; }
.artifact-quality { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 1px; border-top: 1px solid var(--accent-border); background: var(--accent-border); }
.artifact-quality span { min-width: 0; display: grid; gap: 2px; padding: 9px 11px; background: color-mix(in srgb, var(--bg-base) 74%, transparent); }
.artifact-quality b { overflow: hidden; color: var(--text-primary); font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.artifact-quality small { color: var(--text-faint); font-size: 8px; }
.artifact-checks { border-top: 1px solid var(--accent-border); }
.artifact-checks summary { padding: 8px 12px; color: var(--accent); font-size: 9px; font-weight: 700; cursor: pointer; list-style: none; }
.artifact-checks summary::-webkit-details-marker { display: none; }
.artifact-checks ul { display: grid; gap: 7px; margin: 0; padding: 2px 12px 11px; list-style: none; }
.artifact-checks li { display: grid; grid-template-columns: 7px minmax(0, 1fr); gap: 7px; align-items: start; }
.artifact-checks i { width: 7px; height: 7px; margin-top: 4px; border-radius: 50%; background: var(--danger, #d9342b); }
.artifact-checks i.is-pass { background: var(--success, #34c759); }
.artifact-checks li span { min-width: 0; display: grid; gap: 1px; }
.artifact-checks li b { color: var(--text-secondary); font-size: 9px; }
.artifact-checks li small { overflow-wrap: anywhere; color: var(--text-faint); font-size: 8px; line-height: 1.45; }
.review-result-link { width: 100%; padding: 10px 12px; border: 0; border-radius: 11px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.task-result-evidence { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
.task-result-evidence button { min-width: 0; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 2px 8px; align-items: center; padding: 10px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; }
.task-result-evidence b { grid-row: 1 / 3; padding: 3px 6px; border-radius: 6px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.task-result-evidence span, .task-result-evidence small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-result-evidence span { font-size: 11px; font-weight: 650; }
.task-result-evidence small { color: var(--text-faint); font-size: 9px; }
.run-inspector { overflow: hidden; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.run-inspector-title { min-height: 46px; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 4px 12px; padding: 8px 13px; color: var(--text-primary); }
.run-inspector-title > span { display: flex; align-items: center; gap: 7px; color: var(--accent); font-size: 12px; }
.run-inspector-title small { overflow-wrap: anywhere; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; }
.effective-prompt > summary::-webkit-details-marker { display: none; }
.run-inspector-body { display: grid; gap: 13px; padding: 13px; border-top: 1px solid var(--border-faint); }
.run-inspector-metrics { display: grid; grid-template-columns: 1.4fr repeat(3, 1fr); gap: 7px; }
.run-inspector-metrics > span { min-width: 0; display: grid; gap: 3px; padding: 9px; border-radius: 10px; background: color-mix(in srgb, var(--bg-base) 44%, transparent); }
.run-inspector-metrics small, .run-inspector-section h4 { color: var(--text-faint); font-size: 10px; font-weight: 650; letter-spacing: .05em; text-transform: uppercase; }
.run-inspector-metrics strong, .run-inspector-metrics code { overflow: hidden; color: var(--text-primary); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.run-inspector-section { display: grid; gap: 7px; }
.run-inspector-section h4 { margin: 0; }
.run-skill-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; }
.run-skill-list > span { min-width: 0; display: grid; gap: 2px; padding: 8px 10px; border: 1px solid var(--accent-border); border-radius: 10px; background: var(--accent-light); }
.run-skill-list b { overflow: hidden; color: var(--text-primary); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.run-skill-list small { overflow-wrap: anywhere; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; }
.run-config-list { display: grid; gap: 5px; }
.run-config-list details { overflow: hidden; border: 1px solid var(--border-faint); border-radius: 9px; background: color-mix(in srgb, var(--bg-base) 38%, transparent); }
.run-config-list summary { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 8px 10px; cursor: pointer; list-style: none; }
.run-config-list summary::-webkit-details-marker { display: none; }
.run-config-list summary > span { min-width: 0; display: grid; gap: 2px; }
.run-config-list summary b { overflow: hidden; color: var(--text-primary); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.run-config-list summary small, .run-config-list summary em { color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; font-style: normal; }
.run-config-list pre { max-height: 220px; margin: 0; overflow: auto; padding: 11px; border-top: 1px solid var(--border-faint); color: var(--text-secondary); font-family: var(--font-mono); font-size: 9px; line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
.run-tool-list { display: flex; flex-wrap: wrap; gap: 5px; }
.run-tool-list code, .run-tool-list span { padding: 4px 7px; border-radius: 7px; background: color-mix(in srgb, var(--text-primary) 5%, transparent); color: var(--text-muted); font-size: 8px; }
.run-evidence-list { display: grid; gap: 5px; }
.run-evidence-list > div { min-width: 0; display: grid; gap: 3px; padding: 8px 10px; border: 1px solid var(--border-faint); border-radius: 9px; background: color-mix(in srgb, var(--bg-base) 38%, transparent); }
.run-evidence-list strong { overflow-wrap: anywhere; color: var(--text-secondary); font-size: 10px; font-weight: 650; }
.run-evidence-list small { overflow-wrap: anywhere; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; }
.run-event-list { max-height: 210px; display: grid; gap: 0; overflow: auto; margin: 0; padding: 0; list-style: none; }
.run-event-list li { min-height: 38px; display: grid; grid-template-columns: 12px minmax(0, 1fr); gap: 7px; align-items: start; }
.run-event-list li > i { width: 7px; height: 7px; margin-top: 5px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.run-event-list li > div { display: grid; gap: 2px; padding: 0 0 9px 8px; border-left: 1px solid var(--border-faint); }
.run-event-list strong { color: var(--text-secondary); font-size: 10px; font-weight: 600; }
.run-event-list small { color: var(--text-faint); font-size: 10px; }
.effective-prompt { overflow: hidden; border: 1px solid var(--border-faint); border-radius: 11px; }
.effective-prompt > summary { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 11px; color: var(--text-secondary); font-size: 10px; cursor: pointer; list-style: none; }
.effective-prompt > summary span { max-width: 66%; overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.effective-prompt pre { max-height: 360px; margin: 0; overflow: auto; padding: 13px; border-top: 1px solid var(--border-faint); background: color-mix(in srgb, var(--bg-base) 55%, transparent); color: var(--text-secondary); font-family: var(--font-mono); font-size: 9px; line-height: 1.65; white-space: pre-wrap; word-break: break-word; }
.run-inspector-legacy { color: var(--text-faint); font-size: 9px; }
@keyframes task-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@keyframes task-live-pulse { 60%, 100% { box-shadow: 0 0 0 7px transparent; } }
@media (min-width: 769px) and (max-width: 1100px) {
  .research-task { grid-template-columns: 44px minmax(0, 1fr) 42px; gap: 10px; }
  .task-kind { width: 44px; height: 44px; }
  .task-action { width: 42px; min-height: 42px; padding: 0; }
  .task-action span { display: none; }
}
@media (max-width: 768px) {
  .task-stage-link { min-height: 44px; }
  .task-filter { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .mobile-create-task { width: 46px; min-height: 46px; display: grid; place-items: center; border: 0; border-radius: 14px; background: var(--accent); color: white; font-size: 18px; box-shadow: 0 7px 18px color-mix(in srgb, var(--accent) 22%, transparent); }
  .task-summary { grid-column: 1 / -1; }
  .research-task { grid-template-columns: minmax(0, 1fr); gap: 12px; padding: 14px; }
  .task-kind { display: none; }
  .task-main { grid-column: 1 / -1; }
  .task-card-actions { grid-column: 1 / -1; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); padding-top: 10px; border-top: 1px solid var(--border-faint); }
  .task-stage-link { margin: 0; font-size: 13px; }
  .task-action { width: 100%; min-height: 44px; padding: 8px; border-radius: 11px; font-size: 13px; }
  .task-action span { display: inline; }
  .task-main > p { font-size: 14px; }
  .task-main h2 { font-size: 17px; line-height: 1.45; }
  .task-topline { justify-content: space-between; font-size: 12px; }
  .task-topline > span:first-child { flex: 1; }
  .task-summary { font-size: 11px; }
  .task-main > p { white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
  .task-result-evidence { grid-template-columns: 1fr; }
  .task-result-modal { height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); }
  .task-result-tabs { margin: 0 12px 10px; gap: 2px; }
  .task-result-tabs button { min-width: 0; min-height: 44px; gap: 3px; padding: 4px 2px; font-size: 11px; }
  .task-result-modal .knowledge-modal-head { flex-wrap: wrap; gap: 8px; }
  .task-result-heading { flex: 1 1 100%; }
  .task-result-scroll { padding: 0 16px max(24px, env(safe-area-inset-bottom)); -webkit-overflow-scrolling: touch; }
  .artifact-main-row { grid-template-columns: 38px minmax(0, 1fr) auto; }
  .artifact-main-row > button { grid-column: 2; justify-self: start; }
  .artifact-main-row > a { grid-column: 3; grid-row: 1 / 3; min-height: 38px; }
  .artifact-quality { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .run-inspector-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .run-skill-list { grid-template-columns: 1fr; }
  .run-inspector-metrics strong, .run-inspector-metrics code { white-space: normal; overflow-wrap: anywhere; }
  .task-result-modal .knowledge-modal-actions { padding-top: 8px; }
}
@media (prefers-reduced-motion: reduce) { .research-task, .task-live i { animation: none; } }
</style>
