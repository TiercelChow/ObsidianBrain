import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { attachGlassLens, buildLensMap, canUseSvgBackdrop, detachGlassLens } from '../src/utils/glassOptics.ts'

const pixel = (map: ReturnType<typeof buildLensMap>, x: number, y: number) => {
  const offset = (y * map.width + x) * 4
  return [...map.pixels.slice(offset, offset + 4)]
}

test('lens map leaves the center neutral and bends only a narrow rounded edge', () => {
  const map = buildLensMap(360, 62, 20)
  assert.deepEqual(pixel(map, 180, 31), [128, 128, 128, 255])
  assert.deepEqual(pixel(map, 40, 31), [128, 128, 128, 255])
  assert.ok(pixel(map, 180, 3)[1] > 128)
  assert.ok(pixel(map, 180, 58)[1] < 128)
  assert.ok(pixel(map, 3, 31)[0] > 128)
  assert.ok(pixel(map, 356, 31)[0] < 128)
  assert.deepEqual(pixel(map, 0, 0), [128, 128, 128, 255], 'outside corners stay neutral')
  assert.ok(pixel(map, 7, 7)[0] > 128 && pixel(map, 7, 7)[1] > 128, 'rounded corner uses both axes')
})

test('edge displacement eases to neutral without ripples or a shifted center', () => {
  const map = buildLensMap(360, 62, 20)
  const values = Array.from({ length: 12 }, (_, y) => pixel(map, 180, y)[1])
  assert.ok(values[2] > values[8])
  assert.equal(values[11], 128)
  assert.ok(values.every(value => value >= 128 && value <= 224))
})

test('maps stay bounded at large sizes and handle degenerate geometry safely', () => {
  for (const dimensions of [[4000, 1000, 500], [1, 1, 50], [0, 0, 0], [NaN, Infinity, -3]]) {
    const map = buildLensMap(...dimensions as [number, number, number])
    assert.ok(map.width >= 1 && map.width <= 512)
    assert.ok(map.height >= 1 && map.height <= 160)
    assert.equal(map.pixels.length, map.width * map.height * 4)
    assert.ok(map.pixels.every(Number.isFinite))
  }
})

test('SVG backdrop enhancement excludes WebKit and unknown engines even when CSS accepts url()', () => {
  assert.equal(canUseSvgBackdrop('Mozilla/5.0 AppleWebKit/537.36 Chrome/146.0.0.0 Safari/537.36'), true)
  assert.equal(canUseSvgBackdrop('Mozilla/5.0 AppleWebKit/605.1.15 Version/26.0 Safari/605.1.15'), false)
  assert.equal(canUseSvgBackdrop('Mozilla/5.0 iPhone AppleWebKit/605.1.15 CriOS/146.0 Mobile/15E148 Safari/604.1'), false)
  assert.equal(canUseSvgBackdrop('Mozilla/5.0 Firefox/140.0'), false)
  assert.equal(canUseSvgBackdrop(''), false)
})

