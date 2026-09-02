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

/** Transition for a scroll event.
 *  `inPinGrace` absorbs the layout shift that fires immediately after a pin:
 *  expanding the toolbar pushes content down and inflates scrollTop, which
 *  would otherwise instantly exceed pinScrollTop+RECOLLAPSE_DELTA and recollapse
 *  the toolbar we just opened. During grace, the baseline tracks scrollTop up
 *  (never down) so the shift is absorbed; real downward scroll after grace then
 *  recollapses. */
export function computeScrollState(
  prev: ScrollState,
  scrollTop: number,
  opts: { inPinGrace?: boolean } = {},
): ScrollState {
  const inGrace = opts.inPinGrace ?? false
  const isScrolled = scrollTop > SCROLL_THRESHOLD
  let toolbarPinned = prev.toolbarPinned
  let pinScrollTop = prev.pinScrollTop

  if (scrollTop <= SCROLL_THRESHOLD) {
    // Back to top → natural full expand, pin cleared.
    toolbarPinned = false
  } else if (inGrace && toolbarPinned) {
    // Absorb the post-pin layout shift: raise the baseline with scrollTop
    // (never lower it) so the shift doesn't trip the recollapse delta.
    pinScrollTop = Math.max(pinScrollTop, scrollTop)
  } else if (toolbarPinned && scrollTop > pinScrollTop + RECOLLAPSE_DELTA) {
    // Continued scrolling down after grace → re-collapse to the grip.
    toolbarPinned = false
  }
  // Scrolling up while pinned, or jitter < delta, keeps it expanded.

  return { isScrolled, toolbarPinned, pinScrollTop }
}

/** Re-baseline pinScrollTop to the live scrollTop after the toolbar's
 *  max-height transition settles. iOS Safari throttles scroll events during
 *  the transition, so the inPinGrace absorption above (which depends on
 *  intermediate scroll events arriving during the grace window) may never
 *  run — iOS only delivers the final settled position, after grace, which
 *  would then trip the recollapse delta and instantly close the toolbar we
 *  just opened. The caller reads the live DOM scrollTop after the transition
 *  (not the store ref, which is stale on iOS because the throttled events
 *  never updated it) and raises the baseline here. Only raises, never lowers;
 *  no-op when not pinned. */
export function rebaselinePin(prev: ScrollState, liveScrollTop: number): ScrollState {
  if (!prev.toolbarPinned) return prev
  return { ...prev, pinScrollTop: Math.max(prev.pinScrollTop, liveScrollTop) }
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
