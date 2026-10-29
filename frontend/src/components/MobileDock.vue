<template>
  <div
    ref="dockRef"
    class="mobile-navigation"
    :class="{ 'has-subnav': !!subnav?.items.length, 'is-compact': compact }"
    :style="{
      '--dock-progress': geometry.shape,
      '--dock-settle-progress': geometry.sink,
      '--dock-travel-x': geometry.travelX,
      '--dock-travel-y': geometry.travelY,
      '--dock-rebound': rebound,
      '--dock-width': `${width}px`,
    }"
    :data-progress="Math.round(effectiveProgress)"
    :data-settled="effectiveProgress === 0 || effectiveProgress === 100"
    :data-rebound="rebound"
  >
    <div
      v-show="!!presentation"
      v-glass-lens="'css'"
      class="mobile-auxiliary-surface dock-glass"
      data-glass="floating"
      data-glass-rim
      :class="{ 'mobile-sub-dock': !!subnav?.items.length, 'icon-layout': displayedSubnav?.layout === 'icons', 'is-page-action': !subnav?.items.length }"
      :style="auxiliaryStyle"
      :data-mode="subnav?.items.length ? 'subnav' : 'action'"
      :data-morphing="morphing"
    >
      <nav v-if="displayedSubnav" class="mobile-sub-dock-content" :style="subnavContentStyle" :aria-label="displayedSubnav.label" :aria-hidden="!subnav?.items.length ? 'true' : undefined" :inert="!subnav?.items.length ? true : undefined">
        <template v-for="item in displayedSubnav.items" :key="item.id">
          <router-link v-if="item.to" :to="item.to" class="mobile-sub-dock-item" :class="{ active: item.active }" :aria-current="item.active ? 'page' : undefined" :aria-label="item.label">
            {{ item.compactLabel || item.label }}
          </router-link>
          <button v-else type="button" class="mobile-sub-dock-item" :class="{ active: item.active, 'is-icon': !!item.icon }" :aria-label="item.label" :title="item.label" :aria-pressed="item.active" @click="item.select?.()">
            <el-icon v-if="item.icon"><component :is="item.icon" /></el-icon>
            <template v-else>{{ item.compactLabel || item.label }}</template>
          </button>
        </template>
      </nav>
      <button
        v-if="displayedAction"
        type="button"
        class="mobile-page-action"
        :style="{ opacity: presentation?.actionOpacity }"
        :aria-label="displayedAction.label"
        :title="displayedAction.label"
        :aria-hidden="!action || subnav?.items.length ? 'true' : undefined"
        :inert="!action || subnav?.items.length ? true : undefined"
        @click="action?.select($event)"
      ><component :is="displayedAction.icon" class="mobile-page-action-glyph" aria-hidden="true" /></button>
    </div>

    <div v-glass-lens class="mobile-dock dock-glass" data-glass-rim>
      <nav id="mobile-main-navigation" class="mobile-dock-links" aria-label="主要导航" :aria-hidden="compact ? 'true' : undefined" :inert="compact ? true : undefined">
        <router-link to="/" class="mobile-dock-item" :class="{ active: section === 'home' }" :aria-current="section === 'home' ? 'page' : undefined">
          <el-icon><House /></el-icon><span>首页</span>
        </router-link>
        <router-link :to="{ path: '/reader', query: { view: 'shelf' } }" class="mobile-dock-item" :class="{ active: section === 'reader' }" :aria-current="section === 'reader' ? 'page' : undefined">
          <el-icon><Files /></el-icon><span>阅境轩</span>
        </router-link>
        <router-link to="/timeline" class="mobile-dock-item" :class="{ active: section === 'timeline' }" :aria-current="section === 'timeline' ? 'page' : undefined">
          <el-icon><Calendar /></el-icon><span>时光机</span>
        </router-link>
        <router-link :to="{ path: '/tasks', query: { view: 'tasks' } }" class="mobile-dock-item" :class="{ active: section === 'tasks' }" :aria-current="section === 'tasks' ? 'page' : undefined">
          <el-icon><Finished /></el-icon><span>任务中枢</span>
        </router-link>
        <router-link to="/knowledge" class="mobile-dock-item" :class="{ active: section === 'wiki' }" :aria-current="section === 'wiki' ? 'page' : undefined">
          <el-icon><Notebook /></el-icon><span>Wiki</span>
        </router-link>
      </nav>
      <button data-icon-only type="button" class="mobile-dock-orb" aria-label="展开主要导航" aria-controls="mobile-main-navigation" :aria-expanded="!compact" :aria-hidden="!compact ? 'true' : undefined" :inert="!compact ? true : undefined" @click="expand">
        <el-icon><component :is="orbIcon" /></el-icon>
        <span class="dock-orb-hint" aria-hidden="true"></span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { Calendar, Files, Finished, House, Notebook } from '@element-plus/icons-vue'
