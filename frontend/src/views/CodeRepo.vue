<template>
  <div class="code-repo-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">代码仓管理</h1>
        <p class="page-subtitle">注册本地 Git 仓库，自动提取元信息，关联笔记</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadRepos" :loading="loading">
          <el-icon><Refresh /></el-icon> 刷新
        </el-button>
        <el-button type="primary" @click="showAddDialog = true">
          <el-icon><Plus /></el-icon> 注册仓库
        </el-button>
      </div>
    </header>

    <!-- 仓库列表 -->
    <div class="repo-grid" v-if="repos.length > 0">
      <div v-for="repo in repos" :key="repo.name" class="repo-card">
        <div class="repo-header">
          <div class="repo-name">{{ repo.name }}</div>
          <el-tag :type="repo.is_dirty ? 'warning' : 'success'">
            {{ repo.is_dirty ? '有未提交' : '干净' }}
          </el-tag>
        </div>
        <div class="repo-branch">
          <el-icon><Connection /></el-icon>
          <span class="repo-branch-name">{{ repo.current_branch }}</span>
        </div>
        <div class="repo-path" :title="repo.path">{{ repo.path }}</div>
        <div class="repo-meta" v-if="repo.languages && Object.keys(repo.languages).length > 0">
          <span v-for="(ratio, lang) in repo.languages" :key="lang" class="lang-tag">
            {{ lang }} {{ (ratio * 100).toFixed(0) }}%
          </span>
        </div>
        <div class="repo-actions">
          <el-button @click="viewDetail(repo.name)">
            <el-icon><View /></el-icon> 详情
          </el-button>
          <el-button type="primary" @click="openRepo(repo)">
            <el-icon><component :is="isMobile ? CopyDocument : Monitor" /></el-icon>
            {{ isMobile ? '复制路径' : 'VSCode' }}
          </el-button>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="暂无注册的代码仓库">
      <el-button type="primary" @click="showAddDialog = true">注册第一个仓库</el-button>
    </el-empty>

    <!-- 仓库详情对话框 -->
    <el-dialog v-model="showDetailDialog" class="repo-detail-dialog" title="仓库详情" width="720px" align-center destroy-on-close v-if="selectedRepo">
      <div class="repo-detail-content" tabindex="0" role="region" aria-label="仓库详情内容">
        <el-descriptions :column="isMobile ? 1 : 2" border>
          <el-descriptions-item label="名称">{{ selectedRepo.name }}</el-descriptions-item>
          <el-descriptions-item label="分支">{{ selectedRepo.current_branch }}</el-descriptions-item>
          <el-descriptions-item label="HEAD">{{ selectedRepo.head_hash?.substring(0, 7) || '-' }}</el-descriptions-item>
          <el-descriptions-item label="总提交数">{{ selectedRepo.total_commits || 0 }}</el-descriptions-item>
          <el-descriptions-item label="贡献者" :span="isMobile ? 1 : 2">
            {{ selectedRepo.contributors?.join(', ') || '无' }}
          </el-descriptions-item>
          <el-descriptions-item label="路径" :span="isMobile ? 1 : 2">
            <span class="detail-path">{{ selectedRepo.path }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="语言统计" :span="isMobile ? 1 : 2" v-if="selectedRepo.languages && Object.keys(selectedRepo.languages).length > 0">
            <div class="language-tags">
              <el-tag v-for="(ratio, lang) in selectedRepo.languages" :key="lang" effect="plain">
                {{ lang }} {{ (ratio * 100).toFixed(0) }}%
              </el-tag>
            </div>
          </el-descriptions-item>
        </el-descriptions>

        <div class="commits-section">
          <h4>最近提交</h4>
          <div v-if="selectedRepo.recent_commits && selectedRepo.recent_commits.length > 0" class="commit-list">
            <div v-for="commit in selectedRepo.recent_commits" :key="commit.hash" class="commit-item">
              <span class="commit-hash">{{ commit.hash?.substring(0, 7) || '-' }}</span>
              <span class="commit-msg">{{ commit.message || '无提交消息' }}</span>
              <span class="commit-author">{{ commit.author || '未知' }}</span>
            </div>
          </div>
          <div v-else class="no-commits">
            <el-empty description="暂无提交记录" :image-size="60" />
          </div>
        </div>
      </div>
    </el-dialog>

    <!-- 注册仓库对话框 -->
    <el-dialog v-model="showAddDialog" title="注册代码仓库" width="400px">
      <el-form :model="addForm" label-position="top">
        <el-form-item label="仓库路径" required>
          <el-input v-model="addForm.path" placeholder="/path/to/repo" />
        </el-form-item>
        <el-form-item label="仓库名称" required>
          <el-input v-model="addForm.name" placeholder="my-project" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddDialog = false">取消</el-button>
        <el-button type="primary" @click="registerRepo" :loading="adding">注册</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { addCodeRepo, listCodeRepos, getRepoDetail, openInVscode } from '@/api'
import { Refresh, Plus, Connection, View, Monitor, CopyDocument } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

interface RepoInfo {
  name: string
  path: string
  current_branch: string
  is_dirty: boolean
  languages: Record<string, number>
  language_stats?: Record<string, number>
  linked_notes_count: number
}

interface RepoDetail extends RepoInfo {
  head_hash: string
  total_commits: number
  contributors: string[]
  branches: string[]
  recent_commits: Array<{ hash: string; author: string; message: string; timestamp: string }>
}

const repos = ref<RepoInfo[]>([])
const loading = ref(false)
const showAddDialog = ref(false)
const showDetailDialog = ref(false)
const adding = ref(false)
const selectedRepo = ref<RepoDetail | null>(null)
const addForm = ref({ path: '', name: '' })
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => windowWidth.value <= 768)
function onResize() { windowWidth.value = window.innerWidth }

