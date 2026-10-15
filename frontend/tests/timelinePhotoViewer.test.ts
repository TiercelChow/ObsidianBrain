import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/Timeline.vue', import.meta.url), 'utf8')

test('timeline photo uses one explicit transform owner with load/resize fit and local pointer anchors', async () => {
  const timeline = await source()
  assert.match(timeline, /createImageViewerMotion/)
  assert.match(timeline, /new ResizeObserver\(fitViewerPhoto\)/)
  assert.match(timeline, /event\.target !== viewerImgRef\.value/)
  assert.match(timeline, /viewerMotion\.fit\(img\?\.naturalWidth/)
  assert.match(timeline, /function openImageViewer[\s\S]*?viewerRetry\.value\+\+/)
  assert.match(timeline, /event\.clientX - rect\.left - rect\.width \/ 2/)
  assert.match(timeline, /\.viewer-image\s*\{[^}]*position: absolute;[^}]*transform-origin: center;/)
  assert.doesNotMatch(timeline, /viewerPz|from 'panzoom'|viewer-enter-from \.viewer-image|viewer-enter-active \.viewer-image/)
  const motionCss = await readFile(new URL('../src/styles/motion.css', import.meta.url), 'utf8')
  assert.doesNotMatch(motionCss, /\.viewer-enter-from \.viewer-image/)
})

test('photo swipe/pinch share captured pointers and only adjacent thumbnails are previewed', async () => {
  const timeline = await source()
  for (const event of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel', 'lostpointercapture']) assert.ok(timeline.includes(`@${event}=`), event)
  assert.match(timeline, /setPointerCapture\(event\.pointerId\)/)
  assert.match(timeline, /v-for="offset in \[-1, 1\]"[\s\S]*?:src="thumbnailUrl\(viewerNeighbor\(offset\)\)"/)
  assert.match(timeline, /\.viewer-image-wrap\s*\{[^}]*touch-action: none;/)
  assert.match(timeline, /viewerMotion\.dispose\(\)[\s\S]*viewerResizeObserver\?\.disconnect\(\)/)
  assert.match(timeline, /viewerFrame\.scale <= 1\.001/)
  assert.match(timeline, /prefers-reduced-motion: reduce/)
})
