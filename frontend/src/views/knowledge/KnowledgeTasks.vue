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
      <div class="task-summary">{{ tasks.length }} 项任务 · 当前体验版先保存为数据库草稿</div>
    </div>

    <div v-if="loading" class="knowledge-empty knowledge-surface"><el-icon class="is-loading"><Loading /></el-icon>正在加载任务…</div>
    <section v-else-if="tasks.length" class="research-task-list">
      <article v-for="(task, index) in tasks" :key="task.id" class="research-task knowledge-surface" :style="{ '--order': index }">
        <div class="task-kind" :class="`is-${task.task_type}`"><el-icon><component :is="taskIcon(task.task_type)" /></el-icon></div>
        <div class="task-main">
          <div class="task-topline"><span>{{ task.book_name }}</span><span class="knowledge-status" :class="`is-${task.status}`">{{ statusLabel(task.status) }}</span></div>
          <h2>{{ task.title }}</h2>
          <p>{{ task.description || '没有补充任务说明' }}</p>
          <footer><span>{{ typeLabel(task.task_type) }}</span><time>{{ formatDate(task.updated_at) }}</time></footer>
        </div>
        <button class="task-more" aria-label="任务执行尚未启用" disabled><el-icon><MoreFilled /></el-icon></button>
      </article>
    </section>
    <div v-else class="knowledge-empty knowledge-surface">
      <div class="task-empty-symbol"><el-icon><Operation /></el-icon></div>
      <strong>还没有研究任务</strong>
      <span>创建的问题会先安全保存为草稿；Harness 接通后可继续执行并记录完整过程。</span>
      <el-button type="primary" :disabled="!bases.length" @click="openCreate">创建第一项任务</el-button>
    </div>

    <MotionModal v-model="createVisible" aria-label="新建研究任务">
      <div class="knowledge-modal-card">
        <div class="knowledge-modal-head"><h3>新建研究任务</h3><p>任务严格绑定一本书，不会跨库读取。</p></div>
        <div class="knowledge-modal-body">
          <el-select v-model="draft.knowledgeBaseId" placeholder="选择知识库">
            <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
          </el-select>
          <el-input v-model="draft.title" placeholder="要研究或核实的问题" />
          <el-input v-model="draft.description" type="textarea" :rows="4" placeholder="补充目标、范围和期望结果" />
          <el-select v-model="draft.taskType">
            <el-option label="专题研究" value="research" />
            <el-option label="知识刷新" value="refresh" />
            <el-option label="事实审核" value="review" />
          </el-select>
        </div>
        <div class="knowledge-modal-actions">
          <el-button @click="createVisible = false">取消</el-button>
          <el-button type="primary" :loading="creating" :disabled="!draft.knowledgeBaseId || !draft.title.trim()" @click="createTask">保存草稿</el-button>
        </div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { DataAnalysis, Loading, MoreFilled, Operation, Plus, Refresh, Select } from '@element-plus/icons-vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  createKnowledgeTask,
  listBookKnowledgeBases,
  listKnowledgeTasks,
  type KnowledgeBaseSummary,
  type KnowledgeTask,
} from '@/api/knowledge'

const bases = ref<KnowledgeBaseSummary[]>([])
const tasks = ref<KnowledgeTask[]>([])
const filterBaseId = ref('')
const loading = ref(false)
const creating = ref(false)
const createVisible = ref(false)
const draft = reactive({ knowledgeBaseId: '', title: '', description: '', taskType: 'research' as KnowledgeTask['task_type'] })

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
    ElMessage.success('研究任务已保存为草稿')
    await loadTasks()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    creating.value = false
  }
}

function taskIcon(type: KnowledgeTask['task_type']) { return type === 'refresh' ? Refresh : type === 'review' ? Select : DataAnalysis }
function typeLabel(type: KnowledgeTask['task_type']) { return ({ research: '专题研究', refresh: '知识刷新', review: '事实审核' })[type] }
function statusLabel(status: KnowledgeTask['status']) { return ({ draft: '草稿', queued: '排队中', running: '执行中', completed: '已完成', failed: '失败', cancelled: '已取消' })[status] }
function formatDate(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) }

onMounted(loadData)
</script>

<style scoped>
.task-filter { margin-bottom: 12px; }
.task-filter :deep(.el-select) { width: 230px; }
.task-summary { color: var(--text-faint); font-size: 12px; }
.research-task-list { display: grid; gap: 9px; }
.research-task { display: grid; grid-template-columns: 52px minmax(0, 1fr) 36px; align-items: center; gap: 15px; padding: 16px 17px; animation: task-in var(--motion-normal) var(--ease-spring-gentle) both; animation-delay: calc(var(--order) * 30ms); }
.task-kind { width: 52px; height: 52px; display: grid; place-items: center; border-radius: 16px; background: var(--accent-light); color: var(--accent); font-size: 22px; }
.task-kind.is-refresh { background: color-mix(in srgb, #32ade6 13%, transparent); color: #1685b8; }
.task-kind.is-review { background: color-mix(in srgb, #34c759 13%, transparent); color: #248a3d; }
.task-main { min-width: 0; }
.task-topline { display: flex; align-items: center; gap: 9px; color: var(--accent); font-size: 10px; font-weight: 650; }
.task-main h2 { margin: 5px 0 4px; font-size: 16px; }
.task-main > p { overflow: hidden; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.task-main footer { display: flex; gap: 12px; margin-top: 9px; color: var(--text-faint); font-size: 10px; }
.task-more { width: 36px; height: 36px; display: grid; place-items: center; border: 0; border-radius: 11px; background: transparent; color: var(--text-faint); }
.task-empty-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.mobile-create-task { display: none; }
@keyframes task-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@media (max-width: 768px) {
  .task-filter { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .task-filter :deep(.el-select) { width: 100%; }
  .mobile-create-task { width: 46px; min-height: 46px; display: grid; place-items: center; border: 0; border-radius: 14px; background: var(--accent); color: white; font-size: 18px; box-shadow: 0 7px 18px color-mix(in srgb, var(--accent) 22%, transparent); }
  .task-summary { grid-column: 1 / -1; }
  .research-task { grid-template-columns: 44px minmax(0, 1fr); padding: 14px; }
  .task-kind { width: 44px; height: 44px; border-radius: 14px; }
  .task-more { display: none; }
  .task-main > p { white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
}
</style>
