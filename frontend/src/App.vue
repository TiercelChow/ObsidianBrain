<template>
  <div class="app-shell">
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

/* ── Element Plus overrides ── */
.el-card {
  border-radius: 20px !important;
  border: 1px solid rgba(0, 0, 0, 0.04) !important;
  background: #fff !important;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04),
              0 1px 2px rgba(0, 0, 0, 0.02) !important;
  transition: box-shadow 0.25s ease,
              transform 0.25s ease !important;
}
.el-card:hover {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.06),
              0 1px 3px rgba(0, 0, 0, 0.04) !important;
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
  z-index: 1;
}

.app-aside {
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  background: transparent;
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
