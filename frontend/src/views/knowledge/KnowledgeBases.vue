<template>
  <KnowledgePageShell title="书籍知识库" subtitle="书架中的每一本书，都是一座独立、可追溯的知识库">
    <template #actions>
      <el-button :loading="loading" @click="loadCards"><el-icon><Refresh /></el-icon>刷新</el-button>
    </template>

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
        <div class="book-cover" :class="`is-${card.book.kind}`">
          <el-icon><component :is="card.book.kind === 'pdf' ? Document : FolderOpened" /></el-icon>
          <span>{{ card.book.kind === 'pdf' ? 'PDF' : 'MD' }}</span>
        </div>
        <div class="book-card-main">
          <div class="book-card-title-row">
            <div>
              <span v-if="card.book.category" class="book-category">{{ card.book.category }}</span>
              <h2>{{ card.book.name }}</h2>
            </div>
            <span
              class="knowledge-status"
              :class="statusClass(card)"
            >{{ statusLabel(card) }}</span>
          </div>
          <p class="book-description">{{ card.book.description || '还没有添加书籍说明' }}</p>
          <p class="book-path" :title="card.book.path">{{ card.book.path }}</p>

          <div v-if="card.knowledge_base" class="book-stats">
            <span><strong>{{ card.knowledge_base.source_count }}</strong> 来源</span>
            <span><strong>{{ card.knowledge_base.entry_count }}</strong> 实体</span>
            <span><strong>{{ card.knowledge_base.claim_count }}</strong> 论断</span>
            <span><strong>{{ card.knowledge_base.task_count }}</strong> 任务</span>
          </div>
          <div v-else class="book-uninitialized">
            尚未初始化。创建后，Markdown 章节将进入数据库并保留来源引用。
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
                :loading="busyBookId === card.book.id"
                @click="sync(card)"
              ><el-icon><Refresh /></el-icon>同步</el-button>
            </template>
            <el-button text @click="$router.push({ path: '/reader', query: { book: card.book.id } })">
              阅读
            </el-button>
          </div>
          <p v-if="card.knowledge_base?.last_error" class="book-error">
            {{ card.knowledge_base.last_error }}
          </p>
        </div>
      </article>
    </section>

    <div v-else class="knowledge-empty knowledge-surface">
      <div class="empty-orb"><el-icon><Collection /></el-icon></div>
      <strong>书架还是空的</strong>
      <span>先在阅境轩加入 Markdown 文件夹或 PDF，再回来为它建立知识库。</span>
      <el-button type="primary" @click="$router.push('/reader')">前往阅境轩</el-button>
    </div>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Collection, Document, FolderOpened, Loading, Refresh } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  initializeBookKnowledgeBase,
  listBookKnowledgeBases,
  syncBookKnowledgeBase,
  type BookKnowledgeCard,
} from '@/api/knowledge'

const router = useRouter()
const cards = ref<BookKnowledgeCard[]>([])
const loading = ref(false)
const busyBookId = ref('')

const initializedCount = computed(() => cards.value.filter(card => card.knowledge_base).length)
const entryCount = computed(() => cards.value.reduce((sum, card) => sum + (card.knowledge_base?.entry_count ?? 0), 0))
const attentionCount = computed(() => cards.value.filter(card => {
  const base = card.knowledge_base
  return base && (base.sync_state !== 'clean' || base.health_state !== 'healthy')
}).length)

