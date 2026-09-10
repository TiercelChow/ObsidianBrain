import assert from 'node:assert/strict'
import test from 'node:test'

import { renderMarkdownDocument } from '../src/markdown/renderMarkdown.ts'

function render(source: string) {
  return renderMarkdownDocument(source, { documentKey: '/notes/demo.md' })
}

test('renders inline math next to CJK text and alternative delimiters', () => {
  const result = render(String.raw`当$x_i^2$收敛时，误差为\(\epsilon\)。`)

  assert.equal(result.diagnostics.length, 0)
  assert.equal((result.html.match(/class="katex"/g) || []).length, 2)
  assert.doesNotMatch(result.html, /\$x_i\^2\$/)
  assert.doesNotMatch(result.html, /\\\(\\epsilon\\\)/)
})

test('renders legacy formula-shaped inline code without swallowing ordinary code', () => {
  const result = render(String.raw`注意 ` + '`d\\_k=d\\_v=128`' + String.raw`，但 ` + '`npm run_build`' + ' 仍是代码。')

  assert.equal((result.html.match(/class="katex"/g) || []).length, 1)
  assert.match(result.html, /<annotation[^>]*>d_k=d_v=128<\/annotation>/)
  assert.match(result.html, /<code>npm run_build<\/code>/)
  assert.doesNotMatch(result.html, /<code>d\\_k=d\\_v=128<\/code>/)
})

test('does not treat paired currency values or escaped dollars as math', () => {
  const result = render(String.raw`价格从 $5 降到 $3，转义的 \$x\$ 保持文本，公式 $2x+1$ 正常。`)

  assert.equal((result.html.match(/class="katex"/g) || []).length, 1)
  assert.match(result.html, /\$5 降到 \$3/)
  assert.match(result.html, /\$x\$/)
})

