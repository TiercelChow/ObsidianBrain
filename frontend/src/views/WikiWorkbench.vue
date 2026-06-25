<template>
  <div class="wiki-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">Wiki 工作台</h1>
        <p class="page-subtitle">LLM 增量维护的持久知识库</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="loadStatus" :loading="statusLoading">
          <el-icon v-if="!statusLoading"><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </header>

    <!-- Wiki 状态概览 -->
    <div class="status-row" v-if="status">
      <div class="status-chip"><span class="num">{{ status.total_pages }}</span><span class="label">总页数</span></div>
      <div class="status-chip"><span class="num">{{ status.entities }}</span><span class="label">实体</span></div>
      <div class="status-chip"><span class="num">{{ status.concepts }}</span><span class="label">概念</span></div>
      <div class="status-chip"><span class="num">{{ status.sources }}</span><span class="label">源摘要</span></div>
      <div class="status-chip"><span class="num">{{ status.synthesis }}</span><span class="label">综合论述</span></div>
      <div class="status-chip" v-if="!status.initialized"><span class="num">⚠️</span><span class="label">未初始化</span></div>
    </div>

    <!-- Tab 切换 -->
    <div class="tab-bar">
      <button v-for="tab in tabs" :key="tab.key" class="tab-btn" :class="{ active: activeTab === tab.key }" @click="activeTab = tab.key">
        {{ tab.label }}
      </button>
    </div>

    <!-- Ingest 面板 -->
    <div v-if="activeTab === 'ingest'" class="tab-panel">
      <div class="form-card">
        <div class="form-row">
          <label>原始资料路径</label>
          <el-input v-model="ingestForm.sourcePath" placeholder="Raw/articles/xxx.md" size="default" />
        </div>
        <div class="form-row">
          <label>资料类型</label>
          <el-select v-model="ingestForm.sourceType" size="default" style="width: 160px">
            <el-option label="文章" value="article" />
            <el-option label="论文" value="paper" />
            <el-option label="书籍章节" value="book_chapter" />
            <el-option label="播客笔记" value="podcast" />
            <el-option label="会议记录" value="meeting" />
            <el-option label="笔记" value="note" />
          </el-select>
        </div>
        <div class="form-row">
          <label>来源 URL（可选）</label>
          <el-input v-model="ingestForm.sourceUrl" placeholder="https://..." size="default" />
        </div>
        <el-button type="primary" @click="doIngest" :loading="ingesting" :disabled="!ingestForm.sourcePath.trim()">
          开始摄入
        </el-button>
      </div>

      <!-- 摄入进度 -->
      <div v-if="ingesting" class="progress-bar">
        <span class="step" :class="{ done: ingestStep >= 1 }">① 读取</span>
        <span class="step" :class="{ done: ingestStep >= 2 }">② 摘要</span>
        <span class="step" :class="{ done: ingestStep >= 3 }">③ 提取</span>
        <span class="step" :class="{ done: ingestStep >= 4 }">④ 写页</span>
        <span class="step" :class="{ done: ingestStep >= 5 }">⑤ 索引</span>
      </div>

      <!-- 摄入结果 -->
      <div v-if="ingestResult" class="result-card">
        <h3>摄入完成</h3>
        <div class="result-section">
          <span class="result-label">摘要页：</span>
          <a class="result-link" @click="openInObsidian(ingestResult.summary_page)">{{ ingestResult.summary_page }}</a>
        </div>
        <div v-if="ingestResult.created_pages.length > 0" class="result-section">
          <span class="result-label">新建页面（{{ ingestResult.created_pages.length }}）：</span>
          <div v-for="p in ingestResult.created_pages" :key="p" class="result-item" @click="openInObsidian(p)">{{ p }}</div>
        </div>
        <div v-if="ingestResult.updated_pages.length > 0" class="result-section">
          <span class="result-label">更新页面（{{ ingestResult.updated_pages.length }}）：</span>
          <div v-for="p in ingestResult.updated_pages" :key="p" class="result-item" @click="openInObsidian(p)">{{ p }}</div>
        </div>
        <div v-if="ingestResult.entities.length > 0" class="result-section">
          <span class="result-label">提取实体：</span>
          <el-tag v-for="e in ingestResult.entities" :key="e" size="small" effect="plain">{{ e }}</el-tag>
        </div>
        <div v-if="ingestResult.concepts.length > 0" class="result-section">
          <span class="result-label">提取概念：</span>
          <el-tag v-for="c in ingestResult.concepts" :key="c" size="small" effect="plain">{{ c }}</el-tag>
        </div>
      </div>
    </div>

    <!-- Query 面板 -->
    <div v-if="activeTab === 'query'" class="tab-panel">
      <div class="form-card">
        <div class="form-row">
          <label>问题</label>
          <el-input v-model="queryForm.question" type="textarea" :rows="3" placeholder="基于你的 Wiki 回答问题..." />
        </div>
        <div class="form-row inline">
          <el-switch v-model="queryForm.saveAnswer" />
          <span class="switch-label">归档为综合论述</span>
        </div>
        <el-button type="primary" @click="doQuery" :loading="querying" :disabled="!queryForm.question.trim()">
          查询 Wiki
        </el-button>
      </div>

      <div v-if="queryResult" class="result-card">
        <h3>回答</h3>
        <div class="answer-text" v-html="renderMarkdown(queryResult.answer)"></div>
        <div v-if="queryResult.cited_pages.length > 0" class="result-section">
          <span class="result-label">引用来源：</span>
          <div v-for="p in queryResult.cited_pages" :key="p" class="result-item" @click="openInObsidian(p)">{{ p }}</div>
        </div>
        <div v-if="queryResult.saved_to" class="result-section">
          <span class="result-label">已归档：</span>
          <a class="result-link" @click="openInObsidian(queryResult.saved_to)">{{ queryResult.saved_to }}</a>
        </div>
      </div>
    </div>

    <!-- Lint 面板 -->
    <div v-if="activeTab === 'lint'" class="tab-panel">
      <div class="form-card">
        <div class="form-row inline">
          <el-switch v-model="lintForm.autoFix" />
          <span class="switch-label">自动修复</span>
        </div>
        <el-button type="primary" @click="doLint" :loading="linting">
          执行检查
        </el-button>
      </div>

      <div v-if="lintResult" class="result-card">
        <h3>检查结果（{{ lintResult.total_pages }} 页）</h3>

        <div v-if="lintResult.orphans.length > 0" class="lint-section">
          <div class="lint-title">🔴 孤岛页（{{ lintResult.orphans.length }}）</div>
          <div v-for="p in lintResult.orphans" :key="p" class="result-item" @click="openInObsidian(p)">{{ p }}</div>
        </div>

        <div v-if="lintResult.missing_pages.length > 0" class="lint-section">
          <div class="lint-title">🔵 缺失页面（{{ lintResult.missing_pages.length }}）</div>
          <div v-for="p in lintResult.missing_pages" :key="p" class="result-item">{{ p }}</div>
        </div>

        <div v-if="lintResult.hubs.length > 0" class="lint-section">
          <div class="lint-title">🕸️ 知识枢纽 Top {{ lintResult.hubs.length }}</div>
          <div v-for="h in lintResult.hubs" :key="h[0]" class="result-item" @click="openInObsidian(h[0])">{{ h[0] }} — {{ h[1] }} 引用</div>
        </div>

        <div v-if="lintResult.fixed > 0" class="lint-section">
          <div class="lint-title">✅ 已修复 {{ lintResult.fixed }} 处</div>
        </div>

        <div v-if="lintResult.suggestions.length > 0" class="lint-section">
          <div class="lint-title">💡 建议</div>
          <div v-for="(s, i) in lintResult.suggestions" :key="i" class="suggestion-item">{{ s }}</div>
        </div>
      </div>
    </div>

    <!-- Schema 面板 -->
    <div v-if="activeTab === 'schema'" class="tab-panel">
      <div class="form-card">
        <div class="form-row">
          <label>维护规则（Wiki/schema.md）</label>
          <el-input v-model="schemaContent" type="textarea" :rows="20" placeholder="加载中..." />
        </div>
        <el-button type="primary" @click="saveSchema" :loading="schemaSaving">
          保存
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { ingestSource, queryWiki, lintWiki, getWikiStatus } from '@/api/wiki'
import { callTool } from '@/api'

