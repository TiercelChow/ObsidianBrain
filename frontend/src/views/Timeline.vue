<template>
  <div class="timeline-page">

    <header class="page-header">
      <div>
        <h1 class="page-title">时光机</h1>
        <p class="page-subtitle">记录碎片化想法，回顾思考历程</p>
      </div>
      <div class="header-actions">
        <el-button class="desktop-sync-action" @click="storageOpen = true">
          <el-icon><FolderOpened /></el-icon>
          图片存储
        </el-button>
        <el-button type="primary" @click="openCreateDialog">
          <el-icon><Plus /></el-icon>
          写小记
        </el-button>
      </div>
    </header>

      <!-- Toolbar -->
      <div class="toolbar">
        <div class="toolbar-row">
        <div class="search-box glass-surface" data-glass-rim>
          <el-icon class="search-icon"><Search /></el-icon>
          <input
            v-model="searchQuery"
            placeholder="搜索小记..."
            class="glass-input"
            @input="onSearchInput"
          />
          <button v-if="searchQuery" type="button" class="clear-btn" aria-label="清除搜索" @click="clearSearch">✕</button>
        </div>

        <button v-if="isMobile" type="button" class="mobile-compose-action" aria-label="写小记" @click="openCreateDialog">
          <el-icon><Plus /></el-icon>
        </button>

        <button
          v-if="isMobile"
          type="button"
          class="mobile-filter-summary glass-surface"
          :class="{ active: hasActiveFilter || mobileFiltersOpen }"
          :aria-label="`时间范围：${rangeFilterLabel}`"
          :title="rangeFilterLabel"
          :aria-expanded="mobileFiltersOpen"
          aria-controls="timeline-mobile-filters"
          @click="mobileFiltersOpen = !mobileFiltersOpen"
        >
          <el-icon><MoreFilled /></el-icon>
        </button>

        <Transition name="filter-panel">
        <div v-if="!isMobile || mobileFiltersOpen" id="timeline-mobile-filters" class="filter-right">
          <div v-if="isMobile" class="mobile-filter-heading">
            <span>时间范围</span>
            <button type="button" class="mobile-sync-action" @click="storageOpen = true">
              <el-icon><FolderOpened /></el-icon>
              图片存储
            </button>
          </div>
          <div class="preset-chips">
            <div class="chip-track">
              <div
                v-for="preset in timePresets"
                :key="preset.label"
                class="chip"
                :class="{ active: activePreset === preset.label }"
                role="button"
                tabindex="0"
                @click="applyPreset(preset)"
                @keydown.enter.prevent="applyPreset(preset)"
                @keydown.space.prevent="applyPreset(preset)"
              >
                {{ preset.label }}
              </div>
            </div>
          </div>

          <div class="date-range-picker">
            <!-- Desktop: daterange picker (two panels) -->
            <el-date-picker
              v-if="!isMobile"
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
            <!-- Mobile: two single-date pickers (one panel each) -->
            <template v-else>
              <el-date-picker
                v-model="mobileStartDate"
                type="date"
                placeholder="起始"
                size="default"
                popper-class="glass-picker"
                @change="onMobileDateChange"
                :clearable="true"
                class="mobile-date-input"
              />
              <span class="mobile-date-sep">→</span>
              <el-date-picker
                v-model="mobileEndDate"
                type="date"
                placeholder="结束"
                size="default"
                popper-class="glass-picker"
                @change="onMobileDateChange"
                :clearable="true"
                class="mobile-date-input"
              />
            </template>
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
        </Transition>
      </div>
    </div>

    <!-- Main Content -->
    <div class="main-content">
      <Transition name="published-notice">
        <button v-if="showPublishedNotice" type="button" class="published-notice" @click="viewLatestMemo">
          <span>小记已发布</span>
          <strong>查看最新</strong>
        </button>
      </Transition>
      <!-- Left Timeline Nav -->
      <aside class="time-nav" data-glass="structural" v-if="timelineMonths.length > 0">
        <div class="time-nav-inner">
          <div v-for="month in timelineMonths" :key="month.key" class="month-group">
            <div class="month-label">{{ month.label }}</div>
            <div class="month-days">
              <div
                v-for="day in month.days"
                :key="day.date"
                class="day-link"
                :class="{ active: selectedDate === day.date }"
                :data-date="day.date"
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
      <div class="memo-scroll" ref="memoScrollRef" @scroll="onMemoScroll">
        <!-- Filter hint -->
        <Transition name="hint">
          <div v-if="hasActiveFilter && !loading && filteredMemos.length > 0" class="filter-hint glass-surface">
            <span>当前筛选：{{ filteredMemos.length }} 条结果</span>
            <button @click="clearFilter">清除</button>
          </div>
        </Transition>

        <div v-if="locatedMemo" class="filter-hint glass-surface"><span>来自首页的小记</span><button type="button" @click="clearMemoLocation">查看全部小记</button></div>
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
                  <div class="memo-actions">
                    <button v-if="!isMobile" type="button" class="memo-action-btn" aria-label="编辑小记" title="编辑小记" @click="openEditDialog(memo)"><el-icon><Edit /></el-icon></button>
                    <button v-if="!isMobile" type="button" class="memo-action-btn danger" aria-label="删除小记" title="删除小记" @click="deleteTarget = memo"><el-icon><Delete /></el-icon></button>
                    <button v-else type="button" class="memo-action-btn" aria-label="小记操作" @click="actionMemo = memo"><el-icon><MoreFilled /></el-icon></button>
                  </div>
                  <div class="memo-card-main" :class="{ 'has-images': memo.images.length > 0 }">
                    <div v-if="memo.images.length > 0" class="memo-images-wrap">
                      <div class="memo-images" :class="'memo-images-' + imageGridClass(memo.images.length)">
                        <img
                          v-for="(img, i) in memo.images"
                          :key="i"
                          :src="thumbnailUrl(img)"
                          class="memo-image"
                          loading="lazy"
                          decoding="async"
                          :alt="`小记图片 ${i + 1}`"
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
            {{ isMobile ? '点击搜索栏旁的「＋」开始记录' : '点击右上角「写小记」开始记录' }}
          </div>
        </div>

        <div v-if="loading" class="loading-state">
          <div class="loading-dots">
            <span></span><span></span><span></span>
          </div>
        </div>
      </div>
    </div>

    <!-- Create Dialog: centered on desktop, velocity-aware bottom sheet on mobile. -->
    <MotionModal v-model="composerOpen" :aria-label="editedMemo ? '编辑小记' : '写小记'">
      <div class="dialog-content glass-surface-heavy">
          <div class="dialog-header">
            <h3>{{ editedMemo ? '编辑小记' : '写小记' }}</h3>
            <button type="button" class="glass-icon-btn" aria-label="关闭小记面板" :disabled="creating" @click="closeDraft">✕</button>
          </div>
          <div class="create-form">
            <textarea
              v-model="newMemo.content"
              :disabled="creating"
              :rows="7"
              placeholder="写下你此刻的想法...（支持 Markdown：**加粗**、- 列表）"
              class="glass-textarea"
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
                <button type="button" class="image-remove-btn" :aria-label="`移除第 ${idx + 1} 张图片`" :disabled="creating" @click="removePendingImage(idx)">✕</button>
                <div v-if="img.uploading" class="image-upload-overlay">
                  <el-icon class="is-loading"><Loading /></el-icon>
                </div>
              </div>
              <button
                v-if="pendingImages.length < 9"
                type="button"
                class="image-add-btn"
                aria-label="继续添加图片"
                :disabled="creating"
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
                  :disabled="creating"
                  placeholder="标签，逗号分隔（如：灵感,想法）"
                  class="glass-input inline"
                />
              </div>
              <button type="button" class="glass-btn image-btn" :disabled="creating" @click="triggerFileInput" v-if="pendingImages.length === 0">
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
              <button type="button" class="glass-btn" @click="closeDraft" :disabled="creating">取消</button>
              <button
                type="button"
                class="glass-btn primary"
                @click="submitMemo"
                :disabled="(!newMemo.content.trim() && pendingImages.length === 0) || creating"
              >
                <el-icon v-if="creating" class="is-loading"><Loading /></el-icon>
                <span>{{ creating ? '正在保存…' : editedMemo ? '保存修改' : '发布小记' }}</span>
              </button>
            </div>
          </div>
      </div>
    </MotionModal>

    <MotionModal v-model="actionSheetOpen" aria-label="小记操作">
      <div class="dialog-content glass-surface-heavy memo-action-sheet compact-dialog">
        <div class="dialog-header"><h3>小记操作</h3><button type="button" class="glass-icon-btn" aria-label="关闭操作" @click="actionMemo = null">✕</button></div>
        <button type="button" class="glass-btn" @click="editFromSheet"><el-icon><Edit /></el-icon>编辑小记</button>
        <button type="button" class="glass-btn danger" @click="deleteFromSheet"><el-icon><Delete /></el-icon>删除小记</button>
      </div>
    </MotionModal>

    <MotionModal v-model="deleteOpen" aria-label="删除小记">
      <div class="dialog-content glass-surface-heavy compact-dialog">
        <div class="dialog-header"><h3>删除这条小记？</h3></div>
        <p class="memo-delete-hint">删除后无法恢复。这条小记不再使用的原图和缩略图也会清理；其他小记仍在使用的图片会保留。</p>
        <p class="memo-delete-preview">{{ deleteTarget?.content.slice(0, 160) || '图片小记' }}</p>
        <div class="dialog-footer"><div class="dialog-btns">
          <button type="button" class="glass-btn" :disabled="deleting" @click="deleteTarget = null">保留</button>
          <button type="button" class="glass-btn danger" :disabled="deleting" @click="confirmDelete">{{ deleting ? '正在删除…' : '删除小记及独占图片' }}</button>
        </div></div>
      </div>
    </MotionModal>

    <MotionModal v-model="storageOpen" aria-label="时光机图片存储">
      <div class="dialog-content glass-surface-heavy">
        <div class="dialog-header"><h3>图片存储</h3><button type="button" class="glass-icon-btn" aria-label="关闭图片存储" @click="storageOpen = false">✕</button></div>
        <div class="create-form"><TimelineStoragePanel @migrated="loadMemos()" /></div>
      </div>
    </MotionModal>

    <!-- Image Viewer Modal — teleported to body so .app-main.mobile-full's
         transform no longer reparents the fixed overlay away from the viewport. -->
    <Teleport to="body">
      <Transition name="viewer">
        <div v-if="imageViewer.show" class="image-viewer-overlay" @click.self="closeImageViewer">
          <button class="viewer-close" aria-label="关闭图片" @click="closeImageViewer">✕</button>
          <button
            v-if="imageViewer.images.length > 1"
            class="viewer-nav viewer-prev"
            @click="viewerPrev"
          ><el-icon :size="22"><ArrowLeft /></el-icon></button>
          <div class="viewer-image-wrap" ref="viewerStageRef">
            <img
              ref="viewerImgRef"
              :key="`${imageViewer.images[imageViewer.index]}:${viewerRetry}`"
              :src="memoImageUrl(imageViewer.images[imageViewer.index])"
              class="viewer-image"
              :class="{ 'viewer-image-pending': viewerStatus !== 'ready' }"
              alt="小记原图"
              @load="onViewerLoad"
              @error="viewerStatus = 'error'"
              @click.stop
            />
            <p v-if="viewerStatus === 'loading'" class="viewer-status" role="status">正在加载原图…</p>
            <div v-if="viewerStatus === 'error'" class="viewer-status" role="alert">
              <p>原图暂时无法读取。旧图片可在「图片存储」中重新迁移。</p>
              <button class="glass-btn" @click.stop="retryViewerImage">重试</button>
            </div>
            <div v-if="imageViewer.images.length > 1" class="viewer-counter">
              {{ imageViewer.index + 1 }} / {{ imageViewer.images.length }}
            </div>
          </div>
          <div v-if="viewerStatus === 'ready'" class="viewer-controls">
            <button class="viewer-zoom-btn" title="缩小" @click.stop="viewerZoom(0.8)"><el-icon :size="18"><Minus /></el-icon></button>
            <button class="viewer-zoom-btn" title="放大" @click.stop="viewerZoom(1.25)"><el-icon :size="18"><Plus /></el-icon></button>
            <button class="viewer-zoom-btn" title="重置" @click.stop="viewerReset"><el-icon :size="18"><Refresh /></el-icon></button>
          </div>
          <button
            v-if="imageViewer.images.length > 1"
            class="viewer-nav viewer-next"
            @click="viewerNext"
          ><el-icon :size="22"><ArrowRight /></el-icon></button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getMemo } from '@/api'
