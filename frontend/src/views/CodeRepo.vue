<template>
  <div class="code-repo-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">代码仓管理</h1>
        <p class="page-subtitle">注册本地 Git 仓库，自动提取元信息，关联笔记</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadRepos" :loading="loading" size="small">
          <el-icon><Refresh /></el-icon> 刷新
        </el-button>
        <el-button type="primary" size="small" @click="showAddDialog = true">
          <el-icon><Plus /></el-icon> 注册仓库
        </el-button>
      </div>
    </header>

    <!-- 仓库列表 -->
    <div class="repo-grid" v-if="repos.length > 0">
      <div v-for="repo in repos" :key="repo.name" class="repo-card">
        <div class="repo-header">
          <div class="repo-name">{{ repo.name }}</div>
          <el-tag :type="repo.is_dirty ? 'warning' : 'success'" size="small">
            {{ repo.is_dirty ? '有未提交' : '干净' }}
          </el-tag>
        </div>
        <div class="repo-branch">
          <el-icon><Connection /></el-icon>
          {{ repo.current_branch }}
        </div>
        <div class="repo-path" :title="repo.path">{{ repo.path }}</div>
        <div class="repo-meta" v-if="repo.languages && Object.keys(repo.languages).length > 0">
          <span v-for="(ratio, lang) in repo.languages" :key="lang" class="lang-tag">
            {{ lang }} {{ (ratio * 100).toFixed(0) }}%
          </span>
        </div>
        <div class="repo-actions">
          <el-button size="small" @click="viewDetail(repo.name)">详情</el-button>
          <el-button size="small" @click="openVscode(repo.name)">VSCode</el-button>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!loading" description="暂无注册的代码仓库">
      <el-button type="primary" @click="showAddDialog = true">注册第一个仓库</el-button>
    </el-empty>

    <!-- 仓库详情对话框 -->
    <el-dialog v-model="showDetailDialog" title="仓库详情" width="720px" v-if="selectedRepo">
      <el-descriptions :column="2" border>
        <el-descriptions-item label="名称" :min-width="120">{{ selectedRepo.name }}</el-descriptions-item>
        <el-descriptions-item label="分支" :min-width="120">{{ selectedRepo.current_branch }}</el-descriptions-item>
        <el-descriptions-item label="HEAD" :min-width="120">{{ selectedRepo.head_hash?.substring(0, 7) || '-' }}</el-descriptions-item>
        <el-descriptions-item label="总提交数" :min-width="120">{{ selectedRepo.total_commits || 0 }}</el-descriptions-item>
        <el-descriptions-item label="贡献者" :span="2">
          {{ selectedRepo.contributors?.join(', ') || '无' }}
        </el-descriptions-item>
        <el-descriptions-item label="路径" :span="2">
          <span class="detail-path">{{ selectedRepo.path }}</span>
        </el-descriptions-item>
        <el-descriptions-item label="语言统计" :span="2" v-if="selectedRepo.languages && Object.keys(selectedRepo.languages).length > 0">
          <div class="language-tags">
            <el-tag v-for="(ratio, lang) in selectedRepo.languages" :key="lang" size="small" effect="plain">
              {{ lang }} {{ (ratio * 100).toFixed(0) }}%
            </el-tag>
          </div>
        </el-descriptions-item>
      </el-descriptions>

      <div class="commits-section">
        <h4>最近提交</h4>
        <div v-if="selectedRepo.recent_commits && selectedRepo.recent_commits.length > 0" class="commit-list">
          <div v-for="commit in selectedRepo.recent_commits.slice(0, 5)" :key="commit.hash" class="commit-item">
            <span class="commit-hash">{{ commit.hash?.substring(0, 7) || '-' }}</span>
            <span class="commit-msg">{{ commit.message || '无提交消息' }}</span>
            <span class="commit-author">{{ commit.author || '未知' }}</span>
          </div>
        </div>
        <div v-else class="no-commits">
          <el-empty description="暂无提交记录" :image-size="60" />
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
import { ref, onMounted } from 'vue'
import { addCodeRepo, listCodeRepos, getRepoDetail, openInVscode } from '@/api'
import { Refresh, Plus, Connection } from '@element-plus/icons-vue'
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

