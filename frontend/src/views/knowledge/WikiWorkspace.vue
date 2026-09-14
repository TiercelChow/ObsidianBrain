<template>
  <KnowledgePageShell title="Wiki 工作台" subtitle="浏览数据库实体，并沿引用回到书中原文">
    <template #actions>
      <el-button v-if="activeBase?.pending_review_count" type="primary" @click="openReviews"><el-icon><Checked /></el-icon>审核 {{ activeBase.pending_review_count }}</el-button>
      <el-button v-if="activeBase" :loading="linting" @click="runLint"><el-icon><DataAnalysis /></el-icon>知识体检</el-button>
      <el-button v-if="activeBase" :loading="syncing" @click="syncActive"><el-icon><Refresh /></el-icon>同步来源</el-button>
    </template>

    <div class="wiki-toolbar knowledge-toolbar">
      <el-select v-model="activeBaseId" class="knowledge-select is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择一本书" @change="onBaseChanged">
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
          <div v-if="activeBase" class="entry-statuses">
            <span class="knowledge-status" :class="`is-${activeBase.sync_state}`">
              来源 · {{ syncLabel(activeBase.sync_state) }}
            </span>
            <span class="knowledge-status" :class="`is-${activeBase.compile_state}`">
              Wiki · {{ compileLabel(activeBase) }}
            </span>
          </div>
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
            <div class="entity-meta-strip">
              <span>Revision {{ detail.revision }}</span>
              <span>{{ detail.edit_policy === 'human_protected' ? '人工保护' : '可由审核变更' }}</span>
              <span v-if="detail.confidence != null">置信度 {{ Math.round(detail.confidence * 100) }}%</span>
              <span v-for="alias in detail.aliases" :key="alias">别名 · {{ alias }}</span>
            </div>
            <div ref="markdownRef" class="entity-markdown markdown-body" v-html="renderedHtml"></div>
            <section v-if="detail.claims.length" class="entity-structure-section">
              <h3>可核验论断 <span>{{ detail.claims.length }}</span></h3>
              <div class="claim-list">
                <article v-for="claim in detail.claims" :key="claim.id" class="structure-card">
                  <header><strong>{{ claim.predicate }}</strong><span>{{ claim.verification_status }}</span></header>
                  <p>{{ claim.claim_text }}</p>
                  <footer><span v-if="claim.object_text">对象 · {{ claim.object_text }}</span><span>{{ claim.citation_count }} 条证据</span></footer>
                </article>
              </div>
            </section>
            <section v-if="detail.relations.length" class="entity-structure-section">
              <h3>知识关系 <span>{{ detail.relations.length }}</span></h3>
              <div class="relation-list">
                <article v-for="relation in detail.relations" :key="relation.id" class="structure-card relation-card">
                  <header><span>{{ relation.direction === 'outgoing' ? '指向' : '来自' }}</span><strong>{{ relation.related_entry_title }}</strong></header>
                  <p>{{ relation.relation_type }}<template v-if="relation.evidence"> · {{ relation.evidence }}</template></p>
                </article>
              </div>
            </section>
            <section class="citation-section">
              <h3>来源引用 <span>{{ detail.citations.length }}</span></h3>
              <div v-for="citation in detail.citations" :key="citation.id" class="citation-card">
                <div><el-icon><Document /></el-icon><strong>{{ citation.source_path }}</strong></div>
                <span v-if="citation.line_start">第 {{ citation.line_start }}–{{ citation.line_end }} 行</span>
                <p v-if="citation.quote_text">{{ citation.quote_text }}</p>
              </div>
            </section>
            <section v-if="detail.versions.length" class="entity-structure-section version-section">
              <h3>版本历史 <span>{{ detail.versions.length }}</span></h3>
              <ol class="version-list">
                <li v-for="version in detail.versions" :key="version.revision">
                  <b>Revision {{ version.revision }}</b>
                  <span>{{ version.title }}</span>
                  <time>{{ formatTime(version.created_at) }}</time>
                </li>
              </ol>
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

    <MotionModal v-model="reviewVisible" aria-label="审核 Wiki 变更" size="wide">
      <div class="knowledge-modal-card review-modal">
        <div class="knowledge-modal-head"><div><h3>审核语义 Wiki 变更</h3><p>批准后在一个事务中写入正式条目、论断、关系、版本和引用。</p></div><span v-if="activeReview" class="knowledge-status" :class="`is-${activeReview.risk_level === 'high' ? 'warning' : 'draft'}`">{{ activeReview.risk_level }} risk</span></div>
        <div v-if="activeReview" class="review-content">
          <div class="review-summary"><strong>{{ activeReview.title }}</strong><span>{{ activeReview.reason }}</span></div>
          <article v-for="change in activeReview.changes" :key="change.id" class="review-change">
            <header><span>{{ change.operation === 'create' ? '新增' : '更新' }}</span><strong>{{ String(change.after.title || change.object_id) }}</strong><code>{{ String(change.after.entry_type || '') }}</code></header>
            <p>{{ String(change.after.summary || '') }}</p>
            <div><span>引用 {{ Array.isArray(change.after.citations) ? change.after.citations.length : 0 }}</span><span v-if="change.expected_revision">基于 Revision {{ change.expected_revision }}</span></div>
          </article>
          <el-input v-model="reviewNote" type="textarea" :rows="2" :maxlength="2000" placeholder="可选：记录审核说明" />
        </div>
        <div v-else class="knowledge-empty"><strong>没有待审核变更</strong><span>智能编译生成的候选会显示在这里。</span></div>
        <div class="knowledge-modal-actions"><el-button @click="reviewVisible = false">稍后处理</el-button><template v-if="activeReview"><el-button :loading="resolvingReview" @click="resolveReview('reject')">驳回</el-button><el-button type="primary" :loading="resolvingReview" @click="resolveReview('approve')">批准并应用</el-button></template></div>
      </div>
    </MotionModal>

    <MotionModal v-model="healthVisible" aria-label="知识库体检结果">
      <div class="knowledge-modal-card health-modal">
        <div class="knowledge-modal-head"><div><h3>知识库体检</h3><p>检查数据库结构与证据完整性，不调用模型，也不会修改知识。</p></div><span v-if="healthReport" class="knowledge-status" :class="healthReport.state === 'healthy' ? 'is-healthy' : 'is-warning'">{{ healthReport.state === 'healthy' ? '健康' : '需处理' }}</span></div>
        <template v-if="healthReport">
          <div class="health-counts"><span><b>{{ healthReport.semantic_entry_count }}</b>主题</span><span><b>{{ healthReport.source_span_count }}</b>片段</span><span><b>{{ healthReport.pending_review_count }}</b>待审核</span></div>
          <div v-if="healthReport.issues.length" class="health-issues">
            <article v-for="issue in healthReport.issues" :key="issue.code + issue.title" :class="`is-${issue.severity}`"><strong>{{ issue.title }}</strong><p>{{ issue.detail }}</p></article>
          </div>
          <div v-else class="knowledge-empty"><strong>没有发现结构问题</strong><span>主题知识、引用和论断证据结构完整。</span></div>
        </template>
        <div class="knowledge-modal-actions"><el-button type="primary" @click="healthVisible = false">完成</el-button></div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowLeft, Checked, DataAnalysis, Document, Loading, Refresh, Search, Tickets } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import {
  getKnowledgeEntry,
  lintBookKnowledgeBase,
  listBookKnowledgeBases,
  listKnowledgeChangeSets,
  listKnowledgeEntries,
  resolveKnowledgeChangeSet,
  syncBookKnowledgeBase,
  type KnowledgeBaseSummary,
  type KnowledgeEntryDetail,
  type KnowledgeEntrySummary,
  type KnowledgeChangeSet,
  type KnowledgeHealthReport,
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
const pendingReviews = ref<KnowledgeChangeSet[]>([])
const activeReview = ref<KnowledgeChangeSet | null>(null)
const reviewVisible = ref(false)
const reviewNote = ref('')
const resolvingReview = ref(false)
const linting = ref(false)
const healthVisible = ref(false)
const healthReport = ref<KnowledgeHealthReport | null>(null)
let searchTimer = 0

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

function compileLabel(base: KnowledgeBaseSummary) {
  if (base.compile_mode === 'chapter' && base.compile_state === 'not_started') return '章节索引'
  const labels: Record<string, string> = {
    not_started: '待编译', outdated: '待更新', compiling: '编译中', ready: '智能 Wiki', failed: '编译失败',
  }
  return labels[base.compile_state] || base.compile_state
}

async function loadBases() {
  loadingBases.value = true
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '加载失败')
    bases.value = response.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    const requested = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requested) ? requested : (bases.value[0]?.id || '')
    if (activeBaseId.value) {
      await Promise.all([loadEntries(String(route.query.entry || '')), loadReviews()])
      const requestedReview = String(route.query.review || '')
      if (requestedReview) {
        activeReview.value = pendingReviews.value.find(item => item.id === requestedReview) || null
        reviewVisible.value = Boolean(activeReview.value)
      }
    }
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
  await Promise.all([loadEntries(), loadReviews()])
}

