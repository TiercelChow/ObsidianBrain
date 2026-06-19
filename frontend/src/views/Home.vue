<template>
  <div class="home-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">控制面板</h1>
        <p class="page-subtitle">系统状态与配置管理</p>
      </div>
      <el-button size="small" @click="loadAll" :loading="loading">
        <el-icon v-if="!loading"><Refresh /></el-icon>
        刷新
      </el-button>
    </header>

    <!-- Stats -->
    <div class="stats-grid">
      <div class="stat-card" v-for="(stat, i) in stats" :key="stat.label"
        :style="{ '--delay': `${i * 0.05}s` }">
        <div class="stat-icon" :style="{ color: stat.color }">
          <el-icon :size="20"><component :is="stat.icon" /></el-icon>
        </div>
        <div class="stat-content">
          <span class="stat-value">{{ stat.value }}</span>
          <span class="stat-label">{{ stat.label }}</span>
        </div>
      </div>
    </div>

    <!-- System Status -->
    <section class="section">
      <h2 class="section-title">系统状态</h2>
      <div class="status-card">
        <div class="status-grid" v-if="health">
          <div class="status-item" v-for="(status, name) in health.components" :key="name">
            <span class="status-dot" :class="status === 'ok' ? 'ok' : 'inactive'"></span>
            <span class="status-name">{{ name }}</span>
            <span class="status-value" :class="status === 'ok' ? 'ok' : 'inactive'">
              {{ status === 'ok' ? '运行中' : status }}
            </span>
          </div>
          <div class="status-item">
            <span class="status-dot ok"></span>
            <span class="status-name">运行时间</span>
            <span class="status-value ok">{{ formatUptime(health.uptime_seconds) }}</span>
          </div>
        </div>
        <div v-else class="status-empty">
          <div class="loading-spinner"></div>
          <span>检测中...</span>
        </div>
      </div>
    </section>

    <!-- Configuration -->
    <section class="section">
      <h2 class="section-title">系统配置</h2>
      <div class="config-card" v-if="config">
        <!-- Vault -->
        <div class="config-group">
          <h3 class="config-group-title">
            <el-icon><FolderOpened /></el-icon>
            Obsidian Vault
          </h3>
          <div class="config-fields">
            <div class="config-field">
              <label>Vault 路径</label>
              <el-input v-model="config.vault.path" size="small" placeholder="/path/to/vault" />
            </div>
            <div class="config-field small">
              <label>Vault 名称</label>
              <el-input v-model="config.vault.name" size="small" placeholder="my-vault" />
            </div>
          </div>
        </div>

        <!-- Obsidian API -->
        <div class="config-group">
          <h3 class="config-group-title">
            <el-icon><Connection /></el-icon>
            Obsidian REST API
          </h3>
          <div class="config-fields">
            <div class="config-field inline">
              <label>启用</label>
              <el-switch v-model="config.obsidian.enabled" />
            </div>
            <div class="config-field">
              <label>API 地址</label>
              <el-input v-model="config.obsidian.url" size="small" placeholder="https://127.0.0.1:27124" />
            </div>
            <div class="config-field">
              <label>API Key</label>
              <el-input v-model="config.obsidian.api_key" size="small" placeholder="API Key" show-password />
            </div>
          </div>
        </div>

        <!-- LLM -->
        <div class="config-group">
          <h3 class="config-group-title">
            <el-icon><MagicStick /></el-icon>
            LLM 配置
          </h3>
          <div class="config-fields">
            <div class="config-field inline">
              <label>提供商</label>
              <el-select v-model="config.llm.provider" size="small" style="width: 140px">
                <el-option label="OpenAI" value="openai" />
                <el-option label="Claude" value="claude" />
                <el-option label="Ollama" value="ollama" />
              </el-select>
            </div>
            <div class="config-field">
              <label>模型</label>
              <el-input v-model="config.llm.model" size="small" placeholder="gpt-4o-mini" />
            </div>
            <div class="config-field inline">
              <label>最大 Token</label>
              <el-input-number v-model="config.llm.max_tokens" size="small" :min="256" :max="8192" :step="256" />
            </div>
            <div class="config-field inline">
              <label>温度</label>
              <el-slider v-model="config.llm.temperature" :min="0" :max="2" :step="0.1" style="width: 160px" />
              <span class="slider-value">{{ config.llm.temperature }}</span>
            </div>
          </div>
        </div>

        <div class="config-actions">
          <el-button type="primary" size="small" @click="saveSettings" :loading="saving">
            保存配置
          </el-button>
          <span class="config-hint">配置保存后需重启服务生效</span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { getHealth, getMemoryStats, getMemoStats, getConfig, saveConfig } from '@/api'
