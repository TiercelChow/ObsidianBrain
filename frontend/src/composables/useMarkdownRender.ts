import { Marked, type Tokens } from 'marked'
import markedKatex from 'marked-katex-extension'
import 'katex/dist/katex.min.css'
import { onScopeDispose, watch } from 'vue'
import { useAppStore } from '@/stores/app'
import { convertObsidianImageEmbeds, isLocalHref } from '@/utils/markdownImages'

// ── helpers ────────────────────────────────────────────────────────────

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/** Strip a leading YAML frontmatter block (`---\n...\n---`). */
function stripFrontmatter(md: string): string {
  return md.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, '')
}

function stripHtml(html: string): string {
  return html.replace(/<[^>]+>/g, '')
}

/** Slugify heading text — keeps letters (incl. CJK), numbers, hyphens. */
function slugify(text: string): string {
  const slug = text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
  return slug || 'heading'
}

const usedIds = new Set<string>()
function uniqueSlug(text: string): string {
  const base = slugify(text)
  let slug = base
  let i = 1
  while (usedIds.has(slug)) {
    slug = `${base}-${i++}`
  }
  usedIds.add(slug)
  return slug
}

// ── marked instance (configured once) ──────────────────────────────────

/** Optional per-render hooks that map local image references to servable URLs
 *  (the Reader wires them to /v1/reader/raw). Omitted → hrefs pass through. */
export interface MarkdownImageResolvers {
  /** Obsidian wiki embed target (`![[a.jpg]]`) → URL; null leaves the embed as text. */
  resolveEmbed?: (target: string) => string | null
  /** Standard markdown image href → URL; null keeps the href unchanged. */
  resolveImage?: (href: string) => string | null
}

/** Per-call image resolver (see renderMarkdown's options). Set only for the
 *  duration of the synchronous md.parse() below, which makes a module-level
 *  slot safe: no other render can interleave. */
let activeImageResolver: ((href: string) => string | null) | null = null

const renderer = {
  image({ href, title, text }: Tokens.Image): string {
    // Local filesystem hrefs get rewritten to a servable URL by the resolver
    // (e.g. the Reader's /v1/reader/raw endpoint); everything else passes through.
    const src = (isLocalHref(href) && activeImageResolver?.(href)) || href
    const titleAttr = title ? ` title="${escapeHtml(title)}"` : ''
    return `<img src="${escapeHtml(src)}" alt="${escapeHtml(text)}"${titleAttr}>`
  },
  code({ text, lang }: Tokens.Code): string {
    const language = (lang || '').trim().split(/\s+/)[0].toLowerCase()
    if (language === 'mermaid') {
      // Convert LaTeX subscripts/superscripts to Unicode characters, but ONLY
      // inside quoted node labels ("...") — not in mermaid keywords/syntax.
      // mermaid renders to SVG, so HTML <sub>/<sup> tags don't work;
      // Unicode subscript/superscript chars work in SVG text elements.
      const subMap: Record<string, string> = { '0':'₀','1':'₁','2':'₂','3':'₃','4':'₄','5':'₅','6':'₆','7':'₇','8':'₈','9':'₉','a':'ₐ','e':'ₑ','o':'ₒ','x':'ₓ','h':'ₕ','k':'ₖ','l':'ₗ','m':'ₘ','n':'ₙ','p':'ₚ','s':'ₛ','t':'ₜ','i':'ᵢ','j':'ⱼ','r':'ᵣ','u':'ᵤ','v':'ᵥ' }
      const supMap: Record<string, string> = { '0':'⁰','1':'¹','2':'²','3':'³','4':'⁴','5':'⁵','6':'⁶','7':'⁷','8':'⁸','9':'⁹','a':'ᵃ','b':'ᵇ','c':'ᶜ','d':'ᵈ','e':'ᵉ','f':'ᶠ','g':'ᵍ','h':'ʰ','i':'ⁱ','j':'ʲ','k':'ᵏ','l':'ˡ','m':'ᵐ','n':'ⁿ','o':'ᵒ','p':'ᵖ','r':'ʳ','s':'ˢ','t':'ᵗ','u':'ᵘ','v':'ᵛ','w':'ʷ','x':'ˣ','y':'ʸ','z':'ᶻ','+':'⁺','-':'⁻','=':'⁼','(':'⁽',')':'⁾' }
      const toSub = (s: string) => s.split('').map(c => subMap[c.toLowerCase()] || c).join('')
      const toSup = (s: string) => s.split('').map(c => supMap[c.toLowerCase()] || c).join('')
      // Only convert inside quoted labels: "h_t" → "hₜ"
      const processed = text.replace(/"([^"]*)"/g, (_match: string, inner: string) => {
        const converted = inner
          .replace(/_\{([^}]+)\}/g, (_: string, g: string) => toSub(g))
          .replace(/_([a-zA-Z0-9])/g, (_: string, c: string) => toSub(c))
          .replace(/\^\{([^}]+)\}/g, (_: string, g: string) => toSup(g))
          .replace(/\^([a-zA-Z0-9])/g, (_: string, c: string) => toSup(c))
        return `"${converted}"`
      })
      const escaped = escapeHtml(processed)
      return `<div class="mermaid" data-raw="${escaped}">${escaped}</div>`
    }
    const cls = language ? `hljs language-${language}` : 'hljs'
    const gutter = text.split('\n').map((_, i) => i + 1).join('\n')
    return `<div class="code-block"><pre class="code-gutter">${gutter}</pre><pre class="code-content"><code class="${cls}">${escapeHtml(text)}</code></pre></div>`
  },
  heading({ tokens, depth }: Tokens.Heading): string {
    // `this` is the marked Renderer instance at runtime; marked injects `.parser`.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const inner = (this as any).parser.parseInline(tokens) as string
    const id = uniqueSlug(stripHtml(inner))
    return `<h${depth} id="${id}">${inner}</h${depth}>\n`
  },
  table(token: Tokens.Table): string {
    // A table cannot reliably scroll itself. Keep the semantic table intact and
    // place it inside a dedicated region so wide columns remain reachable.
    // The enhancement pass below also applies this wrapper to raw HTML tables.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const parser = (this as any).parser
    const renderCell = (cell: Tokens.TableCell) => {
      const tag = cell.header ? 'th' : 'td'
      const align = cell.align && ['left', 'center', 'right'].includes(cell.align)
        ? ` style="text-align:${cell.align}"`
        : ''
      return `<${tag}${align}>${parser.parseInline(cell.tokens)}</${tag}>`
    }
    const renderRow = (cells: Tokens.TableCell[]) => `<tr>${cells.map(renderCell).join('')}</tr>`
    const head = renderRow(token.header)
    const body = token.rows.length
      ? `<tbody>${token.rows.map(renderRow).join('')}</tbody>`
      : ''
    return `<div class="table-scroll" role="region" aria-label="表格"><table><thead>${head}</thead>${body}</table></div>\n`
  },
}

