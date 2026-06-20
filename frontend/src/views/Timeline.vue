<template>
  <div class="timeline-page">

    <header class="page-header">
      <div>
        <h1 class="page-title">时光机</h1>
        <p class="page-subtitle">记录碎片化想法，回顾思考历程</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="doSync" :loading="syncing">
          <el-icon v-if="!syncing"><Refresh /></el-icon>
          同步
        </el-button>
        <el-button size="small" type="primary" @click="showCreateDialog = true">
          <el-icon><Plus /></el-icon>
          写小记
        </el-button>
      </div>
    </header>

      <!-- Toolbar -->
      <div class="toolbar">
        <div class="toolbar-row">
          <div class="search-box glass-surface">
          <el-icon class="search-icon"><Search /></el-icon>
          <input
            v-model="searchQuery"
            placeholder="搜索小记..."
            class="glass-input"
            @input="onSearchInput"
          />
          <button v-if="searchQuery" class="clear-btn" @click="clearSearch">✕</button>
        </div>

        <div class="filter-right">
          <div class="preset-chips">
            <div class="chip-track">
              <div
                v-for="preset in timePresets"
                :key="preset.label"
                class="chip"
                :class="{ active: activePreset === preset.label }"
                @click="applyPreset(preset)"
              >
                {{ preset.label }}
              </div>
            </div>
          </div>

          <div class="date-range-picker">
            <el-date-picker
              v-model="customDateRange"
              type="daterange"
              range-separator="→"
              start-placeholder="起始"
              end-placeholder="结束"
              size="default"
              popper-class="glass-picker"
              @change="onCustomDateChange"
              :clearable="true"
            />
            <button
              v-if="hasActiveFilter"
              class="glass-icon-btn clear-filter"
              @click="clearFilter"
              title="清除筛选"
            >
              ✕
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Main Content -->
    <div class="main-content">
      <!-- Left Timeline Nav -->
      <aside class="time-nav" v-if="timelineMonths.length > 0">
        <div class="time-nav-inner">
          <div v-for="month in timelineMonths" :key="month.key" class="month-group">
            <div class="month-label">{{ month.label }}</div>
            <div class="month-days">
              <div
                v-for="day in month.days"
                :key="day.date"
                class="day-link"
                :class="{ active: selectedDate === day.date }"
                @click="scrollToDate(day.date)"
              >
                <span class="day-dot"></span>
                <span class="day-line"></span>
                <span class="day-text">{{ day.label }}</span>
                <span class="day-count">{{ day.count }}</span>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- Right Memo List -->
      <div class="memo-scroll" @scroll="onMemoScroll">
        <!-- Filter hint -->
        <Transition name="hint">
          <div v-if="hasActiveFilter && !loading && filteredMemos.length > 0" class="filter-hint glass-surface">
            <span>当前筛选：{{ filteredMemos.length }} 条结果</span>
            <button @click="clearFilter">清除</button>
          </div>
        </Transition>

        <TransitionGroup
          v-if="filteredMemos.length > 0"
          name="memo-anim"
          tag="div"
          class="memo-list"
        >
          <div v-for="group in groupedMemos" :key="group.date" class="memo-day-group">
            <div class="day-group-header" :id="'date-' + group.date">
              <div class="day-header-left">
                <span class="day-header-date">{{ formatGroupDate(group.date) }}</span>
                <span class="day-header-weekday">{{ formatWeekday(group.date) }}</span>
              </div>
              <span class="day-header-count glass-chip">{{ group.memos.length }}</span>
            </div>

            <div class="day-group-memos">
                <div v-for="(memo, idx) in group.memos" :key="memo.id" class="memo-card" :style="{ '--delay': idx * 0.06 + 's' }">
                <div class="memo-card-left">
                  <div class="memo-time-dot"></div>
                  <div class="memo-time-line"></div>
                </div>
                <div class="memo-card-body glass-surface">
                  <div class="memo-time">{{ formatTime(memo.timestamp) }}</div>
                  <div class="memo-card-main" :class="{ 'has-images': memo.images.length > 0 }">
                    <div v-if="memo.images.length > 0" class="memo-images-wrap">
                      <div class="memo-images" :class="'memo-images-' + imageGridClass(memo.images.length)">
                        <img
                          v-for="(img, i) in memo.images"
                          :key="i"
                          :src="vaultImageUrl(img)"
                          class="memo-image"
                          @click="openImageViewer(memo.images, i)"
                        />
                      </div>
                    </div>
                    <div class="memo-card-text">
                      <div class="memo-content" v-html="renderContent(memo.content, searchQuery)"></div>
                    </div>
                  </div>
                  <div v-if="memo.tags.length > 0" class="memo-tags">
                    <span
                      v-for="tag in memo.tags"
                      :key="tag"
                      class="memo-tag glass-chip"
                      @click.stop="searchByTag(tag)"
                    >
                      #{{ tag }}
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </TransitionGroup>
        <div class="load-more-area" v-if="hasMore && filteredMemos.length > 0">
          <button class="glass-btn" @click="loadMore" :disabled="loadingMore">
            <el-icon v-if="loadingMore" class="is-loading"><Loading /></el-icon>
            <span>{{ loadingMore ? '加载中' : '加载更多' }}</span>
          </button>
        </div>
        <div class="all-loaded" v-else-if="filteredMemos.length > 0">
          <span class="all-loaded-line"></span>
          <span>已显示全部小记</span>
          <span class="all-loaded-line"></span>
        </div>

        <!-- Empty States -->
        <div v-if="filteredMemos.length === 0 && !loading" class="empty-state">
          <div class="empty-icon">📝</div>
          <div class="empty-title" v-if="searchQuery">没有找到匹配的小记</div>
          <div class="empty-title" v-else-if="hasActiveFilter">该时间范围内没有小记</div>
          <div class="empty-title" v-else>还没有小记</div>
          <div class="empty-hint" v-if="!searchQuery && !hasActiveFilter">
            点击右上角「写小记」开始记录
          </div>
        </div>

        <div v-if="loading" class="loading-state">
          <div class="loading-dots">
            <span></span><span></span><span></span>
          </div>
        </div>
      </div>
    </div>

    <!-- Create Dialog -->
    <Transition name="dialog">
      <div v-if="showCreateDialog" class="dialog-overlay" @click.self="showCreateDialog = false">
        <div class="dialog-content glass-surface-heavy">
          <div class="dialog-header">
            <h3>写小记</h3>
            <button class="glass-icon-btn" @click="showCreateDialog = false">✕</button>
          </div>
          <div class="create-form">
            <textarea
              v-model="newMemo.content"
              :rows="7"
              placeholder="写下你此刻的想法...（支持 Markdown：**加粗**、- 列表）"
              class="glass-textarea"
              autofocus
              @paste="onPaste"
            ></textarea>

            <!-- 图片预览区 -->
            <div class="image-preview-grid" v-if="pendingImages.length > 0">
              <div
                v-for="(img, idx) in pendingImages"
                :key="idx"
                class="image-preview-item"
              >
                <img :src="img.preview" class="image-preview-img" />
                <button class="image-remove-btn" @click="removePendingImage(idx)">✕</button>
                <div v-if="img.uploading" class="image-upload-overlay">
                  <el-icon class="is-loading"><Loading /></el-icon>
                </div>
              </div>
              <button
                v-if="pendingImages.length < 9"
                class="image-add-btn"
                @click="triggerFileInput"
              >
                <el-icon :size="24"><Plus /></el-icon>
              </button>
            </div>

            <div class="form-row">
              <div class="glass-surface tag-input-wrap">
                <el-icon class="tag-icon"><PriceTag /></el-icon>
                <input
                  v-model="tagsInput"
                  placeholder="标签，逗号分隔（如：灵感,想法）"
                  class="glass-input inline"
                />
              </div>
              <button class="glass-btn image-btn" @click="triggerFileInput" v-if="pendingImages.length === 0">
                <el-icon><Picture /></el-icon>
                <span>图片</span>
              </button>
            </div>
            <input
              ref="fileInputRef"
              type="file"
              accept="image/*"
              multiple
              style="display: none"
              @change="onFileSelect"
            />
          </div>
          <div class="dialog-footer">
            <span class="char-count" v-if="newMemo.content.length > 0">
              {{ newMemo.content.length }} 字
            </span>
            <div class="dialog-btns">
              <button class="glass-btn" @click="showCreateDialog = false">取消</button>
              <button
                class="glass-btn primary"
                @click="submitMemo"
                :disabled="!newMemo.content.trim() || creating"
              >
                <el-icon v-if="creating" class="is-loading"><Loading /></el-icon>
                <span>发布小记</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Image Viewer Modal -->
    <Transition name="viewer">
      <div v-if="imageViewer.show" class="image-viewer-overlay" @click.self="closeImageViewer">
        <button class="viewer-close" @click="closeImageViewer">✕</button>
        <button
          v-if="imageViewer.images.length > 1"
          class="viewer-nav viewer-prev"
          @click="viewerPrev"
        ><el-icon :size="22"><ArrowLeft /></el-icon></button>
        <div class="viewer-image-wrap">
          <img
            :src="vaultImageUrl(imageViewer.images[imageViewer.index])"
            class="viewer-image"
            @click.stop
          />
          <div v-if="imageViewer.images.length > 1" class="viewer-counter">
            {{ imageViewer.index + 1 }} / {{ imageViewer.images.length }}
          </div>
        </div>
        <button
          v-if="imageViewer.images.length > 1"
          class="viewer-nav viewer-next"
          @click="viewerNext"
        ><el-icon :size="22"><ArrowRight /></el-icon></button>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Plus, Search, PriceTag, Loading, Picture, Refresh, ArrowLeft, ArrowRight } from '@element-plus/icons-vue'
