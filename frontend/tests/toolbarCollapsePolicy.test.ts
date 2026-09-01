import assert from 'node:assert/strict'
import test from 'node:test'

import { computeScrollState, applyPin } from '../src/utils/toolbarCollapsePolicy.ts'

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
