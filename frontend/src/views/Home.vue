<template>
  <div class="home-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">控制面板</h1>
        <p class="page-subtitle">系统状态与配置管理</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="appStore.toggleTheme()">
          <el-icon><component :is="appStore.theme === 'dark' ? Sunny : Moon" /></el-icon>
          {{ appStore.theme === 'dark' ? '浅色' : '深色' }}
        </el-button>
        <el-button size="small" @click="loadAll" :loading="loading">
          <el-icon v-if="!loading"><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
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
                <el-option label="OpenAI 兼容" value="openai" />
                <el-option label="Ollama (本地)" value="ollama" />
              </el-select>
            </div>
            <div class="config-field">
              <label>模型名称</label>
              <el-input v-model="config.llm.model" size="small" placeholder="glm-5.2 / gpt-4o-mini / qwen2.5" />
            </div>
            <div class="config-field">
              <label>API Key</label>
              <el-input v-model="config.llm.api_key" size="small" placeholder="直接输入 API Key" show-password />
              <span class="field-hint">密钥保存后不再显示，留空则使用环境变量</span>
            </div>
            <div class="config-field">
              <label>API Key 环境变量名（可选）</label>
              <el-input v-model="config.llm.api_key_env" size="small" placeholder="如 ANTHROPIC_AUTH_TOKEN（API Key 为空时使用）" />
            </div>
            <div class="config-field">
              <label>API Base URL</label>
              <el-input v-model="config.llm.base_url" size="small" placeholder="如 https://dashscope.aliyuncs.com/compatible-mode/v1" />
              <span class="field-hint">OpenAI 官方留空即可，第三方服务填兼容地址</span>
            </div>
            <div class="config-field inline">
              <label>最大 Token</label>
              <el-input-number v-model="config.llm.max_tokens" size="small" :min="256" :max="32768" :step="256" />
            </div>
            <div class="config-field inline">
              <label>温度</label>
              <el-slider v-model="config.llm.temperature" :min="0" :max="2" :step="0.1" style="width: 160px" />
              <span class="slider-value">{{ config.llm.temperature }}</span>
            </div>
          </div>
        </div>

        <div class="config-actions">
          <el-button size="small" @click="verifyLlm" :loading="verifyingLlm">
            验证 LLM
          </el-button>
          <el-button type="primary" size="small" @click="saveSettings" :loading="saving">
            保存配置
          </el-button>
          <span class="config-hint">配置保存后立即生效（热更新）</span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { getHealth, getMemoryStats, getMemoStats, getConfig, saveConfig, listCodeRepos, callTool } from '@/api'
import { useAppStore } from '@/stores/app'
import {
  Refresh, Notebook, FolderOpened, Calendar,
  MagicStick, DataLine, Connection, Sunny, Moon,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

const appStore = useAppStore()

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
  llm: { provider: string; model: string; api_key: string; api_key_env: string; base_url: string; max_tokens: number; temperature: number }
}

const health = ref<HealthData | null>(null)
const memStats = ref<{ total_chunks: number; total_notes: number; tags: string[] } | null>(null)
const memoStats = ref<{ total_memos: number } | null>(null)
const config = ref<ConfigData | null>(null)
const repoCount = ref<number | null>(null)
const loading = ref(false)
const saving = ref(false)
const verifyingLlm = ref(false)

