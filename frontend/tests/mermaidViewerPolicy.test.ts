import assert from 'node:assert/strict'
import test from 'node:test'

import { getMermaidViewerPolicy } from '../src/utils/mermaidViewerPolicy.ts'

test('mobile mermaid viewer uses touch instructions and a floating close control', () => {
  assert.deepEqual(getMermaidViewerPolicy(390), {
    mobile: true,
    hint: '双指缩放 · 单指拖动 · 双击放大',
    floatingClose: true,
  })
})

test('desktop mermaid viewer keeps pointer and keyboard instructions', () => {
  assert.deepEqual(getMermaidViewerPolicy(1440), {
    mobile: false,
    hint: '滚轮缩放 · 拖拽平移 · 双击放大 · Esc 关闭',
    floatingClose: false,
  })
})
