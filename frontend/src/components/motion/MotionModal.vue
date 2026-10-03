<template>
  <Teleport to="body">
    <Transition name="motion-modal">
      <div
        v-if="modelValue"
        class="motion-modal"
        :class="{ 'is-dragging': dragging, 'is-settling': settling }"
        :style="overlayStyle"
        @click.self="close"
      >
        <section
          ref="panelRef"
          class="motion-modal__panel"
          :class="{ 'is-wide': size === 'wide' }"
          :style="panelStyle"
          role="dialog"
          aria-modal="true"
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
          <slot />
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useModalEnvironment } from '@/composables/useModalEnvironment'
import { animateSpring, projectMotion } from '@/utils/motionSpring'
import { panelDragPosition, presentationOffset } from '@/utils/panelGesture'

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
const settling = ref(false)
const dragY = ref(0)
const visualHeight = ref(window.visualViewport?.height ?? window.innerHeight)
const visualOffsetTop = ref(window.visualViewport?.offsetTop ?? 0)
let pointerId: number | null = null
let startY = 0
let grabY = 0
let samples: Array<{ y: number; time: number }> = []
let cancelSpring: (() => void) | null = null
let captureTarget: HTMLElement | null = null

const panelStyle = computed(() => ({ '--motion-sheet-y': `${dragY.value}px` }))
const overlayStyle = computed(() => ({
  '--motion-viewport-height': `${visualHeight.value}px`,
  '--motion-viewport-top': `${visualOffsetTop.value}px`,
}))

function isMobile() { return window.matchMedia('(max-width: 768px)').matches }
function close() { emit('update:modelValue', false) }

function stopSpring() {
  cancelSpring?.()
  cancelSpring = null
  settling.value = false
}

function updateVisualViewport() {
  visualHeight.value = window.visualViewport?.height ?? window.innerHeight
  visualOffsetTop.value = window.visualViewport?.offsetTop ?? 0
}

function onPointerDown(event: PointerEvent) {
  if (!isMobile() || (event.pointerType === 'mouse' && event.button !== 0)) return
  stopSpring()
  grabY = panelRef.value ? presentationOffset(getComputedStyle(panelRef.value).transform, 'y', dragY.value) : dragY.value
  dragY.value = grabY
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
  dragY.value = panelDragPosition(grabY, event.clientY, startY, height)
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
  settling.value = true
  const target = shouldClose ? (panelRef.value?.offsetHeight || visualHeight.value) + 24 : 0
  cancelSpring = animateSpring(
    dragY.value,
    target,
    velocity,
    (value) => { dragY.value = value },
    () => {
      cancelSpring = null
      settling.value = false
      if (shouldClose) close()
    },
    { response: 0.36, damping: Math.abs(velocity) > 500 ? 0.9 : 1 },
  )
}
function resetGesture() {
  stopSpring()
  if (pointerId !== null && captureTarget?.hasPointerCapture(pointerId)) {
    captureTarget.releasePointerCapture(pointerId)
  }
  captureTarget = null
  pointerId = null
  dragging.value = false
  dragY.value = 0
}
function cancelGesture() {
  if (pointerId === null) return
  if (captureTarget?.hasPointerCapture(pointerId)) captureTarget.releasePointerCapture(pointerId)
  captureTarget = null
  pointerId = null
  dragging.value = false
  settling.value = true
  cancelSpring = animateSpring(dragY.value, 0, 0, value => { dragY.value = value }, () => {
    cancelSpring = null
    settling.value = false
  })
}
watch(() => props.modelValue, (open) => {
  if (open) resetGesture()
})
useModalEnvironment(() => props.modelValue, panelRef, close)
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
  background: color-mix(in srgb, #000 22%, transparent);
  backdrop-filter: var(--glass-scrim-filter);
  -webkit-backdrop-filter: var(--glass-scrim-filter);
}
.motion-modal__panel {
  position: relative;
  width: min(540px, calc(100vw - 48px));
  max-height: calc(100dvh - 48px);
  overflow: visible;
  outline: none;
  transform: translate3d(0, var(--motion-sheet-y), 0);
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
}
.motion-modal__panel.is-wide { width: min(840px, calc(100vw - 48px)); }
.motion-modal__handle { display: none; }

.motion-modal-enter-active,
.motion-modal-leave-active { transition: opacity var(--motion-fast) var(--ease-emphasized); }
.motion-modal-enter-active .motion-modal__panel,
.motion-modal-leave-active .motion-modal__panel {
  transition: transform var(--motion-normal) var(--ease-spring-gentle),
              opacity var(--motion-fast) var(--ease-emphasized);
}
.motion-modal-enter-from,
.motion-modal-leave-to { opacity: 0; }
.motion-modal-enter-from .motion-modal__panel,
.motion-modal-leave-to .motion-modal__panel { transform: translateY(10px) scale(0.98); opacity: 0; }

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
    transform: translate3d(0, var(--motion-sheet-y), 0);
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
  .is-dragging .motion-modal__panel,
  .is-settling .motion-modal__panel { transition: none; will-change: transform; }
  .motion-modal-enter-from .motion-modal__panel,
  .motion-modal-leave-to .motion-modal__panel { transform: translateY(100%); opacity: 1; }
  .motion-modal.is-dragging .motion-modal__panel,
  .motion-modal.is-settling .motion-modal__panel { transform: translate3d(0, var(--motion-sheet-y), 0); transition: none; }
}

@media (prefers-reduced-motion: reduce) {
  .motion-modal__panel { transform: none !important; }
}

@media (prefers-reduced-transparency: reduce) {
  .motion-modal {
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}
</style>
