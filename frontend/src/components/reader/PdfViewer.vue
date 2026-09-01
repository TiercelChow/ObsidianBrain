<template>
  <div ref="scrollRef" class="pdf-viewer" :class="`pdf-theme-${theme}`">
    <div v-if="loading" class="pdf-state">
      <el-icon class="is-loading"><Loading /></el-icon><span>PDF 加载中…</span>
    </div>
    <div v-else-if="error" class="pdf-state error">⚠️ {{ error }}</div>
    <div v-else class="pdf-pages">
      <div
        v-for="p in pageMetas"
        :key="p.num"
        class="pdf-page-wrap"
        :data-page-num="p.num"
        :style="{ width: p.width + 'px', height: p.height + 'px' }"
      >
        <canvas :ref="(el) => setCanvasRef(p.num, el as HTMLCanvasElement | null)" class="pdf-canvas"></canvas>
        <div :ref="(el) => setTextRef(p.num, el as HTMLDivElement | null)" class="pdf-text-layer"></div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import * as pdfjsLib from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { useAppStore } from '@/stores/app'
import { localFileUrl } from '@/api/reader'
import {
  computePdfZoomScale,
  computeRenderDpr,
  isWithinRenderWindow,
} from './pdfRenderPolicy'
import { getPdfRenderPolicy } from '@/utils/mobileLayoutPolicy'

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

const props = defineProps<{ src: string }>()
const emit = defineEmits<{
  outline: [items: { text: string; level: number; page: number }[]]
  pagechange: [page: number]
  pagecount: [pages: number]
}>()
const appStore = useAppStore()
const theme = ref(appStore.theme)

interface PageMeta { num: number; width: number; height: number }
interface PageWork {
  generation: number
  status: 'queued' | 'rendering' | 'rendered'
  page?: pdfjsLib.PDFPageProxy
  renderTask?: pdfjsLib.RenderTask
  textLayer?: pdfjsLib.TextLayer
  textTimer?: number
  textRendering?: boolean
  textRendered?: boolean
}

const renderPolicy = getPdfRenderPolicy(
  window.innerWidth,
  navigator.hardwareConcurrency,
)
const RENDER_MARGIN_PX = renderPolicy.renderMarginPx
const MAX_CONCURRENT_RENDERS = renderPolicy.maxConcurrentRenders
const RANGE_CHUNK_SIZE = 256 * 1024

const scrollRef = ref<HTMLElement | null>(null)
const loading = ref(true)
const error = ref('')
const pageMetas = ref<PageMeta[]>([])
const canvasRefs: Record<number, HTMLCanvasElement | null> = {}
const textRefs: Record<number, HTMLDivElement | null> = {}
let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null
let loadingTask: pdfjsLib.PDFDocumentLoadingTask | null = null
let fitScale = 1 // stable fit-width baseline used by toolbar zoom ratios
let renderObserver: IntersectionObserver | null = null
let visibleObserver: IntersectionObserver | null = null
let resizeObserver: ResizeObserver | null = null
let resizeTimer: number | null = null
let loadGeneration = 0
let activeRenders = 0
let renderQueue: number[] = []
let unmounted = false
const nearbyPages = new Set<number>()
const visiblePages = new Set<number>()
const pageWork = new Map<number, PageWork>()
const zoomMode = ref<'fit' | number>('fit') // 'fit' = fit-width; number = explicit scale factor

function setCanvasRef(num: number, el: HTMLCanvasElement | null) {
  canvasRefs[num] = el
  // A fresh canvas defaults to 300x150; reset it immediately so a very long
  // PDF never has a large one-frame allocation before IntersectionObserver
  // runs. Never touch a canvas that already shows pixels: Vue re-binds this
  // inline :ref on every pageMetas patch, and after invalidateAllPages
  // cleared pageWork during a fit-width resize the old status check would
  // see "not rendered" and blank an is-rendered canvas — flashing the page.
  if (el && !el.classList.contains('is-rendered')) {
    el.width = 1
    el.height = 1
  }
}
function setTextRef(num: number, el: HTMLDivElement | null) {
  textRefs[num] = el
}

