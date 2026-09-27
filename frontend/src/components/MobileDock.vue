<template>
  <div
    ref="dockRef"
    class="mobile-navigation"
    :class="{ 'has-subnav': !!subnav?.items.length, 'is-compact': compact }"
    :style="{ '--dock-progress': progress / 100, '--dock-width': `${width}px` }"
    :data-progress="Math.round(progress)"
  >
    <nav v-if="subnav?.items.length" class="mobile-sub-dock dock-glass" :aria-label="subnav.label">
      <template v-for="item in subnav.items" :key="item.id">
        <router-link v-if="item.to" :to="item.to" class="mobile-sub-dock-item" :class="{ active: item.active }" :aria-current="item.active ? 'page' : undefined" :aria-label="item.label">
          {{ item.compactLabel || item.label }}
        </router-link>
        <button v-else type="button" class="mobile-sub-dock-item" :class="{ active: item.active }" :aria-pressed="item.active" @click="item.select?.()">
          {{ item.compactLabel || item.label }}
        </button>
      </template>
    </nav>

    <div class="mobile-dock dock-glass">
      <nav id="mobile-main-navigation" class="mobile-dock-links" aria-label="主要导航" :aria-hidden="compact ? 'true' : undefined" :inert="compact ? true : undefined">
        <router-link :to="{ path: '/reader', query: { view: 'shelf' } }" class="mobile-dock-item" :class="{ active: section === 'reader' }" :aria-current="section === 'reader' ? 'page' : undefined">
          <el-icon><Files /></el-icon><span>阅境轩</span>
        </router-link>
        <router-link to="/timeline" class="mobile-dock-item" :class="{ active: section === 'timeline' }" :aria-current="section === 'timeline' ? 'page' : undefined">
          <el-icon><Calendar /></el-icon><span>时光机</span>
        </router-link>
        <router-link :to="{ path: '/tasks', query: { view: 'tasks' } }" class="mobile-dock-item" :class="{ active: section === 'tasks' }" :aria-current="section === 'tasks' ? 'page' : undefined">
          <el-icon><Finished /></el-icon><span>任务中枢</span>
        </router-link>
        <button type="button" class="mobile-dock-item" :class="{ active: section === 'more' || sidebarVisible }" :aria-expanded="sidebarVisible" aria-label="全部模块" @click="emit('openModules')">
          <el-icon><Grid /></el-icon><span>全部</span>
        </button>
      </nav>
      <button type="button" class="mobile-dock-orb" aria-label="展开主要导航" aria-controls="mobile-main-navigation" :aria-expanded="!compact" :aria-hidden="!compact ? 'true' : undefined" :inert="!compact ? true : undefined" @click="expand">
        <el-icon><component :is="orbIcon" /></el-icon>
        <span class="dock-orb-hint" aria-hidden="true"></span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onScopeDispose, ref, watch } from 'vue'
import { Calendar, Files, Finished, Grid } from '@element-plus/icons-vue'
import type { MobileNavSection } from '@/utils/mobileNavigationPolicy'
import type { MobileSubnav } from '@/composables/useMobileSubnav'

const props = defineProps<{
  progress: number
  section: MobileNavSection
  sidebarVisible: boolean
  subnav: MobileSubnav | null
}>()
const emit = defineEmits<{ expand: []; openModules: [] }>()
const compact = computed(() => props.progress > 12)
const orbIcon = computed(() => ({ reader: Files, timeline: Calendar, tasks: Finished, more: Grid })[props.section])
const dockRef = ref<HTMLElement | null>(null)
const width = ref(Math.max(0, window.innerWidth - 16))
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
    if (entry) width.value = entry.contentRect.width
  })
  if (dockRef.value) observer.observe(dockRef.value)
})
onScopeDispose(() => observer?.disconnect())
</script>

<style scoped>
.mobile-navigation {
  --dock-orb-size: 56px;
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
  background: color-mix(in srgb, var(--bg-glass-strong) 94%, transparent);
  backdrop-filter: blur(20px) saturate(160%);
  -webkit-backdrop-filter: blur(20px) saturate(160%);
  box-shadow: 0 8px 26px rgba(0, 0, 0, .1), var(--inset-highlight);
}
.mobile-dock {
  position: absolute;
  right: 0;
  bottom: 0;
  width: calc(100% - (var(--dock-width) - var(--dock-orb-size)) * var(--dock-progress));
  height: calc(var(--mobile-dock-height) - (var(--mobile-dock-height) - var(--dock-orb-size)) * var(--dock-progress));
  border-radius: calc(20px + 8px * var(--dock-progress));
  overflow: hidden;
  pointer-events: auto;
}
.mobile-dock-links {
  position: absolute;
  right: 0;
  top: 0;
  width: calc(var(--dock-width) - 2px);
  height: calc(var(--mobile-dock-height) - 2px);
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  padding: 4px;
  opacity: clamp(0, 1 - var(--dock-progress) * 4, 1);
  transform: translate3d(calc(12px * var(--dock-progress)), 0, 0);
}
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
  opacity: clamp(0, (var(--dock-progress) - .12) * 2, 1);
  transform: scale(calc(.8 + .2 * var(--dock-progress)));
  pointer-events: none;
  transition: background-color var(--motion-fast) ease;
}
.is-compact .mobile-dock-orb { pointer-events: auto; }
.mobile-dock-orb .el-icon { font-size: 24px; }
.dock-orb-hint { position: absolute; bottom: 7px; width: 12px; height: 2px; border-radius: 2px; background: currentColor; opacity: .45; }
.mobile-sub-dock {
  position: absolute;
  bottom: 3px;
  left: calc(12px * (1 - var(--dock-progress)));
  right: calc(12px + (var(--dock-orb-size) + var(--dock-gap) - 12px) * var(--dock-progress));
  height: var(--mobile-sub-dock-height);
  display: flex;
  padding: 2px;
  border-radius: 18px;
  transform: translate3d(0, calc((var(--mobile-dock-height) + var(--dock-gap) - 3px) * (var(--dock-progress) - 1)), 0);
  pointer-events: auto;
}
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
.mobile-sub-dock-item:active { background: color-mix(in srgb, var(--accent) 18%, transparent); }
@media (prefers-reduced-transparency: reduce) {
  .dock-glass { background: var(--bg-primary); backdrop-filter: none; -webkit-backdrop-filter: none; }
}
@media (prefers-contrast: more) {
  .dock-glass { border-color: var(--text-muted); }
  .mobile-sub-dock-item.active, .mobile-dock-item.active { box-shadow: inset 0 0 0 1px var(--accent); }
}
@media (prefers-reduced-motion: reduce) {
  .mobile-dock-item, .mobile-dock-orb, .mobile-sub-dock-item { transition: none; }
}
</style>
