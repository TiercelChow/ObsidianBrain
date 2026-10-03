import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_CANVAS_PIXELS,
  MAX_RENDER_DPR,
  computeCenteredZoomScrollTop,
  computePdfZoomScale,
  computeRenderDpr,
  computePdfFitScale,
  computePdfReadingRatio,
  computePdfDetailRegion,
  computePdfZoomAnchorScroll,
  nextPdfZoomRatio,
  isWithinRenderWindow,
} from '../src/components/reader/pdfRenderPolicy.ts'

test('zoom keeps the same document point at the viewport center', () => {
  assert.equal(computeCenteredZoomScrollTop(700, 600, 2400, 600, 4800), 1700)
})

test('zoom center restoration clamps to document bounds', () => {
  assert.equal(computeCenteredZoomScrollTop(0, 600, 2400, 600, 1200), 0)
  assert.equal(computeCenteredZoomScrollTop(1800, 600, 2400, 600, 1200), 600)
})

test('computeRenderDpr caps high-density displays', () => {
  assert.equal(computeRenderDpr(500, 700, 4, MAX_CANVAS_PIXELS), MAX_RENDER_DPR)
})

test('computeRenderDpr keeps native resolution on a 3x phone display', () => {
  assert.equal(computeRenderDpr(360, 500, 3, MAX_CANVAS_PIXELS), 3)
})

test('computeRenderDpr keeps the canvas inside the pixel budget', () => {
  const dpr = computeRenderDpr(2_000, 3_000, 2, MAX_CANVAS_PIXELS)
  const pixels = 2_000 * 3_000 * dpr * dpr

  assert.ok(pixels <= MAX_CANVAS_PIXELS + 1)
  assert.ok(dpr > 0)
})

test('a 4M phone budget reduces a large 2x page below native', () => {
  // 1200×1696 CSS px on a 2x display: the 4M budget can't hold 2× → soft.
  assert.ok(computeRenderDpr(1200, 1696, 2, 4_000_000) < 2)
})

test('a 16M desktop budget keeps native 2x on the same large page', () => {
  // Same page, 16M budget: budgetDpr ≈ 2.8, clamped to native 2 → sharp.
  assert.equal(computeRenderDpr(1200, 1696, 2, 16_000_000), 2)
})

test('computeRenderDpr handles invalid dimensions without producing NaN', () => {
  assert.equal(computeRenderDpr(0, 0, Number.NaN, MAX_CANVAS_PIXELS), 1)
})

test('isWithinRenderWindow includes nearby pages and excludes distant pages', () => {
  const root = { top: 100, bottom: 900 }

  assert.equal(isWithinRenderWindow({ top: 950, bottom: 1_150 }, root, 300), true)
  assert.equal(isWithinRenderWindow({ top: 1_250, bottom: 1_450 }, root, 300), false)
  assert.equal(isWithinRenderWindow({ top: -500, bottom: -250 }, root, 300), false)
})

test('computePdfZoomScale always uses the stable fit-width scale', () => {
  assert.equal(computePdfZoomScale(0.5, 1.2), 0.6)
  assert.equal(computePdfZoomScale(0.5, 1.45), 0.725)
})

test('phone fit uses actual padding instead of subtracting desktop gutters', () => {
  assert.equal(computePdfFitScale(390, 600, 4), 386 / 600)
  assert.equal(computePdfFitScale(800, 600, 40, 2), 758 / 600)
  assert.equal(computePdfFitScale(0, 600, 40), 0)
})

test('reading fit removes modest PDF margins but never hides access to the whole page', () => {
  assert.equal(computePdfReadingRatio(600, 60, 540), 1.25)
  assert.equal(computePdfReadingRatio(600, 250, 350), 1)
  assert.equal(computePdfReadingRatio(600, 0, 600), 1)
  assert.equal(computePdfReadingRatio(600, Number.NaN, 500), 1)
})

test('zoom controls support 4x and continue from an arbitrary pinch ratio', () => {
  assert.equal(computePdfZoomScale(.5, 4), 2)
  assert.equal(computePdfZoomScale(.5, 20), 2)
  assert.equal(nextPdfZoomRatio(1.34, 1), 1.5)
  assert.equal(nextPdfZoomRatio(1.34, -1), 1.25)
  assert.equal(nextPdfZoomRatio(4, 1), 4)
})

test('zoom preserves a page-local point, independent of document length and page gaps', () => {
  assert.equal(computePdfZoomAnchorScroll(700, -300, 1200, .4, 180, 2400, 600), 700)
  assert.equal(computePdfZoomAnchorScroll(0, 10, 780, .5, 195, 900, 390), 205)
  assert.equal(computePdfZoomAnchorScroll(0, 10, 780, 0, 195, 900, 390), 0)
})

test('high-resolution detail only covers the visible part of a large zoomed page', () => {
  const region = computePdfDetailRegion(
    { left: -450, top: -800, width: 1560, height: 2200 },
    { left: 0, top: 0, width: 390, height: 700 },
  )
  assert.deepEqual(region, { left: 418, top: 768, width: 454, height: 764 })
  assert.equal(computeRenderDpr(region!.width, region!.height, 3, MAX_CANVAS_PIXELS), 3)
  assert.ok(region!.width * region!.height * 9 < MAX_CANVAS_PIXELS)
  assert.equal(computePdfDetailRegion(
    { left: 0, top: 800, width: 390, height: 500 },
    { left: 0, top: 0, width: 390, height: 700 },
  ), null)
})
