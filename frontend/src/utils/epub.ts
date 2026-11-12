const unit = (value: number) =>
  Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0
export function epubPosition(
  index: number,
  ratio: number,
  count: number,
): number {
  return count > 0
    ? unit((Math.max(0, Math.min(count - 1, index)) + unit(ratio)) / count)
    : 0
}
export function epubRestore(
  position: number,
  count: number,
): { index: number; ratio: number } {
  if (count <= 0) return { index: 0, ratio: 0 }
  const value = unit(position) * count,
    index = Math.min(count - 1, Math.floor(value))
  return { index, ratio: unit(value - index) }
}
export function epubResourceUrl(base: string, href: string): string {
  const [path, ...fragment] = href.split('#')
  return (
    base +
    path.split('/').map(encodeURIComponent).join('/') +
    (fragment.length ? '#' + encodeURIComponent(fragment.join('#')) : '')
  )
}
