import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export interface HealthStatus {
  status: string
  version: string
  components: Record<string, string>
}

export const useAppStore = defineStore('app', () => {
  const sidebarCollapsed = ref(false)
  const healthStatus = ref<HealthStatus | null>(null)
  // Whether the page is scrolled (drives the mobile global header + page-header
  // collapse). Set by App.vue (app-main scroll) and the Reader (pane-center scroll).
  const isScrolled = ref(false)

  // Theme: 'light' | 'dark'
  const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | null
  const theme = ref<'light' | 'dark'>(savedTheme || 'light')

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  function toggleTheme() {
    theme.value = theme.value === 'light' ? 'dark' : 'light'
  }

  function setTheme(t: 'light' | 'dark') {
    theme.value = t
  }

  function setScrolled(v: boolean) {
    isScrolled.value = v
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

  return { sidebarCollapsed, healthStatus, theme, isScrolled, toggleSidebar, toggleTheme, setTheme, setScrolled, fetchHealth }
})
