<template>
  <div
    ref="appShellRef"
    class="app-shell"
    :class="{ 'mobile-focus': isMobile && mobileFocusMode, 'has-mobile-subnav': !!mobileSubnav?.items.length }"
  >
    <!-- Mobile root header: one title. Page-level actions occupy the right slot. -->
    <div v-if="isMobile && !mobileFocusMode" v-glass-lens="'css'" class="mobile-global-header" data-glass="scroll-edge" :class="{ 'header-scrolled': isScrolled }">
      <div class="mobile-header-spacer"></div>
      <span class="mobile-page-title">{{ currentTitle }}</span>
      <div class="mobile-header-spacer"></div>
    </div>

    <!-- Mobile sidebar overlay backdrop -->
    <transition name="scrim-fade">
      <div
        v-if="mobileSidebarVisible"
        class="mobile-overlay"
        aria-hidden="true"
        @click="closeMobileSidebar"
      ></div>
    </transition>

    <el-container class="app-container">
      <aside
        ref="appAsideRef"
        class="app-aside"
        :class="{ 'mobile-open': isMobile && !isCollapsed }"
        :style="{ width: isMobile ? '100%' : (isCollapsed ? '72px' : '230px') }"
        :role="isMobile && mobileSidebarVisible ? 'dialog' : undefined"
        :aria-modal="isMobile && mobileSidebarVisible ? 'true' : undefined"
        :aria-label="isMobile && mobileSidebarVisible ? '全部功能' : undefined"
        :aria-hidden="isMobile && !mobileSidebarVisible ? 'true' : undefined"
        :inert="isMobile && !mobileSidebarVisible ? true : undefined"
        :tabindex="isMobile && mobileSidebarVisible ? -1 : undefined"
      >
        <Sidebar :expanded-on-mobile="isMobile && mobileSidebarVisible" />
      </aside>
      <el-main
        class="app-main"
        :class="{
          'mobile-full': isMobile,
          'mobile-focus': isMobile && mobileFocusMode,
          'reader-scroll-locked': lockMobileReaderOuterScroll,
        }"
        @scroll="onMainScroll"
      >
        <div class="route-stage">
          <router-view v-slot="{ Component, route }">
            <transition name="page-slide" @before-enter="enableEnteringPage" @before-leave="disableLeavingPage">
              <component :is="Component" :key="route.path" />
            </transition>
          </router-view>
        </div>
      </el-main>
    </el-container>

    <MobileDock
      v-if="isMobile && !mobileFocusMode && !dockKeyboardOpen"
      :progress="dockProgress"
      :section="mobileNavSection"
      :sidebar-visible="mobileSidebarVisible"
      :subnav="mobileSubnav"
      @expand="expandDock"
      @open-modules="openMobileSidebar"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import Sidebar from './components/Sidebar.vue'
import MobileDock from './components/MobileDock.vue'
import { isPhoneViewport, shouldLockMobileReaderOuterScroll } from './utils/mobileLayoutPolicy'
import { getMobileNavSection, isMobileFocusRoute } from './utils/mobileNavigationPolicy'
import { useModalEnvironment } from './composables/useModalEnvironment'
import { provideMobileSubnav } from './composables/useMobileSubnav'
import { useMobileDockMotion } from './composables/useMobileDockMotion'

const route = useRoute()
const appStore = useAppStore()
// Reset scroll state when navigating between pages.
watch(() => route.path, async path => {
  appStore.handleScroll(0)
  await nextTick()
  // Home owns its return position; other destinations start with controls visible.
  if (path !== '/') {
    const main = appShellRef.value?.querySelector('.app-main')
    if (main) main.scrollTop = 0
  }
})
const isCollapsed = computed(() => appStore.sidebarCollapsed)
const currentTitle = computed(() => (route.meta?.title as string) || '')
const appAsideRef = ref<HTMLElement | null>(null)
const appShellRef = ref<HTMLElement | null>(null)
const mobileSubnav = provideMobileSubnav()

