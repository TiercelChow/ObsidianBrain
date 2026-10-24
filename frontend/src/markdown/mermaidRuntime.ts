import type { MermaidConfig } from 'mermaid'
import { buildMermaidConfig, type DiagramTokens } from './mermaidTheme.ts'

export type DiagramTheme = 'light' | 'dark' | 'eye-care'
export type DiagramProfile = 'optimized' | 'original'
type MermaidApi = typeof import('mermaid')['default']
let engine: Promise<MermaidApi> | undefined
let queue: Promise<unknown> = Promise.resolve()
let layoutRegistered = false
let renderId = 0

export function readDiagramTokens(): DiagramTokens {
  const css = getComputedStyle(document.documentElement)
  const token = (name: string) => css.getPropertyValue(name).trim()
  return {
    background: token('--bg-base'), foreground: token('--text-primary'),
    muted: token('--text-muted'), accent: token('--accent'), fontFamily: token('--font-sans'),
  }
}

async function waitForDiagramFonts(source: string, fontFamily: string) {
  if (!document.fonts) return
  await document.fonts.load(`16px ${fontFamily}`, '阅境轩 Knowledge 0123456789')
  if (source.includes('$$')) {
    await Promise.all(['KaTeX_Main', 'KaTeX_Math', 'KaTeX_Size1', 'KaTeX_Size2', 'KaTeX_Size3', 'KaTeX_Size4', 'KaTeX_AMS'].map(font => document.fonts.load(`16px ${font}`)))
    await document.fonts.load('italic 16px KaTeX_Math')
  }
  await document.fonts.ready
}

/** One global queue owns both configuration and DOM measurement, across readers and previews. */
export function renderMermaidSvg(source: string, options: {
  theme: DiagramTheme
  profile?: DiagramProfile
  isCurrent?: () => boolean
}): Promise<string | null> {
  const task = queue.then(async () => {
    if (options.isCurrent && !options.isCurrent()) return null
    engine ??= import('mermaid').then(module => module.default).catch(error => { engine = undefined; throw error })
    const mermaid = await engine
    const tokens = readDiagramTokens()
    await waitForDiagramFonts(source, tokens.fontFamily)
    if (options.isCurrent && !options.isCurrent()) return null
    const legacy = options.profile === 'original'
    // Detection also understands Mermaid frontmatter/directives. The official
    // parser, not a source-rewriting regex, chooses the diagram type.
    mermaid.initialize(buildMermaidConfig(tokens))
    const diagramType = mermaid.detectType(source)
    const config: MermaidConfig = legacy ? {
      startOnLoad: false, securityLevel: 'strict', forceLegacyMathML: true,
      suppressErrorRendering: true, theme: options.theme === 'dark' ? 'dark' : 'default',
      fontFamily: 'inherit', flowchart: { useMaxWidth: true, htmlLabels: true },
    } : buildMermaidConfig(tokens, diagramType)
    // Register the tiny loader even when only frontmatter requests ELK. The
    // heavy layout engine is still fetched only if that layout is actually used.
    if (!legacy && !layoutRegistered) {
      const { default: layouts } = await import('@mermaid-js/layout-elk')
      mermaid.registerLayoutLoaders(layouts)
      layoutRegistered = true
    }
    mermaid.initialize(config)
    const host = document.createElement('div')
    host.className = 'mermaid-measure-host'
    Object.assign(host.style, {
      position: 'fixed', left: '-100000px', top: '0', width: '1024px',
      visibility: 'hidden', pointerEvents: 'none', fontFamily: tokens.fontFamily,
      fontSize: '16px', lineHeight: '1.5',
    })
    document.body.append(host)
    try {
      let rendered = await mermaid.render(`reader-diagram-${++renderId}`, source, host)
      // Special KaTeX alphabets/weights may request an additional font only
      // when their first label is mounted. If so, re-measure once after it
      // loads rather than publishing boxes measured with a fallback font.
      if (document.fonts?.status === 'loading') {
        await document.fonts.ready
        if (options.isCurrent && !options.isCurrent()) return null
        host.replaceChildren()
        rendered = await mermaid.render(`reader-diagram-${++renderId}`, source, host)
      }
      const { svg } = rendered
      if (options.isCurrent && !options.isCurrent()) return null
      if (legacy) return svg
      const template = document.createElement('template')
      template.innerHTML = svg
      const root = template.content.querySelector('svg')
      const box = root?.viewBox.baseVal
      if (root && box && box.width > 0 && box.height > 0) {
        root.setAttribute('width', `${box.width}`)
        root.setAttribute('height', `${box.height}`)
        root.style.width = `${box.width}px`
        // Inline presentation fits the document; the unchanged viewBox retains
        // full resolution and natural dimensions for the fullscreen viewer.
        root.style.height = 'auto'
        root.style.maxWidth = '100%'
      }
      // Mermaid 11 puts a hard-coded 9×12px CSS size on class cardinalities
      // AFTER measuring their actual dimensions into SVG width/height attrs.
      // Respect those measured attrs; do not change text or re-layout formulas.
      template.content.querySelectorAll<SVGForeignObjectElement>('.edgeTerminals foreignObject').forEach(label => {
        label.style.removeProperty('width')
        label.style.removeProperty('height')
      })
      return template.innerHTML
    } finally {
      host.remove()
    }
  })
  queue = task.catch(() => undefined)
  return task
}

/** A fullscreen copy must not share marker/clip/style IDs with the original SVG. */
export function cloneDiagramSvg(svg: SVGSVGElement): string {
  const copy = svg.cloneNode(true) as SVGSVGElement
  const prefix = `viewer-${++renderId}-`
  const ids = Array.from(copy.querySelectorAll('[id]'), element => element.id)
  if (copy.id) ids.push(copy.id)
  const references = ids.sort((a, b) => b.length - a.length).map(id => ({
    id, next: `${prefix}${id}`, expression: new RegExp(`#${id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![\\w-])`, 'g'),
  }))
  for (const element of [copy, ...copy.querySelectorAll('*')]) {
    for (const attribute of Array.from(element.attributes)) {
      if (attribute.name === 'id') element.id = `${prefix}${attribute.value}`
      else if (attribute.value.includes('#')) {
        let value = attribute.value
        for (const reference of references) value = value.replace(reference.expression, `#${reference.next}`)
        element.setAttribute(attribute.name, value)
      }
    }
    if (element.tagName.toLowerCase() === 'style') {
      let css = element.textContent || ''
      for (const reference of references) css = css.replace(reference.expression, `#${reference.next}`)
      element.textContent = css
    }
  }
  return copy.outerHTML
}
