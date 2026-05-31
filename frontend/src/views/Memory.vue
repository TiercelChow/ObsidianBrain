<template>
  <div class="memory-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">记忆管理</h1>
        <p class="page-subtitle">自动索引 Obsidian 笔记，提供全文与语义混合检索</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadStats" :loading="statsLoading" size="small">
          <el-icon><Refresh /></el-icon> 刷新统计
        </el-button>
      </div>
    </header>

    <!-- Stats -->
    <div class="stats-row" v-if="stats">
      <div class="stat-chip">
        <span class="stat-num">{{ stats.total_notes }}</span>
        <span class="stat-label">笔记</span>
      </div>
      <div class="stat-chip">
        <span class="stat-num">{{ stats.total_chunks }}</span>
        <span class="stat-label">分块</span>
      </div>
      <div class="stat-chip">
        <span class="stat-num">{{ stats.tags?.length || 0 }}</span>
        <span class="stat-label">标签</span>
      </div>
    </div>

    <!-- Search -->
    <section class="search-section">
      <div class="search-bar">
        <el-input
          v-model="searchQuery"
          placeholder="搜索笔记..."
          size="large"
          clearable
          @keyup.enter="doSearch"
        >
          <template #prefix>
            <el-icon><Search /></el-icon>
          </template>
        </el-input>
        <el-button type="primary" size="large" @click="doSearch" :loading="searching" class="search-btn">
          搜索
        </el-button>
      </div>

      <div class="search-options">
        <el-input-number v-model="topK" :min="1" :max="20" size="small" />
        <span class="option-label">条结果</span>
        <el-select v-model="selectedTags" multiple placeholder="标签过滤" size="small" clearable class="tag-select">
          <el-option v-for="tag in allTags" :key="tag" :label="tag" :value="tag" />
        </el-select>
      </div>
    </section>

    <!-- Results -->
    <section class="results-section" v-if="searchResults.length > 0">
      <h3 class="results-title">搜索结果 ({{ searchResults.length }})</h3>
      <div class="results-list">
        <div class="result-card" v-for="(result, i) in searchResults" :key="i">
          <div class="result-header">
            <span class="result-title">{{ result.title || result.note_path }}</span>
            <el-tag size="small" effect="plain">
              得分: {{ (result.score || result.rrf_score || 0).toFixed(3) }}
            </el-tag>
          </div>
          <div class="result-path">
            <el-icon><Document /></el-icon>
            {{ result.note_path || result.path }}
          </div>
          <p class="result-snippet">{{ result.snippet || result.content }}</p>
          <div class="result-meta" v-if="result.tags?.length">
            <el-tag v-for="tag in result.tags" :key="tag" size="small" effect="plain" class="result-tag">
              {{ tag }}
            </el-tag>
          </div>
          <div class="result-actions" v-if="result.obsidian_uri">
            <a :href="result.obsidian_uri" class="obsidian-link" target="_blank">
              在 Obsidian 中打开
            </a>
          </div>
        </div>
      </div>
    </section>

    <section class="results-section" v-else-if="hasSearched && !searching">
      <el-empty description="没有找到相关笔记" :image-size="80" />
    </section>

    <!-- Recent Notes -->
    <section class="recent-section" v-if="recentNotes.length > 0">
      <h3 class="section-title">最近笔记 ({{ recentNotes.length }})</h3>
      <div class="recent-list">
        <div class="recent-card" v-for="(note, i) in recentNotes" :key="i">
          <div class="recent-title">{{ note.title || note.path }}</div>
          <div class="recent-path">{{ note.path }}</div>
          <div class="recent-meta">
            <span v-if="note.updated_at">{{ formatDate(note.updated_at) }}</span>
            <el-tag v-for="tag in (note.tags || []).slice(0, 3)" :key="tag" size="small" effect="plain">
              {{ tag }}
            </el-tag>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { searchNotes, getMemoryStats, listRecentNotes } from '@/api'
import { Search, Document, Refresh } from '@element-plus/icons-vue'

interface SearchItem {
  title?: string
  note_path?: string
  path?: string
  snippet?: string
  content?: string
  score?: number
  rrf_score?: number
  tags?: string[]
  obsidian_uri?: string
}

interface NoteItem {
  title?: string
  path: string
  tags?: string[]
  updated_at?: string
}

