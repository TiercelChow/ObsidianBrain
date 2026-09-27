import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('four main destinations and an accessible orb share one navigation component', async () => {
  const dock = await source('components/MobileDock.vue')
  assert.equal((dock.match(/class="mobile-dock-item"/g) || []).length, 4)
  assert.match(dock, /aria-label="展开主要导航"/)
  assert.match(dock, /:inert="compact \? true : undefined"/)
  assert.match(dock, /prefers-reduced-transparency/)
  assert.match(dock, /prefers-reduced-motion/)
})

test('navigation reserve is stable and does not depend on scroll progress', async () => {
  const app = await source('App.vue')
  assert.match(app, /\.has-mobile-subnav \{ --mobile-navigation-height: calc\(var\(--mobile-dock-height\) \+ var\(--mobile-sub-dock-height\) \+ 8px\)/)
  assert.doesNotMatch(app, /--mobile-navigation-height[^;\n]*dockProgress/)
  assert.match(app, /isMobile && !mobileFocusMode && !dockKeyboardOpen/)
})

test('scroll observation requires a user gesture and filters overlays', async () => {
  const motion = await source('composables/useMobileDockMotion.ts')
  assert.match(motion, /event\.target !== scroller/)
  assert.match(motion, /motion-overlay-open/)
  assert.match(motion, /boundedScrollTop\(/)
  assert.match(motion, /removeEventListener\('scroll', scroll, true\)/)
})

test('sub-navigation ownership cleans up without erasing a successor route', async () => {
  const [nav, tasks, knowledge] = await Promise.all([source('composables/useMobileSubnav.ts'), source('views/Tasks.vue'), source('components/knowledge/KnowledgePageShell.vue')])
  assert.match(nav, /current\.value\?\.owner === owner/)
  assert.match(tasks, /select: \(\) => changeView\(mode\)/)
  assert.match(knowledge, /useMobileSubnav\(/)
  assert.match(knowledge, /active: route\.path === item\.path/)
})

test('compact dock and orb keep the original bottom dock height and footprint', async () => {
  const dock = await source('components/MobileDock.vue')
  assert.match(dock, /--dock-orb-size:\s*var\(--mobile-dock-height\)/)
  assert.match(dock, /\.mobile-dock\s*\{[^}]*height:\s*var\(--mobile-dock-height\)/)
  assert.match(dock, /\.mobile-sub-dock\s*\{[^}]*bottom:\s*0;/)
  assert.match(dock, /--mobile-dock-height\) - var\(--mobile-sub-dock-height\)\) \* var\(--dock-settle-progress\)/)
})

test('scroll retargets the running spring instead of jumping to the gesture position', async () => {
  const motion = await source('composables/useMobileDockMotion.ts')
  assert.match(motion, /stepDockSpring\(/)
  assert.match(motion, /if \(frame\) return/)
  assert.doesNotMatch(motion, /progress\.value = gesture\.progress/)
})

test('endpoint elasticity remains visible and reduced motion removes deformation', async () => {
  const [motion, dock] = await Promise.all([source('composables/useMobileDockMotion.ts'), source('components/MobileDock.vue')])
  assert.match(motion, /progress\.value = state\.value/)
  assert.match(dock, /dockRebound\(props\.progress\)/)
  assert.match(dock, /--dock-rebound/)
  assert.match(dock, /transform-origin:\s*right bottom/)
  assert.match(dock, /prefers-reduced-motion: reduce\)[\s\S]*--dock-rebound:\s*0\s*!important/)
  assert.doesNotMatch(dock, /@keyframes/)
})

test('one persistent glass capsule retracts into the orb without an early circle cross-fade', async () => {
  const dock = await source('components/MobileDock.vue')
  assert.match(dock, /--dock-travel-x': geometry\.travelX/)
  assert.match(dock, /--dock-travel-y': geometry\.travelY/)
  assert.match(dock, /class="mobile-dock dock-glass"/)
  assert.doesNotMatch(dock, /mobile-dock-orb-surface|mobile-dock-surface|mobile-dock-controls/)
  assert.doesNotMatch(dock, /\.mobile-dock\s*\{[^}]*opacity:/)
  assert.match(dock, /\.mobile-dock\s*\{[^}]*overflow:\s*hidden/)
})
