export interface StreamFrameScheduler {
  request(callback: FrameRequestCallback): number
  cancel(id: number): void
  reducedMotion(): boolean
}

export interface StreamedTextBuffer {
  push(text: string): void
  drain(): Promise<void>
  cancel(): void
}

const browserScheduler: StreamFrameScheduler = {
  request: callback => window.requestAnimationFrame(callback),
  cancel: id => window.cancelAnimationFrame(id),
  reducedMotion: () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
}

/** Keep genuine small deltas immediate, but reveal provider-batched text over a few frames. */
export function createStreamedTextBuffer(
  append: (text: string) => void,
  scheduler: StreamFrameScheduler = browserScheduler,
): StreamedTextBuffer {
  let pending = ''
  let frameId: number | null = null
  let cancelled = false
  const waiters: Array<() => void> = []

  function settleWaiters() {
    for (const resolve of waiters.splice(0)) resolve()
  }

  function flush() {
    frameId = null
    if (cancelled) return
    if (!pending) {
      settleWaiters()
      return
    }
    let length = scheduler.reducedMotion()
      ? pending.length
      : Math.min(pending.length, Math.min(320, Math.max(24, Math.ceil(pending.length / 32))))
    if (length < pending.length && /[\uD800-\uDBFF]/.test(pending.charAt(length - 1))) length += 1
    const next = pending.slice(0, length)
    pending = pending.slice(length)
    append(next)
    if (pending) frameId = scheduler.request(flush)
    else settleWaiters()
  }

  return {
    push(text) {
      if (cancelled || !text) return
      pending += text
      if (frameId === null) frameId = scheduler.request(flush)
    },
    drain() {
      if (!pending && frameId === null) return Promise.resolve()
      return new Promise(resolve => { waiters.push(resolve) })
    },
    cancel() {
      cancelled = true
      pending = ''
      if (frameId !== null) scheduler.cancel(frameId)
      frameId = null
      settleWaiters()
    },
  }
}
