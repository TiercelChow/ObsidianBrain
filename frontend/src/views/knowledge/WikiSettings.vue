<template>
  <KnowledgePageShell title="Wiki 配置" subtitle="集中管理 Agent Runtime、每本书的规则文档与能力边界">
    <div class="settings-layout">
      <aside class="settings-nav knowledge-surface">
        <button :class="{ active: section === 'runtime' }" @click="section = 'runtime'"><el-icon><Cpu /></el-icon><span><strong>Agent Runtime</strong><small>执行器与模型</small></span></button>
        <button :class="{ active: section === 'documents' }" @click="section = 'documents'"><el-icon><Document /></el-icon><span><strong>配置文档</strong><small>数据库中的 Markdown</small></span></button>
        <button :class="{ active: section === 'skills' }" @click="section = 'skills'"><el-icon><MagicStick /></el-icon><span><strong>Skills</strong><small>下一阶段接入</small></span></button>
      </aside>

      <section class="settings-main knowledge-surface">
        <template v-if="section === 'runtime'">
          <header class="settings-section-head"><div><span>执行环境</span><h2>Agent Runtime</h2><p>首选 DeepSeek Harness；业务数据仍由 Rust 与 SQLite 管理。</p></div></header>
          <div v-if="loading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <article v-for="item in runtimeHealth" v-else :key="item.profile.id" class="runtime-card">
            <div class="runtime-title">
              <div class="runtime-logo">DS</div>
              <div><h3>{{ item.profile.name }}</h3><p>{{ item.profile.runtime === 'deepseek_harness' ? 'ACP stdio sidecar' : item.profile.runtime }}</p></div>
              <span class="knowledge-status" :class="item.available ? 'is-healthy' : 'is-warning'">{{ item.available ? '可执行' : '未连接' }}</span>
            </div>
            <div class="runtime-message">{{ item.message }}<span v-if="item.version"> · {{ item.version }}</span></div>
            <label><span>可执行文件</span><el-input v-model="item.profile.executable" placeholder="deepseek-harness" /></label>
            <label><span>模型覆盖（可选）</span><el-input v-model="item.profile.model" placeholder="留空则使用 Harness 配置" /></label>
            <div class="runtime-actions"><el-switch v-model="item.profile.enabled" active-text="启用" /><el-button type="primary" :loading="savingRuntime" @click="saveRuntime(item.profile)">保存并检测</el-button></div>
          </article>
        </template>

        <template v-else-if="section === 'documents'">
          <header class="settings-section-head split"><div><span>提示词与规则</span><h2>配置文档</h2><p>内容保存在 SQLite，需要运行时才会物化为临时文件。</p></div><el-select v-model="activeBaseId" placeholder="选择知识库" @change="loadSettings"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select></header>
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
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Cpu, Document, EditPen, Files, Loading, MagicStick, Search } from '@element-plus/icons-vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  getBookWikiSettings,
  listBookKnowledgeBases,
  saveAgentRuntimeProfile,
  saveBookWikiConfigDocument,
  type ConfigDocument,
  type KnowledgeBaseSummary,
  type RuntimeHealth,
  type RuntimeProfile,
} from '@/api/knowledge'

const section = ref<'runtime' | 'documents' | 'skills'>('runtime')
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const documents = ref<ConfigDocument[]>([])
const activeDocumentId = ref('')
const runtimeHealth = ref<RuntimeHealth[]>([])
const loading = ref(false)
const savingRuntime = ref(false)
const savingDocument = ref(false)
const activeDocument = computed(() => documents.value.find(document => document.id === activeDocumentId.value))

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

async function saveRuntime(profile: RuntimeProfile) {
  savingRuntime.value = true
  try {
    const response = await saveAgentRuntimeProfile(profile)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行配置保存失败')
    ElMessage.success('运行配置已保存')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingRuntime.value = false
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
.settings-section-head.split :deep(.el-select) { width: 220px; }
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
.runtime-card label { display: grid; gap: 6px; }
.runtime-card label > span { color: var(--text-muted); font-size: 11px; }
.runtime-actions { display: flex; align-items: center; justify-content: space-between; }
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
  .settings-section-head.split :deep(.el-select) { width: 100%; }
  .runtime-title { flex-wrap: wrap; }
  .runtime-title .knowledge-status { margin-left: 55px; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .skills-preview { grid-template-columns: 1fr; }
}
</style>
