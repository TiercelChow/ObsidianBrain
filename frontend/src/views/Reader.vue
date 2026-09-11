<template>
  <div
    ref="readerPageRef"
    class="reader-page"
    :class="{
      'is-fullscreen': isFullscreen,
      'is-fs-transitioning': isFsTransitioning,
      'fs-ui-hidden': isFullscreen && !showFsUI,
      'is-mobile-immersive': isMobileImmersive,
      'is-read-view': viewMode === 'read',
      'has-document': Boolean(displayedFile),
    }"
  >
    <header class="page-header">
      <div>
        <h1 class="page-title">阅境轩</h1>
        <p class="page-subtitle">浏览本地 Markdown 与 PDF，沉浸阅读</p>
      </div>
    </header>

    <!-- Compact trigger bar — always visible; the view switch leads it like
         the Tasks toolbar, reading controls join in read view. -->
    <div class="reader-topbar glass-surface">
      <div class="view-switch" aria-label="视图切换">
        <span class="switch-indicator" :class="{ read: viewMode === 'read' }"></span>
        <button type="button" :class="{ active: viewMode === 'shelf' }" @click="changeView('shelf')">书架</button>
        <button type="button" :class="{ active: viewMode === 'read' }" @click="changeView('read')">阅读</button>
      </div>
      <div v-show="viewMode === 'shelf'" class="shelf-search glass-surface" role="search">
        <el-icon class="shelf-search-icon"><Search /></el-icon>
        <input v-model="shelfQuery" type="search" aria-label="搜索书籍" placeholder="搜索书名、描述或类别" />
        <button v-if="shelfQuery" type="button" class="shelf-search-clear" aria-label="清除搜索" @click="shelfQuery = ''">×</button>
      </div>
      <button v-show="viewMode === 'shelf'" type="button" class="reader-shelf-add" @click="bookshelfRef?.openAdd()">
        <el-icon><Plus /></el-icon><span>添加</span>
      </button>
      <button
        v-show="viewMode === 'read'"
        class="path-trigger reader-file-search-trigger"
        :aria-label="rootPath ? '搜索当前目录文件' : '返回书架选择书籍'"
        @click="openFileSearch"
      >
        <el-icon><Search /></el-icon>
        <span v-if="rootPath" class="pt-path">搜索当前目录中的 Markdown 与 PDF</span>
        <span v-else class="pt-hint">请先从书架选择书籍</span>
      </button>
      <el-button v-show="viewMode === 'read'" class="icon-btn reader-fullscreen-btn" :title="isImmersive ? '退出沉浸阅读' : '沉浸阅读'" @click="toggleFullscreen">
        <el-icon><FullScreen /></el-icon>
      </el-button>
    </div>

    <!-- Bookshelf view (kept alive via v-show alongside the reading panes) -->
    <BookshelfView ref="bookshelfRef" v-show="viewMode === 'shelf'" class="bookshelf-root" :query="shelfQuery" @open="openBook" />

    <!-- Current-book file search (command-palette style) -->
    <transition name="overlay-fade">
      <div v-if="fileSearchOpen" class="path-overlay" @click.self="fileSearchOpen = false">
        <transition name="overlay-pop" appear>
          <div
            v-if="fileSearchOpen"
            ref="fileSearchCardRef"
            class="path-card is-file-search"
            role="dialog"
            aria-modal="true"
            aria-label="搜索当前目录文件"
            tabindex="-1"
          >
            <div class="reader-search-modal">
              <div class="reader-search-header">
                <div>
                  <div class="pane-title">搜索文件</div>
                  <small :title="rootPath">{{ rootPath }}</small>
                </div>
                <button type="button" class="reader-search-close" aria-label="关闭文件搜索" @click="fileSearchOpen = false">×</button>
              </div>
              <label class="reader-file-search glass-surface">
                <el-icon><Search /></el-icon>
                <input
                  ref="fileSearchInputRef"
                  v-model="fileSearchQuery"
                  type="search"
                  inputmode="search"
                  autocomplete="off"
                  aria-label="搜索当前目录文件"
                  placeholder="输入文件名或路径"
                  @keydown.escape="fileSearchOpen = false"
                />
                <button v-if="fileSearchQuery" type="button" aria-label="清除文件搜索" @click="fileSearchQuery = ''">×</button>
              </label>
              <p v-if="fileSearchQuery" class="reader-search-count">找到 {{ fileSearchResults.length }} 个可阅读文件</p>
              <ul v-if="fileSearchResults.length" class="reader-search-results">
                <li v-for="file in fileSearchResults" :key="file.path">
                  <button
                    type="button"
                    class="reader-search-result"
                    :class="{ active: file.path === displayedFile }"
                    @click="selectSearchResult(file.path)"
                  >
                    <span class="reader-search-kind">{{ file.kind === 'pdf' ? 'PDF' : 'MD' }}</span>
                    <span class="reader-search-copy">
                      <strong>{{ file.name }}</strong>
                      <small>{{ file.relativePath }}</small>
                    </span>
                  </button>
                </li>
              </ul>
              <div v-else class="reader-search-empty">
                <el-icon><Search /></el-icon>
                <span>{{ fileSearchQuery ? '没有匹配的 Markdown 或 PDF' : '搜索当前目录中的 Markdown 与 PDF' }}</span>
              </div>
            </div>
          </div>
        </transition>
      </div>
    </transition>

    <!-- Body: 3 panes -->
    <div v-show="viewMode === 'read'" ref="readerBodyRef" class="reader-body">
      <!-- Left: file tree -->
      <aside class="pane pane-left">
        <div class="pane-title-bar">
          <span class="pane-title-text">文件</span>
          <button v-if="tree.length" class="pane-title-btn" title="刷新目录" @click="refreshTree">
            <el-icon :size="14"><Refresh /></el-icon>
          </button>
        </div>
        <div class="pane-scroll">
          <FileTree v-if="tree.length" :entries="tree" :active-path="activeFile" @select="onSelectFile" @refresh="refreshTree" />
          <div v-else class="pane-hint">请先从书架选择书籍</div>
        </div>
      </aside>

      <!-- Center: rendered content (page-turn transition on file switch) -->
      <main
        ref="contentRef"
        class="pane pane-center"
        @scroll="onContentScroll"
        @touchmove.passive="revealMobileToolbar"
      >
        <transition
          :name="transitionDir"
          @enter="onArticleEnter"
          @after-leave="onContentAfterLeave"
        >
          <PdfViewer
            v-if="fileKind === 'pdf' && renderedHtml === ''"
            ref="pdfViewerRef"
            :key="displayedFile"
            :src="displayedFile"
            @outline="onPdfOutline"
            @pagechange="onPdfPageChange"
            @pagecount="onPdfPageCount"
          />
          <article
            v-else-if="renderedHtml"
            :key="displayedFile"
            class="markdown-body"
            v-html="renderedHtml"
          ></article>
          <div v-else key="empty" class="center-state">
            <el-icon class="cs-icon"><Document /></el-icon>
            <p>选择文件开始阅读</p>
          </div>
        </transition>

        <!-- top loading bar; old content stays visible underneath while loading -->
        <div v-if="fileLoading" class="loadbar"><span></span></div>
        <!-- error banner (old content remains behind) -->
        <div v-if="error" class="error-banner">⚠️ {{ error }}</div>
      </main>

      <!-- Right: TOC -->
      <aside class="pane pane-right">
        <div class="pane-title">目录</div>
        <div class="pane-scroll">
          <div v-if="!toc.length" class="pane-hint">无目录</div>
          <a
            v-for="t in toc"
            :key="t.id"
            class="toc-item"
            :class="{ active: activeHeading === t.id }"
            :style="{ paddingLeft: 8 + (t.level - 1) * 12 + 'px' }"
            :title="t.text"
            @click="onTocClick(t)"
          ><span v-if="t.html" class="toc-label" v-html="t.html"></span><span v-else>{{ t.text }}</span></a>
        </div>
      </aside>
    </div>

    <div
      v-if="viewMode === 'read' && mobileToolbarState.rendered"
      class="reader-mobile-toolbar"
      :class="{
        'is-visible': mobileToolbarState.visible,
        'is-pinned': mobileToolbarState.pinned,
      }"
      :aria-hidden="!mobileToolbarState.visible"
      :inert="!mobileToolbarState.visible ? true : undefined"
      aria-label="阅读工具"
      @pointerdown="revealMobileToolbar"
    >
      <div class="mobile-toolbar-side mobile-toolbar-left">
        <button
          type="button"
          :aria-label="!rootPath ? '返回书架选择书籍' : mobileToolbarState.pinned ? '选择文章' : '打开文件列表'"
          :title="!rootPath ? '返回书架' : mobileToolbarState.pinned ? '选择文章' : '文件'"
          @click="openMobileFilePicker"
        >
          <el-icon><FolderOpened /></el-icon>
        </button>
        <button type="button" aria-label="切换沉浸阅读" :title="isImmersive ? '退出沉浸阅读' : '沉浸阅读'" @click="toggleFullscreen">
          <el-icon><FullScreen /></el-icon>
        </button>
      </div>
      <button
        v-if="fileKind === 'pdf' && displayedFile"
        type="button"
        class="reader-document-center"
        :aria-label="`${mobileDocumentLabel}，点按恢复适宽`"
        @click="fitPdf"
      >
        <span class="reader-document-label">{{ mobileDocumentLabel }}</span>
        <small>{{ pdfCurrentPage || 1 }} / {{ pdfPageCount || '—' }}</small>
      </button>
      <div v-else class="reader-document-center" :aria-label="mobileDocumentLabel">
        <span class="reader-document-label">{{ mobileDocumentLabel }}</span>
      </div>
      <div class="mobile-toolbar-side mobile-toolbar-right">
        <template v-if="fileKind === 'pdf' && displayedFile">
          <button type="button" aria-label="缩小 PDF" title="缩小" @click="setPdfZoom(-1)">
            <el-icon><Minus /></el-icon>
          </button>
          <button type="button" aria-label="放大 PDF" title="放大" @click="setPdfZoom(1)">
            <el-icon><Plus /></el-icon>
          </button>
        </template>
        <template v-else>
          <button type="button" aria-label="打开文章目录" title="目录" @click="tocDrawer = true">
            <el-icon><Menu /></el-icon>
          </button>
          <button type="button" aria-label="搜索当前目录文件" title="搜索文件" @click="openFileSearch">
            <el-icon><Search /></el-icon>
          </button>
        </template>
      </div>
    </div>

    <!-- Mobile drawers: direct-manipulation panels that can be interrupted mid-swipe. -->
    <MotionDrawer v-model="treeDrawer" direction="left" aria-label="文件列表">
      <div class="drawer-inner">
        <div class="pane-title">文件</div>
        <FileTree
          v-if="tree.length"
          ref="drawerTreeRef"
          :entries="tree"
          :active-path="activeFile"
          @select="(p) => { onSelectFile(p); treeDrawer = false }"
        />
        <div v-else class="pane-hint">请先从书架选择书籍</div>
      </div>
    </MotionDrawer>
    <MotionDrawer v-model="tocDrawer" direction="right" aria-label="文章目录">
      <div class="drawer-inner">
        <div class="pane-title">目录</div>
        <a
          v-for="t in toc"
          :key="t.id"
          class="toc-item"
          :class="{ active: activeHeading === t.id }"
          :style="{ paddingLeft: 8 + (t.level - 1) * 12 + 'px' }"
          :title="t.text"
          @click="onTocClick(t); tocDrawer = false"
        ><span v-if="t.html" class="toc-label" v-html="t.html"></span><span v-else>{{ t.text }}</span></a>
        <div v-if="!toc.length" class="pane-hint">无目录</div>
      </div>
    </MotionDrawer>
    <!-- Floating fullscreen UI (auto-hides) -->
    <div v-if="isImmersive" class="fs-ui" :class="{ hidden: isFullscreen && !showFsUI }">
      <button class="fs-fab" :title="isFullscreen ? '退出全屏 (Esc)' : '退出沉浸阅读'" @click="toggleFullscreen">
        <el-icon :size="20"><FullScreen /></el-icon>
      </button>
    </div>

    <!-- In-page toast (works inside fullscreen) -->
    <transition name="toast-slide">
      <div v-if="refreshFlash !== 'none'" class="reader-toast" :class="refreshFlash">
        {{ refreshFlash === 'success' ? '目录已刷新' : '刷新失败' }}
      </div>
    </transition>

    <!-- Mermaid fullscreen viewer -->
    <MermaidViewer v-if="viewerSvg" :svg-html="viewerSvg" :source="viewerSource" :title="viewerTitle" @close="viewerSvg = ''" />

    <!-- Path preview popup (folders / code / out-of-folder md) -->
    <PathPreviewModal
      v-if="previewPath"
      :path="previewPath"
      :anchor="previewAnchor"
      :root="rootPath"
      @close="previewPath = ''"
      @open-in-reader="onPreviewOpenInReader"
    />
  </div>
