import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')
const channel = (v: number) => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4
function luminance(hex: string) {
  const rgb = [0, 2, 4].map(i => channel(parseInt(hex.slice(i + 1, i + 3), 16) / 255))
  return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722
}
function contrast(a: string, b: string) {
  const values = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (values[0] + .05) / (values[1] + .05)
}

test('all text levels remain readable on the stable three-theme canvas', async () => {
  // Before token extraction this measures the actual old palette, rather than
  // failing merely because a new file has not yet been created.
  const app = await source('App.vue')
  const css = await source('styles/tokens.css').catch(() => app)
  for (const theme of ['light', 'dark', 'eye-care']) {
    const block = css.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)`))?.[1] || ''
    const color = (token: string) => block.match(new RegExp(`--${token}:\\s*(#[a-f\\d]{6})`, 'i'))?.[1] || ''
    assert.ok(color('bg-base'), theme)
    for (const token of ['text-primary', 'text-secondary', 'text-tertiary', 'text-muted', 'text-faint']) {
      assert.ok(contrast(color(token), color('bg-base')) >= 4.5, `${theme} ${token}: ${contrast(color(token), color('bg-base')).toFixed(2)}`)
    }
  }
})

test('shared theme tokens replace duplicate application palettes and own geometry', async () => {
  const [app, css] = await Promise.all([source('App.vue'), source('styles/tokens.css')])
  assert.match(app, /<style src="\.\/styles\/tokens\.css"><\/style>/)
  assert.doesNotMatch(app, /--text-faint:\s*#/)
  for (const token of ['page-gutter', 'radius-control', 'radius-content', 'radius-panel', 'control-height', 'font-ui']) assert.ok(css.includes(`--${token}:`), token)
  assert.match(app, /padding:\s*32px var\(--page-gutter\)/)
})

test('static cards are not treated as buttons and presses win over hover transforms', async () => {
  const motion = await source('styles/motion.css')
  assert.doesNotMatch(motion, /\.stat-card:active|\.el-card:active|\.memo-card-body:active/)
  assert.match(motion, /:not\(:active\):hover/)
})

test('switching from desktop to phone closes the inherited desktop navigation', async () => {
  const app = await source('App.vue')
  assert.match(app, /watch\(isMobile,[\s\S]+?if \(mobile\)\s*\{?\s*appStore\.setSidebarCollapsed\(true\)/)
})

test('shared controls own quiet fields, rounded focus, sizing and semantic state', async () => {
  const [app, css] = await Promise.all([source('App.vue'), source('styles/controls.css')])
  assert.ok(app.includes('<style src="./styles/controls.css"></style>'))
  for (const selector of ['.el-input__wrapper', '.el-select__wrapper', '.el-range-editor', '.el-textarea__inner', '.el-button', '.el-checkbox', '.el-switch']) assert.ok(css.includes(selector), selector)
  assert.ok(css.includes('var(--radius-control)'))
  assert.ok(css.includes('var(--control-height)'))
  assert.ok(css.includes(':focus-within'))
  assert.ok(!css.includes('backdrop-filter'), 'fields do not stack another glass sampling layer')
})

test('route transitions do not relocate fixed phone controls or leave old actions interactive', async () => {
  const app = await source('App.vue')
  for (const match of app.matchAll(/\.page-slide-(?:enter-from|leave-to)\s*\{([^}]+)\}/g)) {
    assert.ok(!match[1].includes('transform:'), 'a transformed page would become the fixed-control containing block')
  }
  assert.ok(app.includes('@before-leave="disableLeavingPage"'))
  assert.ok(app.includes('inert = true'))
})

test('theme switching remains reachable on Home after removing the phone module menu', async () => {
  const [sidebar, home] = await Promise.all([source('components/Sidebar.vue'), source('views/Home.vue')])
  assert.ok(sidebar.includes('class="collapse-btn theme-toggle"'))
  assert.ok(sidebar.includes('@click="appStore.toggleTheme()"'))
  assert.ok(home.includes('@click="appStore.toggleTheme()"'))
  assert.match(home, /:aria-label="`切换主题，当前\$\{themeName\}`"/)
})
