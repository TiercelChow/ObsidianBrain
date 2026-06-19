<template>
  <div class="memory-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">知识库</h1>
        <p class="page-subtitle">洞察你的知识体系：孤岛、枢纽、尘封与新生</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="loadInsights(true)" :loading="loading">
          <el-icon v-if="!loading"><Refresh /></el-icon>
          重新统计
        </el-button>
      </div>
    </header>

    <div v-if="loading && !insights" class="loading-state">
      <el-icon class="is-loading" :size="24"><Loading /></el-icon>
      <span>正在分析知识库...</span>
    </div>

    <div v-else-if="insights" class="insights-grid">
      <!-- 知识孤岛 -->
      <div class="insight-card">
        <div class="card-header">
          <span class="card-icon">🏝️</span>
          <div>
            <h3 class="card-title">知识孤岛</h3>
            <p class="card-desc">没有被其他笔记引用的笔记</p>
          </div>
          <span class="card-count">{{ insights.islands.count }}</span>
        </div>
        <div class="card-body">
          <div v-if="insights.islands.notes.length > 0" class="note-list">
            <div v-for="note in insights.islands.notes" :key="note.path" class="note-item" @click="openInObsidian(note.path)">
              <span class="note-path">{{ formatPath(note.path) }}</span>
              <span class="note-meta">{{ note.days_ago }}天前</span>
            </div>
          </div>
          <div v-else class="empty-hint">没有知识孤岛 🎉</div>
        </div>
      </div>

      <!-- 知识枢纽 -->
      <div class="insight-card">
        <div class="card-header">
          <span class="card-icon">🕸️</span>
          <div>
            <h3 class="card-title">知识枢纽</h3>
            <p class="card-desc">被引用最多的核心笔记</p>
          </div>
        </div>
        <div class="card-body">
          <div v-if="insights.hubs.notes.length > 0" class="note-list">
            <div v-for="note in insights.hubs.notes" :key="note.path" class="note-item" @click="openInObsidian(note.path)">
              <span class="note-path">{{ formatPath(note.path) }}</span>
              <span class="note-badge">{{ note.refs }} 引用</span>
            </div>
          </div>
          <div v-else class="empty-hint">暂无引用数据</div>
        </div>
      </div>

      <!-- 尘封笔记 -->
      <div class="insight-card">
        <div class="card-header">
          <span class="card-icon">📅</span>
          <div>
            <h3 class="card-title">尘封笔记</h3>
            <p class="card-desc">最久未修改的笔记</p>
          </div>
        </div>
        <div class="card-body">
          <div v-if="insights.dormant.notes.length > 0" class="note-list">
            <div v-for="note in insights.dormant.notes" :key="note.path" class="note-item" @click="openInObsidian(note.path)">
              <span class="note-path">{{ formatPath(note.path) }}</span>
              <span class="note-meta">{{ note.days_ago }}天未改</span>
            </div>
          </div>
          <div v-else class="empty-hint">暂无数据</div>
        </div>
      </div>

      <!-- 新生知识 -->
      <div class="insight-card">
        <div class="card-header">
          <span class="card-icon">🌱</span>
          <div>
            <h3 class="card-title">新生知识</h3>
            <p class="card-desc">最近创建的笔记</p>
          </div>
        </div>
        <div class="card-body">
          <div v-if="insights.fresh.notes.length > 0" class="note-list">
            <div v-for="note in insights.fresh.notes" :key="note.path" class="note-item" @click="openInObsidian(note.path)">
              <span class="note-path">{{ formatPath(note.path) }}</span>
              <span class="note-meta">{{ note.created }}</span>
            </div>
          </div>
          <div v-else class="empty-hint">暂无数据</div>
        </div>
      </div>

      <!-- 知识领域 -->
      <div class="insight-card domain-card">
        <div class="card-header">
          <span class="card-icon">📊</span>
          <div>
            <h3 class="card-title">知识领域</h3>
            <p class="card-desc">按文件夹的笔记分布</p>
          </div>
        </div>
        <div class="card-body">
          <div v-if="insights.domains.folders.length > 0" class="domain-list">
            <div v-for="folder in insights.domains.folders" :key="folder.folder" class="domain-item">
              <div class="domain-info">
                <span class="domain-name">{{ folder.folder }}</span>
                <span class="domain-count">{{ folder.count }} 篇 ({{ folder.percentage.toFixed(1) }}%)</span>
              </div>
              <div class="domain-bar">
                <div class="domain-bar-fill" :style="{ width: folder.percentage + '%' }"></div>
              </div>
            </div>
          </div>
          <div v-else class="empty-hint">暂无数据</div>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="无法加载知识库洞察" :image-size="80">
      <el-button type="primary" @click="loadInsights">重试</el-button>
    </el-empty>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getKnowledgeInsights } from '@/api'