</template>

<script setup lang="ts">
import {
  computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch,
} from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  FolderOpened, Menu, Document, FullScreen, Refresh, Minus, Plus, Search,
} from '@element-plus/icons-vue'
import {
  listLocalDir, readLocalFile,
  type DirEntry, type ReaderBook,
} from '@/api/reader'
import { makeReaderImageResolvers } from '@/utils/readerImages'
import { resolveRelativePath } from '@/utils/markdownImages'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { normalizeHeadingAnchor } from '@/markdown/headingAnchors'
import { useBookshelf } from '@/composables/useBookshelf'
import { clampPdfPage } from '@/utils/readerBooks'
import {
  captureMdFileProgress,
  capturePdfFileProgress,
  deriveFileKind,
} from '@/utils/readerProgress'
import { useAppStore } from '@/stores/app'
import FileTree from '@/components/reader/FileTree.vue'
import MotionDrawer from '@/components/motion/MotionDrawer.vue'
import BookshelfView from '@/components/reader/BookshelfView.vue'
import { getMobileReaderToolbarState, isPhoneViewport } from '@/utils/mobileLayoutPolicy'
import { useModalEnvironment } from '@/composables/useModalEnvironment'
import { searchReaderFiles } from '@/utils/readerFileSearch'

// Heavy, optional readers stay out of the Markdown-first route chunk.
const MermaidViewer = defineAsyncComponent(() => import('@/components/reader/MermaidViewer.vue'))
const PathPreviewModal = defineAsyncComponent(() => import('@/components/reader/PathPreviewModal.vue'))
const PdfViewer = defineAsyncComponent(() => import('@/components/reader/PdfViewer.vue'))

const appStore = useAppStore()

interface TocItem { id: string; text: string; level: number; page?: number; html?: string }

// Per-browser "last session" (folder + file to reopen).
const LAST_FOLDER_KEY = 'reader.lastFolder'
const LAST_FILE_KEY = 'reader.lastFile'

const tree = ref<DirEntry[]>([])
const rootPath = ref('')          // the currently opened folder (absolute)
const activeFile = ref('')
const fileLoading = ref(false)
const renderedHtml = ref('')
const error = ref('')
const toc = ref<TocItem[]>([])
const activeHeading = ref('')
const fileSearchOpen = ref(false)
const fileSearchCardRef = ref<HTMLElement | null>(null)
const fileSearchQuery = ref('')
const fileSearchInputRef = ref<HTMLInputElement | null>(null)
const fileSearchResults = computed(() => searchReaderFiles(tree.value, rootPath.value, fileSearchQuery.value))

// Auto-focus the input when the overlay opens.
watch(fileSearchOpen, (v) => {
  if (!v) return
  nextTick(() => fileSearchInputRef.value?.focus())
})
useModalEnvironment(
  () => fileSearchOpen.value,
  fileSearchCardRef,
  () => { fileSearchOpen.value = false },
)
const treeDrawer = ref(false)
const tocDrawer = ref(false)

function openFileSearch() {
  if (!rootPath.value) {
    changeView('shelf')
    return
  }
  fileSearchQuery.value = ''
  fileSearchOpen.value = true
}

function selectSearchResult(path: string) {
  fileSearchOpen.value = false
  void onSelectFile(path)
}

function openMobileFilePicker() {
  if (rootPath.value) treeDrawer.value = true
  else changeView('shelf')
}
// Ref to the mobile drawer's FileTree so we can scroll it to the active file
// when the drawer opens — the tree's own on-activePath scroll can't fire then
// (the drawer is hidden while activeFile changes), so Reader triggers it here.
const drawerTreeRef = ref<InstanceType<typeof FileTree> | null>(null)
watch(treeDrawer, (open) => {
  if (open && activeFile.value) {
    void nextTick(() => requestAnimationFrame(() => drawerTreeRef.value?.scrollToActive()))
  }
})

// ── view switch (shelf ↔ read), Tasks-style slider ───────────────────
// The shelf is the new default landing view; the choice persists in
// localStorage and syncs to ?view= so it survives reloads and links.
const route = useRoute()
const router = useRouter()
const VIEW_STORAGE_KEY = 'reader.view'
type ReaderView = 'shelf' | 'read'

function initialViewMode(): ReaderView {
  const q = route.query.view
  if (q === 'shelf' || q === 'read') return q
  return localStorage.getItem(VIEW_STORAGE_KEY) === 'read' ? 'read' : 'shelf'
}

const viewMode = ref<ReaderView>(initialViewMode())
const shelfQuery = ref('')
const bookshelfRef = ref<InstanceType<typeof BookshelfView> | null>(null)

function changeView(mode: ReaderView) {
  // Leaving the reading view flushes any debounced progress first (FR-16).
  if (viewMode.value === 'read' && mode === 'shelf') flushProgressNow()
  viewMode.value = mode
  localStorage.setItem(VIEW_STORAGE_KEY, mode)
  void router.replace({ query: { ...route.query, view: mode } })
  // Entering the shelf from an immersive/fullscreen reading session restores the shell.
  if (mode === 'shelf') {
    if (isFullscreen.value && document.fullscreenElement) void document.exitFullscreen()
    leaveMobileImmersive()
    // The shelf scrolls in its own container and never drives handleScroll, so
    // a topbar collapsed by reading-scroll would strand the view switch — re-expand.
    appStore.handleScroll(0)
  }
}

/**
 * Open a shelf book and restore its progress (FR-12..14): folder books reopen
 * lastFile and scroll to the saved ratio (md) or jump to the saved page (pdf);
 * single-pdf books jump to the saved page (clamped). Stale/missing lastFile
 * silently falls back to the first file. Progress is per-file (see
 * utils/readerProgress.ts): a folder's lastFile may be .md (ratio) or .pdf
 * (page), disambiguated by the file's own `kind`.
 */
async function openBook(book: ReaderBook) {
  changeView('read')
  const state = shelf.getFileProgressState(book.id)
  if (book.kind === 'pdf') {
    const dir = book.path.substring(0, book.path.lastIndexOf('/'))
    await openPath(dir)
    const fp = state?.byFile[book.path]
    pendingPdfPage = fp ? clampPdfPage(fp.position, fp.pageCount ?? 0) : null
    await onSelectFile(book.path)
    return
  }
  await openPath(book.path)
  const lastFile = state?.lastFile
  const fp = lastFile ? state?.byFile[lastFile] : undefined
  if (lastFile && fp && flatFiles.value.includes(lastFile)) {
    if (fp.kind === 'pdf') {
      pendingPdfPage = clampPdfPage(fp.position, fp.pageCount ?? 0)
    } else {
      pendingRestoreRatio = fp.position
    }
    await onSelectFile(lastFile)
  } else if (flatFiles.value.length) {
    // Fallback (FR-13): stale/missing lastFile → first file, from the top.
    await onSelectFile(flatFiles.value[0])
  }
}

// ── bookshelf progress tracking ──────────────────────────────────────
// md books record lastFile + scroll ratio (debounced); pdf books record the
// page on every pagechange. A book is matched by rootPath (folder) or the
// displayed pdf path, so reading outside any book simply records nothing.
const shelf = useBookshelf()

const currentShelfBookId = computed<string | null>(() => {
  const books = shelf.books.value
  if (fileKind.value === 'pdf') {
    return books.find((b) => b.kind === 'pdf' && b.path === displayedFile.value)?.id ?? null
  }
  return books.find((b) => b.kind === 'folder' && b.path === rootPath.value)?.id ?? null
})

// Pending restore targets consumed after the file finishes rendering (set by
// openBook in the restore flow); while set, onSelectFile must not overwrite
// the saved progress it is about to restore.
let pendingRestoreRatio: number | null = null
let pendingPdfPage: number | null = null

/**
 * Keep a restored scroll ratio on target while the article's layout settles.
 * After the initial jump, late image loads and mermaid/code blocks above the
 * restore point (enhanced on intersection) keep changing scrollHeight, which
 * would silently drift the ratio (measured 0.5 → 0.37 on an image-heavy
 * note). Re-assert the ratio for a short window; any real user input
 * (wheel/touch/key) cancels the correction so it never fights the reader.
 */
let restoreCorrectionTimer: ReturnType<typeof setInterval> | null = null
function stopRestoreCorrection() {
  if (restoreCorrectionTimer !== null) {
    clearInterval(restoreCorrectionTimer)
    restoreCorrectionTimer = null
  }
}

function holdRatioForRestore(ratio: number, pane: HTMLElement) {
  stopRestoreCorrection()
  const cancel = () => stopRestoreCorrection()
  pane.addEventListener('wheel', cancel, { capture: true, passive: true, once: true })
  pane.addEventListener('touchstart', cancel, { capture: true, passive: true, once: true })
  pane.addEventListener('keydown', cancel, { capture: true, passive: true, once: true })
  const startedAt = performance.now()
  restoreCorrectionTimer = setInterval(() => {
    if (performance.now() - startedAt > 1800) {
      stopRestoreCorrection()
      return
    }
    pane.scrollTop = Math.round(ratio * (pane.scrollHeight - pane.clientHeight))
  }, 120)
}

const PROGRESS_DEBOUNCE_MS = 1500
let progressTimer: ReturnType<typeof setTimeout> | null = null

/** Debounced capture of the md scroll ratio (FR-15). */
function scheduleProgressCapture() {
  if (fileKind.value === 'pdf') return
  if (progressTimer) clearTimeout(progressTimer)
  progressTimer = setTimeout(() => {
    progressTimer = null
    captureProgressNow()
  }, PROGRESS_DEBOUNCE_MS)
}