test('shares macros within one document but never leaks them into the next document', () => {
  const first = render(String.raw`$\gdef\RR{\mathbb{R}}$ 定义后 $x \in \RR$。`)
  const second = render(String.raw`另一篇中的 $x \in \RR$ 不继承宏。`)

  assert.doesNotMatch(first.html, /#cc0000/)
  assert.match(second.html, /#cc0000/)
  assert.equal(first.diagnostics.length, 0)
  assert.equal(second.diagnostics[0]?.kind, 'math')
})

test('renders heading math once and generates stable semantic heading metadata', () => {
  const result = render(String.raw`## 损失函数$\mathcal{L}(\theta)$`)

  assert.equal(result.headings.length, 1)
  assert.equal(result.headings[0].depth, 2)
  assert.equal(result.headings[0].label, String.raw`损失函数 \mathcal{L}(\theta)`)
  assert.equal(result.headings[0].anchor, '损失函数-mathcal-l-theta')
  assert.match(result.html, /<h2[^>]+data-heading-label="损失函数 \\mathcal\{L\}\(\\theta\)"/)
  assert.equal((result.html.match(/class="katex"/g) || []).length, 1)
})

test('treats paired dollars as inline math inside headings and table cells', () => {
  const result = render([
    '## 标题 $$x_i$$ 后缀',
    '',
    '| 项目 | 数值 |',
    '| --- | --- |',
    '| 向量 | $$y_i$$ |',
  ].join('\n'))

  assert.equal(result.headings[0]?.label, '标题 x_i 后缀')
  assert.equal((result.html.match(/class="katex"/g) || []).length, 2)
  assert.match(result.html, /<h2[^>]*>标题 <span class="katex"/)
  assert.match(result.html, /<td><span class="katex"/)
  assert.doesNotMatch(result.html, /katex-display/)
})

test('protects long backtick, tilde, inline and indented code from math normalization', () => {
  const source = [
    '````markdown',
    String.raw`正文 \(x_i\)`,
    '```js',
    String.raw`const price = "$5"`,
    '```',
    '````',
    '',
    '~~~text',
    String.raw`\[y_i\]`,
    '~~~',
    '',
    '行内 ``\\(z_i\\) and `tick` ``',
    '',
    String.raw`    \(w_i\)`,
  ].join('\n')

  const result = render(source)

  assert.equal((result.html.match(/class="katex"/g) || []).length, 0)
  assert.match(result.html, /\\\(x_i\\\)/)
  assert.match(result.html, /\\\[y_i\\\]/)
  assert.match(result.html, /\\\(z_i\\\)/)
  assert.match(result.html, /\\\(w_i\\\)/)
})

test('keeps multiline display math source semantics including TeX comments', () => {
  const result = render(String.raw`\[
x + % comment stays on this line
y
\]`)

  assert.equal((result.html.match(/class="katex-display"/g) || []).length, 1)
  assert.match(result.html, /annotation[^>]*>[^<]*% comment stays on this line\n+y/)
  assert.doesNotMatch(result.html, /code-block|code-gutter/)
})

test('keeps Mermaid math as TeX source instead of rewriting subscripts to Unicode', () => {
  const source = [
    '```mermaid',
    'flowchart LR',
    String.raw`  A["$$x_i^2$$"] -->|"$$\frac{x_i}{y_i}$$"| B["结果"]`,
    '```',
  ].join('\n')
  const result = render(source)

  assert.match(result.html, /class="mermaid"/)
  assert.match(result.html, /x_i\^2/)
  assert.match(result.html, /\\frac\{x_i\}\{y_i\}/)
  assert.doesNotMatch(result.html, /xᵢ²/)
})

test('renders GFM tables, task lists and footnotes', () => {
  const result = render([
    '| 项目 | 公式 |',
    '| --- | --- |',
    '| 范数 | $\\lVert x\\rVert_2$ |',
    '',
    '- [x] 已完成',
    '',
    '正文脚注[^note]。',
    '',
    '[^note]: 脚注中的 $x_i$。',
  ].join('\n'))

  assert.match(result.html, /class="table-scroll"/)
  assert.match(result.html, /type="checkbox"/)
  assert.match(result.html, /data-footnotes/)
  assert.ok((result.html.match(/class="katex"/g) || []).length >= 2)
})

test('renders Obsidian wikilinks, highlights, comments and callouts', () => {
  const result = render([
    String.raw`阅读 [[数学/损失函数#定义|损失函数]] 与 ==重点公式 $x_i$==。%%这段不显示%%`,
    '',
    String.raw`> [!TIP]- 当 $\alpha > 0$ 时`,
    '> 可以逐步衰减。',
  ].join('\n'))

  assert.match(result.html, /class="internal-link"/)
  assert.match(result.html, /href="%E6%95%B0%E5%AD%A6\/%E6%8D%9F%E5%A4%B1%E5%87%BD%E6%95%B0\.md#%E5%AE%9A%E4%B9%89"/)
  assert.match(result.html, /<mark>重点公式 <span class="katex"/)
  assert.doesNotMatch(result.html, /这段不显示/)
  assert.match(result.html, /<details[^>]+class="callout callout-tip"/)
  assert.match(result.html, /<summary class="callout-title"/)
  assert.match(result.html, /class="callout-content"/)
})

test('removes multiline Obsidian comments without touching code markers', () => {
  const result = render([
    '保留正文。',
    '',
    '%% 隐藏段落',
    String.raw`隐藏公式 $x_i$ 与 \(y_i\)。`,
    '仍然隐藏 %%',
    '',
    '保留公式 $z_i$ 与代码 `%%keep%%`。',
  ].join('\n'))

  assert.doesNotMatch(result.html, /隐藏段落|隐藏公式|仍然隐藏/)
  assert.match(result.html, /%%keep%%/)
  assert.equal((result.html.match(/class="katex"/g) || []).length, 1)
})

test('rewrites standard and Obsidian images while retaining requested dimensions', () => {
  const result = renderMarkdownDocument([
    '![普通图](./images/a.png)',
    '',
    '![[images/b.png|300x180]]',
  ].join('\n'), {
    documentKey: '/notes/demo.md',
    resolveImage: (href) => `/raw/standard?src=${encodeURIComponent(href)}`,
    resolveEmbed: (target) => `/raw/embed?src=${encodeURIComponent(target)}`,
  })

  assert.match(result.html, /src="\/raw\/standard\?src=\.%2Fimages%2Fa.png"/)
  assert.match(result.html, /src="\/raw\/embed\?src=images%2Fb.png"/)
  assert.match(result.html, /width="300"/)
  assert.match(result.html, /height="180"/)
  assert.equal((result.html.match(/decoding="async"/g) || []).length, 2)
  assert.equal((result.html.match(/loading="lazy"/g) || []).length, 2)
})

test('sanitizes dangerous raw HTML and unsafe protocols', () => {
  const result = render([
    '<script>globalThis.pwned = true</script>',
    '<img src="x" onerror="globalThis.pwned = true">',
    '<a href="javascript:alert(1)" onclick="alert(2)">危险</a>',
    '<details open><summary>安全详情</summary><kbd>⌘K</kbd></details>',
  ].join('\n'))

  assert.doesNotMatch(result.html, /<script|onerror|onclick|javascript:/i)
  assert.match(result.html, /<details open>/)
  assert.match(result.html, /<summary>安全详情<\/summary>/)
  assert.match(result.html, /<kbd>⌘K<\/kbd>/)
})

test('namespaces DOM ids while preserving logical anchors for duplicate headings', () => {
  const result = render('# 同名\n\n# 同名')

  assert.deepEqual(result.headings.map((heading) => heading.anchor), ['同名', '同名-1'])
  assert.equal(new Set(result.headings.map((heading) => heading.id)).size, 2)
  assert.ok(result.headings.every((heading) => heading.id.startsWith('md-')))
  assert.match(result.html, /data-anchor="同名"/)
  assert.match(result.html, /data-anchor="同名-1"/)
})

test('hides Obsidian block references and exposes document-scoped block anchors', () => {
  const result = render('这段可以被引用。 ^proof-1')

  assert.doesNotMatch(result.html, /\^proof-1<\/p>/)
  assert.match(result.html, /id="md-[^"]+-block-proof-1"/)
  assert.match(result.html, /data-block-anchor="\^proof-1"/)
})

test('contains raw HTML tables in the same horizontal scrolling region', () => {
  const result = render('<table><tr><td>左侧</td><td>右侧</td></tr></table>')

  assert.match(result.html, /<div class="table-scroll"[^>]*><table>/)
})
