export type HorizontalOverflowPosition = 'none' | 'start' | 'middle' | 'end'

/**
 * Describe which horizontal edges still contain hidden content. A one-pixel
 * tolerance avoids flickering between states on fractional mobile layouts.
 */
export function getHorizontalOverflowPosition(
  scrollLeft: number,
  clientWidth: number,
  scrollWidth: number,
): HorizontalOverflowPosition {
  const maxScroll = Math.max(0, scrollWidth - clientWidth)
  if (maxScroll <= 1) return 'none'
  if (scrollLeft <= 1) return 'start'
  if (scrollLeft >= maxScroll - 1) return 'end'
  return 'middle'
}
