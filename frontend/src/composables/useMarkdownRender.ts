import { Marked, type Tokens } from 'marked'
import mermaid from 'mermaid'
import hljs from 'highlight.js'
import markedKatex from 'marked-katex-extension'
import 'katex/dist/katex.min.css'
import { watch } from 'vue'
import { useAppStore } from '@/stores/app'

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

const renderer = {
  code({ text, lang }: Tokens.Code): string {
    const language = (lang || '').trim().split(/\s+/)[0].toLowerCase()
    if (language === 'mermaid') {
      const escaped = escapeHtml(text)
      // text content lets mermaid.run read it; data-raw preserves source for re-render.
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
}

const md = new Marked({ gfm: true, breaks: false, renderer })
// LaTeX math via KaTeX: $...$ inline, $$...$$ block.
// \[...\], \(...\), and bare [math] are pre-processed to $$/$$ in renderMarkdown().
md.use(markedKatex({ throwOnError: false }))

// ── mermaid ────────────────────────────────────────────────────────────

function initMermaid(theme: 'light' | 'dark' | 'eye-care') {
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
}

/** Re-render every mermaid block in a container (used on theme change). */
async function rerenderMermaid(container: HTMLElement) {
  const els = Array.from(container.querySelectorAll<HTMLElement>('.mermaid'))
  if (!els.length) return
  for (const el of els) {
    const raw = el.getAttribute('data-raw') || ''
    el.removeAttribute('data-processed')
    el.textContent = raw // clears old SVG; mermaid reads textContent
  }
  try {
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

  initMermaid(appStore.theme)

  watch(
    () => appStore.theme,
    (t) => {
      initMermaid(t)
      if (currentContainer) void rerenderMermaid(currentContainer)
    },
  )

  function renderMarkdown(src: string): string {
    usedIds.clear()
    let text = stripFrontmatter(src)

    // Protect fenced code blocks (including mermaid) from math preprocessing.
    // marked's tokenizer hasn't run yet, so we manually extract ```...``` blocks.
    const codeBlocks: string[] = []
    text = text.replace(/```[\s\S]*?```/g, (block) => {
      codeBlocks.push(block)
      return `\x00CB${codeBlocks.length - 1}\x00`
    })

    // Convert LaTeX delimiters to $...$ / $$...$$ (the only ones marked-katex-extension v5 supports).
    // \[...\] → $$...$$ (display), \(...\) → $...$ (inline)
    // Note: marked-katex-extension v5 does NOT support newlines inside $$...$$,
    // so we collapse multi-line formulas to single line (joining with space).
    const mathBlocks: string[] = []
    text = text.replace(/\\\[([\s\S]+?)\\\]/g, (_, inner) => {
      const collapsed = inner.trim().replace(/\n\s*/g, ' ')
      mathBlocks.push(`$$${collapsed}$$`)
      return `\x00MATH${mathBlocks.length - 1}\x00`
    })
    text = text.replace(/\\\(([\s\S]+?)\\\)/g, (_, inner) => {
      const collapsed = inner.trim().replace(/\n\s*/g, ' ')
      mathBlocks.push(`$${collapsed}$`)
      return `\x00MATH${mathBlocks.length - 1}\x00`
    })

    // Convert bare [math] → $$...$$, but only when it looks like a formula
    // and is NOT a Markdown link ([text](url)) or image (![alt](url)).
    text = text.replace(/(?<![!\]])\[([^\[\]\n]+)\](?!\()/g, (match, inner) => {
      const trimmed = inner.trim()
      // Heuristic: math if it contains =, _, ^, {}, \, or letter+operator patterns
      if (/[=_^{}\\]/.test(trimmed) || /[a-zA-Z]\s*[+\-*/=]/.test(trimmed)) {
        return `$$${trimmed}$$`
      }
      return match
    })

    // Restore math blocks (already in $...$ / $$...$$ format).
    text = text.replace(/\x00MATH(\d+)\x00/g, (_, i) => mathBlocks[parseInt(i, 10)])

    // Restore code blocks.
    text = text.replace(/\x00CB(\d+)\x00/g, (_, i) => codeBlocks[parseInt(i, 10)])

    return md.parse(text) as string
  }

  async function enhance(container: HTMLElement) {
    currentContainer = container

    // 1. syntax highlighting
    container.querySelectorAll<HTMLElement>('pre code').forEach((el) => {
      try {
        hljs.highlightElement(el)
      } catch (e) {
        console.warn('hljs 高亮失败:', e)
      }
    })

    // 2. mermaid diagrams
    const mermaidEls = Array.from(container.querySelectorAll<HTMLElement>('.mermaid:not([data-processed])'))
    if (mermaidEls.length) {
      try {
        await mermaid.run({ nodes: mermaidEls, suppressErrors: true })
      } catch (e) {
        console.error('mermaid 渲染失败:', e)
      }
      // 3. bind click → open viewer
      mermaidEls.forEach((el) => {
        const svg = el.querySelector('svg')
        if (svg) {
          el.classList.add('mermaid-clickable')
          el.addEventListener('click', () => {
            onMermaidClick(svg as SVGElement, el.getAttribute('data-raw') || '')
          })
        } else {
          el.classList.add('mermaid-error')
        }
      })
    }

    // 4. intercept links: external → new tab, #anchor → scroll, relative → onLinkClick
    container.querySelectorAll<HTMLAnchorElement>('a[href]').forEach((a) => {
      const href = a.getAttribute('href') || ''
      if (!href) return
      if (/^(https?:|mailto:|ftp:|tel:)/i.test(href)) {
        a.target = '_blank'
        a.rel = 'noopener noreferrer'
        return
      }
      if (href.startsWith('#')) {
        a.addEventListener('click', (e) => {
          const id = decodeURIComponent(href.slice(1))
          const target = document.getElementById(id)
          if (target) {
            e.preventDefault()
            target.scrollIntoView({ behavior: 'smooth', block: 'start' })
          }
        })
        return
      }
      // relative path to another document
      a.addEventListener('click', (e) => {
        e.preventDefault()
        onLinkClick?.(href)
      })
    })

    // 5. images — click to zoom
    if (onImageClick) {
      container.querySelectorAll<HTMLImageElement>('img').forEach((img) => {
        img.style.cursor = 'zoom-in'
        img.addEventListener('click', () => {
          onImageClick(img.src, img.alt || '')
        })
      })
    }
  }

  return { renderMarkdown, enhance }
}
