<template>
  <KnowledgePageShell title="Wiki 配置" subtitle="集中管理 Agent Runtime、每本书的规则文档与能力边界">
    <div class="settings-layout">
      <aside class="settings-nav knowledge-surface">
        <button :class="{ active: section === 'runtime' }" @click="section = 'runtime'"><el-icon><Cpu /></el-icon><span><strong>Agent Runtime</strong><small>执行器与模型</small></span></button>
        <button :class="{ active: section === 'usage' }" @click="section = 'usage'"><el-icon><DataAnalysis /></el-icon><span><strong>Token 用量</strong><small>调用趋势与来源</small></span></button>
        <button :class="{ active: section === 'documents' }" @click="section = 'documents'"><el-icon><Document /></el-icon><span><strong>配置文档</strong><small>数据库中的 Markdown</small></span></button>
        <button :class="{ active: section === 'skills' }" @click="section = 'skills'"><el-icon><MagicStick /></el-icon><span><strong>Skills</strong><small>下一阶段接入</small></span></button>
      </aside>

      <section class="settings-main knowledge-surface">
        <template v-if="section === 'runtime'">
          <header class="settings-section-head"><div><span>执行环境</span><h2>Agent Runtime</h2><p>DeepSeek Harness 负责执行，模型可连接任意受支持的供应商；业务数据仍由 Rust 与 SQLite 管理。</p></div></header>
          <div v-if="loading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <article v-for="item in runtimeHealth" v-else :key="item.profile.id" class="runtime-card">
            <div class="runtime-title">
              <div class="runtime-logo">DS</div>
              <div><h3>{{ item.profile.name }}</h3><p>{{ item.profile.runtime === 'deepseek_harness' ? 'ACP stdio sidecar' : item.profile.runtime }}</p></div>
              <span class="knowledge-status" :class="runtimeVerification[item.profile.id] ? 'is-healthy' : (item.available ? '' : 'is-warning')">{{ runtimeVerification[item.profile.id] ? '连接已验证' : (item.available ? '启动器可用' : '不可用') }}</span>
            </div>
            <div class="runtime-message" :class="{ 'is-verified': runtimeVerification[item.profile.id] }">{{ runtimeVerification[item.profile.id] || item.message }}<span v-if="item.version"> · {{ item.version }}</span></div>
            <label><span>ACP 启动命令</span><el-input v-model="item.profile.executable" placeholder="npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp" /></label>
            <div class="provider-mode">
              <div><strong>第三方模型供应商</strong><span>为当前 Runtime 注入独立的模型路由</span></div>
              <el-switch :model-value="Boolean(item.profile.provider_config)" @change="toggleProviderConfig(item.profile, Boolean($event))" />
            </div>
            <div v-if="item.profile.provider_config" class="provider-fields">
              <label><span>供应商名称</span><el-input v-model="item.profile.provider_config.display_name" placeholder="例如：阿里云百炼" /></label>
              <label><span>供应商 ID</span><el-input v-model="item.profile.provider_config.provider_id" placeholder="例如：aliyun-bailian" /></label>
              <label><span>API 协议</span><el-select v-model="item.profile.provider_config.api_protocol" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true"><el-option label="OpenAI Chat Completions" value="openai-completions" /><el-option label="OpenAI Responses" value="openai-responses" /><el-option label="Anthropic Messages" value="anthropic-messages" /></el-select></label>
              <label class="is-wide"><span>API Base URL</span><el-input v-model="item.profile.provider_config.base_url" placeholder="https://example.com/v1" /></label>
              <label><span>模型 ID</span><el-input v-model="item.profile.model" placeholder="例如：glm-5.2" /></label>
              <label><span>API Key 环境变量</span><el-input v-model="item.profile.provider_config.api_key_env" placeholder="CUSTOM_LLM_API_KEY" /></label>
            </div>
            <label v-else><span>模型覆盖（可选）</span><el-input v-model="item.profile.model" placeholder="留空则使用 Harness 默认模型" /></label>
            <p class="credential-hint">这里只保存环境变量名，不保存密钥。<template v-if="item.profile.provider_config">启动 ObsidianBrain 前请设置 <code>{{ item.profile.provider_config.api_key_env || 'CUSTOM_LLM_API_KEY' }}</code>。</template><template v-else>凭据由 Harness 默认 Profile 或 Harness Web 的 Models 页面管理。</template></p>
            <div class="runtime-actions">
              <el-switch v-model="item.profile.enabled" active-text="启用" />
              <div>
                <el-button :loading="verifyingRuntimeId === item.profile.id" :disabled="!item.profile.enabled || savingRuntime" @click="verifyRuntime(item.profile)">验证已保存配置</el-button>
                <el-button type="primary" :loading="savingRuntime" @click="saveRuntime(item.profile)">保存</el-button>
              </div>
            </div>
          </article>
        </template>

        <template v-else-if="section === 'usage'">
          <header class="settings-section-head split usage-head">
            <div><span>运行可观测性</span><h2>Token 用量</h2><p>按时间与调用方查看输入、输出和调用趋势。</p></div>
            <div class="usage-filters">
              <el-date-picker v-model="usageDateRange" type="daterange" value-format="YYYY-MM-DD" format="YYYY/MM/DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" :clearable="false" unlink-panels popper-class="glass-picker" @change="loadUsage" />
              <el-select v-model="usageCaller" class="knowledge-select is-compact" popper-class="system-select-popper system-toolbar-popper" placement="bottom-start" :offset="0" :fit-input-width="true" @change="loadUsage">
                <el-option label="全部调用方" value="" />
                <el-option label="知识问答" value="knowledge_qa" />
                <el-option label="研究任务" value="knowledge_task" />
              </el-select>
            </div>
          </header>
          <div v-if="usageLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <div v-else class="usage-dashboard">
            <div class="usage-disclosure" :class="`is-${usageStats?.usage_source || 'unavailable'}`">
              <el-icon><InfoFilled /></el-icon>
              <span><strong>{{ usageSourceLabel }}</strong>当前 Harness ACP 未上报精确 token，现有问答和研究任务使用本地文本估算；未来运行时上报后会自动标记为实测。</span>
            </div>
            <div class="usage-metrics">
              <article><span>总 Token</span><strong>{{ formatTokenCount(usageStats?.totals.total_tokens || 0) }}</strong><small>{{ usageStats?.totals.runs || 0 }} 次有记录调用</small></article>
              <article><span>输入</span><strong>{{ formatTokenCount(usageStats?.totals.input_tokens || 0) }}</strong><small>提示词与检索证据</small></article>
              <article><span>输出</span><strong>{{ formatTokenCount(usageStats?.totals.output_tokens || 0) }}</strong><small>模型回答与任务结果</small></article>
              <article><span>未统计</span><strong>{{ usageStats?.totals.unreported_runs || 0 }}</strong><small>升级前的历史运行</small></article>
            </div>
            <section class="usage-chart-card">
              <header><div><span>每日趋势</span><strong>{{ usageDateRange[0] }} — {{ usageDateRange[1] }}</strong></div><em>输入 + 输出</em></header>
              <div v-if="usageDailyBars.length" class="usage-bars" aria-label="每日 Token 用量柱状图">
                <div v-for="point in usageDailyBars" :key="point.date" class="usage-bar-column" :title="`${point.date} · ${point.total_tokens} tokens`">
                  <span>{{ formatTokenCount(point.total_tokens) }}</span>
                  <i :style="{ height: `${point.height}%` }"></i>
                  <small>{{ point.label }}</small>
                </div>
              </div>
              <div v-else class="usage-empty">当前筛选范围内还没有可统计的调用。</div>
            </section>
            <section class="usage-callers">
              <header><span>调用方分布</span><strong>用于定位问答与研究任务的上下文成本</strong></header>
              <article v-for="item in usageStats?.by_caller || []" :key="item.caller">
                <div><strong>{{ callerLabel(item.caller) }}</strong><span>{{ item.runs }} 次 · {{ formatTokenCount(item.total_tokens) }} tokens</span></div>
                <i><b :style="{ width: `${callerShare(item.total_tokens)}%` }"></b></i>
              </article>
            </section>
          </div>
        </template>

        <template v-else-if="section === 'documents'">
          <header class="settings-section-head split"><div><span>提示词与规则</span><h2>配置文档</h2><p>内容保存在 SQLite，需要运行时才会物化为临时文件。</p></div><el-select v-model="activeBaseId" class="knowledge-select is-compact is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库" @change="loadSettings"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select></header>
          <div v-if="!activeBaseId" class="knowledge-empty"><strong>选择一本书</strong><span>查看并调整它的知识建模、问答与任务规则。</span></div>
          <div v-else class="document-editor">
            <nav class="document-tabs"><button v-for="document in documents" :key="document.id" :class="{ active: document.id === activeDocument?.id }" @click="activeDocumentId = document.id"><el-icon><Document /></el-icon>{{ document.name }}</button></nav>
            <template v-if="activeDocument">
              <div class="document-meta"><span>{{ activeDocument.scope === 'book' ? '书籍级配置' : '全局配置' }}</span><span>Revision {{ activeDocument.revision }}</span></div>
              <textarea v-model="activeDocument.content_md" spellcheck="false"></textarea>
              <div class="document-actions"><span>Markdown 内容仅作为配置载荷，数据库是唯一事实来源。</span><el-button type="primary" :loading="savingDocument" @click="saveDocument">保存文档</el-button></div>
            </template>
          </div>
        </template>

        <template v-else>
          <header class="settings-section-head"><div><span>能力扩展</span><h2>Skills</h2><p>按知识库启用可审计、可版本化的 Agent 能力。</p></div></header>
          <div class="skills-preview">
            <div><el-icon><Search /></el-icon><strong>书内检索</strong><span>只读访问实体、论断与引用</span><em>内置</em></div>
            <div><el-icon><EditPen /></el-icon><strong>知识变更集</strong><span>提交候选修改，交由后端校验</span><em>规划中</em></div>
            <div><el-icon><Files /></el-icon><strong>PDF 摄入</strong><span>版面提取、分块与引用定位</span><em>规划中</em></div>
          </div>
        </template>
      </section>
    </div>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Cpu, DataAnalysis, Document, EditPen, Files, InfoFilled, Loading, MagicStick, Search } from '@element-plus/icons-vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  getBookWikiSettings,
  getAgentUsageStats,
  listBookKnowledgeBases,
  saveAgentRuntimeProfile,
  saveBookWikiConfigDocument,
  verifyAgentRuntime,
  type ConfigDocument,
  type AgentUsageStats,
  type KnowledgeBaseSummary,
  type RuntimeHealth,
  type RuntimeProfile,
} from '@/api/knowledge'

