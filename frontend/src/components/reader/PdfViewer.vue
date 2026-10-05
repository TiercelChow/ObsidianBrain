<template>
  <div ref="scrollRef" class="pdf-viewer" :class="`pdf-theme-${theme}`" @dblclick="onDoubleClick">
    <div v-if="loading" class="pdf-state">
      <el-icon class="is-loading"><Loading /></el-icon><span>PDF 加载中…</span>
    </div>
    <div v-else-if="error" class="pdf-state error">⚠️ {{ error }}</div>
    <div v-else ref="pagesRef" class="pdf-pages">
      <div
        v-for="p in pageMetas"
        :key="p.num"
        class="pdf-page-wrap"
        :data-page-num="p.num"
        :style="{ width: p.width + 'px', height: p.height + 'px', marginLeft: Math.max(0, (fitViewportWidth - p.width) / 2) + 'px' }"
      >
        <canvas :ref="(el) => setCanvasRef(p.num, el as HTMLCanvasElement | null)" class="pdf-canvas"></canvas>
        <canvas :ref="(el) => setDetailRef(p.num, el as HTMLCanvasElement | null)" class="pdf-canvas pdf-detail-canvas" aria-hidden="true"></canvas>
        <div :ref="(el) => setTextRef(p.num, el as HTMLDivElement | null)" class="pdf-text-layer"></div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import * as pdfjsLib from 'pdfjs-dist'
import type { TextItem } from 'pdfjs-dist/types/src/display/api'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { useAppStore } from '@/stores/app'
import { localFileUrl } from '@/api/reader'
import {
  computeCenteredZoomScrollTop,
  computePdfZoomScale,
  computeRenderDpr,
  computePdfFitScale,
  computePdfReadingRatio,
  computePdfDetailRegion,
  computePdfZoomAnchorScroll,
  isWithinRenderWindow,
} from './pdfRenderPolicy'
import { getPdfRenderPolicy, isPhoneViewport } from '@/utils/mobileLayoutPolicy'

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

const props = defineProps<{ src: string }>()
const emit = defineEmits<{
  outline: [items: { text: string; level: number; page: number }[]]
  pagechange: [page: number, src: string]
  pagecount: [pages: number, src: string]
  zoomchange: [ratio: number]
}>()
const appStore = useAppStore()
const theme = ref(appStore.theme)

interface PageMeta { num: number; width: number; height: number; nativeWidth: number; nativeHeight: number }
interface ZoomAnchor { num: number; x: number; y: number; clientX: number; clientY: number }
interface PageWork {
  generation: number
  status: 'queued' | 'rendering' | 'rendered'
  page?: pdfjsLib.PDFPageProxy
  renderTask?: pdfjsLib.RenderTask
  textLayer?: pdfjsLib.TextLayer
  textTimer?: number
  textRendering?: boolean
  textRendered?: boolean
  dpr?: number
  detailKey?: string
}

function renderPolicy() { return getPdfRenderPolicy(window.innerWidth, navigator.hardwareConcurrency) }
const RANGE_CHUNK_SIZE = 256 * 1024

const scrollRef = ref<HTMLElement | null>(null)
const pagesRef = ref<HTMLElement | null>(null)
const loading = ref(true)
const error = ref('')
const pageMetas = ref<PageMeta[]>([])
const canvasRefs: Record<number, HTMLCanvasElement | null> = {}
const textRefs: Record<number, HTMLDivElement | null> = {}
const detailRefs: Record<number, HTMLCanvasElement | null> = {}
let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null
let loadingTask: pdfjsLib.PDFDocumentLoadingTask | null = null
let fitScale = 1 // stable fit-width baseline used by toolbar zoom ratios
const fitViewportWidth = ref(0)
let renderObserver: IntersectionObserver | null = null
let visibleObserver: IntersectionObserver | null = null
let resizeObserver: ResizeObserver | null = null
let resizeTimer: number | null = null
let loadGeneration = 0
let layoutGeneration = 0
let activeRenders = 0
let renderQueue: number[] = []
let unmounted = false
const nearbyPages = new Set<number>()
const visiblePages = new Set<number>()
const pageWork = new Map<number, PageWork>()
// Ratios survive rotation/fullscreen; absolute PDF scales do not.
const zoomRatio = ref(1)
let detailTimer: number | null = null
let detailGeneration = 0
let detailTask: pdfjsLib.RenderTask | null = null
let detailRendering = false
let pinch: { distance: number; ratio: number; anchor: ZoomAnchor; originX: number; originY: number } | null = null
let lastTap: { time: number; x: number; y: number } | null = null
let touchStart: { x: number; y: number } | null = null
let hadPinch = false
let lastTouchZoomAt = -Infinity

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
function setDetailRef(num: number, el: HTMLCanvasElement | null) {
  detailRefs[num] = el
  if (el && !el.classList.contains('is-rendered')) { el.width = 1; el.height = 1 }
}

