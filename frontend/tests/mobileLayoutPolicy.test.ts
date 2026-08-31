import assert from 'node:assert/strict'
import test from 'node:test'

import {
  getPdfRenderPolicy,
  getMobileReaderToolbarState,
  isPhoneViewport,
  shouldLockMobileReaderOuterScroll,
} from '../src/utils/mobileLayoutPolicy.ts'

test('isPhoneViewport treats the shared 768px breakpoint as phone layout', () => {
  assert.equal(isPhoneViewport(320), true)
  assert.equal(isPhoneViewport(768), true)
  assert.equal(isPhoneViewport(769), false)
})

test('getPdfRenderPolicy reduces mobile pre-render work', () => {
  assert.deepEqual(getPdfRenderPolicy(390, 8), {
    renderMarginPx: 420,
    maxConcurrentRenders: 1,
    maxRenderDpr: 3,
    maxCanvasPixels: 4_000_000,
  })
})

test('getPdfRenderPolicy keeps desktop rendering responsive', () => {
  assert.deepEqual(getPdfRenderPolicy(1440, 8), {
    renderMarginPx: 700,
    maxConcurrentRenders: 2,
    maxRenderDpr: 2,
    maxCanvasPixels: 16_000_000,
  })
})

test('getPdfRenderPolicy avoids parallel canvas work on low-core devices', () => {
  assert.equal(getPdfRenderPolicy(1024, 4).maxConcurrentRenders, 1)
})

test('mobile reader toolbar stays pinned until the first document is selected', () => {
  assert.deepEqual(getMobileReaderToolbarState(true, false, false), {
    rendered: true,
    pinned: true,
    visible: true,
  })
})

test('mobile reader toolbar returns to transient behavior while reading', () => {
  assert.deepEqual(getMobileReaderToolbarState(true, true, false), {
    rendered: true,
    pinned: false,
    visible: false,
  })
  assert.equal(getMobileReaderToolbarState(true, true, true).visible, true)
})

test('mobile reader toolbar is not rendered before a folder is opened', () => {
  assert.deepEqual(getMobileReaderToolbarState(false, false, true), {
    rendered: false,
    pinned: false,
    visible: false,
  })
})

test('mobile reader owns vertical scrolling instead of chaining into the app shell', () => {
  assert.equal(shouldLockMobileReaderOuterScroll(390, '/reader'), true)
  assert.equal(shouldLockMobileReaderOuterScroll(768, '/reader'), true)
  assert.equal(shouldLockMobileReaderOuterScroll(769, '/reader'), false)
  assert.equal(shouldLockMobileReaderOuterScroll(390, '/timeline'), false)
})
