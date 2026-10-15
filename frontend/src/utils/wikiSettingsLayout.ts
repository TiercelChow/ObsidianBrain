/** Leave the shared select anchored to its field, limiting only its scroll area. */
export function settingsSelectMaxHeight(
  trigger: { top: number; bottom: number },
  viewportTop: number,
  viewportHeight: number,
  placement: string,
): number {
  const available = placement.startsWith('top')
    ? trigger.top - viewportTop
    : viewportTop + viewportHeight - trigger.bottom
  // 8px viewport margin and the popper's two 1px edges.
  return Math.max(0, Math.min(320, available - 10))
}
