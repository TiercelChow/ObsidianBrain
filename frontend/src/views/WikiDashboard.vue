<template>
  <div class="wiki-dashboard">
    <header class="page-header">
      <div>
        <h1 class="page-title">Wiki 看板</h1>
        <p class="page-subtitle">知识库健康度、图谱与活动监控</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="refreshAll" :loading="loading">
          <el-icon v-if="!loading"><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </header>

    <div v-if="loading && !status" class="loading-state">
      <el-icon class="is-loading" :size="24"><Loading /></el-icon>
      <span>加载中...</span>
    </div>

    <template v-else>
      <!-- 统计概览 -->
      <div class="stats-grid">
        <div class="stat-card" v-for="s in statCards" :key="s.label">
          <div class="stat-icon" :style="{ color: s.color }">
            <span>{{ s.icon }}</span>
          </div>
          <div class="stat-content">
            <span class="stat-value">{{ s.value }}</span>
            <span class="stat-label">{{ s.label }}</span>
          </div>
        </div>
      </div>

      <!-- 未初始化提示 -->
      <div v-if="status && !status.initialized" class="init-banner">
        <div class="init-content">
          <span class="init-icon">📦</span>
          <div>
            <div class="init-title">Wiki 尚未初始化</div>
            <div class="init-desc">前往「Wiki 工作台」摄入第一篇资料，系统将自动创建 Wiki 目录结构</div>
          </div>
          <el-button size="small" type="primary" @click="$router.push('/wiki')">前往工作台</el-button>
        </div>
      </div>

      <template v-if="status && status.total_pages > 0">
        <!-- 知识健康度 + 领域分布 -->
        <div class="two-col">
          <!-- 健康度 -->
          <div class="insight-card">
            <div class="card-header">
              <span class="card-icon">🏥</span>
              <h3 class="card-title">知识健康度</h3>
              <el-button v-if="lintResult && lintResult.orphans.length > 0" size="small" text @click="$router.push('/wiki')">
                修复 →
              </el-button>
            </div>
            <div class="card-body" v-if="lintResult">
              <div class="health-row" v-if="lintResult.orphans.length > 0">
                <span class="health-icon red">🔴</span>
                <span class="health-label">孤岛页</span>
                <span class="health-value">{{ lintResult.orphans.length }}</span>
              </div>
              <div class="health-row" v-if="lintResult.missing_pages.length > 0">
                <span class="health-icon blue">🔵</span>
                <span class="health-label">缺失页面</span>
                <span class="health-value">{{ lintResult.missing_pages.length }}</span>
              </div>
              <div class="health-row" v-for="h in lintResult.hubs.slice(0, 3)" :key="h[0]">
                <span class="health-icon">🕸️</span>
                <span class="health-label">{{ formatPageName(h[0]) }}</span>
                <span class="health-value">{{ h[1] }} 引用</span>
              </div>
              <div class="health-row" v-if="lintResult.orphans.length === 0 && lintResult.missing_pages.length === 0">
                <span class="health-icon green">✅</span>
                <span class="health-label">一切正常</span>
              </div>
              <div v-if="lintResult.suggestions.length > 0" class="suggestions">
                <div class="suggestion-title">💡 建议</div>
                <div v-for="(s, i) in lintResult.suggestions.slice(0, 3)" :key="i" class="suggestion-item">{{ s }}</div>
              </div>
            </div>
            <div v-else-if="lintLoading" class="card-empty">
              <el-icon class="is-loading" :size="16"><Loading /></el-icon>
              <span>分析中...</span>
            </div>
            <div v-else class="card-empty">点击刷新加载</div>
          </div>

          <!-- 领域分布 -->
          <div class="insight-card">
            <div class="card-header">
              <span class="card-icon">📊</span>
              <h3 class="card-title">知识领域分布</h3>
            </div>
            <div class="card-body">
              <div class="domain-item" v-for="d in domainData" :key="d.label">
                <div class="domain-info">
                  <span class="domain-name">{{ d.icon }} {{ d.label }}</span>
                  <span class="domain-count">{{ d.value }} 页 ({{ d.percentage }}%)</span>
                </div>
                <div class="domain-bar">
                  <div class="domain-bar-fill" :style="{ width: d.percentage + '%', background: d.color }"></div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 最近操作时间线 -->
        <div class="insight-card full-width">
          <div class="card-header">
            <span class="card-icon">📅</span>
            <h3 class="card-title">最近操作</h3>
          </div>
          <div class="card-body">
            <div v-if="logEntries.length > 0" class="log-timeline">
              <div v-for="(entry, i) in logEntries" :key="i" class="log-entry">
                <div class="log-dot" :class="entry.type"></div>
                <div class="log-content">
                  <div class="log-header">
                    <span class="log-type" :class="entry.type">{{ entry.typeLabel }}</span>
                    <span class="log-date">{{ entry.date }}</span>
                  </div>
                  <div class="log-summary">{{ entry.summary }}</div>
                  <div v-if="entry.pages.length > 0" class="log-pages">
                    影响 {{ entry.pages.length }} 页
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="card-empty">暂无操作记录</div>
          </div>
        </div>
      </template>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Refresh, Loading } from '@element-plus/icons-vue'
