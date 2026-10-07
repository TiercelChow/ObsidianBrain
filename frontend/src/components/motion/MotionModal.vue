<template>
  <Teleport to="body">
      <div
        v-if="present"
        class="motion-modal"
        :class="{ 'is-dragging': dragging, 'is-settling': settling }"
        :style="overlayStyle"
        @click.self="close"
      >
        <section
          ref="panelRef"
          class="motion-modal__panel"
          :class="{ 'is-wide': size === 'wide' }"
          :style="{ transform: panelTransform, opacity: isMobile() ? 1 : opacity }"
          role="dialog"
          aria-modal="true"
          :inert="!modelValue ? true : undefined"
          :aria-label="ariaLabel"
          tabindex="-1"
        >
          <div
            class="motion-modal__handle"
            aria-hidden="true"
            @pointerdown="onPointerDown"
            @pointermove="onPointerMove"
            @pointerup="onPointerUp"
            @pointercancel="cancelGesture"
          ><span /></div>
          <MotionContent :live="modelValue"><slot /></MotionContent>
        </section>
      </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useModalEnvironment } from '@/composables/useModalEnvironment'
import { useSurfaceMotion } from '@/composables/useSurfaceMotion'
import MotionContent from './MotionContent'
import { projectMotion } from '@/utils/motionSpring'
import { panelDragPosition } from '@/utils/panelGesture'

const props = withDefaults(defineProps<{
  modelValue: boolean
  ariaLabel?: string
  size?: 'regular' | 'wide'
}>(), {
  ariaLabel: '对话框',
  size: 'regular',
})
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const panelRef = ref<HTMLElement | null>(null)
const dragging = ref(false)
const { present, moving: settling, offset: dragY, opacity, panelTransform, grab, follow, handoff, settle } = useSurfaceMotion(() => props.modelValue, panelRef, 'y')
const visualHeight = ref(window.visualViewport?.height ?? window.innerHeight)
const visualOffsetTop = ref(window.visualViewport?.offsetTop ?? 0)
let pointerId: number | null = null
let startY = 0
let grabY = 0
let samples: Array<{ y: number; time: number }> = []
let captureTarget: HTMLElement | null = null

const overlayStyle = computed(() => ({
  '--motion-viewport-height': `${visualHeight.value}px`,
  '--motion-viewport-top': `${visualOffsetTop.value}px`,
  '--motion-scrim-opacity': opacity.value,
}))

function isMobile() { return window.matchMedia('(max-width: 768px)').matches }
function close() { emit('update:modelValue', false) }

function updateVisualViewport() {
  visualHeight.value = window.visualViewport?.height ?? window.innerHeight
  visualOffsetTop.value = window.visualViewport?.offsetTop ?? 0
}

function onPointerDown(event: PointerEvent) {
  if (!isMobile() || (event.pointerType === 'mouse' && event.button !== 0)) return
  if (!props.modelValue) return
  grab()
  grabY = dragY.value
  pointerId = event.pointerId
  startY = event.clientY
  samples = [{ y: event.clientY, time: performance.now() }]
  dragging.value = true
  captureTarget = event.currentTarget as HTMLElement
  captureTarget.setPointerCapture(event.pointerId)
}
function onPointerMove(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  event.preventDefault()
  const height = panelRef.value?.offsetHeight || 500
  follow(panelDragPosition(grabY, event.clientY, startY, height))
  const now = performance.now()
  samples.push({ y: event.clientY, time: now })
  samples = samples.filter((sample) => now - sample.time <= 100)
}
function onPointerUp(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  const first = samples[0]
  const last = samples[samples.length - 1]
  const elapsed = first && last ? Math.max(1, last.time - first.time) : 1
  const velocity = first && last ? ((last.y - first.y) / elapsed) * 1000 : 0
  const projected = projectMotion(dragY.value, velocity)
  const shouldClose = projected > (panelRef.value?.offsetHeight || 500) * 0.3
  if (captureTarget?.hasPointerCapture(event.pointerId)) captureTarget.releasePointerCapture(event.pointerId)
  captureTarget = null
  pointerId = null
  dragging.value = false
  if (shouldClose) { handoff(velocity); close() }
  else settle(velocity)
}
function resetGesture() {
  grab()
  if (pointerId !== null && captureTarget?.hasPointerCapture(pointerId)) {
    captureTarget.releasePointerCapture(pointerId)
  }
  captureTarget = null
  pointerId = null
  dragging.value = false
}
function cancelGesture() {
  if (pointerId === null) return
  if (captureTarget?.hasPointerCapture(pointerId)) captureTarget.releasePointerCapture(pointerId)
  captureTarget = null
  pointerId = null
  dragging.value = false
  settle(0)
}
useModalEnvironment(() => present.value, panelRef, close)
onMounted(() => {
  updateVisualViewport()
  window.visualViewport?.addEventListener('resize', updateVisualViewport)
  window.visualViewport?.addEventListener('scroll', updateVisualViewport)
})
onBeforeUnmount(() => {
  resetGesture()
  window.visualViewport?.removeEventListener('resize', updateVisualViewport)
  window.visualViewport?.removeEventListener('scroll', updateVisualViewport)
})
</script>

<style scoped>
.motion-modal {
  position: fixed;
  inset: 0;
  z-index: 2400;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: transparent;
}
/* Fade the dimming/blur plane as one composited layer, not the sheet itself.
   The blur radius stays constant throughout the physical transition. */
.motion-modal::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: var(--glass-scrim-fill);
  backdrop-filter: var(--glass-scrim-filter);
  -webkit-backdrop-filter: var(--glass-scrim-filter);
  opacity: var(--motion-scrim-opacity);
}
.motion-modal__panel {
  position: relative;
  z-index: 1;
  width: min(540px, calc(100vw - 48px));
  max-height: calc(100dvh - 48px);
  overflow: visible;
  outline: none;
}
.is-settling .motion-modal__panel, .is-dragging .motion-modal__panel { will-change: transform; }
.motion-modal__panel.is-wide { width: min(840px, calc(100vw - 48px)); }
.motion-modal__panel > :deep(:not(.motion-modal__handle)) { max-height: inherit; }
.motion-modal__handle { display: none; }

@media (max-width: 768px) {
  .motion-modal {
    top: var(--motion-viewport-top);
    bottom: auto;
    height: var(--motion-viewport-height);
    align-items: flex-end;
    padding: 0;
  }
  .motion-modal__panel {
    width: 100%;
    max-height: min(88dvh, 760px, calc(var(--motion-viewport-height) - 16px));
    border-radius: 24px 24px 0 0 !important;
    touch-action: pan-y;
    overscroll-behavior: contain;
    padding-bottom: env(safe-area-inset-bottom);
  }
  .motion-modal__panel.is-wide { width: 100%; }
  .motion-modal__handle {
    display: flex;
    position: absolute;
    top: 2px;
    left: 0;
    right: 0;
    height: 30px;
    align-items: flex-start;
    justify-content: center;
    padding-top: 7px;
    cursor: grab;
    touch-action: none;
    z-index: 2;
  }
  .motion-modal__handle span {
    width: 38px;
    height: 5px;
    border-radius: 999px;
    background: var(--text-faint);
    opacity: 0.45;
  }
}

@media (prefers-reduced-motion: reduce) {
  .motion-modal__panel { transform: none !important; }
}

@media (prefers-reduced-transparency: reduce) {
  .motion-modal::before {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}
</style>
