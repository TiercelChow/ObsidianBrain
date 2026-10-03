import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { attachGlassLens, detachGlassLens } from '../src/utils/glassOptics.ts'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('top chrome uses an open scroll edge rather than a closed glass rim', async () => {
  const [app, reader, main] = await Promise.all([source('App.vue'), source('views/Reader.vue'), source('main.ts')])
  assert.match(app, /v-glass-lens="'css'"[^>]+class="mobile-global-header"[^>]+data-glass="scroll-edge"/)
  assert.match(reader, /v-glass-lens="'css'"[^>]+class="reader-topbar glass-surface"[^>]+data-glass="scroll-edge"/)
  assert.match(reader, /heading\.setAttribute\('data-glass', 'scroll-edge'\)/)
  assert.match(reader, /attachGlassLens\(heading, \{ mode: 'css' \}\)/)
  assert.match(reader, /heading\.removeAttribute\('data-glass'\)/)
  assert.doesNotMatch(reader.match(/function syncTitleLens[\s\S]+?watch\(isFullscreen/)?.[0] || '', /data-glass-rim/)
  assert.match(main, /binding\.value === 'css'/)
  const title = reader.match(/:deep\(h1:first-of-type\)\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(title, /overflow:\s*visible/)
  assert.match(title, /border:\s*0;/)
  assert.match(title, /box-shadow:\s*none;/)
  assert.doesNotMatch(title, /backdrop-filter:\s*var|background:\s*var\(--glass-sheen\)/)
})

test('scroll edge fades only its extended background plane and has no divider', async () => {
  const css = await source('styles/materials.css')
  const host = css.match(/:root\[data-theme\] \[data-glass="scroll-edge"\]\s*\{([^}]+)\}/)?.[1] || ''
  for (const contract of [/background:\s*none\s*!important/, /border:\s*0\s*!important/, /box-shadow:\s*none\s*!important/, /backdrop-filter:\s*none\s*!important/, /overflow:\s*visible/]) assert.match(host, contract)
  assert.doesNotMatch(host, /mask-image|filter:\s*blur/)
  const plane = css.match(/\[data-glass="scroll-edge"\] > \.glass-optics\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(plane, /bottom:\s*calc\(var\(--glass-scroll-edge-fade\) \* -1\)/)
  assert.match(plane, /mask-image:\s*linear-gradient\(to bottom/)
  assert.match(plane, /transparent 100%/)
  assert.match(plane, /border-radius:\s*0/)
  assert.doesNotMatch(plane, /glass-sheen|box-shadow|url\(/)
  const edge = css.match(/\[data-glass="scroll-edge"\] \.glass-optics__edge\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(edge, /mask-image:\s*linear-gradient\(to bottom/)
  assert.match(edge, /transparent 75%/)
  assert.doesNotMatch(edge, /to right|rim-|url\(/)
  assert.match(css, /\.reader-topbar\[data-glass="scroll-edge"\]\s*\{[^}]+z-index:\s*2/)
  assert.match(css, /is-fs-transitioning\s+\.glass-optics\s*\{\s*display:\s*none/)
  assert.match(css, /prefers-reduced-transparency[\s\S]+?\[data-glass="scroll-edge"\][^}]+background:\s*var\(--glass-solid\)/)
  assert.match(css, /forced-colors[\s\S]+?\[data-glass="scroll-edge"\][^}]+background:\s*Canvas/)
})

test('CSS scroll chrome requires no geometry maps, observers or preference listeners', () => {
  class Layer {
    className = ''
    parent?: Layer
    children: Layer[] = []
    dataset: Record<string, string> = {}
    style = { setProperty() {} }
    setAttribute() {}
    append(child: Layer) { child.parent = this; this.children.push(child) }
    prepend(child: Layer) { child.parent = this; this.children.unshift(child) }
    remove() { if (this.parent) this.parent.children = this.parent.children.filter(child => child !== this) }
    // Deliberately no browser observer APIs, canvas, or matchMedia.
    ownerDocument = { defaultView: {}, createElement: () => new Layer() }
  }
  const host = new Layer()
  const cleanup = attachGlassLens(host as unknown as HTMLElement, { mode: 'css' })
  assert.equal(host.dataset.glassLens, 'css')
  assert.equal(host.children.length, 1)
  assert.equal(host.children[0].children[0].className, 'glass-optics__edge')
  attachGlassLens(host as unknown as HTMLElement, { mode: 'css' })
  assert.equal(host.children.length, 1, 'replacement stays singular')
  cleanup()
  assert.equal(host.children.length, 1, 'stale disposer cannot remove the replacement')
  assert.equal(host.dataset.glassLens, 'css')
  detachGlassLens(host as unknown as HTMLElement)
  assert.equal(host.children.length, 0)
  assert.equal(host.dataset.glassLens, undefined)
  cleanup()
  assert.equal(host.children.length, 0, 'stale disposer is harmless')
})
