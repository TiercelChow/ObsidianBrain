import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { compileStyle, parse } from '@vue/compiler-sfc'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('phone primary actions register with one persistent circular-to-pill floating surface', async () => {
  const [registration, action, tokens] = await Promise.all([source('components/MobilePageAction.vue'), source('components/MobileDock.vue'), source('styles/tokens.css')])
  assert.match(registration, /useMobilePageAction\(/)
  assert.doesNotMatch(registration, /<button/)
  assert.match(action, /v-glass-lens="'css'"[\s\S]*?class="mobile-auxiliary-surface dock-glass"[\s\S]*?data-glass="floating"/)
  assert.match(action, /:aria-label="displayedAction.label"/)
  assert.match(action, /:title="displayedAction.label"/)
  assert.match(action, /<component :is="displayedAction.icon"[^>]*aria-hidden="true"/)
  for (const dimension of ['width', 'height']) assert.match(action, new RegExp(`${dimension}: var\\(--mobile-page-action-size\\);`))
  assert.match(action, /border-radius: 999px;/)
  assert.match(tokens, /--mobile-page-action-size: 56px;/)
  assert.match(tokens, /--mobile-page-action-gap: 12px;/)
  assert.doesNotMatch(action, /<button[^>]*data-icon-only[^>]*class="mobile-page-action"/)
})

test('floating actions respond on press without moving their anchor and respect reduced motion and keyboard visibility', async () => {
  const [action, app] = await Promise.all([source('components/MobileDock.vue'), source('App.vue')])
  assert.match(action, /:active\s*\{[^}]*transform: scale\(\.94\)/)
  assert.match(action, /transition-duration: 70ms/)
  assert.match(action, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?transform: none;[\s\S]*?transition: none;/)
  assert.match(app, /isMobile && !mobileFocusMode && !dockKeyboardOpen/)
  assert.match(app, /'mobile-keyboard-open': dockKeyboardOpen/)
  assert.match(app, /\.mobile-keyboard-open :deep\(\.mobile-page-action\)\s*\{[^}]*visibility: hidden;[^}]*pointer-events: none;/)
})

test('compiled modal visibility rules hide only the floating action, never the document or dialog', async () => {
  const action = await source('components/MobileDock.vue')
  const { descriptor } = parse(action)
  const compiled = compileStyle({ source: descriptor.styles[0].content, filename: 'MobilePageAction.vue', id: 'data-v-test', scoped: true })
  assert.deepEqual(compiled.errors, [])
  assert.match(compiled.code, /html\.motion-overlay-open \.mobile-auxiliary-surface.is-page-action(?:\[data-v-test\])?\s*\{[^}]*visibility: hidden;/)
  assert.doesNotMatch(compiled.code, /html\.motion-overlay-open\s*\{/)
})

test('route handoff preserves surface identity and prevents stale page actions from being invoked', async () => {
  const [dock, registry, subnav, spring] = await Promise.all([
    source('components/MobileDock.vue'), source('composables/useMobilePageAction.ts'), source('composables/useMobileSubnav.ts'), source('composables/useDockAuxiliaryMotion.ts'),
  ])
  assert.match(dock, /v-show="!!presentation"[\s\S]*?class="mobile-auxiliary-surface dock-glass"/)
  assert.match(dock, /:inert="!action \|\| subnav\?\.items.length \? true : undefined"/)
  assert.match(dock, /:inert="!subnav\?\.items.length \? true : undefined"/)
  for (const scoped of [registry, subnav]) {
    assert.match(scoped, /current\.value\?\.owner === owner/)
    assert.match(scoped, /current\.value\?\.path === route.path/)
  }
  assert.match(spring, /if \(animation\) return/)
  assert.match(spring, /reduced.addEventListener\('change'/)
  assert.match(spring, /cancelAnimationFrame\(animation\); reduced.removeEventListener/)
})

test('floating action pages leave enough trailing space to reach content behind the button', async () => {
  for (const path of ['views/Home.vue', 'views/Reader.vue', 'views/Timeline.vue']) {
    const page = await source(path)
    assert.match(page, /padding-bottom: var\(--mobile-page-action-clearance\);/, path)
  }
  const reader = await source('views/Reader.vue')
  // The shell already subtracts header, Dock and safe areas. A second viewport
  // calculation pushes the shelf past that content box and under the controls.
  assert.match(reader, /@media \(max-width: 768px\)[\s\S]*?\.reader-page\s*\{\s*height: 100%;\s*min-height: 0;/)
})
