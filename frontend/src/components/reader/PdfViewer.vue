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
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import * as pdfjsLib from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { useAppStore } from '@/stores/app'
import { localFileUrl } from '@/api/reader'

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

const props = defineProps<{ src: string }>()
const emit = defineEmits<{
  outline: [items: { text: string; level: number; page: number }[]]
}>()
const appStore = useAppStore()
const theme = ref(appStore.theme)

interface PageMeta { num: number; width: number; height: number }

const scrollRef = ref<HTMLElement | null>(null)
const loading = ref(true)
const error = ref('')
const pageMetas = ref<PageMeta[]>([])
const canvasRefs: Record<number, HTMLCanvasElement | null> = {}
const textRefs: Record<number, HTMLDivElement | null> = {}
let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null
let baseScale = 1 // fit-width scale derived from container width
let renderedPages = new Set<number>()
let observer: IntersectionObserver | null = null
const zoomMode = ref<'fit' | number>('fit') // 'fit' = fit-width; number = explicit scale factor

function setCanvasRef(num: number, el: HTMLCanvasElement | null) {
  canvasRefs[num] = el
}
function setTextRef(num: number, el: HTMLDivElement | null) {
  textRefs[num] = el
}

function currentScale(): number {
  if (zoomMode.value === 'fit') return baseScale
  return zoomMode.value
}

/** Compute fit-width scale so the PDF page fills the container width. */
function computeFitScale(page: pdfjsLib.PDFPageProxy): number {
  const containerWidth = scrollRef.value?.clientWidth ?? 800
  const viewport0 = page.getViewport({ scale: 1 })
  return containerWidth / viewport0.width
}

/** Render a single page to its canvas (idempotent — skips if already rendered). */
async function renderPage(num: number) {
  if (!pdfDoc || renderedPages.has(num)) return
  const canvas = canvasRefs[num]
  if (!canvas) return
  try {
    const page = await pdfDoc.getPage(num)
    const viewport = page.getViewport({ scale: currentScale() })
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.floor(viewport.width * dpr)
    canvas.height = Math.floor(viewport.height * dpr)
    canvas.style.width = `${viewport.width}px`
    canvas.style.height = `${viewport.height}px`
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const transform = dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined
    await page.render({ canvasContext: ctx, viewport, transform }).promise
    renderedPages.add(num)
    // Best-effort text layer for selection/search; failure is non-fatal.
    try {
      const textContent = await page.getTextContent()
      const textDiv = textRefs[num]
      if (textDiv) {
        textDiv.innerHTML = ''
        textDiv.style.width = `${viewport.width}px`
        textDiv.style.height = `${viewport.height}px`
        // pdf.js v4 TextLayer class
        const textLayer = new pdfjsLib.TextLayer({
          textContentSource: textContent,
          container: textDiv,
          viewport,
        })
        void textLayer.render()
      }
    } catch (e) {
      console.warn(`文字层渲染失败 (第 ${num} 页):`, e)
    }
  } catch (e) {
    console.warn(`渲染第 ${num} 页失败:`, e)
  }
}

/** Set up lazy rendering: observe each page wrapper, render when near viewport. */
function setupObserver() {
  if (!scrollRef.value) return
  observer?.disconnect()
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const num = Number((entry.target as HTMLElement).dataset.pageNum)
          void renderPage(num)
        }
      }
    },
    { root: null, rootMargin: '800px 0px' },
  )
  const wraps = scrollRef.value.querySelectorAll<HTMLElement>('.pdf-page-wrap')
  wraps.forEach((w) => observer?.observe(w))
}

async function load() {
  // Destroy the prior document so switching props.src doesn't leak the
  // previous pdf.js worker + memory. onBeforeUnmount handles unmount.
  if (pdfDoc) { void pdfDoc.destroy(); pdfDoc = null }
  renderedPages = new Set<number>()
  loading.value = true
  error.value = ''
  pageMetas.value = []
  try {
    const task = pdfjsLib.getDocument(localFileUrl(props.src))
    pdfDoc = await task.promise
    // Use page 1 to derive the fit-width scale; record every page's placeholder
    // size at that scale so the scroll area has correct height before render.
    const page1 = await pdfDoc.getPage(1)
    baseScale = computeFitScale(page1)
    const vp1 = page1.getViewport({ scale: baseScale })
    const metas: PageMeta[] = []
    for (let i = 1; i <= pdfDoc.numPages; i++) {
      // Assume uniform page size (common case); page 1 dimensions for all.
      // Non-uniform PDFs will have slightly mismatched placeholders — acceptable
      // for v1; lazy render corrects the actual canvas size on render.
      metas.push({ num: i, width: vp1.width, height: vp1.height })
    }
    pageMetas.value = metas
    loading.value = false
    // Wait for placeholders to mount, then observe + render visible pages.
    await nextTickAsync()
    setupObserver()
    // Render the first page immediately so the user sees content at once.
    await renderPage(1)
    // Extract PDF outline (bookmarks/TOC) and emit for the parent (Task 7 TOC).
    const rawOutline = await pdfDoc.getOutline()
    const outline = await buildOutline(rawOutline, 1)
    emit('outline', outline)
  } catch (e) {
    error.value = (e as Error)?.message || 'PDF 解析失败'
    loading.value = false
  }
}

