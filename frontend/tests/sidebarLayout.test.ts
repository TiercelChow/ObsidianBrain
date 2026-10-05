import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const sidebar = () => readFile(new URL('../src/components/Sidebar.vue', import.meta.url), 'utf8')

test('sidebar icons, logo and toggle share an invariant icon rail', async () => {
  const source = await sidebar()
  assert.match(source, /--sidebar-icon-track:/)
  for (const cls of ['logo-section', 'nav-item', 'collapse-btn']) {
    const style = source.match(new RegExp(`\\.${cls}\\s*\\{([^}]+)\\}`))?.[1] || ''
    assert.match(style, /grid-template-columns:\s*var\(--sidebar-icon-track\)\s+minmax\(0,\s*1fr\)/, cls)
  }
  assert.doesNotMatch(source, /\.sidebar\.collapsed\s+\.(?:nav-item|collapse-btn|logo-section)\s*\{[^}]*justify-content:\s*center/)
})

test('collapsing fades labels in place instead of removing layout participants', async () => {
  const source = await sidebar()
  assert.doesNotMatch(source, /v-show="!isCollapsed"/)
  assert.doesNotMatch(source, /<transition name="(?:logo-text|nav-label)"/)
  assert.match(source, /\.nav-group-label\s*\{[^}]*height:\s*28px/)
  assert.match(source, /:aria-expanded="!isCollapsed"/)
  assert.match(source, /:aria-label="item.label"/)
  assert.match(source, /ref="navListRef"/)
})

test('desktop rail preserves its layout without obsolete mobile module-sheet controls', async () => {
  const source = await sidebar()
  assert.doesNotMatch(source, /v-if="expandedOnMobile"/)
  assert.match(source, /@media \(min-width: 769px\)/)
  assert.doesNotMatch(source, /mobile-sheet|mobile-sidebar-close|expandedOnMobile/)
  assert.match(source, /prefers-reduced-motion:\s*reduce/)
})