function currentScale(): number {
  if (zoomMode.value === 'fit') return fitScale
  return zoomMode.value
}

/** Compute fit-width scale so the PDF page fills the container width. */
function computeFitScale(page: pdfjsLib.PDFPageProxy): number {
  const containerWidth = Math.max(240, (scrollRef.value?.clientWidth ?? 800) - 40)
  const viewport0 = page.getViewport({ scale: 1 })
  return containerWidth / viewport0.width
}

function isCancellationError(value: unknown): boolean {
  const name = value instanceof Error ? value.name : ''
  return name === 'RenderingCancelledException' || name === 'AbortException'
}

function releaseCanvas(num: number) {
  const canvas = canvasRefs[num]
  if (canvas) {
    // Resetting width/height is the reliable way to return the GPU/bitmap
    // backing store; clearRect alone keeps the large allocation alive.
    canvas.width = 1
    canvas.height = 1
    canvas.style.width = ''
    canvas.style.height = ''
    canvas.classList.remove('is-rendered')
  }
  textRefs[num]?.replaceChildren()
}

function releasePage(num: number) {
  const work = pageWork.get(num)
  if (work) {
    if (work.textTimer !== undefined) window.clearTimeout(work.textTimer)
    work.renderTask?.cancel()
    work.textLayer?.cancel()
    work.page?.cleanup()
    pageWork.delete(num)
  }
  releaseCanvas(num)
}

function releaseAllPages() {
  for (const num of [...pageWork.keys()]) releasePage(num)
  renderQueue = []
  nearbyPages.clear()
  visiblePages.clear()
}

/**
 * Drop render state for every page so they re-queue on the next observer pass,
 * but leave the visible canvas pixels in place. A subsequent renderPage then
 * swaps the new pixels in double-buffered (see renderPage) — so a fit-width
 * resize never blanks the page the user is reading. Used by rerenderAll, where
 * releaseAllPages (which resets each canvas to 1x1) would flash the whole view.
 */
function invalidateAllPages() {
  for (const num of [...pageWork.keys()]) {
    const work = pageWork.get(num)
    if (work) {
      if (work.textTimer !== undefined) window.clearTimeout(work.textTimer)
      work.renderTask?.cancel()
      work.textLayer?.cancel()
      work.page?.cleanup()
    }
  }
  pageWork.clear()
  renderQueue = []
  nearbyPages.clear()
  visiblePages.clear()
}

function queuePage(num: number, priority = false) {
  if (!pdfDoc || pageWork.has(num) || !nearbyPages.has(num)) return
  pageWork.set(num, {
    generation: loadGeneration,
    status: 'queued',
  })
  if (priority) renderQueue.unshift(num)
  else renderQueue.push(num)
  drainRenderQueue()
}

function drainRenderQueue() {
  while (activeRenders < MAX_CONCURRENT_RENDERS && renderQueue.length) {
    const num = renderQueue.shift()
    if (num === undefined) return
    const work = pageWork.get(num)
    if (!work || work.status !== 'queued' || !nearbyPages.has(num)) continue
    work.status = 'rendering'
    activeRenders += 1
    void renderPage(num, work)
  }
}

