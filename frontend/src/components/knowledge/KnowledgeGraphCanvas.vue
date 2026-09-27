<template>
  <section class="graph-canvas-shell" aria-label="知识关系图谱">
    <header>
      <div>
        <strong>关系画布</strong>
        <span>{{ snapshot.entries.length }} / {{ snapshot.total_entries }} 个实体 · {{ snapshot.relations.length }} 条可见关系</span>
      </div>
      <div class="canvas-controls">
        <button type="button" aria-label="缩小图谱" @click="zoomBy(-0.18)">−</button>
        <button type="button" aria-label="重置图谱视角" @click="resetView">{{ Math.round(view.scale * 100) }}%</button>
        <button type="button" aria-label="放大图谱" @click="zoomBy(0.18)">＋</button>
      </div>
    </header>
    <div
      class="graph-stage"
      :class="{ 'is-dragging': dragging }"
      @wheel.prevent="onWheel"
      @pointerdown="startPan"
      @pointermove="movePan"
      @pointerup="endPan"
      @pointercancel="endPan"
    >
      <svg viewBox="0 0 1000 620" role="img" aria-label="可拖动缩放的知识关系图">
        <defs>
          <linearGradient id="knowledge-graph-edge" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="var(--accent)" stop-opacity=".16" />
            <stop offset="1" stop-color="var(--accent)" stop-opacity=".48" />
          </linearGradient>
          <filter id="knowledge-graph-glow" x="-80%" y="-80%" width="260%" height="260%">
            <feGaussianBlur stdDeviation="7" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>
        <g :transform="`translate(${view.x} ${view.y}) scale(${view.scale})`">
          <line
            v-for="edge in positionedEdges"
            :key="edge.id"
            :x1="edge.from.x"
            :y1="edge.from.y"
            :x2="edge.to.x"
            :y2="edge.to.y"
            :class="{ 'is-highlighted': pathEdgeIds.has(edge.id) }"
          >
            <title>{{ edge.relation_type }}{{ edge.evidence ? ` · ${edge.evidence}` : '' }}</title>
          </line>
          <g
            v-for="node in positionedNodes"
            :key="node.id"
            class="graph-node"
            :class="{ 'is-selected': node.id === selectedId, 'is-on-path': pathIds.includes(node.id) }"
            :transform="`translate(${node.x} ${node.y})`"
            role="button"
            tabindex="0"
            @pointerdown.stop
            @click.stop="$emit('select', node.entry)"
            @keydown.enter.prevent="$emit('select', node.entry)"
            @keydown.space.prevent="$emit('select', node.entry)"
          >
            <circle class="node-halo" :r="node.radius + 10" />
            <circle class="node-core" :r="node.radius" />
            <text text-anchor="middle" dominant-baseline="central">{{ shortTitle(node.entry.title) }}</text>
            <title>{{ node.entry.title }} · {{ node.degree }} 个连接</title>
          </g>
        </g>
      </svg>
      <div v-if="snapshot.truncated" class="canvas-notice">为保持流畅，仅显示连接度最高的 {{ snapshot.entries.length }} 个实体</div>
      <div class="canvas-hint">拖动画布 · 滚轮缩放 · 点击实体查看详情</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import type { KnowledgeEntrySummary, KnowledgeGraphSnapshot } from '@/api/knowledge'

const props = withDefaults(defineProps<{
  snapshot: KnowledgeGraphSnapshot
  selectedId?: string
  pathIds?: string[]
}>(), {
  selectedId: '',
  pathIds: () => [],
})

defineEmits<{ select: [entry: KnowledgeEntrySummary] }>()

interface PositionedNode {
  id: string
  entry: KnowledgeEntrySummary
  degree: number
  radius: number
  x: number
  y: number
}

const view = reactive({ x: 0, y: 0, scale: 1 })
const dragging = ref(false)
let pointerId = -1
let dragX = 0
let dragY = 0
let originX = 0
let originY = 0

const degrees = computed(() => {
  const result = new Map<string, number>()
  for (const edge of props.snapshot.relations) {
    result.set(edge.from_entry_id, (result.get(edge.from_entry_id) || 0) + 1)
    result.set(edge.to_entry_id, (result.get(edge.to_entry_id) || 0) + 1)
  }
  return result
})

const positionedNodes = computed<PositionedNode[]>(() => {
  const entries = [...props.snapshot.entries].sort((left, right) => {
    const degreeDelta = (degrees.value.get(right.id) || 0) - (degrees.value.get(left.id) || 0)
    return degreeDelta || left.title.localeCompare(right.title, 'zh-CN')
  })
  if (!entries.length) return []
  const center = { x: 500, y: 310 }
  return entries.map((entry, index) => {
    const degree = degrees.value.get(entry.id) || 0
    if (index === 0) return { id: entry.id, entry, degree, radius: 34, ...center }
    const ring = Math.floor(Math.sqrt(index - 1)) + 1
    const ringStart = ring * ring
    const count = Math.max(6, ring * 7)
    const slot = index - ringStart
    const angle = (slot / count) * Math.PI * 2 + ring * 0.42
    const distance = Math.min(250, 82 + ring * 72)
    return {
      id: entry.id,
      entry,
      degree,
      radius: Math.min(30, 17 + Math.sqrt(degree) * 3.5),
      x: center.x + Math.cos(angle) * distance,
      y: center.y + Math.sin(angle) * distance * 0.72,
    }
  })
})

