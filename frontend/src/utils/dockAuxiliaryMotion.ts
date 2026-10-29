import { dockGeometry, dockRebound } from './mobileDockMotion.ts'
import { stepSpring } from './motionSpring.ts'

export interface AuxiliaryDockFrame {
  x: number; y: number; width: number; height: number
  barOpacity: number; actionOpacity: number
}
export type AuxiliaryDockMode = 'action' | 'subnav'
export interface AuxiliaryDockState {
  frame: AuxiliaryDockFrame
  velocity: Partial<Record<keyof AuxiliaryDockFrame, number>>
}
const axes: (keyof AuxiliaryDockFrame)[] = ['x', 'y', 'width', 'height', 'barOpacity', 'actionOpacity']

/** Local to the main Dock's bottom edge; matches its existing scroll geometry. */
export function auxiliaryDockLayout(width: number, mode: AuxiliaryDockMode, progress: number, options: { actionInset?: number; actionSize?: number; actionGap?: number } = {}): AuxiliaryDockFrame {
  const w = Number.isFinite(width) ? Math.max(56, width) : 56
  if (mode === 'action') {
    const size = options.actionSize ?? 56
    return { x: Math.max(0, w - (options.actionInset ?? 8) - size), y: -(62 + (options.actionGap ?? 12)), width: size, height: size, barOpacity: 0, actionOpacity: 1 }
  }
  const geometry = dockGeometry(progress)
  const x = 12 * (1 - geometry.travelX)
  const right = 12 + 58 * geometry.travelX
  return { x, y: -70 + 64 * geometry.travelY + 6 * geometry.sink - 4 * dockRebound(progress), width: Math.max(0, w - x - right), height: 50 + 12 * geometry.sink, barOpacity: 1, actionOpacity: 0 }
}

/** Independent springs preserve live geometry AND velocity when a route reverses. */
export function stepAuxiliaryDock(state: AuxiliaryDockState, target: AuxiliaryDockFrame, elapsed: number): AuxiliaryDockState {
  const dt = Number.isFinite(elapsed) ? Math.max(0, Math.min(1 / 30, elapsed)) : 0
  const steps = Math.max(1, Math.ceil(dt * 120))
  const frame = { ...state.frame }, velocity = { ...state.velocity }
  for (let i = 0; i < steps; i++) for (const axis of axes) {
    const next = stepSpring({ value: frame[axis], velocity: velocity[axis] ?? 0 }, target[axis], dt / steps, { response: .34, damping: 1 })
    frame[axis] = next.value
    velocity[axis] = next.velocity
  }
  return { frame, velocity }
}

export function auxiliaryDockSettled(state: AuxiliaryDockState, target: AuxiliaryDockFrame) {
  return axes.every(axis => {
    const opacity = axis.endsWith('Opacity')
    return Math.abs(state.frame[axis] - target[axis]) < (opacity ? .002 : .1) && Math.abs(state.velocity[axis] ?? 0) < (opacity ? .02 : 1)
  })
}
