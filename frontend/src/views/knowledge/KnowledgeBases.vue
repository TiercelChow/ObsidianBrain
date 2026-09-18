<template>
  <KnowledgePageShell title="书籍知识库" subtitle="书架中的每一本书，都是一座独立、可追溯的知识库">
    <template #actions>
      <el-button :loading="loading" @click="loadCards()"><el-icon><Refresh /></el-icon>刷新</el-button>
    </template>

    <div class="mobile-base-actions">
      <span>{{ cards.length }} 本 Markdown 书籍</span>
      <button type="button" :disabled="loading" aria-label="刷新书籍知识库" @click="loadCards()"><el-icon :class="{ 'is-loading': loading }"><Refresh /></el-icon><span>刷新</span></button>
    </div>

    <section v-if="cards.length" class="overview-strip knowledge-surface">
      <div><strong>{{ cards.length }}</strong><span>书架藏书</span></div>
      <div><strong>{{ initializedCount }}</strong><span>已建知识库</span></div>
      <div><strong>{{ entryCount }}</strong><span>数据库实体</span></div>
      <div><strong>{{ attentionCount }}</strong><span>需要处理</span></div>
    </section>

    <div v-if="loading && !cards.length" class="knowledge-empty knowledge-surface">
      <el-icon class="is-loading" :size="26"><Loading /></el-icon>
      <span>正在读取阅境轩书架…</span>
    </div>

    <section v-else-if="cards.length" class="book-wiki-grid">
      <article
        v-for="(card, index) in cards"
        :key="card.book.id"
        class="book-wiki-card knowledge-surface"
        :style="{ '--order': index }"
      >
        <div class="book-cover">
          <el-icon><FolderOpened /></el-icon>
          <span>MD</span>
        </div>
        <div class="book-card-main">
          <div class="book-card-title-row">
            <div>
              <span v-if="card.book.category" class="book-category">{{ card.book.category }}</span>
              <h2>{{ card.book.name }}</h2>
            </div>
            <div class="knowledge-status-stack">
              <span
                class="knowledge-status"
                :class="sourceStatusClass(card)"
              >来源 · {{ sourceStatusLabel(card) }}</span>
              <span
                v-if="card.knowledge_base"
                class="knowledge-status"
                :class="`is-${card.knowledge_base.compile_state}`"
              >Wiki · {{ compileStatusLabel(card.knowledge_base) }}</span>
              <span
                v-if="card.knowledge_base && card.knowledge_base.lifecycle !== 'active'"
                class="knowledge-status is-warning"
              >{{ lifecycleLabel(card.knowledge_base.lifecycle) }}</span>
            </div>
          </div>
          <p class="book-description">{{ card.book.description || '还没有添加书籍说明' }}</p>
          <p class="book-path" :title="card.book.path">{{ card.book.path }}</p>

          <div v-if="card.knowledge_base" class="book-stats">
            <span><strong>{{ card.knowledge_base.source_count }}</strong> 来源</span>
            <span><strong>{{ card.knowledge_base.entry_count }}</strong> 实体</span>
            <span><strong>{{ card.knowledge_base.claim_count }}</strong> 论断</span>
            <span><strong>{{ card.knowledge_base.task_count }}</strong> 任务</span>
          </div>
          <div v-if="card.knowledge_base?.compile_state === 'compiling'" class="compile-activity">
            <div class="compile-phase">
              <span class="compile-phase-dot" :class="{ 'is-still': !isCompileActivelyRunning(card.knowledge_base) }"></span>
              <div>
                <strong>{{ compilePhaseLabel(card.knowledge_base) }}</strong>
                <p>{{ card.knowledge_base.compile_message || '正在准备智能编译…' }}</p>
              </div>
              <span class="compile-percent">{{ compileProgressValue(card.knowledge_base) }}%</span>
            </div>
            <div class="compile-progress" role="progressbar" :aria-valuenow="compileProgressValue(card.knowledge_base)" aria-valuemin="0" aria-valuemax="100">
              <i :style="{ width: compileProgress(card.knowledge_base) }"></i>
            </div>
            <div class="compile-meta">
              <span>{{ card.knowledge_base.compile_processed_sources }}/{{ card.knowledge_base.compile_total_sources }} 个来源</span>
              <span v-if="card.knowledge_base.compile_total_batches">
                第 {{ card.knowledge_base.compile_current_batch }}/{{ card.knowledge_base.compile_total_batches }} 批
              </span>
              <span v-else>等待模型任务启动</span>
              <span v-if="card.knowledge_base.compile_heartbeat_at" :title="card.knowledge_base.compile_heartbeat_at">
                {{ formatCompileActivityTime(card.knowledge_base.compile_heartbeat_at) }} 更新
              </span>
            </div>
            <div class="compile-controls">
              <el-button
                v-if="card.knowledge_base.compile_phase === 'waiting_review' && card.knowledge_base.compile_change_set_id"
                type="primary"
                size="small"
                @click="openCompileReview(card.knowledge_base)"
              >查看并审核</el-button>
              <el-button
                v-else
                size="small"
                :loading="cancellingBaseId === card.knowledge_base.id"
                :disabled="card.knowledge_base.compile_cancel_requested"
                @click="cancelCompile(card.knowledge_base)"
              >{{ card.knowledge_base.compile_cancel_requested ? '正在停止…' : '停止编译' }}</el-button>
            </div>
          </div>
          <div v-else-if="!card.knowledge_base" class="book-uninitialized">
            尚未初始化。创建后，Markdown 章节将进入数据库并保留来源引用。
          </div>
          <div v-else-if="!card.knowledge_base.source_available" class="book-uninitialized is-warning">
            原书目录已失效。历史知识仍可浏览，请先在阅境轩重新添加正确目录。
          </div>

          <div class="book-actions">
            <el-button
              v-if="!card.knowledge_base"
              type="primary"
              :loading="busyBookId === card.book.id"
              @click="initialize(card)"
            >建立知识库</el-button>
            <template v-else>
              <el-button type="primary" @click="openWiki(card.knowledge_base.id)">打开 Wiki</el-button>
              <el-button
                v-if="card.book.kind === 'folder' && card.knowledge_base.sync_state === 'clean' && card.knowledge_base.compile_state !== 'compiling'"
                :disabled="card.knowledge_base.lifecycle !== 'active' || !card.knowledge_base.source_available"
                :loading="compilingBaseId === card.knowledge_base.id"
                @click="compileWiki(card)"
              ><el-icon><MagicStick /></el-icon>{{ compileActionLabel(card.knowledge_base) }}</el-button>
              <el-button
                :loading="busyBookId === card.book.id"
                :disabled="card.knowledge_base.lifecycle !== 'active' || !card.knowledge_base.source_available || card.knowledge_base.compile_state === 'compiling'"
                @click="sync(card)"
              ><el-icon><Refresh /></el-icon>同步</el-button>
              <el-button @click="openManage(card)">管理</el-button>
            </template>
            <el-button text @click="$router.push({ path: '/reader', query: { book: card.book.id } })">
              阅读
            </el-button>
          </div>
          <p v-if="card.knowledge_base?.compile_error || card.knowledge_base?.last_error" class="book-error">
            {{ card.knowledge_base.compile_error || card.knowledge_base.last_error }}
          </p>
        </div>
      </article>
    </section>

    <div v-else class="knowledge-empty knowledge-surface">
      <div class="empty-orb"><el-icon><Collection /></el-icon></div>
      <strong>书架还是空的</strong>
      <span>这里只显示阅境轩中的 Markdown 文件夹；先加入一个文集，再回来建立知识库。</span>
      <el-button type="primary" @click="$router.push('/reader')">前往阅境轩</el-button>
    </div>

    <MotionModal v-model="manageVisible" aria-label="管理知识库">
      <div v-if="managedCard?.knowledge_base" class="knowledge-modal-card manage-modal">
        <div class="knowledge-modal-head">
          <div><h3>管理知识库</h3><p>{{ managedCard.book.name }} · 这里只管理生成知识，不会改动原书。</p></div>
          <span class="knowledge-status" :class="managedCard.knowledge_base.lifecycle === 'active' ? 'is-healthy' : 'is-warning'">{{ lifecycleLabel(managedCard.knowledge_base.lifecycle) }}</span>
        </div>
        <div class="manage-actions">
          <el-button v-if="managedCard.knowledge_base.lifecycle !== 'active'" type="primary" :loading="managing" @click="changeLifecycle('active')">恢复使用</el-button>
          <el-button v-if="managedCard.knowledge_base.lifecycle === 'active'" :loading="managing" @click="changeLifecycle('paused')">暂停知识库</el-button>
          <el-button v-if="managedCard.knowledge_base.lifecycle !== 'archived'" :loading="managing" @click="changeLifecycle('archived')">归档知识库</el-button>
        </div>
        <div class="delete-zone">
          <strong>删除生成知识</strong>
          <p>实体、问答、研究任务、审核记录和统计会被永久删除；阅境轩书架与 Markdown 原书不受影响。</p>
          <el-input v-model="deleteConfirmation" :placeholder="`输入书名「${managedCard.book.name}」确认`" />
          <el-button type="danger" plain :disabled="deleteConfirmation !== managedCard.book.name" :loading="managing" @click="deleteManagedBase">删除知识库</el-button>
        </div>
        <div class="knowledge-modal-actions"><el-button @click="manageVisible = false">完成</el-button></div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Collection, FolderOpened, Loading, MagicStick, Refresh } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { canFocusDocument } from '@/utils/modalFocusPolicy'
