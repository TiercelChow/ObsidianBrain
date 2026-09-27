<template>
  <div class="sidebar" :class="{ collapsed: isCollapsed }">
    <span class="mobile-sheet-handle" aria-hidden="true"></span>
    <!-- Logo -->
    <div class="logo-section">
      <img class="logo-mark" src="/favicon.svg" alt="" aria-hidden="true" />
      <span class="logo-name" :aria-hidden="isCollapsed ? true : undefined">ObsidianBrain</span>
      <strong class="mobile-sheet-title">全部功能</strong>
      <button
        type="button"
        class="mobile-sidebar-close"
        aria-label="关闭全部模块"
        @click="appStore.setSidebarCollapsed(true)"
      >×</button>
    </div>

    <!-- Navigation -->
    <nav id="app-navigation" ref="navListRef" class="nav-list" aria-label="主导航">
      <span
        v-if="activeIndicator.visible"
        class="nav-active-indicator"
        aria-hidden="true"
        :style="activeIndicatorStyle"
      ></span>
      <div v-for="group in navGroups" :key="group.label" class="nav-group">
        <span class="nav-group-label" :aria-hidden="isCollapsed ? true : undefined">{{ group.label }}</span>
        <router-link
          v-for="item in group.items"
          :key="item.path"
          :to="item.path"
          class="nav-item"
          :class="{ active: isActive(item.path) }"
          :aria-current="isActive(item.path) ? 'page' : undefined"
          :data-nav-path="item.path"
          :aria-label="item.label"
          :title="isCollapsed ? item.label : undefined"
        >
          <el-icon :size="18" class="nav-icon"><component :is="item.icon" /></el-icon>
          <span class="nav-label" :aria-hidden="isCollapsed ? true : undefined">{{ item.label }}</span>
        </router-link>
      </div>
    </nav>

    <!-- Collapse Toggle -->
    <div class="sidebar-footer">
      <button class="collapse-btn" type="button" :aria-label="isCollapsed ? '展开导航' : '收起导航'" :aria-expanded="!isCollapsed" aria-controls="app-navigation" @click="appStore.toggleSidebar()">
        <el-icon :size="16">
          <component :is="isCollapsed ? Expand : Fold" />
        </el-icon>
        <span class="nav-label" :aria-hidden="isCollapsed ? true : undefined">收起</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
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
const navListRef = ref<HTMLElement | null>(null)
const activeIndicator = reactive({ top: 0, height: 0, visible: false })
const activeIndicatorStyle = computed(() => ({
  height: `${activeIndicator.height}px`,
  transform: `translate3d(0, ${activeIndicator.top}px, 0)`,
}))

function updateActiveIndicator() {
  nextTick(() => {
    const list = navListRef.value
    const active = list?.querySelector<HTMLElement>('.nav-item.active')
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
  /* 72px rail - 24px padding - 1px outer edge, identical in both states. */
  --sidebar-icon-track: 47px;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-glass);
  backdrop-filter: var(--glass-structural-filter);
  -webkit-backdrop-filter: var(--glass-structural-filter);
  border-right: 1px solid var(--border-glass);
  box-shadow: inset -1px 0 0 var(--border-faint);
  padding: 0 12px;
}

/* ── Logo ── */
.logo-section {
  height: 56px;
  display: grid;
  grid-template-columns: var(--sidebar-icon-track) minmax(0, 1fr);
  align-items: center;
  padding: 0;
  margin-bottom: 8px;
  flex-shrink: 0;
}

.logo-mark {
  width: 32px;
  height: 32px;
  display: block;
  justify-self: center;
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
  overflow-x: hidden;
  position: relative;
}

.nav-group { display: flex; flex-direction: column; gap: 2px; }
.nav-group + .nav-group { margin-top: 8px; }
.nav-group-label {
  height: 28px;
  flex: none;
  display: flex;
  align-items: center;
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
  display: grid;
  grid-template-columns: var(--sidebar-icon-track) minmax(0, 1fr);
  align-items: center;
  min-height: 40px;
  padding: 8px 0;
  border-radius: 12px;
  text-decoration: none;
  color: var(--text-muted);
  font-size: 14px;
  font-weight: 450;
  cursor: pointer;
  position: relative;
  z-index: 1;
  transform-origin: calc(var(--sidebar-icon-track) / 2) 50%;
  transition: color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized);
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
  justify-self: center;
  flex-shrink: 0;
  opacity: 0.6;
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}

.nav-item:hover .nav-icon,
.nav-item.active .nav-icon {
  opacity: 1;
}

.nav-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-inline-end: 8px;
  white-space: nowrap;
}

/* ── Footer ── */
.sidebar-footer {
  border-top: 1px solid var(--border-faint);
  padding: 12px 0;
  flex-shrink: 0;
}

.collapse-btn {
  display: grid;
  grid-template-columns: var(--sidebar-icon-track) minmax(0, 1fr);
  align-items: center;
  min-height: 44px;
  width: 100%;
  padding: 8px 0;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: var(--text-faint);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transform-origin: calc(var(--sidebar-icon-track) / 2) 50%;
  transition: color var(--motion-fast) var(--ease-emphasized),
              background-color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized);
}

.collapse-btn > .el-icon { justify-self: center; }

.collapse-btn:hover {
  background: var(--bg-glass);
  color: var(--text-tertiary);
}

/* Keep grid participants mounted: label fading never re-centers the icons. */
@media (min-width: 769px) {
  .logo-name, .nav-label, .nav-group-label {
    transition: opacity var(--motion-fast) var(--ease-emphasized),
                visibility 0s;
  }
  .sidebar.collapsed .logo-name,
  .sidebar.collapsed .nav-label,
  .sidebar.collapsed .nav-group-label {
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: opacity var(--motion-instant) var(--ease-emphasized),
                visibility 0s var(--motion-instant);
  }
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
    backdrop-filter: var(--glass-structural-filter);
    -webkit-backdrop-filter: var(--glass-structural-filter);
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
  .logo-section { height: 54px; display: flex; margin-bottom: 2px; padding-inline: 4px; }
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
    display: flex;
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
    transform-origin: center;
  }
  .nav-item.active { background: color-mix(in srgb, var(--accent) 11%, var(--bg-glass)); border-color: var(--accent-border); color: var(--accent); }
  .nav-icon { font-size: 22px; opacity: .85; }
  .nav-label { width: 100%; padding: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nav-group + .nav-group { margin-top: 0; }
  .nav-group-label { font-size: 11px; }
}
@media (prefers-reduced-motion: reduce) {
  .logo-name, .nav-label, .nav-group-label { transition-delay: 0s; }
}
</style>
