/** Clear-center lens. The map contains geometry only, never document pixels. */
export function buildLensMap(width: number, height: number, radius: number) {
  const safe = (value: number) => Number.isFinite(value) ? Math.max(1, value) : 1
  const w = safe(width)
  const h = safe(height)
  const r = Math.min(Math.max(0, Number.isFinite(radius) ? radius : 0), w / 2, h / 2)
  const sampling = Math.min(1, 512 / w, 160 / h)
  const mapWidth = Math.max(1, Math.round(w * sampling))
  const mapHeight = Math.max(1, Math.round(h * sampling))
  const pixels = new Uint8ClampedArray(mapWidth * mapHeight * 4)
  const band = Math.min(10, h / 3, w / 3)
  for (let y = 0; y < mapHeight; y++) {
    for (let x = 0; x < mapWidth; x++) {
      const px = (x + .5) * w / mapWidth - w / 2
      const py = (y + .5) * h / mapHeight - h / 2
      const qx = Math.abs(px) - (w / 2 - r)
      const qy = Math.abs(py) - (h / 2 - r)
      const ox = Math.max(qx, 0)
      const oy = Math.max(qy, 0)
      const outside = Math.hypot(ox, oy)
      const depth = r - outside - Math.min(Math.max(qx, qy), 0)
      let nx = 0
      let ny = 0
      if (outside > 0) {
        nx = Math.sign(px) * ox / outside
        ny = Math.sign(py) * oy / outside
      } else if (qx > qy) nx = Math.sign(px)
      else ny = Math.sign(py)
      // The edge samples inward; the center is exactly neutral. No turbulence.
      const bend = depth >= 0 && depth < band ? .36 * (1 - depth / band) ** 2 : 0
      const offset = (y * mapWidth + x) * 4
      pixels[offset] = 128 - nx * bend * 255
      pixels[offset + 1] = 128 - ny * bend * 255
      pixels[offset + 2] = 128
      pixels[offset + 3] = 255
    }
  }
  return { width: mapWidth, height: mapHeight, pixels }
}

/** CSS.supports accepts url() in engines that cannot render SVG backdrops.
 * Keep enhancement conservative until WebKit/Gecko have verified support. */
export function canUseSvgBackdrop(userAgent: string) {
  return /(?:Chrome|Chromium)\/\d+/.test(userAgent) && !/(?:iPhone|iPad|iPod|CriOS|FxiOS)/.test(userAgent)
}

let lensId = 0
const svgNamespace = 'http://www.w3.org/2000/svg'
const mounted = new WeakMap<HTMLElement, () => void>()

