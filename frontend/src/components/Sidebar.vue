<template>
  <div class="sidebar" :class="{ collapsed: isCollapsed }">
    <!-- Logo -->
    <div class="logo-section">
      <div class="logo-mark">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
          <path d="M12 2L2 7L12 12L22 7L12 2Z" fill="currentColor" opacity="0.9"/>
          <path d="M2 17L12 22L22 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" opacity="0.5"/>
          <path d="M2 12L12 17L22 12" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/>
        </svg>
      </div>
      <transition name="logo-text">
        <span v-show="!isCollapsed" class="logo-name">ObsidianBrain</span>
      </transition>
    </div>

    <!-- Navigation -->
    <nav class="nav-list">
      <router-link
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="nav-item"
        :class="{ active: isActive(item.path) }"
      >
        <el-icon :size="18" class="nav-icon"><component :is="item.icon" /></el-icon>
        <transition name="nav-label">
          <span v-show="!isCollapsed" class="nav-label">{{ item.label }}</span>
        </transition>
      </router-link>
    </nav>

    <!-- Collapse Toggle -->
    <div class="sidebar-footer">
      <button class="collapse-btn" @click="appStore.toggleSidebar()">
        <el-icon :size="16">
          <component :is="isCollapsed ? Expand : Fold" />
        </el-icon>
        <transition name="nav-label">
          <span v-show="!isCollapsed">收起</span>
        </transition>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import {
  House,
  Notebook,
  FolderOpened,
  Calendar,
  MagicStick,
  DataLine,
  Expand,
  Fold,
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)

const navItems = [
  { path: '/', label: '首页', icon: House },
  { path: '/memory', label: '记忆管理', icon: Notebook },
  { path: '/code-repo', label: '代码仓', icon: FolderOpened },
  { path: '/timeline', label: '时光机', icon: Calendar },
  { path: '/inspiration', label: '灵感熔炉', icon: MagicStick },
  { path: '/radar', label: '智识雷达', icon: DataLine },
]

function isActive(path: string) {
  return route.path === path
}
</script>

<style scoped>
.sidebar {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.5);
  backdrop-filter: blur(24px) saturate(180%);
  -webkit-backdrop-filter: blur(24px) saturate(180%);
  border-right: 1px solid rgba(255, 255, 255, 0.6);
  box-shadow: inset -1px 0 0 rgba(255, 255, 255, 0.3);
  padding: 0 12px;
  transition: padding 0.45s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.sidebar.collapsed {
  padding: 0 8px;
}

/* ── Logo ── */
.logo-section {
  height: 56px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px;
  margin-bottom: 8px;
  flex-shrink: 0;
  transition: padding 0.45s cubic-bezier(0.34, 1.56, 0.64, 1),
              justify-content 0.45s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.sidebar.collapsed .logo-section {
  padding: 0;
  justify-content: center;
}

.logo-mark {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #18181b;
  flex-shrink: 0;
}

.logo-name {
  font-size: 15px;
  font-weight: 600;
  color: #18181b;
  letter-spacing: -0.3px;
  white-space: nowrap;
}

/* ── Navigation ── */
.nav-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 12px;
  text-decoration: none;
  color: #71717a;
  font-size: 14px;
  font-weight: 450;
  cursor: pointer;
  transition: all 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 10px 0;
  gap: 0;
}

.nav-item:hover {
  background: rgba(255, 255, 255, 0.5);
  color: #27272a;
}

.nav-item.active {
  background: rgba(255, 255, 255, 0.6);
  color: #18181b;
  font-weight: 500;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
}

.nav-icon {
  flex-shrink: 0;
  opacity: 0.6;
  transition: opacity 0.2s ease, transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.sidebar.collapsed .nav-icon {
  transform: scale(1.1);
}

.nav-item:hover .nav-icon,
.nav-item.active .nav-icon {
  opacity: 1;
}

.nav-label {
  white-space: nowrap;
}

/* ── Footer ── */
.sidebar-footer {
  border-top: 1px solid rgba(255, 255, 255, 0.4);
  padding: 12px 0;
  flex-shrink: 0;
}

.collapse-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 12px;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: #a1a1aa;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.sidebar.collapsed .collapse-btn {
  justify-content: center;
  padding: 8px 0;
  gap: 0;
}

.collapse-btn:hover {
  background: rgba(255, 255, 255, 0.5);
  color: #52525b;
}

/* ── Transitions ── */
.logo-text-enter-active {
  transition: opacity 0.25s ease 0.15s, transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1) 0.15s;
}
.logo-text-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.logo-text-enter-from {
  opacity: 0;
  transform: translateX(-8px);
}
.logo-text-leave-to {
  opacity: 0;
  transform: translateX(-4px);
}

.nav-label-enter-active {
  transition: opacity 0.2s ease 0.1s, transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1) 0.1s;
}
.nav-label-leave-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}
.nav-label-enter-from {
  opacity: 0;
  transform: translateX(-6px);
}
.nav-label-leave-to {
  opacity: 0;
  transform: translateX(-3px);
}
</style>