const stats = computed(() => [
  { icon: Notebook, label: '笔记总数', value: memStats.value?.total_notes ?? '—', color: '#6366f1' },
  { icon: DataLine, label: '已注册工具', value: health.value?.tools_count ?? '—', color: '#06b6d4' },
  { icon: Calendar, label: '小记数', value: memoStats.value?.total_memos ?? '—', color: '#10b981' },
  { icon: FolderOpened, label: '代码仓', value: repoCount.value ?? '—', color: '#f59e0b' },
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
    const [healthRes, memRes, memoRes, configRes, reposRes] = await Promise.allSettled([
      getHealth(),
      getMemoryStats(),
      getMemoStats(),
      getConfig(),
      listCodeRepos(),
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
    if (reposRes.status === 'fulfilled') {
      const r = reposRes.value as unknown as { result: unknown }
      const repos = r.result
      repoCount.value = Array.isArray(repos) ? repos.length : 0
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
    ElMessage.success('配置已保存并生效')
  } catch (e) {
    ElMessage.error('保存失败')
  } finally {
    saving.value = false
  }
}

async function verifyLlm() {
  if (!config.value) return
  verifyingLlm.value = true
  try {
    const res = await callTool('verify_llm', {
      provider: config.value.llm.provider,
      model: config.value.llm.model,
      api_key: config.value.llm.api_key,
      api_key_env: config.value.llm.api_key_env,
      base_url: config.value.llm.base_url,
    }) as unknown as { result: { valid: boolean; message: string; response?: string } }
    const r = res.result
    if (r.valid) {
      ElMessage.success(`✅ ${r.message}（回复：${r.response || ''}）`)
    } else {
      ElMessage.error(`❌ ${r.message}`)
    }
  } catch (e: any) {
    ElMessage.error('验证失败：' + (e?.message || '未知错误'))
  } finally {
    verifyingLlm.value = false
  }
}

onMounted(() => { loadAll() })
</script>

<style scoped>
.home-page {
  min-height: 100%;
  max-width: 100%;
}

.stats-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; margin-bottom: 28px; }
.stat-card {
  display: flex; align-items: center; gap: 14px; padding: 20px;
  border-radius: 18px;
  animation: fade-in var(--duration-normal) var(--ease-out) both; animation-delay: var(--delay, 0s);
}
.stat-icon {
  width: 44px; height: 44px; display: flex; align-items: center; justify-content: center;
  background: var(--bg-glass-subtle); backdrop-filter: blur(8px);
  border: 1px solid var(--border-subtle);
  border-radius: 14px; flex-shrink: 0;
}
.stat-content { display: flex; flex-direction: column; }
.stat-value { font-size: 22px; font-weight: 700; color: var(--text-primary); line-height: 1.2; }
.stat-label { font-size: 12px; color: var(--text-faint); margin-top: 3px; font-weight: 500; }

.section { margin-bottom: 28px; animation: fade-in var(--duration-normal) var(--ease-out) both; animation-delay: var(--delay, 0s); }
.section-title { font-size: 15px; font-weight: 600; color: var(--text-primary); margin-bottom: 12px; letter-spacing: var(--tracking-tight); }

.status-card { padding: 16px 20px; border-radius: 16px; }
.status-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 8px; }
.status-item {
  display: flex; align-items: center; gap: 10px; padding: 8px 14px;
  border-radius: 10px; background: var(--bg-glass-subtle);
  border: 1px solid var(--border-subtle);
}
.status-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
.status-dot.ok { background: #10b981; box-shadow: 0 0 6px rgba(16,185,129,0.3); }
.status-dot.inactive { background: #d4d4d8; }
.status-name { flex: 1; font-size: 13px; color: var(--text-tertiary); font-weight: 500; text-transform: capitalize; }
.status-value { font-size: 12px; font-weight: 600; }
.status-value.ok { color: #10b981; }
.status-value.inactive { color: var(--text-faint); }
.status-empty { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 20px; color: var(--text-faint); font-size: 13px; }
.loading-spinner {
  width: 14px; height: 14px; border: 2px solid #e4e4e7; border-top-color: #6366f1;
  border-radius: 50%; animation: spin 0.8s linear infinite;
}

/* Config */
.config-card { padding: 24px; border-radius: 18px; }
.config-group {
  margin-bottom: 24px;
  padding-bottom: 20px;
  border-bottom: 1px solid rgba(0,0,0,0.04);
}
.config-group:last-of-type { margin-bottom: 16px; border-bottom: none; padding-bottom: 0; }
.config-group-title {
  display: flex; align-items: center; gap: 8px;
  font-size: 14px; font-weight: 600; color: var(--text-primary);
  margin-bottom: 14px;
}
.config-group-title .el-icon { color: #6366f1; }
.config-fields { display: flex; flex-direction: column; gap: 10px; }
.config-field { display: flex; flex-direction: column; gap: 4px; }
.config-field label { font-size: 12px; color: var(--text-muted); font-weight: 500; }
.field-hint { font-size: 11px; color: var(--text-faint); margin-top: 2px; }
.config-field.inline { flex-direction: row; align-items: center; gap: 12px; }
.config-field.inline label { min-width: 72px; flex-shrink: 0; }
.config-field.small { max-width: 200px; }
.slider-value { font-size: 13px; color: var(--text-primary); font-weight: 600; min-width: 28px; text-align: center; }
.config-actions {
  display: flex; align-items: center; gap: 12px;
  padding-top: 12px; margin-top: 4px;
}
.config-hint { font-size: 12px; color: var(--text-faint); }

/* ── Mobile ── */
@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .stats-grid { grid-template-columns: 1fr; gap: 10px; margin-bottom: 20px; }
  .stat-card { padding: 14px; gap: 10px; border-radius: 14px; }
  .stat-icon { width: 36px; height: 36px; border-radius: 10px; }
  .stat-value { font-size: 18px; }
  .stat-label { font-size: 11px; }
  .section { margin-bottom: 20px; }
  .section-title { font-size: 14px; margin-bottom: 10px; }
  .status-card { padding: 12px 14px; border-radius: 14px; }
  .status-grid { grid-template-columns: 1fr; gap: 6px; }
  .status-item { padding: 6px 10px; }
  .config-card { padding: 16px; border-radius: 14px; }
  .config-group { margin-bottom: 16px; padding-bottom: 14px; }
  .config-field.small { max-width: 100%; }
}
</style>
