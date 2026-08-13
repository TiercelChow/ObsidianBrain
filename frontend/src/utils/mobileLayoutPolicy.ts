export const PHONE_BREAKPOINT = 768

export function isPhoneViewport(width: number): boolean {
  return Number.isFinite(width) && width <= PHONE_BREAKPOINT
}

export interface PdfRenderPolicy {
  renderMarginPx: number
  maxConcurrentRenders: number
  maxRenderDpr: number
}

/** Keep phone canvas work close to the viewport and avoid parallel allocations. */
export function getPdfRenderPolicy(
  viewportWidth: number,
  hardwareConcurrency = 8,
): PdfRenderPolicy {
  const lowCoreDevice = !Number.isFinite(hardwareConcurrency) || hardwareConcurrency <= 4
  if (isPhoneViewport(viewportWidth)) {
    return { renderMarginPx: 420, maxConcurrentRenders: 1, maxRenderDpr: 1.5 }
  }

  return {
    renderMarginPx: 700,
    maxConcurrentRenders: lowCoreDevice ? 1 : 2,
    maxRenderDpr: 2,
  }
}
