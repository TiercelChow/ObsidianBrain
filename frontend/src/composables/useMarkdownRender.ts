import 'katex/dist/katex.min.css'
import { onScopeDispose, watch } from 'vue'
import type { MarkdownRenderOptions, RenderedMarkdown } from '@/markdown/renderMarkdown'
import { useAppStore } from '@/stores/app'
import { getHorizontalOverflowPosition } from '@/utils/readerOverflow'

/** Optional per-render hooks that map local image references to servable URLs
 *  (the Reader wires them to /v1/reader/raw). Omitted → hrefs pass through. */
export type MarkdownImageResolvers = Pick<
  MarkdownRenderOptions,
  'resolveEmbed' | 'resolveImage' | 'resourceContext'
>

interface MarkdownWorkerResponse {
  id: number
  result: RenderedMarkdown
}

interface PendingMarkdownRender {
  resolve: (html: string) => void
  reject: (reason: unknown) => void
}

// ── lazy enhancement dependencies ─────────────────────────────────────

type AppTheme = 'light' | 'dark' | 'eye-care'
type MermaidApi = typeof import('mermaid')['default']
type HighlightApi = typeof import('highlight.js/lib/common')['default']

let mermaidPromise: Promise<MermaidApi> | null = null
let highlightPromise: Promise<HighlightApi> | null = null
let mermaidQueue = Promise.resolve()

