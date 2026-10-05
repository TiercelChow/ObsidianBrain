<template>
  <div class="sidebar" :class="{ collapsed: isCollapsed }">
    <!-- Logo -->
    <div class="logo-section">
      <img class="logo-mark" src="/favicon.svg" alt="" aria-hidden="true" />
      <span class="logo-name" :aria-hidden="isCollapsed ? true : undefined">ObsidianBrain</span>
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
      <button class="collapse-btn theme-toggle" data-glass-action type="button" :aria-label="`切换主题，当前${themeName}`" :title="`切换主题，当前${themeName}`" @click="appStore.toggleTheme()">
        <el-icon :size="18"><component :is="themeIcon" /></el-icon>
        <span class="nav-label" :aria-hidden="isCollapsed ? true : undefined">外观 · {{ themeName }}</span>
      </button>
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
  Document,
  ChatDotRound,
  Files,
  Finished,
  Operation,
  Setting,
  Expand,
  Fold,
  Sunny,
  Moon,
  View,
} from '@element-plus/icons-vue'

const route = useRoute()
const appStore = useAppStore()
const isCollapsed = computed(() => appStore.sidebarCollapsed)
const themeName = computed(() => ({ light: '浅色', dark: '深色', 'eye-care': '护眼' }[appStore.theme]))
const themeIcon = computed(() => ({ light: Sunny, dark: Moon, 'eye-care': View }[appStore.theme]))
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

watch(() => route.path, () => {
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
      { path: '/code-repo', label: '代码仓', icon: FolderOpened },
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
  font-size: 11px;
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
  font-weight: 500;
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
  font-weight: 650;
}

.nav-icon {
  justify-self: center;
  flex-shrink: 0;
  opacity: 0.85;
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

@media (prefers-reduced-motion: reduce) {
  .logo-name, .nav-label, .nav-group-label { transition-delay: 0s; }
}
</style>