import { parseLocalDate } from '@/utils/taskDates'
import { ElMessage } from 'element-plus'
import { Plus, Minus, Search, PriceTag, Loading, Picture, Refresh, ArrowLeft, ArrowRight, MoreFilled, Edit, Delete, FolderOpened } from '@element-plus/icons-vue'
import hljs from 'highlight.js/lib/common'
import 'highlight.js/styles/github-dark.css'
import panzoom, { type PanZoom } from 'panzoom'
import { createMemo, browseTimeline, searchMemos, uploadImages, updateMemo, deleteMemo, discardMemoImages } from '@/api'
import { memoImageUrl, memoImageSource, memoLocalDate, memoToolResult, type TimelineMemo } from '@/utils/timelineMemo'
import TimelineStoragePanel from '@/components/timeline/TimelineStoragePanel.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { pickActiveDate, type SpyHeader } from '@/utils/timelineSpy'

// ── Types ──
type Memo = TimelineMemo
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
const route = useRoute()
const router = useRouter()
const locatedMemo = computed(() => typeof route.query.memo === 'string' ? route.query.memo : '')
const loadingMore = ref(false)
const creating = ref(false)
const memos = ref<Memo[]>([])
const searchQuery = ref('')
const activePreset = ref('')
const customDateRange = ref<[Date, Date] | null>(null)
const mobileStartDate = ref<Date | null>(null)
const mobileEndDate = ref<Date | null>(null)
const mobileFiltersOpen = ref(false)
const showPublishedNotice = ref(false)
let wasReviewingHistory = false

