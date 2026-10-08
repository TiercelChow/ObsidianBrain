import { scenarios } from './demoPlayback.mjs'

// Bounding rectangles include the surrounding section's reveal transform.
// Reserve the final layout dimensions even while that reveal is in flight.
const layoutHeight = element => Math.max(element.scrollHeight, parseFloat(getComputedStyle(element).height) || element.offsetHeight)

// Measure the example states once per width, not on every animation frame.
export function preserveDemoLayout(host, panels) {
  const stage = host.querySelector('.showcase-stage')
  const detail = host.querySelector('[data-demo-detail]')
  let width = 0
  let frame = 0

  function measure() {
    frame = 0
    width = stage.clientWidth
    const active = panels.find(panel => !panel.hidden)
    if (!active || !width) return
    const previewWidth = active.querySelector('[data-demo-viewport]').clientWidth + 2
    const copyWidth = getComputedStyle(active.querySelector('.showcase-copy')).width
    const measuring = document.createElement('div')
    measuring.className = 'demo-measure'
    measuring.setAttribute('aria-hidden', 'true')
    measuring.inert = true
    measuring.style.width = `${previewWidth}px`
    stage.append(measuring)
    let previewHeight = 0
    let copyHeight = 0
    try {
      const description = detail.cloneNode(true)
      description.style.minHeight = '0'
      description.style.width = `${detail.clientWidth}px`
      measuring.append(description)
      let descriptionHeight = 0
      Object.values(scenarios).flat().forEach(step => {
        description.textContent = step.detail
        descriptionHeight = Math.max(descriptionHeight, layoutHeight(description))
      })
      detail.style.minHeight = `${Math.ceil(descriptionHeight)}px`
      description.remove()
      panels.forEach(panel => {
        const preview = panel.querySelector('[data-demo-viewport]').cloneNode(true)
        preview.style.minHeight = '0'
        measuring.append(preview)
        scenarios[panel.dataset.featurePanel].forEach((_, index) => {
          preview.querySelectorAll('[data-demo-view]').forEach(view => {
            view.hidden = !view.dataset.demoView.split(' ').includes(String(index))
          })
          previewHeight = Math.max(previewHeight, preview.scrollHeight + 2, layoutHeight(preview))
        })
        preview.remove()
        const copy = panel.querySelector('.showcase-copy').cloneNode(true)
        copy.style.minHeight = '0'
        copy.style.width = copyWidth
        measuring.append(copy)
        copyHeight = Math.max(copyHeight, layoutHeight(copy))
        copy.remove()
      })
      panels.forEach(panel => {
        panel.querySelector('[data-demo-viewport]').style.minHeight = `${Math.ceil(previewHeight)}px`
        panel.querySelector('.showcase-copy').style.minHeight = `${Math.ceil(copyHeight)}px`
      })
    } finally { measuring.remove() }
  }

  measure()
  const observer = 'ResizeObserver' in window ? new ResizeObserver(() => {
    if (stage.clientWidth === width) return
    if (frame) cancelAnimationFrame(frame)
    frame = requestAnimationFrame(measure)
  }) : null
  observer?.observe(stage)
  return () => { observer?.disconnect(); if (frame) cancelAnimationFrame(frame) }
}

// Incoming and outgoing frames overlap; no fade-out-then-fade-in gap.
export function createDemoMotion(motion) {
  const layers = new Set()
  const incoming = new Map()

  function release(layer) {
    layer.animation?.cancel()
    layer.node.remove()
    layers.delete(layer)
  }

  function capture(source, parent) {
    if (motion.matches || typeof source?.animate !== 'function') return null
    const rect = source.getBoundingClientRect()
    const origin = parent.getBoundingClientRect()
    const snapshot = source.cloneNode(true)
    const originals = [source, ...source.querySelectorAll('*')]
    const copies = [snapshot, ...snapshot.querySelectorAll('*')]
    originals.forEach((element, index) => {
      const copy = copies[index]
      copy.removeAttribute('id')
      if (!element.getAnimations().length) return
      const live = getComputedStyle(element)
      copy.style.opacity = live.opacity
      copy.style.transform = live.transform
    })
    const opacity = getComputedStyle(source).opacity
    snapshot.removeAttribute('data-feature-panel')
    snapshot.setAttribute('aria-hidden', 'true')
    snapshot.inert = true
    snapshot.classList.add('demo-transition-layer')
    Object.assign(snapshot.style, { left: `${rect.left - origin.left}px`, top: `${rect.top - origin.top}px`, width: `${rect.width}px`, height: `${rect.height}px`, minHeight: '0', opacity })
    return { node: snapshot, parent, opacity }
  }

  function transition(snapshot, target, sceneChange) {
    if (!snapshot) return
    const duration = sceneChange ? 280 : 240
    const options = { duration, easing: 'cubic-bezier(.22, .68, 0, 1)' }
    snapshot.parent.append(snapshot.node)
    layers.add(snapshot)
    // Keep rapid manual input bounded without locking any controls.
    if (layers.size > 4) release(layers.values().next().value)
    snapshot.animation = snapshot.node.animate([{ opacity: snapshot.opacity }, { opacity: 0 }], options)
    snapshot.animation.onfinish = () => release(snapshot)
    incoming.get(target)?.cancel()
    const animation = target.animate([{ opacity: 0 }, { opacity: 1 }], options)
    incoming.set(target, animation)
    animation.onfinish = () => { if (incoming.get(target) === animation) incoming.delete(target) }
  }

  function clear() {
    incoming.forEach(animation => animation.cancel())
    incoming.clear()
    ;[...layers].forEach(release)
  }
  motion.addEventListener('change', clear)
  return { capture, transition, dispose() { clear(); motion.removeEventListener('change', clear) } }
}
