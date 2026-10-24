<template>
  <Teleport to="body" :disabled="isNativeFullscreen">
    <section ref="panelRef" class="mermaid-viewer" role="dialog" aria-modal="true" :aria-label="title" :aria-description="viewerPolicy.hint" tabindex="-1">
      <div v-glass-lens="'css'" class="mv-close-surface" data-glass="floating">
        <button data-icon-only type="button" class="mv-button mv-close" aria-label="关闭图表查看器" title="关闭" @click="close"><el-icon><Close /></el-icon></button>
      </div>

      <div ref="stageRef" class="mv-stage" :aria-busy="rendering" @dblclick.self="setView('fit')" @keydown="onStageKey">
        <div ref="canvasRef" class="mv-canvas" v-html="displayHtml"></div>
      </div>

      <footer class="mv-footer">
        <div v-glass-lens="'css'" class="mv-controls" data-glass="floating">
          <div class="mv-view-modes" aria-label="图表显示方式">
            <button type="button" :class="{ active: viewMode === 'readable' }" :aria-pressed="viewMode === 'readable'" @click="setView('readable')">可读尺寸</button>
            <button type="button" :class="{ active: viewMode === 'fit' }" :aria-pressed="viewMode === 'fit'" @click="setView('fit')">完整查看</button>
          </div>
          <span class="mv-divider" aria-hidden="true"></span>
          <button data-icon-only type="button" class="mv-button" aria-label="缩小图表" title="缩小" @click="zoomBy(0.8)"><el-icon><Minus /></el-icon></button>
          <output class="mv-zoom-label" aria-label="图表缩放比例">{{ zoomPct }}%</output>
          <button data-icon-only type="button" class="mv-button" aria-label="放大图表" title="放大" @click="zoomBy(1.25)"><el-icon><Plus /></el-icon></button>
        </div>
      </footer>
    </section>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Close, Minus, Plus } from '@element-plus/icons-vue'
import panzoom, { type PanZoom } from 'panzoom'
import { useAppStore } from '@/stores/app'
import { useModalEnvironment } from '@/composables/useModalEnvironment'
import { cloneDiagramSvg, renderMermaidSvg } from '@/markdown/mermaidRuntime'
import { getMermaidViewerPolicy } from '@/utils/mermaidViewerPolicy'
import { constrainDiagramTransform, getDiagramViewport, type DiagramSize, type DiagramViewMode } from '@/utils/mermaidViewport'

const props = defineProps<{ svgHtml: string; source: string; title?: string }>()
const emit = defineEmits<{ close: [] }>()
const appStore = useAppStore()
const title = computed(() => props.title || 'Mermaid 图')
const panelRef = ref<HTMLElement | null>(null)
const stageRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLDivElement | null>(null)
const displayHtml = ref('')
const zoomPct = ref(100)
const viewportWidth = ref(window.innerWidth)
const isNativeFullscreen = ref(Boolean(document.fullscreenElement))
const viewerPolicy = computed(() => getMermaidViewerPolicy(viewportWidth.value))
const viewMode = ref<DiagramViewMode>('readable')
const rendering = ref(false)
let diagramSize: DiagramSize = { width: 1, height: 1 }
let pz: PanZoom | null = null
let resizeObserver: ResizeObserver | null = null
let resizeFrame = 0
let generation = 0
let mountGeneration = 0
let disposed = false

function close() { emit('close') }
useModalEnvironment(() => true, panelRef, close)

function stageSize() {
  return { width: stageRef.value?.clientWidth || 1, height: stageRef.value?.clientHeight || 1 }
}

function syncTransform() {
  if (!pz) return
  const current = pz.getTransform()
  const bounded = constrainDiagramTransform(current, diagramSize, stageSize())
  if (Math.abs(current.x - bounded.x) > 0.5 || Math.abs(current.y - bounded.y) > 0.5) pz.moveTo(bounded.x, bounded.y)
  zoomPct.value = Math.round(current.scale * 100)
}

