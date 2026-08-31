export interface VerticalRect {
  top: number
  bottom: number
}

/** Preserve native resolution up to 3x; the pixel budget caps large/zoomed pages. */
export const MAX_RENDER_DPR = 3

/** Bound every live page canvas to roughly 16 MB of RGBA pixels. */
export const MAX_CANVAS_PIXELS = 4_000_000

/**
 * Pick a device-pixel ratio that balances text sharpness with a per-canvas
 * memory ceiling. The CSS viewport size is unchanged; only the backing buffer
 * is reduced for unusually large pages or very dense displays.
 *
 * `maxCanvasPixels` is the per-device budget (phone ~4M, desktop ~16M) so a
 * large page on a wide desktop window still renders at native retina instead
 * of being throttled to ~1× — text stays crisp.
 */
export function computeRenderDpr(
  viewportWidth: number,
  viewportHeight: number,
  devicePixelRatio: number,
  maxCanvasPixels: number,
): number {
  if (
    !Number.isFinite(viewportWidth)
    || !Number.isFinite(viewportHeight)
    || viewportWidth <= 0
    || viewportHeight <= 0
  ) {
    return 1
  }

  const safeDeviceDpr = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0
    ? devicePixelRatio
    : 1
  const budget = Number.isFinite(maxCanvasPixels) && maxCanvasPixels > 0
    ? maxCanvasPixels
    : MAX_CANVAS_PIXELS
  const budgetDpr = Math.sqrt(budget / (viewportWidth * viewportHeight))

  return Math.max(Number.EPSILON, Math.min(safeDeviceDpr, MAX_RENDER_DPR, budgetDpr))
}

/** Apply toolbar zoom relative to the immutable fit-width baseline. */
export function computePdfZoomScale(fitScale: number, ratio: number): number {
  const safeFitScale = Number.isFinite(fitScale) && fitScale > 0 ? fitScale : 1
  const safeRatio = Number.isFinite(ratio) ? Math.max(0.6, Math.min(2, ratio)) : 1
  return safeFitScale * safeRatio
}

/** Whether a page overlaps the viewport plus the pre-render/recycle margin. */
export function isWithinRenderWindow(
  page: VerticalRect,
  root: VerticalRect,
  margin: number,
): boolean {
  const safeMargin = Math.max(0, margin)
  return page.bottom >= root.top - safeMargin && page.top <= root.bottom + safeMargin
}