/** Immediate capture — used by the debounce expiry and the flush points. */
function captureProgressNow() {
  if (fileKind.value === 'pdf') return
  const bookId = currentShelfBookId.value
  const el = contentRef.value
  if (!bookId || !el || !displayedFile.value) return
  shelf.saveFileProgress(
    bookId,
    displayedFile.value,
    captureMdFileProgress(displayedFile.value, el.scrollTop, el.scrollHeight, el.clientHeight, Date.now()),
  )
}

/** Flush pending debounced progress (view switch / unmount) without waiting. */
function flushProgressNow() {
  if (progressTimer) {
    clearTimeout(progressTimer)
    progressTimer = null
    captureProgressNow()
  }
}
/** App backgrounded (visibilitychange) — flush the md debounce so the position
 *  survives without a route-leave. localStorage writes synchronously. */
function onVisibilityHidden() {
  if (document.visibilityState === 'hidden') flushProgressNow()
}
/** Tab close (pagehide) — same flush for browsers that fire pagehide without
 *  a prior visibilitychange hidden (or in addition to it). */
function onPageHide() {
  flushProgressNow()
}
const viewerSvg = ref('')
const viewerTitle = ref('Mermaid 图')
const previewPath = ref('')        // non-md / out-of-folder link target → popup
const previewAnchor = ref('')      // line/symbol/heading anchor for the popup target
const viewerSource = ref('')
const contentRef = ref<HTMLElement | null>(null)
const readerPageRef = ref<HTMLElement | null>(null)
const readerBodyRef = ref<HTMLElement | null>(null)
const isFullscreen = ref(false)
const isMobileImmersive = ref(false)
const isImmersive = computed(() => isFullscreen.value || isMobileImmersive.value)
// Hide the mobile toolbar grip while immersed/fullscreen (the header + toolbar
// are display:none in those modes, so the grip would float pointlessly).
watch(isImmersive, (v) => appStore.setImmersive(v), { immediate: true })
const isFsTransitioning = ref(false)
interface RectSnapshot { left: number; top: number; width: number; height: number }
let pendingFullscreenRect: RectSnapshot | null = null
let settledFullscreenRect: RectSnapshot | null = null
let fullscreenAnimation: Animation | null = null
let fullscreenAnimationGeneration = 0
// Auto-hide UI in fullscreen for immersive reading.
const showFsUI = ref(true)
let fsUiTimer: ReturnType<typeof setTimeout> | null = null

function onFsActivity() {
  showFsUI.value = true
  if (fsUiTimer) clearTimeout(fsUiTimer)
  fsUiTimer = setTimeout(() => { showFsUI.value = false }, 3000)
}

function snapshotReaderBody(): RectSnapshot | null {
  const rect = readerBodyRef.value?.getBoundingClientRect()
  if (!rect || rect.width <= 0 || rect.height <= 0) return null
  return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
}

function cancelFullscreenAnimation() {
  fullscreenAnimationGeneration += 1
  fullscreenAnimation?.cancel()
  fullscreenAnimation = null
  isFsTransitioning.value = false
  readerPageRef.value?.classList.remove('is-fs-transitioning')
}

function animateFullscreenLayout(from: RectSnapshot | null, entering: boolean) {
  const body = readerBodyRef.value
  const page = readerPageRef.value
  if (!body || !page) return

  cancelFullscreenAnimation()
  const generation = fullscreenAnimationGeneration
  const to = snapshotReaderBody()
  if (!to) return
  if (entering) settledFullscreenRect = to

  // Temporarily flatten expensive glass materials while the large reading
  // surface moves. They materialise again once the compositor-only motion ends.
  isFsTransitioning.value = true
  page.classList.add('is-fs-transitioning')

  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  let keyframes: Keyframe[]
  let duration: number
  if (reduceMotion) {
    keyframes = [{ opacity: 0.86 }, { opacity: 1 }]
    duration = 140
  } else if (from) {
    const dx = from.left - to.left
    const dy = from.top - to.top
    const scaleX = from.width / to.width
    const scaleY = from.height / to.height
    keyframes = [
      { transform: `translate3d(${dx}px, ${dy}px, 0) scale(${scaleX}, ${scaleY})` },
      { transform: 'translate3d(0, 0, 0) scale(1, 1)' },
    ]
    duration = entering ? 340 : 300
  } else {
    keyframes = [
      { transform: `translate3d(0, ${entering ? 10 : -8}px, 0) scale(0.992)` },
      { transform: 'translate3d(0, 0, 0) scale(1)' },
    ]
    duration = 260
  }

  const animation = body.animate(keyframes, {
    duration,
    easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
    fill: 'none',
  })
  fullscreenAnimation = animation
  void animation.finished.catch(() => undefined).then(() => {
    if (generation !== fullscreenAnimationGeneration) return
    fullscreenAnimation = null
    isFsTransitioning.value = false
    page.classList.remove('is-fs-transitioning')
    if (entering) settledFullscreenRect = snapshotReaderBody()
  })
}

function leaveMobileImmersive() {
  isMobileImmersive.value = false
  document.documentElement.classList.remove('reader-mobile-immersive')
}

function onReaderKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && isMobileImmersive.value) leaveMobileImmersive()
}

async function toggleFullscreen() {
  const el = readerPageRef.value
  if (!el) return

  if (isMobileImmersive.value) {
    leaveMobileImmersive()
    return
  }

  if (isPhoneViewport(window.innerWidth)) {
    isMobileImmersive.value = true
    document.documentElement.classList.add('reader-mobile-immersive')
    return
  }

  // Capture the live presentation rect, so a rapid reverse starts from what is
  // currently on screen instead of snapping to the previous logical target.
  pendingFullscreenRect = snapshotReaderBody()
  cancelFullscreenAnimation()
  try {
    if (document.fullscreenElement === el) await document.exitFullscreen()
    else await el.requestFullscreen()
  } catch (e) {
    pendingFullscreenRect = null
    console.warn('切换全屏失败:', e)
    ElMessage.warning('当前浏览器不支持全屏')
  }
}
function onFullscreenChange() {
  const wasFullscreen = isFullscreen.value
  const entered = document.fullscreenElement === readerPageRef.value
  const liveRect = fullscreenAnimation ? snapshotReaderBody() : null
  const from = pendingFullscreenRect
    ?? liveRect
    ?? (wasFullscreen ? settledFullscreenRect : null)
  pendingFullscreenRect = null
  isFullscreen.value = entered
  animateFullscreenLayout(from, entered)
  if (!entered) settledFullscreenRect = null

  // Auto-hide UI listeners — only active in fullscreen.
  if (entered) {
    document.addEventListener('mousemove', onFsActivity)
    document.addEventListener('touchstart', onFsActivity)
    onFsActivity()
  } else {
    document.removeEventListener('mousemove', onFsActivity)
    document.removeEventListener('touchstart', onFsActivity)
    showFsUI.value = true
    if (fsUiTimer) { clearTimeout(fsUiTimer); fsUiTimer = null }
  }
}
// The file currently displayed — swaps only when its content is ready, driving the
// page-turn transition. (Separate from activeFile, which updates immediately for the
// tree highlight.)
const displayedFile = ref('')
const fileKind = ref<'md' | 'pdf'>('md')
const mobileDocumentLabel = computed(() => (
  displayedFile.value.split('/').pop() || (rootPath.value ? '选择文章' : '阅读工具')
))
const pdfViewerRef = ref<{
  scrollToPage: (n: number) => void
  setZoom: (m: 'fit' | number) => void
  setZoomRatio: (ratio: number) => void
} | null>(null)
const pdfCurrentPage = ref(1)
const pdfPageCount = ref(0)
const pdfZoomIndex = ref(2)
const pdfZoomLevels = [0.7, 0.85, 1, 1.2, 1.45, 1.75]

function fitPdf() {
  pdfZoomIndex.value = 2
  pdfViewerRef.value?.setZoom('fit')
}

function setPdfZoom(direction: -1 | 1) {
  pdfZoomIndex.value = Math.max(0, Math.min(pdfZoomLevels.length - 1, pdfZoomIndex.value + direction))
  pdfViewerRef.value?.setZoomRatio(pdfZoomLevels[pdfZoomIndex.value])
}
const transitionDir = ref<'page-next' | 'page-prev'>('page-next')

// Markdown paths in tree display order (depth-first) — used to pick turn direction.
const flatFiles = computed(() => {
  const out: string[] = []
  const walk = (entries: DirEntry[]) => {
    for (const e of entries) {
      if (e.is_dir) e.children && walk(e.children)
      else if (e.is_markdown || e.is_pdf) out.push(e.path)
    }
  }
  walk(tree.value)
  return out
})

function handleMermaidClick(svg: SVGElement, source: string) {
  viewerSource.value = source
  viewerTitle.value = 'Mermaid 图'
  viewerSvg.value = svg.outerHTML
}
function handleImageClick(src: string, alt: string) {
  viewerSource.value = alt
  viewerTitle.value = alt || '图片'
  viewerSvg.value = `<img src="${src}" alt="${alt}" />`
}

// Anchor to scroll to after a cross-file link opens a new document.
const pendingAnchor = ref('')

/** Resolve a relative href against the current file's directory. */

/** Whether a path is inside the currently opened folder. */
function isUnderRoot(p: string): boolean {
  const root = rootPath.value
  if (!root) return false
  const r = root.endsWith('/') ? root : root + '/'
  return p === root || p.startsWith(r)
}

/** A relative link was clicked — route by target type and folder membership. */
function handleLinkClick(href: string) {
  const base = displayedFile.value
  if (!base) return
  const baseDir = base.substring(0, base.lastIndexOf('/'))
  const [pathPartRaw, anchor] = href.split('#')
  const resolved = resolveRelativePath(baseDir, decodeURIComponent(pathPartRaw))
  if (!resolved) return

  // Markdown / PDF under the opened folder → jump in the reader (page-turn).
  if (/\.(md|markdown|pdf)$/i.test(resolved) && isUnderRoot(resolved)) {
    if (anchor) pendingAnchor.value = decodeURIComponent(anchor)
    if (resolved !== displayedFile.value) {
      void onSelectFile(resolved) // onArticleEnter scrolls to pendingAnchor after render
    } else if (pendingAnchor.value) {
      scrollToHeading(pendingAnchor.value)
      pendingAnchor.value = ''
    }
    return
  }

  // Everything else (folders, code, out-of-folder md, …) → preview popup.
  previewAnchor.value = anchor ? decodeURIComponent(anchor) : ''
  previewPath.value = resolved
}

/** The preview modal asked to open an md (under the folder) in the main reader. */
function onPreviewOpenInReader(path: string, anchor?: string) {
  previewPath.value = ''
  if (anchor) pendingAnchor.value = anchor
  void onSelectFile(path)
}

const {
  renderMarkdown,
  enhance,
  cleanup: cleanupMarkdown,
} = useMarkdownRender(handleMermaidClick, handleLinkClick, handleImageClick, scrollToHeading)

// ── open folder ───────────────────────────────────────────────────────
// Refresh feedback — works inside fullscreen (ElMessage is hidden in fullscreen).
const refreshFlash = ref<'none' | 'success' | 'error'>('none')
let refreshFlashTimer: ReturnType<typeof setTimeout> | null = null
function showRefreshFeedback(type: 'success' | 'error') {
  if (isFullscreen.value) {
    // ElMessage is invisible in fullscreen mode — use in-page toast instead.
    refreshFlash.value = type
    if (refreshFlashTimer) clearTimeout(refreshFlashTimer)
    refreshFlashTimer = setTimeout(() => { refreshFlash.value = 'none' }, 2000)
  } else {
    // Non-fullscreen: use ElMessage (same style as Timeline sync).
    if (type === 'success') ElMessage.success('目录已刷新')
    else ElMessage.error('刷新失败')
  }
}

