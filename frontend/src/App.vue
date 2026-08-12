<template>
  <div
    ref="appShellRef"
    class="app-shell"
    :class="{ 'sidebar-dragging': isSidebarDragging }"
    @pointerdown="onSidebarPointerDown"
    @pointermove="onSidebarPointerMove"
    @pointerup="onSidebarPointerUp"
    @pointercancel="onSidebarPointerCancel"
  >
    <!-- Ambient gradient orbs for liquid glass effect -->
    <div class="ambient-bg">
      <div class="orb orb-1"></div>
      <div class="orb orb-2"></div>
      <div class="orb orb-3"></div>
    </div>
    <!-- Subtle grain texture background -->
    <div class="bg-grain"></div>

    <!-- Mobile global header: hamburger + centered page title -->
    <div class="mobile-global-header" v-if="isMobile" :class="{ 'header-scrolled': isScrolled }">
      <button
        class="mobile-menu-btn"
        type="button"
        aria-label="打开导航"
        :aria-expanded="!isCollapsed"
        @click="openMobileSidebar"
      >
        <el-icon :size="20"><component :is="isCollapsed ? Expand : Fold" /></el-icon>
      </button>
      <transition name="title-fade">
        <span v-if="isScrolled" class="mobile-page-title">{{ currentTitle }}</span>
      </transition>
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
      <el-aside
        :width="isMobile ? '260px' : (isCollapsed ? '72px' : '230px')"
        class="app-aside"
        :class="{ 'mobile-open': isMobile && !isCollapsed }"
      >
        <Sidebar />
      </el-aside>
      <el-main
        class="app-main"
        :class="{ 'mobile-full': isMobile, 'mobile-scrolled': isMobile && isScrolled }"
        @scroll="onMainScroll"
      >
        <div class="route-stage">
          <router-view v-slot="{ Component, route }">
            <transition name="page-slide">
              <component :is="Component" :key="route.path" />
            </transition>
          </router-view>
        </div>
      </el-main>
    </el-container>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import { Expand, Fold } from '@element-plus/icons-vue'
import Sidebar from './components/Sidebar.vue'

const route = useRoute()
// Reset scroll state when navigating between pages.
watch(() => route.path, () => appStore.setScrolled(false))
const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)
const currentTitle = computed(() => (route.meta?.title as string) || '')
const appShellRef = ref<HTMLElement | null>(null)

// Mobile detection
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => windowWidth.value <= 768)

// Mobile navigation follows the pointer 1:1. On release, position and recent
// velocity are projected forward before snapping to the nearest resting state.
const DRAWER_WIDTH = 260
const EDGE_ACTIVATION_WIDTH = 24
const GESTURE_THRESHOLD = 8
const isSidebarDragging = ref(false)
const mobileSidebarVisible = computed(() => isMobile.value && (!isCollapsed.value || isSidebarDragging.value))
let pointerId: number | null = null
let pointerStartX = 0
let pointerStartY = 0
let pointerBaseX = -DRAWER_WIDTH
let pointerAxis: 'pending' | 'horizontal' | 'vertical' = 'pending'
let drawerX = -DRAWER_WIDTH
let samples: Array<{ x: number; time: number }> = []

function rubberband(overshoot: number, dimension: number, constant = 0.55) {
  return (overshoot * dimension * constant) / (dimension + constant * Math.abs(overshoot))
}

function projectVelocity(velocity: number, decelerationRate = 0.995) {
  return (velocity / 1000) * decelerationRate / (1 - decelerationRate)
}

function applyMobileSidebarPosition(x: number) {
  drawerX = x
  const progress = Math.max(0, Math.min(1, (x + DRAWER_WIDTH) / DRAWER_WIDTH))
  const shell = appShellRef.value
  shell?.style.setProperty('--mobile-sidebar-x', `${x}px`)
  shell?.style.setProperty('--mobile-sidebar-progress', String(progress))
  shell?.style.setProperty('--mobile-content-scale', String(1 - progress * 0.015))
  shell?.style.setProperty('--mobile-content-shift', `${progress * 8}px`)
}

function syncMobileSidebarPosition() {
  applyMobileSidebarPosition(isCollapsed.value ? -DRAWER_WIDTH : 0)
}

function openMobileSidebar() {
  appStore.setSidebarCollapsed(false)
  requestAnimationFrame(syncMobileSidebarPosition)
}

