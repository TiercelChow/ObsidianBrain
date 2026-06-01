<template>
  <div class="timeline-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">时间线</h1>
        <p class="page-subtitle">知识演变的时间维度可视化</p>
      </div>
      <div class="header-actions">
        <el-date-picker
          v-model="dateRange"
          type="daterange"
          range-separator="至"
          start-placeholder="开始日期"
          end-placeholder="结束日期"
          size="small"
          @change="loadTimeline"
        />
        <el-button @click="loadTimeline" :loading="loading" size="small">
          <el-icon><Refresh /></el-icon> 刷新
        </el-button>
      </div>
    </header>

    <!-- 统计概览 -->
    <div class="stats-row" v-if="stats">
      <div class="stat-card">
        <div class="stat-value">{{ stats.total_events }}</div>
        <div class="stat-label">总事件</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">{{ stats.active_days }}</div>
        <div class="stat-label">活跃天数</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">{{ stats.daily_average?.toFixed(1) }}</div>
        <div class="stat-label">日均事件</div>
      </div>
    </div>

    <!-- 标签云 -->
    <div class="tags-section" v-if="stats?.most_active_tags?.length">
      <h3>高频标签</h3>
      <div class="tag-cloud">
        <el-tag v-for="tag in stats.most_active_tags" :key="tag" size="small" effect="plain">
          {{ tag }}
        </el-tag>
      </div>
    </div>

    <!-- 事件类型分布 -->
    <div class="type-dist" v-if="stats?.by_type && Object.keys(stats.by_type).length > 0">
      <h3>事件类型分布</h3>
      <div class="type-bars">
        <div v-for="(count, type) in stats.by_type" :key="type" class="type-bar-item">
          <span class="type-name">{{ formatEventType(type as string) }}</span>
          <el-progress :percentage="Math.round((count / stats.total_events) * 100)" :stroke-width="8" />
          <span class="type-count">{{ count }}</span>
        </div>
      </div>
    </div>

    <!-- 时间线事件列表 -->
    <div class="events-section" v-if="dailyEvents.length > 0">
      <h3>事件列表</h3>
      <div class="timeline-list">
        <div v-for="day in dailyEvents" :key="day.date" class="timeline-day">
          <div class="day-header">
            <span class="day-date">{{ formatDate(day.date) }}</span>
            <span class="day-count">{{ day.event_count }} 个事件</span>
          </div>
          <div class="day-events">
            <div v-for="event in day.events" :key="event.id" class="event-item">
              <div class="event-icon">
                {{ getEventIcon(event.event_type) }}
              </div>
              <div class="event-content">
                <div class="event-title">{{ event.title }}</div>
                <div class="event-summary" v-if="event.summary">{{ event.summary }}</div>
                <div class="event-tags" v-if="event.tags?.length">
                  <el-tag v-for="tag in event.tags" :key="tag" size="small" effect="plain">
                    {{ tag }}
                  </el-tag>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="暂无时间线事件" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getTimeline } from '@/api'
import { Refresh } from '@element-plus/icons-vue'

interface TimelineEvent {
  id: string
  event_type: string
  title: string
  summary: string
  tags: string[]
  related_paths: string[]
}

interface DailyEvents {
  date: string
  event_count: number
  events: TimelineEvent[]
}

interface TimelineStats {
  total_events: number
  active_days: number
  daily_average: number
  most_active_tags: string[]
  by_type: Record<string, number>
}

const dateRange = ref<[Date, Date]>([
  new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
  new Date()
])
const dailyEvents = ref<DailyEvents[]>([])
const stats = ref<TimelineStats | null>(null)
const loading = ref(false)

async function loadTimeline() {
  if (!dateRange.value) return

  loading.value = true
  try {
    const start = dateRange.value[0].toISOString().split('T')[0]
    const end = dateRange.value[1].toISOString().split('T')[0]

    const res = await getTimeline(start, end) as unknown as {
      result: { daily_events: DailyEvents[]; statistics: TimelineStats }
    }

    dailyEvents.value = res.result?.daily_events || []
    stats.value = res.result?.statistics || null
  } catch (e) {
    console.error('加载时间线失败:', e)
    dailyEvents.value = []
    stats.value = null
  } finally {
    loading.value = false
  }
}

function formatDate(dateStr: string): string {
  const d = new Date(dateStr)
  return d.toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'short' })
}

function formatEventType(type: string): string {
  const map: Record<string, string> = {
    note_created: '新建笔记',
    note_modified: '修改笔记',
    repo_commit: '代码提交',
    radar_saved: '保存文章',
    memory_created: '创建记忆'
  }
  return map[type] || type
}

function getEventIcon(type: string): string {
  const map: Record<string, string> = {
    note_created: '📝',
    note_modified: '✏️',
    repo_commit: '💻',
    radar_saved: '💾',
    memory_created: '🧠'
  }
  return map[type] || '📋'
}

onMounted(() => { loadTimeline() })
</script>

<style scoped>
.timeline-page { max-width: 100%; }
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.stats-row { display: flex; gap: 16px; margin-bottom: 24px; }
.stat-card { display: flex; flex-direction: column; padding: 16px 24px; background: #fff; border: 1px solid #f0f0f0; border-radius: 16px; min-width: 100px; }
.stat-value { font-size: 24px; font-weight: 700; color: #18181b; }
.stat-label { font-size: 12px; color: #a1a1aa; margin-top: 4px; }

.tags-section, .type-dist, .events-section { margin-bottom: 24px; }
.tags-section h3, .type-dist h3, .events-section h3 { font-size: 15px; font-weight: 600; color: #18181b; margin-bottom: 12px; }
.tag-cloud { display: flex; gap: 8px; flex-wrap: wrap; }

.type-bars { display: flex; flex-direction: column; gap: 8px; }
.type-bar-item { display: flex; align-items: center; gap: 12px; }
.type-name { font-size: 13px; color: #52525b; min-width: 80px; }
.type-count { font-size: 13px; color: #a1a1aa; min-width: 30px; text-align: right; }

.timeline-list { display: flex; flex-direction: column; gap: 16px; }
.timeline-day { border-left: 2px solid #e4e4e7; padding-left: 16px; }
.day-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.day-date { font-size: 14px; font-weight: 600; color: #18181b; }
.day-count { font-size: 12px; color: #a1a1aa; }

.day-events { display: flex; flex-direction: column; gap: 8px; }
.event-item { display: flex; gap: 12px; padding: 12px; background: #fff; border-radius: 12px; }
.event-icon { width: 36px; height: 36px; border-radius: 10px; display: flex; align-items: center; justify-content: center; font-size: 18px; background: #f4f4f5; flex-shrink: 0; }
.event-content { flex: 1; }
.event-title { font-size: 14px; font-weight: 500; color: #18181b; }
.event-summary { font-size: 13px; color: #71717a; margin-top: 4px; }
.event-tags { display: flex; gap: 4px; margin-top: 6px; }
</style>
