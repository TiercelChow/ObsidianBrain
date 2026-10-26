import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (file: string) => readFile(new URL(`../src/styles/${file}`, import.meta.url), 'utf8')

test('every Select menu has a full rounded, clipped shell regardless of custom popper class', async () => {
  const controls = await source('controls.css')
  const shell = controls.match(/:root\[data-theme\] \.el-popper:has\(\.el-select-dropdown\)\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(shell, /border-radius:\s*14px\s*!important/)
  assert.match(shell, /overflow:\s*hidden/)
  assert.match(controls, /\.el-popper:has\(\.el-select-dropdown\) \.el-select-dropdown__list\s*\{[^}]*padding:\s*5px\s*!important/)
  assert.match(controls, /\.el-popper:has\(\.el-select-dropdown\) \.el-select-dropdown__item\s*\{[^}]*border-radius:\s*9px/)
})

test('attached menus never square off either edge when Popper flips its placement', async () => {
  const motion = await source('motion.css')
  assert.doesNotMatch(motion, /border-radius:\s*(?:0\s+0\s+14px\s+14px|14px\s+14px\s+0\s+0)/)
  assert.doesNotMatch(motion, /\.system-select-popper\.el-popper\[data-popper-placement/)
  assert.match(motion, /\.system-select-popper\.el-popper\s*\{[^}]*margin:\s*0\s*!important/)
})
