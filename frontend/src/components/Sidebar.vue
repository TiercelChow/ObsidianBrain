<template>
  <div class="sidebar" :class="{ collapsed: isCollapsed }">
    <span v-if="expandedOnMobile" class="mobile-sheet-handle" aria-hidden="true"></span>
    <!-- Logo -->
    <div class="logo-section">
      <img class="logo-mark" src="/favicon.svg" alt="" aria-hidden="true" />
      <transition name="logo-text">
        <span v-show="!isCollapsed" class="logo-name">ObsidianBrain</span>
      </transition>
      <strong v-if="expandedOnMobile" class="mobile-sheet-title">全部功能</strong>
      <button
        v-if="expandedOnMobile"
        type="button"
        class="mobile-sidebar-close"
        aria-label="关闭全部模块"
        @click="appStore.setSidebarCollapsed(true)"
      >×</button>
    </div>

    <!-- Navigation -->
    <nav class="nav-list">
      <span
        v-if="activeIndicator.visible"
        class="nav-active-indicator"
        aria-hidden="true"
        :style="activeIndicatorStyle"
      ></span>
      <div v-for="group in navGroups" :key="group.label" class="nav-group">
        <span v-show="!isCollapsed" class="nav-group-label">{{ group.label }}</span>
        <router-link
          v-for="item in group.items"
          :key="item.path"
          :to="item.path"
          class="nav-item"
          :class="{ active: isActive(item.path) }"
          :aria-current="isActive(item.path) ? 'page' : undefined"
          :data-nav-path="item.path"
        >
          <el-icon :size="18" class="nav-icon"><component :is="item.icon" /></el-icon>
          <transition name="nav-label">
            <span v-show="!isCollapsed" class="nav-label">{{ item.label }}</span>
          </transition>
        </router-link>
      </div>
    </nav>

    <!-- Collapse Toggle -->
    <div class="sidebar-footer">
      <button class="collapse-btn" type="button" :aria-label="isCollapsed ? '展开导航' : '收起导航'" @click="appStore.toggleSidebar()">
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
import { computed, nextTick, onMounted, onUnmounted, reactive, watch } from 'vue'
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
  ChatDotRound,
  Files,
  Finished,
  Operation,
  Setting,
  Expand,
  Fold,
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const props = defineProps<{ expandedOnMobile?: boolean }>()
const isCollapsed = computed(() => appStore.sidebarCollapsed && !props.expandedOnMobile)
const activeIndicator = reactive({ top: 0, height: 0, visible: false })
const activeIndicatorStyle = computed(() => ({
  height: `${activeIndicator.height}px`,
  transform: `translate3d(0, ${activeIndicator.top}px, 0)`,
}))

function updateActiveIndicator() {
  nextTick(() => {
    const active = document.querySelector('.nav-list .nav-item.active') as HTMLElement | null
    const list = active?.closest<HTMLElement>('.nav-list')
    if (!active || !list) {
      activeIndicator.visible = false
      return
    }
    const listRect = list.getBoundingClientRect()
    const activeRect = active.getBoundingClientRect()
    activeIndicator.top = activeRect.top - listRect.top + list.scrollTop
    activeIndicator.height = active.offsetHeight
    activeIndicator.visible = true
  })
}

// Auto-close the mobile sidebar overlay on navigation. The match is checked at
// navigation time (not cached) so resizing between routes stays correct.
watch(() => route.path, () => {
  if (window.matchMedia('(max-width: 768px)').matches && !isCollapsed.value) {
    appStore.setSidebarCollapsed(true)
  }
  updateActiveIndicator()
})
watch(isCollapsed, updateActiveIndicator)
onMounted(() => {
  updateActiveIndicator()
  window.addEventListener('resize', updateActiveIndicator)
})
onUnmounted(() => window.removeEventListener('resize', updateActiveIndicator))

