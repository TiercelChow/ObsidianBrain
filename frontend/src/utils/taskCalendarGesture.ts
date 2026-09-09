export interface CalendarSwipeSample {
  dx: number
  dy: number
  /** Horizontal velocity in CSS pixels per millisecond. */
  velocityX: number
}

const INTENT_DISTANCE = 10
const DIRECTION_RATIO = 1.5
const COMMIT_DISTANCE = 44
const COMMIT_VELOCITY = 0.42

/** Claim only gestures that are clearly horizontal so the page keeps native vertical scrolling. */
export function claimCalendarSwipe(dx: number, dy: number): boolean {
  return Math.abs(dx) >= INTENT_DISTANCE
    && Math.abs(dx) >= Math.abs(dy) * DIRECTION_RATIO
}

/** Return +1 for next month, -1 for previous month, and 0 for a cancelled swipe. */
export function resolveCalendarSwipe({ dx, dy, velocityX }: CalendarSwipeSample): -1 | 0 | 1 {
  if (!claimCalendarSwipe(dx, dy)) return 0
  if (Math.abs(dx) < COMMIT_DISTANCE && Math.abs(velocityX) < COMMIT_VELOCITY) return 0
  return dx < 0 ? 1 : -1
}