async function refreshTree() {
  if (!rootPath.value) return
  try {
    const res = await listLocalDir(rootPath.value)
    if (res.status === 'success' && res.result) {
      tree.value = res.result.entries
      showRefreshFeedback('success')
    } else {
      showRefreshFeedback('error')
    }
  } catch (e) {
    console.error('刷新目录失败:', e)
    showRefreshFeedback('error')
  }
}

async function openPath(rawPath: string) {
  const path = rawPath.trim()
  if (!path) return
  cancelPendingFileSelection()
  error.value = ''
  try {
    const res = await listLocalDir(path)
    if (res.status === 'error' || !res.result) {
      error.value = res.error?.message || '打开失败'
      ElMessage.error(error.value)
      return
    }
    tree.value = res.result.entries
    rootPath.value = res.result.root
    renderedHtml.value = ''
    displayedFile.value = ''
    activeFile.value = ''
    toc.value = []
    // Reset file kind: opening a folder clears the current document. Without this,
    // a stale fileKind==='pdf' + cleared renderedHtml/displayedFile would mount
    // PdfViewer with an empty src (→ 400 /v1/reader/raw?path=).
    fileKind.value = 'md'
    // Remember last opened folder; clear stale last-file until a new one is chosen.
    localStorage.setItem(LAST_FOLDER_KEY, path)
    localStorage.removeItem(LAST_FILE_KEY)
  } catch (e) {
    error.value = (e as Error)?.message || '打开失败'
    ElMessage.error(error.value)
  }
}

// ── select & render file ──────────────────────────────────────────────
let fileRequest: AbortController | null = null
let selectionVersion = 0

function cancelPendingFileSelection() {
  selectionVersion += 1
  fileRequest?.abort()
  fileRequest = null
  fileLoading.value = false
}

async function onSelectFile(path: string) {
  activeFile.value = path
  // Already displaying this file — skip to avoid resetting rendered mermaid back to source.
  if (path === displayedFile.value) return
  fileRequest?.abort()
  const request = new AbortController()
  fileRequest = request
  const version = ++selectionVersion
  fileLoading.value = true
  error.value = ''

  const isPdf = /\.pdf$/i.test(path)
  // Determine page-turn direction from the file's position in the tree.
  const oldIdx = flatFiles.value.indexOf(displayedFile.value)
  const newIdx = flatFiles.value.indexOf(path)
  transitionDir.value =
    oldIdx >= 0 && newIdx >= 0 && newIdx < oldIdx ? 'page-prev' : 'page-next'

  try {
    if (isPdf) {
      // PDF: don't read/render text — just hand the path to PdfViewer. The outline
      // arrives via onPdfOutline after pdf.js loads the document.
      fileKind.value = 'pdf'
      pdfCurrentPage.value = 1
      pdfPageCount.value = 0
      pdfZoomIndex.value = 2
      renderedHtml.value = '' // ensure PdfViewer branch shows
      displayedFile.value = path
      toc.value = [] // outline arrives via onPdfOutline after load
      localStorage.setItem(LAST_FILE_KEY, path)
    } else {
      const res = await readLocalFile(path, request.signal)
      if (version !== selectionVersion || request.signal.aborted) return
      if (res.status === 'error' || !res.result) {
        error.value = res.error?.message || '读取失败'
        ElMessage.error(error.value)
        return
      }
      // Render new content, then swap the transition key in the SAME tick so the leaving
      // <article> stays frozen on the OLD content while the new one slides in.
      fileKind.value = 'md'
      const html = await renderMarkdown(
        res.result.content,
        makeReaderImageResolvers(path, rootPath.value),
        path,
      )
      if (version !== selectionVersion || request.signal.aborted) return
      renderedHtml.value = html
      displayedFile.value = path
      localStorage.setItem(LAST_FILE_KEY, path)
      // Folder-book progress: a fresh file starts from the top (FR-15 lastFile).
      // Skip while restoring a saved position — the restore owns the next write.
      // openFile only seeds a new file's entry (position 0); an existing file's
      // saved progress is never clobbered here.
      const bookId = currentShelfBookId.value
      if (bookId && pendingRestoreRatio === null && pendingPdfPage === null) {
        shelf.openFile(bookId, path, deriveFileKind(path))
      }
      // enhance() + buildToc() run in the transition's @enter hook (onArticleEnter).
    }
  } catch (e) {
    if (version !== selectionVersion || request.signal.aborted) return
    error.value = (e as Error)?.message || '读取失败'
  } finally {
    if (version === selectionVersion) {
      fileRequest = null
      fileLoading.value = false
    }
  }
}

/** Runs when a new <article> enters the page-turn transition: highlight, mermaid, TOC. */
async function onArticleEnter(el: Element) {
  // Only the markdown <article> needs enhancing; PdfViewer handles itself.
  if (el.tagName !== 'ARTICLE') return
  // Don't reset scroll if we're jumping to a cross-file anchor.
  if (contentRef.value && !pendingAnchor.value) contentRef.value.scrollTop = 0
  buildToc(el as HTMLElement)
  await enhance(el as HTMLElement)
  if (pendingAnchor.value) {
    scrollToHeading(pendingAnchor.value)
    pendingAnchor.value = ''
  }
  // Book progress restore (FR-13): scroll to the saved ratio after enhance,
  // so images/code highlighting have settled into the layout — then keep the
  // ratio on target while late layout shifts settle (holdRatioForRestore).
  if (pendingRestoreRatio !== null) {
    const ratio = pendingRestoreRatio
    pendingRestoreRatio = null
    await nextTick()
    const pane = contentRef.value
    if (pane) {
      pane.scrollTop = Math.round(ratio * (pane.scrollHeight - pane.clientHeight))
      // Skip one debounce cycle: the restore scroll fires onContentScroll,
      // which would re-capture (and re-save) the exact ratio we just restored.
      if (progressTimer) clearTimeout(progressTimer)
      progressTimer = null
      holdRatioForRestore(ratio, pane)
    }
  }
}

function onContentAfterLeave(el: Element) {
  if (el.tagName === 'ARTICLE') cleanupMarkdown(el as HTMLElement)
}

/** PdfViewer emits its outline after load; populate the TOC. */
function onPdfPageChange(page: number) {
  pdfCurrentPage.value = page
  // Skip capturing during a restore jump — pdf.js fires intermediate page
  // events as it renders toward the target page; the restore owns the position.
  if (pendingPdfPage !== null) return
  const bookId = currentShelfBookId.value
  const file = displayedFile.value
  if (bookId && file) {
    shelf.saveFileProgress(
      bookId,
      file,
      capturePdfFileProgress(file, page, pdfPageCount.value || undefined, Date.now()),
    )
  }
}

function onPdfPageCount(count: number) {
  pdfPageCount.value = count
  // Book-open restore (FR-14): page wraps mount with pageMetas as the pdf
  // loads, so one rAF after the count arrives the target wrap is addressable.
  if (pendingPdfPage !== null) {
    const target = clampPdfPage(pendingPdfPage, count)
    pendingPdfPage = null
    requestAnimationFrame(() => {
      // Same programmatic-scroll family as TOC jumps — don't let the mobile
      // header collapse mid-restore (see holdHeaderForJump).
      holdHeaderForJump()
      pdfViewerRef.value?.scrollToPage(target)
    })
  }
}

function onPdfOutline(items: { text: string; level: number; page: number }[]) {
  toc.value = items.map((it, i) => ({
    id: `pdf-outline-${i}`,
    text: it.text,
    level: it.level,
    page: it.page,
  }))
}

/** TOC click: jump to a PDF page (if pdf) or a markdown heading (if md). */
function onTocClick(t: TocItem) {
  if (t.page !== undefined && fileKind.value === 'pdf') {
    // PDF pages flow in pane-center too, so the jump scrolls the same
    // container — hold the header state before PdfViewer scrolls it.
    holdHeaderForJump()
    pdfViewerRef.value?.scrollToPage(t.page)
  } else {
    scrollToHeading(t.id)
  }
}

function currentMarkdownBody(): HTMLElement | null {
  const bodies = contentRef.value?.querySelectorAll<HTMLElement>('.markdown-body')
  return bodies?.[bodies.length - 1] ?? null
}

function findMarkdownHeading(id: string, root: HTMLElement | null = currentMarkdownBody()): HTMLElement | null {
  if (!root) return null
  const candidates = Array.from(new Set([id, normalizeHeadingAnchor(id)]))
  for (const candidate of candidates) {
    const escaped = CSS.escape(candidate)
    const heading = root.querySelector<HTMLElement>(
      `#${escaped}, [data-anchor="${escaped}"], [data-block-anchor="${escaped}"]`,
    )
    if (heading) return heading
  }
  return null
}

function headingTocHtml(heading: HTMLElement): string | undefined {
  if (!heading.querySelector('.katex')) return undefined
  const clone = heading.cloneNode(true) as HTMLElement
  clone.querySelectorAll('[id]').forEach((element) => element.removeAttribute('id'))
  clone.querySelectorAll('a').forEach((anchor) => anchor.replaceWith(...anchor.childNodes))
  clone.querySelectorAll('img').forEach((image) => image.replaceWith(document.createTextNode(image.alt)))
  return clone.innerHTML
}

function buildToc(article?: HTMLElement) {
  const body = article?.matches('.markdown-body') ? article : currentMarkdownBody()
  if (!body) { toc.value = []; return }
  const els = Array.from(body.querySelectorAll<HTMLElement>('h1,h2,h3,h4,h5,h6'))
  toc.value = els
    .filter((el) => el.id && el.dataset.headingLabel)
    .map((el) => ({
      id: el.id,
      text: el.dataset.headingLabel || el.textContent || '',
      level: Number(el.tagName[1]),
      html: headingTocHtml(el),
    }))
}

// Mobile controls stay out of the reading surface until the user moves it.
const mobileToolbarVisible = ref(false)
const mobileToolbarState = computed(() => getMobileReaderToolbarState(
  Boolean(displayedFile.value),
  mobileToolbarVisible.value,
  treeDrawer.value || tocDrawer.value || fileSearchOpen.value,
))
const MOBILE_TOOLBAR_IDLE_MS = 2800
let mobileToolbarTimer: ReturnType<typeof setTimeout> | null = null
function revealMobileToolbar() {
  if (!isPhoneViewport(window.innerWidth)) return
  mobileToolbarVisible.value = true
  if (mobileToolbarTimer) clearTimeout(mobileToolbarTimer)
  if (mobileToolbarState.value.pinned) {
    mobileToolbarTimer = null
    return
  }
  mobileToolbarTimer = setTimeout(() => {
    mobileToolbarVisible.value = false
    mobileToolbarTimer = null
  }, MOBILE_TOOLBAR_IDLE_MS)
}

// Selecting the first document releases the pinned picker into the normal
// transient reading chrome, while keeping it visible long enough for context.
watch(displayedFile, (file, previousFile) => {
  if (file && file !== previousFile) revealMobileToolbar()
})

