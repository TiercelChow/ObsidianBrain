import { Marked, type Tokens } from 'marked'
import mermaid from 'mermaid'
import hljs from 'highlight.js'
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
    return `<pre><code class="${cls}">${escapeHtml(text)}</code></pre>`
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

// ── mermaid ────────────────────────────────────────────────────────────

function initMermaid(theme: 'light' | 'dark') {
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'loose',
    theme: theme === 'dark' ? 'dark' : 'default',
    fontFamily: 'inherit',
    // Transparent background so diagrams blend with the glass surface they sit on.
    themeVariables: { background: 'transparent' },
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
 */
export function useMarkdownRender(onMermaidClick: (svg: SVGElement, source: string) => void) {
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
    return md.parse(stripFrontmatter(src)) as string
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
  }

  return { renderMarkdown, enhance }
}
