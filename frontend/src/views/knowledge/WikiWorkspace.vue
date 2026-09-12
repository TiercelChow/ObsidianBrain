<template>
  <KnowledgePageShell title="Wiki 工作台" subtitle="浏览数据库实体，并沿引用回到书中原文">
    <template #actions>
      <el-button v-if="activeBase" :loading="syncing" @click="syncActive"><el-icon><Refresh /></el-icon>同步来源</el-button>
    </template>

    <div class="wiki-toolbar knowledge-toolbar">
      <el-select v-model="activeBaseId" placeholder="选择一本书" @change="onBaseChanged">
        <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
      </el-select>
      <label class="knowledge-search">
        <el-icon><Search /></el-icon>
        <input v-model="query" placeholder="搜索标题、摘要与正文" @input="scheduleSearch" />
      </label>
      <button class="mobile-sync-button" type="button" :disabled="syncing" aria-label="同步来源" @click="syncActive">
        <el-icon :class="{ 'is-loading': syncing }"><Refresh /></el-icon>
      </button>
    </div>

    <div v-if="loadingBases" class="knowledge-empty knowledge-surface">
      <el-icon class="is-loading" :size="24"><Loading /></el-icon><span>正在加载知识库…</span>
    </div>
    <div v-else-if="!bases.length" class="knowledge-empty knowledge-surface">
      <strong>还没有可浏览的知识库</strong>
      <span>先从知识库页面选择一本书并完成初始化。</span>
      <el-button type="primary" @click="$router.push('/knowledge')">前往初始化</el-button>
    </div>

    <section v-else class="wiki-workspace" :class="{ 'show-detail': Boolean(selectedEntry) }">
      <aside class="entry-pane knowledge-surface">
        <div class="entry-pane-head">
          <div>
            <strong>{{ activeBase?.book_name }}</strong>
            <span>{{ entries.length >= 160 ? '显示前 160 个结果' : `${entries.length} 个结果` }}</span>
          </div>
          <span v-if="activeBase" class="knowledge-status" :class="`is-${activeBase.sync_state}`">
            {{ syncLabel(activeBase.sync_state) }}
          </span>
        </div>
        <div class="entry-list">
          <button
            v-for="entry in entries"
            :key="entry.id"
            class="entry-row"
            :class="{ active: entry.id === selectedEntry?.id }"
            @click="selectEntry(entry)"
          >
            <span class="entry-type">{{ entryTypeLabel(entry.entry_type) }}</span>
            <strong>{{ entry.title }}</strong>
            <span class="entry-summary">{{ entry.summary || '此章节没有摘要' }}</span>
            <span class="entry-source">{{ entry.source_path || '数据库实体' }}</span>
          </button>
          <div v-if="!loadingEntries && !entries.length" class="entry-empty">没有匹配的实体</div>
          <div v-if="loadingEntries" class="entry-loading"><el-icon class="is-loading"><Loading /></el-icon>检索中</div>
        </div>
      </aside>

      <article class="entry-detail knowledge-surface">
        <template v-if="selectedEntry">
          <button class="mobile-back" type="button" @click="selectedEntry = null">
            <el-icon><ArrowLeft /></el-icon>返回实体列表
          </button>
          <header class="entry-detail-head">
            <div>
              <span class="entry-type">{{ entryTypeLabel(selectedEntry.entry_type) }}</span>
              <h2>{{ selectedEntry.title }}</h2>
              <p>{{ selectedEntry.source_path || '数据库实体' }}</p>
            </div>
            <span class="knowledge-status is-healthy">{{ selectedEntry.status === 'verified' ? '可追溯' : selectedEntry.status }}</span>
          </header>
          <div v-if="detailLoading" class="detail-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <template v-else-if="detail">
            <div ref="markdownRef" class="entity-markdown markdown-body" v-html="renderedHtml"></div>
            <section class="citation-section">
              <h3>来源引用 <span>{{ detail.citations.length }}</span></h3>
              <div v-for="citation in detail.citations" :key="citation.id" class="citation-card">
                <div><el-icon><Document /></el-icon><strong>{{ citation.source_path }}</strong></div>
                <span v-if="citation.line_start">第 {{ citation.line_start }}–{{ citation.line_end }} 行</span>
                <p v-if="citation.quote_text">{{ citation.quote_text }}</p>
              </div>
            </section>
          </template>
        </template>
        <div v-else class="knowledge-empty">
          <div class="detail-symbol"><el-icon><Tickets /></el-icon></div>
          <strong>选择一个实体开始阅读</strong>
          <span>正文、公式、表格与 Mermaid 将沿用阅境轩的 Markdown 渲染链路。</span>
        </div>
      </article>
    </section>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowLeft, Document, Loading, Refresh, Search, Tickets } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import {
  getKnowledgeEntry,
  listBookKnowledgeBases,
  listKnowledgeEntries,
  syncBookKnowledgeBase,
  type KnowledgeBaseSummary,
  type KnowledgeEntryDetail,
  type KnowledgeEntrySummary,
} from '@/api/knowledge'

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const query = ref('')
const entries = ref<KnowledgeEntrySummary[]>([])
const selectedEntry = ref<KnowledgeEntrySummary | null>(null)
const detail = ref<KnowledgeEntryDetail | null>(null)
const renderedHtml = ref('')
const markdownRef = ref<HTMLElement | null>(null)
const loadingBases = ref(false)
const loadingEntries = ref(false)
const detailLoading = ref(false)
const syncing = ref(false)
let searchTimer = 0

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

