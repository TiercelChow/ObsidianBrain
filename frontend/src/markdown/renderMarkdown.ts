import type { Element, ElementContent, Root as HastRoot, Text as HastText } from 'hast'
import type { Root as MdastRoot } from 'mdast'
import rehypeKatex from 'rehype-katex'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema, type Options as SanitizeSchema } from 'rehype-sanitize'
import rehypeStringify from 'rehype-stringify'
import remarkFrontmatter from 'remark-frontmatter'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import remarkParse from 'remark-parse'
import remarkRehype from 'remark-rehype'
import { unified } from 'unified'
import { markdownDocumentHash, normalizeHeadingAnchor } from './headingAnchors.ts'

export { normalizeHeadingAnchor } from './headingAnchors.ts'

export interface MarkdownRenderOptions {
  documentKey?: string
  resolveEmbed?: (target: string) => string | null
  resolveImage?: (href: string) => string | null
  resourceContext?: MarkdownResourceContext
}

export interface MarkdownResourceContext {
  noteDir: string
  rootDir: string
}

export interface MarkdownHeading {
  id: string
  anchor: string
  label: string
  depth: number
}

export interface MarkdownDiagnostic {
  kind: 'parse' | 'math'
  message: string
  source?: string
}

export interface RenderedMarkdown {
  html: string
  headings: MarkdownHeading[]
  diagnostics: MarkdownDiagnostic[]
}

interface PositionLike {
  start?: { offset?: number }
  end?: { offset?: number }
}

interface MdastNode {
  type: string
  value?: string
  depth?: number
  url?: string
  alt?: string
  title?: string | null
  position?: PositionLike
  data?: {
    hName?: string
    hProperties?: Record<string, unknown>
    hChildren?: Array<{ type: 'text'; value: string }>
  }
  children?: MdastNode[]
}

const protectedMdastTypes = new Set([
  'code',
  'inlineCode',
  'html',
  'yaml',
  'toml',
  'math',
  'inlineMath',
])

const inlineParentTypes = new Set([
  'paragraph',
  'heading',
  'emphasis',
  'strong',
  'delete',
  'link',
  'tableCell',
])

