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
import { computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import {
  House,
  Notebook,
  FolderOpened,
  Calendar,
  MagicStick,
  DataLine,
  Document,
  Connection,
  Reading,
  Files,
  Expand,
  Fold,
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)

// Auto-close the mobile sidebar overlay on navigation. The match is checked at
// navigation time (not cached) so resizing between routes stays correct.
watch(() => route.path, () => {
  if (window.matchMedia('(max-width: 768px)').matches && !isCollapsed.value) {
    appStore.toggleSidebar()
  }
})

const navItems = [
  { path: '/', label: '首页', icon: House },
  { path: '/memory', label: '知识库', icon: Notebook },
  { path: '/wiki-dashboard', label: 'Wiki 看板', icon: DataLine },
  { path: '/wiki', label: 'Wiki 工作台', icon: Document },
  { path: '/explore', label: '知识探索', icon: MagicStick },
  { path: '/ingest', label: '外部摄入', icon: Connection },
  { path: '/manual', label: '使用手册', icon: Reading },
  { path: '/reader', label: '阅境轩', icon: Files },
  { path: '/code-repo', label: '代码仓', icon: FolderOpened },
  { path: '/timeline', label: '时光机', icon: Calendar },
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
  background: var(--bg-glass);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  border-right: 1px solid var(--border-glass);
  box-shadow: inset -1px 0 0 var(--border-faint);
  padding: 0 12px;
  transition: padding var(--duration-slow) var(--ease-spring);
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
  transition: padding var(--duration-slow) var(--ease-spring),
              justify-content var(--duration-slow) var(--ease-spring);
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
  color: var(--text-primary);
  flex-shrink: 0;
}

.logo-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: var(--tracking-tight);
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
  color: var(--text-muted);
  font-size: 14px;
  font-weight: 450;
  cursor: pointer;
  transition: all var(--duration-slow) var(--ease-spring);
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 10px 0;
  gap: 0;
}

.nav-item:hover {
  background: var(--bg-glass);
  color: var(--text-secondary);
}

.nav-item.active {
  background: var(--bg-glass);
  color: var(--text-primary);
  font-weight: 500;
  box-shadow: var(--shadow-sm);
}

.nav-icon {
  flex-shrink: 0;
  opacity: 0.6;
  transition: opacity var(--duration-fast) var(--ease-out), transform var(--duration-slow) var(--ease-spring);
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
  border-top: 1px solid var(--border-faint);
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
  color: var(--text-faint);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--duration-slow) var(--ease-spring);
}

.sidebar.collapsed .collapse-btn {
  justify-content: center;
  padding: 8px 0;
  gap: 0;
}

.collapse-btn:hover {
  background: var(--bg-glass);
  color: var(--text-tertiary);
}

/* ── Transitions ── */
.logo-text-enter-active {
  transition: opacity 0.25s var(--ease-out) 0.15s, transform var(--duration-normal) var(--ease-spring) 0.15s;
}
.logo-text-leave-active {
  transition: opacity 0.12s var(--ease-out), transform 0.12s var(--ease-out);
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
  transition: opacity var(--duration-fast) var(--ease-out) 0.1s, transform 0.25s var(--ease-spring) 0.1s;
}
.nav-label-leave-active {
  transition: opacity 0.1s var(--ease-out), transform 0.1s var(--ease-out);
}
.nav-label-enter-from {
  opacity: 0;
  transform: translateX(-6px);
}
.nav-label-leave-to {
  opacity: 0;
  transform: translateX(-3px);
}

/* ── Mobile ── */
@media (max-width: 768px) {
  .sidebar {
    padding: 0 12px;
    border-right: none;
    box-shadow: none;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(32px) saturate(200%);
    -webkit-backdrop-filter: blur(32px) saturate(200%);
  }
  .sidebar-footer {
    display: none;
  }
  .nav-item {
    padding: 12px 14px;
    font-size: 15px;
  }
}
</style>
