export const PHONE_BREAKPOINT = 768

export function isPhoneViewport(width: number): boolean {
  return Number.isFinite(width) && width <= PHONE_BREAKPOINT
}

/** Reader uses its own document scroller on phones; the app shell must stay fixed. */
export function shouldLockMobileReaderOuterScroll(width: number, routePath: string): boolean {
  return isPhoneViewport(width) && routePath === '/reader'
}

export interface MobileReaderToolbarState {
  rendered: boolean
  pinned: boolean
  visible: boolean
}

/** Keep the document picker reachable until a folder has an active document. */
export function getMobileReaderToolbarState(
  hasOpenFolder: boolean,
  hasDisplayedDocument: boolean,
  transientVisible: boolean,
): MobileReaderToolbarState {
  const rendered = hasOpenFolder || hasDisplayedDocument
  const pinned = hasOpenFolder && !hasDisplayedDocument
  return {
    rendered,
    pinned,
    visible: rendered && (pinned || transientVisible),
  }
}

export interface PdfRenderPolicy {
  renderMarginPx: number
  maxConcurrentRenders: number
  maxRenderDpr: number
  maxCanvasPixels: number
}

/** Keep phone canvas work close to the viewport and avoid parallel allocations. */
export function getPdfRenderPolicy(
  viewportWidth: number,
  hardwareConcurrency = 8,
): PdfRenderPolicy {
  const lowCoreDevice = !Number.isFinite(hardwareConcurrency) || hardwareConcurrency <= 4
  if (isPhoneViewport(viewportWidth)) {
    // A 1.5x backing buffer is visibly soft on 2x/3x phone displays,
    // especially for PDFs with small type. Keep fewer pages alive instead of
    // sacrificing the resolution of the page the user is actually reading.
    return { renderMarginPx: 420, maxConcurrentRenders: 1, maxRenderDpr: 3, maxCanvasPixels: 4_000_000 }
  }

  // Desktops have the memory to keep a large/zoomed page at full retina (2×)
  // instead of throttling the backing store to ~1× when the page exceeds a
  // tight phone-sized budget — that throttling is what made text look blurry.
  return {
    renderMarginPx: 700,
    maxConcurrentRenders: lowCoreDevice ? 1 : 2,
    maxRenderDpr: 2,
    maxCanvasPixels: 16_000_000,
  }
}
