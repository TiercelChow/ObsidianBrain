import { onMounted, onScopeDispose, ref, watch, type Ref } from 'vue'
import { advanceDockGesture, boundedScrollTop, createDockGesture, dockReleaseTarget } from '@/utils/mobileDockMotion'
import { stepSpring } from '@/utils/motionSpring'

/** Only deliberate scrolling of the touched pane drives navigation geometry. */
export function useMobileDockMotion(root: Ref<HTMLElement | null>, enabled: () => boolean, routeKey: () => string) {
  const progress = ref(0)
  const keyboardOpen = ref(false)
  let velocity = 0
  let frame = 0
  let releaseTimer = 0
  let scroller: HTMLElement | null = null
  let lastTop = 0
  let lastTime = 0
  let intentUntil = 0
  let touching = false
  let keyboardBaseline = window.innerHeight
  let gesture = createDockGesture(0)
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')

  function cancelSpring() {
    cancelAnimationFrame(frame)
    frame = 0
  }

  function clearGesture() {
    window.clearTimeout(releaseTimer)
    scroller = null
    intentUntil = 0
    touching = false
  }

  function settle(target: number) {
    cancelSpring()
    if (reducedMotion.matches) {
      progress.value = target
      velocity = 0
      return
    }
    let lastFrame = performance.now()
    function tick(now: number) {
      const state = stepSpring({ value: progress.value, velocity }, target, (now - lastFrame) / 1000, { response: 0.34, damping: 1 })
      lastFrame = now
      progress.value = Math.min(100, Math.max(0, state.value))
      velocity = state.velocity
      if (Math.abs(state.value - target) < 0.2 && Math.abs(velocity) < 2) {
        progress.value = target
        velocity = 0
        frame = 0
      } else {
        frame = requestAnimationFrame(tick)
      }
    }
    frame = requestAnimationFrame(tick)
  }

  function expand() {
    clearGesture()
    settle(0)
  }

  function blocked() {
    return !enabled() || keyboardOpen.value || document.documentElement.classList.contains('motion-overlay-open')
      || !!(document.activeElement?.closest('.mobile-navigation') && document.activeElement.matches(':focus-visible'))
  }

  function findScroller(target: EventTarget | null) {
    if (!(target instanceof Element) || !root.value?.contains(target) || target.closest('.mobile-navigation, [role="dialog"], input, textarea, select, [contenteditable="true"]')) return null
    for (let element: HTMLElement | null = target instanceof HTMLElement ? target : target.parentElement; element && root.value.contains(element); element = element.parentElement) {
      if (element.scrollHeight > element.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(element).overflowY)) return element
    }
    return null
  }

  function begin(event: Event) {
    if (blocked()) return
    if ((event.type === 'touchstart' && (event as TouchEvent).touches.length !== 1) || (event.type === 'wheel' && (event as WheelEvent).ctrlKey)) {
      clearGesture()
      return
    }
    const next = findScroller(event.target)
    if (!next) return
    if (scroller !== next || performance.now() > intentUntil) {
      scroller = next
      lastTop = boundedScrollTop(next.scrollTop, next.scrollHeight, next.clientHeight)
      gesture = createDockGesture(progress.value)
      lastTime = performance.now()
    }
    // Touch/wheel grabs the presentation, including an unfinished spring.
    cancelSpring()
    window.clearTimeout(releaseTimer)
    touching = event.type === 'touchstart'
    intentUntil = performance.now() + 900
    if (!touching) scheduleRelease()
  }

  function scheduleRelease() {
    window.clearTimeout(releaseTimer)
    releaseTimer = window.setTimeout(() => {
      if (touching) return
      const target = dockReleaseTarget(progress.value, velocity)
      clearGesture()
      settle(target)
    }, 120)
  }

  function end() {
    if (!scroller) return
    touching = false
    intentUntil = performance.now() + 900
    scheduleRelease()
  }

  function scroll(event: Event) {
    if (blocked() || !scroller || event.target !== scroller || (!touching && performance.now() > intentUntil)) return
    const now = performance.now()
    const top = boundedScrollTop(scroller.scrollTop, scroller.scrollHeight, scroller.clientHeight)
    const delta = top - lastTop
    lastTop = top
    const previous = progress.value
    gesture = advanceDockGesture(gesture, delta)
    if (reducedMotion.matches) {
      if (gesture.direction) progress.value = gesture.direction < 0 ? 100 : 0
      velocity = 0
    } else {
      progress.value = gesture.progress
      velocity = Math.max(-800, Math.min(800, (progress.value - previous) / Math.max(0.016, (now - lastTime) / 1000)))
    }
    lastTime = now
    intentUntil = now + 900
    if (!touching) scheduleRelease()
  }

  function syncKeyboard() {
    const viewport = window.visualViewport
    const editable = document.activeElement?.matches('input, textarea, [contenteditable="true"]')
    if (!editable) keyboardBaseline = window.innerHeight
    keyboardOpen.value = !!(editable && viewport && Math.max(keyboardBaseline, window.innerHeight) - viewport.height > 140)
    if (keyboardOpen.value) clearGesture()
  }

  function reset() {
    clearGesture()
    cancelSpring()
    progress.value = 0
    velocity = 0
  }

  watch([enabled, routeKey], reset)
  onMounted(() => {
    root.value?.addEventListener('touchstart', begin, { passive: true })
    root.value?.addEventListener('wheel', begin, { passive: true })
    root.value?.addEventListener('scroll', scroll, { passive: true, capture: true })
    window.addEventListener('touchend', end, { passive: true })
    window.addEventListener('touchcancel', end, { passive: true })
    window.visualViewport?.addEventListener('resize', syncKeyboard)
    window.addEventListener('resize', syncKeyboard)
    document.addEventListener('focusin', syncKeyboard)
    document.addEventListener('focusout', syncKeyboard)
  })
  onScopeDispose(() => {
    reset()
    root.value?.removeEventListener('touchstart', begin)
    root.value?.removeEventListener('wheel', begin)
    root.value?.removeEventListener('scroll', scroll, true)
    window.removeEventListener('touchend', end)
    window.removeEventListener('touchcancel', end)
    window.visualViewport?.removeEventListener('resize', syncKeyboard)
    window.removeEventListener('resize', syncKeyboard)
    document.removeEventListener('focusin', syncKeyboard)
    document.removeEventListener('focusout', syncKeyboard)
  })
  return { progress, keyboardOpen, expand }
}
