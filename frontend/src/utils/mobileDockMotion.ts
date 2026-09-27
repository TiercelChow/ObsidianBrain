import { projectMotion, stepSpring, type SpringState } from './motionSpring.ts'

export interface DockGesture {
  progress: number
  direction: number
  pending: number
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

/** Retraction and the clearing arc share one clock, with no staged holds or swaps. */
export function dockGeometry(progress: number) {
  const value = Number.isFinite(progress) ? clamp(progress / 100, 0, 1) : 0
  return { shape: 1 - (1 - value) ** 4, sink: value, travelX: value * (2 - value), travelY: value ** 4 }
}

/** Soft landing feedback is independent of layout and never shrinks subnav targets. */
export function dockRebound(progress: number): number {
  if (!Number.isFinite(progress)) return 0
  const overshoot = Math.abs(progress - clamp(progress, 0, 100))
  return overshoot / (1 + overshoot)
}

/** Navigation trails scrolling gently; a flick must not collapse it in one frame. */
export function stepDockSpring(state: SpringState, target: number, elapsedSeconds: number): SpringState {
  const dt = clamp(elapsedSeconds, 0, 1 / 30)
  // Follow the finger without oscillation; only endpoint landings have elasticity.
  const options = { response: 0.44, damping: target === 0 || target === 100 ? .76 : 1 }
  // Small integration steps keep 30/60/120Hz equally elastic, even on long frames.
  const steps = Math.max(1, Math.ceil(dt * 120))
  const stepTime = dt / steps
  let current = state
  for (let step = 0; step < steps; step++) {
    const next = stepSpring(current, target, stepTime, options)
    const velocity = clamp(next.velocity, -300, 300)
    // Preserve physical overshoot. dockGeometry alone bounds layout dimensions.
    current = { value: current.value + velocity * stepTime, velocity }
  }
  return current
}

export function createDockGesture(progress: number): DockGesture {
  return { progress: Number.isFinite(progress) ? clamp(progress, 0, 100) : 0, direction: 0, pending: 0 }
}

/** Finger up (positive scroll delta) contracts; finger down restores both layers. */
export function advanceDockGesture(state: DockGesture, scrollDelta: number): DockGesture {
  if (!Number.isFinite(scrollDelta) || scrollDelta === 0) return state
  const direction = Math.sign(scrollDelta)
  let pending = state.pending + scrollDelta
  if (direction !== state.direction) {
    if (Math.abs(pending) < 6) return { ...state, pending }
  } else {
    pending = scrollDelta
  }
  return { progress: clamp(state.progress + pending * (100 / 120), 0, 100), direction, pending: 0 }
}

export function dockReleaseTarget(progress: number, velocity: number): 0 | 100 {
  return projectMotion(progress, velocity, 0.12) >= 50 ? 100 : 0
}

/** Safari rubber-banding is not real content movement. */
export function boundedScrollTop(top: number, scrollHeight: number, clientHeight: number): number {
  return Number.isFinite(top) ? clamp(top, 0, Math.max(0, scrollHeight - clientHeight)) : 0
}