import { getWikiStatus, lintWiki } from '@/api/wiki'
import { callTool } from '@/api'

interface WikiStatus {
  total_pages: number
  entities: number
  concepts: number
  sources: number
  synthesis: number
  initialized: boolean
}

interface LintResult {
  total_pages: number
  orphans: string[]
  missing_pages: string[]
  hubs: [string, number][]
  fixed: number
  suggestions: string[]
}

interface LogEntry {
  date: string
  type: string
  typeLabel: string
  summary: string
  pages: string[]
}

const loading = ref(false)
const lintLoading = ref(false)
const status = ref<WikiStatus | null>(null)
const lintResult = ref<LintResult | null>(null)
const logEntries = ref<LogEntry[]>([])

const statCards = computed(() => {
  if (!status.value) return []
  const s = status.value
  return [
    { icon: '📄', label: '总页数', value: s.total_pages, color: '#6366f1' },
    { icon: '👤', label: '实体', value: s.entities, color: '#10b981' },
    { icon: '💡', label: '概念', value: s.concepts, color: '#f59e0b' },
    { icon: '📚', label: '源摘要', value: s.sources, color: '#06b6d4' },
    { icon: '🔗', label: '综合论述', value: s.synthesis, color: '#ec4899' },
  ]
})

const domainData = computed(() => {
  if (!status.value) return []
  const s = status.value
  const total = s.total_pages || 1
  return [
    { icon: '👤', label: '实体', value: s.entities, percentage: Math.round(s.entities / total * 100), color: 'linear-gradient(90deg, #34d399, #10b981)' },
    { icon: '💡', label: '概念', value: s.concepts, percentage: Math.round(s.concepts / total * 100), color: 'linear-gradient(90deg, #fbbf24, #f59e0b)' },
    { icon: '📚', label: '源摘要', value: s.sources, percentage: Math.round(s.sources / total * 100), color: 'linear-gradient(90deg, #22d3ee, #06b6d4)' },
    { icon: '🔗', label: '综合论述', value: s.synthesis, percentage: Math.round(s.synthesis / total * 100), color: 'linear-gradient(90deg, #f472b6, #ec4899)' },
  ]
})

function formatPageName(path: string): string {
  return path.replace(/^Wiki\/[^/]+\//, '').replace(/\.md$/, '')
}

async function loadAll() {
  loading.value = true
  try {
    // Only load status + log on mount (fast, < 1s)
    const [statusRes] = await Promise.allSettled([
      getWikiStatus(),
    ])

    if (statusRes.status === 'fulfilled') {
      status.value = (statusRes.value as unknown as { result: WikiStatus }).result
    }

    await loadLog()
  } catch (e) {
    console.error('加载失败:', e)
  } finally {
    loading.value = false
  }
}

// Refresh button: load everything including lint
async function refreshAll() {
  loading.value = true
  try {
    const [statusRes] = await Promise.allSettled([getWikiStatus()])
    if (statusRes.status === 'fulfilled') {
      status.value = (statusRes.value as unknown as { result: WikiStatus }).result
    }
    await loadLog()
    await loadLintLazy()
  } catch (e) {
    console.error('刷新失败:', e)
  } finally {
    loading.value = false
  }
}

async function loadLog() {
  try {
    const res = await callTool('get_note', { path: 'Wiki/log.md' }) as unknown as { result: { content: string } }
    const content = res.result?.content || ''
    logEntries.value = parseLog(content)
  } catch {
    logEntries.value = []
  }
}

function parseLog(content: string): LogEntry[] {
  const entries: LogEntry[] = []
  const lines = content.split('\n')
  let current: Partial<LogEntry> | null = null
  let pages: string[] = []

  for (const line of lines) {
    const trimmed = line.trim()

    // ## [2026-06-24] ingest | 文章标题
    const match = trimmed.match(/^##\s*\[(\d{4}-\d{2}-\d{2})\]\s+(\w+)\s*\|\s*(.+)/)
    if (match) {
      if (current && current.date) {
        entries.push({ ...current, pages } as LogEntry)
      }
      const type = match[2]
      current = {
        date: match[1],
        type,
        typeLabel: type === 'ingest' ? '摄入' : type === 'query' ? '查询' : type === 'lint' ? '检查' : type,
        summary: match[3],
      }
      pages = []
      continue
    }

    // - 影响页面：xxx, yyy
    const pagesMatch = trimmed.match(/^-\s*影响页面：(.+)/)
    if (pagesMatch) {
      pages = pagesMatch[1].split(',').map((s: string) => s.trim()).filter(Boolean)
    }
  }

  if (current && current.date) {
    entries.push({ ...current, pages } as LogEntry)
  }

  return entries.reverse().slice(0, 10)
}

// Lazy load lint in background — doesn't block page render
async function loadLintLazy() {
  lintLoading.value = true
  try {
    const res = await lintWiki(false) as unknown as { result: LintResult }
    lintResult.value = res.result
  } catch (e) {
    console.error('Lint 加载失败:', e)
  } finally {
    lintLoading.value = false
  }
}

onMounted(() => { loadAll() })
</script>

<style scoped>
.wiki-dashboard { max-width: 100%; min-height: 100%; }
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-muted); font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.loading-state { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: var(--text-muted); }

.stats-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; margin-bottom: 24px; }
.stat-card { display: flex; align-items: center; gap: 12px; padding: 16px; border-radius: 14px; }
.stat-icon { width: 40px; height: 40px; display: flex; align-items: center; justify-content: center; font-size: 20px; background: var(--bg-glass-subtle); border-radius: 12px; }
.stat-value { font-size: 20px; font-weight: 700; color: var(--text-primary); }
.stat-label { font-size: 12px; color: var(--text-muted); display: block; margin-top: 2px; }

