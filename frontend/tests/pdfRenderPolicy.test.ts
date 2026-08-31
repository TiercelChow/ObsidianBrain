import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_CANVAS_PIXELS,
  MAX_RENDER_DPR,
  computePdfZoomScale,
  computeRenderDpr,
  isWithinRenderWindow,
} from '../src/components/reader/pdfRenderPolicy.ts'

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