/** Render one canvas. Text selection is added later while the page is visible. */
async function renderPage(num: number, work: PageWork) {
  const doc = pdfDoc
  const canvas = canvasRefs[num]
  if (!doc || !canvas) {
    pageWork.delete(num)
    activeRenders -= 1
    drainRenderQueue()
    return
  }

  let page: pdfjsLib.PDFPageProxy | undefined
  try {
    page = await doc.getPage(num)
    if (pageWork.get(num) !== work || work.generation !== loadGeneration) return
    work.page = page
    const viewport = page.getViewport({ scale: currentScale() })
    const dpr = computeRenderDpr(
      viewport.width,
      viewport.height,
      Math.min(window.devicePixelRatio || 1, renderPolicy.maxRenderDpr),
      renderPolicy.maxCanvasPixels,
    )
    const backingW = Math.max(1, Math.floor(viewport.width * dpr))
    const backingH = Math.max(1, Math.floor(viewport.height * dpr))
    const transform = dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined
    // Double-buffer: render into an offscreen canvas, then swap the result
    // onto the visible canvas in one synchronous step. Setting canvas.width
    // clears its pixels, so doing it only after the render resolves means the
    // display is never painted blank mid-render — the flash that used to show
    // on resize/zoom/fullscreen toggles is gone.
    const off = document.createElement('canvas')
    off.width = backingW
    off.height = backingH
    const offCtx = off.getContext('2d', { alpha: false })
    if (!offCtx) throw new Error('无法创建 PDF canvas 上下文')
    work.renderTask = page.render({ canvasContext: offCtx, viewport, transform })
    await work.renderTask.promise
    if (pageWork.get(num) !== work || work.generation !== loadGeneration) return
    work.renderTask = undefined
    // Synchronous swap: the width-reset and the pixel copy happen in the same
    // task, so the next paint sees the new pixels, never an empty canvas.
    canvas.width = backingW
    canvas.height = backingH
    canvas.style.width = `${viewport.width}px`
    canvas.style.height = `${viewport.height}px`
    const ctx = canvas.getContext('2d', { alpha: false })
    if (!ctx) throw new Error('无法创建 PDF canvas 上下文')
    ctx.drawImage(off, 0, 0)
    work.status = 'rendered'
    canvas.classList.add('is-rendered')
    if (visiblePages.has(num)) scheduleTextLayer(num, work)
  } catch (e) {
    if (!isCancellationError(e) && pageWork.get(num) === work) {
      console.warn(`渲染第 ${num} 页失败:`, e)
    }
    if (pageWork.get(num) === work) {
      pageWork.delete(num)
      releaseCanvas(num)
    }
  } finally {
    if (page && pageWork.get(num) !== work) page.cleanup()
    activeRenders = Math.max(0, activeRenders - 1)
    drainRenderQueue()
  }
}

function scheduleTextLayer(num: number, work: PageWork) {
  if (
    work.status !== 'rendered'
    || work.textRendered
    || work.textRendering
    || work.textTimer !== undefined
  ) return

  // Canvas first: deferring text extraction keeps time-to-first-page low.
  work.textTimer = window.setTimeout(() => {
    work.textTimer = undefined
    if (visiblePages.has(num) && pageWork.get(num) === work) {
      void renderTextLayer(num, work)
    }
  }, 80)
}

async function renderTextLayer(num: number, work: PageWork) {
  const page = work.page
  const textDiv = textRefs[num]
  if (!page || !textDiv || work.textRendering || work.textRendered) return
  work.textRendering = true
  try {
    const viewport = page.getViewport({ scale: currentScale() })
    const textContent = await page.getTextContent()
    if (pageWork.get(num) !== work || !nearbyPages.has(num)) return
    textDiv.replaceChildren()
    textDiv.style.width = `${viewport.width}px`
    textDiv.style.height = `${viewport.height}px`
    const textLayer = new pdfjsLib.TextLayer({
      textContentSource: textContent,
      container: textDiv,
      viewport,
    })
    work.textLayer = textLayer
    await textLayer.render()
    if (pageWork.get(num) === work) work.textRendered = true
  } catch (e) {
    if (!isCancellationError(e)) console.warn(`文字层渲染失败 (第 ${num} 页):`, e)
  } finally {
    if (pageWork.get(num) === work) work.textRendering = false
  }
}

function scrollRoot(): HTMLElement | null {
  return scrollRef.value?.parentElement ?? null
}

