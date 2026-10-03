/** Preserve the grabbed presentation position, including interrupted springs. */
export function presentationOffset(transform: string, axis: 'x' | 'y', fallback = 0): number {
  if (transform === 'none') return 0
  const match = transform.match(/^matrix(3d)?\(([^)]+)\)$/)
  if (!match) return fallback
  const values = match[2].split(',').map(Number)
  const expected = match[1] ? 16 : 6
  if (values.length !== expected || !values.every(Number.isFinite)) return fallback
  return values[match[1] ? (axis === 'x' ? 12 : 13) : (axis === 'x' ? 4 : 5)]
}

export function panelDragPosition(offset: number, pointer: number, origin: number, dimension: number, closingSign = 1): number {
  const next = offset + pointer - origin
  if (next * closingSign >= 0) return next
  const extent = Math.max(1, dimension)
  return (next * extent * .35) / (extent + .35 * Math.abs(next))
}
