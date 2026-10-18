<template>
  <div
    ref="scrollRef" class="date-wheel-column" role="listbox" tabindex="0"
    :aria-label="label" :aria-activedescendant="`${columnId}-${selected}`"
    :data-position="frame.position" :data-moving="frame.moving" :data-dragging="frame.dragging"
    @keydown="onKeydown" @wheel.prevent.stop="onWheel"
    @pointerdown="onPointerDown" @pointermove="onPointerMove" @pointerup="onPointerUp"
    @pointercancel="onPointerCancel" @lostpointercapture="onPointerCancel"
  >
    <div class="date-wheel-track" :style="{ transform: `translate3d(0, ${-frame.position}px, 0)` }">
      <div v-for="(option, index) in options" :id="`${columnId}-${option}`" :key="option" :data-index="index"
        class="date-wheel-option" :class="{ 'is-selected': option === selected }"
        role="option" :aria-selected="option === selected" @click="onAccessibleClick(index, $event)"
      >{{ option }}{{ label }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { getCurrentInstance, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { createDateWheelMotion, type DateWheelFrame } from '@/utils/dateWheelMotion'

const props = defineProps<{ modelValue: number; options: number[]; label: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: number] }>()
const scrollRef = ref<HTMLElement | null>(null)
const selected = ref(props.modelValue)
const columnId = `date-wheel-${getCurrentInstance()?.uid}`
const frame = ref<DateWheelFrame>({ position: 0, index: 0, moving: false, dragging: false })
let pressedIndex: number | null = null
const motion = createDateWheelMotion({
  count: () => props.options.length,
  clock: { now: () => performance.now(), request: callback => requestAnimationFrame(callback), cancel: id => cancelAnimationFrame(id), reduced: () => window.matchMedia('(prefers-reduced-motion: reduce)').matches },
  change: value => {
    frame.value = value
    const option = props.options[value.index]
    if (option === undefined || option === selected.value) return
    selected.value = option
    emit('update:modelValue', option)
  },
})

function pointer(event: PointerEvent) { return { id: event.pointerId, y: event.clientY, time: event.timeStamp } }
function onPointerDown(event: PointerEvent) {
  if (event.button !== 0 || !motion.down(pointer(event))) return
  pressedIndex = event.target instanceof Element ? Number(event.target.closest<HTMLElement>('[data-index]')?.dataset.index ?? NaN) : null
  scrollRef.value?.setPointerCapture(event.pointerId)
}
function onPointerMove(event: PointerEvent) { motion.move(pointer(event)) }
function release(event: PointerEvent) {
  if (scrollRef.value?.hasPointerCapture(event.pointerId)) scrollRef.value.releasePointerCapture(event.pointerId)
}
function onPointerUp(event: PointerEvent) {
  if (!scrollRef.value?.hasPointerCapture(event.pointerId)) return
  if (!frame.value.dragging) { pressedIndex = null; release(event); return }
  const dragged = motion.up(pointer(event))
  if (!dragged && pressedIndex !== null && Number.isFinite(pressedIndex)) motion.select(pressedIndex)
  pressedIndex = null
  release(event)
}
function onPointerCancel(event: PointerEvent) { motion.cancel(event.pointerId); release(event) }
function onAccessibleClick(index: number, event: MouseEvent) { if (event.detail === 0) motion.select(index) }
function onWheel(event: WheelEvent) { motion.wheel(event.deltaY * (event.deltaMode === 1 ? 44 : event.deltaMode === 2 ? 220 : 1)) }
function settle() { motion.settle() }
function onKeydown(event: KeyboardEvent) {
  const index = motion.targetIndex()
  const target = { ArrowUp: index - 1, ArrowDown: index + 1, PageUp: index - 5, PageDown: index + 5, Home: 0, End: props.options.length - 1 }[event.key]
  if (target === undefined) return
  event.preventDefault()
  event.stopPropagation()
  motion.select(Math.max(0, Math.min(props.options.length - 1, target)))
}
function sync() {
  selected.value = props.modelValue
  motion.sync(Math.max(0, props.options.indexOf(props.modelValue)))
}
watch([() => props.modelValue, () => props.options], ([value, options], [, previous]) => {
  // Acknowledging our own scroll must not reset its velocity. Only external
  // selection or a real month/year option change should reposition this wheel.
  if (value !== selected.value || options.length !== previous.length || options.some((option, index) => option !== previous[index])) void sync()
}, { flush: 'post' })
onMounted(sync)
onBeforeUnmount(() => motion.dispose())
defineExpose({ settle })
</script>

<style scoped>
.date-wheel-column { position:relative; z-index:1; min-width:0; height:calc(var(--date-wheel-rows, 5) * 44px); overflow:hidden; overscroll-behavior:contain; touch-action:none; mask-image:linear-gradient(transparent, #000 32%, #000 68%, transparent); -webkit-mask-image:linear-gradient(transparent, #000 32%, #000 68%, transparent); }
.date-wheel-track { padding-top:calc((var(--date-wheel-rows, 5) - 1) * 22px); }
.date-wheel-column[data-moving="true"] .date-wheel-track, .date-wheel-column[data-dragging="true"] .date-wheel-track { will-change:transform; }
.date-wheel-column:focus-visible { outline:2px solid var(--accent-border); outline-offset:-2px; border-radius:14px; }
.date-wheel-option { display:flex; align-items:center; justify-content:center; height:44px; color:var(--text-muted); font-size:18px; line-height:1; font-variant-numeric:tabular-nums; user-select:none; cursor:pointer; }
.date-wheel-option.is-selected { color:var(--text-primary); font-weight:650; }
</style>