async function loadRepos() {
  loading.value = true
  try {
    const res = await listCodeRepos() as unknown as { result: { repos: RepoInfo[] } }
    repos.value = res.result?.repos || []
  } catch (e) {
    console.error('加载仓库列表失败:', e)
    repos.value = []
  } finally {
    loading.value = false
  }
}

async function registerRepo() {
  if (!addForm.value.path || !addForm.value.name) {
    ElMessage.warning('请填写完整信息')
    return
  }
  adding.value = true
  try {
    await addCodeRepo(addForm.value.path, addForm.value.name)
    ElMessage.success('仓库注册成功')
    showAddDialog.value = false
    addForm.value = { path: '', name: '' }
    await loadRepos()
  } catch (e) {
    ElMessage.error('注册失败: ' + (e as Error).message)
  } finally {
    adding.value = false
  }
}

async function viewDetail(name: string) {
  try {
    const res = await getRepoDetail(name) as unknown as { result: RepoDetail }
    selectedRepo.value = res.result
    showDetailDialog.value = true
  } catch {
    ElMessage.error('获取详情失败')
  }
}

async function openVscode(name: string) {
  try {
    await openInVscode(name)
    ElMessage.success('VSCode 已打开')
  } catch {
    ElMessage.error('打开失败')
  }
}

async function openRepo(repo: RepoInfo) {
  if (!isMobile.value) {
    await openVscode(repo.name)
    return
  }
  try {
    await navigator.clipboard.writeText(repo.path)
    ElMessage.success('仓库路径已复制')
  } catch {
    ElMessage.info(repo.path)
  }
}

onMounted(() => {
  window.addEventListener('resize', onResize)
  loadRepos()
})
onUnmounted(() => window.removeEventListener('resize', onResize))
</script>

<style scoped>
.code-repo-page {
  max-width: 100%;
  min-height: 100%;
}
.code-repo-page .repo-card {
  animation: fade-in var(--duration-normal) var(--ease-out) both;
}
.code-repo-page .repo-card:nth-child(2) { animation-delay: 0.06s; }
.code-repo-page .repo-card:nth-child(3) { animation-delay: 0.12s; }
.code-repo-page .repo-card:nth-child(4) { animation-delay: 0.18s; }