import {
  cancelBookKnowledgeCompile,
  compileBookKnowledgeBase,
  deleteBookKnowledgeBase,
  initializeBookKnowledgeBase,
  listBookKnowledgeBases,
  setBookKnowledgeBaseLifecycle,
  syncBookKnowledgeBase,
  type BookKnowledgeCard,
  type KnowledgeBaseSummary,
} from '@/api/knowledge'

const router = useRouter()
const cards = ref<BookKnowledgeCard[]>([])
const loading = ref(false)
const busyBookId = ref('')
const compilingBaseId = ref('')
const cancellingBaseId = ref('')
const manageVisible = ref(false)
const managedCard = ref<BookKnowledgeCard | null>(null)
const deleteConfirmation = ref('')
const managing = ref(false)
let pollingTimer: number | undefined

const initializedCount = computed(() => cards.value.filter(card => card.knowledge_base).length)
const entryCount = computed(() => cards.value.reduce((sum, card) => sum + (card.knowledge_base?.entry_count ?? 0), 0))
const attentionCount = computed(() => cards.value.filter(card => {
  const base = card.knowledge_base
  return base && (
    base.sync_state !== 'clean'
    || base.health_state !== 'healthy'
    || (base.compile_mode === 'smart' && base.compile_state !== 'ready')
  )
}).length)