function closeMobileSidebar() {
  appStore.setSidebarCollapsed(true)
  requestAnimationFrame(syncMobileSidebarPosition)
}

function onSidebarPointerDown(event: PointerEvent) {
  if (!isMobile.value || (event.pointerType === 'mouse' && event.button !== 0)) return
  const drawerOpen = !isCollapsed.value
  if (!drawerOpen && event.clientX > EDGE_ACTIVATION_WIDTH) return

  pointerId = event.pointerId
  pointerStartX = event.clientX
  pointerStartY = event.clientY
  pointerBaseX = drawerOpen ? 0 : -DRAWER_WIDTH
  drawerX = pointerBaseX
  pointerAxis = 'pending'
  samples = [{ x: event.clientX, time: performance.now() }]
  isSidebarDragging.value = true
  applyMobileSidebarPosition(pointerBaseX)
}

function onSidebarPointerMove(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  const dx = event.clientX - pointerStartX
  const dy = event.clientY - pointerStartY

  if (pointerAxis === 'pending') {
    if (Math.max(Math.abs(dx), Math.abs(dy)) < GESTURE_THRESHOLD) return
    pointerAxis = Math.abs(dx) > Math.abs(dy) ? 'horizontal' : 'vertical'
    if (pointerAxis === 'vertical') {
      cancelSidebarGesture()
      return
    }
    appShellRef.value?.setPointerCapture(event.pointerId)
  }

  event.preventDefault()
  let nextX = pointerBaseX + dx
  if (nextX > 0) nextX = rubberband(nextX, DRAWER_WIDTH)
  if (nextX < -DRAWER_WIDTH) nextX = -DRAWER_WIDTH + rubberband(nextX + DRAWER_WIDTH, DRAWER_WIDTH)
  applyMobileSidebarPosition(nextX)

  const now = performance.now()
  samples.push({ x: event.clientX, time: now })
  samples = samples.filter((sample) => now - sample.time <= 100)
}

function finishSidebarGesture(commit: boolean) {
  if (pointerId !== null && appShellRef.value?.hasPointerCapture(pointerId)) {
    appShellRef.value.releasePointerCapture(pointerId)
  }
  pointerId = null
  isSidebarDragging.value = false

  if (!commit || pointerAxis !== 'horizontal') {
    syncMobileSidebarPosition()
    return
  }

  const first = samples[0]
  const last = samples[samples.length - 1]
  const elapsed = first && last ? Math.max(1, last.time - first.time) : 1
  const velocity = first && last ? ((last.x - first.x) / elapsed) * 1000 : 0
  const projected = drawerX + projectVelocity(velocity)
  const shouldOpen = projected > -DRAWER_WIDTH / 2
  appStore.setSidebarCollapsed(!shouldOpen)
  requestAnimationFrame(syncMobileSidebarPosition)
}

function cancelSidebarGesture() {
  finishSidebarGesture(false)
}

function onSidebarPointerUp(event: PointerEvent) {
  if (event.pointerId === pointerId) finishSidebarGesture(true)
}

function onSidebarPointerCancel(event: PointerEvent) {
  if (event.pointerId === pointerId) cancelSidebarGesture()
}

// Scroll detection for mobile header (shared via store so the Reader's internal
// pane-center scroll can also drive the global header + page-header collapse).
const isScrolled = computed(() => appStore.isScrolled)
function onMainScroll(e: Event) {
  const el = e.target as HTMLElement
  appStore.setScrolled(el.scrollTop > 20)
}

