import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface HealthStatus {
  status: string
  version: string
  components: Record<string, string>
}

export const useAppStore = defineStore('app', () => {
  const sidebarCollapsed = ref(false)
  const healthStatus = ref<HealthStatus | null>(null)

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  async function fetchHealth() {
    try {
      const { data } = await (await import('@/api')).default.get('/health')
      healthStatus.value = data
    } catch (e) {
      console.error('健康检查失败:', e)
    }
  }

  return { sidebarCollapsed, healthStatus, toggleSidebar, fetchHealth }
})
