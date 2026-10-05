import { computed, onScopeDispose, ref } from 'vue'
import { isPhoneViewport } from '@/utils/mobileLayoutPolicy'

/** Share one breakpoint contract between the shell and responsive page content. */
export function usePhoneViewport() {
  const width = ref(window.innerWidth)
  const isMobile = computed(() => isPhoneViewport(width.value))
  const resize = () => { width.value = window.innerWidth }
  window.addEventListener('resize', resize)
  onScopeDispose(() => window.removeEventListener('resize', resize))
  return { width, isMobile }
}
