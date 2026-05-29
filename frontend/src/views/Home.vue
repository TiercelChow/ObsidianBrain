<template>
  <div class="home-page">
    <!-- Header -->
    <header class="page-header">
      <div>
        <h1 class="page-title">控制面板</h1>
        <p class="page-subtitle">系统状态与模块概览</p>
      </div>
      <button class="refresh-btn" @click="loadHealth" :disabled="loading">
        <el-icon :size="14"><Refresh /></el-icon>
        <span>刷新</span>
      </button>
    </header>

    <!-- Stats -->
    <div class="stats-grid">
      <div
        v-for="(stat, i) in stats"
        :key="stat.label"
        class="stat-card"
        :style="{ '--delay': `${i * 0.05}s` }"
      >
        <div class="stat-icon" :style="{ color: stat.color }">
          <el-icon :size="20"><component :is="stat.icon" /></el-icon>
        </div>
        <div class="stat-content">
          <span class="stat-value">{{ stat.value }}</span>
          <span class="stat-label">{{ stat.label }}</span>
        </div>
      </div>
    </div>

    <!-- Modules -->
    <section class="section">
      <h2 class="section-title">功能模块</h2>
      <div class="modules-grid">
        <div
          v-for="(mod, i) in modules"
          :key="mod.title"
          class="module-card"
          :style="{ '--accent': mod.color, '--delay': `${0.1 + i * 0.05}s` }"
        >
          <div class="module-header">
            <div class="module-icon">
              <el-icon :size="18"><component :is="mod.icon" /></el-icon>
            </div>
            <el-tag
              :type="mod.status === '开发中' ? 'warning' : 'success'"
              size="small"
              effect="plain"
            >
              {{ mod.status }}
            </el-tag>
          </div>
          <h3 class="module-title">{{ mod.title }}</h3>
          <p class="module-desc">{{ mod.desc }}</p>
          <div class="module-tags">
            <span v-for="tag in mod.tags" :key="tag" class="tag">{{ tag }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- System Status -->
    <section class="section" style="--delay: 0.4s">
      <h2 class="section-title">系统状态</h2>
      <div class="status-card">
        <div v-if="health" class="status-grid">
          <div
            v-for="(status, name) in health.components"
            :key="name"
            class="status-item"
          >
            <span class="status-dot" :class="status === 'ok' ? 'ok' : 'inactive'"></span>
            <span class="status-name">{{ name }}</span>
            <span class="status-value" :class="status === 'ok' ? 'ok' : 'inactive'">
              {{ status === 'ok' ? '运行中' : '未配置' }}
            </span>
          </div>
        </div>
        <div v-else class="status-empty">
          <div class="loading-spinner"></div>
          <span>检测中...</span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getHealth } from '@/api'
import {
  Refresh,
  Notebook,
  FolderOpened,
  Calendar,
  MagicStick,
  DataLine,
  Connection,
} from '@element-plus/icons-vue'

interface Health {
  status: string
  version: string
  components: Record<string, string>
}

const health = ref<Health | null>(null)
const loading = ref(false)

const stats = [
  { icon: Notebook, label: '笔记总数', value: '—', color: '#6366f1' },
  { icon: Connection, label: '记忆单元', value: '—', color: '#8b5cf6' },
  { icon: FolderOpened, label: '代码仓库', value: '—', color: '#10b981' },
  { icon: DataLine, label: '雷达条目', value: '—', color: '#06b6d4' },
]

const modules = [
  {
    icon: Notebook, title: '记忆引擎',
    desc: '自动索引 Obsidian 笔记，提供全文 + 语义混合检索，支持记忆 CRUD。',
    tags: ['Tantivy', 'Qdrant', 'RRF'],
    color: '#6366f1',
    status: '开发中',
  },
  {
    icon: FolderOpened, title: '代码仓管理',
    desc: '注册本地 Git 仓库，展示元信息卡片，自动关联笔记并生成项目文档。',
    tags: ['git2', 'LLM', 'VSCode'],
    color: '#10b981',
    status: '开发中',
  },
  {
    icon: Calendar, title: '时间线',
    desc: '从笔记日期、文件名、Git 提交中聚合事件，按时间维度回顾知识演变。',
    tags: ['事件收集', '统计', '摘要'],
    color: '#f59e0b',
    status: '开发中',
  },
  {
    icon: MagicStick, title: '灵感熔炉',
    desc: '概念组合、反向提问、对立观点——三种模式故意制造知识碰撞与创意激发。',
    tags: ['TF-IDF', '概念距离', 'LLM'],
    color: '#ec4899',
    status: '开发中',
  },
  {
    icon: DataLine, title: '智识雷达',
    desc: '聚合 RSS/arXiv/HN 等外部源，基于个人知识图谱做个性化语义推荐。',
    tags: ['feed-rs', '语义排序', '纳藏'],
    color: '#06b6d4',
    status: '开发中',
  },
  {
    icon: Connection, title: '工具协议',
    desc: 'MCP + HTTP REST 双模式统一 API 层，供 Claude 等 LLM 前端调用。',
    tags: ['MCP', 'JSON-RPC', 'Axum'],
    color: '#71717a',
    status: '开发中',
  },
]

