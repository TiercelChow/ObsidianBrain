<template>
  <div class="app-shell">
    <!-- Ambient gradient orbs for liquid glass effect -->
    <div class="ambient-bg">
      <div class="orb orb-1"></div>
      <div class="orb orb-2"></div>
      <div class="orb orb-3"></div>
    </div>
    <!-- Subtle grain texture background -->
    <div class="bg-grain"></div>

    <!-- Mobile global header: hamburger + page title -->
    <div class="mobile-global-header" v-if="isMobile" :class="{ 'header-scrolled': isScrolled }">
      <button class="mobile-menu-btn" @click="appStore.toggleSidebar()">
        <el-icon :size="20"><component :is="isCollapsed ? Expand : Fold" /></el-icon>
      </button>
      <transition name="title-fade">
        <span v-if="isScrolled" class="mobile-page-title">{{ currentTitle }}</span>
      </transition>
    </div>

    <!-- Mobile sidebar overlay backdrop -->
    <div
      v-if="isMobile && !isCollapsed"
      class="mobile-overlay"
      @click="appStore.toggleSidebar()"
    ></div>

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
        <router-view v-slot="{ Component, route }">
          <transition name="page-slide" mode="out-in">
            <component :is="Component" :key="route.path" />
          </transition>
        </router-view>
      </el-main>
    </el-container>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import { Expand, Fold } from '@element-plus/icons-vue'
import Sidebar from './components/Sidebar.vue'

const route = useRoute()
const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)
const currentTitle = computed(() => (route.meta?.title as string) || '')

// Mobile detection
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => windowWidth.value <= 768)

// Scroll detection for mobile header
const isScrolled = ref(false)
function onMainScroll(e: Event) {
  const el = e.target as HTMLElement
  isScrolled.value = el.scrollTop > 20
}

function onResize() { windowWidth.value = window.innerWidth }
onMounted(() => {
  window.addEventListener('resize', onResize)
  if (isMobile.value && !isCollapsed.value) appStore.toggleSidebar()
})
onUnmounted(() => window.removeEventListener('resize', onResize))
</script>

<style>
/* ── Global Reset ── */
*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--bg-base);
  color: var(--text-primary);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC',
    'Hiragino Sans GB', 'Microsoft YaHei', sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

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
  --bg-base: #fafafa;
  --bg-glass: rgba(255, 255, 255, 0.55);
  --bg-glass-strong: rgba(255, 255, 255, 0.7);
  --bg-glass-subtle: rgba(255, 255, 255, 0.45);
  --bg-hover: rgba(255, 255, 255, 0.5);
  --border-glass: rgba(255, 255, 255, 0.6);
  --border-subtle: rgba(255, 255, 255, 0.5);
  --border-faint: rgba(0, 0, 0, 0.04);

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

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.03);
  --shadow-md: 0 1px 2px rgba(0, 0, 0, 0.03), 0 4px 16px rgba(0, 0, 0, 0.04);
  --shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.06);
  --inset-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.5);

  --orb-opacity: 1;
}

/* ── Dark Theme — Deep Black ── */
:root[data-theme="dark"] {
  --bg-base: #000000;
  --bg-glass: rgba(15, 15, 15, 0.7);
  --bg-glass-strong: rgba(20, 20, 20, 0.9);
  --bg-glass-subtle: rgba(28, 28, 28, 0.5);
  --bg-hover: rgba(35, 35, 35, 0.6);
  --border-glass: rgba(55, 55, 55, 0.6);
  --border-subtle: rgba(45, 45, 45, 0.5);
  --border-faint: rgba(255, 255, 255, 0.06);

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

  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.4);
  --shadow-md: 0 1px 3px rgba(0, 0, 0, 0.3), 0 4px 16px rgba(0, 0, 0, 0.2);
  --shadow-lg: 0 8px 32px rgba(0, 0, 0, 0.3);
  --inset-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.03);

  --orb-opacity: 0.3;
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
  backdrop-filter: blur(12px) saturate(180%) !important;
  -webkit-backdrop-filter: blur(12px) saturate(180%) !important;
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
  transition: box-shadow 0.25s ease, transform 0.25s ease !important;
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
  transition: all 0.35s cubic-bezier(0.4, 0, 0.2, 1);
}

/* ── Global mobile title size ── */
@media (max-width: 768px) {
  .page-title {
    font-size: 18px !important;
  }
}
</style>

<style scoped>
.app-shell {
  height: 100vh;
  width: 100vw;
  position: relative;
  overflow: hidden;
  background: var(--bg-base);
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
  transition: width 0.45s cubic-bezier(0.34, 1.56, 0.64, 1);
  overflow: hidden;
  background: transparent;
  will-change: width;
}

.app-main {
  background: transparent;
  padding: 32px 40px;
  overflow-y: auto;
  overflow-x: hidden;
}

/* ── Page Transition ── */
.page-slide-enter-active {
  transition: opacity 0.3s ease, transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
.page-slide-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.page-slide-enter-from {
  opacity: 0;
  transform: translateY(12px);
}
.page-slide-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

/* ── Mobile Global Header ── */
.mobile-global-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 1001;
  height: 52px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  transition: background 0.3s ease, box-shadow 0.3s ease, backdrop-filter 0.3s ease;
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
  transition: all 0.2s ease;
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
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.2px;
  white-space: nowrap;
}

/* Title fade animation */
.title-fade-enter-active {
  transition: opacity 0.35s ease, transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
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
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  animation: fadeIn 0.2s ease;
}
@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* Mobile aside: fixed overlay sliding from left */
.app-aside {
  transition: width 0.45s cubic-bezier(0.34, 1.56, 0.64, 1),
              transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
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
  padding-top: 56px; /* space for global header (52px + 4px gap) */
  padding-bottom: 40px; /* extra space at bottom so last content is fully visible */
  overflow-x: hidden;
  width: 100%;
  max-width: 100vw;
}

@media (max-width: 768px) {
  .app-aside:not(.mobile-open) {
    position: fixed;
    left: -280px;
    z-index: 1000;
  }
}
</style>