async function loadBases() {
  loadingBases.value = true
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '加载失败')
    bases.value = response.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    const requested = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requested) ? requested : (bases.value[0]?.id || '')
    if (activeBaseId.value) await loadEntries(String(route.query.entry || ''))
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loadingBases.value = false
  }
}

async function loadEntries(preselectId = '') {
  if (!activeBaseId.value) return
  loadingEntries.value = true
  try {
    const response = await listKnowledgeEntries(activeBaseId.value, { query: query.value, limit: 160 })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '实体加载失败')
    entries.value = response.result.entries
    const preselect = entries.value.find(entry => entry.id === preselectId)
    if (preselect) await selectEntry(preselect)
    else if (selectedEntry.value && !entries.value.some(entry => entry.id === selectedEntry.value?.id)) selectedEntry.value = null
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loadingEntries.value = false
  }
}

function scheduleSearch() {
  window.clearTimeout(searchTimer)
  searchTimer = window.setTimeout(() => loadEntries(), 220)
}

async function selectEntry(entry: KnowledgeEntrySummary) {
  selectedEntry.value = entry
  detail.value = null
  detailLoading.value = true
  router.replace({ query: { ...route.query, base: activeBaseId.value, entry: entry.id } })
  try {
    const response = await getKnowledgeEntry(entry.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '实体读取失败')
    detail.value = response.result
    renderedHtml.value = await renderMarkdown(response.result.content_md, undefined, entry.id)
    await nextTick()
    if (markdownRef.value) await enhance(markdownRef.value)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    detailLoading.value = false
  }
}

async function onBaseChanged() {
  selectedEntry.value = null
  detail.value = null
  router.replace({ query: { base: activeBaseId.value } })
  await loadEntries()
}

async function syncActive() {
  if (!activeBaseId.value) return
  syncing.value = true
  try {
    const response = await syncBookKnowledgeBase(activeBaseId.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '同步失败')
    ElMessage.success(response.result.message)
    await loadBases()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    syncing.value = false
  }
}

function entryTypeLabel(type: string) {
  return type === 'source_section' ? '来源章节' : type
}

function syncLabel(status: string) {
  return ({ clean: '已同步', outdated: '待同步', failed: '同步失败', scanning: '扫描中' } as Record<string, string>)[status] || status
}

onMounted(loadBases)
onBeforeUnmount(() => { window.clearTimeout(searchTimer); cleanup() })
</script>