/** Attach to explicitly selected, positioned floating chrome only. */
export function attachGlassLens(host: HTMLElement, options: { mode?: 'auto' | 'css' } = {}) {
  mounted.get(host)?.()
  const doc = host.ownerDocument
  const view = doc.defaultView
  if (!view) return () => {}
  const layer = doc.createElement('span')
  layer.className = 'glass-optics'
  layer.setAttribute('aria-hidden', 'true')
  layer.style.setProperty('pointer-events', 'none')
  const edge = doc.createElement('span')
  edge.className = 'glass-optics__edge'
  layer.append(edge)
  host.prepend(layer)
  host.dataset.glassLens = 'css'

  // Open scroll edges need only CSS masks, not a closed lens map or observers.
  // Accessibility changes are already handled by the shared material CSS.
  if (options.mode === 'css') {
    let disposed = false
    const cleanup = () => {
      if (disposed) return
      disposed = true
      layer.remove()
      delete host.dataset.glassLens
      mounted.delete(host)
    }
    mounted.set(host, cleanup)
    return cleanup
  }

  const preferences = ['(prefers-reduced-transparency: reduce)', '(prefers-contrast: more)', '(forced-colors: active)'].map(query => view.matchMedia(query))
  const supported = canUseSvgBackdrop(view.navigator.userAgent) && view.CSS?.supports('backdrop-filter', 'url("#glass-probe")')
  let visible = false
  let disposed = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let filter: SVGFilterElement | undefined
  let svg: SVGSVGElement | undefined
  let mapImage: SVGFEImageElement | undefined
  let lastGeometry = ''

  function releaseFilter() {
    filter?.remove()
    svg?.remove()
    filter = undefined
    svg = undefined
    mapImage = undefined
    lastGeometry = ''
    layer.style.removeProperty('--glass-lens-filter')
    host.dataset.glassLens = 'css'
  }

  function update() {
    clearTimeout(timer)
    timer = undefined
    if (disposed) return
    if (!supported || !visible || host.getAttribute('aria-hidden') === 'true' || preferences.some(query => query.matches)) {
      releaseFilter()
      return
    }
    const width = host.offsetWidth - 2 * host.clientLeft
    const height = host.offsetHeight - 2 * host.clientTop
    if (width < 2 || height < 2) { releaseFilter(); return }
    const radius = Math.min(parseFloat(view!.getComputedStyle(host).borderTopLeftRadius) || 0, width / 2, height / 2)
    const geometry = `${width}:${height}:${Math.round(radius)}`
    if (geometry === lastGeometry) return
    const map = buildLensMap(width, height, radius)
    const canvas = doc.createElement('canvas')
    canvas.width = map.width
    canvas.height = map.height
    const context = canvas.getContext('2d')
    if (!context) { releaseFilter(); return }
    const data = context.createImageData(map.width, map.height)
    data.data.set(map.pixels)
    context.putImageData(data, 0, 0)
    try {
      const image = canvas.toDataURL('image/png')
      if (!filter) {
        const id = `glass-lens-${++lensId}`
        svg = doc.createElementNS(svgNamespace, 'svg')
        svg.classList.add('glass-optics-defs')
        svg.setAttribute('aria-hidden', 'true')
        svg.setAttribute('width', '0')
        svg.setAttribute('height', '0')
        const defs = doc.createElementNS(svgNamespace, 'defs')
        filter = doc.createElementNS(svgNamespace, 'filter')
        filter.id = id
        filter.setAttribute('color-interpolation-filters', 'sRGB')
        filter.setAttribute('x', '-10%')
        filter.setAttribute('y', '-10%')
        filter.setAttribute('width', '120%')
        filter.setAttribute('height', '120%')
        mapImage = doc.createElementNS(svgNamespace, 'feImage')
        mapImage.setAttribute('x', '0')
        mapImage.setAttribute('y', '0')
        mapImage.setAttribute('result', 'edge-map')
        mapImage.setAttribute('preserveAspectRatio', 'none')
        const displace = doc.createElementNS(svgNamespace, 'feDisplacementMap')
        displace.setAttribute('in', 'SourceGraphic')
        displace.setAttribute('in2', 'edge-map')
        displace.setAttribute('scale', '18')
        displace.setAttribute('xChannelSelector', 'R')
        displace.setAttribute('yChannelSelector', 'G')
        const blur = doc.createElementNS(svgNamespace, 'feGaussianBlur')
        blur.setAttribute('stdDeviation', '2.4')
        filter.append(mapImage, displace, blur)
        defs.append(filter)
        svg.append(defs)
        doc.body.append(svg)
        layer.style.setProperty('--glass-lens-filter', `url("#${id}") saturate(1.12)`)
      }
      mapImage?.setAttribute('width', String(width))
      mapImage?.setAttribute('height', String(height))
      mapImage?.setAttribute('href', image)
      host.dataset.glassLens = 'svg'
      lastGeometry = geometry
    } catch {
      // Canvas can be restricted by privacy settings; retain the CSS lens.
      releaseFilter()
    }
  }

  function schedule() {
    clearTimeout(timer)
    // Stretch the existing map with the capsule while it morphs. Rebuild its
    // rounded geometry only after settling, not on each spring frame.
    mapImage?.setAttribute('width', String(host.offsetWidth - 2 * host.clientLeft))
    mapImage?.setAttribute('height', String(host.offsetHeight - 2 * host.clientTop))
    // Do not rasterize at every frame of the existing Dock spring animation.
    timer = setTimeout(update, 140)
  }
  const resize = new ResizeObserver(schedule)
  const intersection = new IntersectionObserver(([entry]) => {
    visible = !!entry?.isIntersecting
    clearTimeout(timer)
    update()
  })
  resize.observe(host)
  intersection.observe(host)
  // Reader chrome hides while idle without leaving the viewport. Its explicit
  // aria-hidden state lets us release the SVG immediately, with no scroll work.
  const visibility = new MutationObserver(update)
  visibility.observe(host, { attributes: true, attributeFilter: ['aria-hidden'] })
  preferences.forEach(query => query.addEventListener('change', update))

  const cleanup = () => {
    if (disposed) return
    disposed = true
    clearTimeout(timer)
    resize.disconnect()
    intersection.disconnect()
    visibility.disconnect()
    preferences.forEach(query => query.removeEventListener('change', update))
    releaseFilter()
    layer.remove()
    delete host.dataset.glassLens
    mounted.delete(host)
  }
  mounted.set(host, cleanup)
  return cleanup
}

export function detachGlassLens(host: HTMLElement) { mounted.get(host)?.() }