const navGroups = [
  {
    label: '日常',
    items: [
      { path: '/', label: '首页', icon: House },
      { path: '/reader', label: '阅境轩', icon: Files },
      { path: '/timeline', label: '时光机', icon: Calendar },
      { path: '/tasks', label: '任务中枢', icon: Finished },
    ],
  },
  {
    label: '知识',
    items: [
      { path: '/knowledge', label: '书籍知识库', icon: Notebook },
      { path: '/knowledge/wiki', label: 'Wiki 工作台', icon: Document },
      { path: '/knowledge/chat', label: '知识问答', icon: ChatDotRound },
      { path: '/knowledge/tasks', label: '研究任务', icon: Operation },
      { path: '/knowledge/settings', label: 'Wiki 配置', icon: Setting },
    ],
  },
  {
    label: '管理',
    items: [
      { path: '/code-repo', label: '代码仓', icon: FolderOpened },
      { path: '/inspiration', label: '灵感熔炉', icon: MagicStick },
      { path: '/radar', label: '智识雷达', icon: DataLine },
    ],
  },
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
  display: block;
  flex-shrink: 0;
  border-radius: 9px;
  object-fit: cover;
  box-shadow: 0 5px 14px color-mix(in srgb, var(--accent) 22%, transparent);
}

.logo-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: var(--tracking-tight);
  white-space: nowrap;
}

.mobile-sidebar-close { display: none; }
.mobile-sheet-title, .mobile-sheet-handle { display: none; }

/* ── Navigation ── */
.nav-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  position: relative;
}

.nav-group { display: flex; flex-direction: column; gap: 2px; }
.nav-group + .nav-group { margin-top: 8px; }
.nav-group-label {
  padding: 6px 12px 4px;
  color: var(--text-faint);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.nav-active-indicator {
  position: absolute;
  inset-inline: 0;
  top: 0;
  z-index: 0;
  border-radius: 12px;
  background: var(--bg-glass);
  border: 1px solid var(--border-subtle);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  pointer-events: none;
  transition: transform var(--motion-slow) var(--ease-spring-gentle),
              height var(--motion-normal) var(--ease-spring-gentle),
              opacity var(--motion-fast) var(--ease-emphasized);
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
  position: relative;
  z-index: 1;
  transition: padding var(--motion-slow) var(--ease-spring-gentle),
              gap var(--motion-slow) var(--ease-spring-gentle),
              color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized);
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
  background: transparent;
  color: var(--text-primary);
  font-weight: 500;
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
  transition: padding var(--motion-slow) var(--ease-spring-gentle),
              gap var(--motion-slow) var(--ease-spring-gentle),
              color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized);
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
    position: relative;
    padding-top: 28px;
    padding-left: 12px;
    padding-right: 12px;
    border-right: none;
    border-radius: inherit;
    box-shadow: none;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(32px) saturate(200%);
    -webkit-backdrop-filter: blur(32px) saturate(200%);
  }
  .mobile-sheet-handle {
    position: absolute;
    top: 8px;
    left: 50%;
    width: 38px;
    height: 5px;
    display: block;
    border-radius: 999px;
    background: var(--text-faint);
    opacity: .45;
    transform: translateX(-50%);
  }
  .logo-section { height: 54px; margin-bottom: 2px; padding-inline: 4px; }
  .logo-mark, .logo-name { display: none !important; }
  .mobile-sheet-title { display: block; color: var(--text-primary); font-size: 19px; font-weight: 700; letter-spacing: -.02em; }
  .mobile-sidebar-close {
    display: inline-flex;
    width: var(--tap-target);
    height: var(--tap-target);
    margin-left: auto;
    border: 1px solid var(--border-subtle);
    border-radius: 14px;
    background: var(--bg-glass-subtle);
    color: var(--text-muted);
    align-items: center;
    justify-content: center;
    font: inherit;
    font-size: 24px;
    line-height: 1;
  }
  .mobile-sidebar-close:active { transform: scale(0.94); }
  .sidebar-footer {
    display: none;
  }
  .nav-list { gap: 12px; padding-bottom: max(18px, var(--safe-bottom)); }
  .nav-active-indicator { display: none; }
  .nav-group {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  .nav-group-label { grid-column: 1 / -1; padding: 2px 4px 0; }
  .nav-item {
    min-width: 0;
    min-height: 70px;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
    padding: 8px 4px;
    border: 1px solid var(--border-subtle);
    border-radius: 16px;
    background: color-mix(in srgb, var(--bg-glass) 72%, transparent);
    font-size: 12px;
    font-weight: 600;
    text-align: center;
  }
  .nav-item.active { background: color-mix(in srgb, var(--accent) 11%, var(--bg-glass)); border-color: var(--accent-border); color: var(--accent); }
  .nav-icon { font-size: 22px; opacity: .85; }
  .nav-label { width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nav-group + .nav-group { margin-top: 0; }
  .nav-group-label { font-size: 11px; }
}
</style>
