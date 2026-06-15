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

    <!-- Mobile hamburger button -->
    <button class="mobile-menu-btn" @click="appStore.toggleSidebar()" v-if="isMobile">
      <el-icon :size="20"><component :is="isCollapsed ? Expand : Fold" /></el-icon>
    </button>

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
      <el-main class="app-main" :class="{ 'mobile-full': isMobile }">
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
import { useAppStore } from './stores/app'
import { Expand, Fold } from '@element-plus/icons-vue'
import Sidebar from './components/Sidebar.vue'

const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)

// Mobile detection
const windowWidth = ref(window.innerWidth)
const isMobile = computed(() => windowWidth.value <= 768)

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

/* ── Glass Card 全局覆盖 ── */
.el-card,
.stat-card, .module-card, .status-card, .tool-card,
.stat-chip, .result-card, .recent-card,
.repo-card, .mode-option, .radar-card,
.combo-result, .question-result, .counterpoint-result,
.combo-result .concept-chip,
.question-result .question-item,
.counterpoint-result .counterpoint-item,
.assessment {
  background: rgba(255, 255, 255, 0.55) !important;
  backdrop-filter: blur(12px) saturate(180%) !important;
  -webkit-backdrop-filter: blur(12px) saturate(180%) !important;
  border: 1px solid rgba(255, 255, 255, 0.6) !important;
  box-shadow:
    0 1px 2px rgba(0, 0, 0, 0.03),
    0 4px 16px rgba(0, 0, 0, 0.04),
    inset 0 1px 0 rgba(255, 255, 255, 0.5) !important;
}

/* ── Glass Tag 全局覆盖 ── */
.lang-tag, .tag, .stat-chip .stat-label {
  background: rgba(255, 255, 255, 0.45) !important;
  backdrop-filter: blur(8px) !important;
  -webkit-backdrop-filter: blur(8px) !important;
  border: 1px solid rgba(255, 255, 255, 0.5) !important;
}

/* ── Element Plus overrides ── */
.el-card {
  border-radius: 20px !important;
  transition: box-shadow 0.25s ease, transform 0.25s ease !important;
}
.el-card:hover {
  box-shadow:
    0 4px 16px rgba(0, 0, 0, 0.06),
    inset 0 1px 0 rgba(255, 255, 255, 0.5) !important;
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
  color: #a1a1aa;
}
</style>

<style scoped>
.app-shell {
  height: 100vh;
  width: 100vw;
  position: relative;
  overflow: hidden;
  background: #fafafa;
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
    radial-gradient(ellipse at 50% 50%, rgba(253, 230, 138, 0.12), transparent 50%),
    #fafafa;
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

/* ── Mobile ── */
.mobile-menu-btn {
  position: fixed;
  top: 12px;
  left: 12px;
  z-index: 1001;
  width: 40px;
  height: 40px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.5);
  background: rgba(255, 255, 255, 0.6);
  backdrop-filter: blur(16px) saturate(180%);
  -webkit-backdrop-filter: blur(16px) saturate(180%);
  color: #18181b;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  transition: all 0.2s ease;
}
.mobile-menu-btn:active {
  transform: scale(0.92);
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
  padding-top: 60px; /* space for hamburger button */
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
