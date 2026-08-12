import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_CANVAS_PIXELS,
  MAX_RENDER_DPR,
  computeRenderDpr,
  isWithinRenderWindow,
} from '../src/components/reader/pdfRenderPolicy.ts'

test('computeRenderDpr caps high-density displays', () => {
  assert.equal(computeRenderDpr(800, 1_000, 3), MAX_RENDER_DPR)
})

test('computeRenderDpr keeps the canvas inside the pixel budget', () => {
  const dpr = computeRenderDpr(2_000, 3_000, 2)
  const pixels = 2_000 * 3_000 * dpr * dpr

  assert.ok(pixels <= MAX_CANVAS_PIXELS + 1)
  assert.ok(dpr > 0)
})

test('computeRenderDpr handles invalid dimensions without producing NaN', () => {
  assert.equal(computeRenderDpr(0, 0, Number.NaN), 1)
})

test('isWithinRenderWindow includes nearby pages and excludes distant pages', () => {
  const root = { top: 100, bottom: 900 }

  assert.equal(isWithinRenderWindow({ top: 950, bottom: 1_150 }, root, 300), true)
  assert.equal(isWithinRenderWindow({ top: 1_250, bottom: 1_450 }, root, 300), false)
  assert.equal(isWithinRenderWindow({ top: -500, bottom: -250 }, root, 300), false)
})