// Mobile detection
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => windowWidth.value <= 768)
function onResize() { windowWidth.value = window.innerWidth }
const selectedDate = ref('')
const memoScrollRef = ref<HTMLElement | null>(null)
// Scroll-spy: left-nav highlight follows the day group at the top of the memo list.
const SPY_THRESHOLD = 90
let spyRafId: number | null = null
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
  file?: File
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
const viewerStageRef = ref<HTMLDivElement | null>(null)
const viewerImgRef = ref<HTMLImageElement | null>(null)
const viewerStatus = ref<'loading' | 'ready' | 'error'>('loading')
const viewerRetry = ref(0)
let viewerPz: PanZoom | null = null

function onViewerLoad() {
  viewerStatus.value = 'ready'
  void nextTick(initViewerPz)
}
function retryViewerImage() {
  viewerStatus.value = 'loading'
  viewerRetry.value++
}

function initViewerPz() {
  if (!viewerImgRef.value) return
  viewerPz?.dispose()
  const img = viewerImgRef.value
  img.style.transform = ''
  img.style.transformOrigin = ''
  viewerPz = panzoom(img, {
    maxZoom: 8,
    minZoom: 0.1,
    zoomDoubleClickSpeed: 1.8,
    bounds: false,
  })
}
function viewerZoom(factor: number) {
  if (!viewerPz || !viewerStageRef.value) return
  const r = viewerStageRef.value.getBoundingClientRect()
  viewerPz.smoothZoom(r.width / 2 + r.left, r.height / 2 + r.top, factor)
}
function viewerReset() {
  if (!viewerPz || !viewerImgRef.value) return
  viewerPz.dispose()
  const img = viewerImgRef.value
  img.style.transform = ''
  img.style.transformOrigin = ''
  viewerPz = panzoom(img, {
    maxZoom: 8,
    minZoom: 0.1,
    zoomDoubleClickSpeed: 1.8,
    bounds: false,
  })
}
function openImageViewer(images: string[], index: number) {
  viewerStatus.value = 'loading'
  imageViewer.value = { show: true, images, index }
  document.addEventListener('keydown', onViewerKeydown)
}
function closeImageViewer() {
  imageViewer.value.show = false
  document.removeEventListener('keydown', onViewerKeydown)
  viewerPz?.dispose()
  viewerPz = null
}
function viewerPrev() {
  if (imageViewer.value.images.length < 2) return
  viewerPz?.dispose()
  viewerPz = null
  viewerStatus.value = 'loading'
  const v = imageViewer.value
  v.index = (v.index - 1 + v.images.length) % v.images.length
}
function viewerNext() {
  if (imageViewer.value.images.length < 2) return
  viewerPz?.dispose()
  viewerPz = null
  viewerStatus.value = 'loading'
  const v = imageViewer.value
  v.index = (v.index + 1) % v.images.length
}
function onViewerKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') closeImageViewer()
  if (e.key === 'ArrowLeft') viewerPrev()
  if (e.key === 'ArrowRight') viewerNext()
}

onMounted(() => {
  window.addEventListener('resize', onResize)
})
onUnmounted(() => {
  document.removeEventListener('keydown', onViewerKeydown)
  window.removeEventListener('resize', onResize)
  if (spyRafId !== null) cancelAnimationFrame(spyRafId)
  if (searchTimer) clearTimeout(searchTimer)
  void releaseDraftImages()
  pendingImages.value.forEach((image) => { if (image.file) URL.revokeObjectURL(image.preview) })
  viewerPz?.dispose()
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
  if (creating.value) return
  const remaining = 9 - pendingImages.value.length
  const toAdd = files.slice(0, remaining)
  for (const file of toAdd) {
    if (!['image/png', 'image/jpeg', 'image/gif', 'image/webp'].includes(file.type) || file.size > 20 * 1024 * 1024) { ElMessage.warning('仅支持 20 MB 以内的 PNG、JPEG、GIF、WebP 图片'); continue }
    const preview = URL.createObjectURL(file)
    pendingImages.value.push({ file, preview, uploading: false })
  }
}

function removePendingImage(idx: number) {
  const img = pendingImages.value[idx]
  if (creating.value) return
  if (img?.file) URL.revokeObjectURL(img.preview)
  if (img?.file && img.path) void discardMemoImages([img.path]).catch(() => ElMessage.warning('暂存图片清理将由系统自动重试'))
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
const rangeFilterLabel = computed(() => {
  if (activePreset.value) return activePreset.value === '7天' || activePreset.value === '30天'
    ? `最近 ${activePreset.value}`
    : activePreset.value
  if (customDateRange.value) {
    const [start, end] = customDateRange.value
    return `${start.getMonth() + 1}/${start.getDate()} – ${end.getMonth() + 1}/${end.getDate()}`
  }
  return '全部时间'
})

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
    const date = memoLocalDate(memo)
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
    const date = memoLocalDate(memo)
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
let loadSequence = 0
async function loadMemos(reset = true) {
  const sequence = ++loadSequence
  if (reset) { loading.value = true; hasMore.value = true }
  const range = activeDateRange.value
  const startDate = range?.[0], endDate = range?.[1]
  try {
    if (locatedMemo.value) {
      if (reset) memos.value = []
      const memo = await getMemo(locatedMemo.value)
      if (sequence !== loadSequence) return
      memos.value = [memo]; hasMore.value = false; totalCount.value = 1
      return
    }
    let res: unknown
    if (searchQuery.value) {
      res = await searchMemos(searchQuery.value, startDate, endDate, undefined, PAGE_SIZE, reset ? 0 : memos.value.length)
    } else {
      res = await browseTimeline(startDate, endDate, PAGE_SIZE, reset ? 0 : memos.value.length)
    }
    const result = memoToolResult<{ memos: Memo[]; has_more?: boolean; total?: number }>(res)
    if (sequence !== loadSequence) return
    const newMemos = result?.memos || []
    if (reset) { memos.value = newMemos } else { memos.value = [...memos.value, ...newMemos] }
    hasMore.value = newMemos.length >= PAGE_SIZE
    totalCount.value = result?.total ?? memos.value.length
  } catch (e) {
    console.error('加载小记失败:', e)
    ElMessage.error('加载小记失败')
  } finally {
    if (sequence === loadSequence) { loading.value = false; loadingMore.value = false }
  }
}

async function loadMore() {
  if (loadingMore.value || !hasMore.value) return
  loadingMore.value = true
  await loadMemos(false)
}

// ── Search ──
let searchTimer: ReturnType<typeof setTimeout> | null = null
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => void loadFilteredMemos(), 300)
}
async function loadFilteredMemos() {
  if (locatedMemo.value || route.query.start || route.query.end) {
    const query = {...route.query}; delete query.memo; delete query.start; delete query.end
    await router.replace({query})
  }
  await loadMemos()
}
function clearSearch() { searchQuery.value = ''; void loadFilteredMemos() }
function searchByTag(tag: string) { searchQuery.value = tag; void loadFilteredMemos() }