import { Refresh, Loading } from '@element-plus/icons-vue'

interface NoteInfo {
  path: string
  modified: string
  days_ago: number
}

interface HubNote {
  path: string
  refs: number
  referenced_by: string[]
}

interface FreshNote {
  path: string
  created: string
}

interface FolderStat {
  folder: string
  count: number
  percentage: number
}

interface Insights {
  islands: { count: number; notes: NoteInfo[] }
  hubs: { notes: HubNote[] }
  dormant: { notes: NoteInfo[] }
  fresh: { notes: FreshNote[] }
  domains: { folders: FolderStat[] }
}

const loading = ref(false)
const insights = ref<Insights | null>(null)

async function loadInsights(force = false) {
  loading.value = true
  try {
    const res = await getKnowledgeInsights(force) as unknown as { result: Insights }
    insights.value = res.result
  } catch (e) {
    console.error('加载知识库洞察失败:', e)
    insights.value = null
  } finally {
    loading.value = false
  }
}

function formatPath(path: string): string {
  return path.replace(/\.md$/, '')
}

function openInObsidian(path: string) {
  window.open(`obsidian://open?path=${encodeURIComponent(path)}`, '_blank')
}

onMounted(() => { loadInsights() })
</script>

<style scoped>
.memory-page {
  max-width: 100%;
  min-height: 100%;
}

.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.insights-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
}

.insight-card {
  border-radius: 16px;
  padding: 20px;
  animation: pageFadeIn 0.5s ease both;
}
.insight-card:nth-child(2) { animation-delay: 0.05s; }
.insight-card:nth-child(3) { animation-delay: 0.1s; }
.insight-card:nth-child(4) { animation-delay: 0.15s; }
.insight-card:nth-child(5) { animation-delay: 0.2s; }

.domain-card {
  grid-column: span 1;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
}
.card-icon {
  font-size: 24px;
  flex-shrink: 0;
}
.card-title {
  font-size: 15px;
  font-weight: 600;
  color: #18181b;
  margin: 0;
}
.card-desc {
  font-size: 12px;
  color: #a1a1aa;
  margin: 2px 0 0;
}
.card-count {
  margin-left: auto;
  font-size: 22px;
  font-weight: 700;
  color: #6366f1;
}

.card-body {
  max-height: 280px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(0,0,0,0.06) transparent;
}

.note-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.note-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.15s ease;
}
.note-item:hover {
  background: rgba(99, 102, 241, 0.06);
}
.note-path {
  font-size: 13px;
  color: #27272a;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note-meta {
  font-size: 11px;
  color: #a1a1aa;
  flex-shrink: 0;
  margin-left: 8px;
}
.note-badge {
  font-size: 11px;
  color: #6366f1;
  font-weight: 600;
  flex-shrink: 0;
  margin-left: 8px;
}

.domain-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.domain-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.domain-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.domain-name {
  font-size: 13px;
  font-weight: 500;
  color: #27272a;
}
.domain-count {
  font-size: 11px;
  color: #a1a1aa;
}
.domain-bar {
  height: 6px;
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.04);
  overflow: hidden;
}
.domain-bar-fill {
  height: 100%;
  border-radius: 3px;
  background: linear-gradient(90deg, #818cf8, #6366f1);
  transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}

.empty-hint {
  text-align: center;
  color: #a1a1aa;
  font-size: 13px;
  padding: 20px 0;
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 60px 0;
  color: #71717a;
  font-size: 14px;
}

@keyframes pageFadeIn {
  from { opacity: 0; transform: translateY(16px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .insights-grid { grid-template-columns: 1fr; gap: 12px; }
  .insight-card { padding: 16px; border-radius: 14px; }
  .card-header { gap: 8px; margin-bottom: 10px; }
  .card-icon { font-size: 20px; }
  .card-title { font-size: 14px; }
  .card-desc { font-size: 11px; }
  .card-count { font-size: 18px; }
  .card-body { max-height: none; }
  .note-path { font-size: 12px; white-space: normal; overflow: visible; text-overflow: clip; word-break: break-all; }
  .note-meta, .note-badge { font-size: 10px; flex-shrink: 0; }
  .domain-info { font-size: 12px; }
}
</style>