/** Observe a small page window and recycle canvases after they leave it. */
function setupObservers() {
  const container = scrollRef.value
  const root = scrollRoot()
  if (!container || !root) return
  renderObserver?.disconnect()
  visibleObserver?.disconnect()
  renderObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const num = Number((entry.target as HTMLElement).dataset.pageNum)
        if (entry.isIntersecting) {
          nearbyPages.add(num)
          queuePage(num)
        } else {
          nearbyPages.delete(num)
          visiblePages.delete(num)
          releasePage(num)
        }
      }
    },
    { root, rootMargin: `${RENDER_MARGIN_PX}px 0px` },
  )
  visibleObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const num = Number((entry.target as HTMLElement).dataset.pageNum)
        if (entry.isIntersecting) {
          visiblePages.add(num)
          const work = pageWork.get(num)
          if (work) scheduleTextLayer(num, work)
        } else {
          visiblePages.delete(num)
        }
      }
      if (visiblePages.size > 0) emit('pagechange', Math.min(...visiblePages))
    },
    { root },
  )
  const wraps = container.querySelectorAll<HTMLElement>('.pdf-page-wrap')
  wraps.forEach((wrap) => {
    renderObserver?.observe(wrap)
    visibleObserver?.observe(wrap)
  })
}

async function load() {
  const generation = ++loadGeneration
  await destroyCurrentDocument()
  if (unmounted || generation !== loadGeneration || !props.src) return
  loading.value = true
  error.value = ''
  pageMetas.value = []
  try {
    const task = pdfjsLib.getDocument({
      url: localFileUrl(props.src),
      rangeChunkSize: RANGE_CHUNK_SIZE,
      disableStream: true,
      disableAutoFetch: true,
      canvasMaxAreaInBytes: renderPolicy.maxCanvasPixels * 4,
    })
    loadingTask = task
    const doc = await task.promise
    if (generation !== loadGeneration || unmounted) {
      await task.destroy()
      return
    }
    pdfDoc = doc
    emit('pagecount', doc.numPages)
    // Use page 1 to derive the fit-width scale; record every page's placeholder
    // size at that scale so the scroll area has correct height before render.
    const page1 = await doc.getPage(1)
    fitScale = computeFitScale(page1)
    const vp1 = page1.getViewport({ scale: fitScale })
    const metas: PageMeta[] = []
    for (let i = 1; i <= doc.numPages; i++) {
      // Assume uniform page size (common case); page 1 dimensions for all.
      // Non-uniform PDFs will have slightly mismatched placeholders — acceptable
      // for v1; lazy render corrects the actual canvas size on render.
      metas.push({ num: i, width: vp1.width, height: vp1.height })
    }
    pageMetas.value = metas
    loading.value = false
    await nextTick()
    if (generation !== loadGeneration) return
    setupObservers()
    nearbyPages.add(1)
    queuePage(1, true)
    void emitOutline(doc, generation)
  } catch (e) {
    if (generation !== loadGeneration || isCancellationError(e)) return
    const failedTask = loadingTask
    loadingTask = null
    pdfDoc = null
    try {
      await failedTask?.destroy()
    } catch (destroyError) {
      if (!isCancellationError(destroyError)) console.warn('释放失败的 PDF 加载任务:', destroyError)
    }
    error.value = (e as Error)?.message || 'PDF 解析失败'
    loading.value = false
  }
}

async function emitOutline(doc: pdfjsLib.PDFDocumentProxy, generation: number) {
  try {
    const rawOutline = await doc.getOutline()
    const outline = await buildOutline(rawOutline, 1, doc)
    if (generation === loadGeneration) emit('outline', outline)
  } catch (e) {
    if (generation === loadGeneration) console.warn('读取 PDF 目录失败:', e)
  }
}