// ── Filter ──
function applyPreset(preset: TimePreset) {
  if (activePreset.value === preset.label) { clearFilter(); return }
  activePreset.value = preset.label
  customDateRange.value = null
  void loadFilteredMemos()
}
function onCustomDateChange(_val: [Date, Date] | null) {
  activePreset.value = ''
  void loadFilteredMemos()
}
function onMobileDateChange() {
  activePreset.value = ''
  if (mobileStartDate.value && mobileEndDate.value) {
    customDateRange.value = [mobileStartDate.value, mobileEndDate.value]
  } else {
    customDateRange.value = null
  }
  void loadFilteredMemos()
}
function clearFilter() {
  activePreset.value = ''
  customDateRange.value = null
  mobileStartDate.value = null
  mobileEndDate.value = null
  void loadFilteredMemos()
}

// ── Memo lifecycle ──
const editedMemo = ref<Memo | null>(null)
const actionMemo = ref<Memo | null>(null)
const deleteTarget = ref<Memo | null>(null)
const deleting = ref(false)
const storageOpen = ref(false)
const composerOpen = computed({ get: () => showCreateDialog.value, set: (value: boolean) => { if (!value) closeDraft() } })
const actionSheetOpen = computed({ get: () => !!actionMemo.value, set: (value: boolean) => { if (!value) actionMemo.value = null } })
const deleteOpen = computed({ get: () => !!deleteTarget.value, set: (value: boolean) => { if (!value && !deleting.value) deleteTarget.value = null } })

function resetDraft() {
  pendingImages.value.forEach(img => { if (img.file) URL.revokeObjectURL(img.preview) })
  pendingImages.value = []
  newMemo.value = { content: '', images: [], tags: [] }
  tagsInput.value = ''
  editedMemo.value = null
}
async function releaseDraftImages() {
  const paths = pendingImages.value.filter(img => img.file && img.path).map(img => img.path!)
  if (paths.length) {
    try { await discardMemoImages(paths) }
    catch { ElMessage.warning('暂存图片清理将由系统自动重试') }
  }
}
function closeDraft() {
  if (creating.value) return
  void releaseDraftImages()
  resetDraft()
  showCreateDialog.value = false
}
function openCreateDialog() {
  resetDraft()
  const timelineTop = memoScrollRef.value?.getBoundingClientRect().top ?? 0
  wasReviewingHistory = isMobile.value && timelineTop < -180
  showCreateDialog.value = true
}
function openEditDialog(memo: Memo) {
  resetDraft()
  editedMemo.value = memo
  newMemo.value.content = memo.content
  tagsInput.value = memo.tags.join(', ')
  pendingImages.value = memo.images.map(path => ({ preview: memoImageUrl(path, true), path, uploading: false }))
  showCreateDialog.value = true
}
function editFromSheet() { const memo = actionMemo.value; actionMemo.value = null; if (memo) openEditDialog(memo) }
function deleteFromSheet() { deleteTarget.value = actionMemo.value; actionMemo.value = null }
async function confirmDelete() {
  if (!deleteTarget.value || deleting.value) return
  deleting.value = true
  try {
    const memo = deleteTarget.value
    const result = await deleteMemo(memo.id, memo.revision)
    memos.value = memos.value.filter(item => item.id !== memo.id)
    totalCount.value = Math.max(0, totalCount.value - 1)
    if (imageViewer.value.show) closeImageViewer()
    deleteTarget.value = null
    if (result.pending_cleanup) ElMessage.warning('小记已删除；部分图片清理待重试，可在图片存储中查看')
    else ElMessage.success('小记及不再使用的图片已删除')
  } catch (e) { ElMessage.error(e instanceof Error ? e.message : '删除失败，未移除小记') }
  finally { deleting.value = false }
}

async function viewLatestMemo() {
  showPublishedNotice.value = false
  searchQuery.value = ''
  activePreset.value = ''
  customDateRange.value = null
  mobileStartDate.value = null
  mobileEndDate.value = null
  await loadMemos()
  await nextTick()
  document.querySelector('.app-main')?.scrollTo({ top: 0, behavior: 'smooth' })
}

async function submitMemo() {
  if (creating.value || (!newMemo.value.content.trim() && !pendingImages.value.length)) return
  creating.value = true
  try {
    const tags = tagsInput.value ? tagsInput.value.split(/[,，]/).map(t => t.trim()).filter(Boolean) : []
    const toUpload = pendingImages.value.filter(img => img.file && !img.path)
    for (const img of toUpload) {
      // Upload separately: nine 20 MB originals should not exceed one HTTP body's limit.
      // Successful paths stay in the draft if a later upload fails.
      img.uploading = true
      const result = await uploadImages([img.file!])
      if (result.paths.length !== 1) throw new Error('图片上传未完成，请重试')
      img.path = result.paths[0]
      img.uploading = false
    }
    const imagePaths = pendingImages.value.map(img => img.path!)
    if (editedMemo.value) {
      const memo = await updateMemo(editedMemo.value.id, editedMemo.value.revision, newMemo.value.content, imagePaths, tags)
      memos.value = memos.value.map(item => item.id === memo.id ? memo : item)
      ElMessage.success('小记已更新')
    } else {
      const result = memoToolResult<{ id: string; timestamp: string; date: string; revision: number }>(await createMemo(newMemo.value.content, imagePaths, tags))
      const memo: Memo = { ...result, content: newMemo.value.content, images: imagePaths, tags }
      const keepHistoryPosition = wasReviewingHistory || hasActiveFilter.value || !!searchQuery.value
      if (keepHistoryPosition) showPublishedNotice.value = true
      else memos.value = [memo, ...memos.value]
      totalCount.value++
      ElMessage.success('小记已发布')
    }
    resetDraft()
    showCreateDialog.value = false
  } catch (e) { ElMessage.error(e instanceof Error ? e.message : '保存失败，草稿已保留') }
  finally {
    pendingImages.value.forEach(img => { img.uploading = false })
    creating.value = false
  }
}

