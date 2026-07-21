<template>
  <div ref="readerPageRef" class="reader-page" :class="{ 'is-fullscreen': isFullscreen, 'fs-anim': fsAnim, 'fs-ui-hidden': isFullscreen && !showFsUI }">
    <header class="page-header">
      <div>
        <h1 class="page-title">阅境轩</h1>
        <p class="page-subtitle">浏览本地 Markdown，沉浸阅读</p>
      </div>
    </header>

    <!-- Compact trigger bar -->
    <div class="reader-topbar">
      <button class="path-trigger" @click="openHistoryOverlay">
        <el-icon><FolderOpened /></el-icon>
        <span v-if="currentFolderName" class="pt-name">{{ currentFolderName }}</span>
        <span v-if="rootPath" class="pt-path">{{ rootPath }}</span>
        <span v-if="!rootPath" class="pt-hint">输入本地文件夹路径</span>
      </button>
      <el-button class="icon-btn" :title="isFullscreen ? '退出全屏 (Esc)' : '全屏阅读'" @click="toggleFullscreen">
        <el-icon><FullScreen /></el-icon>
      </el-button>
    </div>

    <!-- Floating path overlay (command-palette style) -->
    <transition name="overlay-fade">
      <div v-if="showHistory" class="path-overlay" @click.self="showHistory = false">
        <transition name="overlay-pop" appear>
          <div v-if="showHistory" class="path-card">
            <div class="path-input-wrap">
              <el-input
                ref="pathInputRef"
                v-model="pathInput"
                class="path-input"
                placeholder="输入本地文件夹路径，如 /Users/.../docs"
                clearable
                size="large"
                @keyup.enter="openPath()"
                @keydown.escape="showHistory = false"
              >
                <template #prefix><el-icon><FolderOpened /></el-icon></template>
              </el-input>
            </div>

            <div class="history-panel">
              <div class="hp-head">
                <span>历史记录</span>
                <button v-if="history.length" class="hp-clear" @click="clearHistory">清空</button>
              </div>
              <div class="hp-list">
                <div v-if="!history.length" class="hp-empty">暂无历史记录</div>
                <div v-else-if="!filteredHistory.length" class="hp-empty">无匹配记录</div>
                <div v-for="h in filteredHistory" :key="h.path" class="hp-row" :class="{ pinned: h.pinned }">
                  <div class="hp-item">
                    <button class="hp-pin" :title="h.pinned ? '取消置顶' : '置顶'" @click="togglePin(h.path)">
                      <el-icon><StarFilled v-if="h.pinned" /><Star v-else /></el-icon>
                    </button>
                    <div class="hp-info" @click="useHistory(h.path)">
                      <span v-if="h.name" class="hp-name">{{ h.name }}</span>
                      <span class="hp-path" :title="h.path">{{ h.path }}</span>
                    </div>
                    <button class="hp-edit" title="命名" @click.stop="startRename(h)">
                      <el-icon><EditPen /></el-icon>
                    </button>
                    <button class="hp-del" title="删除" @click="removeHistory(h.path)">
                      <el-icon><Delete /></el-icon>
                    </button>
                  </div>
                  <div v-if="renamingPath === h.path" class="hp-rename">
                    <input
                      v-model="renameValue"
                      class="hp-rename-input"
                      placeholder="输入名称（留空清除名称）"
                      @keyup.enter="confirmRename(h)"
                      @keydown.escape="cancelRename"
                    />
                    <button class="hp-rename-ok" @click="confirmRename(h)">确定</button>
                    <button class="hp-rename-cancel" @click="cancelRename">取消</button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </transition>
      </div>
    </transition>

    <!-- Mobile floating buttons (file / TOC) — fade out while scrolling -->
    <button
      class="fab fab-left"
      :class="{ hide: fabHide }"
      title="文件"
      @click="treeDrawer = true"
    ><el-icon :size="24"><FolderOpened /></el-icon></button>
    <button
      class="fab fab-right"
      :class="{ hide: fabHide }"
      title="目录"
      @click="tocDrawer = true"
    ><el-icon :size="24"><Menu /></el-icon></button>

    <!-- Body: 3 panes -->
    <div class="reader-body">
      <!-- Left: file tree -->
      <aside class="pane pane-left">
        <div class="pane-title">文件</div>
        <div class="pane-scroll">
          <FileTree v-if="tree.length" :entries="tree" :active-path="activeFile" @select="onSelectFile" />
          <div v-else class="pane-hint">打开一个文件夹后在此浏览</div>
        </div>
      </aside>

      <!-- Center: rendered content (page-turn transition on file switch) -->
      <main ref="contentRef" class="pane pane-center" @scroll="onContentScroll">
        <transition :name="transitionDir" mode="out-in" @enter="onArticleEnter">
          <article
            v-if="renderedHtml"
            :key="displayedFile"
            class="markdown-body"
            v-html="renderedHtml"
          ></article>
          <div v-else key="empty" class="center-state">
            <el-icon class="cs-icon"><Document /></el-icon>
            <p>选择左侧的 Markdown 文件开始阅读</p>
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
            @click="scrollToHeading(t.id)"
          >{{ t.text }}</a>
        </div>
      </aside>
    </div>

    <!-- Mobile drawers -->
    <el-drawer v-model="treeDrawer" direction="ltr" size="70%" :with-header="false">
      <div class="drawer-inner">
        <div class="pane-title">文件</div>
        <FileTree
          v-if="tree.length"
          :entries="tree"
          :active-path="activeFile"
          @select="(p) => { onSelectFile(p); treeDrawer = false }"
        />
        <div v-else class="pane-hint">打开一个文件夹后在此浏览</div>
      </div>
    </el-drawer>
    <el-drawer v-model="tocDrawer" direction="rtl" size="70%" :with-header="false">
      <div class="drawer-inner">
        <div class="pane-title">目录</div>
        <a
          v-for="t in toc"
          :key="t.id"
          class="toc-item"
          :class="{ active: activeHeading === t.id }"
          :style="{ paddingLeft: 8 + (t.level - 1) * 12 + 'px' }"
          @click="scrollToHeading(t.id); tocDrawer = false"
        >{{ t.text }}</a>
        <div v-if="!toc.length" class="pane-hint">无目录</div>
      </div>
    </el-drawer>

    <!-- Floating fullscreen UI (auto-hides) -->
    <div v-if="isFullscreen" class="fs-ui" :class="{ hidden: !showFsUI }">
      <button class="fs-fab" title="退出全屏 (Esc)" @click="toggleFullscreen">
        <el-icon :size="20"><FullScreen /></el-icon>
      </button>
    </div>

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
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
import { ElMessage } from 'element-plus'
import {
  FolderOpened, Star, StarFilled, Delete, Menu, Document, FullScreen, EditPen,
} from '@element-plus/icons-vue'
import {
  listLocalDir, readLocalFile, getReaderHistory, saveReaderHistory,
  type DirEntry, type HistoryItem,
} from '@/api/reader'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { useAppStore } from '@/stores/app'
import FileTree from '@/components/reader/FileTree.vue'
import MermaidViewer from '@/components/reader/MermaidViewer.vue'
import PathPreviewModal from '@/components/reader/PathPreviewModal.vue'