onMounted(() => { loadRepos() })
</script>

<style scoped>
.code-repo-page {
  max-width: 100%;
  min-height: 100%;
}
.code-repo-page .repo-card {
  animation: pageFadeIn 0.5s ease both;
}
.code-repo-page .repo-card:nth-child(2) { animation-delay: 0.06s; }
.code-repo-page .repo-card:nth-child(3) { animation-delay: 0.12s; }
.code-repo-page .repo-card:nth-child(4) { animation-delay: 0.18s; }
@keyframes pageFadeIn {
  from { opacity: 0; transform: translateY(20px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.repo-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 16px; }
.repo-card { padding: 20px; border-radius: 16px; transition: box-shadow 0.2s ease; }
.repo-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.04); }
.repo-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; }
.repo-name { font-size: 16px; font-weight: 600; color: #18181b; }
.repo-branch { display: flex; align-items: center; gap: 4px; font-size: 13px; color: #6366f1; margin-bottom: 4px; }
.repo-path {
  font-size: 12px; color: #a1a1aa; font-family: monospace; margin-bottom: 12px;
  word-break: break-all; overflow: hidden; text-overflow: ellipsis;
  display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; max-height: 2.4em;
}
.repo-meta { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 12px; }
.lang-tag { font-size: 11px; padding: 2px 8px; border-radius: 8px; color: #52525b; }
.repo-actions { display: flex; gap: 8px; }

.commits-section { margin-top: 20px; }
.commits-section h4 { font-size: 15px; font-weight: 600; color: #18181b; margin-bottom: 12px; }
.commit-list { display: flex; flex-direction: column; gap: 8px; }
.commit-item { display: flex; gap: 12px; align-items: center; padding: 10px 12px; border-radius: 10px; font-size: 13px; }
.commit-hash { font-family: monospace; color: #6366f1; min-width: 60px; font-weight: 500; }
.commit-msg { flex: 1; color: #18181b; }
.commit-author { color: #a1a1aa; font-size: 12px; }
.no-commits { padding: 20px; text-align: center; }
.detail-path { font-family: monospace; font-size: 12px; color: #6366f1; word-break: break-all; }
.language-tags { display: flex; gap: 6px; flex-wrap: wrap; }

/* 弹窗表格样式 - 解决圆角缺口问题 */
.detail-descriptions {
  width: 100%;
  border-radius: 12px !important;
  overflow: hidden !important;
}

.detail-descriptions .el-descriptions__table {
  width: 100%;
  border-collapse: separate !important;
  border-spacing: 0 !important;
}

.detail-descriptions .el-descriptions__cell {
  padding: 12px 16px !important;
  border: 1px solid #ebeef5 !important;
}

.detail-descriptions .el-descriptions__label {
  min-width: 100px;
  font-weight: 500;
  background-color: #f5f7fa !important;
}

.detail-descriptions .el-descriptions__content {
  min-width: 150px;
  background-color: #fff !important;
}

/* 修复四角圆角 */
.detail-descriptions .el-descriptions__body .el-descriptions__table .el-descriptions__cell:first-child {
  border-top-left-radius: 12px !important;
}
.detail-descriptions .el-descriptions__body .el-descriptions__table .el-descriptions__cell:last-child {
  border-top-right-radius: 12px !important;
}
.detail-descriptions .el-descriptions__body .el-descriptions__table tr:last-child .el-descriptions__cell:first-child {
  border-bottom-left-radius: 12px !important;
}
.detail-descriptions .el-descriptions__body .el-descriptions__table tr:last-child .el-descriptions__cell:last-child {
  border-bottom-right-radius: 12px !important;
}
</style>

<style>
/* 全局样式 - 圆角弹窗和表格 */
.el-dialog { border-radius: 16px !important; overflow: hidden; }
.el-dialog__header { border-radius: 16px 16px 0 0 !important; padding: 20px 24px 16px !important; }
.el-dialog__body { padding: 16px 24px 24px !important; }
.el-descriptions { border-radius: 12px !important; overflow: hidden; }
.el-descriptions__table { border-radius: 12px !important; }
</style>