async function loadCards() {
  loading.value = true
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    cards.value = response.result.items
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loading.value = false
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

function openWiki(baseId: string) {
  router.push({ path: '/knowledge/wiki', query: { base: baseId } })
}

function statusLabel(card: BookKnowledgeCard) {
  if (!card.knowledge_base) return '未初始化'
  const labels: Record<string, string> = {
    clean: '已同步', outdated: '待同步', scanning: '扫描中', extracting: '提取中',
    ingesting: '建模中', failed: '同步失败',
  }
  return labels[card.knowledge_base.sync_state] || card.knowledge_base.sync_state
}

function statusClass(card: BookKnowledgeCard) {
  return card.knowledge_base ? `is-${card.knowledge_base.sync_state}` : 'is-draft'
}

onMounted(loadCards)
</script>

<style scoped>
.overview-strip { display: grid; grid-template-columns: repeat(4, 1fr); margin-bottom: 16px; padding: 18px 22px; }
.overview-strip > div { display: grid; gap: 2px; padding: 0 20px; border-left: 1px solid var(--border-faint); }
.overview-strip > div:first-child { padding-left: 0; border-left: 0; }
.overview-strip strong { font-size: 25px; font-weight: 720; font-variant-numeric: tabular-nums; }
.overview-strip span { color: var(--text-faint); font-size: 12px; }
.book-wiki-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
.book-wiki-card { min-width: 0; display: grid; grid-template-columns: 82px 1fr; gap: 18px; padding: 19px; animation: knowledge-card-in var(--motion-slow) var(--ease-spring-gentle) both; animation-delay: calc(var(--order) * 36ms); }
.book-cover { height: 108px; display: grid; place-content: center; justify-items: center; gap: 8px; border-radius: 15px 12px 12px 15px; background: linear-gradient(145deg, color-mix(in srgb, var(--accent) 78%, #9b7bff), color-mix(in srgb, var(--accent) 56%, #263b9d)); color: white; box-shadow: 7px 8px 20px color-mix(in srgb, var(--accent) 18%, transparent), inset -5px 0 10px rgba(0,0,0,.1), inset 1px 0 rgba(255,255,255,.3); }
.book-cover.is-pdf { background: linear-gradient(145deg, #ff6b63, #c83432); }
.book-cover .el-icon { font-size: 27px; }
.book-cover span { font-size: 10px; font-weight: 760; letter-spacing: .12em; opacity: .8; }
.book-card-main { min-width: 0; }
.book-card-title-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.book-card-title-row h2 { margin: 3px 0 0; font-size: 18px; font-weight: 690; line-height: 1.3; }
.book-category { color: var(--accent); font-size: 10px; font-weight: 720; letter-spacing: .07em; }
.book-description { min-height: 20px; margin: 8px 0 4px; color: var(--text-muted); font-size: 13px; }
.book-path { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.book-stats { display: flex; flex-wrap: wrap; gap: 7px 14px; margin-top: 14px; color: var(--text-muted); font-size: 11px; }
.book-stats strong { color: var(--text-primary); font-size: 14px; font-variant-numeric: tabular-nums; }
.book-uninitialized { margin-top: 13px; padding: 9px 11px; border-radius: 10px; background: var(--bg-glass-subtle); color: var(--text-faint); font-size: 11px; line-height: 1.55; }
.book-actions { display: flex; align-items: center; gap: 7px; margin-top: 15px; }
.book-actions .el-button + .el-button { margin-left: 0; }
.book-error { margin-top: 10px; color: #d52d25; font-size: 11px; }
.empty-orb { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 28px; }
@keyframes knowledge-card-in { from { opacity: 0; transform: translateY(12px) scale(.985); } to { opacity: 1; transform: none; } }
@media (max-width: 1050px) { .book-wiki-grid { grid-template-columns: 1fr; } }
@media (max-width: 768px) {
  .overview-strip { grid-template-columns: repeat(2, 1fr); padding: 8px; }
  .overview-strip > div { padding: 12px; border-left: 0; }
  .overview-strip > div:nth-child(even) { border-left: 1px solid var(--border-faint); }
  .overview-strip > div:nth-child(n+3) { border-top: 1px solid var(--border-faint); }
  .book-wiki-card { grid-template-columns: 62px 1fr; gap: 13px; padding: 15px; }
  .book-cover { height: 86px; border-radius: 13px 10px 10px 13px; }
  .book-card-title-row { display: block; }
  .book-card-title-row .knowledge-status { margin-top: 7px; }
  .book-actions { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr; }
  .book-actions .el-button { width: 100%; margin: 0; }
  .book-actions .el-button:last-child:nth-child(3) { grid-column: 1 / -1; }
}
</style>