/** Minimal nextTick promise (avoid importing vue's nextTick name clash). */
function nextTickAsync(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

// pdfjs-dist v4 does not export OutlineNode/ExplicitDest as named types
// (they exist only as JSDoc typedefs). Use any for outline internals.
async function buildOutline(
  raw: any[] | null,
  level: number,
): Promise<{ text: string; level: number; page: number }[]> {
  if (!raw || !raw.length) return []
  const out: { text: string; level: number; page: number }[] = []
  for (const node of raw) {
    let page = 1
    try {
      // dest may be a named dest (string) or an explicit dest array.
      // Resolve named dests to the explicit array form via getDestination.
      let dest: any = node.dest
      if (typeof dest === 'string' && pdfDoc) {
        dest = await pdfDoc.getDestination(dest)
      }
      // Explicit dest array: dest[0] is the page RefProxy ({num, gen}).
      // pdf.js v4 getPageIndex validates the arg is a RefProxy (isRefProxy),
      // so we must pass dest[0], not the whole dest array.
      const ref = Array.isArray(dest) ? dest[0] : null
      if (ref && pdfDoc) {
        const idx = await pdfDoc.getPageIndex(ref)
        if (typeof idx === 'number') page = idx + 1
      }
    } catch {
      page = 1
    }
    out.push({ text: node.title, level, page })
    if (node.items?.length) {
      out.push(...await buildOutline(node.items, level + 1))
    }
  }
  return out
}

/** Re-render all already-observed pages at the current scale (zoom change). */
async function rerenderAll() {
  if (!pdfDoc) return
  // Recompute placeholder sizes for the new scale.
  const page1 = await pdfDoc.getPage(1)
  const s = zoomMode.value === 'fit' ? computeFitScale(page1) : zoomMode.value
  baseScale = s
  const vp = page1.getViewport({ scale: s })
  pageMetas.value = pageMetas.value.map((p) => ({ ...p, width: vp.width, height: vp.height }))
  renderedPages = new Set<number>()
  await nextTickAsync()
  // Re-render currently visible pages.
  const visible = scrollRef.value?.querySelectorAll<HTMLElement>('.pdf-page-wrap') ?? []
  for (const w of Array.from(visible)) {
    const num = Number(w.dataset.pageNum)
    void renderPage(num)
  }
}

function setZoom(mode: 'fit' | number) {
  zoomMode.value = mode
  void rerenderAll()
}

function scrollToPage(num: number) {
  const el = scrollRef.value?.querySelector<HTMLElement>(`.pdf-page-wrap[data-page-num="${num}"]`)
  el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ scrollToPage, setZoom })

watch(() => props.src, () => { void load() })
watch(() => appStore.theme, (t) => { theme.value = t })

onMounted(() => { void load() })
onBeforeUnmount(() => {
  observer?.disconnect()
  void pdfDoc?.destroy()
  pdfDoc = null
})
</script>

<style scoped>
.pdf-viewer {
  /* NOT its own scroll container — flows in .pane-center so onContentScroll
     (FAB hide, mobile header collapse) and page-turn transitions are reused. */
  padding: 12px 20px 120px;
}
/* Theme color via CSS filter on the canvas pages — no re-render needed. */
.pdf-theme-light .pdf-pages { filter: none; }
.pdf-theme-dark .pdf-pages { filter: invert(1) hue-rotate(180deg); }
.pdf-theme-eye-care .pdf-pages { filter: sepia(0.4) brightness(0.96) saturate(0.85); }

.pdf-pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}
.pdf-page-wrap {
  position: relative;
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
}
.pdf-canvas { display: block; }

.pdf-text-layer {
  position: absolute;
  inset: 0;
  overflow: hidden;
  line-height: 1;
  opacity: 0.25;
  /* text layer must not invert with the canvas filter — counter-filter */
}
.pdf-theme-dark .pdf-text-layer { filter: invert(1) hue-rotate(180deg); opacity: 1; }
.pdf-theme-eye-care .pdf-text-layer { filter: sepia(0.4) brightness(0.96) saturate(0.85); opacity: 1; }
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
</style>
