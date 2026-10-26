import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (file: string) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8')

test('home summary, information panels and system card use the common card material', async () => {
  const home = await source('views/Home.vue')
  const panel = await source('components/home/HomePanel.vue')
  assert.match(home, /class="attention-tile glass-surface"[^>]*data-glass="card"/)
  assert.match(home, /class="home-system glass-surface"[^>]*data-glass="card"/)
  assert.match(panel, /class="home-panel glass-surface"[^>]*data-glass="card"/)
  assert.doesNotMatch(panel, /v-glass-lens/)
})

test('all Reader and Task searches have explicit control optics instead of the nested flat override', async () => {
  const reader = await source('views/Reader.vue')
  const tasks = await source('views/Tasks.vue')
  assert.equal((reader.match(/data-glass="search"/g) || []).length, 3)
  assert.equal((tasks.match(/class="task-search glass-surface"[^>]*data-glass="search"/g) || []).length, 2)
  assert.doesNotMatch(await source('styles/materials.css'), /:is\(\.task-toolbar \.task-search, \.reader-search-field\)\s*\{/)
})

test('all Select triggers and teleported menus receive independent optical layers', async () => {
  const css = await source('styles/materials.css')
  assert.match(css, /\.el-select__wrapper[^{}]*::before/)
  assert.match(css, /\.el-popper:has\(\.el-select-dropdown\)/)
  assert.match(css, /\.el-select__wrapper[^{}]*::after/)
  assert.match(css, /--glass-control-surface-fill:/)
  assert.match(css, /--glass-menu-fill:/)
  assert.match(css, /\.el-select-dropdown__item\.is-selected/)
})

test('plain input paint no longer overwrites Select surface or focused state', async () => {
  const css = await source('styles/controls.css')
  const field = css.match(/:root\[data-theme\] :is\(\.el-input__wrapper, \.el-select__wrapper, \.el-range-editor, \.el-textarea__inner\)\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(field, /min-height:/)
  assert.doesNotMatch(field, /background:|box-shadow:/)
  assert.match(css, /--glass-control-surface-edge:\s*var\(--glass-danger-label\)/)
})

test('new material fills become opaque without changing foreground filters or adding frame observers', async () => {
  const css = await source('styles/materials.css')
  for (const signal of ['@supports not', '@media (prefers-reduced-transparency: reduce)', '@media (forced-colors: active)']) {
    const block = css.slice(css.indexOf(signal))
    for (const token of ['glass-structural-fill', 'glass-control-surface-fill', 'glass-menu-fill']) {
      assert.match(block, new RegExp(`--${token}:\\s*(?:var\\(--glass-solid\\)|Canvas)`), signal)
    }
  }
  assert.doesNotMatch(await source('utils/glassOptics.ts'), /addEventListener\(['"]scroll|requestAnimationFrame|cloneNode/)
})
