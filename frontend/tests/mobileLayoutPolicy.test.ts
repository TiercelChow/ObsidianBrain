import assert from 'node:assert/strict'
import test from 'node:test'

import {
  getPdfRenderPolicy,
  isPhoneViewport,
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
    maxRenderDpr: 1.5,
  })
})

test('getPdfRenderPolicy keeps desktop rendering responsive', () => {
  assert.deepEqual(getPdfRenderPolicy(1440, 8), {
    renderMarginPx: 700,
    maxConcurrentRenders: 2,
    maxRenderDpr: 2,
  })
})

test('getPdfRenderPolicy avoids parallel canvas work on low-core devices', () => {
  assert.equal(getPdfRenderPolicy(1024, 4).maxConcurrentRenders, 1)
})
