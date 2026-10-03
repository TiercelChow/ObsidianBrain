import assert from 'node:assert/strict'
import test from 'node:test'
import { panelDragPosition, presentationOffset } from '../src/utils/panelGesture.ts'

test('regrabbing a settling panel preserves its current on-screen offset', () => {
  assert.equal(panelDragPosition(82, 400, 400, 600), 82)
  assert.equal(panelDragPosition(82, 410, 400, 600), 92)
  assert.equal(panelDragPosition(-82, 390, 400, 600, -1), -92)
})
test('reverse movement crosses the open position continuously with resistance', () => {
  const crossing = panelDragPosition(20, 370, 400, 600)
  assert.ok(crossing < 0 && crossing > -10)
  assert.equal(panelDragPosition(20, 380, 400, 600), 0)
  assert.equal(panelDragPosition(20, 380, 400, 600, 1), 0)
})
test('presentation matrices preserve entry and interrupted spring positions', () => {
  assert.equal(presentationOffset('matrix(1, 0, 0, 1, 0, 120)', 'y'), 120)
  assert.equal(presentationOffset('matrix3d(1,0,0,0,0,1,0,0,0,0,1,0,-82,40,0,1)', 'x'), -82)
  assert.equal(presentationOffset('none', 'y'), 0)
  assert.equal(presentationOffset('invalid', 'x', 25), 25)
})