interface WikiStatus {
  total_pages: number
  entities: number
  concepts: number
  sources: number
  synthesis: number
  initialized: boolean
}

interface IngestResult {
  summary_page: string
  created_pages: string[]
  updated_pages: string[]
  entities: string[]
  concepts: string[]
}

interface QueryResult {
  answer: string
  cited_pages: string[]
  saved_to: string | null
}

interface LintResult {
  total_pages: number
  orphans: string[]
  missing_pages: string[]
  hubs: [string, number][]
  fixed: number
  suggestions: string[]
}

const tabs = [
  { key: 'ingest', label: 'Ingest' },
  { key: 'query', label: 'Query' },
  { key: 'lint', label: 'Lint' },
  { key: 'schema', label: 'Schema' },
]

const activeTab = ref('ingest')
const status = ref<WikiStatus | null>(null)
const statusLoading = ref(false)

const ingestForm = ref({ sourcePath: '', sourceType: 'article', sourceUrl: '' })
const ingestStep = ref(0)
const ingesting = ref(false)
const ingestResult = ref<IngestResult | null>(null)

const queryForm = ref({ question: '', saveAnswer: false })
const querying = ref(false)
const queryResult = ref<QueryResult | null>(null)

const lintForm = ref({ autoFix: false })
const linting = ref(false)
const lintResult = ref<LintResult | null>(null)