let contentScrollFrame: number | null = null
// Programmatic jumps (TOC click, in-note anchor, PDF page jump) scroll
// pane-center, whose scroll handler also drives the app's mobile header
// collapse. Toggling the header mid-jump shifts the layout ~100px — the
// whole page visibly floats up — so jumps freeze the header state for a
// window that outlasts the smooth scroll; user scrolls afterwards resume
// driving the collapse.
let headerHoldUntil = 0

function holdHeaderForJump() {
  headerHoldUntil = performance.now() + 1600
}

function onContentScroll() {
  if (contentScrollFrame !== null) return
  contentScrollFrame = window.requestAnimationFrame(processContentScroll)
}

function processContentScroll() {
  contentScrollFrame = null
  revealMobileToolbar()
  scheduleProgressCapture()
  // Drive the app's mobile header + page-header collapse from the pane-center
  // scroll (app-main doesn't scroll on the Reader page) — unless a
  // programmatic jump is currently scrolling (see holdHeaderForJump).
  if (contentRef.value && performance.now() > headerHoldUntil) {
    appStore.handleScroll(contentRef.value.scrollTop)
  }
  if (!contentRef.value || fileKind.value === 'pdf' || !toc.value.length) return
  const containerTop = contentRef.value.getBoundingClientRect().top
  let current = ''
  for (const t of toc.value) {
    const el = findMarkdownHeading(t.id)
    if (!el) continue
    const top = el.getBoundingClientRect().top - containerTop
    if (top <= 90) current = t.id
    else break
  }
  if (current !== activeHeading.value) {
    activeHeading.value = current
    // Auto-scroll TOC panel to keep the active heading centered.
    scrollTocToActive()
  }
}

/** Scroll the TOC sidebar so the active heading is centered (unless at top/bottom). */
function scrollTocToActive() {
  const tocEl = document.querySelector('.pane-right .pane-scroll') as HTMLElement | null
  if (!tocEl || !activeHeading.value) return
  const activeEl = tocEl.querySelector(`.toc-item.active`) as HTMLElement | null
  if (!activeEl) return
  const target = activeEl.offsetTop - tocEl.clientHeight / 2 + activeEl.clientHeight / 2
  const maxScroll = tocEl.scrollHeight - tocEl.clientHeight
  tocEl.scrollTop = Math.max(0, Math.min(target, maxScroll))
}

function scrollToHeading(id: string) {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  holdHeaderForJump()
  findMarkdownHeading(id)?.scrollIntoView({
    behavior: reduceMotion ? 'auto' : 'smooth',
    block: 'start',
  })
}

onMounted(async () => {
  document.addEventListener('fullscreenchange', onFullscreenChange)
  document.addEventListener('keydown', onReaderKeydown)
  // Flush pending md progress when the app is backgrounded or the tab is
  // closing — on mobile, route-leave (onBeforeUnmount) doesn't fire on tab
  // close / app switch, and pagehide clears the debounce timer. localStorage
  // writes synchronously, so the position survives. (PDF saves on every page
  // change, so it's already current — this only covers the md debounce.)
  document.addEventListener('visibilitychange', onVisibilityHidden)
  window.addEventListener('pagehide', onPageHide)
  void shelf.ensureLoaded()
  // Restore last opened folder + file (per-browser).
  const lastFolder = localStorage.getItem(LAST_FOLDER_KEY)
  const lastFile = localStorage.getItem(LAST_FILE_KEY)
  if (lastFolder) {
    await openPath(lastFolder)
    if (lastFile) await onSelectFile(lastFile)
  }
})

onBeforeUnmount(() => {
  leaveMobileImmersive()
  flushProgressNow() // route-leave flush point (FR-16)
  stopRestoreCorrection()
  cancelPendingFileSelection()
  cleanupMarkdown()
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  document.removeEventListener('keydown', onReaderKeydown)
  document.removeEventListener('visibilitychange', onVisibilityHidden)
  window.removeEventListener('pagehide', onPageHide)
  document.removeEventListener('mousemove', onFsActivity)
  document.removeEventListener('touchstart', onFsActivity)
  cancelFullscreenAnimation()
  if (fsUiTimer) clearTimeout(fsUiTimer)
  if (refreshFlashTimer) clearTimeout(refreshFlashTimer)
  if (mobileToolbarTimer) clearTimeout(mobileToolbarTimer)
  if (contentScrollFrame !== null) cancelAnimationFrame(contentScrollFrame)
  if (document.fullscreenElement) void document.exitFullscreen()
})
</script>

<style scoped>
.reader-page {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 64px);
  height: calc(100dvh - 64px);
  gap: 10px;
}
/* Tighter page-header spacing — the Reader is a tool page, not a content page. */
.reader-page .page-header { margin-bottom: 6px; }

/* Tasks-style view switch (shelf ↔ read) */
.view-switch {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: 174px;
  padding: 3px;
  border-radius: 13px;
  background: color-mix(in srgb, var(--text-primary) 5%, transparent);
  isolation: isolate;
  flex-shrink: 0;
}
.switch-indicator {
  position: absolute;
  inset: 3px auto 3px 3px;
  width: calc(50% - 3px);
  border-radius: 10px;
  background: var(--bg-glass-strong);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
  z-index: -1;
}
.switch-indicator.read { transform: translateX(100%); }
.view-switch button {
  min-height: 36px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-weight: 570;
  cursor: pointer;
}
.view-switch button.active { color: var(--text-primary); }

/* Bookshelf fills the body area like reader-body does */
.bookshelf-root {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}
/* Fullscreen reading mode — immersive: hide app chrome (title/input), keep file
   tree + TOC. The article's H1 sticks to the top with a frosted glass bar. */
.reader-page:fullscreen {
  position: relative;
  isolation: isolate;
  height: 100vh;
  width: 100%;
  background: var(--bg-base);
  border-radius: 0;
  overflow: hidden;
  cursor: default;
}
.reader-page:fullscreen::before {
  content: '';
  position: absolute;
  inset: -10%;
  pointer-events: none;
  z-index: 0;
  background:
    radial-gradient(ellipse at 75% 15%, rgba(196, 181, 253, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(165, 243, 252, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%);
  opacity: var(--orb-opacity, 1);
}
:root[data-theme="eye-care"] .reader-page:fullscreen::before {
  background:
    radial-gradient(ellipse at 75% 15%, rgba(120, 180, 100, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(160, 200, 140, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(200, 220, 170, 0.12), transparent 50%);
}
:root[data-theme="dark"] .reader-page:fullscreen::before {
  opacity: 0.35;
}
.reader-page:fullscreen.fs-ui-hidden { cursor: none; }
.reader-page:fullscreen .reader-body { position: relative; z-index: 1; }
/* Hide only the page title + input bar; keep file tree + TOC + FABs. */
.reader-page:fullscreen .page-header,
.reader-page:fullscreen .reader-topbar { display: none !important; }
.reader-page:fullscreen .pane-center {
  display: block;
  background: transparent; border: none;
  backdrop-filter: none; -webkit-backdrop-filter: none;
  box-shadow: none; border-radius: 0;
}
.reader-page:fullscreen .markdown-body {
  --fullscreen-document-gutter: 14px;
}
/* The glass uses the same outer width as .markdown-body. Its layered, static
   highlights imply refraction without adding a perpetual paint animation. */
.reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
  --title-glass-fill: color-mix(in srgb, var(--bg-glass-strong) 68%, transparent);
  --title-glass-edge: color-mix(in srgb, var(--border-glass) 74%, white 26%);
  --title-glass-glint: rgba(255, 255, 255, 0.5);
  --title-glass-shadow: rgba(28, 31, 45, 0.12);
  position: sticky;
  top: 0;
  z-index: 10;
  width: calc(100% + (var(--fullscreen-document-gutter) * 2));
  max-width: none;
  margin: 0 calc(var(--fullscreen-document-gutter) * -1) 24px;
  padding: 14px var(--fullscreen-document-gutter);
  overflow: hidden;
  isolation: isolate;
  color: var(--text-primary);
  font-weight: 680;
  letter-spacing: -0.025em;
  background:
    radial-gradient(120% 180% at 8% -90%, var(--title-glass-glint), transparent 58%),
    linear-gradient(112deg, color-mix(in srgb, var(--accent) 8%, transparent), transparent 38% 72%, rgba(255, 255, 255, 0.12)),
    var(--title-glass-fill);
  border: 1px solid var(--title-glass-edge);
  border-radius: 18px;
  backdrop-filter: blur(24px) saturate(175%) contrast(1.04) brightness(1.02);
  -webkit-backdrop-filter: blur(24px) saturate(175%) contrast(1.04) brightness(1.02);
  box-shadow:
    inset 0 1px 0 var(--title-glass-glint),
    inset 0 -1px 0 color-mix(in srgb, var(--accent) 10%, transparent),
    0 10px 32px var(--title-glass-shadow),
    0 1px 2px rgba(0, 0, 0, 0.04);
  text-shadow: 0 1px 0 color-mix(in srgb, var(--bg-base) 46%, transparent);
}
:root[data-theme="dark"] .reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
  --title-glass-fill: color-mix(in srgb, var(--bg-glass-strong) 78%, transparent);
  --title-glass-edge: rgba(255, 255, 255, 0.13);
  --title-glass-glint: rgba(255, 255, 255, 0.17);
  --title-glass-shadow: rgba(0, 0, 0, 0.38);
  background:
    radial-gradient(120% 180% at 8% -90%, rgba(255, 255, 255, 0.16), transparent 58%),
    linear-gradient(112deg, color-mix(in srgb, var(--accent) 12%, transparent), transparent 42% 74%, rgba(255, 255, 255, 0.045)),
    var(--title-glass-fill);
  backdrop-filter: blur(26px) saturate(150%) contrast(1.08) brightness(0.86);
  -webkit-backdrop-filter: blur(26px) saturate(150%) contrast(1.08) brightness(0.86);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
}
:root[data-theme="eye-care"] .reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
  --title-glass-fill: color-mix(in srgb, var(--bg-glass-strong) 76%, transparent);
  --title-glass-edge: rgba(230, 255, 222, 0.44);
  --title-glass-glint: rgba(245, 255, 240, 0.4);
  --title-glass-shadow: rgba(40, 78, 34, 0.14);
}
/* Desktop fullscreen: wider document, more breathing room. */
@media (min-width: 769px) {
  .reader-page:fullscreen {
    padding: 16px 24px;
  }
  .reader-page:fullscreen .markdown-body {
    --fullscreen-document-gutter: 48px;
    max-width: 920px;
    padding: 32px 48px 160px;
    font-size: 16px;
    line-height: var(--leading-relaxed);
  }
  .reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
    padding-block: 16px;
  }
}

/* FLIP transition: only the reader body moves. Blur is flattened during motion
   so PDF canvases and long Markdown pages remain on the compositor fast path. */
.reader-page.is-fs-transitioning .reader-body {
  transform-origin: 0 0;
  will-change: transform, opacity;
}
.reader-page.is-fs-transitioning .pane,
.reader-page.is-fs-transitioning .markdown-body :deep(h1:first-of-type) {
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}
.reader-page.is-fs-transitioning .pane { background: var(--bg-glass-strong); }
.reader-page.is-fs-transitioning .markdown-body :deep(h1:first-of-type) {
  background: var(--title-glass-fill);
  box-shadow: inset 0 1px 0 var(--title-glass-glint), 0 6px 20px var(--title-glass-shadow);
}

