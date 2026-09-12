<template>
  <KnowledgePageShell title="研究任务" subtitle="让 Agent 围绕一本书持续研究、刷新与审核知识">
    <template #actions>
      <el-button type="primary" :disabled="!bases.length" @click="openCreate"><el-icon><Plus /></el-icon>新建任务</el-button>
    </template>

    <div class="task-filter knowledge-toolbar">
      <el-select v-model="filterBaseId" placeholder="全部知识库" clearable @change="loadTasks">
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
          <p v-if="task.result_summary" class="task-result-preview">{{ task.result_summary }}</p>
          <footer><span>{{ typeLabel(task.task_type) }}</span><time>{{ formatDate(task.updated_at) }}</time></footer>
        </div>
        <button class="task-action" type="button" :aria-label="taskActionLabel(task)" :disabled="task.status === 'running' || task.status === 'queued' || task.status === 'cancelled' || Boolean(executingTaskId) || Boolean(loadingResultId)" @click="runOrOpen(task)">
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
          <el-select v-model="draft.knowledgeBaseId" placeholder="选择知识库">
            <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
          </el-select>
          <el-input v-model="draft.title" :maxlength="200" show-word-limit placeholder="要研究或核实的问题" />
          <el-input v-model="draft.description" type="textarea" :rows="4" :maxlength="4000" show-word-limit placeholder="补充目标、范围和期望结果" />
          <el-select v-model="draft.taskType">
            <el-option label="专题研究" value="research" />
            <el-option label="知识刷新" value="refresh" />
            <el-option label="事实审核" value="review" />
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
          <template v-for="(segment, index) in activeResultSegments" :key="index">
            <button v-if="segment.sourceIndex !== undefined" type="button" @click="openEvidence(segment.sourceIndex)">{{ segment.text }}</button>
            <span v-else>{{ segment.text }}</span>
          </template>
        </div>
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
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { DataAnalysis, Loading, Operation, Plus, Refresh, Select, VideoPlay, View } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  createKnowledgeTask,
  executeKnowledgeTask,
  getKnowledgeTaskResult,
  listBookKnowledgeBases,
  listKnowledgeTasks,
  type KnowledgeBaseSummary,
  type KnowledgeEntrySummary,
  type KnowledgeTask,
} from '@/api/knowledge'
import { parseKnowledgeCitations } from '@/utils/knowledgeCitations'

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
const draft = reactive({ knowledgeBaseId: '', title: '', description: '', taskType: 'research' as KnowledgeTask['task_type'] })
const activeResultSegments = computed(() => parseKnowledgeCitations(activeTask.value?.result_summary || '', activeEvidence.value.length))

async function loadData() {
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    bases.value = response.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
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
  if (task.status === 'completed') openResult(task)
  else runTask(task)
}

async function runTask(task: KnowledgeTask) {
  executingTaskId.value = task.id
  try {
    const response = await executeKnowledgeTask(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '任务执行失败')
    const index = tasks.value.findIndex(item => item.id === task.id)
    if (index >= 0) tasks.value[index] = response.result.task
    activeTask.value = response.result.task
    activeEvidence.value = response.result.evidence
    resultVisible.value = true
    ElMessage.success('研究报告已生成')
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadTasks()
  } finally {
    executingTaskId.value = ''
  }
}

async function openResult(task: KnowledgeTask) {
  loadingResultId.value = task.id
  try {
    const response = await getKnowledgeTaskResult(task.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '报告加载失败')
    activeTask.value = response.result.task
    activeEvidence.value = response.result.evidence
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

function taskIcon(type: KnowledgeTask['task_type']) { return type === 'refresh' ? Refresh : type === 'review' ? Select : DataAnalysis }
function typeLabel(type: KnowledgeTask['task_type']) { return ({ research: '专题研究', refresh: '知识刷新', review: '事实审核' })[type] }
function statusLabel(status: KnowledgeTask['status']) { return ({ draft: '草稿', queued: '排队中', running: '执行中', completed: '已完成', failed: '失败', cancelled: '已取消' })[status] }
function taskActionLabel(task: KnowledgeTask) {
  if (task.status === 'running' || executingTaskId.value === task.id) return '执行中'
  if (loadingResultId.value === task.id) return '加载中'
  if (task.status === 'queued') return '等待执行'
  if (task.status === 'cancelled') return '已取消'
  if (task.status === 'completed') return '查看'
  if (task.status === 'failed') return '重试'
  return '运行'
}
function formatDate(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }

onMounted(loadData)
</script>

<style scoped>
.task-filter { margin-bottom: 12px; }
.task-filter :deep(.el-select) { width: 230px; }
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
.task-main footer { display: flex; gap: 12px; margin-top: 9px; color: var(--text-faint); font-size: 10px; }
.task-action { min-height: 36px; display: flex; align-items: center; justify-content: center; gap: 5px; padding: 0 10px; border: 1px solid var(--accent-border); border-radius: 11px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 650; cursor: pointer; }
.task-action:disabled { opacity: .48; cursor: default; }
.task-empty-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.mobile-create-task { display: none; }
.task-result-modal { width: min(720px, calc(100vw - 28px)); }
.task-result-modal .knowledge-modal-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.task-result-heading > span { color: var(--accent); font-size: 10px; font-weight: 680; }
.task-result-heading h3 { margin-top: 5px; }
.task-result-content { max-height: min(48vh, 430px); overflow: auto; padding: 16px; border-radius: 14px; background: var(--bg-glass-subtle); color: var(--text-secondary); font-size: 13px; line-height: 1.75; white-space: pre-wrap; }
.task-result-content button { display: inline-flex; margin: 0 2px; padding: 1px 5px; border: 0; border-radius: 6px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 720; cursor: pointer; }
.task-result-evidence { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; }
.task-result-evidence button { min-width: 0; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 2px 8px; align-items: center; padding: 10px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; }
.task-result-evidence b { grid-row: 1 / 3; padding: 3px 6px; border-radius: 6px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.task-result-evidence span, .task-result-evidence small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.task-result-evidence span { font-size: 11px; font-weight: 650; }
.task-result-evidence small { color: var(--text-faint); font-size: 9px; }
@keyframes task-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@media (max-width: 768px) {
  .task-filter { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .task-filter :deep(.el-select) { width: 100%; }
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