const section = ref<'runtime' | 'usage' | 'documents' | 'skills'>('runtime')
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const documents = ref<ConfigDocument[]>([])
const activeDocumentId = ref('')
const runtimeHealth = ref<RuntimeHealth[]>([])
const loading = ref(false)
const savingRuntime = ref(false)
const verifyingRuntimeId = ref('')
const runtimeVerification = ref<Record<string, string>>({})
const savingDocument = ref(false)
const usageLoading = ref(false)
const usageCaller = ref<'' | 'knowledge_qa' | 'knowledge_task'>('')
const usageDateRange = ref<[string, string]>(defaultUsageRange())
const usageStats = ref<AgentUsageStats | null>(null)
const activeDocument = computed(() => documents.value.find(document => document.id === activeDocumentId.value))
const usageDailyBars = computed(() => {
  const points = usageStats.value?.daily || []
  const maximum = Math.max(...points.map(point => point.total_tokens), 1)
  return points.map(point => ({
    ...point,
    height: Math.max(8, Math.round((point.total_tokens / maximum) * 100)),
    label: new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric' }).format(new Date(`${point.date}T00:00:00`)),
  }))
})
const usageSourceLabel = computed(() => ({
  measured: '实测数据',
  mixed: '混合数据',
  estimated: '估算数据',
  unavailable: '暂无数据',
}[usageStats.value?.usage_source || 'unavailable']))