/* Floating fullscreen UI — auto-hides after inactivity. */
.fs-ui {
  position: fixed; top: max(24px, calc(var(--safe-top) + 12px)); right: max(24px, calc(var(--safe-right) + 12px)); z-index: 100;
  display: flex; gap: 10px;
  transition: opacity 0.4s var(--ease-out);
  animation: fs-ui-materialize var(--motion-fast) var(--ease-emphasized) backwards;
}
.fs-ui.hidden { opacity: 0; pointer-events: none; }
.fs-fab {
  width: 42px; height: 42px; border-radius: 50%;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  backdrop-filter: blur(12px) saturate(150%);
  -webkit-backdrop-filter: blur(12px) saturate(150%);
  color: var(--text-secondary);
  display: flex; align-items: center; justify-content: center;
  cursor: pointer;
  box-shadow: var(--shadow-md);
  transition: transform 100ms ease-out, color 0.2s var(--ease-out);
}
.fs-fab:hover { color: var(--text-primary); }
.fs-fab:active { transform: scale(0.92); }
@keyframes fs-ui-materialize {
  from { opacity: 0; transform: translateY(-6px) scale(0.96); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}
/* Match the native fullscreen backdrop to the reading surface, preventing a
   black flash while the browser hands the element to/from the top layer. */
.reader-page::backdrop {
  background:
    radial-gradient(ellipse at 75% 15%, rgba(196, 181, 253, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(165, 243, 252, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%),
    var(--bg-base);
}
/* ── Top bar — glass container like the Tasks toolbar ── */
.glass-surface {
  background: var(--bg-glass);
  border: 1px solid var(--border-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
}
.reader-topbar {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-shrink: 0;
  min-height: 58px;
  padding: 8px 10px;
  border-radius: 18px;
}
.reader-shelf-add { display: none; }
/* Book search pill — task-search metrics (nested glass inside the toolbar). */
.shelf-search {
  flex: 1;
  min-width: 170px;
  height: 40px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 14px;
  border-radius: 12px;
  color: var(--text-faint);
  transition: background-color var(--motion-fast) var(--ease-emphasized),
    border-color var(--motion-fast) var(--ease-emphasized),
    box-shadow var(--motion-fast) var(--ease-emphasized);
}
.shelf-search:focus-within {
  background: var(--bg-glass-strong);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.06), inset 0 1px 0 rgba(255, 255, 255, 0.03);
}
.shelf-search-icon {
  flex: none;
  color: var(--text-faint);
  transition: color var(--motion-fast) var(--ease-emphasized);
}
.shelf-search:focus-within .shelf-search-icon { color: var(--accent); }
.shelf-search input {
  width: 100%;
  min-width: 0;
  padding: 0;
  border: 0;
  outline: 0;
  appearance: none;
  -webkit-appearance: none;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: 14px;
}
.shelf-search input::-webkit-search-cancel-button { display: none; }
.shelf-search input::placeholder { color: var(--text-faint); }
.shelf-search-clear {
  width: 22px;
  height: 22px;
  flex: none;
  display: grid;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: var(--border-faint);
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease,
    transform var(--motion-instant) ease;
}
.shelf-search-clear:active { transform: scale(0.9); }
.path-trigger {
  flex: 1; display: flex; align-items: center; gap: 8px;
  min-height: 40px; padding: 0 14px; border-radius: 12px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  color: var(--text-muted); font-size: 14px;
  cursor: pointer; text-align: left;
  transition: color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
  overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
}
.path-trigger:hover { background: var(--bg-hover); color: var(--text-secondary); border-color: var(--accent-border); }
.pt-name { font-weight: 600; color: var(--text-primary); flex-shrink: 0; }
.pt-path { color: var(--text-faint); font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; }
.pt-hint { color: var(--text-faint); }
.path-trigger:hover { background: var(--bg-hover); color: var(--text-secondary); border-color: var(--accent-border); }
/* Fullscreen tile — 40px glass square matching the path pill beside it
   (colors ride Element Plus button vars so hover/active stay native). */
.reader-topbar .icon-btn {
  --el-button-bg-color: var(--bg-glass);
  --el-button-text-color: var(--text-muted);
  --el-button-border-color: var(--border-glass);
  --el-button-hover-bg-color: var(--bg-glass-strong);
  --el-button-hover-text-color: var(--accent);
  --el-button-hover-border-color: var(--accent-border);
  --el-button-active-bg-color: var(--bg-glass-strong);
  --el-button-active-text-color: var(--accent);
  --el-button-active-border-color: var(--accent-border);
  width: 40px;
  height: 40px;
  padding: 0;
  border-radius: 12px !important;
  flex-shrink: 0;
}
.reader-topbar .icon-btn .el-icon { font-size: 17px; }

/* ── Floating path overlay (command-palette style) ── */
.path-overlay {
  position: fixed; inset: 0; z-index: 200;
  display: flex; align-items: flex-start; justify-content: center;
  padding-top: 15vh;
  background: rgba(0, 0, 0, 0.15);
  backdrop-filter: blur(12px) saturate(150%);
  -webkit-backdrop-filter: blur(12px) saturate(150%);
}
:root[data-theme="dark"] .path-overlay { background: rgba(0, 0, 0, 0.35); }
.path-card {
  width: 720px; max-width: calc(100vw - 32px);
  display: flex; flex-direction: column;
  background: var(--bg-glass-strong);
  backdrop-filter: blur(28px) saturate(180%);
  -webkit-backdrop-filter: blur(28px) saturate(180%);
  border: 1px solid var(--border-glass);
  border-radius: 20px;
  box-shadow: var(--shadow-lg), var(--inset-highlight);
  overflow: hidden;
}
.path-card.is-file-search { max-height: min(72vh, 620px); }

/* Overlay transitions */
.overlay-fade-enter-active, .overlay-fade-leave-active { transition: opacity 0.25s var(--ease-out); }
.overlay-fade-enter-from, .overlay-fade-leave-to { opacity: 0; }
.overlay-pop-enter-active { transition: opacity var(--motion-normal) var(--ease-emphasized), transform var(--motion-normal) var(--ease-spring-gentle); }
.overlay-pop-leave-active { transition: opacity 0.15s var(--ease-out), transform 0.15s var(--ease-out); }
.overlay-pop-enter-from { opacity: 0; transform: scale(0.96) translateY(-12px); }
.overlay-pop-leave-to { opacity: 0; transform: scale(0.98) translateY(-8px); }

.reader-mobile-toolbar { display: none; }


/* ── Body / panes ── */
.reader-body { flex: 1; display: flex; gap: 16px; min-height: 0; }
.pane {
  display: flex; flex-direction: column;
  background: var(--bg-glass);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  border: 1px solid var(--border-glass);
  border-radius: 18px;
  box-shadow: var(--shadow-md), var(--inset-highlight);
  overflow: hidden;
  min-height: 0;
}
.pane-title {
  flex-shrink: 0;
  padding: 12px 16px;
  font-size: 12px; font-weight: 600; letter-spacing: 0.5px;
  color: var(--text-muted); text-transform: uppercase;
  border-bottom: 1px solid var(--border-faint);
}
.pane-title-bar {
  flex-shrink: 0;
  display: flex; align-items: center; justify-content: space-between;
  padding: 8px 10px 8px 16px;
  border-bottom: 1px solid var(--border-faint);
}
.pane-title-text {
  font-size: 12px; font-weight: 600; letter-spacing: 0.5px;
  color: var(--text-muted); text-transform: uppercase;
}
.pane-title-btn {
  width: 24px; height: 24px; border-radius: 7px; border: none;
  background: transparent; color: var(--text-muted); cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized);
}
.pane-title-btn:hover { background: var(--bg-glass-subtle); color: var(--accent); }
.pane-title-btn:active { transform: scale(0.92); }
.reader-toast {
  position: fixed; top: 80px; left: 50%; transform: translateX(-50%);
  z-index: 9999;
  padding: 10px 24px; border-radius: 12px;
  font-size: 14px; font-weight: 500;
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  box-shadow: var(--shadow-lg);
  pointer-events: none;
}
.reader-toast.success { background: rgba(74, 222, 128, 0.25); color: #4ade80; border: 1px solid rgba(74, 222, 128, 0.3); }
.reader-toast.error { background: rgba(248, 113, 113, 0.25); color: #f87171; border: 1px solid rgba(248, 113, 113, 0.3); }
.toast-slide-enter-active { transition: opacity 0.3s var(--ease-out), transform 0.3s var(--ease-spring); }
.toast-slide-leave-active { transition: opacity 0.3s var(--ease-out), transform 0.3s var(--ease-out); }
.toast-slide-enter-from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
.toast-slide-leave-to { opacity: 0; transform: translateX(-50%) translateY(-8px); }
.pane-scroll { flex: 1; overflow-y: auto; padding: 8px 0; }
.pane-hint { padding: 24px 16px; font-size: 12px; color: var(--text-faint); text-align: center; line-height: 1.6; }

.pane-left { width: 250px; flex-shrink: 0; }
.pane-right { width: 210px; flex-shrink: 0; }
.pane-center {
  flex: 1; min-width: 0; overflow-y: auto;
  padding: 0; /* .markdown-body has its own padding */
  position: relative; /* anchor for loadbar / error-banner */
}

/* ── page-turn transition (directional slide + fade) ── */
.page-next-enter-active,
.page-next-leave-active,
.page-prev-enter-active,
.page-prev-leave-active {
  transition: transform var(--motion-page) var(--ease-emphasized),
              opacity var(--motion-page) var(--ease-emphasized);
}
.page-next-leave-active,
.page-prev-leave-active {
  position: absolute;
  inset: 0;
  width: 100%;
  pointer-events: none;
}
.page-next-enter-from { transform: translateX(16px); opacity: 0; }
.page-next-leave-to { transform: translateX(-16px); opacity: 0; }
.page-prev-enter-from { transform: translateX(-16px); opacity: 0; }
.page-prev-leave-to { transform: translateX(16px); opacity: 0; }

/* ── loading bar + error banner ── */
.loadbar {
  position: absolute; top: 0; left: 0; right: 0; height: 2px;
  overflow: hidden; z-index: 5; pointer-events: none;
}
.loadbar span {
  display: block; height: 100%; width: 40%;
  background: var(--accent); border-radius: 2px;
  animation: loadbar-slide 1.1s ease-in-out infinite;
}
@keyframes loadbar-slide {
  0% { transform: translateX(-120%); }
  100% { transform: translateX(350%); }
}
.error-banner {
  position: absolute; top: 12px; left: 50%; transform: translateX(-50%);
  z-index: 6; max-width: calc(100% - 32px);
  padding: 8px 16px; border-radius: 10px;
  background: var(--bg-glass-strong);
  backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
  border: 1px solid rgba(248, 113, 113, 0.3);
  color: #f87171; font-size: 13px;
}

/* ── Center states ── */
.center-state {
  height: 100%; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 12px;
  color: var(--text-faint); font-size: 14px;
}
.center-state.error { color: #f87171; }
.center-state .cs-icon { font-size: 40px; opacity: 0.4; }
.center-state .is-loading { animation: spin 1s linear infinite; font-size: 26px; color: var(--accent); }

/* ── TOC ── */
.toc-item {
  display: block;
  padding: 5px 8px;
  font-size: 12.5px; line-height: 1.5;
  color: var(--text-muted);
  cursor: pointer; text-decoration: none;
  border-left: 2px solid transparent;
  transition: color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.toc-item:hover { color: var(--text-secondary); background: var(--bg-glass-subtle); }
.toc-item.active { color: var(--accent); border-left-color: var(--accent); background: var(--accent-light); font-weight: 600; }
.toc-item .toc-label { display: inline; }
.toc-item .katex { font-size: 0.94em; }

/* ── Drawer inner ── */
.drawer-inner { padding: 12px 6px; }
.drawer-inner .pane-title { padding: 0 2px 8px; border-bottom: 1px solid var(--border-faint); margin-bottom: 4px; }
.reader-search-modal { min-height: 280px; max-height: min(72vh, 620px); display: flex; flex-direction: column; padding: 18px 20px 20px; }
.reader-search-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
.reader-search-header .pane-title { padding: 0; margin: 0 0 2px; border: 0; font-size: 17px; color: var(--text-primary); }
.reader-search-header small { display: block; max-width: 230px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-faint); font-size: 11px; }
.reader-search-close { width: 44px; height: 44px; display: grid; place-items: center; flex: none; padding: 0; border: 1px solid var(--border-subtle); border-radius: 13px; background: var(--bg-glass); color: var(--text-muted); font: inherit; font-size: 23px; }
.reader-file-search { height: 46px; display: flex; align-items: center; gap: 9px; padding: 0 12px; border-radius: 14px; color: var(--text-faint); }
.reader-file-search input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--text-primary); font: inherit; font-size: 16px; }
.reader-file-search input::-webkit-search-cancel-button { display: none; }
.reader-file-search input::placeholder { color: var(--text-faint); }
.reader-file-search button { width: 28px; height: 28px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 50%; background: var(--border-faint); color: var(--text-muted); font: inherit; }
.reader-search-count { margin: 12px 4px 7px; color: var(--text-faint); font-size: 12px; }
.reader-search-results { min-height: 0; display: grid; gap: 7px; margin: 0; padding: 2px; overflow: hidden auto; overscroll-behavior: contain; list-style: none; }
.reader-search-result { width: 100%; min-height: 58px; display: flex; align-items: center; gap: 10px; padding: 9px 10px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); color: var(--text-secondary); text-align: left; }
.reader-search-result.active { border-color: var(--accent-border); background: var(--accent-light); color: var(--accent); }
.reader-search-result:active { transform: scale(.98); }
.reader-search-kind { min-width: 34px; padding: 4px 5px; border-radius: 8px; background: var(--bg-glass-strong); color: var(--text-muted); font-size: 10px; font-weight: 750; text-align: center; }
.reader-search-copy { min-width: 0; display: grid; gap: 2px; }
.reader-search-copy strong, .reader-search-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.reader-search-copy strong { color: inherit; font-size: 14px; font-weight: 650; }
.reader-search-copy small { color: var(--text-faint); font-size: 11px; }
.reader-search-empty { min-height: 180px; display: grid; place-content: center; justify-items: center; gap: 10px; padding: 24px; color: var(--text-faint); font-size: 13px; text-align: center; }
.reader-search-empty .el-icon { font-size: 28px; opacity: .55; }

/* ── Mobile ── */
@media (max-width: 768px) {
  .reader-page {
    height: calc(100dvh - var(--mobile-header-height) - var(--safe-top) - 40px);
    gap: 6px;
  }
  .reader-page.is-read-view {
    height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 16px);
  }
  /* Tasks-Hub mobile pattern: the toolbar wraps and the switch takes its own row. */
  .reader-topbar { flex-wrap: wrap; padding: 7px; }
  .view-switch { width: 100%; }
  .view-switch button { min-height: 34px; }
  .shelf-search { order: 2; min-width: 0; flex: 1; min-height: 44px; }
  .shelf-search input { font-size: 16px; }
  .reader-shelf-add {
    order: 2;
    min-width: 78px;
    min-height: var(--tap-target);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 0 12px;
    border: 0;
    border-radius: 13px;
    background: var(--accent);
    color: white;
    font: inherit;
    font-size: 14px;
    font-weight: 650;
    box-shadow: 0 8px 22px color-mix(in srgb, var(--accent) 24%, transparent);
  }
  .reader-shelf-add:active { transform: scale(.96); }
  .reader-file-search-trigger,
  .reader-fullscreen-btn { display: none !important; }
  .path-trigger { min-height: var(--tap-target); padding-block: 8px; }
  .reader-topbar .icon-btn { width: 44px; height: 44px; }
  .pane-left, .pane-right { display: none; }
  .pane-center {
    width: 100%;
    overflow-x: clip;
    overscroll-behavior-y: contain;
    -webkit-overflow-scrolling: touch;
  }
  .markdown-body { padding: 12px 14px 100px; max-width: 100%; }
  .path-card { width: calc(100vw - 24px); }
  .path-card.is-file-search { max-height: min(76dvh, 620px); }
  .path-overlay { padding-top: 10vh; }
  .reader-search-modal { min-height: min(58dvh, 440px); max-height: min(76dvh, 620px); padding: 16px 14px calc(16px + var(--safe-bottom)); }
  .toc-item { min-height: var(--tap-target); display: flex; align-items: center; }
  .drawer-inner { padding-bottom: var(--safe-bottom); }
  .reader-mobile-toolbar {
    position: fixed;
    left: 50%;
    bottom: max(10px, var(--safe-bottom));
    z-index: 80;
    width: min(calc(100vw - 20px), 430px);
    display: grid;
    grid-template-columns: 108px minmax(0, 1fr) 108px;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border-glass);
    border-radius: 18px;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(22px) saturate(180%);
    -webkit-backdrop-filter: blur(22px) saturate(180%);
    box-shadow: var(--shadow-lg), var(--inset-highlight);
    max-width: calc(100vw - 20px);
    overflow: hidden;
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transform: translate3d(-50%, 12px, 0) scale(0.98);
    transition: opacity 180ms var(--ease-out),
                transform 260ms var(--ease-emphasized),
                visibility 0s linear 260ms;
    will-change: opacity, transform;
  }
  .reader-mobile-toolbar.is-visible {
    opacity: 1;
    visibility: visible;
    pointer-events: auto;
    transform: translate3d(-50%, 0, 0) scale(1);
    transition-delay: 0s;
  }
  .reader-mobile-toolbar button {
    min-width: 0;
    height: 52px;
    border: 0;
    border-radius: 13px;
    background: transparent;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .reader-mobile-toolbar button .el-icon { font-size: 24px; }
  .reader-mobile-toolbar button:active { transform: scale(0.94); background: var(--accent-light); }
  .mobile-toolbar-side {
    height: 52px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: center;
  }
  .mobile-toolbar-side button { width: 52px; justify-self: center; }
  .reader-document-center {
    width: 100%;
    min-height: 52px;
    display: grid !important;
    align-content: center;
    justify-items: center;
    gap: 1px;
    padding: 0 4px;
    color: var(--text-primary) !important;
  }
  .reader-document-label {
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: inherit;
    font-size: 12.5px;
    font-weight: 650;
    text-align: center;
  }
  .reader-document-center small { color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }

  .reader-page.is-mobile-immersive {
    position: fixed;
    inset: 0;
    z-index: 2100;
    width: 100%;
    height: 100dvh;
    padding: var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left);
    background: var(--bg-base);
  }
  .reader-page.is-mobile-immersive .page-header,
  .reader-page.is-mobile-immersive .reader-topbar { display: none !important; }
  .reader-page.is-mobile-immersive .reader-body { min-height: 0; }
  .reader-page.is-mobile-immersive .pane-center { border: 0; border-radius: 0; }
}

