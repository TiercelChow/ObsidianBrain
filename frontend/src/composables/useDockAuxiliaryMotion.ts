import { onScopeDispose, ref, shallowRef, watch } from 'vue'
import { auxiliaryDockSettled, stepAuxiliaryDock, type AuxiliaryDockFrame, type AuxiliaryDockMode, type AuxiliaryDockState } from '@/utils/dockAuxiliaryMotion'

/** Route changes retarget the live surface; scroll keeps its existing 1:1 spring. */
export function useDockAuxiliaryMotion(source: () => { mode: AuxiliaryDockMode; frame: AuxiliaryDockFrame } | null) {
  const presentation = shallowRef<AuxiliaryDockFrame | null>(null)
  const moving = ref(false)
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)')
  let target = source()
  let mode = target?.mode
  let state: AuxiliaryDockState | null = target ? { frame: target.frame, velocity: {} } : null
  let animation = 0, previousTime = 0
  presentation.value = state?.frame ?? null

  function snap() {
    cancelAnimationFrame(animation)
    animation = 0
    moving.value = false
    if (target) {
      state = { frame: target.frame, velocity: {} }
      presentation.value = target.frame
    }
  }
  function tick(now: number) {
    if (!state || !target) return
    state = stepAuxiliaryDock(state, target.frame, (now - previousTime) / 1000)
    previousTime = now
    presentation.value = state.frame
    if (auxiliaryDockSettled(state, target.frame)) snap()
    else animation = requestAnimationFrame(tick)
  }
  watch(source, next => {
    // Lazy successors can register after the outgoing route is disposed.
    // Keep the surface; its old controls are already inert in MobileDock.
    if (!next) return
    const changed = mode !== next.mode
    mode = next.mode
    target = next
    if (!state || reduced.matches || (!moving.value && !changed)) { snap(); return }
    if (animation) return // Retain presentation and velocity on interruption.
    moving.value = true
    previousTime = performance.now()
    animation = requestAnimationFrame(tick)
  }, { flush: 'post' })
  function preferencesChanged() { if (reduced.matches) snap() }
  reduced.addEventListener('change', preferencesChanged)
  onScopeDispose(() => { cancelAnimationFrame(animation); reduced.removeEventListener('change', preferencesChanged) })
  return { presentation, moving }
}
