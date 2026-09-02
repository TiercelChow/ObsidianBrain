import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { applyPin, computeScrollState, rebaselinePin } from '@/utils/toolbarCollapsePolicy'

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
  // Brief grace window after a pin during which the layout shift from the
  // toolbar re-expanding is absorbed (see toolbarCollapsePolicy).
  const pinGraceUntil = ref(0)

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
      { inPinGrace: performance.now() < pinGraceUntil.value },
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
    // Give the re-expand a moment to settle before allowing scroll-driven
    // recollapse. The toolbar's max-height transition (--duration-slow ≈ 450ms)
    // continuously shifts .app-main.scrollTop while animating; without this
    // grace the shift trips the recollapse delta and instantly re-collapses
    // the toolbar we just opened (notably on Timeline where .app-main is the
    // scroller). Must exceed the transition duration. Extended to 800ms to
    // cover iOS Safari's throttled end-of-transition scroll event arriving
    // just before the rebaselinePin call (see rebasePinScrollTop).
    if (next.toolbarPinned) pinGraceUntil.value = performance.now() + 800
  }

  /** Re-baseline pinScrollTop to the live DOM scrollTop after the toolbar's
   *  max-height transition settles. iOS Safari throttles scroll events during
   *  the transition, so the inPinGrace absorption (which depends on
   *  intermediate scroll events arriving) never runs — iOS only delivers the
   *  final settled position after the grace window, which would otherwise
   *  trip the recollapse delta and instantly close the toolbar. The caller
   *  reads the live DOM scrollTop (not this store's ref, which is stale on
   *  iOS because the throttled events never updated it) and passes it here. */
  function rebasePinScrollTop(liveScrollTop: number) {
    const next = rebaselinePin(
      { isScrolled: isScrolled.value, toolbarPinned: toolbarPinned.value, pinScrollTop: pinScrollTop.value },
      liveScrollTop,
    )
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
    rebasePinScrollTop,
    setImmersive,
    fetchHealth,
  }
})