const schemaContent = ref('')
const schemaSaving = ref(false)

async function loadStatus() {
  statusLoading.value = true
  try {
    const res = await getWikiStatus() as unknown as { result: WikiStatus }
    status.value = res.result
  } catch (e) {
    console.error('加载 Wiki 状态失败:', e)
  } finally {
    statusLoading.value = false
  }
}

async function doIngest() {
  ingesting.value = true
  ingestStep.value = 1
  ingestResult.value = null
  try {
    ingestStep.value = 2
    const res = await ingestSource(
      ingestForm.value.sourcePath,
      ingestForm.value.sourceType,
      ingestForm.value.sourceUrl || undefined,
    ) as unknown as { result: IngestResult }
    ingestStep.value = 5
    ingestResult.value = res.result
    ElMessage.success('摄入完成')
    await loadStatus()
  } catch (e) {
    console.error('摄入失败:', e)
    ElMessage.error('摄入失败')
  } finally {
    ingesting.value = false
  }
}

async function doQuery() {
  querying.value = true
  queryResult.value = null
  try {
    const res = await queryWiki(queryForm.value.question, queryForm.value.saveAnswer) as unknown as { result: QueryResult }
    queryResult.value = res.result
  } catch (e) {
    console.error('查询失败:', e)
    ElMessage.error('查询失败')
  } finally {
    querying.value = false
  }
}

async function doLint() {
  linting.value = true
  lintResult.value = null
  try {
    const res = await lintWiki(lintForm.value.autoFix) as unknown as { result: LintResult }
    lintResult.value = res.result
    if (res.result.fixed > 0) {
      ElMessage.success(`修复了 ${res.result.fixed} 处问题`)
    }
  } catch (e) {
    console.error('Lint 失败:', e)
    ElMessage.error('Lint 失败')
  } finally {
    linting.value = false
  }
}

