<template>
  <Teleport to="body">
    <Transition :name="`motion-drawer-${direction}`">
      <div v-if="modelValue" class="motion-drawer" :class="{ 'is-dragging': dragging }">
        <div class="motion-drawer__scrim" :style="scrimStyle" aria-hidden="true" @click="close" />
        <aside
          ref="panelRef"
          class="motion-drawer__panel"
          :class="`is-${direction}`"
          :style="panelStyle"
          role="dialog"
          aria-modal="true"
          :aria-label="ariaLabel"
          tabindex="-1"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerCancel"
        >
          <div class="motion-drawer__grabber" aria-hidden="true" />
          <slot />
        </aside>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useModalEnvironment } from '@/composables/useModalEnvironment'

const props = withDefaults(defineProps<{
  modelValue: boolean
  direction?: 'left' | 'right'
  ariaLabel?: string
}>(), {
  direction: 'left',
  ariaLabel: '侧边面板',
})

const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const panelRef = ref<HTMLElement | null>(null)
const dragging = ref(false)
const dragX = ref(0)
let pointerId: number | null = null
let startX = 0
let startY = 0
let axis: 'pending' | 'horizontal' | 'vertical' = 'pending'
let samples: Array<{ x: number; time: number }> = []

const directionSign = computed(() => props.direction === 'left' ? -1 : 1)
const panelStyle = computed(() => ({ '--motion-drawer-x': `${dragX.value}px` }))
const scrimStyle = computed(() => {
  const width = panelRef.value?.offsetWidth || 320
  const progress = Math.max(0, 1 - Math.abs(dragX.value) / width)
  return { opacity: String(progress) }
})

function rubberband(overshoot: number, dimension: number, constant = 0.55) {
  return (overshoot * dimension * constant) / (dimension + constant * Math.abs(overshoot))
}

function projectVelocity(velocity: number, decelerationRate = 0.99) {
  return (velocity / 1000) * decelerationRate / (1 - decelerationRate)
}

function close() {
  emit('update:modelValue', false)
}

function onPointerDown(event: PointerEvent) {
  if (event.pointerType === 'mouse' && event.button !== 0) return
  pointerId = event.pointerId
  startX = event.clientX
  startY = event.clientY
  axis = 'pending'
  samples = [{ x: event.clientX, time: performance.now() }]
}

function onPointerMove(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  const dx = event.clientX - startX
  const dy = event.clientY - startY
  if (axis === 'pending') {
    if (Math.max(Math.abs(dx), Math.abs(dy)) < 10) return
    axis = Math.abs(dx) > Math.abs(dy) ? 'horizontal' : 'vertical'
    if (axis === 'vertical') {
      resetGesture()
      return
    }
    dragging.value = true
    panelRef.value?.setPointerCapture(event.pointerId)
  }

  event.preventDefault()
  const closingDistance = dx * directionSign.value
  const width = panelRef.value?.offsetWidth || 320
  dragX.value = closingDistance >= 0
    ? dx
    : rubberband(dx, width, 0.25)
  const now = performance.now()
  samples.push({ x: event.clientX, time: now })
  samples = samples.filter((sample) => now - sample.time <= 100)
}

function finishGesture() {
  const first = samples[0]
  const last = samples[samples.length - 1]
  const elapsed = first && last ? Math.max(1, last.time - first.time) : 1
  const velocity = first && last ? ((last.x - first.x) / elapsed) * 1000 : 0
  const projected = dragX.value + projectVelocity(velocity)
  const width = panelRef.value?.offsetWidth || 320
  const shouldClose = projected * directionSign.value > width * 0.35
  if (pointerId !== null && panelRef.value?.hasPointerCapture(pointerId)) {
    panelRef.value.releasePointerCapture(pointerId)
  }
  pointerId = null
  dragging.value = false
  if (shouldClose) close()
  else dragX.value = 0
}

function resetGesture() {
  if (pointerId !== null && panelRef.value?.hasPointerCapture(pointerId)) {
    panelRef.value.releasePointerCapture(pointerId)
  }
  pointerId = null
  dragging.value = false
  dragX.value = 0
}

function onPointerUp(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  if (axis === 'horizontal') finishGesture()
  else resetGesture()
}

function onPointerCancel(event: PointerEvent) {
  if (event.pointerId === pointerId) resetGesture()
}

watch(() => props.modelValue, (open) => {
  if (open) resetGesture()
})
useModalEnvironment(() => props.modelValue, panelRef, close)
onBeforeUnmount(() => {
  resetGesture()
})
</script>

<style scoped>
.motion-drawer {
  position: fixed;
  inset: 0;
  z-index: 2200;
}

.motion-drawer__scrim {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.28);
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}

.motion-drawer__panel {
  position: absolute;
  top: 0;
  bottom: 0;
  width: min(78vw, 360px);
  overflow: hidden auto;
  overscroll-behavior: contain;
  background: var(--bg-glass-strong);
  backdrop-filter: blur(28px) saturate(180%);
  -webkit-backdrop-filter: blur(28px) saturate(180%);
  border: 1px solid var(--border-glass);
  box-shadow: var(--shadow-lg), var(--inset-highlight);
  transform: translate3d(var(--motion-drawer-x), 0, 0);
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
  touch-action: pan-y;
  outline: none;
  padding-top: env(safe-area-inset-top);
  padding-bottom: env(safe-area-inset-bottom);
}

.motion-drawer__panel.is-left { left: 0; border-radius: 0 22px 22px 0; }
.motion-drawer__panel.is-right { right: 0; border-radius: 22px 0 0 22px; }

.motion-drawer__grabber {
  position: absolute;
  top: 50%;
  width: 4px;
  height: 42px;
  border-radius: 999px;
  background: var(--text-faint);
  opacity: 0.28;
  transform: translateY(-50%);
}
.is-left .motion-drawer__grabber { right: 5px; }
.is-right .motion-drawer__grabber { left: 5px; }

.is-dragging .motion-drawer__panel,
.is-dragging .motion-drawer__scrim { transition: none; }

.motion-drawer-left-enter-active,
.motion-drawer-left-leave-active,
.motion-drawer-right-enter-active,
.motion-drawer-right-leave-active {
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}
.motion-drawer-left-enter-active .motion-drawer__panel,
.motion-drawer-left-leave-active .motion-drawer__panel,
.motion-drawer-right-enter-active .motion-drawer__panel,
.motion-drawer-right-leave-active .motion-drawer__panel {
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
}
.motion-drawer-left-enter-from,
.motion-drawer-left-leave-to,
.motion-drawer-right-enter-from,
.motion-drawer-right-leave-to { opacity: 0; }
.motion-drawer-left-enter-from .motion-drawer__panel,
.motion-drawer-left-leave-to .motion-drawer__panel { transform: translate3d(-105%, 0, 0); }
.motion-drawer-right-enter-from .motion-drawer__panel,
.motion-drawer-right-leave-to .motion-drawer__panel { transform: translate3d(105%, 0, 0); }

@media (prefers-reduced-motion: reduce) {
  .motion-drawer__panel { transform: none !important; }
}
</style>