const appStore = useAppStore()

interface TocItem { id: string; text: string; level: number }

// Per-browser "last session" (folder + file to reopen). History itself is server-stored.
const LAST_FOLDER_KEY = 'reader.lastFolder'
const LAST_FILE_KEY = 'reader.lastFile'

const pathInput = ref('')
const tree = ref<DirEntry[]>([])
const rootPath = ref('')          // the currently opened folder (absolute)
const activeFile = ref('')
const loading = ref(false)
const fileLoading = ref(false)
const renderedHtml = ref('')
const error = ref('')
const toc = ref<TocItem[]>([])
const activeHeading = ref('')
const showHistory = ref(false)
const renamingPath = ref('')
const renameValue = ref('')
const pathInputRef = ref<{ focus: () => void } | null>(null)

// Auto-focus the input when the overlay opens.
watch(showHistory, (v) => {
  if (v) nextTick(() => pathInputRef.value?.focus())
})
const history = ref<HistoryItem[]>([])
const treeDrawer = ref(false)
const tocDrawer = ref(false)
const viewerSvg = ref('')
const viewerTitle = ref('Mermaid 图')
const previewPath = ref('')        // non-md / out-of-folder link target → popup
const previewAnchor = ref('')      // line/symbol/heading anchor for the popup target
const viewerSource = ref('')
const contentRef = ref<HTMLElement | null>(null)
const readerPageRef = ref<HTMLElement | null>(null)
const isFullscreen = ref(false)
const fsAnim = ref(false)
let fsAnimTimer: ReturnType<typeof setTimeout> | null = null
// Auto-hide UI in fullscreen for immersive reading.
const showFsUI = ref(true)
let fsUiTimer: ReturnType<typeof setTimeout> | null = null

