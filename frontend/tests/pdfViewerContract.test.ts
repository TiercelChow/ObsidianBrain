import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

const viewer = readFileSync(new URL('../src/components/reader/PdfViewer.vue', import.meta.url), 'utf8')
const reader = readFileSync(new URL('../src/views/Reader.vue', import.meta.url), 'utf8')

test('PDF detail rendering is viewport-bounded, serialised and cancelled on scroll/unmount', () => {
  assert.match(viewer, /computePdfDetailRegion\(rect/)
  assert.match(viewer, /if \(detailRendering\) return/)
  assert.match(viewer, /if \(activeRenders \|\| detailRendering\)/)
  assert.match(viewer, /off\.width = 1\s+off\.height = 1/)
  assert.match(viewer, /function onPdfScroll\(\) \{\s+cancelDetail\(\)/)
  assert.match(viewer, /request !== detailGeneration \|\| pageWork\.get\(num\) !== work/)
  assert.match(viewer, /visiblePages\.delete\(num\)\s+releaseDetail\(num\)/)
})

test('PDF touch zoom is local, continuous and removes every listener', () => {
  for (const type of ['scroll', 'touchstart', 'touchmove', 'touchend', 'touchcancel']) {
    assert.ok(viewer.includes(`addEventListener('${type}'`))
    assert.ok(viewer.includes(`removeEventListener('${type}'`))
  }
  assert.match(viewer, /lastTouchZoomAt < 600/)
  assert.match(viewer, /computePdfZoomAnchorScroll/)
  assert.match(viewer, /touch-action: pan-x pan-y/)
  assert.doesNotMatch(viewer, /zoomMode\.value !== 'fit'/)
})

test('phone PDF reserves only one scroller, keeps return controls and permits horizontal pan', () => {
  assert.match(reader, /is-pdf-reading \.pane-center \{ overflow-x: auto/)
  assert.match(reader, /is-read-view \{\s+height: 100%;/)
  assert.match(reader, /@zoomchange="pdfZoomRatio = \$event"/)
  assert.match(reader, /nextPdfZoomRatio\(pdfZoomRatio.value, direction\)/)
  assert.doesNotMatch(reader, /is-pdf-reading \.reader-topbar \{[^}]*display:\s*none/)
})

test('default reading focus is bounded and restoration follows mounted page placeholders', () => {
  assert.match(viewer, /Promise\.race/)
  assert.match(viewer, /window\.setTimeout\(\(\) => resolve\(null\), 120\)/)
  assert.match(viewer, /await nextTick\(\)[\s\S]*emit\('pagecount', doc.numPages\)/)
  assert.match(viewer, /root\.scrollTo\(\{ top:/)
  assert.match(viewer, /box-sizing: content-box/)
})
