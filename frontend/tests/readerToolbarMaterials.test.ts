import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')
const rgb = (hex: string) => [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16))
function luminance(channels: number[]) {
  const values = channels.map(value => value / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4)
  return values[0] * .2126 + values[1] * .7152 + values[2] * .0722
}

test('reader toolbar has a denser theme-adaptive tint with readable labels over contrasting content', async () => {
  const [css, tokens] = await Promise.all([source('styles/materials.css'), source('styles/tokens.css')])
  for (const theme of ['light', 'dark', 'eye-care']) {
    const material = css.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)\\}`))?.[1] || ''
    const palette = tokens.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)\\}`))?.[1] || ''
    const fill = material.match(/--glass-reader-toolbar-fill:\s*color-mix\(in srgb, var\(--glass-solid\) (\d+)%, transparent\)/)
    assert.ok(fill, `${theme}: dedicated tinted glass`)
    const alpha = Number(fill[1]) / 100
    assert.ok(alpha >= .6 && alpha < .8, `${theme}: translucent, not nearly invisible or opaque`)
    const solid = rgb(material.match(/--glass-solid:\s*(#[a-f\d]{6})/i)?.[1] || '')
    const label = rgb(palette.match(/--text-primary:\s*(#[a-f\d]{6})/i)?.[1] || '')
    for (const background of [0, 255]) {
      const surface = solid.map(channel => channel * alpha + background * (1 - alpha))
      const [lighter, darker] = [luminance(surface), luminance(label)].sort((a, b) => b - a)
      assert.ok((lighter + .05) / (darker + .05) >= 4.5, `${theme}: title contrast over ${background} backdrop`)
    }
  }
  assert.match(css, /--glass-floating-filter:\s*blur\(3px\)/, 'other floating materials remain clear')
})

test('only the reader toolbar receives stronger background separation without losing its refraction', async () => {
  const css = await source('styles/materials.css')
  const plane = css.match(/\.reader-mobile-toolbar > \.glass-optics\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(plane, /background:\s*var\(--glass-reader-toolbar-sheen\), var\(--glass-reader-toolbar-fill\)/)
  assert.match(plane, /backdrop-filter:\s*var\(--glass-lens-filter, var\(--glass-floating-filter\)\) var\(--glass-reader-toolbar-soften-filter\)/)
  assert.match(plane, /-webkit-backdrop-filter:/)
  assert.match(css, /\.reader-mobile-toolbar \.glass-optics__edge\s*\{[^}]+var\(--glass-reader-toolbar-edge-filter\)/)
  assert.match(css, /\.reader-mobile-toolbar button\s*\{[^}]+color:\s*var\(--text-primary\)/)
  assert.doesNotMatch(plane, /animation|transition|will-change|pointer-events:\s*auto|radial-gradient/)
  assert.match(css, /prefers-reduced-transparency[\s\S]+?\.glass-optics\s*\{\s*display:\s*none/)
  assert.match(css, /forced-colors[\s\S]+?\.glass-optics\s*\{\s*display:\s*none/)
})

test('reader toolbar keeps four large actions, centered title, idle hiding and bounded lens lifecycle', async () => {
  const [reader, optics] = await Promise.all([source('views/Reader.vue'), source('utils/glassOptics.ts')])
  assert.match(reader, /class="reader-mobile-toolbar"\s+v-glass-lens\s+data-glass-rim/)
  assert.match(reader, /:aria-hidden="!mobileToolbarState.visible"/)
  assert.match(reader, /grid-template-columns:\s*108px minmax\(0, 1fr\) 108px/)
  assert.match(reader, /\.reader-mobile-toolbar button\s*\{[^}]+height:\s*52px/)
  assert.match(reader, /\.reader-mobile-toolbar button \.el-icon\s*\{\s*font-size:\s*24px/)
  assert.match(reader, /\.reader-document-label\s*\{[^}]+text-align:\s*center/)
  assert.match(optics, /host\.getAttribute\('aria-hidden'\) === 'true'/)
  assert.match(optics, /512 \/ w, 160 \/ h/)
})