// pdfjs-dist v4 does not export OutlineNode/ExplicitDest as named types
// (they exist only as JSDoc typedefs). Use any for outline internals.
async function buildOutline(
  raw: any[] | null,
  level: number,
  doc: pdfjsLib.PDFDocumentProxy,
): Promise<{ text: string; level: number; page: number }[]> {
  if (!raw || !raw.length) return []
  const out: { text: string; level: number; page: number }[] = []
  for (const node of raw) {
    let page = 1
    try {
      // dest may be a named dest (string) or an explicit dest array.
      // Resolve named dests to the explicit array form via getDestination.
      let dest: any = node.dest
      if (typeof dest === 'string') {
        dest = await doc.getDestination(dest)
      }
      // Explicit dest array: dest[0] is the page RefProxy ({num, gen}).
      // pdf.js v4 getPageIndex validates the arg is a RefProxy (isRefProxy),
      // so we must pass dest[0], not the whole dest array.
      const ref = Array.isArray(dest) ? dest[0] : null
      if (ref) {
        const idx = await doc.getPageIndex(ref)
        if (typeof idx === 'number') page = idx + 1
      }
    } catch {
      page = 1
    }
    out.push({ text: node.title, level, page })
    if (node.items?.length) {
      out.push(...await buildOutline(node.items, level + 1, doc))
    }
  }
  return out
}

/** Recompute placeholders and render only the nearby page window at new scale. */
async function rerenderAll() {
  const doc = pdfDoc
  if (!doc) return
  const generation = loadGeneration
  // A fit-width resize changes every page's height, so the total document
  // height changes too. Without preserving the proportional scroll position,
  // the same absolute scrollTop points at different content — that is the
  // jump the user sees when toggling fullscreen (or any width change).
  const root = scrollRoot()
  const prevMax = root ? root.scrollHeight - root.clientHeight : 0
  const fraction = prevMax > 0
    ? Math.min(1, Math.max(0, root!.scrollTop / prevMax))
    : 0
  // Recompute placeholder sizes for the new scale.
  const page1 = await doc.getPage(1)
  if (zoomMode.value === 'fit') fitScale = computeFitScale(page1)
  const s = zoomMode.value === 'fit' ? fitScale : zoomMode.value
  const vp = page1.getViewport({ scale: s })
  pageMetas.value = pageMetas.value.map((p) => ({ ...p, width: vp.width, height: vp.height }))
  // Drop render state but keep the old canvas pixels; renderPage swaps the
  // new pixels in double-buffered, so the visible page never flashes blank.
  invalidateAllPages()
  await nextTick()
  if (generation !== loadGeneration) return
  // Restore the proportional reading position for the new layout. The
  // placeholder heights are bound to pageMetas and already live in the DOM
  // after nextTick, so scrollHeight reflects the new document height.
  if (root && prevMax > 0) {
    const newMax = root.scrollHeight - root.clientHeight
    root.scrollTop = fraction * newMax
  }
  setupObservers()
  const rootRect = scrollRoot()?.getBoundingClientRect()
  if (!rootRect) return
  const wraps = Array.from(scrollRef.value?.querySelectorAll<HTMLElement>('.pdf-page-wrap') ?? [])
  for (const w of wraps) {
    const rect = w.getBoundingClientRect()
    if (!isWithinRenderWindow(rect, rootRect, RENDER_MARGIN_PX)) continue
    const num = Number(w.dataset.pageNum)
    nearbyPages.add(num)
    if (isWithinRenderWindow(rect, rootRect, 0)) visiblePages.add(num)
    queuePage(num)
  }
}

function setZoom(mode: 'fit' | number) {
  zoomMode.value = mode
  void rerenderAll()
}

function setZoomRatio(ratio: number) {
  zoomMode.value = computePdfZoomScale(fitScale, ratio)
  void rerenderAll()
}

function scrollToPage(num: number) {
  const el = scrollRef.value?.querySelector<HTMLElement>(`.pdf-page-wrap[data-page-num="${num}"]`)
  el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ scrollToPage, setZoom, setZoomRatio })