function onResize() {
  windowWidth.value = window.innerWidth
  if (isMobile.value) syncMobileSidebarPosition()
}
watch(isCollapsed, () => {
  if (isMobile.value && !isSidebarDragging.value) requestAnimationFrame(syncMobileSidebarPosition)
})
onMounted(() => {
  window.addEventListener('resize', onResize)
  if (isMobile.value && !isCollapsed.value) appStore.setSidebarCollapsed(true)
  requestAnimationFrame(syncMobileSidebarPosition)
})
onUnmounted(() => {
  window.removeEventListener('resize', onResize)
  if (pointerId !== null) cancelSidebarGesture()
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
  font-size: 15px;
  line-height: var(--leading-normal);
  font-optical-sizing: auto;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
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
  font-weight: 600;
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

/* ═══════════════════════════════════════════════════
   Theme System — CSS Custom Properties
   ═══════════════════════════════════════════════════ */

/* ── Light Theme (default) ── */
:root, :root[data-theme="light"] {
  --bg-base: #f0f0f3;
  --bg-glass: rgba(255, 255, 255, 0.45);
  --bg-glass-strong: rgba(255, 255, 255, 0.65);
  --bg-glass-subtle: rgba(255, 255, 255, 0.3);
  --bg-hover: rgba(255, 255, 255, 0.4);
  --border-glass: rgba(255, 255, 255, 0.7);
  --border-subtle: rgba(255, 255, 255, 0.5);
  --border-faint: rgba(0, 0, 0, 0.05);

  --text-primary: #18181b;
  --text-secondary: #27272a;
  --text-tertiary: #52525b;
  --text-muted: #71717a;
  --text-faint: #a1a1aa;

  --accent: #6366f1;
  --accent-light: rgba(129, 140, 248, 0.12);
  --accent-border: rgba(129, 140, 248, 0.4);

  --code-bg: rgba(24, 24, 27, 0.06);
  --code-inline-color: #c026d3;
  --code-block-bg: #1e1e2e;
  --code-block-text: #cdd6f4;

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.06), 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-lg: 0 8px 32px rgba(0, 0, 0, 0.08);
  --inset-highlight: inset 0 1px 1px rgba(255, 255, 255, 0.6);
  --glass-blur: 20px;
  --glass-saturate: 180%;

  --orb-opacity: 1;

  /* ── Design Tokens (Apple Design) ── */
  --ease-standard: cubic-bezier(0.4, 0, 0.2, 1);
  --ease-out: cubic-bezier(0.32, 0.72, 0, 1);
  --ease-spring: cubic-bezier(0.2, 0.8, 0.2, 1);
  --ease-ios: cubic-bezier(0.25, 0.46, 0.45, 0.94);
  --duration-fast: 0.2s;
  --duration-normal: 0.3s;
  --duration-slow: 0.45s;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif;
  --font-mono: 'SF Mono', 'JetBrains Mono', Menlo, Consolas, monospace;
  --tracking-tight: -0.02em;
  --tracking-normal: 0;
  --tracking-wide: 0.02em;
  --leading-tight: 1.2;
  --leading-normal: 1.5;
  --leading-relaxed: 1.75;
}

/* ── Dark Theme — Deep Black ── */
:root[data-theme="dark"] {
  --bg-base: #000000;
  --bg-glass: rgba(20, 20, 25, 0.4);
  --bg-glass-strong: rgba(25, 25, 30, 0.6);
  --bg-glass-subtle: rgba(30, 30, 35, 0.25);
  --bg-hover: rgba(40, 40, 45, 0.4);
  --border-glass: rgba(255, 255, 255, 0.08);
  --border-subtle: rgba(255, 255, 255, 0.06);
  --border-faint: rgba(255, 255, 255, 0.04);

  --text-primary: #ffffff;
  --text-secondary: #e5e5e5;
  --text-tertiary: #b3b3b3;
  --text-muted: #808080;
  --text-faint: #555555;

  --accent: #7c7cff;
  --accent-light: rgba(124, 124, 255, 0.12);
  --accent-border: rgba(124, 124, 255, 0.35);

  --code-bg: rgba(255, 255, 255, 0.06);
  --code-inline-color: #ff79c6;
  --code-block-bg: #0d0d0d;
  --code-block-text: #e5e5e5;

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.3);
  --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.2), 0 1px 2px rgba(0, 0, 0, 0.15);
  --shadow-lg: 0 8px 32px rgba(0, 0, 0, 0.25);
  --inset-highlight: inset 0 1px 1px rgba(255, 255, 255, 0.05);
  --glass-blur: 24px;
  --glass-saturate: 160%;

  --orb-opacity: 0.35;
}