const md = new Marked({ gfm: true, breaks: false, renderer })
// LaTeX math via KaTeX: $...$ inline, $$...$$ block.
// \[...\], \(...\), and bare [math] are pre-processed to $$/$$ in renderMarkdown().
md.use(markedKatex({ throwOnError: false }))

// ── lazy enhancement dependencies ─────────────────────────────────────

type AppTheme = 'light' | 'dark' | 'eye-care'
type MermaidApi = typeof import('mermaid')['default']
type HighlightApi = typeof import('highlight.js/lib/common')['default']

let mermaidPromise: Promise<MermaidApi> | null = null
let highlightPromise: Promise<HighlightApi> | null = null

async function getMermaid(theme: AppTheme): Promise<MermaidApi> {
  mermaidPromise ??= import('mermaid').then((module) => module.default)
  const mermaid = await mermaidPromise
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'loose',
    theme: theme === 'dark' ? 'dark' : 'default',
    fontFamily: 'inherit',
    flowchart: { useMaxWidth: true, htmlLabels: true },
    sequence: { useMaxWidth: true },
    gantt: { useMaxWidth: true },
    journey: { useMaxWidth: true },
  })
  return mermaid
}

async function getHighlighter(): Promise<HighlightApi> {
  highlightPromise ??= import('highlight.js/lib/common').then((module) => module.default)
  return highlightPromise
}

/** Re-render every mermaid block in a container (used on theme change). */
async function rerenderMermaid(container: HTMLElement, theme: AppTheme) {
  const els = Array.from(container.querySelectorAll<HTMLElement>('.mermaid[data-processed]'))
  if (!els.length) return
  for (const el of els) {
    const raw = el.getAttribute('data-raw') || ''
    el.removeAttribute('data-processed')
    el.textContent = raw // clears old SVG; mermaid reads textContent
  }
  try {
    const mermaid = await getMermaid(theme)
    if (!container.isConnected) return
    await mermaid.run({ nodes: els, suppressErrors: true })
  } catch (e) {
    console.error('mermaid 重新渲染失败:', e)
  }
}

// ── composable ─────────────────────────────────────────────────────────

