export interface VerticalRect {
  top: number
  bottom: number
}

/** Retina remains crisp at 2x; denser buffers cost memory without visible gain. */
export const MAX_RENDER_DPR = 2

/** Bound every live page canvas to roughly 16 MB of RGBA pixels. */
export const MAX_CANVAS_PIXELS = 4_000_000

/**
 * Pick a device-pixel ratio that balances text sharpness with a strict canvas
 * memory ceiling. The CSS viewport size is unchanged; only the backing buffer
 * is reduced for unusually large pages or very dense displays.
 */
export function computeRenderDpr(
  viewportWidth: number,
  viewportHeight: number,
  devicePixelRatio: number,
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
  const budgetDpr = Math.sqrt(MAX_CANVAS_PIXELS / (viewportWidth * viewportHeight))

  return Math.max(Number.EPSILON, Math.min(safeDeviceDpr, MAX_RENDER_DPR, budgetDpr))
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