// Mobile detection
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => isPhoneViewport(windowWidth.value))
// An expanded desktop rail must not become an unsolicited modal on resize.
watch(isMobile, (mobile) => {
  if (mobile) appStore.setSidebarCollapsed(true)
})
const mobileFocusMode = computed(() => isMobileFocusRoute(route.path, route.query))
const mobileNavSection = computed(() => getMobileNavSection(route.path))
const { progress: dockProgress, keyboardOpen: dockKeyboardOpen, expand: expandDock } = useMobileDockMotion(
  appShellRef,
  () => isMobile.value && !mobileFocusMode.value,
  () => route.path,
)
const lockMobileReaderOuterScroll = computed(() => (
  shouldLockMobileReaderOuterScroll(windowWidth.value, route.path)
))

const mobileSidebarVisible = computed(() => isMobile.value && !isCollapsed.value)
useModalEnvironment(
  () => isMobile.value && !isCollapsed.value,
  appAsideRef,
  closeMobileSidebar,
)

function openMobileSidebar() {
  appStore.setSidebarCollapsed(false)
}

function closeMobileSidebar() {
  appStore.setSidebarCollapsed(true)
}

function disableLeavingPage(element: Element) {
  // Fixed descendants can override pointer-events:none; inert also removes
  // outgoing controls from keyboard/assistive navigation during the cross-fade.
  if (element instanceof HTMLElement) element.inert = true
}
function enableEnteringPage(element: Element) {
  if (element instanceof HTMLElement) element.inert = false
}
function syncVisualViewport() {
  const viewport = window.visualViewport
  document.documentElement.style.setProperty('--visual-viewport-height', `${viewport?.height ?? window.innerHeight}px`)
  document.documentElement.style.setProperty('--visual-viewport-top', `${viewport?.offsetTop ?? 0}px`)
}

// Scroll detection for mobile header (shared via store so the Reader's internal
// pane-center scroll can also drive the global header + page-header collapse).
const isScrolled = computed(() => appStore.isScrolled)
function onMainScroll(e: Event) {
  const el = e.target as HTMLElement
  appStore.handleScroll(el.scrollTop)
}

function onResize() {
  windowWidth.value = window.innerWidth
}
onMounted(() => {
  syncVisualViewport()
  window.visualViewport?.addEventListener('resize', syncVisualViewport)
  window.visualViewport?.addEventListener('scroll', syncVisualViewport)
  window.addEventListener('resize', onResize)
  if (isMobile.value && !isCollapsed.value) appStore.setSidebarCollapsed(true)
})
onUnmounted(() => {
  window.visualViewport?.removeEventListener('resize', syncVisualViewport)
  window.visualViewport?.removeEventListener('scroll', syncVisualViewport)
  document.documentElement.style.removeProperty('--visual-viewport-height')
  document.documentElement.style.removeProperty('--visual-viewport-top')
  window.removeEventListener('resize', onResize)
})
</script>

<style>
/* ── Global Reset ── */
*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  -webkit-tap-highlight-color: transparent;
}

html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--bg-base);
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: var(--font-ui);
  line-height: var(--leading-normal);
  font-optical-sizing: auto;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

html.reader-mobile-immersive .mobile-global-header {
  display: none !important;
}

/* ── Typography: size-specific tracking (Apple Design §15) ── */
.page-title, h1, h2, h3 { letter-spacing: var(--tracking-tight); }
.page-subtitle, .el-tag, .stat-chip .stat-label { letter-spacing: var(--tracking-wide); }
code, pre, .code-block { font-family: var(--font-mono); }

/* ── Global page-header (shared by all views) ── */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 24px;
  flex-shrink: 0;
}
.page-title {
  font-size: 22px;
  font-weight: 650;
  line-height: var(--leading-tight);
  color: var(--text-primary);
  letter-spacing: var(--tracking-tight);
  margin: 0;
}
.page-subtitle {
  margin: 4px 0 0;
  color: var(--text-faint);
  font-size: 14px;
  letter-spacing: var(--tracking-wide);
}
.header-actions { display: flex; gap: 8px; flex-shrink: 0; }

