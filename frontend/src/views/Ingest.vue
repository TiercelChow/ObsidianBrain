<template>
  <div class="ingest-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">外部摄入</h1>
        <p class="page-subtitle">从外部源抓取 → LLM 预筛 → 一键摄入 Wiki</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadArticles" :loading="loading">
          <el-icon v-if="!loading"><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </header>

    <div v-if="loading && articles.length === 0" class="loading-state">
      <el-icon class="is-loading" :size="24"><Loading /></el-icon>
      <span>抓取中...</span>
    </div>

    <div v-else-if="articles.length > 0" class="article-list">
      <div v-for="article in articles" :key="article.id" class="article-card" :class="{ ingested: article.ingested }">
        <div class="article-header">
          <el-tag effect="plain">{{ article.source }}</el-tag>
          <span v-if="article.relevance" class="relevance" :class="relevanceClass(article.relevance)">
            {{ relevanceLabel(article.relevance) }}
          </span>
          <span v-if="article.ingested" class="ingested-badge">✅ 已摄入</span>
        </div>
        <h3 class="article-title">
          <a :href="article.url" target="_blank" rel="noopener">{{ article.title }}</a>
        </h3>
        <p class="article-summary" v-if="article.summary">{{ article.summary }}</p>
        <div class="article-reason" v-if="article.reason">📌 {{ article.reason }}</div>
        <div class="article-actions">
          <el-button
            v-if="!article.ingested"
           
            type="primary"
            @click="ingestArticle(article)"
            :loading="article.ingesting"
          >
            摄入到 Wiki
          </el-button>
          <el-button v-if="!article.ingested" @click="skipArticle(article)">跳过</el-button>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="暂无文章" :image-size="80" />

    <!-- 批量操作 -->
    <div v-if="highRelevantCount > 0" class="batch-bar">
      <span>{{ highRelevantCount }} 篇高相关文章</span>
      <el-button type="primary" @click="batchIngestAll" :loading="batchLoading">
        批量摄入
      </el-button>
    </div>
    <UndoSnackbar :show="Boolean(pendingSkip)" message="文章已跳过" @undo="undoSkip" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh, Loading } from '@element-plus/icons-vue'
import { getRadar, dismissRadarItem } from '@/api'
import { ingestSource } from '@/api/wiki'
import UndoSnackbar from '@/components/motion/UndoSnackbar.vue'
import { useUndoableRemoval } from '@/composables/useUndoableRemoval'

interface Article {
  id: string
  title: string
  summary: string
  source: string
  url: string
  relevance_score?: number
  published_at: string | null
  status: string
  ingested?: boolean
  ingesting?: boolean
  reason?: string
  relevance?: 'high' | 'medium' | 'low'
}

const loading = ref(false)
const batchLoading = ref(false)
const articles = ref<Article[]>([])
const { pending: pendingSkip, remove: queueSkip, undo: undoSkip } = useUndoableRemoval(
  articles,
  article => dismissRadarItem(article.id),
  () => ElMessage.error('跳过失败，文章已恢复'),
)

const highRelevantCount = computed(() =>
  articles.value.filter(a => a.relevance === 'high' && !a.ingested).length
)

function relevanceLabel(r: string) {
  return r === 'high' ? '高相关' : r === 'medium' ? '中相关' : '低相关'
}

function relevanceClass(r: string) {
  return r === 'high' ? 'high' : r === 'medium' ? 'medium' : 'low'
}

async function loadArticles() {
  loading.value = true
  try {
    const res = await getRadar(20) as unknown as { result: { items: Article[] } }
    articles.value = (res.result?.items || []).map(a => ({ ...a, ingesting: false }))
  } catch (e) {
    console.error('加载失败:', e)
  } finally {
    loading.value = false
  }
}

async function ingestArticle(article: Article) {
  article.ingesting = true
  try {
    // 先纳藏到 Vault 的 Raw/articles/
    await import('@/api').then(({ addToVault }) => addToVault(article.id, 'Raw/articles'))
    article.status = 'saved'

    // 找到纳藏后的文件路径
    const safeName = article.title.replace(/[^a-zA-Z0-9一-鿿]/g, '-').slice(0, 50)
    const rawPath = `Raw/articles/${safeName}.md`

    // ingest 到 Wiki
    await ingestSource(rawPath, 'article', article.url)
    article.ingested = true
    ElMessage.success('已摄入到 Wiki')
  } catch (e) {
    console.error('摄入失败:', e)
    ElMessage.error('摄入失败')
  } finally {
    article.ingesting = false
  }
}

async function skipArticle(article: Article) {
  queueSkip(article)
}

async function batchIngestAll() {
  const targets = articles.value.filter(a => a.relevance === 'high' && !a.ingested)
  if (targets.length === 0) return

  batchLoading.value = true
  let success = 0
  for (const article of targets) {
    try {
      await ingestArticle(article)
      success++
    } catch {
      // continue
    }
  }
  batchLoading.value = false
  ElMessage.success(`批量摄入完成：${success}/${targets.length}`)
}

onMounted(() => { loadArticles() })
</script>

<style scoped>
.ingest-page { max-width: 100%; min-height: 100%; }

.loading-state { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: var(--text-muted); }

.article-list { display: flex; flex-direction: column; gap: 12px; }
.article-card { padding: 20px; border-radius: 16px; transition: opacity var(--duration-normal) var(--ease-out), transform 150ms ease-out; }
.article-card.ingested { opacity: 0.6; }
.article-card:active { transform: scale(0.98); }
.article-header { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.relevance { font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 6px; }
.relevance.high { background: rgba(74, 222, 128, 0.15); color: #4ade80; }
.relevance.medium { background: rgba(251, 191, 36, 0.15); color: #fbbf24; }
.relevance.low { background: rgba(161, 161, 170, 0.15); color: #a1a1aa; }
.ingested-badge { font-size: 12px; color: #4ade80; margin-left: auto; }
.article-title { font-size: 16px; font-weight: 600; margin-bottom: 6px; }
.article-title a { color: var(--text-primary); text-decoration: none; }
.article-title a:hover { color: var(--accent); }
.article-summary { font-size: 13px; color: var(--text-tertiary); line-height: 1.6; margin-bottom: 8px; }
.article-reason { font-size: 12px; color: var(--accent); margin-bottom: 8px; }
.article-actions { display: flex; gap: 8px; }

.batch-bar { position: sticky; bottom: 0; display: flex; align-items: center; justify-content: space-between; padding: 12px 20px; border-radius: 14px; margin-top: 16px; }

@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .article-card { padding: 16px; border-radius: 15px; }
  .article-header { flex-wrap: wrap; }
  .article-title { font-size: 15px; line-height: 1.45; }
  .article-summary {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .article-actions { display: grid; grid-template-columns: minmax(0, 1fr) auto; }
  .article-actions :deep(.el-button) { margin: 0; }
  .batch-bar {
    bottom: var(--safe-bottom);
    padding: 10px 12px;
    border: 1px solid var(--border-glass);
    background: var(--bg-glass-strong);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
  }
}
</style>