import type { MobileNavSection } from '@/utils/mobileNavigationPolicy'
import type { MobileSubnav } from '@/composables/useMobileSubnav'
import type { MobilePageAction } from '@/composables/useMobilePageAction'
import { useDockAuxiliaryMotion } from '@/composables/useDockAuxiliaryMotion'
import { auxiliaryDockLayout } from '@/utils/dockAuxiliaryMotion'
import { dockGeometry, dockRebound } from '@/utils/mobileDockMotion'

const props = defineProps<{
  progress: number
  section: MobileNavSection
  subnav: MobileSubnav | null
  action: MobilePageAction | null
}>()
const emit = defineEmits<{ expand: [] }>()
// Single-layer navigation always retains all five destinations, even during
// route handoff while the outgoing double dock's spring is still settling.
const effectiveProgress = computed(() => props.subnav?.items.length ? props.progress : 0)
const geometry = computed(() => dockGeometry(effectiveProgress.value))
const rebound = computed(() => dockRebound(effectiveProgress.value))
const compact = computed(() => geometry.value.sink > .12)
const orbIcon = computed(() => ({ home: House, reader: Files, timeline: Calendar, tasks: Finished, wiki: Notebook })[props.section])
const dockRef = ref<HTMLElement | null>(null)
const width = ref(Math.max(0, window.innerWidth - 16))
const actionInset = ref(8)
const actionSize = ref(56), actionGap = ref(12)
const barLayout = computed(() => auxiliaryDockLayout(width.value, 'subnav', effectiveProgress.value))
const { presentation, moving: morphing } = useDockAuxiliaryMotion(() => {
  if (props.subnav?.items.length) return { mode: 'subnav', frame: barLayout.value }
  if (props.action) return { mode: 'action', frame: auxiliaryDockLayout(width.value, 'action', 0, { actionInset: actionInset.value, actionSize: actionSize.value, actionGap: actionGap.value }) }
  return null
})
const displayedSubnav = shallowRef<MobileSubnav | null>(null)
const displayedAction = shallowRef<MobilePageAction | null>(null)
watch([() => props.subnav, () => props.action, morphing], ([subnav, action, moving]) => {
  if (subnav?.items.length) displayedSubnav.value = subnav
  else if (!moving && presentation.value?.barOpacity === 0) displayedSubnav.value = null
  if (action) displayedAction.value = action
  else if (!moving && presentation.value?.actionOpacity === 0) displayedAction.value = null
}, { immediate: true, flush: 'post' })
const auxiliaryStyle = computed(() => presentation.value ? {
  left: `${presentation.value.x}px`, width: `${presentation.value.width}px`, height: `${presentation.value.height}px`,
  transform: `translate3d(0, ${presentation.value.y}px, 0)`,
} : undefined)
const subnavContentStyle = computed(() => ({
  width: `${barLayout.value.width - 2}px`, height: `${barLayout.value.height - 2}px`,
  opacity: presentation.value?.barOpacity ?? 0,
  padding: `${2 + 2 * geometry.value.sink}px`,
}))
let observer: ResizeObserver | undefined
let returnKeyboardFocus = false
function expand(event: MouseEvent) {
  returnKeyboardFocus = event.detail === 0
  emit('expand')
}
watch(() => props.progress, async (value) => {
  if (value !== 0 || !returnKeyboardFocus) return
  returnKeyboardFocus = false
  await nextTick()
  // Only return focus after an explicit expansion in this document.
  if (document.hasFocus() && (document.activeElement === document.body || document.activeElement?.matches('.mobile-dock-orb'))) {
    dockRef.value?.querySelector<HTMLElement>('.mobile-dock-item.active')?.focus({ preventScroll: true })
  }
})
onMounted(() => {
  observer = new ResizeObserver(([entry]) => {
    if (entry) {
      width.value = entry.contentRect.width
      const edge = dockRef.value?.getBoundingClientRect().right ?? window.innerWidth - 8
      actionInset.value = Math.max(0, 16 - (window.innerWidth - edge))
      const tokens = getComputedStyle(document.documentElement)
      actionSize.value = parseFloat(tokens.getPropertyValue('--mobile-page-action-size')) || 56
      actionGap.value = parseFloat(tokens.getPropertyValue('--mobile-page-action-gap')) || 12
    }
  })
  if (dockRef.value) observer.observe(dockRef.value)
})
onScopeDispose(() => observer?.disconnect())
</script>

