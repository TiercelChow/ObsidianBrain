import { stepSpring, type SpringState } from './motionSpring.ts'

interface SpringClock {
  now: () => number
  request: (callback: (time: number) => void) => number
  cancel: (id: number) => void
  reduced: () => boolean
}

/** One live physical track; retargets preserve both position and momentum. */
export function createSpringTrack(update: (value: number) => void, clock: SpringClock) {
  let state: SpringState = { value: 0, velocity: 0 }
  let target = 0
  let frame = 0
  let lastTime = 0
  let complete: (() => void) | undefined

  function pause() {
    if (frame) clock.cancel(frame)
    frame = 0
    complete = undefined
  }

  function tick(time: number) {
    frame = 0
    const elapsed = Math.max(0, Math.min((time - lastTime) / 1000, 1 / 30))
    lastTime = time
    const steps = Math.max(1, Math.ceil(elapsed * 120))
    for (let index = 0; index < steps; index++) {
      state = stepSpring(state, target, elapsed / steps, { response: .36, damping: .94 })
    }
    if (Math.abs(state.value - target) < .35 && Math.abs(state.velocity) < 3) {
      state = { value: target, velocity: 0 }
      update(target)
      const done = complete
      complete = undefined
      done?.()
    } else {
      update(state.value)
      frame = clock.request(tick)
    }
  }

  return {
    state: () => ({ ...state }),
    pause,
    jump(value: number, velocity = 0) {
      pause()
      state = { value, velocity }
      update(value)
    },
    to(value: number, done?: () => void) {
      target = value
      complete = done
      if (clock.reduced()) {
        pause()
        state = { value, velocity: 0 }
        update(value)
        done?.()
      } else if (!frame) {
        lastTime = clock.now()
        frame = clock.request(tick)
      }
    },
    dispose: pause,
  }
}