<style scoped>
.wiki-toolbar { margin-bottom: 12px; }
.wiki-toolbar :deep(.el-select) { width: 230px; flex: none; }
.wiki-toolbar .knowledge-search { flex: 1; }
.wiki-workspace { min-height: 570px; display: grid; grid-template-columns: minmax(250px, 320px) minmax(0, 1fr); gap: 12px; }
.entry-pane, .entry-detail { min-height: 0; overflow: hidden; }
.entry-pane { display: flex; flex-direction: column; }
.entry-pane-head { min-height: 68px; display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 12px 14px; border-bottom: 1px solid var(--border-faint); }
.entry-pane-head > div { min-width: 0; display: grid; gap: 2px; }
.entry-pane-head strong { overflow: hidden; font-size: 14px; text-overflow: ellipsis; white-space: nowrap; }
.entry-pane-head span:not(.knowledge-status) { color: var(--text-faint); font-size: 11px; }
.entry-list { flex: 1; max-height: calc(100vh - 260px); overflow: auto; padding: 7px; }
.entry-row { width: 100%; display: grid; gap: 4px; margin: 0 0 5px; padding: 12px; border: 0; border-radius: 13px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.entry-row:hover { background: var(--bg-hover); }
.entry-row.active { background: var(--accent-light); box-shadow: inset 0 0 0 1px var(--accent-border); }
.entry-type { width: fit-content; color: var(--accent); font-size: 10px; font-weight: 700; letter-spacing: .04em; }
.entry-row strong { overflow: hidden; font-size: 14px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
.entry-summary { display: -webkit-box; overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.45; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.entry-source { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.entry-empty, .entry-loading { display: flex; justify-content: center; gap: 7px; padding: 32px 10px; color: var(--text-faint); font-size: 12px; }
.entry-detail { max-height: calc(100vh - 208px); overflow: auto; padding: 30px clamp(22px, 4vw, 62px); }
.entry-detail-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 24px; padding-bottom: 20px; border-bottom: 1px solid var(--border-faint); }
.entry-detail-head h2 { margin: 6px 0 4px; font-size: clamp(24px, 3vw, 36px); font-weight: 730; }
.entry-detail-head p { color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; }
.detail-loading { min-height: 300px; display: grid; place-content: center; color: var(--accent); font-size: 24px; }
.detail-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.entity-markdown { color: var(--text-secondary); font-size: 15px; line-height: 1.8; overflow-wrap: anywhere; }
.entity-markdown :deep(h1), .entity-markdown :deep(h2), .entity-markdown :deep(h3) { margin: 1.4em 0 .6em; color: var(--text-primary); }
.entity-markdown :deep(p) { margin: .8em 0; }
.entity-markdown :deep(pre) { overflow: auto; margin: 1em 0; padding: 15px; border-radius: 13px; background: var(--code-block-bg); color: var(--code-block-text); }
.entity-markdown :deep(code) { font-family: var(--font-mono); }
.entity-markdown :deep(.table-scroll), .entity-markdown :deep(.katex-display), .entity-markdown :deep(.mermaid) { max-width: 100%; overflow-x: auto; }
.entity-markdown :deep(table) { min-width: max-content; border-collapse: collapse; }
.entity-markdown :deep(th), .entity-markdown :deep(td) { padding: 8px 11px; border: 1px solid var(--border-faint); }
.citation-section { margin-top: 32px; padding-top: 22px; border-top: 1px solid var(--border-faint); }
.citation-section h3 { margin: 0 0 12px; font-size: 15px; }
.citation-section h3 span { color: var(--text-faint); font-size: 11px; }
.citation-card { display: grid; gap: 5px; margin-top: 8px; padding: 12px 14px; border-radius: 13px; background: var(--bg-glass-subtle); }
.citation-card > div { display: flex; align-items: center; gap: 7px; font-size: 12px; }
.citation-card > span { color: var(--text-faint); font-size: 10px; }
.citation-card p { color: var(--text-muted); font-size: 11px; }
.mobile-back { display: none; }
.mobile-sync-button { display: none; }
@media (max-width: 768px) {
  .wiki-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .wiki-toolbar :deep(.el-select) { width: 100%; }
  .wiki-toolbar .knowledge-search { grid-column: 1 / -1; grid-row: 2; }
  .mobile-sync-button { grid-column: 2; grid-row: 1; width: 46px; min-height: 46px; display: grid; place-items: center; border: 1px solid var(--border-subtle); border-radius: 14px; background: var(--bg-glass); color: var(--accent); font-size: 17px; }
  .wiki-workspace { min-height: calc(100dvh - 230px); display: block; }
  .entry-pane, .entry-detail { min-height: calc(100dvh - 230px); }
  .entry-detail { display: none; max-height: none; padding: 16px; }
  .wiki-workspace.show-detail .entry-pane { display: none; }
  .wiki-workspace.show-detail .entry-detail { display: block; animation: mobile-detail-in var(--motion-normal) var(--ease-spring-gentle) both; }
  .entry-list { max-height: none; }
  .mobile-back { display: flex; align-items: center; gap: 5px; margin: -4px 0 14px; padding: 8px 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-size: 13px; }
  .entry-detail-head { display: block; }
  .entry-detail-head .knowledge-status { margin-top: 10px; }
}
@keyframes mobile-detail-in { from { opacity: 0; transform: translateX(18px); } to { opacity: 1; transform: none; } }
</style>