function defaultUsageRange(): [string, string] {
  const end = new Date()
  const start = new Date(end)
  start.setDate(start.getDate() - 29)
  return [formatDateValue(start), formatDateValue(end)]
}

function formatDateValue(value: Date) {
  const year = value.getFullYear()
  const month = String(value.getMonth() + 1).padStart(2, '0')
  const day = String(value.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function formatTokenCount(value: number) {
  return new Intl.NumberFormat('zh-CN', { notation: value >= 10_000 ? 'compact' : 'standard', maximumFractionDigits: 1 }).format(value)
}

function callerLabel(caller: string) {
  return caller === 'knowledge_qa' ? '知识问答' : caller === 'knowledge_task' ? '研究任务' : caller
}

function callerShare(tokens: number) {
  const total = usageStats.value?.totals.total_tokens || 0
  return total ? Math.max(3, Math.round((tokens / total) * 100)) : 0
}

function toggleProviderConfig(profile: RuntimeProfile, enabled: boolean) {
  profile.provider_config = enabled
    ? {
        provider_id: 'custom-provider',
        display_name: '自定义供应商',
        api_protocol: 'openai-completions',
        base_url: '',
        api_key_env: 'CUSTOM_LLM_API_KEY',
      }
    : null
  delete runtimeVerification.value[profile.id]
}

async function initialize() {
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    bases.value = response.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    activeBaseId.value = bases.value[0]?.id || ''
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function loadSettings() {
  loading.value = true
  try {
    const response = await getBookWikiSettings(activeBaseId.value || undefined)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '配置加载失败')
    runtimeHealth.value = response.result.runtime_profiles
    documents.value = response.result.documents
    activeDocumentId.value = documents.value.some(document => document.id === activeDocumentId.value)
      ? activeDocumentId.value
      : (documents.value[0]?.id || '')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loading.value = false
  }
}

async function loadUsage() {
  if (usageDateRange.value.length !== 2) return
  usageLoading.value = true
  try {
    const response = await getAgentUsageStats({
      startDate: usageDateRange.value[0],
      endDate: usageDateRange.value[1],
      ...(usageCaller.value ? { caller: usageCaller.value } : {}),
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Token 用量加载失败')
    usageStats.value = response.result
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    usageLoading.value = false
  }
}

async function saveRuntime(profile: RuntimeProfile) {
  savingRuntime.value = true
  try {
    const response = await saveAgentRuntimeProfile(profile)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行配置保存失败')
    delete runtimeVerification.value[profile.id]
    ElMessage.success('运行配置已保存')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingRuntime.value = false
  }
}

async function verifyRuntime(profile: RuntimeProfile) {
  verifyingRuntimeId.value = profile.id
  try {
    const response = await verifyAgentRuntime(profile.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '连接验证失败')
    runtimeVerification.value[profile.id] = response.result.message
    ElMessage.success(response.result.message)
  } catch (error) {
    delete runtimeVerification.value[profile.id]
    ElMessage.error((error as Error).message)
  } finally {
    verifyingRuntimeId.value = ''
  }
}

async function saveDocument() {
  if (!activeDocument.value) return
  savingDocument.value = true
  try {
    const response = await saveBookWikiConfigDocument(activeDocument.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '文档保存失败')
    const index = documents.value.findIndex(document => document.id === response.result?.id)
    if (index >= 0) documents.value[index] = response.result
    ElMessage.success('配置文档已保存到数据库')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingDocument.value = false
  }
}

watch(section, value => {
  if (value === 'usage' && !usageStats.value) void loadUsage()
})
onMounted(initialize)
</script>

<style scoped>
.settings-layout { min-height: 585px; display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 12px; }
.settings-nav { align-self: start; display: grid; gap: 4px; padding: 7px; }
.settings-nav button { display: flex; align-items: center; gap: 11px; min-height: 58px; padding: 9px 11px; border: 0; border-radius: 13px; background: transparent; color: var(--text-muted); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.settings-nav button:hover { background: var(--bg-hover); color: var(--text-primary); }
.settings-nav button.active { background: var(--accent-light); color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent-border); }
.settings-nav .el-icon { flex: none; font-size: 19px; }
.settings-nav button span { display: grid; gap: 2px; }
.settings-nav strong { color: inherit; font-size: 13px; }
.settings-nav small { color: var(--text-faint); font-size: 10px; }
.settings-main { min-width: 0; padding: clamp(20px, 3vw, 34px); }
.settings-section-head { margin-bottom: 25px; padding-bottom: 20px; border-bottom: 1px solid var(--border-faint); }
.settings-section-head.split { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; }
.settings-section-head span { color: var(--accent); font-size: 10px; font-weight: 720; letter-spacing: .08em; }
.settings-section-head h2 { margin: 5px 0 4px; font-size: 24px; }
.settings-section-head p { color: var(--text-muted); font-size: 12px; }
.settings-loading { min-height: 260px; display: grid; place-content: center; color: var(--accent); }
.runtime-card { max-width: 720px; display: grid; gap: 15px; padding: 18px; border: 1px solid var(--border-faint); border-radius: 17px; background: var(--bg-glass-subtle); }
.runtime-title { display: flex; align-items: center; gap: 11px; }
.runtime-logo { width: 44px; height: 44px; display: grid; place-items: center; border-radius: 14px; background: linear-gradient(145deg, var(--accent), #3c3fae); color: white; font-size: 12px; font-weight: 760; box-shadow: 0 8px 18px color-mix(in srgb, var(--accent) 22%, transparent); }
.runtime-title > div:nth-child(2) { flex: 1; }
.runtime-title h3 { margin: 0; font-size: 16px; }
.runtime-title p { color: var(--text-faint); font-size: 10px; }
.runtime-message { padding: 9px 11px; border-radius: 10px; background: var(--bg-glass); color: var(--text-muted); font-family: var(--font-mono); font-size: 10px; }
.runtime-message.is-verified { background: color-mix(in srgb, #34c759 11%, transparent); color: #248a3d; }
.runtime-card label { display: grid; gap: 6px; }
.runtime-card label > span { color: var(--text-muted); font-size: 11px; }
.provider-mode { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 11px 13px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); }
.provider-mode > div { display: grid; gap: 2px; }
.provider-mode strong { font-size: 12px; }
.provider-mode span { color: var(--text-faint); font-size: 10px; }
.provider-fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; padding: 14px; border: 1px solid var(--accent-border); border-radius: 14px; background: var(--accent-light); }
.provider-fields .is-wide { grid-column: 1 / -1; }
.credential-hint { color: var(--text-faint); font-size: 10px; line-height: 1.55; }
.credential-hint code { color: var(--text-muted); font-family: var(--font-mono); }
.runtime-actions { display: flex; align-items: center; justify-content: space-between; }
.runtime-actions > div { display: flex; gap: 8px; }
.usage-head { align-items: flex-start !important; }
.usage-filters { width: min(100%, 520px); display: grid; grid-template-columns: minmax(250px, 1fr) 170px; gap: 8px; }
.usage-filters :deep(.el-date-editor) { width: 100%; min-height: 40px; border-radius: 12px; background: var(--bg-glass-subtle); box-shadow: inset 0 0 0 1px var(--border-faint); }
.usage-dashboard { display: grid; gap: 13px; }
.usage-disclosure { display: flex; align-items: flex-start; gap: 9px; padding: 11px 13px; border: 1px solid var(--border-faint); border-radius: 13px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.usage-disclosure .el-icon { flex: none; margin-top: 2px; color: var(--accent); font-size: 15px; }
.usage-disclosure strong { margin-right: 7px; color: var(--text-primary); }
.usage-disclosure.is-estimated { border-color: color-mix(in srgb, #ff9f0a 28%, var(--border-faint)); background: color-mix(in srgb, #ff9f0a 6%, var(--bg-glass-subtle)); }
.usage-metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 9px; }
.usage-metrics article { min-width: 0; display: grid; gap: 3px; padding: 15px; border: 1px solid var(--border-faint); border-radius: 15px; background: var(--bg-glass-subtle); box-shadow: var(--inset-highlight); }
.usage-metrics span { color: var(--text-faint); font-size: 10px; }
.usage-metrics strong { overflow: hidden; color: var(--text-primary); font-size: clamp(20px, 3vw, 28px); font-variant-numeric: tabular-nums; text-overflow: ellipsis; }
.usage-metrics small { color: var(--text-muted); font-size: 9px; }
.usage-chart-card, .usage-callers { padding: 16px; border: 1px solid var(--border-faint); border-radius: 16px; background: var(--bg-glass-subtle); }
.usage-chart-card > header, .usage-callers > header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.usage-chart-card > header div { display: grid; gap: 3px; }
.usage-chart-card > header span, .usage-callers > header span { color: var(--text-primary); font-size: 13px; font-weight: 700; }
.usage-chart-card > header strong, .usage-callers > header strong { color: var(--text-faint); font-size: 9px; font-weight: 500; }
.usage-chart-card > header em { color: var(--text-faint); font-size: 9px; font-style: normal; }
.usage-bars { height: 220px; display: flex; align-items: stretch; gap: clamp(3px, .8vw, 9px); margin-top: 18px; overflow-x: auto; padding-top: 22px; }
.usage-bar-column { min-width: 15px; flex: 1; display: grid; grid-template-rows: 14px 1fr 15px; justify-items: center; align-items: end; gap: 4px; }
.usage-bar-column > span { max-width: 46px; overflow: hidden; color: var(--text-faint); font-size: 8px; font-variant-numeric: tabular-nums; text-overflow: ellipsis; white-space: nowrap; opacity: 0; transition: opacity var(--motion-fast) var(--ease-emphasized); }
.usage-bar-column:hover > span { opacity: 1; }
.usage-bar-column > i { width: min(100%, 22px); min-height: 5px; border-radius: 7px 7px 3px 3px; background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 72%, white), var(--accent)); box-shadow: 0 5px 12px color-mix(in srgb, var(--accent) 15%, transparent); transform-origin: bottom; animation: usage-rise var(--motion-slow) var(--ease-emphasized) both; }
.usage-bar-column > small { color: var(--text-faint); font-size: 8px; white-space: nowrap; }
.usage-empty { min-height: 180px; display: grid; place-content: center; color: var(--text-faint); font-size: 11px; }
.usage-callers { display: grid; gap: 12px; }
.usage-callers > header { margin-bottom: 2px; }
.usage-callers article { display: grid; gap: 6px; }
.usage-callers article > div { display: flex; justify-content: space-between; gap: 12px; font-size: 11px; }
.usage-callers article span { color: var(--text-faint); font-size: 9px; }
.usage-callers article > i { height: 7px; overflow: hidden; border-radius: 999px; background: color-mix(in srgb, var(--text-primary) 6%, transparent); }
.usage-callers article > i b { height: 100%; display: block; border-radius: inherit; background: var(--accent); transition: width var(--motion-slow) var(--ease-emphasized); }
@keyframes usage-rise { from { transform: scaleY(.08); opacity: .35; } }
.document-editor { display: grid; gap: 12px; }
.document-tabs { display: flex; gap: 6px; overflow-x: auto; }
.document-tabs button { min-height: 38px; display: flex; align-items: center; gap: 6px; padding: 0 12px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-muted); font: inherit; font-size: 11px; white-space: nowrap; cursor: pointer; }
.document-tabs button.active { border-color: var(--accent-border); background: var(--accent-light); color: var(--accent); }
.document-meta { display: flex; justify-content: space-between; color: var(--text-faint); font-size: 10px; }
.document-editor textarea { width: 100%; min-height: 340px; resize: vertical; padding: 17px; border: 1px solid var(--border-subtle); border-radius: 15px; background: color-mix(in srgb, var(--bg-base) 55%, transparent); color: var(--text-secondary); font-family: var(--font-mono); font-size: 12px; line-height: 1.75; }
.document-editor textarea:focus { border-color: var(--accent-border); }
.document-actions { display: flex; align-items: center; justify-content: space-between; gap: 15px; }
.document-actions span { color: var(--text-faint); font-size: 10px; }
.skills-preview { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.skills-preview > div { display: grid; align-content: start; gap: 7px; min-height: 160px; padding: 16px; border: 1px solid var(--border-faint); border-radius: 16px; background: var(--bg-glass-subtle); }
.skills-preview .el-icon { color: var(--accent); font-size: 23px; }
.skills-preview strong { margin-top: 8px; font-size: 14px; }
.skills-preview span { color: var(--text-muted); font-size: 11px; line-height: 1.5; }
.skills-preview em { width: fit-content; margin-top: auto; padding: 3px 8px; border-radius: 999px; background: var(--accent-light); color: var(--accent); font-size: 9px; font-style: normal; }
@media (max-width: 768px) {
  .settings-layout { display: block; }
  .settings-nav { display: flex; margin-bottom: 10px; overflow-x: auto; }
  .settings-nav button { min-width: 56px; min-height: 48px; justify-content: center; padding: 8px 13px; }
  .settings-nav button span { display: none; }
  .settings-main { padding: 16px; }
  .settings-section-head.split { display: grid; }
  .runtime-title { flex-wrap: wrap; }
  .runtime-title .knowledge-status { margin-left: 55px; }
  .runtime-actions { align-items: stretch; flex-direction: column; gap: 12px; }
  .runtime-actions > div { display: grid; grid-template-columns: 1fr 1fr; }
  .provider-fields { grid-template-columns: 1fr; }
  .provider-fields .is-wide { grid-column: auto; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .usage-filters { width: 100%; grid-template-columns: 1fr; }
  .usage-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .usage-bars { height: 190px; }
  .usage-chart-card, .usage-callers { padding: 13px; }
  .skills-preview { grid-template-columns: 1fr; }
}
@media (prefers-reduced-motion: reduce) {
  .usage-bar-column > i { animation: none; }
}
</style>
