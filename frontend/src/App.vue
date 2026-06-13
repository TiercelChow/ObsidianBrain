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

    <el-container class="app-container">
      <el-aside :width="isCollapsed ? '72px' : '230px'" class="app-aside">
        <Sidebar />
      </el-aside>
      <el-main class="app-main">
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
import { computed } from 'vue'
import { useAppStore } from './stores/app'
import Sidebar from './components/Sidebar.vue'

const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)
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

/* Ambient gradient orbs */
.ambient-bg {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 0;
  background:
    radial-gradient(ellipse 50% 50% at 80% 20%, rgba(196, 181, 253, 0.3), transparent),
    radial-gradient(ellipse 40% 40% at 20% 80%, rgba(165, 243, 252, 0.25), transparent),
    radial-gradient(ellipse 35% 35% at 50% 50%, rgba(253, 230, 138, 0.15), transparent);
  animation: meshShift 30s ease-in-out infinite alternate;
}
.orb {
  display: none;
}
@keyframes meshShift {
  0% {
    background:
      radial-gradient(ellipse 50% 50% at 80% 20%, rgba(196, 181, 253, 0.3), transparent),
      radial-gradient(ellipse 40% 40% at 20% 80%, rgba(165, 243, 252, 0.25), transparent),
      radial-gradient(ellipse 35% 35% at 50% 50%, rgba(253, 230, 138, 0.15), transparent);
  }
  33% {
    background:
      radial-gradient(ellipse 45% 45% at 70% 30%, rgba(196, 181, 253, 0.25), transparent),
      radial-gradient(ellipse 45% 45% at 30% 70%, rgba(165, 243, 252, 0.3), transparent),
      radial-gradient(ellipse 30% 30% at 60% 45%, rgba(253, 230, 138, 0.2), transparent);
  }
  66% {
    background:
      radial-gradient(ellipse 55% 55% at 75% 25%, rgba(196, 181, 253, 0.35), transparent),
      radial-gradient(ellipse 35% 35% at 25% 75%, rgba(165, 243, 252, 0.2), transparent),
      radial-gradient(ellipse 40% 40% at 45% 55%, rgba(253, 230, 138, 0.25), transparent);
  }
  100% {
    background:
      radial-gradient(ellipse 50% 50% at 85% 15%, rgba(196, 181, 253, 0.28), transparent),
      radial-gradient(ellipse 42% 42% at 18% 82%, rgba(165, 243, 252, 0.28), transparent),
      radial-gradient(ellipse 38% 38% at 55% 48%, rgba(253, 230, 138, 0.18), transparent);
  }
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
</style>
