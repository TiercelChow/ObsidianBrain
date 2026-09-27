import { projectMotion } from './motionSpring.ts'

export interface DockGesture {
  progress: number
  direction: number
  pending: number
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

export function createDockGesture(progress: number): DockGesture {
  return { progress: Number.isFinite(progress) ? clamp(progress, 0, 100) : 0, direction: 0, pending: 0 }
}

/** Positive scroll delta is a finger moving upwards; negative contracts right. */
export function advanceDockGesture(state: DockGesture, scrollDelta: number): DockGesture {
  if (!Number.isFinite(scrollDelta) || scrollDelta === 0) return state
  const direction = Math.sign(scrollDelta)
  let pending = state.pending + scrollDelta
  if (direction !== state.direction) {
    if (Math.abs(pending) < 6) return { ...state, pending }
  } else {
    pending = scrollDelta
  }
  return { progress: clamp(state.progress - pending * (100 / 120), 0, 100), direction, pending: 0 }
}

export function dockReleaseTarget(progress: number, velocity: number): 0 | 100 {
  return projectMotion(progress, velocity, 0.12) >= 50 ? 100 : 0
}

/** Safari rubber-banding is not real content movement. */
export function boundedScrollTop(top: number, scrollHeight: number, clientHeight: number): number {
  return Number.isFinite(top) ? clamp(top, 0, Math.max(0, scrollHeight - clientHeight)) : 0
}
