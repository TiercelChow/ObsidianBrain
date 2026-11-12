const palette = {
  text: '--text-primary',
  secondary: '--text-secondary',
  accent: '--accent',
  'accent-border': '--accent-border',
  border: '--border-subtle',
  hover: '--bg-hover',
  'code-bg': '--code-bg',
  'code-inline': '--code-inline-color',
  'code-block': '--code-block-bg',
  'code-text': '--code-block-text',
  mark: '--accent-light',
  selection: '--accent-border',
  font: '--font-sans',
} as const

const artwork = "svg, img, picture, video, canvas, [role='img']"
const paletteProperty = /^(?:color|color-scheme|background-(?:color|image)|border(?:-[\w-]+)?-color|box-shadow|text-shadow|-webkit-text-fill-color)$/

/** Inline important paint otherwise wins over even the first cascade layer. */
export function normalizeEpubInlinePaint(doc: Document) {
  for (const element of doc.querySelectorAll<HTMLElement>('[style]')) {
    if (element.closest(artwork)) continue
    const style = element.style
    const properties = Array.from(style).filter(
      (name) => paletteProperty.test(name) && style.getPropertyPriority(name),
    )
    for (const name of properties) style.setProperty(name, style.getPropertyValue(name))
  }
}

export function syncEpubPalette(doc: Document, system: CSSStyleDeclaration) {
  const target = doc.documentElement.style
  target.setProperty('--epub-scheme', system.colorScheme, 'important')
  for (const [name, source] of Object.entries(palette)) {
    target.setProperty(`--epub-${name}`, system.getPropertyValue(source).trim(), 'important')
  }
}
