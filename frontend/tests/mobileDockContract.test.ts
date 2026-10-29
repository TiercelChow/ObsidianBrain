import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('five direct destinations replace the module menu while preserving the accessible orb', async () => {
  const dock = await source('components/MobileDock.vue')
  assert.equal((dock.match(/class="mobile-dock-item"/g) || []).length, 5)
  const labels = [...dock.matchAll(/<span>(首页|阅境轩|时光机|任务中枢|Wiki)<\/span>/g)].map(match => match[1])
  assert.deepEqual(labels, ['首页', '阅境轩', '时光机', '任务中枢', 'Wiki'])
  assert.match(dock, /grid-template-columns: repeat\(5, minmax\(0, 1fr\)\)/)
  assert.match(dock, /to="\/knowledge"[^>]*class="mobile-dock-item"/)
  assert.doesNotMatch(dock, /全部|openModules|sidebarVisible|section === 'more'/)
  assert.match(dock, /home: House/)
  assert.match(dock, /wiki: Notebook/)
  assert.match(dock, /aria-label="展开主要导航"/)
  assert.match(dock, /:inert="compact \? true : undefined"/)
  assert.match(dock, /prefers-reduced-transparency/)
  assert.match(dock, /prefers-reduced-motion/)
})

test('phone shell has no hidden module menu and guards desktop-only routes on resize', async () => {
  const [app, router] = await Promise.all([source('App.vue'), source('router/index.ts')])
  assert.match(app, /<aside[\s\S]*?v-if="!isMobile"/)
  assert.doesNotMatch(app, /openMobileSidebar|mobileSidebarVisible|open-modules|expanded-on-mobile|mobile-overlay/)
  assert.match(app, /getMobileRouteRedirect\(route\.path, windowWidth\.value\)/)
  assert.match(router, /router\.beforeEach\(/)
  assert.match(router, /getMobileRouteRedirect\(to\.path, window\.innerWidth\)/)
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
  assert.match(tasks, /select: \(\) => changeView\(viewMode\.value === 'tasks' \? 'calendar' : 'tasks'\)/)
  assert.match(knowledge, /useMobileSubnav\(/)
  assert.match(knowledge, /active: route\.path === item\.path/)
})

test('icon-only task dock distributes actions evenly without changing the shared motion geometry', async () => {
  const [tasks, dock] = await Promise.all([source('views/Tasks.vue'), source('components/MobileDock.vue')])
  assert.match(tasks, /layout: 'icons'/)
  assert.match(dock, /'icon-layout': displayedSubnav\?\.layout === 'icons'/)
  assert.match(dock, /\.mobile-sub-dock\.icon-layout \.mobile-sub-dock-item\s*\{\s*flex: 1 1 0; min-width: 44px;/)
})

test('task icon feedback keeps the same rounded rectangle as Wiki navigation, not a stretched circle', async () => {
  const [dock, materials] = await Promise.all([source('components/MobileDock.vue'), source('styles/materials.css')])
  const quietActions = materials.slice(materials.indexOf('/* Borderless icon actions:'), materials.indexOf(':root[data-theme] .image-viewer-overlay'))
  assert.doesNotMatch(quietActions, /\.mobile-sub-dock-item/)
  assert.match(dock, /\.mobile-sub-dock-item\s*\{[^}]*min-height:\s*44px;[^}]*border:\s*0;[^}]*border-radius:\s*14px;/)
  assert.match(dock, /\.mobile-sub-dock-item:active\s*\{[^}]*background:\s*color-mix/)
  assert.doesNotMatch(dock, /\.mobile-sub-dock-item\.is-icon\s*\{[^}]*border-radius/)
})

test('compact dock and orb keep the original bottom dock height and footprint', async () => {
  const dock = await source('components/MobileDock.vue')
  assert.match(dock, /--dock-orb-size:\s*var\(--mobile-dock-height\)/)
  assert.match(dock, /\.mobile-dock\s*\{[^}]*height:\s*var\(--mobile-dock-height\)/)
  assert.match(dock, /\.mobile-auxiliary-surface\s*\{[^}]*bottom:\s*0;/)
  const layout = await source('utils/dockAuxiliaryMotion.ts')
  assert.match(layout, /height: 50 \+ 12 \* geometry.sink/)
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
  assert.match(dock, /dockRebound\(effectiveProgress\.value\)/)
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

test('both dock surfaces retain semicircular pill ends throughout their height and width changes', async () => {
  const dock = await source('components/MobileDock.vue')
  const material = dock.match(/\.dock-glass\s*\{([^}]+)\}/)?.[1] ?? ''
  assert.match(material, /border-radius:\s*999px;/)
  for (const selector of ['mobile-dock', 'mobile-sub-dock']) {
    const geometry = dock.match(new RegExp(`\\.${selector}\\s*\\{([^}]+)\\}`))?.[1] ?? ''
    assert.doesNotMatch(geometry, /border-radius:/, `${selector} keeps the shared pill contour rather than a separate rounded rectangle`)
  }
})