// ── Formatting ──
function thumbnailUrl(path: string): string { return memoImageUrl(path, true) }
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
  html = html.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

  // Phase 3: inline elements
  html = html.replace(/`([^`\n]+)`/g, '<code class="memo-inline-code">$1</code>')
  html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_match, alt, path) => {
    const src = memoImageSource(path).replace(/&/g, '&amp;').replace(/"/g, '&quot;')
    return `<img class="memo-inline-img" src="${src}" alt="${alt}" loading="lazy" />`
  })
  html = html.replace(/!\[\[([^\]]+)\]\]/g, (_match, path) => `<img class="memo-inline-img" src="${memoImageSource(path)}" alt="小记图片" loading="lazy" />`)
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_match, label, href) => {
    if (!/^(?:https?:\/\/|mailto:|#|\/)/i.test(href)) return label
    return `<a class="memo-link" href="${href}" target="_blank" rel="noopener">${label}</a>`
  })
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

function updateActiveFromScroll() {
  spyRafId = null
  const container = memoScrollRef.value
  if (!container || isMobile.value) return
  const baseTop = container.getBoundingClientRect().top
  const headers: SpyHeader[] = []
  for (const group of groupedMemos.value) {
    const el = document.getElementById('date-' + group.date)
    if (!el) continue
    const top = el.getBoundingClientRect().top - baseTop
    headers.push({ date: group.date, top })
    if (top > SPY_THRESHOLD) break
  }
  const next = pickActiveDate(headers, SPY_THRESHOLD)
  if (next === null) {
    selectedDate.value = ''
    return
  }
  if (next !== selectedDate.value) {
    selectedDate.value = next
    document.querySelector(`.day-link[data-date="${next}"]`)?.scrollIntoView({ block: 'nearest' })
  }
}

function requestSpyUpdate() {
  if (spyRafId !== null) return
  spyRafId = requestAnimationFrame(updateActiveFromScroll)
}

// Re-sync after data changes (load / load-more / filter / create) so the
// highlight never points at a date that left the list.
watch(groupedMemos, () => { nextTick(requestSpyUpdate) })

function onMemoScroll(e: Event) {
  const el = e.target as HTMLElement
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 100) loadMore()
  requestSpyUpdate()
}

function applyHomepageContext() {
  const start = String(route.query.start || ''), end = String(route.query.end || '')
  const valid = (value:string) => { try { parseLocalDate(value); return true } catch { return false } }
  if (valid(start) && valid(end) && start <= end) {
    activePreset.value = ''; customDateRange.value = [new Date(`${start}T12:00:00`),new Date(`${end}T12:00:00`)]
    mobileStartDate.value = customDateRange.value[0]; mobileEndDate.value = customDateRange.value[1]
  }
  void loadMemos()
}
function clearMemoLocation() {
  const query = {...route.query}; delete query.memo
  void router.replace({query})
}
watch(() => [route.query.memo,route.query.start,route.query.end], applyHomepageContext)
onMounted(() => { applyHomepageContext(); if (route.query.storage === '1') storageOpen.value = true })
</script>

<style scoped>
/* ── Page Root ── */
.timeline-page {
  height: calc(100dvh - 64px);
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
  max-width: 100%;
  overflow: hidden;
}
.timeline-page > .page-header,
.timeline-page > .toolbar { flex: none; }

/* ── Glass Surfaces ── */
.glass-surface {
  background: var(--bg-glass);
  backdrop-filter: var(--glass-content-filter);
  -webkit-backdrop-filter: var(--glass-content-filter);
  border: 1px solid var(--border-glass);
  box-shadow:
    var(--shadow-sm),
    var(--shadow-md),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
}
.glass-surface-heavy {
  background: var(--bg-glass-strong);
  backdrop-filter: var(--glass-panel-filter);
  -webkit-backdrop-filter: var(--glass-panel-filter);
  border: 1px solid var(--border-glass);
  box-shadow:
    var(--shadow-lg),
    var(--shadow-sm),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
}
.glass-chip {
  background: var(--bg-glass-subtle);
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  border: 1px solid var(--border-subtle);
}

/* ── Glass Buttons ── */
.glass-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 9px 20px;
  border-radius: 14px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-hover);
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
  box-shadow: var(--shadow-sm);
}
.glass-btn:hover {
  background: var(--bg-glass-strong);
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}
.glass-btn:active {
  transform: translateY(0) scale(0.97);
  box-shadow: var(--shadow-sm);
}
.glass-btn:disabled {
  opacity: 0.5;
  pointer-events: none;
}
.glass-icon-btn {
  width: 32px; height: 32px;
  border-radius: 10px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-glass-subtle);
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized);
}
.glass-icon-btn:hover {
  background: var(--bg-glass);
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
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  background: var(--bg-glass-subtle);
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  padding: 14px 16px;
  font-size: 14px;
  font-family: inherit;
  color: var(--text-primary);
  line-height: 1.6;
  resize: vertical;
  outline: none;
  transition: border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized);
  box-shadow: inset var(--shadow-sm);
}
.glass-textarea::placeholder { color: var(--text-faint); }
.glass-textarea:focus {
  border-color: rgba(129, 140, 248, 0.4);
  box-shadow: inset var(--shadow-sm);
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
@media (min-width: 769px) and (max-width: 1100px) {
  .toolbar-row { flex-wrap: wrap; }
  .toolbar-row .search-box { max-width: none; }
  .filter-right { margin-left: 0; flex: 1 1 100%; flex-wrap: wrap; min-width: 0; }
  .date-range-picker { flex: 0 1 340px; min-width: 0; }
  .date-range-picker :deep(.el-date-editor) { width: 100%; min-width: 0; }
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
  transition: background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}
.search-box:focus-within {
  background: var(--bg-glass-strong);
  box-shadow:
    0 4px 16px rgba(0, 0, 0, 0.06),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
}
.search-icon {
  color: var(--text-faint);
  flex-shrink: 0;
  transition: color var(--duration-fast) var(--ease-out);
}
.search-box:focus-within .search-icon {
  color: var(--accent);
}
.clear-btn {
  width: 22px; height: 22px;
  border-radius: 50%;
  border: none;
  background: var(--border-faint);
  color: var(--text-muted);
  font-size: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: var(--transition-interactive);
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
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
  position: relative;
  user-select: none;
}
.chip:hover {
  color: var(--text-secondary);
  background: var(--bg-glass-subtle);
}
.chip.active {
  color: var(--text-primary);
  background: var(--glass-action-sheen), var(--glass-action-fill);
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  box-shadow: var(--glass-action-shadow);
}

/* ── Date Picker (scoped) ── */
.date-range-picker {
  display: flex;
  align-items: center;
  gap: 6px;
}
.mobile-date-sep {
  color: var(--text-faint);
  font-size: 13px;
  flex-shrink: 0;
}
.mobile-date-input {
  flex: 1;
  min-width: 0;
}
.date-range-picker :deep(.el-range-editor) {
  border-radius: 14px !important;
  border: 1px solid var(--border-subtle) !important;
  background: var(--bg-glass-subtle) !important;
  backdrop-filter: var(--glass-control-filter);
  -webkit-backdrop-filter: var(--glass-control-filter);
  box-shadow: var(--shadow-sm), var(--inset-highlight) !important;
  height: 40px !important;
  padding: 0 12px !important;
  transition: background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized) !important;
}
.date-range-picker :deep(.el-range-editor:hover) {
  border-color: var(--border-glass) !important;
  background: var(--bg-glass) !important;
}
.date-range-picker :deep(.el-range-editor.is-active) {
  border-color: rgba(129, 140, 248, 0.4) !important;
  box-shadow: var(--shadow-sm), var(--inset-highlight) !important;
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
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 20px;
  overflow: hidden;
}

/* ── Time Nav ── */
.time-nav {
  width: 160px;
  height: 100%;
  flex-shrink: 0;
  overflow-y: auto;
  max-height: none;
  border-radius: 20px;
  padding: 16px 8px 16px 4px;
  scrollbar-width: thin;
  scrollbar-color: var(--border-glass) transparent;
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
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized);
  position: relative;
}
.day-link:hover {
  background: var(--bg-glass-subtle);
  color: var(--text-secondary);
}
.day-link.active {
  background: rgba(124, 124, 255, 0.15);
  color: var(--accent);
}
.day-dot {
  width: 8px; height: 8px;
  border-radius: 50%;
  border: 2px solid #d4d4d8;
  background: var(--bg-glass);
  flex-shrink: 0;
  z-index: 1;
  transition: transform var(--motion-normal) var(--ease-spring-gentle),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
}
.day-link.active .day-dot {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: 0 0 8px rgba(129, 140, 248, 0.5);
  transform: scale(1.2);
}
.day-line {
  position: absolute;
  left: 13px;
  top: -3px;
  bottom: -3px;
  width: 2px;
  background: var(--border-faint);
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
  overscroll-behavior-y: contain;
  -webkit-overflow-scrolling: touch;
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
  color: var(--accent);
}
.filter-hint button {
  border: none;
  background: rgba(99, 102, 241, 0.1);
  color: var(--accent);
  font-size: 12px;
  font-weight: 500;
  padding: 3px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: var(--transition-interactive);
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
  border-bottom: 1px solid var(--border-faint);
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
  letter-spacing: var(--tracking-tight);
}
.day-header-weekday {
  font-size: 12px;
  color: var(--text-faint);
}
.day-header-count {
  font-size: 11px;
  padding: 2px 10px;
  border-radius: 10px;
  color: var(--accent);
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
  transition: transform var(--motion-normal) var(--ease-spring-gentle),
              background-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
}
.memo-card:hover .memo-time-dot {
  background: var(--accent);
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
  transition: transform var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized);
  transform: translateZ(0);
  -webkit-backface-visibility: hidden;
  backface-visibility: hidden;
  min-width: 0;
  overflow: hidden;
}
.memo-card-body:hover {
  transform: translateY(-1px);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.06),
    0 2px 6px rgba(0, 0, 0, 0.03),
    inset 0 1px 0 rgba(255, 255, 255, 0.03);
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
  overflow-wrap: anywhere;
  min-width: 0;
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
  font-family: var(--font-mono);
  overflow-x: auto;
  width: 100%;
  box-sizing: border-box;
  margin: 10px 0;
  line-height: 1.6;
  border: 1px solid rgba(0, 0, 0, 0.1);
  -webkit-overflow-scrolling: touch;
  touch-action: pan-x pan-y;
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
.memo-content :deep(blockquote) {
  border-left: 3px solid var(--accent-border);
  padding: 4px 12px;
  margin: 8px 0;
  color: var(--text-tertiary);
  overflow-wrap: break-word;
  word-break: break-word;
}
.memo-content :deep(.memo-inline-code) {
  background: rgba(24, 24, 27, 0.06);
  padding: 2px 7px;
  border-radius: 5px;
  font-size: 0.9em;
  font-family: var(--font-mono);
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
  background: var(--accent);
  border-color: var(--accent);
}
.memo-content :deep(.memo-check-checked::after) {
  content: '✓';
  position: absolute;
  top: -2px;
  left: 1px;
  color: var(--accent-contrast);
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
  color: var(--accent);
  text-decoration: none;
  border-bottom: 1px solid rgba(99, 102, 241, 0.3);
  transition: border-color var(--duration-fast) var(--ease-out);
}
.memo-content :deep(.memo-link:hover) {
  border-bottom-color: var(--accent);
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
  overflow: hidden;
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
  transition: transform var(--duration-normal) var(--ease-standard);
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
  color: var(--accent);
  padding: 3px 12px;
  border-radius: 10px;
  cursor: pointer;
  transition: var(--transition-interactive);
  font-weight: 500;
}
.memo-tag:hover {
  background: rgba(99, 102, 241, 0.12);
  color: var(--accent);
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
  background: var(--border-faint);
}

/* ── Empty & Loading ── */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 80px 20px;
  animation: fade-in 0.5s var(--ease-out);
}
.empty-icon {
  font-size: 56px;
  margin-bottom: 20px;
  transform: translateY(0);
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
.dialog-content {
  width: 540px;
  max-width: 90vw;
  border-radius: 24px;
  padding: 28px;
  max-height: calc(100dvh - 48px);
  overflow: auto;
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
.image-btn { gap: 4px; height: 42px; padding: 0 14px; font-size: 13px; }

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
  border: 1px solid var(--border-subtle);
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
  transition: background var(--duration-fast) var(--ease-out);
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
  background: var(--bg-glass-subtle);
  color: var(--text-faint);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: var(--transition-interactive);
}
.image-add-btn:hover {
  border-color: rgba(0, 0, 0, 0.2);
  background: var(--bg-hover);
  color: var(--text-muted);
}

/* ── Transitions ── */
.hint-enter-active { transition: opacity var(--motion-normal) var(--ease-emphasized), transform var(--motion-normal) var(--ease-spring-gentle); }
.hint-leave-active { transition: opacity var(--motion-fast) var(--ease-emphasized), transform var(--motion-fast) var(--ease-emphasized); }
.hint-enter-from, .hint-leave-to { opacity: 0; transform: translateY(-8px); }

/* ── Memo Animations ── */
.memo-card {
  animation: fade-in var(--motion-normal) var(--ease-emphasized) both;
  animation-delay: var(--delay, 0s);
  transition: opacity var(--motion-normal) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}

/* TransitionGroup animations for add/remove */
.memo-anim-enter-active {
  transition: opacity var(--duration-slow) var(--ease-standard),
              transform var(--duration-slow) var(--ease-spring);
}
.memo-anim-leave-active {
  transition: opacity var(--duration-normal) var(--ease-out), transform var(--duration-normal) var(--ease-out);
}
.memo-anim-enter-from {
  opacity: 0;
  transform: translateY(12px) scale(0.98);
}
.memo-anim-leave-to {
  opacity: 0;
  transform: translateY(-8px) scale(0.985);
}
.memo-anim-move {
  transition: transform 0.4s var(--ease-standard);
}

/* ── Keyframes ── */
@keyframes slideDown {
  from { opacity: 0; transform: translateY(-16px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes slideRight {
  from { opacity: 0; transform: translateX(-16px); }
  to { opacity: 1; transform: translateX(0); }
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
  backdrop-filter: var(--glass-scrim-filter);
  -webkit-backdrop-filter: var(--glass-scrim-filter);
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
  transition: var(--transition-interactive);
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
  transition: var(--transition-interactive);
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
  cursor: grab;
  touch-action: pan-x pan-y;
}
.viewer-image:active { cursor: grabbing; }
.viewer-image-pending { display: none; }
.viewer-status { max-width: min(70vw, 420px); color: #fff; text-align: center; line-height: 1.6; }
.viewer-controls {
  position: absolute;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 8px;
  z-index: 10;
}
.viewer-zoom-btn {
  width: 40px; height: 40px; border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.2);
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: var(--glass-floating-filter);
  -webkit-backdrop-filter: var(--glass-floating-filter);
  color: rgba(255, 255, 255, 0.8);
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  transition: var(--transition-interactive);
}
.viewer-zoom-btn:hover { background: rgba(255, 255, 255, 0.2); color: #fff; }
.viewer-zoom-btn:active { transform: scale(0.92); }
.viewer-counter {
  color: rgba(255, 255, 255, 0.7);
  font-size: 14px;
  font-weight: 500;
  padding: 4px 14px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: var(--glass-floating-filter);
  -webkit-backdrop-filter: var(--glass-floating-filter);
}

/* Viewer transitions */
.viewer-enter-active { transition: opacity var(--duration-normal) var(--ease-out); }
.viewer-enter-active .viewer-image { transition: transform var(--duration-slow) var(--ease-spring), opacity var(--duration-normal) var(--ease-out); }
.viewer-leave-active { transition: opacity var(--duration-fast) var(--ease-out); }
.viewer-enter-from { opacity: 0; }
.viewer-enter-from .viewer-image { transform: scale(0.9); opacity: 0; }
.viewer-leave-to { opacity: 0; }

/* Mobile-only controls stay out of the desktop hierarchy. */
.mobile-filter-summary,
.mobile-compose-action,
.mobile-filter-heading,
.published-notice { display: none; }
.mobile-sync-action {
  min-height: 36px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  border: 0;
  border-radius: 10px;
  background: var(--accent-light);
  color: var(--accent);
  font: inherit;
  font-size: 13px;
  font-weight: 600;
}
.mobile-sync-action:disabled { opacity: 0.55; }
.filter-panel-enter-active,
.filter-panel-leave-active {
  transition: opacity var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}
.filter-panel-enter-from,
.filter-panel-leave-to { opacity: 0; transform: translateY(-8px); }
.published-notice-enter-active,
.published-notice-leave-active {
  transition: opacity var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}
.published-notice-enter-from,
.published-notice-leave-to { opacity: 0; transform: translate(-50%, -10px) scale(0.98); }

/* ── Responsive ── */
@media (max-width: 768px) {
  .glass-btn { min-height: var(--tap-target); }
  .time-nav { display: none; }
  .timeline-page {
    width: 100%;
    height: auto;
    min-height: 100%;
    display: block;
    overflow-x: hidden;
    overflow-y: visible;
  }
  .timeline-page > .page-header,
  .timeline-page > .toolbar { flex: initial; }
  .main-content { flex: none; min-height: 0; overflow-x: hidden; overflow-y: visible; }
  .memo-scroll { min-width: 0; overflow-x: hidden; overflow-y: visible; overscroll-behavior-y: auto; -webkit-overflow-scrolling: auto; }
  .toolbar-row {
    position: sticky;
    top: calc(var(--mobile-header-height) + var(--safe-top));
    z-index: 20;
    flex-wrap: wrap;
    padding: 8px;
    border-radius: 16px;
    background: var(--bg-glass-strong);
    backdrop-filter: var(--glass-floating-filter);
    -webkit-backdrop-filter: var(--glass-floating-filter);
    border: 1px solid var(--border-glass);
  }
  .search-box { max-width: 100%; min-width: 0; }
  .filter-right { flex-wrap: wrap; margin-left: 0; width: 100%; }
  .memo-card-main.has-images { flex-direction: column; }
  .memo-images-wrap { width: 100%; max-width: 280px; height: auto; aspect-ratio: 1; }
  /* Single image: no forced square — container fits the image naturally. */
  .memo-images-wrap:has(.memo-images-1) {
    aspect-ratio: auto;
    height: auto;
  }
  .memo-images-1 .memo-image { max-height: 280px; width: 100%; height: auto; object-fit: cover; }
  .memo-card-body { padding: 14px 16px; border-radius: 14px; }
  .dialog-content {
    width: 100%;
    max-width: none;
    max-height: min(88dvh, 760px);
    border-radius: 24px 24px 0 0;
    margin: 0;
    padding: 34px 20px calc(24px + var(--safe-bottom));
  }
  .preset-chips { overflow-x: auto; -webkit-overflow-scrolling: touch; }
  .chip-track { flex-wrap: nowrap; }
  .chip { flex-shrink: 0; padding: 6px 12px; font-size: 12px; }

  .image-preview-grid { grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .dialog-btns { width: 100%; }
  .dialog-btns .glass-btn { flex: 1; min-height: var(--tap-target); }
  .dialog-footer { flex-wrap: wrap; gap: 10px; }
  .image-remove-btn { width: 32px; height: 32px; }
  .viewer-close {
    top: calc(var(--safe-top) + 10px);
    right: max(10px, var(--safe-right));
    width: var(--tap-target);
    height: var(--tap-target);
  }
  .viewer-controls { bottom: calc(var(--safe-bottom) + 12px); }
  .viewer-zoom-btn { width: var(--tap-target); height: var(--tap-target); }
  .viewer-prev { left: max(8px, var(--safe-left)); }
  .viewer-next { right: max(8px, var(--safe-right)); }
  .viewer-image-wrap, .viewer-image { max-width: calc(100vw - 16px); max-height: calc(100dvh - 120px - var(--safe-top) - var(--safe-bottom)); }

  /* Date picker responsive */
  .date-range-picker {
    width: 100%;
  }
  .date-range-picker :deep(.el-range-editor) {
    width: 100% !important;
  }

  /* One compact action row: search remains primary, range expands in place. */
  .timeline-page > .page-header { display: none; }
  .desktop-sync-action { display: none; }
  .toolbar { margin-bottom: 14px; padding-top: 0; }
  .toolbar-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--tap-target) var(--tap-target);
    align-items: start;
    gap: 10px;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
  .search-box { height: var(--tap-target); }
  .glass-input { font-size: 16px; }
  .clear-btn { width: 30px; height: 30px; }
  .mobile-compose-action {
    grid-column: 2;
    grid-row: 1;
    width: var(--tap-target);
    height: var(--tap-target);
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: 13px;
    font-size: 20px;
  }
  .mobile-compose-action:active { transform: scale(.94); }
  .mobile-filter-summary {
    grid-column: 3;
    grid-row: 1;
    width: var(--tap-target);
    height: var(--tap-target);
    display: grid;
    place-items: center;
    padding: 0;
    border-radius: 13px;
    color: var(--text-secondary);
    font: inherit;
    font-size: 22px;
  }
  .mobile-filter-summary.active { color: var(--accent); border-color: var(--accent-border); background: var(--accent-light); }
  .mobile-filter-summary:active { transform: scale(.94); }
  .filter-right {
    grid-column: 1 / -1;
    grid-row: 2;
    width: 100%;
    margin: 0;
    padding: 12px;
    align-items: stretch;
    border: 1px solid var(--border-glass);
    border-radius: 16px;
    background: var(--bg-glass-strong);
  }
  .mobile-filter-heading {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    color: var(--text-primary);
    font-size: 14px;
    font-weight: 650;
  }
  .preset-chips { width: 100%; }
  .chip { min-height: 40px; display: inline-flex; align-items: center; }
  .date-range-picker :deep(.el-date-editor) { min-height: var(--tap-target); }

  /* Reading density follows the app's body type instead of shrinking on phones. */
  .memo-card { gap: 10px; }
  .memo-card-left { width: 6px; }
  .memo-card-body {
    position: relative;
    padding: 18px 16px 16px;
    border-radius: 18px;
    background: var(--bg-glass-strong);
  }
  .memo-card-body:hover { transform: none; box-shadow: var(--shadow-sm), var(--shadow-md); }
  .memo-time { position: absolute; top: 14px; left: 16px; right: auto; margin: 0; font-size: 13px; }
  .memo-card-main { padding-top: 18px; }
  .memo-content { font-size: 16px; line-height: 1.75; }
  .memo-content :deep(.memo-code) { padding: 12px 14px; font-size: 13px; }
  .memo-content :deep(ul), .memo-content :deep(ol) { padding-left: 20px; }
  .memo-tags { margin-top: 12px; }
  .memo-tag { min-height: 32px; display: inline-flex; align-items: center; font-size: 13px; }
  .day-group-header { border-bottom: 0; padding: 0 2px; margin-bottom: 8px; }
  .day-header-date { font-size: 17px; }
  .day-header-weekday { font-size: 13px; }

  /* The editor owns one scroll region and keeps publishing above the keyboard. */
  .dialog-content {
    height: min(88dvh, 760px);
    max-height: min(88dvh, 760px);
    display: flex;
    flex-direction: column;
    padding: 34px 16px calc(12px + var(--safe-bottom));
    overflow: hidden;
  }
  .dialog-header { flex: none; margin-bottom: 14px; }
  .dialog-header h3 { font-size: 19px; }
  .dialog-header .glass-icon-btn { width: var(--tap-target); height: var(--tap-target); }
  .create-form {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 2px 1px 12px;
  }
  .glass-textarea { min-height: 150px; max-height: 42dvh; font-size: 16px; resize: none; }
  .tag-input-wrap, .image-btn { min-height: var(--tap-target); height: var(--tap-target); }
  .dialog-footer { flex: none; margin-top: 10px; }
  .image-remove-btn { width: 34px; height: 34px; }

  .published-notice {
    position: fixed;
    top: calc(var(--mobile-header-height, 52px) + var(--safe-top, 0px) + 10px);
    left: 50%;
    z-index: 80;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    min-height: var(--tap-target);
    padding: 0 16px;
    border: 1px solid var(--border-glass);
    border-radius: 999px;
    background: var(--bg-glass-strong);
    color: var(--text-secondary);
    box-shadow: var(--shadow-lg);
    backdrop-filter: var(--glass-floating-filter);
    -webkit-backdrop-filter: var(--glass-floating-filter);
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .published-notice strong { color: var(--accent); font-weight: 650; }
}

@media (max-width: 360px) {
  .toolbar-row { grid-template-columns: minmax(0, 1fr) var(--tap-target) var(--tap-target); gap: 8px; }
  .memo-card-left { display: none; }
  .memo-card { display: block; }
}

@media (hover: none) and (pointer: coarse) {
  .memo-card-body:hover,
  .glass-btn:hover { transform: none; }
  .chip,
  .memo-image,
  .memo-tag,
  .mobile-filter-summary { -webkit-tap-highlight-color: transparent; }
}

.memo-actions { position: absolute; top: 8px; right: 10px; display: flex; gap: 4px; }
.memo-action-btn { width: 36px; height: 36px; display: grid; place-items: center; border: 0; border-radius: 12px; background: transparent; color: var(--text-muted); cursor: pointer; }
.memo-action-btn:hover { background: var(--bg-glass); color: var(--text-primary); }
.memo-action-btn:active { transform: scale(.96); }
.memo-action-btn .el-icon { font-size: 18px; }
.danger { color: var(--glass-danger-label); }
.memo-delete-hint { color: var(--text-secondary); font-size: 14px; line-height: 1.7; }
.memo-delete-preview { color: var(--text-muted); font-size: 13px; white-space: pre-wrap; overflow-wrap: anywhere; }
.memo-action-sheet { display: flex; flex-direction: column; gap: 12px; }
.memo-action-sheet > .glass-btn { justify-content: flex-start; min-height: 48px; }
.memo-card-body { padding-top: 50px; }
.memo-time { position: absolute; top: 20px; left: 20px; right: auto; margin: 0; }
.memo-card-main { padding-top: 0; }
.compact-dialog { height: auto; }
@media (max-width: 768px) {
  .compact-dialog { height: auto; overflow-y: auto; }
  .memo-actions { top: 4px; right: 8px; }
  .memo-action-btn { width: 44px; height: 44px; }
  .memo-card-body { padding-top: 54px; }
  .memo-time { top: 19px; left: 16px; }
  .memo-action-btn .el-icon { font-size: 20px; }
}
</style>

<style>
/* ── Calendar Popup Responsive ── */
@media (max-width: 768px) {
  .glass-picker {
    max-width: calc(100vw - 16px) !important;
    left: 8px !important;
    right: 8px !important;
    width: auto !important;
    border-radius: 16px !important;
  }
  /* Stack the two calendar panels vertically on mobile */
  .glass-picker .el-date-range-picker {
    flex-direction: column !important;
    width: 100% !important;
  }
  .glass-picker .el-date-range-picker__content {
    width: 100% !important;
    padding: 4px !important;
    border-right: none !important;
  }
  .glass-picker .el-date-range-picker__content:last-child {
    border-bottom: none !important;
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
