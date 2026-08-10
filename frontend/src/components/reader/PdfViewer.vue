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
const appStore = useAppStore()
const theme = ref(appStore.theme)

interface PageMeta { num: number; width: number; height: number }

const scrollRef = ref<HTMLElement | null>(null)
const loading = ref(true)
const error = ref('')
const pageMetas = ref<PageMeta[]>([])
const canvasRefs: Record<number, HTMLCanvasElement | null> = {}
let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null
let baseScale = 1 // fit-width scale derived from container width
let renderedPages = new Set<number>()
let observer: IntersectionObserver | null = null

function setCanvasRef(num: number, el: HTMLCanvasElement | null) {
  canvasRefs[num] = el
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
    const viewport = page.getViewport({ scale: baseScale })
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
  loading.value = true
  error.value = ''
  renderedPages = new Set<number>()
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
  } catch (e) {
    error.value = (e as Error)?.message || 'PDF 解析失败'
    loading.value = false
  }
}

/** Minimal nextTick promise (avoid importing vue's nextTick name clash). */
function nextTickAsync(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

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
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
}
.pdf-canvas { display: block; }

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