import hljs from 'highlight.js/lib/common'
import 'highlight.js/styles/github-dark.css'
import { createMemo, browseTimeline, searchMemos, uploadImages, syncMemos } from '@/api'

// ── Types ──
interface Memo {
  id: string
  timestamp: string
  content: string
  images: string[]
  tags: string[]
}
interface MemoDayGroup {
  date: string
  memos: Memo[]
}
interface TimelineMonth {
  key: string
  label: string
  days: { date: string; label: string; count: number }[]
}
interface TimePreset {
  label: string
  getRange: () => [string, string]
}

// ── State ──
const loading = ref(false)
const loadingMore = ref(false)
const creating = ref(false)
const syncing = ref(false)
const memos = ref<Memo[]>([])
const searchQuery = ref('')
const activePreset = ref('')
const customDateRange = ref<[Date, Date] | null>(null)
const selectedDate = ref('')
const hasMore = ref(true)
const showCreateDialog = ref(false)
const tagsInput = ref('')
const totalCount = ref(0)

const newMemo = ref({
  content: '',
  images: [] as string[],
  tags: [] as string[],
})

// ── Image Upload State ──
interface PendingImage {
  file: File
  preview: string
  uploading: boolean
  path?: string
}
const pendingImages = ref<PendingImage[]>([])
const fileInputRef = ref<HTMLInputElement | null>(null)

// ── Image Viewer State ──
const imageViewer = ref({
  show: false,
  images: [] as string[],
  index: 0,
})

function openImageViewer(images: string[], index: number) {
  imageViewer.value = { show: true, images, index }
  document.addEventListener('keydown', onViewerKeydown)
}
function closeImageViewer() {
  imageViewer.value.show = false
  document.removeEventListener('keydown', onViewerKeydown)
}
function viewerPrev() {
  const v = imageViewer.value
  v.index = (v.index - 1 + v.images.length) % v.images.length
}
function viewerNext() {
  const v = imageViewer.value
  v.index = (v.index + 1) % v.images.length
}
function onViewerKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') closeImageViewer()
  if (e.key === 'ArrowLeft') viewerPrev()
  if (e.key === 'ArrowRight') viewerNext()
}

onUnmounted(() => {
  document.removeEventListener('keydown', onViewerKeydown)
})

function triggerFileInput() {
  fileInputRef.value?.click()
}

function onFileSelect(e: Event) {
  const input = e.target as HTMLInputElement
  if (input.files) addImageFiles(Array.from(input.files))
  input.value = '' // reset so same file can be re-selected
}

function onPaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items
  if (!items) return
  const files: File[] = []
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      const file = item.getAsFile()
      if (file) files.push(file)
    }
  }
  if (files.length > 0) addImageFiles(files)
}

function addImageFiles(files: File[]) {
  const remaining = 9 - pendingImages.value.length
  const toAdd = files.slice(0, remaining)
  for (const file of toAdd) {
    const preview = URL.createObjectURL(file)
    pendingImages.value.push({ file, preview, uploading: false })
  }
}

function removePendingImage(idx: number) {
  const img = pendingImages.value[idx]
  if (img) URL.revokeObjectURL(img.preview)
  pendingImages.value.splice(idx, 1)
}

const PAGE_SIZE = 20

// ── Date Range ──
const activeDateRange = computed((): [string, string] | null => {
  if (activePreset.value) {
    const preset = timePresets.find(p => p.label === activePreset.value)
    if (preset) return preset.getRange()
  }
  if (customDateRange.value) {
    return [formatDateStr(customDateRange.value[0]), formatDateStr(customDateRange.value[1])]
  }
  return null
})
const hasActiveFilter = computed(() => !!activeDateRange.value)

