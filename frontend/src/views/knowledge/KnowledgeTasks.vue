<template>
  <KnowledgePageShell title="研究任务" subtitle="让 Agent 围绕一本书持续研究、刷新与审核知识">
    <template #actions>
      <el-button type="primary" :disabled="!bases.length" @click="openCreate"><el-icon><Plus /></el-icon>新建任务</el-button>
    </template>

    <div class="task-filter knowledge-toolbar">
      <el-select v-model="filterBaseId" class="knowledge-select is-responsive" popper-class="system-select-popper system-toolbar-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="全部知识库" clearable @change="loadTasks">
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
          <footer><span>{{ typeLabel(task.task_type) }}</span><span>{{ task.deliverable_type === 'presentation' ? 'PPTX 演示文稿' : '研究报告' }}</span><span v-if="task.knowledge_change_state === 'proposed'">待知识审核</span><time>{{ formatDate(task.updated_at) }}</time></footer>
        </div>
        <button class="task-action" type="button" :class="{ 'is-cancel': task.status === 'running' || task.status === 'queued' }" :aria-label="taskActionLabel(task)" :disabled="(Boolean(executingTaskId) && executingTaskId !== task.id) || Boolean(loadingResultId)" @click="runOrOpen(task)">
          <el-icon :class="{ 'is-loading': task.status === 'running' || executingTaskId === task.id || loadingResultId === task.id }">
            <Loading v-if="task.status === 'running' || executingTaskId === task.id || loadingResultId === task.id" />
            <View v-else-if="task.status === 'completed'" />
            <VideoPlay v-else />
          </el-icon>
          <span>{{ taskActionLabel(task) }}</span>
        </button>
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
        </div>
        <div class="knowledge-modal-actions">
          <el-button @click="createVisible = false">取消</el-button>
          <el-button type="primary" :loading="creating" :disabled="!draft.knowledgeBaseId || !draft.title.trim()" @click="createTask">创建任务</el-button>
        </div>
      </div>
    </MotionModal>

    <MotionModal v-model="resultVisible" aria-label="研究任务结果">
      <div v-if="activeTask" class="knowledge-modal-card task-result-modal">
        <div class="knowledge-modal-head">
          <div class="task-result-heading"><span>{{ activeTask.book_name }} · {{ typeLabel(activeTask.task_type) }}</span><h3>{{ activeTask.title }}</h3></div>
          <span class="knowledge-status" :class="`is-${activeTask.status}`">{{ statusLabel(activeTask.status) }}</span>
        </div>
        <div class="task-result-content">
          <KnowledgeAnswerMarkdown :content="activeTask.result_summary" :evidence-count="activeEvidence.length" @citation="openEvidence" />
        </div>
        <div v-if="activeArtifacts.length" class="task-artifacts">
          <a v-for="artifact in activeArtifacts" :key="artifact.id" :href="knowledgeArtifactDownloadUrl(artifact.id)" download>
            <span class="artifact-icon"><el-icon><Download /></el-icon></span>
            <span><b>{{ artifact.title }}</b><small>{{ formatBytes(artifact.size_bytes) }} · {{ artifact.validation_message }}</small></span>
            <em>下载</em>
          </a>
        </div>
        <button v-if="activeTask.knowledge_change_state === 'proposed'" class="review-result-link" type="button" @click="openTaskReview(activeTask)">研究结论已作为候选保存，前往 Wiki 工作台审核</button>
        <div v-if="activeEvidence.length" class="task-result-evidence">
          <button v-for="(entry, index) in activeEvidence" :key="entry.id" type="button" @click="openEvidence(index)">
            <b>S{{ index + 1 }}</b><span>{{ entry.title }}</span><small>{{ entry.source_path || '数据库实体' }}</small>
          </button>
        </div>
        <div class="knowledge-modal-actions">
          <el-button v-if="activeTask.status === 'failed'" :loading="executingTaskId === activeTask.id" @click="runTask(activeTask)">重新运行</el-button>
          <el-button type="primary" @click="resultVisible = false">完成</el-button>
        </div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { DataAnalysis, Download, Loading, Operation, Plus, Refresh, Select, VideoPlay, View } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  createKnowledgeTask,
  cancelKnowledgeTask,
  executeKnowledgeTask,
  getKnowledgeTaskResult,
  getKnowledgeTaskActivity,
  listBookKnowledgeBases,
  listKnowledgeTasks,
  knowledgeArtifactDownloadUrl,
  type KnowledgeArtifact,
  type KnowledgeBaseSummary,
  type KnowledgeEntrySummary,
  type KnowledgeTask,
} from '@/api/knowledge'