/* Header action buttons share one glass treatment on every view (Tasks' 新建任务
   set the bar): 42px tap size, 13px radius, glass fill, accent fill for primary
   actions. Colors ride Element Plus's button vars so hover/active/loading stay
   native; only radius/weight need !important to beat the global .el-button rules
   below. Placed before the mobile and dark blocks so those can still override:
   mobile raises the tap size to 44px, dark steers its own glass palette. */
.header-actions .el-button {
  --el-button-bg-color: var(--bg-glass);
  --el-button-text-color: var(--text-primary);
  --el-button-border-color: var(--border-subtle);
  --el-button-hover-bg-color: var(--bg-glass-strong);
  --el-button-hover-text-color: var(--text-primary);
  --el-button-hover-border-color: var(--border-subtle);
  --el-button-active-bg-color: var(--bg-glass-strong);
  --el-button-active-text-color: var(--text-primary);
  --el-button-active-border-color: var(--border-subtle);
  min-height: 42px;
  padding: 0 15px;
  border-radius: 13px !important;
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  font-weight: 580 !important;
}
.header-actions .el-button--primary {
  --el-button-text-color: var(--text-primary);
  --el-button-hover-text-color: var(--text-primary);
  --el-button-active-text-color: var(--text-primary);
}

/* ── Task attribute pills (shared by Tasks view + subtask drawer) ── */
.task-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 22px;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 620;
  line-height: 1;
  white-space: nowrap;
}
.task-pill.status-open, .task-pill.status-cancelled { background: color-mix(in srgb, #8e8e93 15%, transparent); color: color-mix(in srgb, #8e8e93 85%, var(--text-primary)); }
.task-pill.status-planned { background: color-mix(in srgb, var(--text-primary) 7%, transparent); color: var(--text-secondary); }
.task-pill.status-in_progress { background: color-mix(in srgb, var(--accent) 13%, transparent); color: color-mix(in srgb, var(--accent) 82%, var(--text-primary)); }
.task-pill.status-blocked { background: color-mix(in srgb, var(--warning) 12%, transparent); color: var(--warning); }
.task-pill.status-completed { background: color-mix(in srgb, var(--success) 12%, transparent); color: var(--success); }
.task-pill.importance-low { background: color-mix(in srgb, #8e8e93 13%, transparent); color: var(--text-muted); }
.task-pill.importance-normal { background: color-mix(in srgb, var(--text-primary) 6%, transparent); color: var(--text-secondary); }
.task-pill.importance-high { background: color-mix(in srgb, var(--warning) 12%, transparent); color: var(--warning); }
.task-pill.importance-urgent { background: color-mix(in srgb, var(--danger) 12%, transparent); color: var(--danger); }
.task-pill.type-progress { background: color-mix(in srgb, var(--accent) 12%, transparent); color: color-mix(in srgb, var(--accent) 82%, var(--text-primary)); }
.task-pill.type-audit { background: color-mix(in srgb, var(--text-primary) 6%, transparent); color: var(--text-secondary); }

/* ── Scrollbar ── */
::-webkit-scrollbar { width: 5px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.08);
  border-radius: 3px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.15);
}

/* Theme colors and shared geometry are owned by styles/tokens.css. */

/* Material palettes and surface weights are owned by styles/materials.css. */

/* ── Element Plus overrides ── */
.el-card {
  border-radius: 20px !important;
  transition: box-shadow var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-fast) var(--ease-emphasized) !important;
}
.el-card:hover {
  box-shadow:
    0 4px 16px rgba(0, 0, 0, 0.06),
    var(--inset-highlight) !important;
}

.el-tag {
  border-radius: 10px !important;
  font-weight: 500 !important;
}

.el-button {
  border-radius: 10px !important;
  font-weight: 500 !important;
  transform: translateZ(0);
}

/* Mobile: buttons should be bigger and easier to tap. */
@media (max-width: 768px) {
  .el-button {
    min-height: var(--tap-target) !important;
    font-size: 14px !important;
    padding: 8px 16px !important;
  }
  .el-button--small {
    min-height: var(--tap-target) !important;
    font-size: 13px !important;
    padding: 7px 14px !important;
  }
  .header-actions .el-button {
    min-height: var(--tap-target) !important;
  }

  .el-input__inner,
  .el-textarea__inner,
  .el-select__selected-item,
  input,
  textarea,
  select {
    font-size: 16px !important;
  }

  .el-input__wrapper,
  .el-select__wrapper,
  .el-input-number,
  .el-date-editor {
    min-height: var(--tap-target) !important;
    backdrop-filter: none !important;
    -webkit-backdrop-filter: none !important;
  }

  .el-card,
  .stat-card, .module-card, .status-card, .tool-card,
  .stat-chip, .result-card, .recent-card,
  .repo-card {
    background: color-mix(in srgb, var(--bg-glass-strong) 88%, var(--bg-base)) !important;
    backdrop-filter: none !important;
    -webkit-backdrop-filter: none !important;
  }

}

.el-empty__description p {
  color: var(--text-faint);
}

/* el-message glass styling (global — must be in App.vue, not per-view, so it loads on all routes) */
.el-message {
  border-radius: 16px !important;
  border: 1px solid var(--border-glass) !important;
  background: var(--bg-glass-strong) !important;
  backdrop-filter: var(--glass-panel-filter) !important;
  -webkit-backdrop-filter: var(--glass-panel-filter) !important;
  box-shadow: var(--shadow-lg), var(--shadow-sm), var(--inset-highlight) !important;
  padding: 14px 22px !important;
}
.el-message .el-message__content {
  font-size: 14px !important;
  font-weight: 500 !important;
  color: var(--text-primary) !important;
}

/* ── Mobile: a single app title with one page action in the right slot. ── */
@media (max-width: 768px) {
  .page-header {
    position: fixed;
    inset: 0 0 auto 0;
    z-index: 1002;
    height: calc(var(--mobile-header-height) + var(--safe-top));
    margin: 0 !important;
    padding: var(--safe-top) max(12px, var(--safe-right)) 0 max(12px, var(--safe-left));
    display: flex;
    align-items: center;
    justify-content: flex-end;
    pointer-events: none;
  }

  .page-header > :first-child { display: none !important; }
  .page-header > :not(:first-child) { pointer-events: auto; }

  .header-actions {
    position: relative;
    z-index: 1;
    max-width: min(44vw, 152px);
    pointer-events: auto;
    overflow: visible;
  }

  .header-actions .el-button:not(:last-child) { display: none; }
  .header-actions .el-button { min-height: var(--tap-target); }

  .app-shell.mobile-focus .page-header { display: none !important; }
}

/* ── Unified keyframes (deduplicated) ── */
@keyframes spin { to { transform: rotate(360deg); } }
@keyframes fade-in {
  from { opacity: 0; transform: translateY(8px) scale(0.992); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}
@keyframes slide-up {
  from { opacity: 0; transform: translateY(12px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>

<style scoped>
.app-shell {
  height: 100vh;
  height: 100dvh;
  width: 100%;
  position: relative;
  overflow: hidden;
  background: var(--bg-base);
  transition: background-color var(--motion-fast) var(--ease-emphasized);
  --mobile-sidebar-x: -260px;
  --mobile-sidebar-progress: 0;
  --mobile-content-scale: 1;
  --mobile-content-shift: 0px;
}

.app-container {
  height: 100vh;
  height: 100dvh;
  position: relative;
}

.app-aside {
  flex-shrink: 0;
  transition: width var(--motion-slow) var(--ease-spring-gentle);
  overflow: hidden;
  background: transparent;
}

.app-main {
  background: transparent;
  padding: 32px var(--page-gutter);
  overflow-y: auto;
  overflow-x: hidden;
}

.route-stage {
  position: relative;
  min-height: 100%;
}

.route-stage > * { width: 100%; }

/* ── Page Transition — stable cross-fade, without locking navigation ── */
.page-slide-enter-active {
  transition: opacity var(--motion-page) var(--ease-emphasized);
}
.page-slide-leave-active {
  position: absolute;
  inset: 0;
  pointer-events: none;
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}
.page-slide-enter-from {
  opacity: 0;
}
.page-slide-leave-to {
  opacity: 0;
}

/* ── Mobile Global Header ── */
.mobile-global-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 1001;
  min-height: calc(var(--mobile-header-height) + var(--safe-top));
  display: flex;
  align-items: center;
  padding: var(--safe-top) calc(12px + var(--safe-right)) 0 calc(12px + var(--safe-left));
}

.mobile-page-title {
  flex: 1;
  text-align: center;
  font-size: 19px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mobile-header-spacer {
  width: var(--tap-target);
  flex-shrink: 0;
}

.mobile-overlay {
  position: fixed;
  inset: 0;
  z-index: 1090;
  background: rgba(0, 0, 0, 0.3);
  opacity: 1;
  backdrop-filter: var(--glass-scrim-filter);
  -webkit-backdrop-filter: var(--glass-scrim-filter);
  touch-action: none;
}

.scrim-fade-enter-active,
.scrim-fade-leave-active { transition: opacity var(--motion-fast) var(--ease-emphasized); }
.scrim-fade-enter-from,
.scrim-fade-leave-to { opacity: 0; }

/* Mobile module sheet shares the same physical bottom origin as other sheets. */
.app-aside {
  transition: width var(--motion-slow) var(--ease-spring-gentle),
              transform var(--motion-normal) var(--ease-spring-gentle),
              box-shadow var(--motion-normal) var(--ease-emphasized);
}

.has-mobile-subnav { --mobile-navigation-height: calc(var(--mobile-dock-height) + var(--mobile-sub-dock-height) + 8px); }

@media (min-width: 769px) and (prefers-reduced-motion: reduce) {
  .app-aside { transition-property: box-shadow; }
}

.app-main.mobile-full {
  padding: 16px max(12px, var(--safe-right)) calc(var(--mobile-navigation-height) + max(8px, var(--safe-bottom)) + 12px) max(12px, var(--safe-left));
  padding-top: calc(var(--mobile-header-height) + var(--safe-top) + 8px);
  overflow-x: hidden;
  width: 100%;
  max-width: 100%;
}

.app-main.mobile-full.mobile-focus {
  padding-top: max(8px, var(--safe-top));
  padding-bottom: max(8px, var(--safe-bottom));
}

/* Reader owns its vertical scroll on phones. Keeping the shell fixed prevents
   a gesture at the document boundary from exposing persistent outer padding. */
.app-main.mobile-full.reader-scroll-locked {
  overflow-y: hidden;
  overscroll-behavior-y: none;
}
.app-main.mobile-full.reader-scroll-locked .route-stage {
  min-height: 0;
  height: 100%;
}

@media (max-width: 768px) {
  .app-aside {
    position: fixed;
    top: auto;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 1100;
    width: 100% !important;
    height: min(76dvh, 660px);
    border-radius: 26px 26px 0 0;
    transform: translate3d(0, 100%, 0);
    will-change: transform;
    touch-action: pan-y;
  }

  .app-shell { touch-action: auto; }

  .route-stage {
    min-height: calc(100dvh - var(--mobile-header-height) - var(--safe-top));
  }

  .app-aside.mobile-open {
    top: auto;
    right: 0;
    height: min(76dvh, 660px);
    transform: translate3d(0, 0, 0);
    box-shadow: 0 -18px 48px rgba(0, 0, 0, 0.18);
  }

}

@media (max-width: 480px) {
  .app-main.mobile-full {
    padding-left: max(10px, var(--safe-left));
    padding-right: max(10px, var(--safe-right));
  }
}
</style>

<style src="./styles/materials.css"></style>
<style src="./styles/tokens.css"></style>
<style src="./styles/controls.css"></style>