async function loadHealth() {
  loading.value = true
  try {
    const data = await getHealth() as unknown as Health
    health.value = data
  } catch {
    health.value = null
  } finally {
    loading.value = false
  }
}

onMounted(() => { loadHealth() })
</script>

<style scoped>
/* ── Page Header ── */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 32px;
}

.page-title {
  font-size: 22px;
  font-weight: 600;
  color: #18181b;
  letter-spacing: -0.3px;
}

.page-subtitle {
  margin-top: 4px;
  color: #a1a1aa;
  font-size: 14px;
}

.refresh-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border: 1px solid #e4e4e7;
  border-radius: 12px;
  background: #fff;
  color: #52525b;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.refresh-btn:hover:not(:disabled) {
  background: #f4f4f5;
  border-color: #d4d4d8;
}

.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* ── Stats Grid ── */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 40px;
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 20px;
  background: #fff;
  border: 1px solid #f0f0f0;
  border-radius: 18px;
  animation: fade-in 0.4s ease both;
  animation-delay: var(--delay, 0s);
}

.stat-icon {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f8fafc;
  border-radius: 14px;
  flex-shrink: 0;
}

.stat-content {
  display: flex;
  flex-direction: column;
}

.stat-value {
  font-size: 20px;
  font-weight: 600;
  color: #18181b;
  line-height: 1.2;
}

.stat-label {
  font-size: 13px;
  color: #a1a1aa;
  margin-top: 2px;
}

/* ── Section ── */
.section {
  margin-bottom: 40px;
  animation: fade-in 0.5s ease both;
  animation-delay: var(--delay, 0s);
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: #18181b;
  margin-bottom: 16px;
  letter-spacing: -0.2px;
}

/* ── Modules Grid ── */
.modules-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.module-card {
  padding: 20px;
  background: #fff;
  border: 1px solid #f0f0f0;
  border-radius: 18px;
  border-left: 3px solid var(--accent, #e4e4e7);
  animation: fade-in 0.4s ease both;
  animation-delay: var(--delay, 0s);
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
}

.module-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.04);
  border-color: #e4e4e7;
}

.module-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 12px;
}

.module-icon {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f8fafc;
  border-radius: 12px;
  color: var(--accent, #71717a);
}

.module-title {
  font-size: 15px;
  font-weight: 600;
  color: #18181b;
  margin-bottom: 6px;
}

.module-desc {
  font-size: 13px;
  color: #71717a;
  line-height: 1.6;
  margin-bottom: 14px;
}

.module-tags {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.tag {
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 8px;
  background: #f4f4f5;
  color: #52525b;
  font-weight: 500;
}

/* ── Status Card ── */
.status-card {
  padding: 20px;
  background: #fff;
  border: 1px solid #f0f0f0;
  border-radius: 18px;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 12px;
  background: #fafafa;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot.ok {
  background: #10b981;
}

.status-dot.inactive {
  background: #d4d4d8;
}

.status-name {
  flex: 1;
  font-size: 13px;
  color: #52525b;
  font-weight: 500;
  text-transform: capitalize;
}

.status-value {
  font-size: 12px;
  font-weight: 500;
}

.status-value.ok {
  color: #10b981;
}

.status-value.inactive {
  color: #a1a1aa;
}

.status-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 20px;
  color: #a1a1aa;
  font-size: 13px;
}

.loading-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid #e4e4e7;
  border-top-color: #6366f1;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ── Animations ── */
@keyframes fade-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