async function loadReviews() {
  if (!activeBaseId.value) return
  const response = await listKnowledgeChangeSets(activeBaseId.value, 'proposed')
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '审核列表加载失败')
  pendingReviews.value = response.result.change_sets
}

function openReviews() {
  activeReview.value = pendingReviews.value[0] || null
  reviewNote.value = ''
  reviewVisible.value = true
}

async function resolveReview(decision: 'approve' | 'reject') {
  if (!activeReview.value) return
  resolvingReview.value = true
  try {
    const response = await resolveKnowledgeChangeSet(activeReview.value.id, decision, reviewNote.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '审核处理失败')
    ElMessage.success(decision === 'approve' ? '知识变更已原子应用' : '知识变更已驳回')
    reviewVisible.value = false
    await loadBases()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    resolvingReview.value = false
  }
}

async function runLint() {
  if (!activeBaseId.value) return
  linting.value = true
  try {
    const response = await lintBookKnowledgeBase(activeBaseId.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识体检失败')
    healthReport.value = response.result
    healthVisible.value = true
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    linting.value = false
  }
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

function formatTime(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  }).format(new Date(value))
}

onMounted(loadBases)
onBeforeUnmount(() => { window.clearTimeout(searchTimer); cleanup() })
</script>

<style scoped>
.wiki-toolbar { margin-bottom: 12px; }
.wiki-toolbar .knowledge-search { flex: 1; }
.wiki-workspace { min-height: 570px; display: grid; grid-template-columns: minmax(250px, 320px) minmax(0, 1fr); gap: 12px; }
.entry-pane, .entry-detail { min-height: 0; overflow: hidden; }
.entry-pane { display: flex; flex-direction: column; }
.entry-pane-head { min-height: 68px; display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 12px 14px; border-bottom: 1px solid var(--border-faint); }
.entry-pane-head > div { min-width: 0; display: grid; gap: 2px; }
.entry-pane-head > .entry-statuses { flex: 0 0 auto; justify-items: end; gap: 5px; }
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
.entity-meta-strip { display: flex; flex-wrap: wrap; gap: 6px; margin: -8px 0 18px; }
.entity-meta-strip span { padding: 5px 9px; border: 1px solid var(--border-faint); border-radius: 999px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 10px; }
.entity-markdown { color: var(--text-secondary); font-size: 15px; line-height: 1.8; overflow-wrap: anywhere; }
.review-modal { max-height: min(780px, calc(100dvh - 48px)); }
.review-content { display: grid; gap: 10px; max-height: min(560px, 62dvh); overflow: auto; padding: 0 2px; }
.review-summary { display: grid; gap: 4px; padding: 12px; border-radius: 13px; background: var(--accent-light); }
.review-summary strong { font-size: 14px; }
.review-summary span { color: var(--text-muted); font-size: 11px; }
.review-change { display: grid; gap: 8px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.review-change header { display: flex; align-items: center; gap: 8px; }
.review-change header span { padding: 3px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.review-change header strong { flex: 1; font-size: 13px; }
.review-change header code { color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; }
.review-change p { color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.review-change > div { display: flex; gap: 12px; color: var(--text-faint); font-size: 9px; }
.health-counts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.health-counts span { display: grid; gap: 3px; padding: 12px; border-radius: 13px; background: var(--bg-glass-subtle); color: var(--text-faint); font-size: 10px; }
.health-counts b { color: var(--text-primary); font-size: 20px; }
.health-issues { display: grid; gap: 8px; max-height: 48dvh; overflow: auto; }
.health-issues article { padding: 12px 14px; border: 1px solid color-mix(in srgb, #ff9f0a 25%, transparent); border-radius: 13px; background: color-mix(in srgb, #ff9f0a 7%, transparent); }
.health-issues article.is-error { border-color: color-mix(in srgb, #ff3b30 25%, transparent); background: color-mix(in srgb, #ff3b30 7%, transparent); }
.health-issues strong { font-size: 12px; }
.health-issues p { margin-top: 4px; color: var(--text-muted); font-size: 11px; line-height: 1.55; }
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
.entity-structure-section { margin-top: 30px; padding-top: 20px; border-top: 1px solid var(--border-faint); }
.entity-structure-section h3 { margin: 0 0 12px; color: var(--text-primary); font-size: 15px; }
.entity-structure-section h3 span { color: var(--text-faint); font-size: 11px; }
.claim-list, .relation-list { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr)); gap: 8px; }
.structure-card { display: grid; gap: 7px; padding: 12px 14px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.structure-card header, .structure-card footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.structure-card header strong { color: var(--text-primary); font-size: 12px; }
.structure-card header span, .structure-card footer { color: var(--text-faint); font-size: 9px; }
.structure-card p { color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.relation-card header { justify-content: flex-start; }
.relation-card header > span { color: var(--accent); }
.version-list { display: grid; gap: 0; padding: 0; list-style: none; }
.version-list li { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 9px 2px; border-bottom: 1px solid var(--border-faint); font-size: 10px; }
.version-list b { color: var(--accent); font-size: 10px; }
.version-list span { overflow: hidden; color: var(--text-secondary); text-overflow: ellipsis; white-space: nowrap; }
.version-list time { color: var(--text-faint); }
.mobile-back { display: none; }
.mobile-sync-button { display: none; }
@media (max-width: 768px) {
  .wiki-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
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