import {
  Refresh, Notebook, FolderOpened, Calendar,
  MagicStick, DataLine, Connection,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

interface HealthData {
  status: string
  version: string
  uptime_seconds: number
  tools_count: number
  vault: { path: string; exists: boolean; watching: boolean }
  components: Record<string, string>
}

interface ConfigData {
  vault: { path: string; name: string }
  obsidian: { enabled: boolean; url: string; api_key: string }
  llm: { provider: string; model: string; max_tokens: number; temperature: number }
}

const health = ref<HealthData | null>(null)
const memStats = ref<{ total_chunks: number; total_notes: number; tags: string[] } | null>(null)
const memoStats = ref<{ total_memos: number } | null>(null)
const config = ref<ConfigData | null>(null)
const loading = ref(false)
const saving = ref(false)

const stats = computed(() => [
  { icon: Notebook, label: '笔记总数', value: memStats.value?.total_notes ?? '—', color: '#6366f1' },
  { icon: DataLine, label: '已注册工具', value: health.value?.tools_count ?? '—', color: '#06b6d4' },
  { icon: Calendar, label: '小记数', value: memoStats.value?.total_memos ?? '—', color: '#10b981' },
])

function formatUptime(seconds: number): string {
  if (!seconds) return '—'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m`
  return `${seconds % 60}s`
}

async function loadAll() {
  loading.value = true
  try {
    const [healthRes, memRes, memoRes, configRes] = await Promise.allSettled([
      getHealth(),
      getMemoryStats(),
      getMemoStats(),
      getConfig(),
    ])
    if (healthRes.status === 'fulfilled') health.value = healthRes.value as unknown as HealthData
    if (memRes.status === 'fulfilled') {
      const r = memRes.value as unknown as { result: { total_chunks: number; total_notes: number; tags: string[] } }
      memStats.value = r.result ?? null
    }
    if (memoRes.status === 'fulfilled') {
      const r = memoRes.value as unknown as { result: { total_memos: number } }
      memoStats.value = r.result ?? null
    }
    if (configRes.status === 'fulfilled') {
      const r = configRes.value as unknown as { result: ConfigData }
      config.value = r.result ?? null
    }
  } finally {
    loading.value = false
  }
}

async function saveSettings() {
  if (!config.value) return
  saving.value = true
  try {
    await saveConfig(config.value)
    ElMessage.success('配置已保存，重启服务后生效')
  } catch (e) {
    ElMessage.error('保存失败')
  } finally {
    saving.value = false
  }
}

onMounted(() => { loadAll() })
</script>

<style scoped>
.home-page {
  min-height: 100%;
  max-width: 100%;
}
.page-header {
  display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px;
}
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }

.stats-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-bottom: 28px; }
.stat-card {
  display: flex; align-items: center; gap: 14px; padding: 20px;
  border-radius: 18px;
  animation: fade-in 0.4s ease both; animation-delay: var(--delay, 0s);
}
.stat-icon {
  width: 40px; height: 40px; display: flex; align-items: center; justify-content: center;
  background: #f8fafc; border-radius: 14px; flex-shrink: 0;
}
.stat-content { display: flex; flex-direction: column; }
.stat-value { font-size: 20px; font-weight: 600; color: #18181b; line-height: 1.2; }
.stat-label { font-size: 13px; color: #a1a1aa; margin-top: 2px; }

.section { margin-bottom: 28px; animation: fade-in 0.5s ease both; animation-delay: var(--delay, 0s); }
.section-title { font-size: 15px; font-weight: 600; color: #18181b; margin-bottom: 12px; letter-spacing: -0.2px; }

.status-card { padding: 16px 20px; border-radius: 16px; }
.status-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 8px; }
.status-item {
  display: flex; align-items: center; gap: 10px; padding: 8px 12px;
  border-radius: 10px; background: rgba(0,0,0,0.02);
}
.status-dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
.status-dot.ok { background: #10b981; }
.status-dot.inactive { background: #d4d4d8; }
.status-name { flex: 1; font-size: 13px; color: #52525b; font-weight: 500; text-transform: capitalize; }
.status-value { font-size: 12px; font-weight: 500; }
.status-value.ok { color: #10b981; }
.status-value.inactive { color: #a1a1aa; }
.status-empty { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 20px; color: #a1a1aa; font-size: 13px; }
.loading-spinner {
  width: 14px; height: 14px; border: 2px solid #e4e4e7; border-top-color: #6366f1;
  border-radius: 50%; animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* Config */
.config-card { padding: 20px; border-radius: 16px; }
.config-group { margin-bottom: 20px; }
.config-group:last-of-type { margin-bottom: 16px; }
.config-group-title {
  display: flex; align-items: center; gap: 6px;
  font-size: 14px; font-weight: 600; color: #18181b;
  margin-bottom: 10px;
}
.config-fields { display: flex; flex-direction: column; gap: 8px; }
.config-field { display: flex; flex-direction: column; gap: 4px; }
.config-field label { font-size: 12px; color: #71717a; font-weight: 500; }
.config-field.inline { flex-direction: row; align-items: center; gap: 10px; }
.config-field.inline label { min-width: 70px; }
.config-field.small { max-width: 200px; }
.slider-value { font-size: 13px; color: #18181b; font-weight: 600; min-width: 28px; }
.config-actions { display: flex; align-items: center; gap: 12px; padding-top: 8px; border-top: 1px solid rgba(0,0,0,0.04); }
.config-hint { font-size: 12px; color: #a1a1aa; }

@keyframes fade-in {
  from { opacity: 0; transform: translateY(20px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

/* ── Mobile ── */
@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .stats-grid { grid-template-columns: 1fr; gap: 10px; margin-bottom: 20px; }
  .stat-card { padding: 14px; gap: 10px; border-radius: 14px; }
  .stat-icon { width: 32px; height: 32px; border-radius: 10px; }
  .stat-value { font-size: 16px; }
  .stat-label { font-size: 11px; }
  .section { margin-bottom: 20px; }
  .section-title { font-size: 14px; margin-bottom: 10px; }
  .status-card { padding: 12px 14px; border-radius: 14px; }
  .status-grid { grid-template-columns: 1fr; gap: 6px; }
  .status-item { padding: 6px 10px; }
  .config-card { padding: 16px; border-radius: 14px; }
  .config-field.small { max-width: 100%; }
}
</style>