test('optical runtime never captures content and releases observers, timers and SVG resources', async () => {
  const source = await readFile(new URL('../src/utils/glassOptics.ts', import.meta.url), 'utf8')
  assert.doesNotMatch(source, /cloneNode|innerHTML|fetch\(|setInterval|requestAnimationFrame|addEventListener\(['"]scroll/)
  for (const contract of ['ResizeObserver', 'IntersectionObserver', 'disconnect()', 'clearTimeout', 'removeEventListener', 'filter?.remove()', 'layer.remove()', 'prefers-reduced-transparency', 'forced-colors']) assert.ok(source.includes(contract), contract)
  assert.match(source, /pointer-events/)
  assert.match(source, /aria-hidden/)
})

test('clear layers own background filters, accessibility disables the whole lens, and foreground remains unfiltered', async () => {
  const css = await readFile(new URL('../src/styles/materials.css', import.meta.url), 'utf8')
  assert.match(css, /\[data-glass-lens\]\s*\{[^}]+backdrop-filter:\s*none\s*!important/)
  assert.match(css, /\.glass-optics\s*\{[^}]+z-index:\s*-1;[^}]+pointer-events:\s*none/)
  assert.match(css, /\.glass-optics__edge\s*\{[^}]+mask-image:\s*linear-gradient/)
  assert.match(css, /prefers-reduced-transparency[\s\S]+?\.glass-optics\s*\{\s*display:\s*none/)
  assert.match(css, /forced-colors[\s\S]+?\.glass-optics\s*\{\s*display:\s*none/)
  assert.match(css, /is-fs-transitioning\s+\.glass-optics\s*\{\s*display:\s*none/)
  assert.doesNotMatch(css, /(?:^|[;\n])\s*filter:\s*url/)
})

test('chat scrolls beneath the lens but reserves enough room to reach the final answer', async () => {
  const chat = await readFile(new URL('../src/views/knowledge/KnowledgeChat.vue', import.meta.url), 'utf8')
  assert.match(chat, /<form v-glass-lens class="chat-composer" data-glass-rim/)
  assert.match(chat, /\.chat-panel\s*\{[^}]+display:\s*grid/)
  assert.match(chat, /\.message-list\s*\{[^}]+padding:[^;]+100px;[^}]+scroll-padding-bottom:\s*100px/)
  assert.match(chat, /\.chat-composer\s*\{[^}]+grid-area:\s*1\s*\/\s*1;[^}]+align-self:\s*end/)
})

test('fullscreen title optics follow the active article and are released on leave and unmount', async () => {
  const reader = await readFile(new URL('../src/views/Reader.vue', import.meta.url), 'utf8')
  assert.match(reader, /watch\(isFullscreen,\s*\(\) => syncTitleLens\(\)/)
  assert.match(reader, /function syncTitleLens[\s\S]+?attachGlassLens\(heading, \{ mode: 'css' \}\)/)
  assert.match(reader, /onContentAfterLeave[\s\S]+?detachGlassLens\(heading\)/)
  assert.match(reader, /onBeforeUnmount\(\(\) => \{[^}]+titleLensCleanup\?\.\(\)/)
})

test('lens resources follow visibility and live preferences, and remount never duplicates layers', async (t) => {
  class Node {
    dataset: Record<string, string> = {}
    attrs = new Map<string, string>()
    properties = new Map<string, string>()
    children: Node[] = []
    parent?: Node
    ownerDocument: unknown
    style = { setProperty: (name: string, value: string) => this.properties.set(name, value), removeProperty: (name: string) => this.properties.delete(name) }
    classList = { add() {} }
    offsetWidth = 360
    offsetHeight = 62
    clientLeft = 1
    clientTop = 1
    setAttribute(name: string, value: string) { this.attrs.set(name, value) }
    getAttribute(name: string) { return this.attrs.get(name) ?? null }
    append(...nodes: Node[]) { for (const node of nodes) { node.parent = this; this.children.push(node) } }
    prepend(node: Node) { node.parent = this; this.children.unshift(node) }
    remove() { if (this.parent) this.parent.children = this.parent.children.filter(node => node !== this) }
    getContext() { return { createImageData: (w: number, h: number) => ({ data: new Uint8ClampedArray(w * h * 4) }), putImageData() {} } }
    toDataURL() { return 'data:image/png;base64,fixture' }
  }
  const preferences = Array.from({ length: 3 }, () => ({ matches: false, listeners: new Set<() => void>(), addEventListener(_event: string, callback: () => void) { this.listeners.add(callback) }, removeEventListener(_event: string, callback: () => void) { this.listeners.delete(callback) } }))
  let preferenceIndex = 0
  const body = new Node()
  const doc = { body, createElement: () => new Node(), createElementNS: () => new Node(), defaultView: { navigator: { userAgent: 'Chrome/146.0' }, CSS: { supports: () => true }, getComputedStyle: () => ({ borderTopLeftRadius: '20px' }), matchMedia: () => preferences[preferenceIndex++ % 3] } }
  const host = new Node()
  host.ownerDocument = doc
  const observers: Observer[] = []
  class Observer {
    disconnected = false
    callback: (entries: { isIntersecting: boolean }[]) => void
    constructor(callback: (entries: { isIntersecting: boolean }[]) => void) { this.callback = callback; observers.push(this) }
    observe() {}
    disconnect() { this.disconnected = true }
  }
  const mutations: VisibilityObserver[] = []
  class VisibilityObserver {
    disconnected = false
    callback: () => void
    constructor(callback: () => void) { this.callback = callback; mutations.push(this) }
    observe() {}
    disconnect() { this.disconnected = true }
  }
  const restore = ['ResizeObserver', 'IntersectionObserver', 'MutationObserver'].map(key => {
    const original = Object.getOwnPropertyDescriptor(globalThis, key)
    Object.defineProperty(globalThis, key, { value: key === 'MutationObserver' ? VisibilityObserver : Observer, configurable: true })
    return () => original ? Object.defineProperty(globalThis, key, original) : Reflect.deleteProperty(globalThis, key)
  })
  t.after(() => { detachGlassLens(host as unknown as HTMLElement); restore.forEach(reset => reset()) })
  attachGlassLens(host as unknown as HTMLElement)
  assert.equal(host.children.length, 1)
  assert.equal(body.children.length, 0, 'invisible hosts allocate no filter')
  observers[1].callback([{ isIntersecting: true }])
  assert.equal(host.dataset.glassLens, 'svg')
  assert.equal(body.children.length, 1)
  host.setAttribute('aria-hidden', 'true')
  mutations[0].callback()
  assert.equal(body.children.length, 0, 'idle reader chrome releases the filter even inside the viewport')
  host.setAttribute('aria-hidden', 'false')
  mutations[0].callback()
  assert.equal(body.children.length, 1)
  const image = body.children[0].children[0].children[0].children[0]
  assert.equal(image.attrs.get('x'), '0', 'the displacement map aligns to the host, not the expanded filter region')
  assert.equal(image.attrs.get('y'), '0')
  preferences[0].matches = true
  preferences[0].listeners.forEach(notify => notify())
  assert.equal(body.children.length, 0)
  assert.equal(host.dataset.glassLens, 'css')
  preferences[0].matches = false
  preferences[0].listeners.forEach(notify => notify())
  assert.equal(body.children.length, 1)
  observers[1].callback([{ isIntersecting: false }])
  assert.equal(body.children.length, 0)
  attachGlassLens(host as unknown as HTMLElement)
  assert.equal(host.children.length, 1)
  assert.ok(observers.slice(0, 2).every(observer => observer.disconnected))
  observers[2].callback([]) // schedule a pending resize, then leave the route
  detachGlassLens(host as unknown as HTMLElement)
  assert.equal(host.children.length, 0)
  assert.equal(body.children.length, 0)
  assert.equal(host.dataset.glassLens, undefined)
  assert.ok(observers.every(observer => observer.disconnected))
  assert.ok(mutations.every(observer => observer.disconnected))
  assert.ok(preferences.every(preference => preference.listeners.size === 0))
  await new Promise(resolve => setTimeout(resolve, 170))
  assert.equal(body.children.length, 0, 'pending resize cannot resurrect a disposed lens')
})
