/**
 * Pure state machine for the mobile collapsing-toolbar interaction.
 *
 * Three states, driven by scroll position:
 *  - expanded          (scrollTop ≤ THRESHOLD): full toolbar + page header
 *  - collapsed-to-grip (scrolled, not pinned): only the floating grip shows
 *  - pinned-expanded   (scrolled, user clicked grip): toolbar re-expanded;
 *    continues to collapse back to the grip once the user scrolls down past
 *    RECOLLAPSE_DELTA from the pin point.
 *
 * Extracted as pure functions so it can be unit-tested with node:test
 * (the appStore wraps these — see stores/app.ts).
 */

export const SCROLL_THRESHOLD = 20
export const RECOLLAPSE_DELTA = 4

export interface ScrollState {
  isScrolled: boolean
  toolbarPinned: boolean
  pinScrollTop: number
}

/** Transition for a scroll event. */
export function computeScrollState(prev: ScrollState, scrollTop: number): ScrollState {
  const isScrolled = scrollTop > SCROLL_THRESHOLD
  let toolbarPinned = prev.toolbarPinned
  const pinScrollTop = prev.pinScrollTop

  if (scrollTop <= SCROLL_THRESHOLD) {
    // Back to top → natural full expand, pin cleared.
    toolbarPinned = false
  } else if (toolbarPinned && scrollTop > pinScrollTop + RECOLLAPSE_DELTA) {
    // Continued scrolling down → re-collapse to the grip.
    toolbarPinned = false
  }
  // Scrolling up while pinned, or jitter < delta, keeps it expanded.

  return { isScrolled, toolbarPinned, pinScrollTop }
}

/** Transition for a grip click (expand while scrolled). */
export function applyPin(prev: ScrollState, currentScrollTop: number): ScrollState {
  if (prev.toolbarPinned) {
    // Safety branch: the grip is hidden when pinned, so this is only reached
    // via a manual collapse control. Clear the pin, keep the pin point.
    return { ...prev, toolbarPinned: false }
  }
  return { isScrolled: prev.isScrolled, toolbarPinned: true, pinScrollTop: currentScrollTop }
}