@media (max-width: 768px) and (prefers-reduced-motion: reduce) {
  .reader-mobile-toolbar {
    transform: translateX(-50%);
    transition: opacity 140ms ease-out, visibility 0s linear 140ms;
  }
  .reader-mobile-toolbar.is-visible { transform: translateX(-50%); }
}

@media (max-width: 768px) and (prefers-reduced-transparency: reduce) {
  .reader-mobile-toolbar {
    background: var(--bg-base);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}

@media (max-width: 768px) and (prefers-contrast: more) {
  .reader-mobile-toolbar {
    background: var(--bg-base);
    border-color: var(--text-muted);
  }
}
</style>

<!-- Global (non-scoped): markdown body + mermaid styling, themed via CSS variables -->
<style>
/* ── syntax-highlight token colors (theme-aware; no hljs theme CSS) ── */
:root[data-theme="light"] {
  --tk-text: #24292e;
  --tk-keyword: #d73a49;
  --tk-string: #032f62;
  --tk-number: #005cc5;
  --tk-comment: #6a737d;
  --tk-function: #6f42c1;
  --tk-builtin: #005cc5;
  --tk-variable: #e36209;
  --tk-tag: #22863a;
}
:root[data-theme="dark"] {
  --tk-text: #c9d1d9;
  --tk-keyword: #ff7b72;
  --tk-string: #a5d6ff;
  --tk-number: #79c0ff;
  --tk-comment: #8b949e;
  --tk-function: #d2a8ff;
  --tk-builtin: #79c0ff;
  --tk-variable: #ffa657;
  --tk-tag: #7ee787;
}
:root[data-theme="eye-care"] {
  --tk-text: #152618;
  --tk-keyword: #1b5e20;
  --tk-string: #33691e;
  --tk-number: #0d47a1;
  --tk-comment: #558b2f;
  --tk-function: #6a1b9a;
  --tk-builtin: #0d47a1;
  --tk-variable: #bf360c;
  --tk-tag: #2e7d32;
}

/* Reader drawers are mobile-only and teleported to body; drop the default 20px
   body padding so .drawer-inner controls the (compact) spacing. */
.el-drawer__body { padding: 0; }

/* Glass styling for the drawers — matches the app's liquid-glass surfaces. */
.el-drawer {
  background: var(--bg-glass-strong);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18), var(--inset-highlight);
}
:root[data-theme="dark"] .el-drawer {
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35), var(--inset-highlight);
}
/* Frosted overlay (only the drawer's overlay, not dialogs). */
.el-overlay:has(.el-drawer) {
  background-color: rgba(0, 0, 0, 0.18);
  backdrop-filter: blur(6px) saturate(140%);
  -webkit-backdrop-filter: blur(6px) saturate(140%);
}
:root[data-theme="dark"] .el-overlay:has(.el-drawer) {
  background-color: rgba(0, 0, 0, 0.4);
}

.markdown-body {
  width: 100%;
  min-width: 0;
  max-width: 860px;
  margin: 0 auto;
  padding: 12px 20px 120px;
  color: var(--text-secondary);
  font-size: 15px;
  line-height: 1.8;
  overflow-wrap: anywhere;
  word-break: normal;
}
.markdown-body > * { max-width: 100%; }
.markdown-body > *:first-child { margin-top: 0; }

.markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4, .markdown-body h5, .markdown-body h6 {
  color: var(--text-primary);
  font-weight: 650;
  line-height: 1.3;
  margin: 1.6em 0 0.7em;
  scroll-margin-top: 16px;
}
.markdown-body h1 {
  font-size: 1.9em; padding-bottom: 0.3em; border-bottom: 1px solid var(--border-faint);
}
.markdown-body h2 {
  font-size: 1.5em; padding-bottom: 0.25em; border-bottom: 1px solid var(--border-faint);
}
.markdown-body h3 { font-size: 1.25em; }
.markdown-body h4 { font-size: 1.05em; }
.markdown-body :is(h1, h2, h3, h4, h5, h6) .katex {
  font-size: 0.94em;
  line-height: 1;
}

