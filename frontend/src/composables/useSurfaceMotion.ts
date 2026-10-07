import { computed, nextTick, onScopeDispose, ref, watch, type Ref } from 'vue'
import { createSpringTrack } from '../utils/springTrack.ts'

/** Shared lifetime for sheets and drawers: no CSS exit timer or second phase. */
export function useSurfaceMotion(open: () => boolean, panel: Ref<HTMLElement | null>, axis: 'x' | 'y', sign = () => 1) {
  const present = ref(false)
  const moving = ref(false)
  const offset = ref(0)
  const distance = ref(1)
  const phoneMedia = window.matchMedia('(max-width: 768px)')
  const reducedMedia = window.matchMedia('(prefers-reduced-motion: reduce)')
  const phone = ref(phoneMedia.matches)
  const reduced = ref(reducedMedia.matches)
  const track = createSpringTrack(value => { offset.value = value }, {
    now: () => performance.now(), request: callback => requestAnimationFrame(callback),
    cancel: id => cancelAnimationFrame(id), reduced: () => reduced.value,
  })
  let request = 0

  const opacity = computed(() => Math.min(1, Math.max(0, 1 - Math.abs(offset.value) / Math.max(1, distance.value))))
  const panelTransform = computed(() => {
    if (reduced.value) return 'none'
    if (axis === 'x') return `translate3d(${offset.value}px, 0, 0)`
    return `translate3d(0, ${offset.value}px, 0)${phone.value ? '' : ` scale(${.98 + .02 * opacity.value})`}`
  })

  function measure() {
    distance.value = axis === 'x' ? (panel.value?.offsetWidth || 320) + 8
      : phone.value ? (panel.value?.offsetHeight || window.innerHeight) + 24 : 18
  }
  function settle(velocity?: number) {
    measure()
    if (velocity !== undefined) track.jump(offset.value, velocity)
    moving.value = true
    track.to(open() ? 0 : distance.value * sign(), () => {
      moving.value = false
      if (!open()) present.value = false
    })
  }
  function grab() { track.pause(); moving.value = false }
  function follow(value: number) { track.jump(value) }
  function handoff(velocity: number) { track.jump(offset.value, velocity) }

  watch(open, async (value) => {
    const ownRequest = ++request
    if (value && !present.value) {
      // Hide the unmeasured mount for this tick, then enter from its actual size.
      distance.value = axis === 'x' ? window.innerWidth : phone.value ? window.innerHeight : 18
      track.jump(distance.value * sign())
      present.value = true
      await nextTick()
      if (ownRequest !== request) return
      measure()
      track.jump(distance.value * sign())
    }
    if (ownRequest === request && present.value) settle()
  }, { immediate: true })
  function syncPreferences() {
    phone.value = phoneMedia.matches
    reduced.value = reducedMedia.matches
    if (present.value) settle()
  }
  phoneMedia.addEventListener?.('change', syncPreferences)
  reducedMedia.addEventListener?.('change', syncPreferences)
  onScopeDispose(() => {
    request++
    track.dispose()
    phoneMedia.removeEventListener?.('change', syncPreferences)
    reducedMedia.removeEventListener?.('change', syncPreferences)
  })
  return { present, moving, offset, opacity, panelTransform, grab, follow, handoff, settle }
}
