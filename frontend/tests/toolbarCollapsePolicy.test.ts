import assert from 'node:assert/strict'
import test from 'node:test'

import { computeScrollState, applyPin, rebaselinePin } from '../src/utils/toolbarCollapsePolicy.ts'

const base = { isScrolled: false, toolbarPinned: false, pinScrollTop: 0 }

test('at top: handleScroll(0) → not scrolled, not pinned', () => {
  assert.deepEqual(computeScrollState(base, 0), { isScrolled: false, toolbarPinned: false, pinScrollTop: 0 })
})

test('scrolled down: handleScroll(100) → scrolled, not pinned', () => {
  assert.deepEqual(computeScrollState(base, 100), { isScrolled: true, toolbarPinned: false, pinScrollTop: 0 })
})

test('pin: sets pinned true, records pinScrollTop', () => {
  const scrolled = { isScrolled: true, toolbarPinned: false, pinScrollTop: 0 }
  assert.deepEqual(applyPin(scrolled, 100), { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 })
})

test('after pin, small jitter (<4px) stays pinned', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 103).toolbarPinned, true)
})

test('after pin, scroll down >4px re-collapses', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 105).toolbarPinned, false)
})

test('after pin, scroll back to top clears both', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  const r = computeScrollState(pinned, 0)
  assert.equal(r.isScrolled, false)
  assert.equal(r.toolbarPinned, false)
})

test('after pin, scroll up but still >threshold stays pinned', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 60).toolbarPinned, true)
})

test('threshold boundary: 20 → not scrolled, 21 → scrolled', () => {
  assert.equal(computeScrollState(base, 20).isScrolled, false)
  assert.equal(computeScrollState(base, 21).isScrolled, true)
})

test('pin then unpin (safety branch) clears pinned', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  const r = applyPin(pinned, 100)
  assert.equal(r.toolbarPinned, false)
  assert.equal(r.pinScrollTop, 100)
})

test('inPinGrace: layout shift after pin is absorbed (baseline tracks up)', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  // Expanding the toolbar shifts scrollTop up by ~168 (layout shift), in grace.
  const r = computeScrollState(pinned, 268, { inPinGrace: true })
  assert.equal(r.toolbarPinned, true)            // not recollapsed
  assert.equal(r.pinScrollTop, 268)             // baseline absorbed the shift
})

test('after grace expires, scroll past absorbed baseline recollapses', () => {
  // Baseline was absorbed to 268 during grace. Now grace is over; scroll down.
  const absorbed = { isScrolled: true, toolbarPinned: true, pinScrollTop: 268 }
  assert.equal(computeScrollState(absorbed, 272).toolbarPinned, true) // < delta
  assert.equal(computeScrollState(absorbed, 273).toolbarPinned, false) // > delta
})

test('inPinGrace: scrolling up during grace lowers no baseline', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  const r = computeScrollState(pinned, 60, { inPinGrace: true })
  assert.equal(r.pinScrollTop, 100) // baseline never lowered
  assert.equal(r.toolbarPinned, true)
})

test('inPinGrace: scroll back to top still clears pin', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  const r = computeScrollState(pinned, 0, { inPinGrace: true })
  assert.equal(r.isScrolled, false)
  assert.equal(r.toolbarPinned, false)
})

// rebaselinePin: closes the iOS scroll-throttle gap. After the toolbar's
// max-height transition settles, the live scrollTop has shifted up (the
// expanding toolbar pushes content). iOS never delivered the intermediate
// positions as events, so pinScrollTop is still the click-time value and the
// first (post-grace) scroll event would recollapse. rebaselinePin raises the
// baseline to the live settled scrollTop.
test('rebaselinePin: raises baseline to live settled scrollTop', () => {
  // Clicked grip at scrollTop=544; transition shifted live scrollTop to 825.
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 544 }
  const r = rebaselinePin(pinned, 825)
  assert.equal(r.pinScrollTop, 825)
  assert.equal(r.toolbarPinned, true)
})

test('rebaselinePin: after rebase, a scroll event at the settled value does not recollapse', () => {
  const rebased = { isScrolled: true, toolbarPinned: true, pinScrollTop: 825 }
  // iOS delivers the throttled event at the settled position, now within delta.
  assert.equal(computeScrollState(rebased, 825).toolbarPinned, true)
})

test('rebaselinePin: never lowers the baseline', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 825 }
  // Live scrollTop read came back lower (e.g. user scrolled up); keep 825.
  assert.equal(rebaselinePin(pinned, 400).pinScrollTop, 825)
})

test('rebaselinePin: no-op when not pinned', () => {
  const notPinned = { isScrolled: true, toolbarPinned: false, pinScrollTop: 0 }
  const r = rebaselinePin(notPinned, 825)
  assert.equal(r.toolbarPinned, false)
  assert.equal(r.pinScrollTop, 0)
})
