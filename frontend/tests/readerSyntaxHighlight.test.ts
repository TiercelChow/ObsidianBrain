import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('lazy code enhancement uses the highlighter completion marker, not its presentation class', async () => {
  const runtime = await source('composables/useMarkdownRender.ts')
  assert.match(runtime, /pre code:not\(\[data-highlighted\]\)/)
  assert.doesNotMatch(runtime, /pre code:not\(\.hljs\)/)
  assert.match(runtime, /if \([^\n]*generation !== enhancementGeneration[^\n]*el\.dataset\.highlighted[^\n]*\) return/)
  assert.ok(runtime.includes("el.dataset.highlighted ??= 'skipped'"))
  assert.match(runtime, /rootMargin: '700px 0px'/)
  assert.match(runtime, /import\('highlight\.js\/lib\/common'\)/)
})

test('syntax themes load for every entry point without a route-dependent fixed dark theme', async () => {
  const [main, reader, timeline, css] = await Promise.all([
    source('main.ts'), source('views/Reader.vue'), source('views/Timeline.vue'), source('styles/syntax-highlight.css'),
  ])
  assert.match(main, /import '\.\/styles\/syntax-highlight\.css'/)
  assert.doesNotMatch(reader, /--tk-|\.hljs-keyword\s*[,\{]/)
  assert.doesNotMatch(timeline, /highlight\.js\/styles\//)
  assert.doesNotMatch(reader, /\.code-block \{/)
  assert.match(css, /\.code-block \{[^}]*background: var\(--syntax-bg\)/)
  assert.match(timeline, /background: var\(--syntax-bg\)/)
  assert.match(css, /:is\(\.code-block, \.memo-code\) \.hljs-keyword/)
  assert.match(css, /\.hljs-title\.class_/)
  assert.match(css, /\.hljs-title\.function_/)
  assert.match(css, /\.hljs-meta \.hljs-string/)
})

function luminance(hex: string) {
  const rgb = hex.match(/[\da-f]{2}/gi)!.map(value => {
    const channel = parseInt(value, 16) / 255
    return channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4
  })
  return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722
}

test('all three syntax palettes have distinct colors and readable small text', async () => {
  const css = await source('styles/syntax-highlight.css')
  const palettes: Record<string, Record<string, string>> = {}
  for (const theme of ['light', 'dark', 'eye-care']) {
    const block = css.match(new RegExp(`:root\\[data-theme="${theme}"\\] \\{([^}]+)\\}`))?.[1]
    assert.ok(block, `${theme} has an explicit palette`)
    const colors = Object.fromEntries([...block.matchAll(/--syntax-([\w-]+):\s*(#[\da-f]{6});/gi)].map(match => [match[1], match[2]]))
    palettes[theme] = colors
    assert.ok(colors.bg)
    for (const token of ['text', 'comment', 'keyword', 'string', 'number', 'function', 'type', 'variable', 'tag', 'meta', 'operator', 'punctuation', 'addition', 'deletion']) {
      assert.ok(colors[token], `${theme}.${token} is defined`)
      const a = luminance(colors[token]), b = luminance(colors.bg)
      const contrast = (Math.max(a, b) + .05) / (Math.min(a, b) + .05)
      assert.ok(contrast >= 4.5, `${theme}.${token} contrast ${contrast.toFixed(2)} >= 4.5`)
    }
    assert.notEqual(colors.keyword, colors.string)
    assert.notEqual(colors.function, colors.comment)
  }
  for (const token of Object.keys(palettes.light)) {
    assert.equal(new Set(Object.values(palettes).map(palette => palette[token])).size, 3, `${token} is designed separately for each theme`)
  }
})
