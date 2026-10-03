export interface VerticalRect {
  top: number
  bottom: number
}

/** Preserve native resolution up to 3x; the pixel budget caps large/zoomed pages. */
export const MAX_RENDER_DPR = 3

/** Bound every live page canvas to roughly 16 MB of RGBA pixels. */
export const MAX_CANVAS_PIXELS = 4_000_000

export const PDF_ZOOM_LEVELS = [0.6, 0.8, 1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4] as const

/** Match the real viewer padding, including a desktop page border. */
export function computePdfFitScale(width: number, pageWidth: number, padding = 0, border = 0): number {
  if (!Number.isFinite(width) || !Number.isFinite(pageWidth) || width <= 0 || pageWidth <= 0) return 0
  return Math.max(1, width - Math.max(0, padding) - Math.max(0, border)) / pageWidth
}

/** Modest text-margin focus on phones; narrow columns/unknown bounds stay full-page. */
export function computePdfReadingRatio(pageWidth: number, left: number, right: number): number {
  const contentWidth = right - left
  if (![pageWidth, left, right].every(Number.isFinite) || pageWidth <= 0
    || left < 0 || right > pageWidth || contentWidth < pageWidth * .6) return 1
  return Math.min(1.4, Math.max(1, pageWidth / contentWidth))
}

export function nextPdfZoomRatio(ratio: number, direction: -1 | 1): number {
  const current = Number.isFinite(ratio) ? ratio : 1
  return direction === 1
    ? PDF_ZOOM_LEVELS.find(value => value > current + .01) ?? 4
    : [...PDF_ZOOM_LEVELS].reverse().find(value => value < current - .01) ?? .6
}

/** Keep a page-local point under the finger/viewport centre after relayout. */
export function computePdfZoomAnchorScroll(
  scroll: number, pageStart: number, pageSize: number, fraction: number,
  anchor: number, scrollSize: number, viewportSize: number,
): number {
  const target = scroll + pageStart + pageSize * Math.max(0, Math.min(1, fraction)) - anchor
  return Math.max(0, Math.min(target, Math.max(0, scrollSize - viewportSize)))
}

export interface PdfRect { left: number; top: number; width: number; height: number }

/** A bounded viewport tile, not another full-page retina allocation. */
export function computePdfDetailRegion(page: PdfRect, root: PdfRect, overscan = 32): PdfRect | null {
  if (![page.left, page.top, page.width, page.height, root.left, root.top, root.width, root.height].every(Number.isFinite)
    || page.width <= 0 || page.height <= 0 || root.width <= 0 || root.height <= 0
    || page.top >= root.top + root.height || page.top + page.height <= root.top
    || page.left >= root.left + root.width || page.left + page.width <= root.left) return null
  const margin = Math.max(0, overscan)
  const left = Math.max(0, Math.floor(root.left - page.left - margin))
  const top = Math.max(0, Math.floor(root.top - page.top - margin))
  const right = Math.min(page.width, Math.ceil(root.left + root.width - page.left + margin))
  const bottom = Math.min(page.height, Math.ceil(root.top + root.height - page.top + margin))
  return { left, top, width: right - left, height: bottom - top }
}

/**
 * Restore scroll after a zoom while keeping the same document point under the
 * viewport center. This is more stable than preserving scrollTop/maxScroll:
 * toolbars and safe-area padding can change the scrollable range without
 * changing the point the reader is looking at.
 */
export function computeCenteredZoomScrollTop(
  previousScrollTop: number,
  previousViewportHeight: number,
  previousScrollHeight: number,
  nextViewportHeight: number,
  nextScrollHeight: number,
): number {
  if (
    !Number.isFinite(previousScrollHeight)
    || !Number.isFinite(nextScrollHeight)
    || previousScrollHeight <= 0
    || nextScrollHeight <= 0
  ) return 0

  const previousCenter = Math.max(0, previousScrollTop) + Math.max(0, previousViewportHeight) / 2
  const centerFraction = Math.min(1, previousCenter / previousScrollHeight)
  const nextTop = centerFraction * nextScrollHeight - Math.max(0, nextViewportHeight) / 2
  return Math.max(0, Math.min(nextTop, Math.max(0, nextScrollHeight - nextViewportHeight)))
}

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
  const safeRatio = Number.isFinite(ratio) ? Math.max(0.6, Math.min(4, ratio)) : 1
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