.markdown-body p { margin: 0 0 1em; min-width: 0; }
.markdown-body a {
  color: var(--accent);
  text-decoration: none;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.markdown-body a:hover { text-decoration: underline; }
.markdown-body .internal-link {
  padding: 0.08em 0.18em;
  border-radius: 5px;
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  text-decoration-line: underline;
  text-decoration-style: dotted;
  text-decoration-color: color-mix(in srgb, var(--accent) 45%, transparent);
  text-underline-offset: 0.2em;
}
.markdown-body mark {
  padding: 0.06em 0.2em;
  border-radius: 4px;
  color: inherit;
  background: color-mix(in srgb, #f3c969 48%, transparent);
  box-decoration-break: clone;
  -webkit-box-decoration-break: clone;
}

.markdown-body ul, .markdown-body ol { padding-left: 1.6em; margin: 0 0 1em; }
.markdown-body li { margin: 0.3em 0; }
.markdown-body li > ul, .markdown-body li > ol { margin: 0.3em 0; }

.markdown-body blockquote {
  margin: 0 0 1em; padding: 0.4em 1em;
  border-left: 3px solid var(--accent-border);
  background: var(--bg-glass-subtle);
  border-radius: 0 8px 8px 0;
  color: var(--text-tertiary);
  min-width: 0;
  overflow-wrap: anywhere;
}
.markdown-body blockquote p { margin: 0.3em 0; }

.markdown-body .callout {
  --callout-color: var(--accent);
  width: 100%;
  margin: 1em 0;
  overflow: clip;
  border: 1px solid color-mix(in srgb, var(--callout-color) 24%, var(--border-faint));
  border-radius: 12px;
  background: color-mix(in srgb, var(--callout-color) 6%, var(--bg-glass-subtle));
}
.markdown-body .callout-warning,
.markdown-body .callout-caution,
.markdown-body .callout-attention { --callout-color: #e49b32; }
.markdown-body .callout-danger,
.markdown-body .callout-error,
.markdown-body .callout-failure { --callout-color: #dc625e; }
.markdown-body .callout-success,
.markdown-body .callout-check,
.markdown-body .callout-done { --callout-color: #42a477; }
.markdown-body .callout-question,
.markdown-body .callout-help,
.markdown-body .callout-faq { --callout-color: #9b75cf; }
.markdown-body .callout-title {
  display: flex;
  align-items: center;
  min-height: 40px;
  padding: 9px 13px;
  color: color-mix(in srgb, var(--callout-color) 82%, var(--text-primary));
  font-weight: 650;
  line-height: 1.45;
  background: color-mix(in srgb, var(--callout-color) 8%, transparent);
}
.markdown-body summary.callout-title {
  cursor: pointer;
  user-select: none;
}
.markdown-body .callout-content { padding: 11px 14px 12px; }
.markdown-body .callout-content > :last-child { margin-bottom: 0; }

.markdown-body code {
  font-family: var(--font-mono);
  font-size: 0.88em;
  background: var(--code-bg);
  color: var(--code-inline-color);
  padding: 0.15em 0.4em;
  border-radius: 5px;
}
.markdown-body :not(pre) > code {
  white-space: break-spaces;
  overflow-wrap: anywhere;
  word-break: break-word;
}
/* code block with line numbers (shared by reader + preview modal) */
.code-block {
  display: flex;
  width: 100%;
  max-width: 100%;
  min-width: 0;
  margin: 0 0 1em;
  background: var(--code-bg);
  border: 1px solid var(--border-faint);
  border-radius: 12px;
  overflow: hidden;
}
.code-block .code-gutter,
.code-block .code-content {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
}
.code-block .code-gutter {
  flex-shrink: 0;
  padding: 16px 12px;
  text-align: right;
  color: var(--text-faint);
  user-select: none;
  white-space: pre;
  border-right: 1px solid var(--border-faint);
  overflow: hidden;
}
.code-block .code-content {
  flex: 1;
  min-width: 0;
  padding: 16px 18px;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
  touch-action: pan-x pan-y;
  background: transparent;
  border: none;
}
.code-block .code-content code,
.code-block .code-content code.hljs {
  background: transparent; color: var(--tk-text);
  padding: 0; font-size: 13px; line-height: 1.6;
  font-family: var(--font-mono);
  white-space: pre;
  overflow-wrap: normal;
  word-break: normal;
}

/* Raw HTML <pre> blocks do not pass through the fenced-code renderer. */
.markdown-body pre:not(.code-gutter):not(.code-content) {
  width: 100%;
  max-width: 100%;
  min-width: 0;
  overflow: auto;
  padding: 16px 18px;
  border-radius: 12px;
  background: var(--code-bg);
  white-space: pre;
  overflow-wrap: normal;
  word-break: normal;
  -webkit-overflow-scrolling: touch;
}
.markdown-body pre code {
  white-space: inherit;
  overflow-wrap: normal;
  word-break: normal;
}
/* hljs token colors — themed via --tk-* variables (global: reader + modal) */
.hljs-comment,
.hljs-quote { color: var(--tk-comment); font-style: italic; }
.hljs-keyword,
.hljs-selector-tag,
.hljs-literal,
.hljs-section,
.hljs-link,
.hljs-deletion { color: var(--tk-keyword); }
.hljs-string,
.hljs-regexp,
.hljs-addition,
.hljs-attribute { color: var(--tk-string); }
.hljs-number,
.hljs-symbol,
.hljs-bullet,
.hljs-meta { color: var(--tk-number); }
.hljs-title,
.hljs-title.function_,
.hljs-title.class_,
.hljs-name { color: var(--tk-function); }
.hljs-built_in,
.hljs-type { color: var(--tk-builtin); }
.hljs-variable,
.hljs-template-variable,
.hljs-attr,
.hljs-property,
.hljs-params { color: var(--tk-variable); }
.hljs-tag { color: var(--tk-tag); }
.hljs-emphasis { font-style: italic; }
.hljs-strong { font-weight: 700; }

/* On mobile, keep code on a single line (no wrap) — scroll horizontally to view
   long lines. Only long prose/URLs wrap. */
@media (max-width: 768px) {
  .markdown-body { overflow-wrap: anywhere; }
}

.markdown-body .table-scroll {
  width: 100%;
  max-width: 100%;
  margin: 1em 0;
  overflow-x: auto;
  overflow-y: hidden;
  border: 1px solid var(--border-faint);
  border-radius: 12px;
  overscroll-behavior-inline: contain;
  -webkit-overflow-scrolling: touch;
  scrollbar-gutter: stable;
}
.markdown-body .table-scroll:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.markdown-body :is(.table-scroll, .code-content, .katex-display, .mermaid).is-overflowing {
  --overflow-cue: color-mix(in srgb, var(--text-primary) 20%, transparent);
}
.markdown-body :is(.table-scroll, .code-content, .katex-display, .mermaid)[data-overflow-position="start"] {
  box-shadow: inset -18px 0 16px -18px var(--overflow-cue);
}
.markdown-body :is(.table-scroll, .code-content, .katex-display, .mermaid)[data-overflow-position="middle"] {
  box-shadow: inset 18px 0 16px -18px var(--overflow-cue), inset -18px 0 16px -18px var(--overflow-cue);
}
.markdown-body :is(.table-scroll, .code-content, .katex-display, .mermaid)[data-overflow-position="end"] {
  box-shadow: inset 18px 0 16px -18px var(--overflow-cue);
}
.markdown-body table {
  width: max-content;
  min-width: 100%;
  max-width: none;
  border-collapse: separate;
  border-spacing: 0;
  margin: 0;
  font-size: 0.93em;
}
.markdown-body th, .markdown-body td {
  padding: 10px 16px; border: none;
  text-align: left;
  min-width: 8rem;
  max-width: 28rem;
  overflow-wrap: anywhere;
  word-break: normal;
  vertical-align: top;
}
.markdown-body th {
  background: var(--accent-light); color: var(--accent); font-weight: 600;
  font-size: 0.95em; letter-spacing: 0.02em;
}
.markdown-body th + th, .markdown-body td + td { border-left: 1px solid var(--border-faint); }
.markdown-body tbody tr + tr td { border-top: 1px solid var(--border-faint); }
.markdown-body tr:nth-child(even) td { background: var(--bg-glass-subtle); }

/* Photos arrive at camera resolution — cap them at a reading-comfortable size
   (never upscale small images) and center; the click-to-zoom viewer still
   shows full resolution. */
.markdown-body img {
  display: block;
  max-width: min(80%, 480px);
  height: auto;
  border-radius: 10px;
  margin: 0.75em auto;
}
.markdown-body video,
.markdown-body iframe,
.markdown-body canvas,
.markdown-body object,
.markdown-body embed,
.markdown-body svg {
  max-width: 100%;
}
.markdown-body video,
.markdown-body audio { width: 100%; }
.markdown-body iframe { border: 0; }
.markdown-body figure,
.markdown-body details { max-width: 100%; min-width: 0; }
.markdown-body hr { border: none; border-top: 1px solid var(--border-faint); margin: 2em 0; }

/* Display math is intentionally kept on one line and scrolled locally. */
.markdown-body .katex-display {
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 0.25em 0;
  -webkit-overflow-scrolling: touch;
}
.markdown-body .katex-display > .katex { min-width: max-content; }
.markdown-body :not(.katex-display) > .katex {
  display: inline-block;
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  vertical-align: middle;
}

/* task lists + footnotes */
.markdown-body input[type="checkbox"] {
  margin-right: 0.4em;
  transform: translateY(1px);
  accent-color: var(--accent);
}
.markdown-body [data-footnotes] {
  margin-top: 2.4em;
  padding-top: 0.8em;
  border-top: 1px solid var(--border-faint);
  color: var(--text-tertiary);
  font-size: 0.9em;
}
.markdown-body [data-footnote-ref] {
  margin-inline: 0.08em;
  font-size: 0.78em;
  font-weight: 650;
}
.markdown-body [data-footnote-backref] { margin-left: 0.3em; }

/* ── mermaid ── */
.markdown-body .mermaid {
  display: flex; justify-content: safe center;
  width: 100%; max-width: 100%; min-width: 0;
  margin: 1.2em 0; padding: 20px;
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 12px;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-inline: contain;
  -webkit-overflow-scrolling: touch;
}
.markdown-body .mermaid svg { display: block; max-width: 100%; height: auto; flex: 0 0 auto; }

@media (max-width: 768px) {
  .markdown-body th,
  .markdown-body td {
    min-width: min(8rem, 42vw);
    padding: 8px 12px;
  }
  .markdown-body ul,
  .markdown-body ol { padding-left: 1.35em; }
  /* 80% of a phone column is too small for photos — use the full width. */
  .markdown-body img { max-width: 100%; }
}
.markdown-body .mermaid-clickable { cursor: zoom-in; transition: box-shadow var(--duration-fast) var(--ease-out), transform var(--duration-fast) var(--ease-out); }
.markdown-body .mermaid-clickable:hover { box-shadow: var(--shadow-lg); transform: translateY(-1px); }
.markdown-body .mermaid-error {
  color: #f87171; font-size: 13px; justify-content: flex-start;
  font-family: var(--font-mono);
}
</style>