const searchQuery = ref('')
const topK = ref(5)
const selectedTags = ref<string[]>([])
const allTags = ref<string[]>([])
const searchResults = ref<SearchItem[]>([])
const recentNotes = ref<NoteItem[]>([])
const searching = ref(false)
const statsLoading = ref(false)
const hasSearched = ref(false)
const stats = ref<{ total_chunks: number; total_notes: number; tags: string[] } | null>(null)

async function doSearch() {
  if (!searchQuery.value.trim()) return
  searching.value = true
  hasSearched.value = true
  try {
    const res = await searchNotes(
      searchQuery.value,
      topK.value,
      selectedTags.value.length > 0 ? selectedTags.value : undefined
    ) as unknown as { result: { notes: SearchItem[] } }
    searchResults.value = res.result?.notes || res.result as unknown as SearchItem[] || []
  } catch (e) {
    console.error('搜索失败:', e)
    searchResults.value = []
  } finally {
    searching.value = false
  }
}

async function loadStats() {
  statsLoading.value = true
  try {
    const res = await getMemoryStats() as unknown as { result: { total_chunks: number; total_notes: number; tags: string[] } }
    const data = res.result || res as unknown as { total_chunks: number; total_notes: number; tags: string[] }
    stats.value = data
    allTags.value = data.tags || []
  } catch {
    stats.value = null
  } finally {
    statsLoading.value = false
  }
}

async function loadRecent() {
  try {
    const res = await listRecentNotes(7, 10) as unknown as { result: { notes: NoteItem[] } }
    recentNotes.value = res.result?.notes || []
  } catch {
    recentNotes.value = []
  }
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    return d.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
  } catch {
    return dateStr
  }
}

onMounted(() => {
  loadStats()
  loadRecent()
})
</script>

<style scoped>
.memory-page { max-width: 100%; }
.page-header {
  display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px;
}
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }

.stats-row { display: flex; gap: 16px; margin-bottom: 24px; }
.stat-chip {
  display: flex; align-items: baseline; gap: 6px;
  padding: 8px 16px; background: #fff; border: 1px solid #f0f0f0; border-radius: 12px;
}
.stat-num { font-size: 18px; font-weight: 600; color: #18181b; }
.stat-label { font-size: 12px; color: #a1a1aa; }

.search-section { margin-bottom: 24px; }
.search-bar { display: flex; gap: 12px; margin-bottom: 12px; }
.search-btn { min-width: 80px; }
.search-options { display: flex; align-items: center; gap: 8px; }
.option-label { font-size: 13px; color: #71717a; }
.tag-select { min-width: 200px; }

.results-section { margin-bottom: 32px; }
.results-title, .section-title {
  font-size: 15px; font-weight: 600; color: #18181b; margin-bottom: 12px;
}
.results-list { display: flex; flex-direction: column; gap: 12px; }
.result-card {
  padding: 16px 20px; background: #fff; border: 1px solid #f0f0f0; border-radius: 14px;
  transition: box-shadow 0.2s ease;
}
.result-card:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.04); }
.result-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; }
.result-title { font-size: 15px; font-weight: 600; color: #18181b; }
.result-path {
  display: flex; align-items: center; gap: 4px;
  font-size: 12px; color: #a1a1aa; margin-bottom: 8px; font-family: monospace;
}
.result-snippet { font-size: 13px; color: #52525b; line-height: 1.6; margin-bottom: 8px; }
.result-meta { display: flex; gap: 4px; flex-wrap: wrap; margin-bottom: 6px; }
.result-tag { font-size: 11px; }
.result-actions { margin-top: 4px; }
.obsidian-link { font-size: 12px; color: #6366f1; text-decoration: none; }
.obsidian-link:hover { text-decoration: underline; }

.recent-list { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; }
.recent-card {
  padding: 12px 16px; background: #fff; border: 1px solid #f0f0f0; border-radius: 12px;
}
.recent-title { font-size: 14px; font-weight: 500; color: #18181b; margin-bottom: 4px; }
.recent-path { font-size: 12px; color: #a1a1aa; font-family: monospace; margin-bottom: 6px; }
.recent-meta { display: flex; gap: 6px; align-items: center; font-size: 12px; color: #71717a; }
</style>