.init-banner { padding: 20px 24px; border-radius: 16px; margin-bottom: 24px; }
.init-content { display: flex; align-items: center; gap: 16px; }
.init-icon { font-size: 32px; }
.init-title { font-size: 15px; font-weight: 600; color: var(--text-primary); }
.init-desc { font-size: 13px; color: var(--text-muted); margin-top: 2px; }

.two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 16px; }
.insight-card { border-radius: 16px; padding: 20px; }
.insight-card.full-width { grid-column: span 2; }
.card-header { display: flex; align-items: center; gap: 8px; margin-bottom: 14px; }
.card-icon { font-size: 20px; }
.card-title { font-size: 15px; font-weight: 600; color: var(--text-primary); margin: 0; flex: 1; }
.card-body { display: flex; flex-direction: column; gap: 8px; }
.card-empty { text-align: center; color: var(--text-faint); font-size: 13px; padding: 20px 0; display: flex; align-items: center; justify-content: center; gap: 6px; }

.health-row { display: flex; align-items: center; gap: 8px; padding: 6px 0; }
.health-icon { font-size: 14px; width: 20px; }
.health-label { flex: 1; font-size: 13px; color: var(--text-secondary); }
.health-value { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.health-icon.green { color: #4ade80; }

.suggestions { margin-top: 8px; padding-top: 8px; border-top: 1px solid var(--border-faint); }
.suggestion-title { font-size: 13px; font-weight: 600; color: var(--text-tertiary); margin-bottom: 4px; }
.suggestion-item { font-size: 12px; color: var(--text-muted); padding: 2px 0; }

.domain-item { display: flex; flex-direction: column; gap: 4px; margin-bottom: 10px; }
.domain-info { display: flex; justify-content: space-between; align-items: center; }
.domain-name { font-size: 13px; font-weight: 500; color: var(--text-secondary); }
.domain-count { font-size: 11px; color: var(--text-muted); }
.domain-bar { height: 6px; border-radius: 3px; background: var(--bg-glass-subtle); overflow: hidden; }
.domain-bar-fill { height: 100%; border-radius: 3px; transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1); }

.log-timeline { display: flex; flex-direction: column; gap: 0; }
.log-entry { display: flex; gap: 12px; padding: 8px 0; position: relative; }
.log-dot { width: 10px; height: 10px; border-radius: 50%; margin-top: 5px; flex-shrink: 0; border: 2px solid var(--border-glass); background: var(--bg-glass-subtle); }
.log-dot.ingest { background: #6366f1; border-color: #818cf8; }
.log-dot.query { background: #10b981; border-color: #34d399; }
.log-dot.lint { background: #f59e0b; border-color: #fbbf24; }
.log-content { flex: 1; }
.log-header { display: flex; align-items: center; gap: 8px; margin-bottom: 2px; }
.log-type { font-size: 11px; font-weight: 600; padding: 1px 8px; border-radius: 6px; }
.log-type.ingest { background: rgba(99, 102, 241, 0.15); color: #818cf8; }
.log-type.query { background: rgba(16, 185, 129, 0.15); color: #34d399; }
.log-type.lint { background: rgba(245, 158, 11, 0.15); color: #fbbf24; }
.log-date { font-size: 12px; color: var(--text-faint); }
.log-summary { font-size: 13px; color: var(--text-secondary); }
.log-pages { font-size: 11px; color: var(--text-faint); margin-top: 2px; }

@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .stats-grid { grid-template-columns: repeat(2, 1fr); gap: 10px; }
  .stat-card { padding: 12px; gap: 8px; }
  .stat-value { font-size: 16px; }
  .two-col { grid-template-columns: 1fr; }
  .insight-card.full-width { grid-column: span 1; }
  .init-content { flex-direction: column; align-items: flex-start; gap: 8px; }
}
</style>
