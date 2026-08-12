<template>
  <Teleport to="body">
    <Transition name="motion-modal">
      <div v-if="modelValue" class="motion-modal" :class="{ 'is-dragging': dragging }" @click.self="close">
        <section
          ref="panelRef"
          class="motion-modal__panel"
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
            @pointercancel="resetGesture"
          ><span /></div>
          <slot />
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  modelValue: boolean
  ariaLabel?: string
}>(), {
  ariaLabel: '对话框',
})
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const panelRef = ref<HTMLElement | null>(null)
const dragging = ref(false)
const dragY = ref(0)
let pointerId: number | null = null
let startY = 0
let samples: Array<{ y: number; time: number }> = []

const panelStyle = computed(() => ({ '--motion-sheet-y': `${dragY.value}px` }))

function rubberband(overshoot: number, dimension: number, constant = 0.35) {
  return (overshoot * dimension * constant) / (dimension + constant * Math.abs(overshoot))
}
function projectVelocity(velocity: number, decelerationRate = 0.99) {
  return (velocity / 1000) * decelerationRate / (1 - decelerationRate)
}
function isMobile() { return window.matchMedia('(max-width: 768px)').matches }
function close() { emit('update:modelValue', false) }

function onPointerDown(event: PointerEvent) {
  if (!isMobile() || (event.pointerType === 'mouse' && event.button !== 0)) return
  pointerId = event.pointerId
  startY = event.clientY
  samples = [{ y: event.clientY, time: performance.now() }]
  dragging.value = true
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}
function onPointerMove(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  event.preventDefault()
  const dy = event.clientY - startY
  const height = panelRef.value?.offsetHeight || 500
  dragY.value = dy >= 0 ? dy : rubberband(dy, height)
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
  const projected = dragY.value + projectVelocity(velocity)
  const shouldClose = projected > (panelRef.value?.offsetHeight || 500) * 0.3
  pointerId = null
  dragging.value = false
  if (shouldClose) close()
  else dragY.value = 0
}
function resetGesture() {
  pointerId = null
  dragging.value = false
  dragY.value = 0
}
function onKeydown(event: KeyboardEvent) {
  if (props.modelValue && event.key === 'Escape') close()
}

watch(() => props.modelValue, (open) => {
  if (open) {
    resetGesture()
    nextTick(() => panelRef.value?.focus({ preventScroll: true }))
  }
})
onMounted(() => document.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))
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
  background: rgba(0, 0, 0, 0.24);
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
.motion-modal-enter-from .motion-modal__panel { transform: translateY(12px) scale(0.97); opacity: 0; }
.motion-modal-leave-to .motion-modal__panel { transform: translateY(8px) scale(0.985); opacity: 0; }

@media (max-width: 768px) {
  .motion-modal { align-items: flex-end; padding: 0; }
  .motion-modal__panel {
    width: 100%;
    max-height: min(88dvh, 760px);
    border-radius: 24px 24px 0 0 !important;
    transform: translate3d(0, var(--motion-sheet-y), 0);
    touch-action: pan-y;
    overscroll-behavior: contain;
  }
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
  .is-dragging .motion-modal__panel { transition: none; }
  .motion-modal-enter-from .motion-modal__panel,
  .motion-modal-leave-to .motion-modal__panel { transform: translateY(100%); opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .motion-modal__panel { transform: none !important; }
}
</style>
