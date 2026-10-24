import assert from 'node:assert/strict'
import test from 'node:test'
import { readFile } from 'node:fs/promises'

import { buildMermaidConfig, getMermaidPalette } from '../src/markdown/mermaidTheme.ts'
import { getDiagramViewport, constrainDiagramTransform } from '../src/utils/mermaidViewport.ts'

const tokens = {
  background: '#0c1018', foreground: '#f3f6fc', muted: '#a2abbc', accent: '#a3a6ff',
  fontFamily: '-apple-system, "PingFang SC", sans-serif',
}

test('diagram colors derive from system tokens, with opaque readable label surfaces', () => {
  const palette = getMermaidPalette(tokens)
  assert.equal(palette.text, tokens.foreground)
  assert.equal(palette.accent, tokens.accent)
  assert.match(palette.node, /^#[\da-f]{6}$/i)
  assert.notEqual(palette.node, tokens.background)
  assert.notEqual(palette.node, palette.group)
})

test('flowcharts use ELK while other diagram types retain their native layout', () => {
  const flow = buildMermaidConfig(tokens, 'flowchart-v2')
  assert.equal(flow.layout, 'elk')
  assert.equal(buildMermaidConfig(tokens, 'sequence').layout, 'dagre')
  assert.equal(flow.theme, 'base')
  assert.equal(flow.fontFamily, tokens.fontFamily)
  assert.equal(flow.securityLevel, 'strict')
  assert.equal(flow.forceLegacyMathML, true)
  assert.equal(flow.markdownAutoWrap, true)
  assert.equal(flow.flowchart?.useMaxWidth, false)
  assert.equal(flow.themeVariables.primaryTextColor, tokens.foreground)
})

test('flowchart routing never adds a basis spline wobble to the layout bend points', () => {
  const config = buildMermaidConfig(tokens)
  assert.equal(config.flowchart?.curve, 'linear')
  assert.equal(config.elk?.nodePlacementStrategy, 'NETWORK_SIMPLEX')
  assert.match(config.themeCSS || '', /stroke-linejoin: round/)
})

test('inline diagrams fit the whole document width without losing their natural viewBox', async () => {
  const runtime = await readFile(new URL('../src/markdown/mermaidRuntime.ts', import.meta.url), 'utf8')
  const css = await readFile(new URL('../src/styles/mermaid.css', import.meta.url), 'utf8')
  assert.match(runtime, /root\.style\.maxWidth = '100%'/)
  assert.match(runtime, /root\.style\.height = 'auto'/)
  assert.match(css, /\.mermaid\[data-raw\] > svg\s*\{[^}]*max-width: 100%/)
})

test('viewer canvas fills its viewport and optical controls float without a header band', async () => {
  const viewer = await readFile(new URL('../src/components/reader/MermaidViewer.vue', import.meta.url), 'utf8')
  assert.doesNotMatch(viewer, /class="mv-header"|class="mv-heading"/)
  assert.match(viewer, /\.mv-stage\s*\{[^}]*position: absolute;[^}]*inset: 0;/)
  assert.match(viewer, /\.mv-footer\s*\{[^}]*position: absolute;/)
  assert.match(viewer, /class="mv-close-surface"[^>]*data-glass="floating"/)
  assert.match(viewer, /class="mv-controls"[^>]*data-glass="floating"/)
})

test('eye-care colors come from the supplied system palette rather than the light theme', () => {
  const config = buildMermaidConfig({ ...tokens, background: '#d8e0d0', foreground: '#1b3020', accent: '#276b43' }, 'flowchart-v2')
  assert.equal(config.themeVariables.primaryTextColor, '#1b3020')
  assert.equal(config.themeVariables.nodeTextColor, '#1b3020')
  assert.notEqual(config.themeVariables.primaryColor, buildMermaidConfig(tokens, 'flowchart-v2').themeVariables.primaryColor)
})

test('embedded math is isolated from prose inline-block and overflow rules during measurement and display', () => {
  const config = buildMermaidConfig(tokens)
  assert.match(config.themeCSS || '', /\.katex\s*\{[^}]*display: inline;[^}]*overflow: visible;[^}]*vertical-align: baseline;/)
  assert.match(config.themeCSS || '', /\.katex-display\s*\{[^}]*overflow: visible;/)
})

test('large diagrams open at a readable scale instead of shrinking every label to fit a phone', () => {
  const viewport = getDiagramViewport({ width: 1800, height: 1000 }, { width: 366, height: 650 }, 'readable')
  assert.ok(viewport.scale >= 0.85)
  assert.ok(viewport.fitScale < 0.3)
  assert.ok(viewport.x <= 0)
  const overview = getDiagramViewport({ width: 1800, height: 1000 }, { width: 366, height: 650 }, 'fit')
  assert.equal(overview.scale, overview.fitScale)
  assert.ok(overview.x >= 0)
})

test('small diagrams are centered without enlarging them and invalid sizes are safe', () => {
  const small = getDiagramViewport({ width: 200, height: 100 }, { width: 400, height: 600 }, 'readable')
  assert.equal(small.scale, 1)
  assert.equal(small.x, 100)
  assert.equal(small.y, 250)
  const invalid = getDiagramViewport({ width: 0, height: NaN }, { width: 0, height: 0 }, 'fit')
  assert.ok(Number.isFinite(invalid.scale) && invalid.scale > 0)
})

test('panning cannot permanently lose the whole diagram outside the stage', () => {
  const constrained = constrainDiagramTransform({ x: -9000, y: 9000, scale: 1 }, { width: 1800, height: 1000 }, { width: 366, height: 650 })
  assert.ok(constrained.x >= 366 - 1800 - 64)
  assert.ok(constrained.y <= 64)
  const small = constrainDiagramTransform({ x: 9000, y: -9000, scale: 1 }, { width: 100, height: 100 }, { width: 366, height: 650 })
  assert.equal(small.x, 133)
  assert.equal(small.y, 275)
})