function setView(mode: DiagramViewMode) {
  viewMode.value = mode
  if (!pz) return
  const position = getDiagramViewport(diagramSize, stageSize(), mode)
  // Zoom and translation are committed together before the next paint. There
  // is no transition on the canvas: touch and pinch always track the pointer.
  pz.zoomAbs(0, 0, position.scale)
  pz.moveTo(position.x, position.y)
  zoomPct.value = Math.round(position.scale * 100)
}

function zoomBy(factor: number) {
  if (!pz || !stageRef.value) return
  const size = stageSize()
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) pz.zoomTo(size.width / 2, size.height / 2, factor)
  else pz.smoothZoom(size.width / 2, size.height / 2, factor)
  viewMode.value = 'readable'
}

function onStageKey(event: KeyboardEvent) {
  if (!pz) return
  const offsets: Record<string, [number, number]> = { ArrowLeft: [64, 0], ArrowRight: [-64, 0], ArrowUp: [0, 64], ArrowDown: [0, -64] }
  const offset = offsets[event.key]
  if (offset) { event.preventDefault(); pz.moveBy(offset[0], offset[1], false) }
  else if (event.key === 'Home') { event.preventDefault(); setView('fit') }
}

async function mountDiagram(html: string) {
  const version = ++mountGeneration
  pz?.dispose()
  pz = null
  const template = document.createElement('template')
  template.innerHTML = html
  const svg = template.content.querySelector('svg')
  displayHtml.value = svg ? cloneDiagramSvg(svg) : html
  await nextTick()
  if (disposed || version !== mountGeneration || !canvasRef.value || !stageRef.value) return
  const target = canvasRef.value.querySelector('svg, img')
  if (target instanceof SVGSVGElement) {
    const box = target.viewBox.baseVal
    diagramSize = { width: box.width || target.width.baseVal.value || 600, height: box.height || target.height.baseVal.value || 400 }
    target.setAttribute('width', `${diagramSize.width}`)
    target.setAttribute('height', `${diagramSize.height}`)
    target.style.width = `${diagramSize.width}px`
    target.style.height = `${diagramSize.height}px`
    target.style.maxWidth = 'none'
    target.style.maxHeight = 'none'
  } else if (target instanceof HTMLImageElement) {
    try { await target.decode() } catch { /* Keep the image's native error state. */ }
    if (disposed || version !== mountGeneration) return
    diagramSize = { width: target.naturalWidth || 600, height: target.naturalHeight || 400 }
    viewMode.value = 'fit'
    target.style.width = `${diagramSize.width}px`
    target.style.height = `${diagramSize.height}px`
  } else return
  Object.assign(canvasRef.value.style, { width: `${diagramSize.width}px`, height: `${diagramSize.height}px`, transformOrigin: '0 0' })
  const initial = getDiagramViewport(diagramSize, stageSize(), viewMode.value)
  pz = panzoom(canvasRef.value, {
    minZoom: Math.min(0.1, initial.fitScale), maxZoom: 6,
    initialZoom: initial.scale, initialX: initial.x, initialY: initial.y,
    zoomDoubleClickSpeed: 1.6, zoomSpeed: 0.12, pinchSpeed: 1,
    smoothScroll: !window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    filterKey: () => true,
  })
  pz.on('transform', syncTransform)
  zoomPct.value = Math.round(initial.scale * 100)
}

watch(() => props.svgHtml, html => { generation += 1; void mountDiagram(html) })
watch(() => appStore.theme, async theme => {
  if (!props.source || !canvasRef.value?.querySelector('svg')) return
  const version = ++generation
  rendering.value = true
  try {
    const svg = await renderMermaidSvg(props.source, { theme, isCurrent: () => !disposed && generation === version })
    if (svg && generation === version) await mountDiagram(svg)
  } catch (error) { console.warn('图表主题更新失败，保留当前图表', error) }
  finally { if (generation === version) rendering.value = false }
})

function onFullscreenChange() { isNativeFullscreen.value = Boolean(document.fullscreenElement) }