/* ── Eye-Care Theme — Soft Green ── */
:root[data-theme="eye-care"] {
  --bg-base: #c5d5b8;
  --bg-glass: rgba(180, 215, 165, 0.55);
  --bg-glass-strong: rgba(165, 205, 150, 0.72);
  --bg-glass-subtle: rgba(150, 195, 135, 0.38);
  --bg-hover: rgba(135, 185, 120, 0.5);
  --border-glass: rgba(100, 155, 85, 0.55);
  --border-subtle: rgba(85, 140, 70, 0.42);
  --border-faint: rgba(30, 60, 20, 0.12);

  --text-primary: #152618;
  --text-secondary: #1e3320;
  --text-tertiary: #355030;
  --text-muted: #4e6e45;
  --text-faint: #708e60;

  --accent: #2e7d4a;
  --accent-light: rgba(46, 125, 74, 0.18);
  --accent-border: rgba(46, 125, 74, 0.5);

  --code-bg: rgba(20, 50, 15, 0.12);
  --code-inline-color: #1b6e2e;
  --code-block-bg: #152612;
  --code-block-text: #9dd495;

  --shadow-sm: 0 1px 2px rgba(30, 60, 20, 0.08);
  --shadow-md: 0 2px 8px rgba(30, 60, 20, 0.1), 0 1px 2px rgba(30, 60, 20, 0.06);
  --shadow-lg: 0 8px 32px rgba(30, 60, 20, 0.12);
  --inset-highlight: inset 0 1px 1px rgba(255, 255, 245, 0.6);
  --glass-blur: 20px;
  --glass-saturate: 180%;

  --orb-opacity: 1;
}

/* ── Glass Card 全局覆盖 ── */
.el-card,
.stat-card, .module-card, .status-card, .tool-card,
.stat-chip, .result-card, .recent-card,
.repo-card, .mode-option, .radar-card,
.insight-card, .config-card,
.combo-result, .question-result, .counterpoint-result,
.combo-result .concept-chip,
.question-result .question-item,
.counterpoint-result .counterpoint-item,
.assessment {
  background: var(--bg-glass) !important;
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate)) !important;
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate)) !important;
  border: 1px solid var(--border-glass) !important;
  box-shadow: var(--shadow-md), var(--inset-highlight) !important;
}

/* ── Glass Tag 全局覆盖 ── */
.lang-tag, .tag, .stat-chip .stat-label {
  background: var(--bg-glass-subtle) !important;
  backdrop-filter: blur(8px) !important;
  -webkit-backdrop-filter: blur(8px) !important;
  border: 1px solid var(--border-subtle) !important;
}

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
    min-height: 38px !important;
    font-size: 14px !important;
    padding: 8px 16px !important;
  }
  .el-button--small {
    min-height: 36px !important;
    font-size: 13px !important;
    padding: 7px 14px !important;
  }
  .header-actions .el-button {
    min-height: 36px !important;
  }
}

.el-empty__description p {
  color: var(--text-faint);
}

/* ── Dark theme Element Plus overrides — Deep Black ── */
:root[data-theme="dark"] {
  --el-bg-color: #141414;
  --el-bg-color-overlay: #1a1a1a;
  --el-bg-color-page: #000000;
  --el-text-color-primary: #ffffff;
  --el-text-color-regular: #e5e5e5;
  --el-text-color-secondary: #b3b3b3;
  --el-text-color-placeholder: #555555;
  --el-border-color: rgba(55, 55, 55, 0.6);
  --el-border-color-light: rgba(45, 45, 45, 0.5);
  --el-border-color-lighter: rgba(40, 40, 40, 0.4);
  --el-fill-color: rgba(28, 28, 28, 0.6);
  --el-fill-color-light: rgba(28, 28, 28, 0.5);
  --el-fill-color-lighter: rgba(28, 28, 28, 0.3);
  --el-fill-color-blank: transparent;
  --el-color-primary: #7c7cff;
  --el-mask-color: rgba(0, 0, 0, 0.7);
  --el-overlay-color: rgba(0, 0, 0, 0.7);
  --el-disabled-bg-color: rgba(28, 28, 28, 0.5);
  --el-disabled-text-color: #555555;
  --el-disabled-border-color: rgba(45, 45, 45, 0.5);
}