async function loadSchema() {
  try {
    const res = await callTool('get_note', { path: 'Wiki/schema.md' }) as unknown as { result: { content: string } }
    schemaContent.value = res.result?.content || ''
  } catch {
    schemaContent.value = ''
  }
}

async function saveSchema() {
  schemaSaving.value = true
  try {
    await callTool('write_note', { path: 'Wiki/schema.md', content: schemaContent.value })
    ElMessage.success('Schema 已保存')
  } catch (e) {
    ElMessage.error('保存失败')
  } finally {
    schemaSaving.value = false
  }
}

function openInObsidian(path: string) {
  window.open(`obsidian://open?path=${encodeURIComponent(path)}`, '_blank')
}

function renderMarkdown(text: string): string {
  return text
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/\[\[([^\]]+)\]\]/g, '<span class="wiki-link">$1</span>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\n/g, '<br>')
}

onMounted(() => {
  loadStatus()
  loadSchema()
})
</script>

<style scoped>
.wiki-page { max-width: 100%; min-height: 100%; }
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-muted); font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.status-row { display: flex; gap: 12px; margin-bottom: 24px; flex-wrap: wrap; }
.status-chip { display: flex; align-items: baseline; gap: 6px; padding: 10px 18px; border-radius: 14px; }
.status-chip .num { font-size: 20px; font-weight: 700; color: var(--text-primary); }
.status-chip .label { font-size: 12px; color: var(--text-muted); }

.tab-bar { display: flex; gap: 4px; margin-bottom: 20px; }
.tab-btn { padding: 8px 20px; border-radius: 10px; border: 1px solid var(--border-glass); background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 14px; font-weight: 500; cursor: pointer; transition: all 0.2s ease; }
.tab-btn:hover { color: var(--text-secondary); }
.tab-btn.active { background: var(--accent); border-color: var(--accent); color: #fff; }

.tab-panel { min-height: 300px; }

.form-card { padding: 24px; border-radius: 16px; margin-bottom: 20px; }
.form-row { display: flex; flex-direction: column; gap: 6px; margin-bottom: 16px; }
.form-row label { font-size: 13px; color: var(--text-muted); font-weight: 500; }
.form-row.inline { flex-direction: row; align-items: center; gap: 10px; }
.switch-label { font-size: 13px; color: var(--text-secondary); }

.progress-bar { display: flex; gap: 8px; padding: 16px 0; align-items: center; }
.progress-bar .step { font-size: 13px; color: var(--text-faint); padding: 4px 10px; border-radius: 8px; background: var(--bg-glass-subtle); }
.progress-bar .step.done { color: #4ade80; background: rgba(74, 222, 128, 0.1); }

.result-card { padding: 20px; border-radius: 14px; }
.result-card h3 { font-size: 15px; font-weight: 600; color: var(--text-primary); margin-bottom: 12px; }
.result-section { margin-bottom: 12px; }
.result-label { font-size: 13px; color: var(--text-muted); font-weight: 500; display: block; margin-bottom: 4px; }
.result-item { font-size: 13px; color: var(--accent); cursor: pointer; padding: 4px 8px; border-radius: 6px; transition: background 0.15s; }
.result-item:hover { background: var(--accent-light); }
.result-link { font-size: 13px; color: var(--accent); cursor: pointer; }
.answer-text { font-size: 14px; color: var(--text-secondary); line-height: 1.7; }
.answer-text :deep(.wiki-link) { color: var(--accent); font-weight: 500; }

.lint-section { margin-bottom: 16px; }
.lint-title { font-size: 14px; font-weight: 600; color: var(--text-primary); margin-bottom: 6px; }
.suggestion-item { font-size: 13px; color: var(--text-tertiary); padding: 4px 0; }

@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .status-row { gap: 8px; }
  .status-chip { padding: 8px 12px; }
  .status-chip .num { font-size: 16px; }
  .form-card { padding: 16px; }
  .tab-btn { padding: 6px 14px; font-size: 13px; }
}
</style>