const imageExtensionPattern = /\.(?:avif|bmp|gif|jpe?g|png|svg|webp)(?:[?#].*)?$/i

function isLocalHref(href: string): boolean {
  return !/^(?:[a-z][a-z\d+.-]*:|\/\/|#|data:|blob:)/i.test(href)
}

function safeDecodeHref(href: string): string {
  try {
    return decodeURIComponent(href)
  } catch {
    return href
  }
}

function resolveRelativePath(baseDir: string, relative: string): string {
  const parts = baseDir ? baseDir.split('/') : []
  for (const segment of relative.replace(/^\.\//, '').split('/')) {
    if (!segment || segment === '.') continue
    if (segment === '..') parts.pop()
    else parts.push(segment)
  }
  return parts.join('/')
}

function localFileUrl(path: string): string {
  return `/v1/reader/raw?path=${encodeURIComponent(path)}`
}

function resolveContextEmbed(target: string, context?: MarkdownResourceContext): string | null {
  if (!context) return null
  return localFileUrl(resolveRelativePath(context.rootDir, target.replace(/^\//, '')))
}

function resolveContextImage(href: string, context?: MarkdownResourceContext): string | null {
  if (!context) return null
  if (href.startsWith('/v1/reader/raw')) return href
  const source = safeDecodeHref(href.split('#')[0])
  return localFileUrl(resolveRelativePath(context.noteDir, source))
}

function walkMdast(node: MdastNode, visitor: (node: MdastNode, parent?: MdastNode) => void, parent?: MdastNode) {
  visitor(node, parent)
  node.children?.forEach((child) => walkMdast(child, visitor, node))
}

function mergeRanges(ranges: Array<[number, number]>): Array<[number, number]> {
  const sorted = ranges
    .filter(([start, end]) => end > start)
    .sort((a, b) => a[0] - b[0])
  const merged: Array<[number, number]> = []
  for (const range of sorted) {
    const previous = merged[merged.length - 1]
    if (previous && range[0] <= previous[1]) previous[1] = Math.max(previous[1], range[1])
    else merged.push([...range])
  }
  return merged
}

function normalizeAlternativeMathInText(text: string): string {
  return text
    .replace(/\\\[([\s\S]*?)\\\]/g, (_match, body: string) => `\n\n$$${body}$$\n\n`)
    .replace(/\\\(([^\r\n]*?)\\\)/g, (_match, body: string) => `$${body}$`)
}

function collectProtectedSourceRanges(source: string): Array<[number, number]> {
  const protectionParser = unified()
    .use(remarkParse)
    .use(remarkFrontmatter, ['yaml', 'toml'])
    .use(remarkGfm)
    .use(remarkMath)
  const tree = protectionParser.parse(source) as MdastRoot
  const ranges: Array<[number, number]> = []
  walkMdast(tree as MdastNode, (node) => {
    if (!protectedMdastTypes.has(node.type)) return
    const start = node.position?.start?.offset
    const end = node.position?.end?.offset
    if (typeof start === 'number' && typeof end === 'number') ranges.push([start, end])
  })
  return mergeRanges(ranges)
}

/** Remove Obsidian comments across paragraphs while preserving offsets/newlines. */
function stripObsidianComments(source: string): string {
  if (!source.includes('%%')) return source
  const ranges = collectProtectedSourceRanges(source)
  let rangeIndex = 0
  let inComment = false
  let index = 0
  let result = ''

  while (index < source.length) {
    const range = ranges[rangeIndex]
    if (range && index === range[0]) {
      const protectedText = source.slice(range[0], range[1])
      result += inComment
        ? protectedText.replace(/[^\r\n]/g, ' ')
        : protectedText
      index = range[1]
      rangeIndex += 1
      continue
    }
    if (source.startsWith('%%', index)) {
      inComment = !inComment
      result += '  '
      index += 2
      continue
    }
    const character = source[index]
    result += inComment && character !== '\n' && character !== '\r' ? ' ' : character
    index += 1
  }
  return result
}

/** Convert MathJax-style delimiters only outside code, HTML and existing math. */
function normalizeAlternativeMath(source: string): string {
  if (!source.includes('\\(') && !source.includes('\\[')) return source
  const ranges = collectProtectedSourceRanges(source)

  let cursor = 0
  let normalized = ''
  for (const [start, end] of ranges) {
    normalized += normalizeAlternativeMathInText(source.slice(cursor, start))
    normalized += source.slice(start, end)
    cursor = end
  }
  normalized += normalizeAlternativeMathInText(source.slice(cursor))
  return normalized
}

function textNode(value: string): MdastNode {
  return { type: 'text', value }
}

function appendNode(nodes: MdastNode[], node: MdastNode) {
  if (node.type === 'text' && !node.value) return
  const previous = nodes[nodes.length - 1]
  if (previous?.type === 'text' && node.type === 'text') previous.value = `${previous.value ?? ''}${node.value ?? ''}`
  else nodes.push(node)
}

function wikiHref(target: string): string {
  const trimmed = target.trim()
  if (trimmed.startsWith('#')) return trimmed
  const match = /^([^#]*)(#.*)?$/.exec(trimmed)
  const path = match?.[1] ?? trimmed
  const anchor = match?.[2] ?? ''
  if (!path || /\.[a-z\d]+$/i.test(path)) return `${path}${anchor}`
  return `${path}.md${anchor}`
}

function parseEmbedSize(alias: string | undefined): { width?: number; height?: number } {
  if (!alias) return {}
  const match = /^(\d+)(?:x(\d+))?$/i.exec(alias.trim())
  if (!match) return {}
  const width = Number(match[1])
  const height = match[2] ? Number(match[2]) : undefined
  return { width, height }
}

function wikiNode(raw: string, options: MarkdownRenderOptions): MdastNode | null {
  const embed = raw.startsWith('!')
  const content = raw.slice(embed ? 3 : 2, -2)
  const separator = content.indexOf('|')
  const target = (separator >= 0 ? content.slice(0, separator) : content).trim()
  const alias = separator >= 0 ? content.slice(separator + 1).trim() : undefined
  if (!target) return null

  if (embed && imageExtensionPattern.test(target)) {
    const src = options.resolveEmbed?.(target)
      ?? resolveContextEmbed(target, options.resourceContext)
      ?? target
    const size = parseEmbedSize(alias)
    return {
      type: 'image',
      url: src,
      alt: target.split('/').slice(-1)[0] ?? target,
      data: {
        hProperties: {
          dataObsidianEmbed: target,
          ...(size.width ? { width: size.width } : {}),
          ...(size.height ? { height: size.height } : {}),
        },
      },
    }
  }

  if (embed) {
    return {
      type: 'link',
      url: wikiHref(target),
      children: [textNode(alias || target)],
      data: {
        hProperties: {
          className: ['internal-link', 'internal-embed'],
          dataWikiTarget: target,
          dataWikiEmbed: true,
        },
      },
    }
  }

  return {
    type: 'link',
    url: wikiHref(target),
    children: [textNode(alias || target)],
    data: {
      hProperties: {
        className: ['internal-link'],
        dataWikiTarget: target,
      },
    },
  }
}

type InlineState = 'normal' | 'highlight' | 'comment'

/** Parse Obsidian inline syntax while retaining Markdown/Math nodes between markers. */
function transformObsidianInline(children: MdastNode[], options: MarkdownRenderOptions): MdastNode[] {
  const output: MdastNode[] = []
  let buffered: MdastNode[] = []
  let state: InlineState = 'normal'

  const destination = () => state === 'highlight' ? buffered : output
  const append = (node: MdastNode) => appendNode(destination(), node)

  for (const child of children) {
    if (child.type !== 'text') {
      if (state !== 'comment') append(child)
      continue
    }

    const value = child.value ?? ''
    let cursor = 0
    while (cursor < value.length) {
      if (state === 'comment') {
        const end = value.indexOf('%%', cursor)
        if (end < 0) {
          cursor = value.length
          continue
        }
        state = 'normal'
        cursor = end + 2
        continue
      }

      const commentAt = value.indexOf('%%', cursor)
      const highlightAt = value.indexOf('==', cursor)
      const embedAt = value.indexOf('![[', cursor)
      const linkAt = value.indexOf('[[', cursor)
      const candidates = [commentAt, highlightAt, embedAt, linkAt].filter((index) => index >= 0)
      if (!candidates.length) {
        append(textNode(value.slice(cursor)))
        cursor = value.length
        continue
      }

      const next = Math.min(...candidates)
      append(textNode(value.slice(cursor, next)))
      if (next === commentAt) {
        if (state === 'highlight') buffered = []
        state = 'comment'
        cursor = next + 2
        continue
      }
      if (next === highlightAt) {
        if (state === 'highlight') {
          output.push({
            type: 'obsidianHighlight',
            children: buffered,
            data: { hName: 'mark' },
          })
          buffered = []
          state = 'normal'
        } else {
          state = 'highlight'
        }
        cursor = next + 2
        continue
      }

      const wikiStart = next === embedAt ? embedAt : linkAt
      const close = value.indexOf(']]', wikiStart + (wikiStart === embedAt ? 3 : 2))
      if (close < 0) {
        append(textNode(value.slice(next)))
        cursor = value.length
        continue
      }
      const raw = value.slice(wikiStart, close + 2)
      append(wikiNode(raw, options) ?? textNode(raw))
      cursor = close + 2
    }
  }

  if (state === 'highlight') {
    appendNode(output, textNode('=='))
    buffered.forEach((node) => appendNode(output, node))
  } else if (state === 'comment') {
    appendNode(output, textNode('%%'))
  }
  return output
}

function splitInlineAtFirstNewline(children: MdastNode[]): [MdastNode[], MdastNode[]] {
  const before: MdastNode[] = []
  const after: MdastNode[] = []
  let split = false
  for (const child of children) {
    if (!split && child.type === 'text') {
      const value = child.value ?? ''
      const index = value.indexOf('\n')
      if (index >= 0) {
        appendNode(before, textNode(value.slice(0, index)))
        appendNode(after, textNode(value.slice(index + 1)))
        split = true
        continue
      }
    }
    appendNode(split ? after : before, child)
  }
  return [before, after]
}

function transformCallout(node: MdastNode): boolean {
  if (node.type !== 'blockquote' || !node.children?.length) return false
  const firstParagraph = node.children[0]
  if (firstParagraph.type !== 'paragraph' || !firstParagraph.children?.length) return false
  const firstText = firstParagraph.children[0]
  if (firstText.type !== 'text') return false
  const match = /^\[!([^\]]+)\]([+-])?[ \t]*/.exec(firstText.value ?? '')
  if (!match) return false

  const type = match[1].trim().toLowerCase().replace(/[^a-z\d_-]/g, '-') || 'note'
  const fold = match[2]
  firstText.value = (firstText.value ?? '').slice(match[0].length)
  const [title, firstBody] = splitInlineAtFirstNewline(firstParagraph.children)
  const titleChildren = title.length ? title : [textNode(match[1].trim().toUpperCase())]
  const bodyChildren: MdastNode[] = []
  if (firstBody.length) bodyChildren.push({ type: 'paragraph', children: firstBody })
  bodyChildren.push(...node.children.slice(1))

  node.type = 'obsidianCallout'
  node.data = {
    hName: fold ? 'details' : 'div',
    hProperties: {
      className: ['callout', `callout-${type}`],
      ...(fold === '+' ? { open: true } : {}),
    },
  }
  node.children = [
    {
      type: 'obsidianCalloutTitle',
      children: titleChildren,
      data: {
        hName: fold ? 'summary' : 'div',
        hProperties: { className: ['callout-title'] },
      },
    },
    {
      type: 'obsidianCalloutContent',
      children: bodyChildren,
      data: {
        hName: 'div',
        hProperties: { className: ['callout-content'] },
      },
    },
  ]
  return true
}

function restoreCurrencyPairs(tree: MdastNode, source: string) {
  walkMdast(tree, (node) => {
    if (node.type !== 'inlineMath' || !/^\d(?:[\d,.]*)(?:\s|[，。,:;])/.test(node.value ?? '')) return
    const start = node.position?.start?.offset
    const end = node.position?.end?.offset
    if (typeof start !== 'number' || typeof end !== 'number' || !/\d/.test(source[end] ?? '')) return
    node.type = 'text'
    node.value = source.slice(start, end)
    delete node.data
  })
}

const legacyInlineMathCommandPattern = /\\(?:alpha|beta|gamma|delta|epsilon|theta|lambda|mu|pi|sigma|phi|omega|frac|sqrt|sum|prod|int|lim|log|sin|cos|tan|mathbf|mathrm|mathcal|text|left|right|cdot|times|leq?|geq?|neq|approx|in|to)\b/
const legacyEscapedScriptPattern = /[\p{L}\p{N})\]}]\\[_^](?:\{[^{}\r\n]+\}|[\p{L}\p{N}])/u
const compactSymbolicEquationPattern = /^[A-Za-z](?:[_^](?:\{[^{}\r\n]+\}|[A-Za-z0-9]))?(?:[=+\-*/<>][A-Za-z0-9.]+(?:[_^](?:\{[^{}\r\n]+\}|[A-Za-z0-9]))?)+$/

/**
 * Some existing notes use backticks as a visual inline-formula wrapper and
 * escape TeX scripts for Markdown, for example `d\_k=d\_v=128`. Keep normal
 * inline code untouched, but promote unambiguous legacy formula shapes to
 * inlineMath so they travel through the same KaTeX pipeline as `$...$`.
 */
function legacyInlineCodeFormula(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed || /[\r\n]/.test(trimmed)) return null

  let formula = trimmed
  let explicitlyDelimited = false
  const dollarDelimited = /^\$(?!\$)([\s\S]+)\$$/.exec(formula)
  const parenDelimited = /^\\\(([\s\S]+)\\\)$/.exec(formula)
  if (dollarDelimited || parenDelimited) {
    formula = (dollarDelimited?.[1] ?? parenDelimited?.[1] ?? '').trim()
    explicitlyDelimited = true
  }

  const hasEscapedScript = legacyEscapedScriptPattern.test(formula)
  const normalized = hasEscapedScript
    ? formula.replace(/\\([_^])(?=\{|[\p{L}\p{N}])/gu, '$1')
    : formula
  const looksMathematical = explicitlyDelimited
    || legacyInlineMathCommandPattern.test(normalized)
    || (hasEscapedScript && /(?:[=+\-*/<>]|[≤≥≈≠∈→])/.test(normalized))
    || compactSymbolicEquationPattern.test(normalized)

  return looksMathematical && normalized ? normalized : null
}

function transformLegacyInlineCodeMath(node: MdastNode) {
  if (node.type !== 'inlineCode') return
  const formula = legacyInlineCodeFormula(node.value ?? '')
  if (!formula) return
  node.type = 'inlineMath'
  node.value = formula
  node.data = {
    hName: 'code',
    hProperties: { className: ['language-math', 'math-inline'] },
    hChildren: [{ type: 'text', value: formula }],
  }
}

function transformBlockReference(node: MdastNode) {
  if (node.type !== 'paragraph' || !node.children?.length) return
  const last = node.children[node.children.length - 1]
  if (last.type !== 'text') return
  const match = /(?:^|\s)\^([a-z\d-]+)\s*$/i.exec(last.value ?? '')
  if (!match) return
  last.value = (last.value ?? '').slice(0, match.index).trimEnd()
  node.data ??= {}
  node.data.hProperties = {
    ...node.data.hProperties,
    dataBlockAnchor: `^${match[1]}`,
  }
}

function transformMdast(tree: MdastNode, options: MarkdownRenderOptions) {
  function visit(node: MdastNode) {
    transformLegacyInlineCodeMath(node)
    node.children?.forEach(visit)
    if (transformCallout(node)) return
    if (node.children && inlineParentTypes.has(node.type) && node.type !== 'link') {
      node.children = transformObsidianInline(node.children, options)
    }
    transformBlockReference(node)
    if (node.type === 'image' && node.url) {
      node.data ??= {}
      const isEmbed = typeof node.data.hProperties?.dataObsidianEmbed === 'string'
      node.data.hProperties = {
        ...node.data.hProperties,
        ...(!isEmbed ? { dataOriginalSrc: node.url } : {}),
        decoding: 'async',
        loading: 'lazy',
      }
    }
  }
  visit(tree)
}

function mdastSemanticLabel(node: MdastNode): string {
  const parts: string[] = []
  function collect(current: MdastNode) {
    if (current.type === 'inlineMath' || current.type === 'math') {
      const value = (current.value ?? '').trim()
      if (value) parts.push(` ${value} `)
      return
    }
    if (current.type === 'image') {
      if (current.alt) parts.push(current.alt)
      return
    }
    if (typeof current.value === 'string') parts.push(current.value)
    current.children?.forEach(collect)
  }
  collect(node)
  return parts.join('').replace(/\s+/g, ' ').trim()
}

function collectHeadings(tree: MdastNode, documentKey: string): MarkdownHeading[] {
  const headings: MarkdownHeading[] = []
  const used = new Map<string, number>()
  walkMdast(tree, (node) => {
    if (node.type !== 'heading' || !node.depth) return
    const label = mdastSemanticLabel(node)
    const base = normalizeHeadingAnchor(label)
    const count = used.get(base) ?? 0
    used.set(base, count + 1)
    const anchor = count ? `${base}-${count}` : base
    headings.push({
      id: `md-${markdownDocumentHash(documentKey)}-${anchor}`,
      anchor,
      label,
      depth: node.depth,
    })
  })
  return headings
}

const sanitizeSchema: SanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), 'mark'],
  attributes: {
    ...defaultSchema.attributes,
    a: [
      ...(defaultSchema.attributes?.a ?? []),
      ['className', 'internal-link', 'internal-embed'],
      'dataWikiTarget',
      'dataWikiEmbed',
    ],
    details: [
      ...(defaultSchema.attributes?.details ?? []),
      ['className', 'callout', /^callout-/],
    ],
    div: [
      ...(defaultSchema.attributes?.div ?? []),
      ['className', 'callout', 'callout-title', 'callout-content', /^callout-/],
    ],
    summary: [
      ...(defaultSchema.attributes?.summary ?? []),
      ['className', 'callout-title'],
    ],
    img: [
      ...(defaultSchema.attributes?.img ?? []),
      'alt',
      'title',
      'width',
      'height',
      'decoding',
      'loading',
      'dataObsidianEmbed',
      'dataOriginalSrc',
    ],
    p: [
      ...(defaultSchema.attributes?.p ?? []),
      'dataBlockAnchor',
    ],
  },
}

function hastText(value: string): HastText {
  return { type: 'text', value }
}

function element(tagName: string, className: string[], children: ElementContent[]): Element {
  return { type: 'element', tagName, properties: { className }, children }
}

function textContent(node: ElementContent): string {
  if (node.type === 'text') return node.value
  if (node.type === 'element') return node.children.map(textContent).join('')
  return ''
}

function languageOf(code: Element): string {
  const classes = Array.isArray(code.properties.className) ? code.properties.className : []
  const languageClass = classes.find((name) => typeof name === 'string' && name.startsWith('language-'))
  return typeof languageClass === 'string' ? languageClass.slice('language-'.length).toLowerCase() : ''
}

function transformHast(
  tree: HastRoot,
  headings: MarkdownHeading[],
  options: MarkdownRenderOptions,
) {
  let headingIndex = 0

  function visitChildren(parent: HastRoot | Element) {
    for (let index = 0; index < parent.children.length; index += 1) {
      const child = parent.children[index]
      if (child.type !== 'element') continue

      if (/^h[1-6]$/.test(child.tagName)) {
        const heading = headings[headingIndex]
        headingIndex += 1
        if (heading) {
          child.properties.id = heading.id
          child.properties.dataAnchor = heading.anchor
          child.properties.dataHeadingLabel = heading.label
        }
      }

      if (child.tagName === 'img') {
        const source = typeof child.properties.src === 'string' ? child.properties.src : ''
        const isEmbed = typeof child.properties.dataObsidianEmbed === 'string'
        if (isEmbed) {
          const target = String(child.properties.dataObsidianEmbed)
          child.properties.src = options.resolveEmbed?.(target)
            ?? resolveContextEmbed(target, options.resourceContext)
            ?? source
        }
        if (!isEmbed && source && isLocalHref(source)) {
          child.properties.src = options.resolveImage?.(source)
            ?? resolveContextImage(source, options.resourceContext)
            ?? source
        }
        child.properties.decoding = 'async'
        child.properties.loading = 'lazy'
      }

      if (child.tagName === 'a' && typeof child.properties.dataWikiTarget === 'string') {
        child.properties.className = [
          'internal-link',
          ...(child.properties.dataWikiEmbed ? ['internal-embed'] : []),
        ]
      }

      if (typeof child.properties.dataBlockAnchor === 'string') {
        const anchor = child.properties.dataBlockAnchor
        child.properties.id = `md-${markdownDocumentHash(options.documentKey ?? 'document')}-block-${anchor.slice(1)}`
      }

      if (child.tagName === 'table') {
        parent.children[index] = {
          type: 'element',
          tagName: 'div',
          properties: {
            className: ['table-scroll'],
            role: 'region',
            ariaLabel: '表格',
          },
          children: [child],
        }
        visitChildren(child)
        continue
      }

      if (child.tagName === 'pre') {
        const code = child.children.find((node): node is Element => node.type === 'element' && node.tagName === 'code')
        if (code) {
          const classes = Array.isArray(code.properties.className) ? code.properties.className : []
          if (
            classes.includes('math-display')
            || classes.includes('math-inline')
            || languageOf(code) === 'math'
          ) {
            visitChildren(child)
            continue
          }
          const raw = textContent(code)
          const language = languageOf(code)
          if (language === 'mermaid') {
            parent.children[index] = {
              type: 'element',
              tagName: 'div',
              properties: { className: ['mermaid'], dataRaw: raw },
              children: [hastText(raw)],
            }
          } else {
            const lines = raw.endsWith('\n') ? raw.slice(0, -1).split('\n') : raw.split('\n')
            const className = ['hljs', ...(language ? [`language-${language}`] : [])]
            code.properties.className = className
            parent.children[index] = element('div', ['code-block'], [
              element('pre', ['code-gutter'], [hastText(lines.map((_line, line) => line + 1).join('\n'))]),
              element('pre', ['code-content'], [code]),
            ])
          }
          continue
        }
      }

      visitChildren(child)
    }
  }

  visitChildren(tree)
}

/**
 * Render one Markdown document through the shared, deterministic parsing core.
 * DOM-only enhancements (highlight.js and Mermaid) intentionally remain lazy.
 */
export function renderMarkdownDocument(
  source: string,
  options: MarkdownRenderOptions = {},
): RenderedMarkdown {
  const normalized = normalizeAlternativeMath(stripObsidianComments(source))
  const headings: MarkdownHeading[] = []
  const diagnostics: MarkdownDiagnostic[] = []

  const processor = unified()
    .use(remarkParse)
    .use(remarkFrontmatter, ['yaml', 'toml'])
    .use(remarkGfm)
    .use(remarkMath)
    .use(() => (tree) => {
      restoreCurrencyPairs(tree as MdastNode, normalized)
      transformMdast(tree as MdastNode, options)
      headings.push(...collectHeadings(tree as MdastNode, options.documentKey ?? 'document'))
    })
    .use(remarkRehype, { allowDangerousHtml: true, clobberPrefix: '' })
    .use(rehypeRaw)
    .use(rehypeSanitize, sanitizeSchema)
    .use(() => (tree) => transformHast(tree as HastRoot, headings, options))
    .use(rehypeKatex, {
      strict: 'warn',
      trust: false,
      macros: {},
      maxExpand: 1000,
      maxSize: 100,
    })
    .use(rehypeStringify)

  try {
    const file = processor.processSync(normalized)
    diagnostics.push(...file.messages.map((message) => ({
      kind: message.source === 'rehype-katex' ? 'math' as const : 'parse' as const,
      message: message.reason,
      source: message.source || undefined,
    })))
    return {
      html: String(file),
      headings,
      diagnostics,
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    diagnostics.push({ kind: 'parse', message })
    return {
      html: `<pre class="markdown-render-error">${source
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')}</pre>`,
      headings: [],
      diagnostics,
    }
  }
}