onMounted(() => {
  void mountDiagram(props.svgHtml)
  resizeObserver = new ResizeObserver(() => {
    cancelAnimationFrame(resizeFrame)
    resizeFrame = requestAnimationFrame(() => { viewportWidth.value = window.innerWidth; setView(viewMode.value) })
  })
  if (stageRef.value) resizeObserver.observe(stageRef.value)
  document.addEventListener('fullscreenchange', onFullscreenChange)
})
onBeforeUnmount(() => {
  disposed = true
  generation += 1
  cancelAnimationFrame(resizeFrame)
  resizeObserver?.disconnect()
  pz?.dispose()
  document.removeEventListener('fullscreenchange', onFullscreenChange)
})
</script>

<style scoped>
.mermaid-viewer {
  position: fixed; inset: 0; z-index: 3000;
  background: var(--bg-base); color: var(--text-primary); outline: none;
  backdrop-filter: var(--glass-content-filter);
  -webkit-backdrop-filter: var(--glass-content-filter);
  animation: diagram-viewer-enter 220ms var(--ease-out);
}
.mv-close-surface { position: absolute; z-index: 2; top: calc(16px + var(--safe-top)); right: calc(16px + var(--safe-right)); border-radius: 50%; }
.mv-stage { position: absolute; inset: 0; overflow: hidden; touch-action: none; overscroll-behavior: contain; }
.mv-canvas { position: absolute; top: 0; left: 0; cursor: grab; transform-origin: 0 0; user-select: none; will-change: transform; }
.mv-canvas:active { cursor: grabbing; }
.mv-canvas :deep(svg) { display: block; overflow: visible; background: transparent; }
.mv-footer { position: absolute; z-index: 2; inset-inline: 0; bottom: calc(20px + var(--safe-bottom)); display: flex; justify-content: center; padding-inline: max(12px, var(--safe-left)) max(12px, var(--safe-right)); pointer-events: none; }
.mv-controls { position: relative; display: flex; align-items: center; gap: 2px; padding: 5px 7px; border-radius: 999px; max-width: 100%; pointer-events: auto; }
.mv-view-modes { display: flex; gap: 2px; }
.mv-view-modes button { min-height: 40px; padding: 0 13px; border: 0; border-radius: 999px; background: transparent; color: var(--text-muted); font: 600 13px var(--font-sans); cursor: pointer; white-space: nowrap; transition: background-color var(--motion-fast), color var(--motion-fast); }
.mv-view-modes button.active { background: var(--bg-hover); color: var(--text-primary); }
.mv-divider { width: 1px; height: 20px; background: var(--border-faint); margin: 0 7px; }
.mv-button { flex: none; width: 40px; height: 40px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-secondary); display: grid; place-items: center; font-size: 19px; cursor: pointer; touch-action: manipulation; transition: transform 120ms var(--ease-out), background-color 140ms ease; }
.mv-button:hover { background: var(--bg-hover); }
.mv-button:active, .mv-view-modes button:active { transform: scale(.94); }
.mv-close { width: 44px; height: 44px; }
.mv-zoom-label { width: 48px; text-align: center; font-size: 12px; font-variant-numeric: tabular-nums; color: var(--text-muted); }
.mv-button:focus-visible, .mv-view-modes button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@keyframes diagram-viewer-enter { from { opacity: 0; } to { opacity: 1; } }
@media (max-width: 768px) {
  .mv-close-surface { top: calc(12px + var(--safe-top)); right: calc(12px + var(--safe-right)); }
  .mv-footer { bottom: calc(16px + var(--safe-bottom)); }
  .mv-controls { gap: 0; padding: 4px; }
  .mv-button, .mv-view-modes button { min-height: 44px; }
  .mv-button { width: 44px; height: 44px; }
  .mv-view-modes button { padding-inline: 11px; font-size: 12px; }
  .mv-divider { margin-inline: 2px; }
  .mv-zoom-label { width: 40px; }
}
@media (max-width: 350px) {
  .mv-view-modes button { padding-inline: 8px; }
  .mv-button { width: 40px; }
  .mv-zoom-label { width: 34px; }
}
@media (prefers-reduced-motion: reduce) { .mermaid-viewer { animation: none; } .mv-button, .mv-view-modes button { transition: none; } }
</style>