function currentScale(): number {
  return computePdfZoomScale(fitScale, zoomRatio.value)
}

/** Compute fit-width scale so the PDF page fills the container width. */
function computeFitScale(page: pdfjsLib.PDFPageProxy): number {
  const root = scrollRoot()
  const style = scrollRef.value ? getComputedStyle(scrollRef.value) : null
  const padding = Number.parseFloat(style?.paddingLeft ?? '0') + Number.parseFloat(style?.paddingRight ?? '0')
  const viewport0 = page.getViewport({ scale: 1 })
  const scale = computePdfFitScale(root?.clientWidth ?? 800, viewport0.width, padding, isPhoneViewport(window.innerWidth) ? 0 : 2) || fitScale
  fitViewportWidth.value = scale * viewport0.width
  return scale
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
  releaseDetail(num)
}

function releaseDetail(num: number) {
  const canvas = detailRefs[num]
  if (canvas) { canvas.width = 1; canvas.height = 1; canvas.classList.remove('is-rendered') }
  const work = pageWork.get(num)
  if (work) work.detailKey = undefined
}

function cancelDetail() {
  detailGeneration += 1
  if (detailTimer !== null) { window.clearTimeout(detailTimer); detailTimer = null }
  detailTask?.cancel()
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
  cancelDetail()
  for (const num of [...pageWork.keys()]) {
    const work = pageWork.get(num)
    if (work) {
      if (work.textTimer !== undefined) window.clearTimeout(work.textTimer)
      work.renderTask?.cancel()
      work.textLayer?.cancel()
      work.page?.cleanup()
    }
    releaseDetail(num)
    textRefs[num]?.replaceChildren()
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
  if (detailRendering) return
  while (activeRenders < renderPolicy().maxConcurrentRenders && renderQueue.length) {
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
  let off: HTMLCanvasElement | null = null
  try {
    page = await doc.getPage(num)
    if (pageWork.get(num) !== work || work.generation !== loadGeneration) return
    work.page = page
    // Correct mixed-size/rotated placeholders when this page is first read.
    const native = page.getViewport({ scale: 1 })
    const meta = pageMetas.value[num - 1]
    if (meta && (meta.nativeWidth !== native.width || meta.nativeHeight !== native.height)) {
      pageMetas.value[num - 1] = { num, nativeWidth: native.width, nativeHeight: native.height, width: native.width * currentScale(), height: native.height * currentScale() }
    }
    const viewport = page.getViewport({ scale: currentScale() })
    const dpr = computeRenderDpr(
      viewport.width,
      viewport.height,
      Math.min(window.devicePixelRatio || 1, renderPolicy().maxRenderDpr),
      renderPolicy().maxCanvasPixels,
    )
    const backingW = Math.max(1, Math.floor(viewport.width * dpr))
    const backingH = Math.max(1, Math.floor(viewport.height * dpr))
    const transform = dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined
    // Double-buffer: render into an offscreen canvas, then swap the result
    // onto the visible canvas in one synchronous step. Setting canvas.width
    // clears its pixels, so doing it only after the render resolves means the
    // display is never painted blank mid-render — the flash that used to show
    // on resize/zoom/fullscreen toggles is gone.
    off = document.createElement('canvas')
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
    work.dpr = dpr
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
    if (off) { off.width = 1; off.height = 1 }
    if (page && pageWork.get(num) !== work) page.cleanup()
    activeRenders = Math.max(0, activeRenders - 1)
    drainRenderQueue()
    scheduleDetail()
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
    textDiv.style.setProperty('--scale-factor', `${viewport.scale}`)
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

function pageElement(num: number): HTMLElement | null {
  return scrollRef.value?.querySelector<HTMLElement>(`.pdf-page-wrap[data-page-num="${num}"]`) ?? null
}

function captureZoomAnchor(clientX?: number, clientY?: number): ZoomAnchor | undefined {
  const root = scrollRoot()
  if (!root) return
  const bounds = root.getBoundingClientRect()
  const x = clientX ?? bounds.left + root.clientWidth / 2
  const y = clientY ?? bounds.top + root.clientHeight / 2
  // Only inspect visible/nearby pages, not every page of a large book.
  let nearest: { num: number; rect: DOMRect; distance: number } | undefined
  for (const num of nearbyPages) {
    const rect = pageElement(num)?.getBoundingClientRect()
    if (!rect || rect.height <= 0) continue
    const distance = Math.max(rect.top - y, y - rect.bottom, 0)
    if (!nearest || distance < nearest.distance) nearest = { num, rect, distance }
  }
  if (!nearest) return
  return { num: nearest.num, x: Math.max(0, Math.min(1, (x - nearest.rect.left) / nearest.rect.width)), y: Math.max(0, Math.min(1, (y - nearest.rect.top) / nearest.rect.height)), clientX: x, clientY: y }
}

/** Full-page previews stay bounded; only the visible rectangle gets retina detail. */
function scheduleDetail() {
  if (unmounted || pinch || !pdfDoc) return
  if (detailTimer !== null) window.clearTimeout(detailTimer)
  detailTimer = window.setTimeout(() => { detailTimer = null; void renderVisibleDetails() }, 100)
}

function onPdfScroll() {
  cancelDetail()
  scheduleDetail()
}

async function renderVisibleDetails() {
  if (unmounted || pinch || !pdfDoc) return
  if (activeRenders || detailRendering) { scheduleDetail(); return }
  const root = scrollRoot()
  if (!root || root.clientWidth <= 0 || root.clientHeight <= 0) return
  const request = detailGeneration
  detailRendering = true
  try {
    for (const num of visiblePages) {
      const work = pageWork.get(num)
      const page = work?.page
      const canvas = detailRefs[num]
      const rect = canvasRefs[num]?.getBoundingClientRect()
      if (!work || work.status !== 'rendered' || !page || !canvas || !rect) continue
      const desiredDpr = Math.min(window.devicePixelRatio || 1, renderPolicy().maxRenderDpr)
      if ((work.dpr ?? 1) >= desiredDpr - .05) { releaseDetail(num); continue }
      const rootRect = root.getBoundingClientRect()
      const region = computePdfDetailRegion(rect, { left: rootRect.left + root.clientLeft, top: rootRect.top + root.clientTop, width: root.clientWidth, height: root.clientHeight })
      if (!region) { releaseDetail(num); continue }
      const dpr = computeRenderDpr(region.width, region.height, desiredDpr, renderPolicy().maxCanvasPixels)
      const key = `${layoutGeneration}:${region.left}:${region.top}:${region.width}:${region.height}:${dpr}`
      if (work.detailKey === key) continue
      const off = document.createElement('canvas')
      off.width = Math.max(1, Math.floor(region.width * dpr))
      off.height = Math.max(1, Math.floor(region.height * dpr))
      try {
        const ctx = off.getContext('2d', { alpha: false })
        if (!ctx) continue
        detailTask = page.render({ canvasContext: ctx, viewport: page.getViewport({ scale: currentScale() }), transform: [dpr, 0, 0, dpr, -region.left * dpr, -region.top * dpr] })
        await detailTask.promise
        if (request !== detailGeneration || pageWork.get(num) !== work || !visiblePages.has(num)) return
        canvas.width = off.width
        canvas.height = off.height
        canvas.style.width = `${region.width}px`
        canvas.style.height = `${region.height}px`
        canvas.style.left = `${region.left}px`
        canvas.style.top = `${region.top}px`
        canvas.getContext('2d', { alpha: false })?.drawImage(off, 0, 0)
        canvas.classList.add('is-rendered')
        work.detailKey = key
      } finally {
        off.width = 1
        off.height = 1
        detailTask = null
      }
    }
  } catch (e) {
    if (!isCancellationError(e)) console.warn('PDF 可视区域精细渲染失败:', e)
  } finally {
    detailRendering = false
    drainRenderQueue()
  }
}

function resetPinchPreview() {
  if (pagesRef.value) { pagesRef.value.style.transform = ''; pagesRef.value.style.transformOrigin = '' }
}

function touchDistance(touches: TouchList): number {
  return Math.hypot(touches[0].clientX - touches[1].clientX, touches[0].clientY - touches[1].clientY)
}

function onTouchStart(event: TouchEvent) {
  if (event.touches.length === 1) {
    touchStart = { x: event.touches[0].clientX, y: event.touches[0].clientY }
    hadPinch = false
  }
  if (event.touches.length !== 2 || loading.value) return
  const x = (event.touches[0].clientX + event.touches[1].clientX) / 2
  const y = (event.touches[0].clientY + event.touches[1].clientY) / 2
  const anchor = captureZoomAnchor(x, y)
  if (!anchor) return
  if (event.cancelable) event.preventDefault()
  cancelDetail()
  hadPinch = true
  lastTap = null
  pinch = { distance: Math.max(1, touchDistance(event.touches)), ratio: zoomRatio.value, anchor, originX: x, originY: y }
  const bounds = pagesRef.value?.getBoundingClientRect()
  if (bounds && pagesRef.value) pagesRef.value.style.transformOrigin = `${x - bounds.left}px ${y - bounds.top}px`
}

function onTouchMove(event: TouchEvent) {
  if (!pinch || event.touches.length !== 2 || !pagesRef.value) return
  if (event.cancelable) event.preventDefault()
  const ratio = computePdfZoomScale(1, pinch.ratio * touchDistance(event.touches) / pinch.distance)
  const x = (event.touches[0].clientX + event.touches[1].clientX) / 2
  const y = (event.touches[0].clientY + event.touches[1].clientY) / 2
  pinch.anchor.clientX = x
  pinch.anchor.clientY = y
  pagesRef.value.style.transform = `translate(${x - pinch.originX}px, ${y - pinch.originY}px) scale(${ratio / zoomRatio.value})`
}

function onTouchEnd(event: TouchEvent) {
  if (pinch && event.touches.length < 2) {
    if (event.cancelable) event.preventDefault()
    const state = pinch
    const previewScale = Number(pagesRef.value?.style.transform.match(/scale\(([^)]+)\)/)?.[1] ?? 1)
    pinch = null
    lastTouchZoomAt = performance.now()
    setZoomRatio(state.ratio * previewScale, state.anchor)
    return
  }
  if (hadPinch || event.touches.length || !touchStart || event.changedTouches.length !== 1) return
  const touch = event.changedTouches[0]
  if (Math.hypot(touch.clientX - touchStart.x, touch.clientY - touchStart.y) > 10) { lastTap = null; return }
  if (window.getSelection()?.toString()) return
  const now = performance.now()
  if (lastTap && now - lastTap.time < 320 && Math.hypot(touch.clientX - lastTap.x, touch.clientY - lastTap.y) < 24) {
    if (event.cancelable) event.preventDefault()
    lastTap = null
    lastTouchZoomAt = now
    setZoomRatio(zoomRatio.value > 1.6 ? 1 : 2, captureZoomAnchor(touch.clientX, touch.clientY))
  } else lastTap = { time: now, x: touch.clientX, y: touch.clientY }
}

function onTouchCancel() {
  pinch = null
  lastTap = null
  hadPinch = true
  resetPinchPreview()
  scheduleDetail()
}

function onDoubleClick(event: MouseEvent) {
  // Desktop double-click retains native text selection. Touch is handled above.
  if (!isPhoneViewport(window.innerWidth) || performance.now() - lastTouchZoomAt < 600) return
  setZoomRatio(zoomRatio.value > 1.6 ? 1 : 2, captureZoomAnchor(event.clientX, event.clientY))
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
          queuePage(num, isWithinRenderWindow(entry.boundingClientRect, root.getBoundingClientRect(), 0))
        } else {
          nearbyPages.delete(num)
          visiblePages.delete(num)
          releasePage(num)
        }
      }
    },
    { root, rootMargin: `${renderPolicy().renderMarginPx}px 0px` },
  )
  visibleObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        const num = Number((entry.target as HTMLElement).dataset.pageNum)
        if (entry.isIntersecting) {
          visiblePages.add(num)
          const work = pageWork.get(num)
          if (work) scheduleTextLayer(num, work)
          queuePage(num, true)
        } else {
          visiblePages.delete(num)
          releaseDetail(num)
        }
      }
      if (visiblePages.size > 0) emit('pagechange', Math.min(...visiblePages), props.src)
      scheduleDetail()
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
  zoomRatio.value = 1
  emit('zoomchange', 1)
  try {
    const task = pdfjsLib.getDocument({
      url: localFileUrl(props.src),
      rangeChunkSize: RANGE_CHUNK_SIZE,
      disableStream: true,
      disableAutoFetch: true,
      // This limits image decoding, not the final page canvas. Do not downsample
      // a scanned source to the tiny phone page budget before zooming into it.
      canvasMaxAreaInBytes: 16_000_000 * 4,
    })
    loadingTask = task
    const doc = await task.promise
    if (generation !== loadGeneration || unmounted) {
      await task.destroy()
      return
    }
    pdfDoc = doc
    // Use page 1 to derive the fit-width scale; record every page's placeholder
    // size at that scale so the scroll area has correct height before render.
    const page1 = await doc.getPage(1)
    if (generation !== loadGeneration || unmounted) return
    fitScale = computeFitScale(page1)
    let readingLeft = 0
    if (isPhoneViewport(window.innerWidth)) {
      let focusTimer: number | undefined
      try {
        const native = page1.getViewport({ scale: 1 })
        // Margin detection is optional: never delay the first visible page for
        // a slow text extraction (complex/scanned PDFs can be expensive).
        const content = await Promise.race([
          page1.getTextContent().catch(() => null),
          new Promise<null>(resolve => { focusTimer = window.setTimeout(() => resolve(null), 120) }),
        ])
        if (generation !== loadGeneration || unmounted) return
        const items = content?.items.filter((item): item is TextItem => 'str' in item && item.str.trim().length > 0) ?? []
        if (items.length >= 8 && native.rotation === 0 && items.every(item => Math.abs(item.transform[1]) < .01)) {
          let left = native.width
          let right = 0
          for (const item of items) {
            const x = native.convertToViewportPoint(item.transform[4], item.transform[5])[0]
            left = Math.min(left, x)
            right = Math.max(right, x + item.width)
          }
          zoomRatio.value = computePdfReadingRatio(native.width, left, right)
          readingLeft = left * currentScale()
        }
      } catch { /* Scans and extraction failures retain full-page fit. */ }
      finally { if (focusTimer !== undefined) window.clearTimeout(focusTimer) }
    }
    const native1 = page1.getViewport({ scale: 1 })
    const vp1 = page1.getViewport({ scale: currentScale() })
    const metas: PageMeta[] = []
    for (let i = 1; i <= doc.numPages; i++) {
      // Assume uniform page size (common case); page 1 dimensions for all.
      // Non-uniform PDFs will have slightly mismatched placeholders — acceptable
      // for v1; lazy render corrects the actual canvas size on render.
      metas.push({ num: i, width: vp1.width, height: vp1.height, nativeWidth: native1.width, nativeHeight: native1.height })
    }
    pageMetas.value = metas
    loading.value = false
    await nextTick()
    if (generation !== loadGeneration) return
    if (scrollRoot()) scrollRoot()!.scrollLeft = readingLeft
    emit('zoomchange', zoomRatio.value)
    // Restoration listeners can now address the page placeholders, even when
    // extracting the first page's text took longer than a frame.
    emit('pagecount', doc.numPages, props.src)
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
async function rerenderAll(anchor = captureZoomAnchor()) {
  const doc = pdfDoc
  if (!doc) return
  const layout = ++layoutGeneration
  const generation = loadGeneration
  // A fit-width resize changes every page's height, so the total document
  // height changes too. Without preserving the proportional scroll position,
  // the same absolute scrollTop points at different content — that is the
  // jump the user sees when toggling fullscreen (or any width change).
  const root = scrollRoot()
  const previousScroll = root
    ? {
        top: root.scrollTop,
        left: root.scrollLeft,
        height: root.scrollHeight,
        width: root.scrollWidth,
        viewportHeight: root.clientHeight,
        viewportWidth: root.clientWidth,
      }
    : null
  // Recompute placeholder sizes for the new scale.
  const page1 = await doc.getPage(1)
  if (layout !== layoutGeneration || generation !== loadGeneration) return
  fitScale = computeFitScale(page1)
  const s = currentScale()
  pageMetas.value = pageMetas.value.map((p) => ({ ...p, width: p.nativeWidth * s, height: p.nativeHeight * s }))
  // Old pixels track the new CSS size until the sharp replacement arrives.
  for (const p of pageMetas.value) {
    const canvas = canvasRefs[p.num]
    if (canvas) { canvas.style.width = `${p.width}px`; canvas.style.height = `${p.height}px` }
  }
  // Drop render state but keep the old canvas pixels; renderPage swaps the
  // new pixels in double-buffered, so the visible page never flashes blank.
  invalidateAllPages()
  await nextTick()
  if (layout !== layoutGeneration || generation !== loadGeneration) return
  // Restore the proportional reading position for the new layout. The
  // placeholder heights are bound to pageMetas and already live in the DOM
  // after nextTick, so scrollHeight reflects the new document height.
  if (root && previousScroll) {
    const target = anchor ? pageElement(anchor.num) : null
    const rect = target?.getBoundingClientRect()
    const rootRect = root.getBoundingClientRect()
    root.scrollTop = rect && anchor ? computePdfZoomAnchorScroll(root.scrollTop, rect.top - rootRect.top, rect.height, anchor.y, anchor.clientY - rootRect.top, root.scrollHeight, root.clientHeight) : computeCenteredZoomScrollTop(
      previousScroll.top,
      previousScroll.viewportHeight,
      previousScroll.height,
      root.clientHeight,
      root.scrollHeight,
    )
    root.scrollLeft = rect && anchor ? computePdfZoomAnchorScroll(root.scrollLeft, rect.left - rootRect.left, rect.width, anchor.x, anchor.clientX - rootRect.left, root.scrollWidth, root.clientWidth) : computeCenteredZoomScrollTop(
      previousScroll.left,
      previousScroll.viewportWidth,
      previousScroll.width,
      root.clientWidth,
      root.scrollWidth,
    )
  }
  setupObservers()
  const rootRect = scrollRoot()?.getBoundingClientRect()
  if (!rootRect) return
  const wraps = Array.from(scrollRef.value?.querySelectorAll<HTMLElement>('.pdf-page-wrap') ?? [])
  for (const w of wraps) {
    const rect = w.getBoundingClientRect()
    if (!isWithinRenderWindow(rect, rootRect, renderPolicy().renderMarginPx)) continue
    const num = Number(w.dataset.pageNum)
    nearbyPages.add(num)
    if (isWithinRenderWindow(rect, rootRect, 0)) visiblePages.add(num)
    queuePage(num, visiblePages.has(num))
  }
  emit('zoomchange', zoomRatio.value)
}

function setZoom(mode: 'fit' | number) {
  setZoomRatio(mode === 'fit' ? 1 : mode / fitScale)
}

function setZoomRatio(ratio: number, anchor = captureZoomAnchor()) {
  resetPinchPreview()
  zoomRatio.value = computePdfZoomScale(1, ratio)
  void rerenderAll(anchor)
}

function scrollToPage(num: number) {
  const el = pageElement(num)
  const root = scrollRoot()
  if (!el || !root) return
  root.scrollTo({ top: root.scrollTop + el.getBoundingClientRect().top - root.getBoundingClientRect().top - root.clientTop, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' })
}

defineExpose({ scrollToPage, setZoom, setZoomRatio })

async function destroyCurrentDocument() {
  cancelDetail()
  resetPinchPreview()
  pinch = null
  lastTap = null
  touchStart = null
  layoutGeneration += 1
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
    if (root.clientWidth <= 0 || Math.abs(root.clientWidth - previousWidth) < 2) return
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
  const root = scrollRoot()
  root?.addEventListener('scroll', onPdfScroll, { passive: true })
  root?.addEventListener('touchstart', onTouchStart, { passive: false })
  root?.addEventListener('touchmove', onTouchMove, { passive: false })
  root?.addEventListener('touchend', onTouchEnd, { passive: false })
  root?.addEventListener('touchcancel', onTouchCancel)
  setupResizeObserver()
  void load()
})
onBeforeUnmount(() => {
  unmounted = true
  const root = scrollRoot()
  root?.removeEventListener('scroll', onPdfScroll)
  root?.removeEventListener('touchstart', onTouchStart)
  root?.removeEventListener('touchmove', onTouchMove)
  root?.removeEventListener('touchend', onTouchEnd)
  root?.removeEventListener('touchcancel', onTouchCancel)
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
  align-items: flex-start;
  width: max-content;
  min-width: 100%;
  gap: 16px;
}
.pdf-page-wrap {
  position: relative;
  box-sizing: content-box; /* The border must not clip two pixels of PDF content. */
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
.pdf-detail-canvas { position: absolute; pointer-events: none; transition: none; }

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
  z-index: 1;
  --scale-factor: 1;
}
.pdf-text-layer :deep(span), .pdf-text-layer :deep(br) { position: absolute; white-space: pre; transform-origin: 0 0; color: transparent; cursor: text; }
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
  .pdf-viewer { padding: 2px 2px calc(76px + var(--safe-bottom)); touch-action: pan-x pan-y; }
  .pdf-pages { gap: 10px; }
  .pdf-page-wrap { border: 0; border-radius: 2px; box-shadow: none; }
}
</style>
