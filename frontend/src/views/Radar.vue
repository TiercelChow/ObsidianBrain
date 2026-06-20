<template>
  <div class="radar-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">智识雷达</h1>
        <p class="page-subtitle">让外部信息来找你：基于个人知识图谱的个性化推荐</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadRadar" :loading="loading" size="small">
          <el-icon><Refresh /></el-icon> 刷新
        </el-button>
      </div>
    </header>

    <!-- 雷达文章列表 -->
    <div class="radar-list" v-if="items.length > 0">
      <div
        v-for="item in items"
        :key="item.id"
        class="radar-card"
      >
        <div class="radar-header">
          <el-tag size="small" effect="plain">{{ item.source }}</el-tag>
          <el-tag :type="statusType(item.status)" size="small" effect="plain">{{ statusLabel(item.status) }}</el-tag>
        </div>
        <h3 class="radar-title">
          <a :href="item.url" target="_blank" rel="noopener">{{ item.title }}</a>
        </h3>
        <p class="radar-summary" v-if="item.summary">{{ item.summary }}</p>
        <div class="radar-meta">
          <span class="radar-date" v-if="item.published_at">{{ formatDate(item.published_at) }}</span>
          <span class="radar-score" v-if="item.relevance_score">
            相关度: {{ (item.relevance_score * 100).toFixed(0) }}%
          </span>
        </div>
        <div class="radar-actions">
          <el-button size="small" type="primary" @click="saveToVault(item.id)" :loading="item._saving">
            <el-icon><Download /></el-icon> 保存到 Vault
          </el-button>
          <el-button size="small" @click="dismissItem(item.id)">忽略</el-button>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="暂无推荐文章" :image-size="80" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getRadar, addToVault, dismissRadarItem } from '@/api'
import { Refresh, Download } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

interface RadarItem {
  id: string
  title: string
  summary: string
  source: string
  url: string
  relevance_score: number
  published_at: string | null
  status: string
  _saving?: boolean
}

const items = ref<RadarItem[]>([])
const loading = ref(false)

async function loadRadar() {
  loading.value = true
  try {
    const res = await getRadar(20) as unknown as { result: { items: RadarItem[] } }
    items.value = (res.result?.items || []).map(item => ({ ...item, _saving: false }))
  } catch (e) {
    console.error('加载雷达失败:', e)
    items.value = []
  } finally {
    loading.value = false
  }
}

async function saveToVault(articleId: string) {
  const item = items.value.find(i => i.id === articleId)
  if (!item) return
  item._saving = true
  try {
    await addToVault(articleId)
    ElMessage.success(`已保存: ${item.title}`)
    item.status = 'saved'
  } catch (e) {
    ElMessage.error('保存失败')
  } finally {
    item._saving = false
  }
}

async function dismissItem(articleId: string) {
  try {
    await dismissRadarItem(articleId)
    items.value = items.value.filter(i => i.id !== articleId)
    ElMessage.success('已忽略')
  } catch {
    ElMessage.error('操作失败')
  }
}

function statusType(status: string): string {
  const map: Record<string, string> = { new: 'info', read: '', saved: 'success', dismissed: 'warning' }
  return map[status] || 'info'
}

function statusLabel(status: string): string {
  const map: Record<string, string> = { new: '新', read: '已读', saved: '已保存', dismissed: '已忽略' }
  return map[status] || status
}

function formatDate(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
  } catch { return dateStr }
}

onMounted(() => { loadRadar() })
</script>

<style scoped>
.radar-page {
  min-height: 100%;
  max-width: 100%;
}
.radar-page .radar-card {
  animation: pageFadeIn 0.5s ease both;
}
.radar-page .radar-card:nth-child(2) { animation-delay: 0.06s; }
.radar-page .radar-card:nth-child(3) { animation-delay: 0.12s; }
.radar-page .radar-card:nth-child(4) { animation-delay: 0.18s; }
@keyframes pageFadeIn {
  from { opacity: 0; transform: translateY(20px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-faint); font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.radar-list { display: flex; flex-direction: column; gap: 12px; }
.radar-card {
  padding: 20px; border-radius: 16px;
  transition: box-shadow 0.2s ease;
}
.radar-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.04); }

.radar-header { display: flex; gap: 8px; margin-bottom: 8px; }
.radar-title { font-size: 16px; font-weight: 600; margin-bottom: 6px; }
.radar-title a { color: var(--text-primary); text-decoration: none; }
.radar-title a:hover { color: #6366f1; }

.radar-summary { font-size: 13px; color: var(--text-tertiary); line-height: 1.6; margin-bottom: 10px; }

.radar-meta {
  display: flex; gap: 16px; align-items: center;
  font-size: 12px; color: var(--text-faint); margin-bottom: 12px;
}

.radar-actions { display: flex; gap: 8px; }
</style>