.repo-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr)); gap: 16px; }
.repo-card { min-width: 0; padding: 20px; border-radius: 16px; transition: box-shadow var(--duration-fast) var(--ease-out); display: flex; flex-direction: column; }
.repo-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.04); }
.repo-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.repo-name { min-width: 0; overflow-wrap: anywhere; font-size: 16px; font-weight: 600; color: var(--text-primary); }
.repo-branch { display: flex; align-items: center; gap: 4px; font-size: 13px; color: var(--accent); margin-bottom: 4px; }
.repo-branch-name { min-width: 0; overflow-wrap: anywhere; }
.repo-branch :deep(.el-icon) { flex-shrink: 0; }
.repo-path {
  font-size: 12px; color: var(--text-faint); font-family: var(--font-mono); margin-bottom: 12px;
  word-break: break-all; overflow: hidden; text-overflow: ellipsis;
  display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; max-height: 2.4em;
}
.repo-meta { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 12px; }
.lang-tag { max-width: 100%; overflow-wrap: anywhere; font-size: 11px; padding: 2px 8px; border-radius: 8px; color: var(--text-tertiary); }
.repo-actions { display: flex; gap: 8px; margin-top: auto; padding-top: 12px; border-top: 1px solid var(--border-faint); justify-content: flex-end; }

/* Target only this dialog: Element Plus overlays may render outside the page. */
:global(:root[data-theme] .repo-detail-dialog) {
  display: flex;
  flex-direction: column;
  height: min(640px, calc(var(--visual-viewport-height, 100dvh) - 64px));
  max-width: calc(100vw - 32px);
  overflow: hidden;
}
:global(:root[data-theme] .repo-detail-dialog .el-dialog__header) { flex: none; }
:global(:root[data-theme] .repo-detail-dialog .el-dialog__body) {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.repo-detail-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  -webkit-overflow-scrolling: touch;
}
.repo-detail-content:focus-visible { outline: 2px solid var(--accent-border); outline-offset: -2px; border-radius: 6px; }
:global(:root[data-theme] .repo-detail-dialog .el-descriptions__table) { width: 100%; table-layout: fixed; }
:global(:root[data-theme] .repo-detail-dialog .el-descriptions__label) { width: 92px; }
:global(:root[data-theme] .repo-detail-dialog .el-descriptions__cell) { overflow-wrap: anywhere; }

.commits-section { margin-top: 20px; }
.commits-section h4 { font-size: 15px; font-weight: 600; color: var(--text-primary); margin-bottom: 12px; }
.commit-list { display: flex; flex-direction: column; gap: 8px; }
.commit-item { display: grid; grid-template-columns: 64px minmax(0, 1fr) minmax(0, 112px); gap: 12px; align-items: start; padding: 10px 12px; border-radius: 10px; font-size: 13px; }
.commit-hash { font-family: var(--font-mono); color: var(--accent); font-weight: 500; }
.commit-msg { min-width: 0; color: var(--text-primary); overflow-wrap: anywhere; white-space: pre-wrap; line-height: 1.6; }
.commit-author { min-width: 0; color: var(--text-faint); font-size: 12px; overflow-wrap: anywhere; }
.no-commits { padding: 20px; text-align: center; }
.detail-path { font-family: var(--font-mono); font-size: 12px; color: var(--accent); word-break: break-all; }
.language-tags { display: flex; gap: 6px; flex-wrap: wrap; }
.language-tags :deep(.el-tag) { max-width: 100%; height: auto; min-height: 24px; white-space: normal; overflow-wrap: anywhere; }

@media (max-width: 768px) {
  :global(:root[data-theme] .repo-detail-dialog) { height: min(720px, calc(var(--visual-viewport-height, 100dvh) - var(--safe-top, 0px) - 24px)); }
  :global(:root[data-theme] .repo-detail-dialog .el-dialog__body) { padding-bottom: calc(16px + var(--safe-bottom, 0px)) !important; }
  .repo-grid { grid-template-columns: minmax(0, 1fr); gap: 12px; }
  .repo-card { padding: 14px; border-radius: 14px; }
  .repo-actions { display: grid; grid-template-columns: 1fr 1fr; }
  .repo-actions :deep(.el-button) { width: 100%; margin: 0; }
  .commit-item { display: grid; grid-template-columns: 64px minmax(0, 1fr); gap: 5px 8px; align-items: start; }
  .commit-author { grid-column: 2; }
  .detail-path { font-size: 11px; }
  :deep(.el-descriptions__label) { width: 92px !important; min-width: 92px !important; }
  :deep(.el-dialog__footer) { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  :deep(.el-dialog__footer .el-button) { width: 100%; margin: 0; }
}
</style>