function onFsActivity() {
  showFsUI.value = true
  if (fsUiTimer) clearTimeout(fsUiTimer)
  fsUiTimer = setTimeout(() => { showFsUI.value = false }, 3000)
}

function toggleFullscreen() {
  const el = readerPageRef.value
  if (document.fullscreenElement) {
    void document.exitFullscreen()
  } else if (el) {
    el.requestFullscreen().catch((e) => {
      console.warn('进入全屏失败:', e)
      ElMessage.warning('当前浏览器不支持全屏')
    })
  }
}
function onFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
  // Replay the pop animation on every toggle (enter & exit).
  fsAnim.value = false
  if (fsAnimTimer) clearTimeout(fsAnimTimer)
  requestAnimationFrame(() => {
    fsAnim.value = true
    fsAnimTimer = setTimeout(() => { fsAnim.value = false }, 400)
  })
  // Auto-hide UI listeners — only active in fullscreen.
  if (isFullscreen.value) {
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
const transitionDir = ref<'page-next' | 'page-prev'>('page-next')

// Markdown paths in tree display order (depth-first) — used to pick turn direction.
// Show the history name for the currently open folder (if any).
const currentFolderName = computed(() => {
  if (!rootPath.value) return ''
  return history.value.find((h) => h.path === rootPath.value)?.name || ''
})

const flatFiles = computed(() => {
  const out: string[] = []
  const walk = (entries: DirEntry[]) => {
    for (const e of entries) {
      if (e.is_dir) e.children && walk(e.children)
      else if (e.is_markdown) out.push(e.path)
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
function resolveRelative(baseDir: string, rel: string): string {
  const parts = baseDir ? baseDir.split('/') : []
  for (const seg of rel.replace(/^\.\//, '').split('/')) {
    if (seg === '' || seg === '.') continue
    if (seg === '..') parts.pop()
    else parts.push(seg)
  }
  return parts.join('/')
}

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
  const resolved = resolveRelative(baseDir, decodeURIComponent(pathPartRaw))
  if (!resolved) return

  // Markdown under the opened folder → jump in the reader (page-turn).
  if (/\.(md|markdown)$/i.test(resolved) && isUnderRoot(resolved)) {
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

const { renderMarkdown, enhance } = useMarkdownRender(handleMermaidClick, handleLinkClick, handleImageClick)

// ── history (server-stored, shared across all users) ──────────────────
async function loadHistory() {
  try {
    const res = await getReaderHistory()
    if (res.status === 'success' && res.result) history.value = res.result.history
  } catch (e) {
    console.error('加载历史失败:', e)
  }
  sortHistory()
}
async function saveHistory() {
  try {
    await saveReaderHistory(history.value)
  } catch (e) {
    console.error('保存历史失败:', e)
  }
}
function sortHistory() {
  history.value.sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1
    return b.lastUsed - a.lastUsed
  })
}
function addHistory(path: string) {
  const existing = history.value.find((h) => h.path === path)
  if (existing) existing.lastUsed = Date.now()
  else history.value.push({ path, pinned: false, lastUsed: Date.now() })
  sortHistory()
  void saveHistory()
}
function togglePin(path: string) {
  const h = history.value.find((x) => x.path === path)
  if (h) { h.pinned = !h.pinned; sortHistory(); void saveHistory() }
}
function removeHistory(path: string) {
  history.value = history.value.filter((h) => h.path !== path)
  void saveHistory()
}
function startRename(h: HistoryItem) {
  renamingPath.value = h.path
  renameValue.value = h.name || ''
}
function confirmRename(h: HistoryItem) {
  h.name = renameValue.value.trim() || undefined
  renamingPath.value = ''
  void saveHistory()
}
function cancelRename() {
  renamingPath.value = ''
}
function clearHistory() {
  history.value = []
  void saveHistory()
}
function openHistoryOverlay() {
  pathInput.value = ''
  showHistory.value = true
}
function useHistory(path: string) {
  pathInput.value = path
  showHistory.value = false
  openPath(path)
}

// History filtered by the current input text (shown in the focus dropdown).
const filteredHistory = computed(() => {
  const q = pathInput.value.trim().toLowerCase()
  if (!q) return history.value
  return history.value.filter((h) => h.path.toLowerCase().includes(q))
})

// ── open folder ───────────────────────────────────────────────────────
async function openPath(p?: string) {
  const path = (p ?? pathInput.value).trim()
  if (!path) return
  pathInput.value = path
  loading.value = true
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
    // Remember last opened folder; clear stale last-file until a new one is chosen.
    localStorage.setItem(LAST_FOLDER_KEY, path)
    localStorage.removeItem(LAST_FILE_KEY)
    addHistory(path)
    showHistory.value = false
  } catch (e) {
    error.value = (e as Error)?.message || '打开失败'
    ElMessage.error(error.value)
  } finally {
    loading.value = false
  }
}

// ── select & render file ──────────────────────────────────────────────
async function onSelectFile(path: string) {
  activeFile.value = path
  // Already displaying this file — skip to avoid resetting rendered mermaid back to source.
  if (path === displayedFile.value) return
  fileLoading.value = true
  error.value = ''
  try {
    const res = await readLocalFile(path)
    if (res.status === 'error' || !res.result) {
      error.value = res.error?.message || '读取失败'
      ElMessage.error(error.value)
      return
    }
    // Determine page-turn direction from the file's position in the tree.
    const oldIdx = flatFiles.value.indexOf(displayedFile.value)
    const newIdx = flatFiles.value.indexOf(path)
    transitionDir.value =
      oldIdx >= 0 && newIdx >= 0 && newIdx < oldIdx ? 'page-prev' : 'page-next'
    // Render new content, then swap the transition key in the SAME tick so the leaving
    // <article> stays frozen on the OLD content while the new one slides in.
    renderedHtml.value = renderMarkdown(res.result.content)
    displayedFile.value = path
    localStorage.setItem(LAST_FILE_KEY, path)
    // enhance() + buildToc() run in the transition's @enter hook (onArticleEnter).
  } catch (e) {
    error.value = (e as Error)?.message || '读取失败'
  } finally {
    fileLoading.value = false
  }
}

/** Runs when a new <article> enters the page-turn transition: highlight, mermaid, TOC. */
async function onArticleEnter(el: Element) {
  // Only the markdown article needs enhancing (the empty-state div also passes through).
  if (!el.classList.contains('markdown-body')) return
  // Don't reset scroll if we're jumping to a cross-file anchor.
  if (contentRef.value && !pendingAnchor.value) contentRef.value.scrollTop = 0
  buildToc()
  await enhance(el as HTMLElement)
  if (pendingAnchor.value) {
    scrollToHeading(pendingAnchor.value)
    pendingAnchor.value = ''
  }
}

function buildToc() {
  const body = contentRef.value?.querySelector('.markdown-body')
  if (!body) { toc.value = []; return }
  const els = Array.from(body.querySelectorAll<HTMLElement>('h1,h2,h3,h4'))
  toc.value = els
    .filter((el) => el.id)
    .map((el) => ({ id: el.id, text: el.textContent || '', level: Number(el.tagName[1]) }))
}

// Mobile FABs fade out while scrolling, fade back in after a short idle.
const fabHide = ref(false)
let fabHideTimer: ReturnType<typeof setTimeout> | null = null
function updateFabOnScroll() {
  if (!fabHide.value) fabHide.value = true
  if (fabHideTimer) clearTimeout(fabHideTimer)
  fabHideTimer = setTimeout(() => { fabHide.value = false }, 600)
}

function onContentScroll() {
  updateFabOnScroll()
  // Drive the app's mobile header + page-header collapse from the pane-center
  // scroll (app-main doesn't scroll on the Reader page).
  if (contentRef.value) appStore.setScrolled(contentRef.value.scrollTop > 20)
  if (!contentRef.value || !toc.value.length) return
  const containerTop = contentRef.value.getBoundingClientRect().top
  let current = ''
  for (const t of toc.value) {
    const el = document.getElementById(t.id)
    if (!el) continue
    const top = el.getBoundingClientRect().top - containerTop
    if (top <= 90) current = t.id
    else break
  }
  activeHeading.value = current
}

function scrollToHeading(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

onMounted(async () => {
  document.addEventListener('fullscreenchange', onFullscreenChange)
  await loadHistory()
  // Restore last opened folder + file (per-browser).
  const lastFolder = localStorage.getItem(LAST_FOLDER_KEY)
  const lastFile = localStorage.getItem(LAST_FILE_KEY)
  if (lastFolder) {
    await openPath(lastFolder)
    if (lastFile) await onSelectFile(lastFile)
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  if (document.fullscreenElement) void document.exitFullscreen()
})
</script>

<style scoped>
.reader-page {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 64px);
  gap: 10px;
}
/* Tighter page-header spacing — the Reader is a tool page, not a content page. */
.reader-page .page-header { margin-bottom: 6px; }
/* Fullscreen reading mode — immersive: hide app chrome (title/input), keep file
   tree + TOC. The article's H1 sticks to the top with a frosted glass bar. */
.reader-page:fullscreen {
  height: 100vh;
  width: 100%;
  background:
    radial-gradient(ellipse at 75% 15%, rgba(196, 181, 253, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(165, 243, 252, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%),
    var(--bg-base);
  border-radius: 0;
  overflow: hidden;
  cursor: default;
}
.reader-page:fullscreen.fs-ui-hidden { cursor: none; }
/* Hide only the page title + input bar; keep file tree + TOC + FABs. */
.reader-page:fullscreen .page-header,
.reader-page:fullscreen .reader-topbar { display: none !important; }
.reader-page:fullscreen .pane-center {
  display: block;
  background: transparent; border: none;
  backdrop-filter: none; -webkit-backdrop-filter: none;
  box-shadow: none; border-radius: 0;
}
/* Desktop fullscreen: wider document, more breathing room, H1 sticky. */
@media (min-width: 769px) {
  .reader-page:fullscreen {
    padding: 16px 24px;
  }
  .reader-page:fullscreen .markdown-body {
    max-width: 920px;
    padding: 32px 48px 160px;
    font-size: 16px;
    line-height: var(--leading-relaxed);
  }
  .reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
    position: sticky;
    top: 0;
    z-index: 10;
    margin: 0 -48px 24px;
    padding: 16px 48px;
    background: var(--bg-glass);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    box-shadow: 0 1px 0 var(--border-faint);
    border-radius: 12px;
  }
}
/* Article H1 sticks to the top of the scroll area with a frosted glass bar —
   content scrolls behind it, visible (blurred) through the glass.
   :deep() needed because h1 is v-html content (no data-v-xxx attribute). */
.reader-page:fullscreen .markdown-body :deep(h1:first-of-type) {
  position: sticky;
  top: 0;
  z-index: 10;
  margin: 0 -48px 24px;
  padding: 16px 48px;
  background: var(--bg-glass);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  box-shadow: 0 1px 0 var(--border-faint);
  border-radius: 12px;
}

/* Floating fullscreen UI — auto-hides after inactivity. */
.fs-ui {
  position: fixed; top: 24px; right: 24px; z-index: 100;
  display: flex; gap: 10px;
  transition: opacity 0.4s var(--ease-out);
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
/* Backdrop behind the fullscreen element — match the bg so the pop animation's
   scale gap is seamless instead of flashing black. */
.reader-page::backdrop {
  background:
    radial-gradient(ellipse at 75% 15%, rgba(196, 181, 253, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(165, 243, 252, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%),
    var(--bg-base);
}
/* Pop animation on enter/exit fullscreen. */
.reader-page.fs-anim {
  animation: fs-pop 0.38s var(--ease-spring);
}
@keyframes fs-pop {
  0% { transform: scale(0.94); opacity: 0.45; }
  100% { transform: scale(1); opacity: 1; }
}

/* ── Top bar ── */
.reader-topbar {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-shrink: 0;
}
.path-trigger {
  flex: 1; display: flex; align-items: center; gap: 8px;
  padding: 10px 16px; border-radius: 14px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  color: var(--text-muted); font-size: 14px;
  cursor: pointer; text-align: left;
  transition: all 0.2s var(--ease-out);
  overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
}
.path-trigger:hover { background: var(--bg-hover); color: var(--text-secondary); border-color: var(--accent-border); }
.pt-name { font-weight: 600; color: var(--text-primary); flex-shrink: 0; }
.pt-path { color: var(--text-faint); font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; }
.pt-hint { color: var(--text-faint); }
.path-trigger:hover { background: var(--bg-hover); color: var(--text-secondary); border-color: var(--accent-border); }
.icon-btn { flex-shrink: 0; }

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
.path-input-wrap { padding: 16px 20px; border-bottom: 1px solid var(--border-faint); }
.path-input { width: 100%; }
.path-input :deep(.el-input__wrapper) { padding-left: 12px; }

.history-panel {
  max-height: 400px;
  display: flex; flex-direction: column;
  overflow: hidden;
}

.hp-head {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 16px; border-bottom: 1px solid var(--border-faint);
  font-size: 13px; font-weight: 600; color: var(--text-secondary);
}
.hp-clear {
  background: none; border: none; color: var(--text-muted);
  font-size: 12px; cursor: pointer; padding: 2px 6px; border-radius: 6px;
}
.hp-clear:hover { color: #f87171; background: rgba(248, 113, 113, 0.1); }
.hp-list { overflow-y: auto; padding: 6px; }
.hp-empty { padding: 24px; text-align: center; font-size: 12px; color: var(--text-faint); }
.hp-row { border-radius: 10px; transition: background 0.12s var(--ease-out); }
.hp-row:hover { background: var(--bg-glass-subtle); }
.hp-row.pinned { background: var(--accent-light); }
.hp-item {
  display: flex; align-items: center; gap: 8px;
  padding: 7px 10px;
}
.hp-pin, .hp-del, .hp-edit {
  flex-shrink: 0; width: 26px; height: 26px; border-radius: 8px;
  border: none; background: transparent; color: var(--text-muted);
  cursor: pointer; display: flex; align-items: center; justify-content: center;
}
.hp-pin:hover { color: var(--accent); background: var(--bg-glass-subtle); }
.hp-item.pinned .hp-pin { color: var(--accent); }
.hp-edit:hover { color: var(--accent); background: var(--bg-glass-subtle); }
.hp-del:hover { color: #f87171; background: rgba(248, 113, 113, 0.1); }
.hp-info {
  flex: 1; min-width: 0; cursor: pointer; display: flex; flex-direction: column; gap: 1px;
}
.hp-info:hover .hp-name { color: var(--accent); }
.hp-info:hover .hp-path { color: var(--accent); }
.hp-name {
  font-size: 13px; font-weight: 500; color: var(--text-primary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.hp-path {
  font-size: 11.5px; color: var(--text-faint);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.hp-rename {
  display: flex; align-items: center; gap: 8px;
  padding: 0 10px 10px 44px;
}
.hp-rename-input {
  flex: 1; height: 32px; padding: 0 12px; border-radius: 8px;
  border: 1px solid var(--accent-border);
  background: var(--bg-glass-subtle);
  color: var(--text-primary); font-size: 13px;
  outline: none;
}
.hp-rename-input:focus { border-color: var(--accent); }
.hp-rename-ok, .hp-rename-cancel {
  flex-shrink: 0; padding: 5px 12px; border-radius: 8px; border: none;
  font-size: 12px; cursor: pointer; transition: all 0.15s var(--ease-out);
}
.hp-rename-ok { background: var(--accent); color: #fff; }
.hp-rename-ok:hover { opacity: 0.85; }
.hp-rename-cancel { background: var(--bg-glass-subtle); color: var(--text-muted); }
.hp-rename-cancel:hover { color: var(--text-secondary); }

/* Overlay transitions */
.overlay-fade-enter-active, .overlay-fade-leave-active { transition: opacity 0.25s var(--ease-out); }
.overlay-fade-enter-from, .overlay-fade-leave-to { opacity: 0; }
.overlay-pop-enter-active { transition: opacity 0.3s var(--ease-spring), transform 0.3s var(--ease-spring); }
.overlay-pop-leave-active { transition: opacity 0.15s var(--ease-out), transform 0.15s var(--ease-out); }
.overlay-pop-enter-from { opacity: 0; transform: scale(0.96) translateY(-12px); }
.overlay-pop-leave-to { opacity: 0; transform: scale(0.98) translateY(-8px); }

/* ── Mobile floating buttons (file / TOC) ── */
.fab {
  display: none;
  position: fixed;
  bottom: 42%;
  width: 54px;
  height: 54px;
  border-radius: 50%;
  border: none;
  background: transparent;
  backdrop-filter: blur(12px) saturate(150%);
  -webkit-backdrop-filter: blur(12px) saturate(150%);
  color: var(--text-primary);
  align-items: center;
  justify-content: center;
  cursor: pointer;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  z-index: 60;
  opacity: 1;
  transform: scale(1);
  transition: opacity var(--duration-normal) var(--ease-out), transform var(--duration-normal) var(--ease-spring);
}
.fab:active { transform: scale(0.9); }
.fab.hide { opacity: 0; transform: scale(0.8); pointer-events: none; }
.fab-left { left: 14px; }
.fab-right { right: 14px; }


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
  transition: transform 0.28s var(--ease-standard), opacity 0.28s var(--ease-out);
}
.page-next-enter-from { transform: translateX(36px); opacity: 0; }
.page-next-leave-to { transform: translateX(-36px); opacity: 0; }
.page-prev-enter-from { transform: translateX(-36px); opacity: 0; }
.page-prev-leave-to { transform: translateX(36px); opacity: 0; }

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
  transition: all 0.12s var(--ease-out);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.toc-item:hover { color: var(--text-secondary); background: var(--bg-glass-subtle); }
.toc-item.active { color: var(--accent); border-left-color: var(--accent); background: var(--accent-light); font-weight: 600; }

/* ── Drawer inner ── */
.drawer-inner { padding: 12px 6px; }
.drawer-inner .pane-title { padding: 0 2px 8px; border-bottom: 1px solid var(--border-faint); margin-bottom: 4px; }

/* ── Mobile ── */
@media (max-width: 768px) {
  /* Extend the reader-page up under the global header (cancel app-main's 56px top
     padding) and pad its content down so the title/input sit below the header.
     On scroll the top padding collapses so the document slides under the frosted
     global header — same effect as Timeline. */
  .reader-page {
    height: calc(100vh - 40px);
    margin-top: -60px;
    padding-top: 60px;
    gap: 6px;
    transition: padding-top var(--duration-slow) var(--ease-standard);
  }
  .app-main.mobile-scrolled .reader-page { padding-top: 0; }
  .fab { display: flex; }
  .pane-left, .pane-right { display: none; }
  .pane-center { width: 100%; overflow-x: clip; }
  .markdown-body { padding: 12px 14px 100px; max-width: 100%; }
  .history-panel { max-height: 50vh; }
  .path-card { width: calc(100vw - 24px); }
  .path-overlay { padding-top: 10vh; }
  /* Collapse the topbar trigger on scroll, same as the page-header. */
  .reader-topbar { max-height: 60px; transition: max-height var(--duration-slow) var(--ease-standard), opacity var(--duration-normal) var(--ease-out), margin var(--duration-slow); }
  .app-main.mobile-scrolled .reader-topbar {
    max-height: 0; opacity: 0; margin: 0; overflow: hidden; pointer-events: none;
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
  max-width: 860px;
  margin: 0 auto;
  padding: 12px 20px 120px;
  color: var(--text-secondary);
  font-size: 15px;
  line-height: 1.8;
  word-wrap: break-word;
}
.markdown-body > *:first-child { margin-top: 0; }

.markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4, .markdown-body h5, .markdown-body h6 {
  color: var(--text-primary);
  font-weight: 650;
  line-height: 1.3;
  margin: 1.6em 0 0.7em;
  scroll-margin-top: 16px;
}
.markdown-body h1 { font-size: 1.9em; padding-bottom: 0.3em; border-bottom: 1px solid var(--border-faint); }
.markdown-body h2 { font-size: 1.5em; padding-bottom: 0.25em; border-bottom: 1px solid var(--border-faint); }
.markdown-body h3 { font-size: 1.25em; }
.markdown-body h4 { font-size: 1.05em; }

.markdown-body p { margin: 0 0 1em; }
.markdown-body a { color: var(--accent); text-decoration: none; }
.markdown-body a:hover { text-decoration: underline; }

.markdown-body ul, .markdown-body ol { padding-left: 1.6em; margin: 0 0 1em; }
.markdown-body li { margin: 0.3em 0; }
.markdown-body li > ul, .markdown-body li > ol { margin: 0.3em 0; }

.markdown-body blockquote {
  margin: 0 0 1em; padding: 0.4em 1em;
  border-left: 3px solid var(--accent-border);
  background: var(--bg-glass-subtle);
  border-radius: 0 8px 8px 0;
  color: var(--text-tertiary);
}
.markdown-body blockquote p { margin: 0.3em 0; }

.markdown-body code {
  font-family: var(--font-mono);
  font-size: 0.88em;
  background: var(--code-bg);
  color: var(--code-inline-color);
  padding: 0.15em 0.4em;
  border-radius: 5px;
}
/* code block with line numbers (shared by reader + preview modal) */
.code-block {
  display: flex;
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

.markdown-body table {
  width: 100%; border-collapse: collapse; margin: 0 0 1em;
  font-size: 0.93em; display: block; overflow-x: auto;
}
.markdown-body th, .markdown-body td {
  padding: 8px 12px; border: 1px solid var(--border-faint);
  text-align: left;
}
.markdown-body th { background: var(--bg-glass-subtle); color: var(--text-primary); font-weight: 600; }
.markdown-body tr:nth-child(even) td { background: var(--bg-glass-subtle); }

.markdown-body img { max-width: 100%; border-radius: 10px; margin: 0.5em 0; }
.markdown-body hr { border: none; border-top: 1px solid var(--border-faint); margin: 2em 0; }

/* task lists */
.markdown-body input[type="checkbox"] { margin-right: 0.4em; transform: translateY(1px); }

/* ── mermaid ── */
.markdown-body .mermaid {
  display: flex; justify-content: center;
  margin: 1.2em 0; padding: 20px;
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 12px;
  overflow: auto;
}
.markdown-body .mermaid svg { max-width: 100%; height: auto; }
.markdown-body .mermaid-clickable { cursor: zoom-in; transition: box-shadow var(--duration-fast) var(--ease-out), transform var(--duration-fast) var(--ease-out); }
.markdown-body .mermaid-clickable:hover { box-shadow: var(--shadow-lg); transform: translateY(-1px); }
.markdown-body .mermaid-error {
  color: #f87171; font-size: 13px; justify-content: flex-start;
  font-family: var(--font-mono);
}
</style>
