<template>
  <div class="mermaid-viewer" @click.self="close">
    <!-- Toolbar -->
    <div class="mv-toolbar">
      <span class="mv-title">{{ title }}</span>
      <span class="mv-zoom-label">{{ zoomPct }}%</span>
      <div class="mv-actions">
        <button class="mv-btn" title="缩小" @click="zoomBy(0.8)">
          <el-icon><Minus /></el-icon>
        </button>
        <button class="mv-btn" title="放大" @click="zoomBy(1.25)">
          <el-icon><Plus /></el-icon>
        </button>
        <button class="mv-btn" title="重置" @click="reset">
          <el-icon><Refresh /></el-icon>
        </button>
        <button class="mv-btn mv-close" title="关闭" @click="close">
          <el-icon><Close /></el-icon>
        </button>
      </div>
    </div>

    <!-- Stage -->
    <div ref="stageRef" class="mv-stage">
      <div ref="canvasRef" class="mv-canvas" v-html="svgHtml"></div>
    </div>

    <div class="mv-hint">滚轮缩放 · 拖拽平移 · 双击放大 · Esc 关闭</div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, computed } from 'vue'
import { Plus, Minus, Refresh, Close } from '@element-plus/icons-vue'
import panzoom, { type PanZoom } from 'panzoom'

const props = defineProps<{ svgHtml: string; source: string; title?: string }>()
const emit = defineEmits<{ close: [] }>()

const title = computed(() => props.title || 'Mermaid 图')
const stageRef = ref<HTMLDivElement | null>(null)
const canvasRef = ref<HTMLDivElement | null>(null)
const zoomPct = ref(100)
let pz: PanZoom | null = null

function close() {
  emit('close')
}

function zoomBy(factor: number) {
  if (!pz || !stageRef.value) return
  const r = stageRef.value.getBoundingClientRect()
  pz.smoothZoom(r.width / 2 + r.left, r.height / 2 + r.top, factor)
  // smoothZoom is async; update label after a tick
  setTimeout(updateZoomLabel, 60)
}

/** Size the SVG to fit the stage (explicit px from its viewBox) and (re)init panzoom. */
function fitAndInit() {
  const target = (canvasRef.value?.querySelector('svg') || canvasRef.value?.querySelector('img')) as HTMLElement | null
  if (!target) return
  if (target.tagName === 'svg') {
    const svg = target as unknown as SVGSVGElement
    const vb = svg.viewBox?.baseVal
    if (stageRef.value && vb && vb.width > 0 && vb.height > 0) {
      const scale =
        Math.min(stageRef.value.clientWidth / vb.width, stageRef.value.clientHeight / vb.height) * 0.92
      const w = vb.width * scale
      const h = vb.height * scale
      svg.setAttribute('width', `${w}`)
      svg.setAttribute('height', `${h}`)
      svg.style.width = `${w}px`
      svg.style.height = `${h}px`
      svg.style.maxWidth = 'none'
      svg.style.maxHeight = 'none'
    } else {
      svg.style.maxWidth = '92vw'
      svg.style.maxHeight = '78vh'
      svg.style.width = 'auto'
      svg.style.height = 'auto'
    }
  } else {
    target.style.maxWidth = '92vw'
    target.style.maxHeight = '78vh'
    target.style.width = 'auto'
    target.style.height = 'auto'
  }
  target.style.transform = ''
  target.style.transformOrigin = ''
  if (pz) pz.dispose()
  pz = panzoom(target, {
    maxZoom: 8,
    minZoom: 0.1,
    zoomDoubleClickSpeed: 1.8,
    bounds: false,
  })
  pz.on('zoom', updateZoomLabel)
  zoomPct.value = 100
}

function reset() {
  fitAndInit()
}

function updateZoomLabel() {
  if (!pz) return
  const t = pz.getTransform()
  zoomPct.value = Math.round(t.scale * 100)
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') close()
}

onMounted(() => {
  // The SVG is now in the DOM (v-html). Size it explicitly, then init panzoom.
  fitAndInit()
  window.addEventListener('keydown', onKey)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  if (pz) {
    pz.dispose()
    pz = null
  }
})
</script>

<style scoped>
.mermaid-viewer {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.45);
  backdrop-filter: blur(40px) saturate(180%);
  -webkit-backdrop-filter: blur(40px) saturate(180%);
  animation: mv-fade 0.2s ease;
}
:root[data-theme="dark"] .mermaid-viewer {
  background: rgba(20, 20, 24, 0.45);
}
@keyframes mv-fade {
  from { opacity: 0; }
  to { opacity: 1; }
}

.mv-toolbar {
  flex-shrink: 0;
  height: 56px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 20px;
  background: transparent;
  border-bottom: 1px solid var(--border-faint);
}
.mv-title { font-size: 14px; font-weight: 600; color: var(--text-primary); }
.mv-zoom-label {
  font-size: 12px;
  color: var(--text-muted);
  padding: 3px 10px;
  border-radius: 8px;
  background: var(--bg-glass-subtle);
  min-width: 52px;
  text-align: center;
}
.mv-actions { margin-left: auto; display: flex; gap: 8px; }

.mv-btn {
  width: 34px;
  height: 34px;
  border-radius: 10px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s ease;
}
.mv-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.mv-btn.mv-close:hover { background: rgba(248, 113, 113, 0.15); color: #f87171; border-color: rgba(248, 113, 113, 0.3); }

.mv-stage {
  flex: 1;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.mv-canvas {
  /* holds the SVG; panzoom transforms the svg inside */
  display: flex;
  align-items: center;
  justify-content: center;
}
.mv-canvas :deep(svg) {
  background: transparent;
  cursor: grab;
}
.mv-canvas :deep(svg):active { cursor: grabbing; }

.mv-hint {
  flex-shrink: 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-faint);
  padding: 10px;
}

@media (max-width: 768px) {
  .mv-toolbar { height: 50px; padding: 0 12px; gap: 8px; }
  .mv-title { display: none; }
  .mv-hint { font-size: 11px; padding: 8px; }
  .mv-stage { padding: 12px; }
}
</style>