async function destroyCurrentDocument() {
  renderObserver?.disconnect()
  visibleObserver?.disconnect()
  renderObserver = null
  visibleObserver = null
  releaseAllPages()
  const task = loadingTask
  const doc = pdfDoc
  loadingTask = null
  pdfDoc = null
  try {
    if (task) await task.destroy()
    else if (doc) await doc.destroy()
  } catch (e) {
    if (!isCancellationError(e)) console.warn('释放 PDF 资源失败:', e)
  }
  pdfjsLib.TextLayer.cleanup()
}

function setupResizeObserver() {
  const root = scrollRoot()
  if (!root) return
  resizeObserver?.disconnect()
  let previousWidth = root.clientWidth
  resizeObserver = new ResizeObserver(() => {
    if (zoomMode.value !== 'fit' || Math.abs(root.clientWidth - previousWidth) < 2) return
    previousWidth = root.clientWidth
    if (resizeTimer !== null) window.clearTimeout(resizeTimer)
    resizeTimer = window.setTimeout(() => {
      resizeTimer = null
      void rerenderAll()
    }, 160)
  })
  resizeObserver.observe(root)
}

watch(() => props.src, () => { void load() })
watch(() => appStore.theme, (t) => { theme.value = t })

onMounted(() => {
  setupResizeObserver()
  void load()
})
onBeforeUnmount(() => {
  unmounted = true
  loadGeneration += 1
  resizeObserver?.disconnect()
  if (resizeTimer !== null) window.clearTimeout(resizeTimer)
  void destroyCurrentDocument()
})
</script>

<style>
/* pdf.js (raw API) appends a measurement canvas with this class directly to
   <body> and expects the viewer CSS to hide it. Without pdf_viewer.css the
   300×150 canvas sits in normal flow just below the viewport, inflating the
   document by ~156px — enough for scrollIntoView (TOC page jumps) to scroll
   the whole app shell and for touch scrolls to drag the page. Mirror the
   official pdf_viewer.css rule. */
.hiddenCanvasElement {
  position: absolute;
  top: 0;
  left: 0;
  width: 0;
  height: 0;
  display: none;
}
</style>

<style scoped>
.pdf-viewer {
  /* NOT its own scroll container — flows in .pane-center so onContentScroll
     (FAB hide, mobile header collapse) and page-turn transitions are reused. */
  padding: 12px 20px 120px;
}
/* Filter each live page instead of the document-sized parent. Filtering the
   full .pdf-pages stack can allocate an enormous compositor surface. */
.pdf-theme-light .pdf-canvas.is-rendered { filter: none; }
.pdf-theme-dark .pdf-canvas.is-rendered { filter: invert(1) hue-rotate(180deg); }
.pdf-theme-eye-care .pdf-canvas.is-rendered { filter: sepia(0.7) hue-rotate(28deg) brightness(0.8) saturate(2.8); }

.pdf-pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}
.pdf-page-wrap {
  position: relative;
  contain: layout paint style;
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
}
.pdf-canvas {
  display: block;
  width: 100%;
  height: 100%;
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}
.pdf-canvas.is-rendered { opacity: 1; }

.pdf-text-layer {
  position: absolute;
  inset: 0;
  overflow: hidden;
  line-height: 1;
  /* Text is transparent so the layer is invisible — it exists only for
     selection/search. `color` is inherited by pdf.js's dynamically-created
     spans (which have no data-v attr, so scoped span selectors can't reach
     them). opacity 0.25 softens the selection highlight, not the glyphs. */
  color: transparent;
  opacity: 0.25;
}
.pdf-text-layer ::selection { background: var(--accent); color: transparent; }

.pdf-state {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-faint);
  font-size: 14px;
}
.pdf-state.error { color: #f87171; }
.pdf-state .is-loading { animation: spin 1s linear infinite; color: var(--accent); }

@media (max-width: 768px) {
  .pdf-viewer { padding: 8px 8px calc(112px + var(--safe-bottom)); }
  .pdf-pages { gap: 10px; }
  .pdf-page-wrap { border-radius: 6px; }
}
</style>