<style scoped>
.mobile-navigation {
  --dock-orb-size: var(--mobile-dock-height);
  --dock-gap: 8px;
  position: fixed;
  z-index: 1005;
  left: max(8px, var(--safe-left));
  right: max(8px, var(--safe-right));
  bottom: max(8px, var(--safe-bottom));
  height: var(--mobile-dock-height);
  pointer-events: none;
  isolation: isolate;
}
.mobile-navigation.has-subnav { height: calc(var(--mobile-dock-height) + var(--mobile-sub-dock-height) + var(--dock-gap)); }
.dock-glass {
  border: 1px solid var(--border-glass);
  /* The browser clamps the same pill contour to every live height and width. */
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg-glass-strong) 94%, transparent);
  backdrop-filter: var(--glass-floating-filter);
  -webkit-backdrop-filter: var(--glass-floating-filter);
  box-shadow: 0 8px 26px rgba(0, 0, 0, .1), var(--inset-highlight);
}
.mobile-dock {
  position: absolute;
  right: 0;
  bottom: 0;
  width: calc(100% - (var(--dock-width) - var(--dock-orb-size)) * var(--dock-progress));
  height: var(--mobile-dock-height);
  overflow: hidden;
  transform-origin: right bottom;
  transform: translate3d(0, calc(-4px * var(--dock-rebound)), 0) scale(calc(1 + .012 * var(--dock-rebound)), calc(1 - .06 * var(--dock-rebound)));
  pointer-events: none;
}
.mobile-dock-links {
  position: absolute;
  right: 0;
  top: 0;
  width: calc(var(--dock-width) - 2px);
  height: calc(var(--mobile-dock-height) - 2px);
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  padding: 4px;
  opacity: clamp(0, 1 - var(--dock-settle-progress) * 3, 1);
  transform: translate3d(calc(12px * var(--dock-settle-progress)), 0, 0);
  pointer-events: auto;
}
.is-compact .mobile-dock-links { pointer-events: none; }
.mobile-dock-item {
  min-width: 0;
  min-height: 44px;
  border: 0;
  border-radius: 15px;
  background: transparent;
  color: var(--text-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  text-decoration: none;
  font: inherit;
  font-size: 10px;
  font-weight: 570;
  line-height: 1.15;
  transition: color var(--motion-fast) ease, background-color var(--motion-fast) ease, transform var(--motion-instant) ease;
}
.mobile-dock-item .el-icon { font-size: 22px; }
.mobile-dock-item.active { color: var(--accent); background: color-mix(in srgb, var(--accent) 11%, transparent); }
.mobile-dock-item:active, .mobile-dock-orb:active { transform: scale(.94); }
.mobile-dock-orb {
  position: absolute;
  inset: 0 0 0 auto;
  width: calc(var(--dock-orb-size) - 2px);
  border: 0;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: transparent;
  color: var(--accent);
  opacity: clamp(0, (var(--dock-settle-progress) - .12) * 3, 1);
  transform: scale(calc(.8 + .2 * var(--dock-settle-progress)));
  pointer-events: none;
  transition: background-color var(--motion-fast) ease;
}
.is-compact .mobile-dock-orb { pointer-events: auto; }
.mobile-dock-orb .el-icon { font-size: 24px; }
.dock-orb-hint { position: absolute; bottom: 7px; width: 12px; height: 2px; border-radius: 2px; background: currentColor; opacity: .45; }
.mobile-auxiliary-surface {
  position: absolute;
  bottom: 0;
  overflow: hidden;
  pointer-events: none;
}
.mobile-sub-dock-content {
  position: absolute;
  left: 0;
  top: 0;
  display: flex;
  pointer-events: auto;
}
.mobile-sub-dock-content[inert] { pointer-events: none; }
.mobile-page-action {
  position: absolute;
  right: -1px;
  top: calc((100% - var(--mobile-page-action-size)) / 2);
  width: var(--mobile-page-action-size);
  height: var(--mobile-page-action-size);
  display: grid;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  pointer-events: auto;
  -webkit-tap-highlight-color: transparent;
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
}
.mobile-page-action[inert] { pointer-events: none; }
.mobile-page-action-glyph { width: 25px; height: 25px; }
.mobile-page-action:active { transform: scale(.94); transition-duration: 70ms; }
.mobile-page-action:focus-visible { outline: 2px solid var(--accent); outline-offset: -5px; }
html.motion-overlay-open .mobile-auxiliary-surface.is-page-action { visibility: hidden; }
.mobile-sub-dock-item {
  flex: 1 1 0;
  min-width: 0;
  min-height: 44px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 2px;
  border: 0;
  border-radius: 14px;
  background: transparent;
  color: var(--text-muted);
  white-space: nowrap;
  text-decoration: none;
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  transition: color var(--motion-fast) ease, background-color var(--motion-fast) ease;
}
.mobile-sub-dock-item.active { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
.mobile-sub-dock-item.is-icon { flex: 0 0 44px; }
.mobile-sub-dock.icon-layout .mobile-sub-dock-item { flex: 1 1 0; min-width: 44px; }
.mobile-sub-dock-item .el-icon { font-size: 21px; }
.mobile-sub-dock-item:active { background: color-mix(in srgb, var(--accent) 18%, transparent); }
@media (prefers-reduced-transparency: reduce) {
  .dock-glass { background: var(--bg-primary); backdrop-filter: none; -webkit-backdrop-filter: none; }
}
@media (prefers-contrast: more) {
  .dock-glass { border-color: var(--text-muted); }
  .mobile-sub-dock-item.active, .mobile-dock-item.active { box-shadow: inset 0 0 0 1px var(--accent); }
}
@media (prefers-reduced-motion: reduce) {
  .mobile-navigation { --dock-rebound: 0 !important; }
  .mobile-dock-item, .mobile-dock-orb, .mobile-sub-dock-item { transition: none; }
  .mobile-page-action, .mobile-page-action:active { transform: none; transition: none; }
}
</style>