async function loadCards(silent = false) {
  if (!silent) loading.value = true
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    cards.value = response.result.items.filter(card => card.book.kind === 'folder')
  } catch (error) {
    if (!silent) ElMessage.error((error as Error).message)
  } finally {
    if (!silent) loading.value = false
  }
}

async function initialize(card: BookKnowledgeCard) {
  busyBookId.value = card.book.id
  try {
    const response = await initializeBookKnowledgeBase(card.book.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '初始化失败')
    ElMessage.success(response.result.message)
    await loadCards()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    busyBookId.value = ''
  }
}

async function sync(card: BookKnowledgeCard) {
  if (!card.knowledge_base) return
  busyBookId.value = card.book.id
  try {
    const response = await syncBookKnowledgeBase(card.knowledge_base.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '同步失败')
    ElMessage.success(response.result.message)
    await loadCards()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    busyBookId.value = ''
  }
}

async function compileWiki(card: BookKnowledgeCard) {
  if (!card.knowledge_base) return
  compilingBaseId.value = card.knowledge_base.id
  try {
    const response = await compileBookKnowledgeBase(card.knowledge_base.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '智能编译失败')
    card.knowledge_base = response.result.knowledge_base
    ElMessage.success(response.result.message || '智能编译任务已进入后台，可以离开当前页面')
    await loadCards(true)
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadCards(true)
  } finally {
    compilingBaseId.value = ''
  }
}

async function cancelCompile(base: KnowledgeBaseSummary) {
  cancellingBaseId.value = base.id
  try {
    const response = await cancelBookKnowledgeCompile(base.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '停止编译失败')
    const card = cards.value.find(item => item.knowledge_base?.id === base.id)
    if (card) card.knowledge_base = response.result.knowledge_base
    ElMessage.success('已请求停止编译，当前模型调用结束后会安全退出')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    cancellingBaseId.value = ''
    await loadCards(true)
  }
}