const nodeMap = computed(() => new Map(positionedNodes.value.map(node => [node.id, node])))
const positionedEdges = computed(() => props.snapshot.relations.flatMap(edge => {
  const from = nodeMap.value.get(edge.from_entry_id)
  const to = nodeMap.value.get(edge.to_entry_id)
  return from && to ? [{ ...edge, from, to }] : []
}))
const pathEdgeIds = computed(() => {
  const pairs = new Set<string>()
  for (let index = 1; index < props.pathIds.length; index += 1) {
    pairs.add([props.pathIds[index - 1], props.pathIds[index]].sort().join(':'))
  }
  return new Set(positionedEdges.value
    .filter(edge => pairs.has([edge.from_entry_id, edge.to_entry_id].sort().join(':')))
    .map(edge => edge.id))
})

function shortTitle(title: string) {
  return title.length > 8 ? `${title.slice(0, 7)}…` : title
}

function zoomBy(delta: number) {
  view.scale = Math.min(2.2, Math.max(0.55, view.scale + delta))
}

function resetView() {
  view.x = 0
  view.y = 0
  view.scale = 1
}

function onWheel(event: WheelEvent) {
  zoomBy(event.deltaY > 0 ? -0.09 : 0.09)
}

function startPan(event: PointerEvent) {
  const target = event.currentTarget as HTMLElement
  dragging.value = true
  pointerId = event.pointerId
  dragX = event.clientX
  dragY = event.clientY
  originX = view.x
  originY = view.y
  target.setPointerCapture(event.pointerId)
}

function movePan(event: PointerEvent) {
  if (!dragging.value || event.pointerId !== pointerId) return
  view.x = originX + (event.clientX - dragX) / view.scale
  view.y = originY + (event.clientY - dragY) / view.scale
}

function endPan(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  dragging.value = false
  pointerId = -1
}
</script>

<style scoped>
.graph-canvas-shell { display: grid; gap: 9px; }
.graph-canvas-shell > header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.graph-canvas-shell > header > div:first-child { display: grid; gap: 2px; }
.graph-canvas-shell strong { font-size: 13px; }
.graph-canvas-shell header span { color: var(--text-faint); font-size: 10px; }
.canvas-controls { display: flex; align-items: center; gap: 4px; padding: 3px; border: 1px solid var(--border-faint); border-radius: 11px; background: var(--bg-glass-subtle); }
.canvas-controls button { min-width: 31px; height: 29px; border: 0; border-radius: 8px; background: transparent; color: var(--text-secondary); cursor: pointer; }
.canvas-controls button:nth-child(2) { min-width: 48px; color: var(--text-faint); font-size: 9px; }
.canvas-controls button:hover { background: var(--bg-hover); color: var(--accent); }
.graph-stage { position: relative; height: min(58dvh, 620px); min-height: 430px; overflow: hidden; border: 1px solid var(--border-faint); border-radius: 18px; background: radial-gradient(circle at 50% 48%, color-mix(in srgb, var(--accent) 8%, transparent), transparent 38%), var(--bg-glass-subtle); cursor: grab; touch-action: none; }
.graph-stage.is-dragging { cursor: grabbing; }
svg { width: 100%; height: 100%; display: block; }
line { stroke: url(#knowledge-graph-edge); stroke-width: 1.4; transition: stroke-width var(--motion-fast) ease, opacity var(--motion-fast) ease; }
line.is-highlighted { stroke: var(--accent); stroke-width: 3.5; filter: url(#knowledge-graph-glow); }
.graph-node { color: var(--text-primary); cursor: pointer; outline: none; }
.node-halo { fill: color-mix(in srgb, var(--accent) 7%, transparent); opacity: 0; transition: opacity var(--motion-fast) ease; }
.node-core { fill: color-mix(in srgb, var(--bg-surface) 92%, var(--accent)); stroke: color-mix(in srgb, var(--accent) 36%, var(--border-subtle)); stroke-width: 1.5; transition: transform var(--motion-fast) var(--ease-spring-gentle), fill var(--motion-fast) ease, stroke-width var(--motion-fast) ease; }
.graph-node text { max-width: 80px; fill: currentColor; font-size: 10px; font-weight: 650; pointer-events: none; }
.graph-node:hover .node-halo, .graph-node:focus-visible .node-halo, .graph-node.is-selected .node-halo, .graph-node.is-on-path .node-halo { opacity: 1; }
.graph-node:hover .node-core, .graph-node:focus-visible .node-core { transform: scale(1.08); transform-box: fill-box; transform-origin: center; }
.graph-node.is-selected .node-core, .graph-node.is-on-path .node-core { fill: var(--accent); stroke: color-mix(in srgb, var(--accent) 60%, white); stroke-width: 2.5; filter: url(#knowledge-graph-glow); }
.graph-node.is-selected, .graph-node.is-on-path { color: white; }
.canvas-notice, .canvas-hint { position: absolute; bottom: 11px; padding: 6px 9px; border: 1px solid var(--border-faint); border-radius: 9px; background: color-mix(in srgb, var(--bg-surface) 82%, transparent); backdrop-filter: var(--glass-floating-filter); color: var(--text-faint); font-size: 9px; pointer-events: none; }
.canvas-notice { left: 11px; color: var(--accent); }
.canvas-hint { right: 11px; }
@media (max-width: 768px) { .graph-canvas-shell { display: none; } }
@media (prefers-reduced-motion: reduce) { line, .node-halo, .node-core { transition: none; } }
</style>