function formatDateStr(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

const timePresets: TimePreset[] = [
  { label: '今天', getRange: () => { const t = formatDateStr(new Date()); return [t, t] } },
  { label: '7天', getRange: () => {
    const e = new Date(), s = new Date(Date.now() - 6 * 864e5)
    return [formatDateStr(s), formatDateStr(e)]
  }},
  { label: '30天', getRange: () => {
    const e = new Date(), s = new Date(Date.now() - 29 * 864e5)
    return [formatDateStr(s), formatDateStr(e)]
  }},
  { label: '本月', getRange: () => {
    const n = new Date(), s = new Date(n.getFullYear(), n.getMonth(), 1)
    return [formatDateStr(s), formatDateStr(n)]
  }},
  { label: '上月', getRange: () => {
    const n = new Date()
    const s = new Date(n.getFullYear(), n.getMonth() - 1, 1)
    const e = new Date(n.getFullYear(), n.getMonth(), 0)
    return [formatDateStr(s), formatDateStr(e)]
  }},
]

// ── Computed ──
const filteredMemos = computed(() => memos.value)

const groupedMemos = computed((): MemoDayGroup[] => {
  const groups = new Map<string, Memo[]>()
  for (const memo of filteredMemos.value) {
    const date = memo.timestamp.split('T')[0]
    if (!groups.has(date)) groups.set(date, [])
    groups.get(date)!.push(memo)
  }
  return Array.from(groups.entries())
    .sort((a, b) => b[0].localeCompare(a[0]))
    .map(([date, memos]) => ({ date, memos }))
})

const timelineMonths = computed((): TimelineMonth[] => {
  const monthMap = new Map<string, Map<string, number>>()
  for (const memo of filteredMemos.value) {
    const date = memo.timestamp.split('T')[0]
    const [year, month] = date.split('-')
    const monthKey = `${year}-${month}`
    if (!monthMap.has(monthKey)) monthMap.set(monthKey, new Map())
    const dayMap = monthMap.get(monthKey)!
    dayMap.set(date, (dayMap.get(date) || 0) + 1)
  }
  return Array.from(monthMap.entries())
    .sort((a, b) => b[0].localeCompare(a[0]))
    .map(([key, dayMap]) => {
      const [year, month] = key.split('-')
      return {
        key,
        label: `${year}年${parseInt(month)}月`,
        days: Array.from(dayMap.entries())
          .sort((a, b) => b[0].localeCompare(a[0]))
          .map(([date, count]) => ({ date, label: `${parseInt(date.split('-')[2])}日`, count })),
      }
    })
})

// ── Data Loading ──
async function loadMemos(reset = true) {
  if (reset) { loading.value = true; memos.value = []; hasMore.value = true }
  const range = activeDateRange.value
  const startDate = range?.[0], endDate = range?.[1]
  try {
    let res: unknown
    if (searchQuery.value) {
      res = await searchMemos(searchQuery.value, startDate, endDate, undefined, PAGE_SIZE)
    } else {
      res = await browseTimeline(startDate, endDate, PAGE_SIZE, reset ? 0 : memos.value.length)
    }
    const result = (res as { result: { memos: Memo[]; has_more?: boolean; total?: number } })?.result
    const newMemos = result?.memos || []
    if (reset) { memos.value = newMemos } else { memos.value = [...memos.value, ...newMemos] }
    hasMore.value = newMemos.length >= PAGE_SIZE
    totalCount.value = result?.total ?? memos.value.length
  } catch (e) {
    console.error('加载小记失败:', e)
    ElMessage.error('加载小记失败')
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

async function loadMore() {
  if (loadingMore.value || !hasMore.value) return
  loadingMore.value = true
  await loadMemos(false)
}

async function doSync() {
  syncing.value = true
  try {
    const res = await syncMemos(3) as unknown as { result: { synced: number; deleted: number } }
    const count = res.result?.synced ?? 0
    const deleted = res.result?.deleted ?? 0
    ElMessage.success(`同步完成：${count} 条小记${deleted > 0 ? `，删除 ${deleted} 条` : ''}`)
    await loadMemos()
  } catch (e) {
    console.error('同步失败:', e)
    ElMessage.error('同步失败')
  } finally {
    syncing.value = false
  }
}

// ── Search ──
let searchTimer: ReturnType<typeof setTimeout> | null = null
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => loadMemos(), 300)
}
function clearSearch() { searchQuery.value = ''; loadMemos() }
function searchByTag(tag: string) { searchQuery.value = tag; loadMemos() }

// ── Filter ──
function applyPreset(preset: TimePreset) {
  if (activePreset.value === preset.label) { clearFilter(); return }
  activePreset.value = preset.label
  customDateRange.value = null
  loadMemos()
}
function onCustomDateChange(_val: [Date, Date] | null) {
  activePreset.value = ''
  loadMemos()
}
function clearFilter() {
  activePreset.value = ''; customDateRange.value = null; loadMemos()
}

// ── Create ──
async function submitMemo() {
  if (!newMemo.value.content.trim() && pendingImages.value.length === 0) return
  creating.value = true
  try {
    const tags = tagsInput.value
      ? tagsInput.value.split(/[,，]/).map(t => t.trim()).filter(Boolean) : []

    // Upload images first
    let imagePaths: string[] = []
    if (pendingImages.value.length > 0) {
      pendingImages.value.forEach(img => { img.uploading = true })
      const files = pendingImages.value.map(img => img.file)
      const uploadRes = await uploadImages(files)
      imagePaths = uploadRes.paths || []
    }

    const res = await createMemo(newMemo.value.content, imagePaths, tags) as unknown as {
      result: { id: string; timestamp: string; file_path: string }
    }
    ElMessage.success('小记创建成功')

    // Prepend new memo to existing list
    const newMemoItem: Memo = {
      id: res.result.id,
      timestamp: res.result.timestamp,
      content: newMemo.value.content,
      images: imagePaths,
      tags,
    }
    memos.value = [newMemoItem, ...memos.value]
    totalCount.value++

    // Cleanup
    pendingImages.value.forEach(img => URL.revokeObjectURL(img.preview))
    pendingImages.value = []
    newMemo.value = { content: '', images: [], tags: [] }
    tagsInput.value = ''
    showCreateDialog.value = false
  } catch (e) {
    console.error('创建小记失败:', e); ElMessage.error('创建小记失败')
  } finally {
    pendingImages.value.forEach(img => { img.uploading = false })
    creating.value = false
  }
}

// ── Formatting ──
function vaultImageUrl(path: string): string {
  return `/v1/vault/images/${path}`
}
function imageGridClass(count: number): string {
  if (count <= 1) return '1'
  if (count <= 3) return String(count)
  if (count === 4) return '4'
  if (count <= 6) return '5'
  return '7' // 7-9: 3×3 grid
}
function formatTime(ts: string) {
  return new Date(ts).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}
function formatGroupDate(date: string) {
  return new Date(date + 'T00:00:00').toLocaleDateString('zh-CN', { month: 'long', day: 'numeric' })
}
function formatWeekday(date: string) {
  return new Date(date + 'T00:00:00').toLocaleDateString('zh-CN', { weekday: 'short' })
}
function renderContent(content: string, query: string): string {
  // Phase 1: extract code blocks to protect them from later transforms
  const codeBlocks: string[] = []
  let html = content.replace(/```(\w*)\n?([\s\S]*?)```/g, (_m, lang, code) => {
    const i = codeBlocks.length
    const trimmed = code.trim()
    // Syntax highlight if language is specified
    let highlighted: string
    if (lang && hljs.getLanguage(lang)) {
      highlighted = hljs.highlight(trimmed, { language: lang }).value
    } else {
      highlighted = trimmed.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    }
    codeBlocks.push(`<pre class="memo-code"><code class="hljs language-${lang || 'plaintext'}">${highlighted}</code></pre>`)
    return `\x00CB${i}\x00`
  })

  // Phase 2: escape HTML in remaining text
  html = html.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

  // Phase 3: inline elements
  html = html.replace(/`([^`\n]+)`/g, '<code class="memo-inline-code">$1</code>')
  html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img class="memo-inline-img" src="$2" alt="$1" />')
  html = html.replace(/!\[\[([^\]]+)\]\]/g, '<span class="memo-obsidian-img">📎 $1</span>')
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a class="memo-link" href="$2" target="_blank" rel="noopener">$1</a>')
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  html = html.replace(/(?<!\*)\*([^*\n]+)\*(?!\*)/g, '<em>$1</em>')
  html = html.replace(/~~(.+?)~~/g, '<del>$1</del>')
  html = html.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>')
  html = html.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>')
  html = html.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>')
  html = html.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>')
  html = html.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>')
  html = html.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>')
  html = html.replace(/^&gt;\s+(.+)$/gm, '<blockquote>$1</blockquote>')
  html = html.replace(/^(?:---|\*\*\*|___)$/gm, '<hr class="memo-hr" />')

  // Phase 4: lists
  // Checkboxes: - [ ] or - [x] (must process before regular list items)
  html = html.replace(/^[*-]\s+\[ \]\s+(.+)$/gm, '<li class="memo-checkbox"><span class="memo-check-box"></span>$1</li>')
  html = html.replace(/^[*-]\s+\[x\]\s+(.+)$/gim, '<li class="memo-checkbox memo-checkbox-checked"><span class="memo-check-box memo-check-checked"></span>$1</li>')
  // Unordered: - item or * item (skip already-processed checkboxes)
  html = html.replace(/^[*-]\s+(?!\[)(.+)$/gm, '<li>$1</li>')
  html = html.replace(/((?:<li[^>]*>.*<\/li>\n?)+)/g, '<ul>$1</ul>')
  // Ordered: 1. item
  html = html.replace(/^\d+\.\s+(.+)$/gm, '<oli>$1</oli>')
  html = html.replace(/((?:<oli>.*<\/oli>\n?)+)/g, (_m, items) => {
    return `<ol>${items.replace(/<\/?oli>/g, (t: string) => t.replace('oli', 'li'))}</ol>`
  })

  // Phase 5: merge consecutive blockquotes
  html = html.replace(/(<blockquote>.*<\/blockquote>\n?)+/g, m => {
    const inner = m.replace(/<\/?blockquote>\n?/g, '').trim()
    return `<blockquote>${inner}</blockquote>`
  })

  // Phase 6: paragraph splitting — split by double newlines, wrap text in <p>
  const blocks = html.split(/\n{2,}/)
  html = blocks.map(block => {
    block = block.trim()
    if (!block) return ''
    // Already a block-level element — don't wrap
    if (/^\s*<(h[1-6]|ul|ol|blockquote|pre|hr|div|table)/.test(block)) {
      return block
    }
    // Code block placeholder — don't wrap
    if (/^\x00CB\d+\x00$/.test(block)) {
      return block
    }
    // Text block — wrap in <p>, convert single newlines to <br>
    return `<p>${block.replace(/\n/g, '<br>')}</p>`
  }).filter(Boolean).join('\n')

  // Phase 7: restore code blocks
  html = html.replace(/\x00CB(\d+)\x00/g, (_m, i) => codeBlocks[parseInt(i)])

  // Phase 8: search highlight (only in text nodes, not tags)
  if (query) {
    const esc = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const re = new RegExp(`(${esc})`, 'gi')
    html = html.replace(/>([^<]+)</g, (_m, t) => `>${t.replace(re, '<mark>$1</mark>')}<`)
  }

  return html
}
function scrollToDate(date: string) {
  selectedDate.value = date
  document.getElementById('date-' + date)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

function onMemoScroll(e: Event) {
  const el = e.target as HTMLElement
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 100) loadMore()
}

onMounted(() => { loadMemos() })
</script>

<style scoped>
/* ── Page Root ── */
.timeline-page {
  min-height: 100%;
  position: relative;
  max-width: 100%;
}

/* ── Glass Surfaces ── */
.glass-surface {
  background: rgba(255, 255, 255, 0.55);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.6);
  box-shadow:
    0 1px 2px rgba(0, 0, 0, 0.03),
    0 4px 16px rgba(0, 0, 0, 0.04),
    inset 0 1px 0 rgba(255, 255, 255, 0.5);
}
.glass-surface-heavy {
  background: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(40px) saturate(200%);
  -webkit-backdrop-filter: blur(40px) saturate(200%);
  border: 1px solid rgba(255, 255, 255, 0.7);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.08),
    0 2px 8px rgba(0, 0, 0, 0.04),
    inset 0 1px 0 rgba(255, 255, 255, 0.6);
}
.glass-chip {
  background: rgba(255, 255, 255, 0.45);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.5);
}

/* ── Glass Buttons ── */
.glass-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 9px 20px;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.5);
  background: rgba(255, 255, 255, 0.5);
  backdrop-filter: blur(16px) saturate(180%);
  -webkit-backdrop-filter: blur(16px) saturate(180%);
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
}
.glass-btn:hover {
  background: rgba(255, 255, 255, 0.7);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
  transform: translateY(-1px);
}
.glass-btn:active {
  transform: translateY(0) scale(0.97);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
}
.glass-btn:disabled {
  opacity: 0.5;
  pointer-events: none;
}
.glass-btn.primary {
  background: rgba(24, 24, 27, 0.85);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border-color: rgba(255, 255, 255, 0.1);
  color: #fff;
}
.glass-btn.primary:hover {
  background: rgba(24, 24, 27, 0.95);
  box-shadow: 0 4px 20px rgba(24, 24, 27, 0.2);
}

.glass-icon-btn {
  width: 32px; height: 32px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.4);
  background: rgba(255, 255, 255, 0.3);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}
.glass-icon-btn:hover {
  background: rgba(255, 255, 255, 0.6);
  color: var(--text-primary);
}

/* ── Glass Input ── */
.glass-input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: 14px;
  color: var(--text-primary);
  font-family: inherit;
  padding: 0;
}
.glass-input::placeholder {
  color: var(--text-faint);
}

/* ── Glass Textarea ── */
.glass-textarea {
  width: 100%;
  border: 1px solid rgba(255, 255, 255, 0.5);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.35);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  padding: 14px 16px;
  font-size: 14px;
  font-family: inherit;
  color: var(--text-primary);
  line-height: 1.6;
  resize: vertical;
  outline: none;
  transition: all 0.25s ease;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.03);
}
.glass-textarea::placeholder { color: var(--text-faint); }
.glass-textarea:focus {
  border-color: rgba(129, 140, 248, 0.4);
  box-shadow: 0 0 0 3px rgba(129, 140, 248, 0.1), inset 0 1px 2px rgba(0, 0, 0, 0.03);
}

/* ── Header ── */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 24px;
}
.page-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}
.page-subtitle {
  margin-top: 4px;
  color: var(--text-faint);
  font-size: 14px;
}
.header-actions {
  display: flex;
  gap: 8px;
}

/* ── Toolbar ── */
.toolbar {
  margin-bottom: 16px;
  padding: 8px 0;
}
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.filter-right {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}
.search-box {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 14px;
  height: 40px;
  border-radius: 12px;
  flex: 1;
  max-width: 320px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
.search-box:focus-within {
  background: rgba(255, 255, 255, 0.75);
  box-shadow:
    0 0 0 3px rgba(129, 140, 248, 0.12),
    0 4px 16px rgba(0, 0, 0, 0.06),
    inset 0 1px 0 rgba(255, 255, 255, 0.5);
}
.search-icon {
  color: var(--text-faint);
  flex-shrink: 0;
  transition: color 0.2s ease;
}
.search-box:focus-within .search-icon {
  color: #818cf8;
}
.clear-btn {
  width: 22px; height: 22px;
  border-radius: 50%;
  border: none;
  background: rgba(0, 0, 0, 0.06);
  color: var(--text-muted);
  font-size: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
  flex-shrink: 0;
}
.clear-btn:hover {
  background: rgba(0, 0, 0, 0.1);
  color: var(--text-primary);
}

.preset-chips {
  padding: 0;
  background: transparent;
}
.chip-track {
  display: flex;
  gap: 2px;
}
.chip {
  padding: 6px 12px;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  user-select: none;
}
.chip:hover {
  color: var(--text-secondary);
  background: rgba(255, 255, 255, 0.4);
}
.chip.active {
  color: #fff;
  background: rgba(24, 24, 27, 0.8);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  box-shadow: 0 2px 8px rgba(24, 24, 27, 0.15);
}

/* ── Date Picker (scoped) ── */
.date-range-picker {
  display: flex;
  align-items: center;
  gap: 6px;
}
.date-range-picker :deep(.el-range-editor) {
  border-radius: 14px !important;
  border: 1px solid rgba(255, 255, 255, 0.5) !important;
  background: rgba(255, 255, 255, 0.45) !important;
  backdrop-filter: blur(16px) saturate(180%);
  -webkit-backdrop-filter: blur(16px) saturate(180%);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.03), inset 0 1px 0 rgba(255, 255, 255, 0.4) !important;
  height: 40px !important;
  padding: 0 12px !important;
  transition: all 0.25s ease !important;
}
.date-range-picker :deep(.el-range-editor:hover) {
  border-color: rgba(255, 255, 255, 0.7) !important;
  background: rgba(255, 255, 255, 0.6) !important;
}
.date-range-picker :deep(.el-range-editor.is-active) {
  border-color: rgba(129, 140, 248, 0.4) !important;
  box-shadow: 0 0 0 3px rgba(129, 140, 248, 0.1), 0 1px 3px rgba(0, 0, 0, 0.03) !important;
}
.date-range-picker :deep(.el-range-input) {
  background: transparent !important;
  color: var(--text-primary) !important;
  font-size: 13px !important;
}
.date-range-picker :deep(.el-range-input::placeholder) {
  color: var(--text-faint) !important;
}
.date-range-picker :deep(.el-range-separator) {
  color: var(--text-faint) !important;
  font-size: 13px !important;
}
.date-range-picker :deep(.el-range__icon),
.date-range-picker :deep(.el-range__close-icon) {
  color: var(--text-faint) !important;
}
.clear-filter {
  width: 28px; height: 28px;
  font-size: 11px;
}

/* ── Main Content ── */
.main-content {
  display: flex;
  gap: 20px;
}

/* ── Time Nav ── */
.time-nav {
  width: 160px;
  flex-shrink: 0;
  overflow-y: auto;
  max-height: calc(100vh - 280px);
  border-radius: 20px;
  padding: 16px 8px 16px 4px;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.08) transparent;
}
.month-group { margin-bottom: 18px; }
.month-label {
  font-size: 11px;
  font-weight: 700;
  color: var(--text-faint);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  margin-bottom: 6px;
  padding-left: 20px;
}
.month-days { position: relative; }
.day-link {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px 5px 6px;
  border-radius: 10px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-muted);
  transition: all 0.2s ease;
  position: relative;
}
.day-link:hover {
  background: rgba(255, 255, 255, 0.4);
  color: var(--text-secondary);
}
.day-link.active {
  background: rgba(129, 140, 248, 0.12);
  color: #4f46e5;
}
.day-dot {
  width: 8px; height: 8px;
  border-radius: 50%;
  border: 2px solid #d4d4d8;
  background: rgba(255, 255, 255, 0.6);
  flex-shrink: 0;
  z-index: 1;
  transition: all 0.3s ease;
}
.day-link.active .day-dot {
  border-color: #818cf8;
  background: #818cf8;
  box-shadow: 0 0 8px rgba(129, 140, 248, 0.5);
  animation: dotPulse 2s ease-in-out infinite;
}
.day-line {
  position: absolute;
  left: 13px;
  top: -3px;
  bottom: -3px;
  width: 2px;
  background: rgba(0, 0, 0, 0.06);
  z-index: 0;
}
.day-text { flex: 1; }
.day-count {
  font-size: 11px;
  color: var(--text-faint);
  min-width: 16px;
  text-align: center;
}

/* ── Memo Scroll ── */
.memo-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.06) transparent;
  contain: layout style;
}
.memo-list {
  position: relative;
}

/* ── Filter Hint ── */
.filter-hint {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 18px;
  border-radius: 14px;
  margin-bottom: 14px;
  font-size: 13px;
  color: #4f46e5;
}
.filter-hint button {
  border: none;
  background: rgba(99, 102, 241, 0.1);
  color: #6366f1;
  font-size: 12px;
  font-weight: 500;
  padding: 3px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
}
.filter-hint button:hover {
  background: rgba(99, 102, 241, 0.2);
}

/* ── Day Group ── */
.memo-day-group { margin-bottom: 28px; }
.day-group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
  padding-bottom: 10px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.04);
}
.day-header-left {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.day-header-date {
  font-size: 16px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}
.day-header-weekday {
  font-size: 12px;
  color: var(--text-faint);
}
.day-header-count {
  font-size: 11px;
  padding: 2px 10px;
  border-radius: 10px;
  color: #6366f1;
  font-weight: 600;
}

/* ── Memo Card ── */
.day-group-memos {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.memo-card {
  display: flex;
  gap: 14px;
  padding: 3px 0;
}
.memo-card-left {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 8px;
  flex-shrink: 0;
  padding-top: 8px;
}
.memo-time-dot {
  width: 8px; height: 8px;
  border-radius: 50%;
  background: rgba(129, 140, 248, 0.3);
  flex-shrink: 0;
  transition: all 0.3s ease;
}
.memo-card:hover .memo-time-dot {
  background: #818cf8;
  box-shadow: 0 0 8px rgba(129, 140, 248, 0.4);
}
.memo-time-line {
  width: 2px;
  flex: 1;
  background: rgba(0, 0, 0, 0.04);
  margin-top: 4px;
}
.memo-card-body {
  flex: 1;
  padding: 16px 20px;
  border-radius: 18px;
  margin-bottom: 6px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  transform: translateZ(0);
  will-change: transform;
  -webkit-backface-visibility: hidden;
  backface-visibility: hidden;
  contain: layout style paint;
}
.memo-card-body:hover {
  transform: translateY(-1px);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.06),
    0 2px 6px rgba(0, 0, 0, 0.03),
    inset 0 1px 0 rgba(255, 255, 255, 0.6);
}
.memo-time {
  font-size: 12px;
  color: var(--text-faint);
  margin-bottom: 8px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.3px;
}
.memo-content {
  font-size: 14px;
  color: var(--text-secondary);
  line-height: 1.75;
  word-break: break-word;
}
.memo-content :deep(mark) {
  background: rgba(253, 224, 71, 0.4);
  padding: 1px 4px;
  border-radius: 4px;
  color: inherit;
}
.memo-content :deep(strong) {
  font-weight: 700;
  color: var(--text-primary);
}
.memo-content :deep(em) {
  font-style: italic;
  color: #3f3f46;
}
.memo-content :deep(del) {
  text-decoration: line-through;
  color: var(--text-faint);
}

/* Code blocks — dark background with syntax highlighting */
.memo-content :deep(.memo-code) {
  background: #1e1e2e;
  padding: 14px 16px;
  border-radius: 10px;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  overflow-x: auto;
  margin: 10px 0;
  line-height: 1.6;
  border: 1px solid rgba(0, 0, 0, 0.1);
}
.memo-content :deep(.memo-code code) {
  background: none;
  padding: 0;
  font-size: inherit;
  color: #cdd6f4;
}
.memo-content :deep(.memo-code code.hljs) {
  background: none;
  padding: 0;
}
.memo-content :deep(.memo-inline-code) {
  background: rgba(24, 24, 27, 0.06);
  padding: 2px 7px;
  border-radius: 5px;
  font-size: 0.9em;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  color: #c026d3;
}

/* Lists — fix spacing when items contain code/links */
.memo-content :deep(ul) {
  padding-left: 20px;
  margin: 8px 0;
  list-style: disc;
}
.memo-content :deep(ol) {
  padding-left: 20px;
  margin: 8px 0;
  list-style: decimal;
}
.memo-content :deep(li) {
  margin-bottom: 4px;
  line-height: 1.75;
}
.memo-content :deep(li:last-child) {
  margin-bottom: 0;
}
.memo-content :deep(li::marker) {
  color: var(--text-faint);
}
/* Tighten spacing for inline elements inside list items */
.memo-content :deep(li code) {
  vertical-align: baseline;
}
.memo-content :deep(li a) {
  vertical-align: baseline;
}

/* Checkboxes */
.memo-content :deep(.memo-checkbox) {
  list-style: none;
  margin-left: -20px;
  padding-left: 0;
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.memo-content :deep(.memo-checkbox::marker) {
  content: '';
}
.memo-content :deep(.memo-check-box) {
  width: 16px;
  height: 16px;
  border: 2px solid #d4d4d8;
  border-radius: 4px;
  flex-shrink: 0;
  margin-top: 2px;
  position: relative;
}
.memo-content :deep(.memo-check-checked) {
  background: #6366f1;
  border-color: #6366f1;
}
.memo-content :deep(.memo-check-checked::after) {
  content: '✓';
  position: absolute;
  top: -2px;
  left: 1px;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
}
.memo-content :deep(.memo-checkbox-checked) {
  color: var(--text-faint);
  text-decoration: line-through;
}

/* Headings */
.memo-content :deep(h1),
.memo-content :deep(h2),
.memo-content :deep(h3),
.memo-content :deep(h4),
.memo-content :deep(h5),
.memo-content :deep(h6) {
  color: var(--text-primary);
  font-weight: 700;
  margin: 14px 0 6px;
  line-height: 1.35;
}
.memo-content :deep(h1) { font-size: 1.35em; }
.memo-content :deep(h2) { font-size: 1.2em; }
.memo-content :deep(h3) { font-size: 1.1em; }
.memo-content :deep(h4) { font-size: 1.05em; }
.memo-content :deep(h5) { font-size: 1em; }
.memo-content :deep(h6) { font-size: 0.95em; color: var(--text-muted); }

/* Blockquote */
.memo-content :deep(blockquote) {
  border-left: 3px solid rgba(129, 140, 248, 0.5);
  padding: 8px 14px;
  margin: 10px 0;
  color: var(--text-tertiary);
  background: rgba(129, 140, 248, 0.05);
  border-radius: 0 8px 8px 0;
  font-style: italic;
}

/* Links */
.memo-content :deep(.memo-link) {
  color: #6366f1;
  text-decoration: none;
  border-bottom: 1px solid rgba(99, 102, 241, 0.3);
  transition: border-color 0.2s ease;
}
.memo-content :deep(.memo-link:hover) {
  border-bottom-color: #6366f1;
}

/* Images */
.memo-content :deep(.memo-inline-img) {
  max-width: 100%;
  max-height: 200px;
  border-radius: 10px;
  margin: 8px 0;
  display: block;
}
.memo-content :deep(.memo-obsidian-img) {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  background: rgba(24, 24, 27, 0.05);
  border-radius: 8px;
  font-size: 13px;
  color: var(--text-tertiary);
}

/* Horizontal rule */
.memo-content :deep(.memo-hr) {
  border: none;
  height: 1px;
  background: linear-gradient(to right, transparent, rgba(0,0,0,0.1), transparent);
  margin: 14px 0;
}

/* Paragraphs */
.memo-content :deep(p) {
  margin: 0;
}
.memo-content :deep(p + p) {
  margin-top: 8px;
}
.memo-content :deep(p + h1),
.memo-content :deep(p + h2),
.memo-content :deep(p + h3) {
  margin-top: 12px;
}

/* Mobile markdown adjustments */
@media (max-width: 768px) {
  .memo-content { font-size: 13px; line-height: 1.7; }
  .memo-content :deep(.memo-code) { padding: 10px 12px; font-size: 12px; }
  .memo-content :deep(ul), .memo-content :deep(ol) { padding-left: 16px; }
}

.memo-time {
  font-size: 12px;
  color: var(--text-faint);
  margin-bottom: 8px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.3px;
}

/* ── Fixed-size image grid container ── */
.memo-card-main {
  display: flex;
  gap: 14px;
}
.memo-card-main.has-images {
  align-items: flex-start;
}
.memo-card-text {
  flex: 1;
  min-width: 0;
}
.memo-images-wrap {
  flex-shrink: 0;
  width: 200px;
  height: 200px;
  border-radius: 12px;
  overflow: hidden;
}
.memo-images {
  display: grid;
  width: 100%;
  height: 100%;
  gap: 2px;
}
/* 1 image: fills entire container */
.memo-images-1 {
  grid-template-columns: 1fr;
  grid-template-rows: 1fr;
}
/* 2 images: 2 cols × 1 row */
.memo-images-2 {
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: 1fr;
}
/* 3 images: 3 cols × 1 row */
.memo-images-3 {
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: 1fr;
}
/* 4 images: 2 cols × 2 rows (equal squares) */
.memo-images-4 {
  grid-template-columns: 1fr 1fr;
  grid-template-rows: 1fr 1fr;
}
/* 5-6 images: 3 cols × 2 rows */
.memo-images-5 {
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: 1fr 1fr;
}
/* 7-9 images: 3 cols × 3 rows */
.memo-images-7 {
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: repeat(3, 1fr);
}

.memo-image {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  object-fit: cover;
  cursor: pointer;
  display: block;
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
.memo-image:hover {
  transform: scale(1.05);
}

.memo-tags {
  display: flex;
  gap: 6px;
  margin-top: 12px;
  flex-wrap: wrap;
}
.memo-tag {
  font-size: 12px;
  color: #6366f1;
  padding: 3px 12px;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.25s ease;
  font-weight: 500;
}
.memo-tag:hover {
  background: rgba(99, 102, 241, 0.12);
  color: #4f46e5;
  transform: translateY(-1px);
}

/* ── Load More ── */
.load-more-area {
  display: flex;
  justify-content: center;
  padding: 24px 0;
}
.all-loaded {
  display: flex;
  align-items: center;
  gap: 14px;
  justify-content: center;
  color: var(--text-faint);
  font-size: 12px;
  padding: 24px 0;
}
.all-loaded-line {
  display: block;
  width: 40px;
  height: 1px;
  background: rgba(0, 0, 0, 0.06);
}

/* ── Empty & Loading ── */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 80px 20px;
  animation: fadeIn 0.5s ease;
}
.empty-icon {
  font-size: 56px;
  margin-bottom: 20px;
  animation: gentleBounce 3s ease-in-out infinite;
}
.empty-title { font-size: 16px; color: var(--text-tertiary); font-weight: 600; }
.empty-hint { font-size: 13px; color: var(--text-faint); margin-top: 8px; }

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px;
}
.loading-dots {
  display: flex;
  gap: 6px;
}
.loading-dots span {
  width: 8px; height: 8px;
  border-radius: 50%;
  background: rgba(129, 140, 248, 0.5);
  animation: loadingBounce 1.2s ease-in-out infinite;
}
.loading-dots span:nth-child(2) { animation-delay: 0.15s; }
.loading-dots span:nth-child(3) { animation-delay: 0.3s; }

/* ── Dialog ── */
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.15);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}
.dialog-content {
  width: 540px;
  max-width: 90vw;
  border-radius: 24px;
  padding: 28px;
}
.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}
.dialog-header h3 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}
.create-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.form-row { display: flex; gap: 8px; }
.tag-input-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
  height: 42px;
  border-radius: 14px;
  flex: 1;
}
.tag-icon { color: var(--text-faint); flex-shrink: 0; }
.glass-input.inline {
  height: 100%;
}

.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 20px;
}
.char-count { font-size: 12px; color: var(--text-faint); }
.dialog-btns { display: flex; gap: 10px; }
.image-btn { gap: 4px; }

/* ── Image Upload UI ── */
.image-preview-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-top: 8px;
}
.image-preview-item {
  position: relative;
  aspect-ratio: 1;
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.5);
}
.image-preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.image-remove-btn {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: none;
  background: rgba(0, 0, 0, 0.5);
  color: #fff;
  font-size: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s ease;
}
.image-remove-btn:hover {
  background: rgba(0, 0, 0, 0.7);
}
.image-upload-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-size: 20px;
}
.image-add-btn {
  aspect-ratio: 1;
  border-radius: 10px;
  border: 2px dashed rgba(0, 0, 0, 0.12);
  background: rgba(255, 255, 255, 0.3);
  color: var(--text-faint);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
}
.image-add-btn:hover {
  border-color: rgba(0, 0, 0, 0.2);
  background: rgba(255, 255, 255, 0.5);
  color: var(--text-muted);
}

/* ── Transitions ── */
.dialog-enter-active {
  transition: opacity 0.35s cubic-bezier(0.4, 0, 0.2, 1);
}
.dialog-enter-active .dialog-content {
  transition: transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1), opacity 0.35s ease;
}
.dialog-leave-active {
  transition: opacity 0.2s ease;
}
.dialog-leave-active .dialog-content {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.dialog-enter-from {
  opacity: 0;
}
.dialog-enter-from .dialog-content {
  transform: scale(0.9) translateY(20px);
  opacity: 0;
}
.dialog-leave-to {
  opacity: 0;
}
.dialog-leave-to .dialog-content {
  transform: scale(0.95);
  opacity: 0;
}

.hint-enter-active { transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1); }
.hint-leave-active { transition: all 0.25s ease; }
.hint-enter-from, .hint-leave-to { opacity: 0; transform: translateY(-8px); }

/* ── Memo Animations ── */
.memo-card {
  animation: memoFadeIn 0.5s cubic-bezier(0.4, 0, 0.2, 1) both;
  animation-delay: var(--delay, 0s);
  transition: opacity 0.5s cubic-bezier(0.4, 0, 0.2, 1),
              transform 0.5s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes memoFadeIn {
  from {
    opacity: 0;
    transform: translateY(24px) scale(0.96);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

/* TransitionGroup animations for add/remove */
.memo-anim-enter-active {
  transition: opacity 0.45s cubic-bezier(0.4, 0, 0.2, 1),
              transform 0.45s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.memo-anim-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.memo-anim-enter-from {
  opacity: 0;
  transform: translateX(-30px) scale(0.95);
}
.memo-anim-leave-to {
  opacity: 0;
  transform: translateX(30px) scale(0.95);
}
.memo-anim-move {
  transition: transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

/* ── Keyframes ── */
@keyframes slideDown {
  from { opacity: 0; transform: translateY(-16px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes slideUp {
  from { opacity: 0; transform: translateY(20px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes slideRight {
  from { opacity: 0; transform: translateX(-16px); }
  to { opacity: 1; transform: translateX(0); }
}
@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}
@keyframes dotPulse {
  0%, 100% { box-shadow: 0 0 4px rgba(129, 140, 248, 0.3); }
  50% { box-shadow: 0 0 12px rgba(129, 140, 248, 0.6); }
}
@keyframes loadingBounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
  40% { transform: scale(1); opacity: 1; }
}
@keyframes gentleBounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-6px); }
}

/* ── Image Viewer Modal ── */
.image-viewer-overlay {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}
.viewer-close {
  position: absolute;
  top: 20px;
  right: 24px;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.3);
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
  font-size: 18px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
  z-index: 10;
}
.viewer-close:hover {
  background: rgba(255, 255, 255, 0.25);
}
.viewer-nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 48px;
  height: 48px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.3);
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
  font-size: 28px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s ease;
  z-index: 10;
  line-height: 1;
}
.viewer-nav:hover {
  background: rgba(255, 255, 255, 0.25);
}
.viewer-prev { left: 20px; }
.viewer-next { right: 20px; }
.viewer-image-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  max-width: 80vw;
  max-height: 80vh;
}
.viewer-image {
  max-width: 80vw;
  max-height: 75vh;
  object-fit: contain;
  border-radius: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
.viewer-counter {
  color: rgba(255, 255, 255, 0.7);
  font-size: 14px;
  font-weight: 500;
  padding: 4px 14px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}

/* Viewer transitions */
.viewer-enter-active { transition: opacity 0.3s ease; }
.viewer-enter-active .viewer-image { transition: transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1), opacity 0.3s ease; }
.viewer-leave-active { transition: opacity 0.2s ease; }
.viewer-enter-from { opacity: 0; }
.viewer-enter-from .viewer-image { transform: scale(0.9); opacity: 0; }
.viewer-leave-to { opacity: 0; }

/* ── Responsive ── */
@media (max-width: 768px) {
  .time-nav { display: none; }
  .timeline-page {
  min-height: 100%; overflow-x: hidden; width: 100%; }
  .toolbar-row { flex-wrap: wrap; }
  .search-box { max-width: 100%; min-width: 0; }
  .filter-right { flex-wrap: wrap; margin-left: 0; width: 100%; }
  .memo-card-main.has-images { flex-direction: column; }
  .memo-images-wrap { width: 100%; max-width: 280px; height: auto; aspect-ratio: 1; }
  .memo-images-1 .memo-image { max-height: 200px; }
  .memo-card-body { padding: 14px 16px; border-radius: 14px; }
  .preset-chips { overflow-x: auto; -webkit-overflow-scrolling: touch; }
  .chip-track { flex-wrap: nowrap; }
  .chip { flex-shrink: 0; padding: 6px 12px; font-size: 12px; }

  /* Dialog full-screen on mobile */
  .dialog-content {
    width: 100% !important;
    max-width: 100% !important;
    border-radius: 16px !important;
    margin: 0 8px;
  }
  .image-preview-grid { grid-template-columns: repeat(3, 1fr); gap: 6px; }

  /* Date picker responsive */
  .date-range-picker {
    width: 100%;
  }
  .date-range-picker :deep(.el-range-editor) {
    width: 100% !important;
  }
}
</style>

<style>
/* ── ElMessage 适配 ── */
.el-message {
  border-radius: 16px !important;
  border: 1px solid rgba(255, 255, 255, 0.6) !important;
  background: rgba(255, 255, 255, 0.75) !important;
  backdrop-filter: blur(32px) saturate(200%) !important;
  -webkit-backdrop-filter: blur(32px) saturate(200%) !important;
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.08),
    0 2px 8px rgba(0, 0, 0, 0.04),
    inset 0 1px 0 rgba(255, 255, 255, 0.5) !important;
  padding: 14px 22px !important;
}
.el-message .el-message__content {
  font-size: 14px !important;
  font-weight: 500 !important;
  color: var(--text-primary) !important;
}

/* ── Calendar Popup Responsive ── */
@media (max-width: 768px) {
  .glass-picker {
    max-width: calc(100vw - 16px) !important;
    left: 8px !important;
    right: 8px !important;
    width: auto !important;
    border-radius: 16px !important;
  }
  .glass-picker .el-date-range-picker__content {
    padding: 4px !important;
  }
  .glass-picker .el-date-range-picker__header,
  .glass-picker .el-date-picker__header {
    margin: 2px 4px !important;
    font-size: 13px !important;
  }
  .glass-picker .el-date-table {
    font-size: 12px !important;
  }
  .glass-picker .el-date-table td .el-date-table-cell {
    width: 32px !important;
    height: 32px !important;
  }
  .glass-picker .el-date-table th {
    font-size: 11px !important;
    padding: 4px 0 !important;
  }
  .glass-picker .el-picker-panel__footer {
    padding: 4px 8px !important;
  }
  .glass-picker .el-picker-panel__footer button {
    font-size: 12px !important;
    padding: 4px 12px !important;
  }
  .glass-picker .el-date-range-picker__time-picker-wrap .el-input {
    width: 100% !important;
  }
}
</style>