function openCompileReview(base: KnowledgeBaseSummary) {
  if (!base.compile_change_set_id) return
  router.push({ path: '/knowledge/wiki', query: { base: base.id, review: base.compile_change_set_id } })
}

function openWiki(baseId: string) {
  router.push({ path: '/knowledge/wiki', query: { base: baseId } })
}

function openManage(card: BookKnowledgeCard) {
  managedCard.value = card
  deleteConfirmation.value = ''
  manageVisible.value = true
}

async function changeLifecycle(lifecycle: 'active' | 'paused' | 'archived') {
  const base = managedCard.value?.knowledge_base
  if (!base) return
  managing.value = true
  try {
    const response = await setBookKnowledgeBaseLifecycle(base.id, lifecycle)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '状态更新失败')
    ElMessage.success(lifecycle === 'active' ? '知识库已恢复' : lifecycle === 'paused' ? '知识库已暂停' : '知识库已归档')
    manageVisible.value = false
    await loadCards()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    managing.value = false
  }
}

async function deleteManagedBase() {
  const card = managedCard.value
  if (!card?.knowledge_base || deleteConfirmation.value !== card.book.name) return
  managing.value = true
  try {
    const response = await deleteBookKnowledgeBase(card.knowledge_base.id, deleteConfirmation.value)
    if (response.status !== 'success' || !response.result?.deleted) throw new Error(response.error?.message || '删除失败')
    ElMessage.success('知识库生成数据已删除，原书仍保留在阅境轩')
    manageVisible.value = false
    await loadCards()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    managing.value = false
  }
}

function lifecycleLabel(lifecycle: string) {
  return ({ active: '使用中', paused: '已暂停', archived: '已归档' } as Record<string, string>)[lifecycle] || lifecycle
}

function sourceStatusLabel(card: BookKnowledgeCard) {
  if (!card.knowledge_base) return '未初始化'
  const labels: Record<string, string> = {
    clean: '已同步', outdated: '待同步', scanning: '扫描中', extracting: '提取中',
    ingesting: '建模中', failed: '同步失败',
  }
  return labels[card.knowledge_base.sync_state] || card.knowledge_base.sync_state
}

function sourceStatusClass(card: BookKnowledgeCard) {
  return card.knowledge_base ? `is-${card.knowledge_base.sync_state}` : 'is-draft'
}

function compileStatusLabel(base: NonNullable<BookKnowledgeCard['knowledge_base']>) {
  if (base.compile_mode === 'chapter' && base.compile_state === 'not_started') return '章节索引'
  if (base.compile_phase === 'waiting_review') return '待审核'
  if (base.compile_phase === 'cancelling') return '停止中'
  const labels: Record<string, string> = {
    not_started: '待编译', outdated: '待更新', compiling: '编译中', ready: '智能 Wiki', failed: '编译失败',
  }
  return labels[base.compile_state] || base.compile_state
}

function compileActionLabel(base: KnowledgeBaseSummary) {
  if (base.compile_state === 'failed') return '重新编译'
  return base.compile_state === 'ready' ? '增量编译' : '智能编译'
}

function compilePhaseLabel(base: KnowledgeBaseSummary) {
  const labels: Record<string, string> = {
    queued: '已进入后台队列', preparing: '正在准备来源', invoking: '正在连接模型',
    runtime: '正在准备模型请求', launching: '正在启动运行时', connected: '运行时已连接',
    session_ready: '模型会话已就绪', request_sent: '正在等待模型返回',
    stopped: '模型已结束，正在校验返回',
    thinking: '模型正在分析', tool: '正在调用知识工具',
    generating: '正在生成知识候选', retrying: '正在校正模型结果',
    batch_completed: '本批分析完成', finalizing: '正在保存候选变更',
    waiting_review: '智能编译完成，等待审核', cancelling: '正在安全停止',
  }
  return labels[base.compile_phase] || '智能编译进行中'
}

function isCompileActivelyRunning(base: KnowledgeBaseSummary) {
  return base.compile_phase !== 'waiting_review'
}