/**
 * Markdown rendering pipeline: marked + highlight.js + mermaid.
 * Re-renders mermaid diagrams when the app theme changes.
 *
 * @param onMermaidClick called when a rendered mermaid diagram is clicked,
 *   receiving the SVG element and the original diagram source.
 * @param onLinkClick called when a relative (in-vault) link is clicked,
 *   receiving the raw href (may include a `#anchor`). External links open in
 *   a new tab; `#anchor` links scroll within the document — neither calls this.
 */
export function useMarkdownRender(
  onMermaidClick: (svg: SVGElement, source: string) => void,
  onLinkClick?: (href: string) => void,
  onImageClick?: (src: string, alt: string) => void,
) {
  const appStore = useAppStore()
  let currentContainer: HTMLElement | null = null
  let enhancementObserver: IntersectionObserver | null = null
  let tableResizeObserver: ResizeObserver | null = null
  let enhancementGeneration = 0
  let mermaidQueue = Promise.resolve()

  function cleanup(container?: HTMLElement) {
    if (container && currentContainer !== container) return
    enhancementGeneration += 1
    enhancementObserver?.disconnect()
    enhancementObserver = null
    tableResizeObserver?.disconnect()
    tableResizeObserver = null
    if (currentContainer) currentContainer.removeEventListener('click', onContainerClick)
    currentContainer = null
  }

  watch(
    () => appStore.theme,
    (t) => {
      const container = currentContainer
      if (!container) return
      mermaidQueue = mermaidQueue.then(() => rerenderMermaid(container, t))
    },
  )

  onScopeDispose(cleanup)

  function renderMarkdown(src: string, resolvers?: MarkdownImageResolvers): string {
    usedIds.clear()
    let text = stripFrontmatter(src)

    // Protect fenced code blocks and inline code from math preprocessing.
    const codeBlocks: string[] = []
    text = text.replace(/```[\s\S]*?```/g, (block) => {
      codeBlocks.push(block)
      return `\x00CB${codeBlocks.length - 1}\x00`
    })
    text = text.replace(/`[^`\n]+`/g, (block) => {
      codeBlocks.push(block)
      return `\x00CB${codeBlocks.length - 1}\x00`
    })

    // Convert LaTeX delimiters to $...$ / $$...$$ (marked-katex-extension v5 only supports these).
    // \[...\] → $$...$$ (display, collapse multi-line to single line)
    // \(...\) → $...$ (inline, with spaces to satisfy KaTeX's whitespace requirement)
    text = text.replace(/\\\[([\s\S]+?)\\\]/g, (_, inner) => {
      const collapsed = inner.trim().replace(/\n\s*/g, ' ')
      return `\n\n$$${collapsed}$$\n\n`
    })
    text = text.replace(/\\\(([\s\S]+?)\\\)/g, (_, inner) => {
      const collapsed = inner.trim().replace(/\n\s*/g, ' ')
      return ` $${collapsed}$ `
    })

    // Obsidian wiki image embeds → markdown images, while code blocks are stashed
    // so embed syntax inside code samples is never rewritten.
    if (resolvers?.resolveEmbed) {
      text = convertObsidianImageEmbeds(text, resolvers.resolveEmbed)
    }

    // Restore code blocks.
    text = text.replace(/\x00CB(\d+)\x00/g, (_, i) => codeBlocks[parseInt(i, 10)])

    // The image renderer reads this slot during the synchronous md.parse()
    // below; clear it again so later resolver-less renders pass hrefs through.
    activeImageResolver = resolvers?.resolveImage ?? null
    try {
      return md.parse(text) as string
    } finally {
      activeImageResolver = null
    }
  }

  function onContainerClick(event: Event) {
    const container = currentContainer
    const target = event.target instanceof Element ? event.target : null
    if (!container || !target || !container.contains(target)) return

    const mermaidEl = target.closest<HTMLElement>('.mermaid-clickable')
    if (mermaidEl && container.contains(mermaidEl)) {
      const svg = mermaidEl.querySelector('svg')
      if (svg) onMermaidClick(svg, mermaidEl.getAttribute('data-raw') || '')
      return
    }

    const anchor = target.closest<HTMLAnchorElement>('a[href]')
    if (anchor && container.contains(anchor)) {
      const href = anchor.getAttribute('href') || ''
      if (!href || /^(https?:|mailto:|ftp:|tel:)/i.test(href)) return
      event.preventDefault()
      if (href.startsWith('#')) {
        const id = decodeURIComponent(href.slice(1))
        document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      } else {
        onLinkClick?.(href)
      }
      return
    }

    const image = target.closest<HTMLImageElement>('img')
    if (image && container.contains(image) && onImageClick) {
      onImageClick(image.src, image.alt || '')
    }
  }

  async function highlightCode(el: HTMLElement, generation: number) {
    try {
      const hljs = await getHighlighter()
      if (generation !== enhancementGeneration || !el.isConnected) return
      hljs.highlightElement(el)
    } catch (e) {
      console.warn('hljs 高亮失败:', e)
    }
  }

  async function renderMermaid(el: HTMLElement, generation: number) {
    try {
      const mermaid = await getMermaid(appStore.theme)
      if (generation !== enhancementGeneration || !el.isConnected) return
      await mermaid.run({ nodes: [el], suppressErrors: true })
      if (generation !== enhancementGeneration || !el.isConnected) return
      if (el.querySelector('svg')) el.classList.add('mermaid-clickable')
      else el.classList.add('mermaid-error')
    } catch (e) {
      if (generation === enhancementGeneration) console.error('mermaid 渲染失败:', e)
    }
  }

  function observeEnhancements(container: HTMLElement, generation: number) {
    const codeEls = Array.from(container.querySelectorAll<HTMLElement>('pre code'))
    const mermaidEls = Array.from(container.querySelectorAll<HTMLElement>('.mermaid:not([data-processed])'))
    const targets = [...codeEls, ...mermaidEls]
    if (!targets.length) return

    if (typeof IntersectionObserver === 'undefined') {
      codeEls.forEach((el) => { void highlightCode(el, generation) })
      mermaidEls.forEach((el) => {
        mermaidQueue = mermaidQueue.then(() => renderMermaid(el, generation))
      })
      return
    }

    enhancementObserver = new IntersectionObserver(
      (entries, observer) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue
          observer.unobserve(entry.target)
          const el = entry.target as HTMLElement
          if (el.classList.contains('mermaid')) {
            mermaidQueue = mermaidQueue.then(() => renderMermaid(el, generation))
          } else {
            void highlightCode(el, generation)
          }
        }
      },
      { root: container.parentElement, rootMargin: '700px 0px' },
    )
    targets.forEach((el) => enhancementObserver?.observe(el))
  }

  async function enhance(container: HTMLElement) {
    cleanup()
    currentContainer = container
    const generation = enhancementGeneration
    container.addEventListener('click', onContainerClick)

    // GFM tables already use the renderer wrapper. Raw HTML tables need the
    // same containment so an explicit width cannot escape the reading pane.
    container.querySelectorAll<HTMLTableElement>('table').forEach((table) => {
      if (table.parentElement?.classList.contains('table-scroll')) return
      const wrapper = document.createElement('div')
      wrapper.className = 'table-scroll'
      table.before(wrapper)
      wrapper.append(table)
    })

    const updateTableOverflow = () => {
      if (generation !== enhancementGeneration || !container.isConnected) return
      container.querySelectorAll<HTMLElement>('.table-scroll').forEach((wrapper) => {
        const overflowing = wrapper.scrollWidth > wrapper.clientWidth + 1
        wrapper.classList.toggle('is-overflowing', overflowing)
        if (overflowing) {
          wrapper.tabIndex = 0
          wrapper.setAttribute('role', 'region')
          wrapper.setAttribute('aria-label', '可横向滚动的表格')
        } else {
          wrapper.removeAttribute('tabindex')
          wrapper.removeAttribute('role')
          wrapper.removeAttribute('aria-label')
        }
      })
    }

    // Only overflowed tables join the keyboard tab order. Recalculate after
    // layout and whenever sidebars, the preview modal, or the viewport resize.
    requestAnimationFrame(updateTableOverflow)
    if (typeof ResizeObserver !== 'undefined') {
      tableResizeObserver = new ResizeObserver(updateTableOverflow)
      container.querySelectorAll<HTMLElement>('.table-scroll').forEach((wrapper) => {
        tableResizeObserver?.observe(wrapper)
      })
    }

    // External link attributes are static; all click handling is delegated to
    // one container listener instead of one closure per link/image/diagram.
    container.querySelectorAll<HTMLAnchorElement>('a[href]').forEach((a) => {
      const href = a.getAttribute('href') || ''
      if (/^(https?:|mailto:|ftp:|tel:)/i.test(href)) {
        a.target = '_blank'
        a.rel = 'noopener noreferrer'
      }
    })
    if (onImageClick) {
      container.querySelectorAll<HTMLImageElement>('img').forEach((img) => {
        img.style.cursor = 'zoom-in'
      })
    }

    observeEnhancements(container, generation)
  }

  return { renderMarkdown, enhance, cleanup }
}
