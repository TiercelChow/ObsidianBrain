import assert from 'node:assert/strict'
import test from 'node:test'
import { freezeCollectionLeave, resetCollectionEnter } from '../src/utils/collectionMotion.ts'

test('leaving tiles retain their slot geometry and stop receiving input', () => {
  const previous = globalThis.HTMLElement
  class Tile {
    offsetWidth = 118; offsetHeight = 158; offsetTop = 176; offsetLeft = 128; inert = false
    style = {removeProperty(name:string) {delete this[name]}}
  }
  globalThis.HTMLElement = Tile as any
  try {
    const tile = new Tile()
    freezeCollectionLeave(tile as any)
    assert.equal(tile.inert, true)
    assert.equal(tile.style.width, '118px')
    assert.equal(tile.style.height, '158px')
    assert.equal(tile.style.top, '176px')
    assert.equal(tile.style.left, '128px')
    resetCollectionEnter(tile as any)
    assert.equal(tile.inert, false)
    for (const property of ['width','height','top','left']) assert.equal(tile.style[property], undefined)
  } finally {globalThis.HTMLElement = previous}
})

test('non HTML transition targets are ignored safely', () => {
  const previous = globalThis.HTMLElement
  globalThis.HTMLElement = class {} as any
  try {
    assert.doesNotThrow(() => freezeCollectionLeave({} as any))
    assert.doesNotThrow(() => resetCollectionEnter({} as any))
  } finally {globalThis.HTMLElement = previous}
})
