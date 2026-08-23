export interface SpyHeader {
  date: string
  /** Header top offset in px, relative to the scroll container's top edge. */
  top: number
}

/**
 * Pick the date whose group header is the last one to have crossed the
 * threshold (distance from the scroll container's top). Headers must be
 * ordered top-to-bottom as rendered; scanning stops at the first header
 * below the fold. Falls back to the first header when none has crossed
 * (scrolled to the very top); returns null for an empty list.
 */
export function pickActiveDate(headers: SpyHeader[], threshold: number): string | null {
  if (headers.length === 0) return null
  let active: string | null = null
  for (const header of headers) {
    if (header.top <= threshold) active = header.date
    else break
  }
  return active ?? headers[0].date
}