async function getMermaid(theme: AppTheme): Promise<MermaidApi> {
  mermaidPromise ??= import('mermaid').then((module) => module.default)
  const mermaid = await mermaidPromise
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    forceLegacyMathML: true,
    suppressErrorRendering: true,
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
 * Markdown rendering pipeline: unified/remark/rehype + highlight.js + mermaid.
 * Re-renders mermaid diagrams when the app theme changes.
 *
 * @param onMermaidClick called when a rendered mermaid diagram is clicked,
 *   receiving the SVG element and the original diagram source.
 * @param onLinkClick called when a relative (in-vault) link is clicked,
 *   receiving the raw href (may include a `#anchor`). External links open in
 *   a new tab; `#anchor` links scroll within the document — neither calls this.
 * @param onImageClick called when a rendered image is clicked, receiving the
 *   resolved `src` and the alt text.
 * @param onAnchorJump called when an in-document `#anchor` link is clicked,
 *   receiving the decoded anchor id. When omitted the default smooth
 *   `scrollIntoView` runs; the Reader passes its own handler so anchor jumps
 *   share its programmatic-scroll behavior.
 */
export function useMarkdownRender(
  onMermaidClick: (svg: SVGElement, source: string) => void,
  onLinkClick?: (href: string) => void,
  onImageClick?: (src: string, alt: string) => void,
  onAnchorJump?: (id: string) => void,
) {
  const appStore = useAppStore()
  let currentContainer: HTMLElement | null = null
  let enhancementObserver: IntersectionObserver | null = null
  let tableResizeObserver: ResizeObserver | null = null
  let enhancementGeneration = 0
  let markdownWorker: Worker | null = null
  let markdownRequestId = 0
  const pendingMarkdown = new Map<number, PendingMarkdownRender>()

  function rejectPendingMarkdown(reason: unknown) {
    pendingMarkdown.forEach((pending) => pending.reject(reason))
    pendingMarkdown.clear()
  }

  function stopMarkdownWorker(reason?: unknown) {
    markdownWorker?.terminate()
    markdownWorker = null
    if (pendingMarkdown.size) {
      rejectPendingMarkdown(reason ?? new DOMException('Markdown render cancelled', 'AbortError'))
    }
  }

  function getMarkdownWorker(): Worker | null {
    if (typeof Worker === 'undefined') return null
    if (markdownWorker) return markdownWorker
    const worker = new Worker(new URL('../workers/markdown.worker.ts', import.meta.url), { type: 'module' })
    worker.addEventListener('message', (event: MessageEvent<MarkdownWorkerResponse>) => {
      const pending = pendingMarkdown.get(event.data.id)
      if (!pending) return
      pendingMarkdown.delete(event.data.id)
      pending.resolve(event.data.result.html)
    })
    worker.addEventListener('error', (event) => {
      stopMarkdownWorker(event.error ?? new Error(event.message || 'Markdown Worker failed'))
    })
    markdownWorker = worker
    return worker
  }

  function cleanup(container?: HTMLElement) {
    if (container && currentContainer !== container) return
    enhancementGeneration += 1
    enhancementObserver?.disconnect()
    enhancementObserver = null
    tableResizeObserver?.disconnect()
    tableResizeObserver = null
    if (currentContainer) {
      currentContainer.removeEventListener('click', onContainerClick)
      currentContainer.removeEventListener('scroll', onContainerScroll, true)
    }
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

  onScopeDispose(() => {
    cleanup()
    stopMarkdownWorker()
  })

  async function renderMarkdown(
    src: string,
    resolvers?: MarkdownImageResolvers,
    documentKey?: string,
  ): Promise<string> {
    const hasNonSerializableResolvers = Boolean(
      (resolvers?.resolveEmbed || resolvers?.resolveImage) && !resolvers?.resourceContext,
    )
    if (hasNonSerializableResolvers) {
      const { renderMarkdownDocument } = await import('@/markdown/renderMarkdown')
      return renderMarkdownDocument(src, { ...resolvers, documentKey }).html
    }
    const worker = getMarkdownWorker()
    if (!worker) {
      const { renderMarkdownDocument } = await import('@/markdown/renderMarkdown')
      return renderMarkdownDocument(src, { ...resolvers, documentKey }).html
    }

    if (pendingMarkdown.size) {
      stopMarkdownWorker(new DOMException('Markdown render superseded', 'AbortError'))
      return renderMarkdown(src, resolvers, documentKey)
    }

    const id = ++markdownRequestId
    return new Promise<string>((resolve, reject) => {
      pendingMarkdown.set(id, { resolve, reject })
      worker.postMessage({ id, source: src, documentKey, resourceContext: resolvers?.resourceContext })
    })
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
        if (onAnchorJump) onAnchorJump(id)
        else {
          const escaped = typeof CSS === 'undefined' ? id : CSS.escape(id)
          container.querySelector<HTMLElement>(`#${escaped}, [data-anchor="${escaped}"]`)
            ?.scrollIntoView({ behavior: 'smooth', block: 'start' })
        }
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

  function updateOverflowPosition(element: HTMLElement) {
    const position = getHorizontalOverflowPosition(
      element.scrollLeft,
      element.clientWidth,
      element.scrollWidth,
    )
    element.dataset.overflowPosition = position
    element.classList.toggle('is-overflowing', position !== 'none')
  }

  function onContainerScroll(event: Event) {
    const target = event.target
    if (!(target instanceof HTMLElement)) return
    if (!target.matches('.table-scroll, .code-content, .katex-display, .mermaid')) return
    updateOverflowPosition(target)
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
    container.addEventListener('scroll', onContainerScroll, true)

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
      container.querySelectorAll<HTMLElement>('.table-scroll, .code-content, .katex-display, .mermaid').forEach((wrapper) => {
        updateOverflowPosition(wrapper)
        if (wrapper.dataset.overflowPosition !== 'none') {
          wrapper.tabIndex = 0
          wrapper.setAttribute('role', 'region')
          if (!wrapper.getAttribute('aria-label')) {
            const label = wrapper.classList.contains('table-scroll')
              ? '可横向滚动的表格'
              : wrapper.classList.contains('code-content')
                ? '可横向滚动的代码'
                : wrapper.classList.contains('mermaid')
                  ? '可横向滚动的图表'
                  : '可横向滚动的公式'
            wrapper.setAttribute('aria-label', label)
          }
        } else {
          wrapper.removeAttribute('tabindex')
          if (!wrapper.classList.contains('table-scroll')) wrapper.removeAttribute('role')
        }
      })
    }

    // Only overflowed tables join the keyboard tab order. Recalculate after
    // layout and whenever sidebars, the preview modal, or the viewport resize.
    requestAnimationFrame(updateTableOverflow)
    if (typeof ResizeObserver !== 'undefined') {
      tableResizeObserver = new ResizeObserver(updateTableOverflow)
      container.querySelectorAll<HTMLElement>('.table-scroll, .code-content, .katex-display, .mermaid').forEach((wrapper) => {
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