/* Dark: el-input, el-select, el-date-picker */
:root[data-theme="dark"] .el-input__wrapper {
  background: var(--bg-glass-subtle) !important;
  box-shadow: 0 0 0 1px var(--border-glass) inset !important;
}
:root[data-theme="dark"] .el-input__wrapper:hover {
  box-shadow: 0 0 0 1px rgba(124, 124, 255, 0.3) inset !important;
}
:root[data-theme="dark"] .el-input__wrapper.is-focus {
  box-shadow: 0 0 0 2px rgba(124, 124, 255, 0.2) inset !important;
}
:root[data-theme="dark"] .el-input__inner {
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-input__inner::placeholder {
  color: var(--text-faint) !important;
}
:root[data-theme="dark"] .el-input__icon,
:root[data-theme="dark"] .el-range__icon,
:root[data-theme="dark"] .el-range__close-icon {
  color: var(--text-muted) !important;
}
:root[data-theme="dark"] .el-select__wrapper {
  background: var(--bg-glass-subtle) !important;
  box-shadow: 0 0 0 1px var(--border-glass) inset !important;
}
:root[data-theme="dark"] .el-select__wrapper.is-hovering {
  box-shadow: 0 0 0 1px rgba(124, 124, 255, 0.3) inset !important;
}
:root[data-theme="dark"] .el-select__placeholder {
  color: var(--text-faint) !important;
}
:root[data-theme="dark"] .el-select__selected-item {
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-range-editor {
  background: var(--bg-glass-subtle) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="dark"] .el-range-editor:hover {
  border-color: rgba(124, 124, 255, 0.3) !important;
}
:root[data-theme="dark"] .el-range-input {
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-range-input::placeholder {
  color: var(--text-faint) !important;
}
:root[data-theme="dark"] .el-range-separator {
  color: var(--text-muted) !important;
}

/* Dark: el-dialog */
:root[data-theme="dark"] .el-dialog {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="dark"] .el-dialog__header {
  border-bottom: 1px solid var(--border-faint) !important;
}
:root[data-theme="dark"] .el-dialog__title {
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-dialog__headerbtn .el-dialog__close {
  color: var(--text-muted) !important;
}
:root[data-theme="dark"] .el-dialog__headerbtn:hover .el-dialog__close {
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-dialog__body {
  color: var(--text-secondary) !important;
}

/* el-message glass styling (global — must be in App.vue, not per-view, so it loads on all routes) */
.el-message {
  border-radius: 16px !important;
  border: 1px solid var(--border-glass) !important;
  background: var(--bg-glass-strong) !important;
  backdrop-filter: blur(32px) saturate(200%) !important;
  -webkit-backdrop-filter: blur(32px) saturate(200%) !important;
  box-shadow: var(--shadow-lg), var(--shadow-sm), var(--inset-highlight) !important;
  padding: 14px 22px !important;
}
.el-message .el-message__content {
  font-size: 14px !important;
  font-weight: 500 !important;
  color: var(--text-primary) !important;
}

/* Dark: el-message */
:root[data-theme="dark"] .el-message {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-message--success .el-message__content {
  color: #4ade80 !important;
}
:root[data-theme="dark"] .el-message--error .el-message__content {
  color: #f87171 !important;
}

/* Dark: el-tag */
:root[data-theme="dark"] .el-tag {
  background: var(--bg-glass-subtle) !important;
  border: 1px solid var(--border-subtle) !important;
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .el-tag--success {
  background: rgba(74, 222, 128, 0.1) !important;
  border-color: rgba(74, 222, 128, 0.2) !important;
  color: #4ade80 !important;
}
:root[data-theme="dark"] .el-tag--warning {
  background: rgba(251, 191, 36, 0.1) !important;
  border-color: rgba(251, 191, 36, 0.2) !important;
  color: #fbbf24 !important;
}

/* Dark: el-button */
:root[data-theme="dark"] .el-button {
  background: var(--bg-glass) !important;
  border: 1px solid var(--border-glass) !important;
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .el-button:hover {
  background: var(--bg-hover) !important;
  border-color: rgba(124, 124, 255, 0.3) !important;
  color: var(--text-primary) !important;
}
:root[data-theme="dark"] .el-button--primary {
  background: var(--accent) !important;
  border-color: var(--accent) !important;
  color: #fff !important;
}
:root[data-theme="dark"] .el-button--primary:hover {
  background: #6a6aff !important;
  border-color: #6a6aff !important;
}
:root[data-theme="dark"] .el-button.is-disabled,
:root[data-theme="dark"] .el-button.is-loading {
  opacity: 0.4;
}

/* Dark: el-descriptions */
:root[data-theme="dark"] .el-descriptions {
  background: transparent !important;
}
:root[data-theme="dark"] .el-descriptions__label {
  background: var(--bg-glass-subtle) !important;
  color: var(--text-muted) !important;
}
:root[data-theme="dark"] .el-descriptions__content {
  background: transparent !important;
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .el-descriptions__cell {
  border-color: var(--border-faint) !important;
}
/* Eye-care: el-descriptions */
:root[data-theme="eye-care"] .el-descriptions {
  background: transparent !important;
}
:root[data-theme="eye-care"] .el-descriptions__body {
  background-color: transparent !important;
}
:root[data-theme="eye-care"] .el-descriptions__label,
:root[data-theme="eye-care"] .el-descriptions__label.el-descriptions__cell.is-bordered-label {
  background: var(--bg-glass-subtle) !important;
  color: var(--text-muted) !important;
}
:root[data-theme="eye-care"] .el-descriptions__content {
  background: transparent !important;
  color: var(--text-secondary) !important;
}
:root[data-theme="eye-care"] .el-descriptions__cell {
  border-color: var(--border-faint) !important;
}
:root[data-theme="eye-care"] .el-descriptions__table {
  border-color: var(--border-faint) !important;
}

/* Dark: el-switch */
:root[data-theme="dark"] .el-switch__core {
  background-color: var(--bg-glass-subtle) !important;
  border-color: var(--border-glass) !important;
}
:root[data-theme="dark"] .el-switch.is-checked .el-switch__core {
  background-color: var(--accent) !important;
  border-color: var(--accent) !important;
}

/* Dark: el-empty */
:root[data-theme="dark"] .el-empty__description p {
  color: var(--text-muted) !important;
}
:root[data-theme="dark"] .el-empty__image svg path {
  fill: var(--bg-glass-subtle) !important;
}

/* Dark: el-slider */
:root[data-theme="dark"] .el-slider__runway {
  background-color: var(--bg-glass-subtle) !important;
}
:root[data-theme="dark"] .el-slider__bar {
  background-color: var(--accent) !important;
}
:root[data-theme="dark"] .el-slider__button {
  border-color: var(--accent) !important;
  background-color: var(--bg-base) !important;
}

/* Dark: el-input-number */
:root[data-theme="dark"] .el-input-number {
  background: var(--bg-glass-subtle) !important;
}
/* Eye-care: el-input-number */
:root[data-theme="eye-care"] .el-input-number {
  background: var(--bg-glass-subtle) !important;
}
:root[data-theme="eye-care"] .el-input-number .el-input__wrapper {
  background: var(--bg-glass-subtle) !important;
  box-shadow: 0 0 0 1px var(--border-glass) inset !important;
}
:root[data-theme="eye-care"] .el-input-number__decrease,
:root[data-theme="eye-care"] .el-input-number__increase {
  background: var(--bg-glass-subtle) !important;
  color: var(--text-muted) !important;
  border-color: var(--border-glass) !important;
}
:root[data-theme="eye-care"] .el-input-number__decrease:hover,
:root[data-theme="eye-care"] .el-input-number__increase:hover {
  color: var(--accent) !important;
  background: var(--bg-hover) !important;
}

/* Dark: calendar popper */
:root[data-theme="dark"] .glass-picker {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="dark"] .glass-picker .el-date-table th {
  color: var(--text-muted) !important;
  border-bottom-color: var(--border-faint) !important;
}
:root[data-theme="dark"] .glass-picker .el-date-table td .el-date-table-cell:hover {
  background: var(--accent-light) !important;
}
:root[data-theme="dark"] .glass-picker .el-date-table td.today .el-date-table-cell__number {
  color: var(--accent) !important;
}
:root[data-theme="dark"] .glass-picker .el-date-table td .el-date-table-cell__number {
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .glass-picker .el-date-range-picker__header button {
  background: var(--bg-glass-subtle) !important;
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .glass-picker .el-picker-panel__footer {
  background: transparent !important;
  border-top-color: var(--border-faint) !important;
}
:root[data-theme="dark"] .glass-picker .el-picker-panel__footer button {
  background: var(--bg-glass-subtle) !important;
  color: var(--text-secondary) !important;
}

/* Dark: el-overlay (dialog backdrop) */
:root[data-theme="dark"] .el-overlay {
  background-color: rgba(0, 0, 0, 0.7) !important;
}

/* Eye-care: el-dialog + el-overlay */
:root[data-theme="eye-care"] {
  --el-bg-color: #a8c9a0;
  --el-bg-color-overlay: #b5d4ac;
  --el-bg-color-page: #a8c9a0;
  --el-text-color-primary: #152618;
  --el-text-color-regular: #243818;
  --el-text-color-secondary: #355030;
  --el-text-color-placeholder: #5a7048;
  --el-border-color: rgba(100, 155, 85, 0.5);
  --el-border-color-light: rgba(85, 140, 70, 0.4);
  --el-border-color-lighter: rgba(70, 120, 55, 0.3);
  --el-fill-color: rgba(150, 195, 135, 0.3);
  --el-fill-color-light: rgba(135, 185, 120, 0.25);
  --el-fill-color-lighter: rgba(120, 175, 105, 0.15);
  --el-fill-color-blank: transparent;
  --el-color-primary: #2e7d4a;
  --el-mask-color: rgba(30, 60, 20, 0.5);
  --el-overlay-color: rgba(30, 60, 20, 0.5);
  --el-disabled-bg-color: rgba(120, 165, 105, 0.3);
  --el-disabled-text-color: #708e60;
  --el-disabled-border-color: rgba(85, 140, 70, 0.4);
}
:root[data-theme="eye-care"] .el-dialog {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="eye-care"] .el-dialog__header {
  border-bottom: 1px solid var(--border-faint) !important;
}
:root[data-theme="eye-care"] .el-dialog__title {
  color: var(--text-primary) !important;
}
:root[data-theme="eye-care"] .el-dialog__body {
  color: var(--text-secondary) !important;
}
:root[data-theme="eye-care"] .el-overlay {
  background-color: rgba(30, 60, 20, 0.5) !important;
  backdrop-filter: blur(4px) !important;
  -webkit-backdrop-filter: blur(4px) !important;
}
:root[data-theme="eye-care"] .el-popper {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="eye-care"] .el-select-dropdown__item {
  color: var(--text-secondary) !important;
}
:root[data-theme="eye-care"] .el-select-dropdown__item.hover,
:root[data-theme="eye-care"] .el-select-dropdown__item:hover {
  background: var(--bg-hover) !important;
}
:root[data-theme="eye-care"] .el-select-dropdown__item.selected {
  color: var(--accent) !important;
}

/* Dark: el-popper (dropdown lists) */
:root[data-theme="dark"] .el-popper {
  background: var(--bg-glass-strong) !important;
  border: 1px solid var(--border-glass) !important;
}
:root[data-theme="dark"] .el-select-dropdown__item {
  color: var(--text-secondary) !important;
}
:root[data-theme="dark"] .el-select-dropdown__item.hover,
:root[data-theme="dark"] .el-select-dropdown__item:hover {
  background: var(--bg-hover) !important;
}
:root[data-theme="dark"] .el-select-dropdown__item.selected {
  color: var(--accent) !important;
}

/* ── Mobile scroll: hide page headers globally ── */
.app-main.mobile-scrolled .page-header {
  max-height: 0 !important;
  overflow: hidden !important;
  opacity: 0 !important;
  padding: 0 !important;
  margin: 0 !important;
  pointer-events: none !important;
  transition: max-height var(--motion-slow) var(--ease-spring-gentle),
              opacity var(--motion-fast) var(--ease-emphasized),
              padding var(--motion-slow) var(--ease-spring-gentle),
              margin var(--motion-slow) var(--ease-spring-gentle);
}

/* ── Global mobile title size ── */
@media (max-width: 768px) {
  .page-title {
    font-size: 18px !important;
  }
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
  width: 100%;
  position: relative;
  overflow: hidden;
  background: var(--bg-base);
  --mobile-sidebar-x: -260px;
  --mobile-sidebar-progress: 0;
  --mobile-content-scale: 1;
  --mobile-content-shift: 0px;
}

/* Ambient gradient mesh */
.ambient-bg {
  position: fixed;
  inset: -10%;
  pointer-events: none;
  z-index: 0;
  background:
    radial-gradient(ellipse at 75% 15%, rgba(196, 181, 253, 0.25), transparent 55%),
    radial-gradient(ellipse at 15% 85%, rgba(165, 243, 252, 0.2), transparent 55%),
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%);
  animation: meshDrift 40s ease-in-out infinite alternate;
}
.orb { display: none; }
@keyframes meshDrift {
  0%   { transform: translate(0, 0) scale(1); }
  50%  { transform: translate(-15px, 10px) scale(1.02); }
  100% { transform: translate(10px, -8px) scale(0.99); }
}

/* Subtle noise texture */
.bg-grain {
  position: fixed;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  opacity: 0.4;
  background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.04'/%3E%3C/svg%3E");
}

.app-container {
  height: 100vh;
  position: relative;
}

.app-aside {
  transition: width var(--motion-slow) var(--ease-spring-gentle);
  overflow: hidden;
  background: transparent;
}

.app-main {
  background: transparent;
  padding: 32px 40px;
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
  transition: opacity var(--motion-page) var(--ease-emphasized),
              transform var(--motion-page) var(--ease-emphasized);
}
.page-slide-leave-active {
  position: absolute;
  inset: 0;
  pointer-events: none;
  transition: opacity var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-fast) var(--ease-emphasized);
}
.page-slide-enter-from {
  opacity: 0;
  transform: translateY(var(--motion-distance-sm)) scale(0.992);
}
.page-slide-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.996);
}

/* ── Mobile Global Header ── */
.mobile-global-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 1001;
  height: 56px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  transition: background-color var(--motion-normal) var(--ease-emphasized),
              box-shadow var(--motion-normal) var(--ease-emphasized);
}
.mobile-global-header.header-scrolled {
  background: var(--bg-glass-strong);
  backdrop-filter: blur(24px) saturate(180%);
  -webkit-backdrop-filter: blur(24px) saturate(180%);
  box-shadow: 0 1px 8px rgba(0, 0, 0, 0.06);
}

.mobile-menu-btn {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
  color: var(--text-primary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.04);
  transition: transform var(--motion-instant) var(--ease-emphasized),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              box-shadow var(--motion-fast) var(--ease-emphasized);
  flex-shrink: 0;
}
.mobile-global-header.header-scrolled .mobile-menu-btn {
  background: var(--bg-glass-subtle);
  border-color: var(--border-subtle);
  box-shadow: none;
}
.mobile-menu-btn:active {
  transform: scale(0.9);
}

.mobile-page-title {
  flex: 1;
  text-align: center;
  font-size: 17px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mobile-header-spacer {
  width: 36px; /* match menu-btn width so title is visually centered */
  flex-shrink: 0;
}

/* Title fade animation */
.title-fade-enter-active {
  transition: opacity var(--motion-normal) var(--ease-emphasized),
              transform var(--motion-normal) var(--ease-spring-gentle);
}
.title-fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.title-fade-enter-from {
  opacity: 0;
  transform: translateX(-12px);
}
.title-fade-leave-to {
  opacity: 0;
  transform: translateX(-6px);
}

.mobile-overlay {
  position: fixed;
  inset: 0;
  z-index: 999;
  background: rgba(0, 0, 0, 0.3);
  opacity: var(--mobile-sidebar-progress);
  touch-action: none;
}

.scrim-fade-enter-active,
.scrim-fade-leave-active { transition: opacity var(--motion-fast) var(--ease-emphasized); }
.scrim-fade-enter-from,
.scrim-fade-leave-to { opacity: 0; }

/* Mobile aside: fixed overlay sliding from left */
.app-aside {
  transition: width var(--motion-slow) var(--ease-spring-gentle),
              transform var(--motion-normal) var(--ease-spring-gentle),
              box-shadow var(--motion-normal) var(--ease-emphasized);
}
.app-aside.mobile-open {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  z-index: 1000;
  transform: translateX(0);
  box-shadow: 4px 0 24px rgba(0, 0, 0, 0.1);
}

.app-main.mobile-full {
  padding: 16px 12px;
  padding-top: 60px; /* space for global header (56px + 4px gap) */
  padding-bottom: 40px; /* extra space at bottom so last content is fully visible */
  overflow-x: hidden;
  width: 100%;
  max-width: 100%;
  transform-origin: right center;
  transform: translateX(var(--mobile-content-shift)) scale(var(--mobile-content-scale));
  transition: transform var(--motion-normal) var(--ease-spring-gentle);
}

@media (max-width: 768px) {
  .app-aside {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 1000;
    transform: translate3d(var(--mobile-sidebar-x), 0, 0);
    will-change: transform;
    touch-action: pan-y;
  }

  .app-shell { touch-action: pan-y; }

  .app-aside.mobile-open { transform: translate3d(var(--mobile-sidebar-x), 0, 0); }

  .sidebar-dragging .app-aside,
  .sidebar-dragging .app-main.mobile-full,
  .sidebar-dragging .mobile-overlay {
    transition: none !important;
  }
}
</style>
