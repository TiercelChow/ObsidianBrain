import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { applyPin, computeScrollState } from '@/utils/toolbarCollapsePolicy'

export interface HealthStatus {
  status: string
  version: string
  components: Record<string, string>
}

export const useAppStore = defineStore('app', () => {
  const sidebarCollapsed = ref(false)
  const healthStatus = ref<HealthStatus | null>(null)
  // Three-state mobile toolbar collapse: expanded / collapsed-to-grip /
  // pinned-expanded. Driven by handleScroll (scroll position) and togglePin
  // (grip click). The pure transition lives in toolbarCollapsePolicy.
  const isScrolled = ref(false)
  const scrollTop = ref(0)
  const toolbarPinned = ref(false)
  const pinScrollTop = ref(0)
  // Reader immersive/fullscreen hides the grip even when scrolled.
  const immersiveHidden = ref(false)

  // Theme: 'light' | 'dark' | 'eye-care'
  const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | 'eye-care' | null
  const theme = ref<'light' | 'dark' | 'eye-care'>(savedTheme || 'light')

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  function setSidebarCollapsed(collapsed: boolean) {
    sidebarCollapsed.value = collapsed
  }

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : theme.value === 'dark' ? 'eye-care' : 'light'
  }

  function setTheme(t: 'light' | 'dark' | 'eye-care') {
    theme.value = t
  }

  function handleScroll(st: number) {
    scrollTop.value = st
    const next = computeScrollState(
      {
        isScrolled: isScrolled.value,
        toolbarPinned: toolbarPinned.value,
        pinScrollTop: pinScrollTop.value,
      },
      st,
    )
    isScrolled.value = next.isScrolled
    toolbarPinned.value = next.toolbarPinned
    pinScrollTop.value = next.pinScrollTop
  }

  function togglePin() {
    const next = applyPin(
      {
        isScrolled: isScrolled.value,
        toolbarPinned: toolbarPinned.value,
        pinScrollTop: pinScrollTop.value,
      },
      scrollTop.value,
    )
    toolbarPinned.value = next.toolbarPinned
    pinScrollTop.value = next.pinScrollTop
  }

  function setImmersive(v: boolean) {
    immersiveHidden.value = v
  }

  // Apply theme to <html> element and persist
  watch(theme, (t) => {
    document.documentElement.setAttribute('data-theme', t)
    localStorage.setItem('theme', t)
  }, { immediate: true })

  async function fetchHealth() {
    try {
      const { data } = await (await import('@/api')).default.get('/health')
      healthStatus.value = data
    } catch (e) {
      console.error('健康检查失败:', e)
    }
  }

  return {
    sidebarCollapsed,
    healthStatus,
    theme,
    isScrolled,
    scrollTop,
    toolbarPinned,
    pinScrollTop,
    immersiveHidden,
    toggleSidebar,
    setSidebarCollapsed,
    toggleTheme,
    setTheme,
    handleScroll,
    togglePin,
    setImmersive,
    fetchHealth,
  }
})