function formatCompileActivityTime(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '刚刚'
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
  }).format(date)
}

function compileProgressValue(base: KnowledgeBaseSummary) {
  const sourceProgress = base.compile_total_sources
    ? base.compile_processed_sources / base.compile_total_sources
    : 0
  const batchProgress = base.compile_total_batches
    ? Math.max(0, base.compile_current_batch - (base.compile_phase === 'batch_completed' ? 0 : 1)) / base.compile_total_batches
    : 0
  if (base.compile_phase === 'waiting_review') return 100
  return Math.min(99, Math.round(Math.max(sourceProgress, batchProgress) * 100))
}

function compileProgress(base: NonNullable<BookKnowledgeCard['knowledge_base']>) {
  return `${compileProgressValue(base)}%`
}

onMounted(() => {
  void loadCards()
  pollingTimer = window.setInterval(() => {
    if (!canFocusDocument(document)) return
    if (cards.value.some(card => card.knowledge_base?.compile_state === 'compiling' && card.knowledge_base.compile_phase !== 'waiting_review')) {
      void loadCards(true)
    }
  }, 2500)
})

onBeforeUnmount(() => {
  if (pollingTimer !== undefined) window.clearInterval(pollingTimer)
})
</script>

<style scoped>
.mobile-base-actions { display: none; }
.overview-strip { display: grid; grid-template-columns: repeat(4, 1fr); margin-bottom: 16px; padding: 18px 22px; }
.overview-strip > div { display: grid; gap: 2px; padding: 0 20px; border-left: 1px solid var(--border-faint); }
.overview-strip > div:first-child { padding-left: 0; border-left: 0; }
.overview-strip strong { font-size: 25px; font-weight: 720; font-variant-numeric: tabular-nums; }
.overview-strip span { color: var(--text-faint); font-size: 12px; }
.book-wiki-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 460px), 1fr)); gap: 14px; }
.book-wiki-card { min-width: 0; display: grid; grid-template-columns: 82px 1fr; gap: 18px; padding: 19px; animation: knowledge-card-in var(--motion-slow) var(--ease-spring-gentle) both; animation-delay: calc(var(--order) * 36ms); }
.book-cover { height: 108px; display: grid; place-content: center; justify-items: center; gap: 8px; border-radius: 15px 12px 12px 15px; background: linear-gradient(145deg, color-mix(in srgb, var(--accent) 78%, #9b7bff), color-mix(in srgb, var(--accent) 56%, #263b9d)); color: white; box-shadow: 7px 8px 20px color-mix(in srgb, var(--accent) 18%, transparent), inset -5px 0 10px rgba(0,0,0,.1), inset 1px 0 rgba(255,255,255,.3); }
.book-cover .el-icon { font-size: 27px; }
.book-cover span { font-size: 10px; font-weight: 760; letter-spacing: .12em; opacity: .8; }
.book-card-main { min-width: 0; }
.book-card-title-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.book-card-title-row > div:first-child { min-width: 0; flex: 1; }
.knowledge-status-stack { min-width: 0; max-width: 50%; display: grid; justify-items: end; gap: 6px; flex: 0 1 auto; }
.book-card-title-row h2 { margin: 3px 0 0; overflow-wrap: anywhere; font-size: 18px; font-weight: 690; line-height: 1.3; }
.book-category { color: var(--accent); font-size: 10px; font-weight: 720; letter-spacing: .07em; }
.book-description { min-height: 20px; margin: 8px 0 4px; overflow-wrap: anywhere; color: var(--text-muted); font-size: 13px; }
.book-path { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.book-stats { display: flex; flex-wrap: wrap; gap: 7px 14px; margin-top: 14px; color: var(--text-muted); font-size: 11px; }
.book-stats strong { color: var(--text-primary); font-size: 14px; font-variant-numeric: tabular-nums; }
.compile-activity { display: grid; gap: 9px; margin-top: 13px; padding: 12px; border: 1px solid color-mix(in srgb, var(--accent) 14%, var(--border-faint)); border-radius: 14px; background: linear-gradient(145deg, color-mix(in srgb, var(--accent) 6%, var(--bg-glass-subtle)), var(--bg-glass-subtle)); }
.compile-phase { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: start; gap: 9px; }
.compile-phase-dot { width: 8px; height: 8px; margin-top: 5px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 30%, transparent); animation: compile-pulse 1.8s ease-out infinite; }
.compile-phase-dot.is-still { animation: none; box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 10%, transparent); }
.compile-phase strong { display: block; font-size: 12px; font-weight: 690; line-height: 1.45; }
.compile-phase p { margin: 2px 0 0; overflow-wrap: anywhere; color: var(--text-muted); font-size: 10px; line-height: 1.5; }
.compile-percent { color: var(--accent); font-size: 12px; font-weight: 700; font-variant-numeric: tabular-nums; }
.compile-progress { position: relative; height: 5px; overflow: hidden; border-radius: 999px; background: color-mix(in srgb, var(--accent) 10%, var(--bg-glass-subtle)); }
.compile-progress i { position: absolute; inset: 0 auto 0 0; border-radius: inherit; background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 75%, white), var(--accent)); box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 28%, transparent); transition: width var(--motion-slow) var(--ease-emphasized); }
.compile-meta { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 4px 10px; color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }
.compile-controls { display: flex; justify-content: flex-end; gap: 7px; }
.compile-controls .el-button { margin: 0; }
.book-uninitialized { margin-top: 13px; padding: 9px 11px; border-radius: 10px; background: var(--bg-glass-subtle); color: var(--text-faint); font-size: 11px; line-height: 1.55; }
.book-uninitialized.is-warning { color: #b56600; background: color-mix(in srgb, #ff9f0a 10%, var(--bg-glass-subtle)); }
.book-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; margin-top: 15px; }
.book-actions .el-button + .el-button { margin-left: 0; }
.book-error { margin-top: 10px; overflow-wrap: anywhere; color: #d52d25; font-size: 11px; }
.empty-orb { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 28px; }
.manage-modal { display: grid; gap: 16px; overflow-y: auto; }
.manage-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.manage-actions .el-button { margin: 0; }
.delete-zone { display: grid; gap: 9px; padding: 15px; border: 1px solid color-mix(in srgb, #ff3b30 22%, var(--border-faint)); border-radius: 15px; background: color-mix(in srgb, #ff3b30 5%, var(--bg-glass-subtle)); }
.delete-zone strong { font-size: 13px; }
.delete-zone p { color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.delete-zone .el-button { width: fit-content; margin: 0; }
@keyframes knowledge-card-in { from { opacity: 0; transform: translateY(12px) scale(.985); } to { opacity: 1; transform: none; } }
@keyframes compile-pulse { 0% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 32%, transparent); } 70%, 100% { box-shadow: 0 0 0 8px transparent; } }
@media (prefers-reduced-motion: reduce) { .compile-phase-dot { animation: none; } }
@media (max-width: 1050px) { .book-wiki-grid { grid-template-columns: 1fr; } }
@media (max-width: 768px) {
  .mobile-base-actions { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 10px; color: var(--text-faint); font-size: 12px; }
  .mobile-base-actions button { min-height: 44px; display: inline-flex; align-items: center; gap: 6px; padding: 0 13px; border: 1px solid var(--border-subtle); border-radius: 13px; background: var(--bg-glass); color: var(--accent); font: inherit; font-weight: 650; }
  .mobile-base-actions button:disabled { opacity: .5; }
  .overview-strip { grid-template-columns: repeat(2, 1fr); padding: 8px; }
  .overview-strip > div { padding: 12px; border-left: 0; }
  .overview-strip > div:nth-child(even) { border-left: 1px solid var(--border-faint); }
  .overview-strip > div:nth-child(n+3) { border-top: 1px solid var(--border-faint); }
  .book-wiki-card { grid-template-columns: 62px 1fr; gap: 13px; padding: 15px; }
  .book-cover { height: 86px; border-radius: 13px 10px 10px 13px; }
  .book-card-title-row { display: block; }
  .knowledge-status-stack { max-width: 100%; justify-items: start; margin-top: 8px; }
  .compile-activity { grid-column: 1 / -1; }
  .book-actions { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr; }
  .book-actions .el-button { width: 100%; margin: 0; }
  .book-actions .el-button:last-child:nth-child(3) { grid-column: 1 / -1; }
}
</style>
