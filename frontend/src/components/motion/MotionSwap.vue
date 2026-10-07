<template>
  <div class="motion-swap" :class="{ 'is-contained': contained, 'is-horizontal': horizontal }" :style="{ '--swap-direction': direction }">
    <Transition name="content-flow" @before-leave="leave" @before-enter="enter">
      <div :key="viewKey" class="motion-swap__panel"><MotionContent :live="true"><slot /></MotionContent></div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import MotionContent from './MotionContent'

withDefaults(defineProps<{ viewKey: string | number; horizontal?: boolean; direction?: number; contained?: boolean }>(), {
  direction: 1, horizontal: false, contained: false,
})
function leave(element: Element) {
  if (!(element instanceof HTMLElement)) return
  element.inert = true
  // Freeze outgoing geometry without making the incoming page wait for it.
  element.style.width = `${element.offsetWidth}px`
  element.style.height = `${element.offsetHeight}px`
}
function enter(element: Element) {
  if (!(element instanceof HTMLElement)) return
  element.inert = false
  element.style.removeProperty('width')
  element.style.removeProperty('height')
}
</script>

<style scoped>
.motion-swap { position: relative; min-width: 0; max-height: inherit; --swap-in-x: 0px; --swap-out-x: 0px; --swap-in-y: 18px; --swap-out-y: -8px; }
.motion-swap__panel { min-width: 0; max-height: inherit; }
.motion-swap.is-contained { flex: 1; min-height: 0; height: 100%; }
.is-contained > .motion-swap__panel { height: 100%; min-height: 0; }
.motion-swap.is-horizontal { --swap-in-x: calc(32px * var(--swap-direction)); --swap-out-x: calc(-18px * var(--swap-direction)); --swap-in-y: 0px; --swap-out-y: 0px; }
.content-flow-enter-active { transition: transform 380ms var(--ease-spring-gentle), opacity 240ms ease; }
.content-flow-leave-active { position: absolute; top: 0; left: 0; pointer-events: none; transition: transform 300ms var(--ease-emphasized), opacity 180ms ease; }
.content-flow-enter-active, .content-flow-leave-active { will-change: transform, opacity; }
.content-flow-enter-from { opacity: 0; transform: translate3d(var(--swap-in-x), var(--swap-in-y), 0) scale(.985); }
.content-flow-leave-to { opacity: 0; transform: translate3d(var(--swap-out-x), var(--swap-out-y), 0) scale(.985); }
@media(prefers-reduced-motion: reduce) {
  .content-flow-enter-active, .content-flow-leave-active { transition: opacity 120ms ease; }
  .content-flow-enter-from, .content-flow-leave-to { transform: none; }
}
</style>