const router = useRouter()
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
const taskActivity = ref<Record<string, string>>({})
const draft = reactive({ knowledgeBaseId: '', title: '', description: '', taskType: 'research' as KnowledgeTask['task_type'], deliverableType: 'report' as KnowledgeTask['deliverable_type'] })
let viewActive = true

async function loadData() {
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    bases.value = response.result.items.flatMap(card => (
      card.book.kind === 'folder' && card.knowledge_base ? [card.knowledge_base] : []
    ))
    await loadTasks()
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function loadTasks() {
  loading.value = true
  try {
    const response = await listKnowledgeTasks(filterBaseId.value || undefined)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务加载失败')
    tasks.value = response.result.tasks
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loading.value = false
  }
}

function openCreate() {
  draft.knowledgeBaseId = filterBaseId.value || bases.value[0]?.id || ''
  draft.title = ''
  draft.description = ''
  draft.taskType = 'research'
  draft.deliverableType = 'report'
  createVisible.value = true
}

async function createTask() {
  creating.value = true
  try {
    const response = await createKnowledgeTask(draft)
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
  if (task.status === 'completed') openResult(task)
  else runTask(task)
}

async function runTask(task: KnowledgeTask) {
  executingTaskId.value = task.id
  const index = tasks.value.findIndex(item => item.id === task.id)
  try {
    const response = await executeKnowledgeTask(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务执行失败')
    if (index >= 0) tasks.value[index] = response.result
    ElMessage.info('任务已进入后台队列，可以离开此页面')
    const completed = await waitForTask(task.id)
    if (!completed) return
    if (completed.status === 'cancelled') {
      ElMessage.info('任务已取消')
      return
    }
    if (completed.status === 'failed') throw new Error(completed.result_summary || '任务执行失败')
    const result = await getKnowledgeTaskResult(task.id)
    if (result.status !== 'success' || !result.result) throw new Error(result.error?.message || '报告加载失败')
    activeTask.value = result.result.task
    activeEvidence.value = result.result.evidence
    activeArtifacts.value = result.result.artifacts
    resultVisible.value = true
    ElMessage.success(result.result.artifacts.length ? '报告与演示文稿已生成' : '研究报告已生成')
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadTasks()
  } finally {
    executingTaskId.value = ''
  }
}

async function waitForTask(taskId: string) {
  while (viewActive) {
    await new Promise(resolve => window.setTimeout(resolve, 1200))
    if (!viewActive) return null
    const response = await listKnowledgeTasks(filterBaseId.value || undefined)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务状态读取失败')
    tasks.value = response.result.tasks
    const current = tasks.value.find(item => item.id === taskId)
    if (!current) throw new Error('任务已不存在')
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
    ElMessage.info(response.result.status === 'cancelled' ? '任务已取消' : '已发送中断请求，Harness 正在停止')
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function openResult(task: KnowledgeTask) {
  loadingResultId.value = task.id
  try {
    const response = await getKnowledgeTaskResult(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '报告加载失败')
    activeTask.value = response.result.task
    activeEvidence.value = response.result.evidence
    activeArtifacts.value = response.result.artifacts
    resultVisible.value = true
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loadingResultId.value = ''
  }
}

function openEvidence(sourceIndex: number) {
  const entry = activeEvidence.value[sourceIndex]
  if (!entry) return
  resultVisible.value = false
  router.push({ path: '/knowledge/wiki', query: { base: entry.knowledge_base_id, entry: entry.id } })
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
  if (task.status === 'failed') return '重试'
  return '运行'
}
function formatBytes(bytes: number) { return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB` }
function formatDate(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }

onMounted(() => { viewActive = true; void loadData() })
onBeforeUnmount(() => { viewActive = false })
</script>

<style scoped>
.task-filter { margin-bottom: 12px; }
.task-summary { color: var(--text-faint); font-size: 12px; }
.research-task-list { display: grid; gap: 9px; }
.research-task { display: grid; grid-template-columns: 52px minmax(0, 1fr) 76px; align-items: center; gap: 15px; padding: 16px 17px; animation: task-in var(--motion-normal) var(--ease-spring-gentle) both; animation-delay: calc(var(--order) * 30ms); }
.task-kind { width: 52px; height: 52px; display: grid; place-items: center; border-radius: 16px; background: var(--accent-light); color: var(--accent); font-size: 22px; }
.task-kind.is-refresh { background: color-mix(in srgb, #32ade6 13%, transparent); color: #1685b8; }
.task-kind.is-review { background: color-mix(in srgb, #34c759 13%, transparent); color: #248a3d; }
.task-main { min-width: 0; }
.task-topline { display: flex; align-items: center; gap: 9px; color: var(--accent); font-size: 10px; font-weight: 650; }
.task-main h2 { margin: 5px 0 4px; font-size: 16px; }
.task-main > p { overflow: hidden; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.task-main > p.task-result-preview { margin-top: 6px; color: var(--text-faint); font-size: 11px; }
.task-live { display: flex; align-items: center; gap: 7px; margin-top: 7px; color: var(--accent); font-size: 11px; font-weight: 620; }
.task-live i { width: 6px; height: 6px; flex: none; border-radius: 50%; background: currentColor; box-shadow: 0 0 0 0 color-mix(in srgb, currentColor 24%, transparent); animation: task-live-pulse 1.4s ease-out infinite; }
.task-main footer { display: flex; gap: 12px; margin-top: 9px; color: var(--text-faint); font-size: 10px; }
.task-action { min-height: 36px; display: flex; align-items: center; justify-content: center; gap: 5px; padding: 0 10px; border: 1px solid var(--accent-border); border-radius: 11px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.task-action:disabled { opacity: .48; cursor: default; }
.task-action.is-cancel { border-color: color-mix(in srgb, var(--danger, #ff3b30) 25%, transparent); background: color-mix(in srgb, var(--danger, #ff3b30) 9%, transparent); color: var(--danger, #d9342b); }
.task-empty-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.mobile-create-task { display: none; }
.task-result-modal { width: min(720px, calc(100vw - 28px)); }
.task-result-modal .knowledge-modal-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.task-result-heading > span { color: var(--accent); font-size: 10px; font-weight: 680; }
.task-result-heading h3 { margin-top: 5px; }
.task-result-content { max-height: min(48vh, 430px); overflow: auto; }
.task-result-content :deep(.knowledge-answer-markdown) { border-radius: 14px; }
.task-artifacts { display: grid; gap: 7px; }
.task-artifacts a { display: grid; grid-template-columns: 38px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 10px 12px; border: 1px solid var(--accent-border); border-radius: 13px; background: var(--accent-light); color: var(--text-primary); text-decoration: none; }
.artifact-icon { width: 38px; height: 38px; display: grid; place-items: center; border-radius: 11px; background: var(--accent); color: white; font-size: 18px; }
.task-artifacts a > span:nth-child(2) { min-width: 0; display: grid; gap: 3px; }
.task-artifacts b, .task-artifacts small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-artifacts b { font-size: 12px; }
.task-artifacts small { color: var(--text-faint); font-size: 9px; }
.task-artifacts em { color: var(--accent); font-size: 11px; font-style: normal; font-weight: 700; }
.review-result-link { width: 100%; padding: 10px 12px; border: 0; border-radius: 11px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.task-result-evidence { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
.task-result-evidence button { min-width: 0; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 2px 8px; align-items: center; padding: 10px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; }
.task-result-evidence b { grid-row: 1 / 3; padding: 3px 6px; border-radius: 6px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.task-result-evidence span, .task-result-evidence small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-result-evidence span { font-size: 11px; font-weight: 650; }
.task-result-evidence small { color: var(--text-faint); font-size: 9px; }
@keyframes task-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@keyframes task-live-pulse { 60%, 100% { box-shadow: 0 0 0 7px transparent; } }
@media (max-width: 768px) {
  .task-filter { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .mobile-create-task { width: 46px; min-height: 46px; display: grid; place-items: center; border: 0; border-radius: 14px; background: var(--accent); color: white; font-size: 18px; box-shadow: 0 7px 18px color-mix(in srgb, var(--accent) 22%, transparent); }
  .task-summary { grid-column: 1 / -1; }
  .research-task { grid-template-columns: 44px minmax(0, 1fr) 42px; gap: 10px; padding: 14px; }
  .task-kind { width: 44px; height: 44px; border-radius: 14px; }
  .task-action { width: 42px; min-height: 42px; padding: 0; border-radius: 13px; }
  .task-action span { display: none; }
  .task-main > p { white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
  .task-result-evidence { grid-template-columns: 1fr; }
}
</style>
