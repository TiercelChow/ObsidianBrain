export interface SpringState {
  value: number
  velocity: number
}

export interface SpringOptions {
  response?: number
  damping?: number
}

export function projectMotion(value: number, velocity: number, projectionSeconds = 0.18) {
  return value + velocity * projectionSeconds
}

/** A small display-synchronised spring step for gesture handoff. */
export function stepSpring(
  state: SpringState,
  target: number,
  elapsedSeconds: number,
  options: SpringOptions = {},
): SpringState {
  const response = options.response ?? 0.36
  const dampingRatio = options.damping ?? 1
  const dt = Math.min(Math.max(elapsedSeconds, 0), 1 / 30)
  const angularFrequency = (2 * Math.PI) / response
  const stiffness = angularFrequency * angularFrequency
  const damping = 2 * dampingRatio * angularFrequency
  const acceleration = -stiffness * (state.value - target) - damping * state.velocity
  const velocity = state.velocity + acceleration * dt
  return { value: state.value + velocity * dt, velocity }
}

export function animateSpring(
  from: number,
  target: number,
  velocity: number,
  onUpdate: (value: number) => void,
  onComplete: () => void,
  options: SpringOptions = {},
) {
  if (typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    onUpdate(target)
    onComplete()
    return () => undefined
  }

  let state: SpringState = { value: from, velocity }
  let frame = 0
  let lastTime = performance.now()
  let cancelled = false

  const tick = (time: number) => {
    if (cancelled) return
    state = stepSpring(state, target, (time - lastTime) / 1000, options)
    lastTime = time
    onUpdate(state.value)
    if (Math.abs(state.value - target) < 0.35 && Math.abs(state.velocity) < 3) {
      onUpdate(target)
      onComplete()
      return
    }
    frame = requestAnimationFrame(tick)
  }

  frame = requestAnimationFrame(tick)
  return () => {
    cancelled = true
    cancelAnimationFrame(frame)
  }
}
