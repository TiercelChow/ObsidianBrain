import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('floating media icons keep a separate glass host while the buttons remain borderless', async () => {
  const timeline = await source('views/Timeline.vue')
  assert.match(timeline, /class="viewer-tool-surface viewer-close-surface"[^>]*data-glass="floating"/)
  assert.match(timeline, /class="viewer-tool-surface viewer-nav-surface viewer-prev"/)
  assert.match(timeline, /class="viewer-tool-surface viewer-nav-surface viewer-next"/)
  assert.match(timeline, /class="viewer-controls"[^>]*data-glass="floating"/)
  assert.match(timeline, /data-glass-tone="media"/)
  assert.match(timeline, /<button data-icon-only class="viewer-close"/)
  assert.match(await source('views/Reader.vue'), /class="fs-tool-surface"[^>]*data-glass="floating"/)
})

test('phone settings explicitly flatten their layout shell and use one content surface per card', async () => {
  const settings = await source('views/knowledge/WikiSettings.vue')
  assert.match(settings, /class="settings-main knowledge-surface"[^>]*:data-glass="isMobile \? 'flat' : 'card'"/)
  assert.equal((settings.match(/class="settings-card"[^>]*:data-glass="isMobile \? 'card' : 'flat'"/g) || []).length, 5)
  assert.match(await source('styles/materials.css'), /\[data-glass="flat"\]\s*\{[^}]*--material-fill:\s*transparent;[^}]*--material-filter:\s*none;[^}]*--material-shadow:\s*none;/)
})

test('segmented choices and moving indicators share paint without owning geometry or adding filters', async () => {
  for (const file of ['views/Tasks.vue', 'views/Reader.vue', 'views/knowledge/KnowledgeTasks.vue', 'views/knowledge/WikiWorkspace.vue', 'components/controls/AppDatePicker.vue']) {
    assert.match(await source(file), /data-glass="inset"/, file)
  }
  assert.match(await source('views/Tasks.vue'), /class="switch-indicator"[^>]*data-glass-indicator/)
  const css = await source('styles/materials.css')
  assert.match(css, /--glass-selection-fill:/)
  assert.match(css, /\[data-glass-indicator\]/)
  assert.match(css, /\[data-glass="inset"\][\s\S]*?--material-filter:\s*none/)
  assert.match(await source('components/reader/BookshelfView.vue'), /class="shelf-chip"[^>]*data-glass-choice/)
})

test('graph and Skill surfaces no longer reference missing legacy material tokens', async () => {
  for (const file of ['components/knowledge/KnowledgeGraphCanvas.vue', 'views/knowledge/WikiWorkspace.vue', 'views/knowledge/WikiSettings.vue']) {
    assert.doesNotMatch(await source(file), /var\(--(?:bg-surface|bg-soft|shadow-xs)\)/, file)
  }
  assert.match(await source('components/knowledge/KnowledgeGraphCanvas.vue'), /class="canvas-controls"[^>]*data-glass="group"/)
  assert.match(await source('components/tasks/TaskCalendar.vue'), /class="calendar-nav"[^>]*:data-glass="compact \? 'flat' : 'group'"/)
})

test('panels use a distinct softer rim and optics consume the host material rather than forcing floating tint', async () => {
  const css = await source('styles/materials.css')
  assert.match(css, /--glass-panel-filter:\s*blur\(12px\)/)
  assert.match(css, /--glass-panel-sheen:/)
  assert.match(css, /--glass-panel-edge:/)
  assert.match(css, /--glass-panel-shadow:[^;]*var\(--glass-panel-inner-edge\)/)
  assert.match(css, /\.glass-optics\s*\{[^}]*background:\s*var\(--material-sheen,[^;]*var\(--material-fill,/)
  assert.match(css, /\[data-glass-tone="media"\]/)
})

test('choice materials retain opaque, high-contrast and reduced-motion fallbacks', async () => {
  const css = await source('styles/materials.css')
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\[data-glass-choice\]/)
  for (const signal of ['@supports not', '@media (prefers-reduced-transparency: reduce)', '@media (forced-colors: active)']) {
    const block = css.slice(css.indexOf(signal))
    assert.match(block, /--glass-selection-fill:\s*(?:var\(--glass-solid\)|Highlight)/, signal)
  }
  assert.match(css, /\[data-glass-lens\]\[data-glass-tone="media"\]\s*\{\s*background:\s*var\(--glass-media-fill\)\s*!important/)
})

test('translucent panels keep primary and secondary labels readable over extreme scrim backgrounds', async () => {
  const css = await source('styles/materials.css')
  const tokens = await source('styles/tokens.css')
  const linear = (value: number) => value / 255 <= .04045 ? value / 255 / 12.92 : ((value / 255 + .055) / 1.055) ** 2.4
  const luminance = (rgb: number[]) => rgb.reduce((sum, value, index) => sum + linear(value) * [.2126, .7152, .0722][index], 0)
  const over = (rgba: number[], bg: number[]) => bg.map((value, index) => rgba[index] * rgba[3] + value * (1 - rgba[3]))
  for (const theme of ['light', 'dark', 'eye-care']) {
    const material = css.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)`))![1]
    const palette = tokens.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)`))![1]
    const rgba = (name: string) => material.match(new RegExp(`--glass-${name}: rgba\\(([^)]+)`))![1].split(',').map(Number)
    for (const label of ['text-primary', 'text-secondary']) {
      const hex = palette.match(new RegExp(`--${label}: #([\\da-f]+)`))![1]
      const foreground = luminance([0, 2, 4].map(index => parseInt(hex.slice(index, index + 2), 16)))
      for (const backdrop of [0, 255]) {
        const background = luminance(over(rgba('panel-fill'), over(rgba('scrim-fill'), [backdrop, backdrop, backdrop])))
        assert.ok((Math.max(foreground, background) + .05) / (Math.min(foreground, background) + .05) >= 4.5, `${theme}: ${label}`)
      }
    }
  }
  assert.match(css, /--material-fill:\s*var\(--glass-panel-fill\);[^}]*--text-muted:\s*var\(--text-secondary\);[^}]*--text-faint:\s*var\(--text-secondary\);/)
})
