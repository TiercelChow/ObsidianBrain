import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('site and app share the actual palette instead of only matching optical recipes', async () => {
  const [css, main] = await Promise.all([read('src/style.css'), read('src/main.js')])
  assert.ok(css.includes("@import '../../frontend/src/styles/tokens.css'"))
  assert.doesNotMatch(css, /--text-faint:\s*#/)
  assert.ok(main.includes("getComputedStyle(root).getPropertyValue('--bg-base')"))
})

test('phone chrome meets touch targets without clipping the brand on 320px screens', async () => {
  const css = await read('src/style.css')
  assert.ok(css.includes('min-width: 44px'))
  assert.ok(css.includes('min-height: 44px'))
  assert.ok(css.includes('.header-actions { gap: .25rem; }'))
})

test('pressed feedback wins over pointer hover and reduced motion drops gestures', async () => {
  const css = await read('src/materials.css')
  assert.ok(css.includes(':not(:active):hover'))
  assert.ok(css.includes('transform: scale(.97)'))
  assert.ok(css.includes('prefers-reduced-motion'))
})
