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
  const [nav, tasks, inspiration] = await Promise.all([source('composables/useMobileSubnav.ts'), source('views/Tasks.vue'), source('views/Inspiration.vue')])
  assert.match(nav, /current\.value\?\.owner === owner/)
  assert.match(tasks, /select: \(\) => changeView\(mode\)/)
  assert.match(inspiration, /selectedMode\.value = mode\.value/)
})
