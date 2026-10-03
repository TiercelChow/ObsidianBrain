import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('one material entry owns all three themes and distinct surface weights', async () => {
  const [css, app] = await Promise.all([source('styles/materials.css'), source('App.vue')])
  assert.match(app, /<style src="\.\/styles\/materials\.css"><\/style>/)
  for (const theme of ['light', 'dark', 'eye-care']) {
    const block = css.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\s*\\{([^}]+)\\}`))?.[1] || ''
    for (const token of ['floating-fill', 'panel-fill', 'content-fill', 'solid', 'rim-light', 'rim-shadow']) {
      assert.match(block, new RegExp(`--glass-${token}:`), `${theme}: ${token}`)
    }
  }
  assert.match(css, /--glass-floating-filter:\s*blur\(3px\)/)
  assert.match(css, /--glass-panel-filter:\s*blur\(16px\)/)
  assert.match(css, /--bg-primary:\s*var\(--glass-solid\)/)
})

test('floating controls and overlays share optics while content and nested fields do not blur', async () => {
  const css = await source('styles/materials.css')
  for (const consumer of ['.dock-glass', '.reader-mobile-toolbar', '.fs-fab', '.chat-composer', '.sidebar', '.knowledge-modal-card', '.system-select-popper', '.el-dialog']) {
    assert.ok(css.includes(consumer), `${consumer} participates in shared materials`)
  }
  assert.match(css, /\/\* Content[\s\S]+?--material-filter:\s*none/)
  assert.match(css, /\/\* Nested[\s\S]+?\.el-input__wrapper[\s\S]+?backdrop-filter:\s*none\s*!important/)
  assert.match(css, /\.task-toolbar\s+\.task-search[\s\S]+?--material-filter:\s*none/)
})

test('rim highlights stay pointer-transparent and filter-free beside the independent optical layer', async () => {
  const [css, dock] = await Promise.all([source('styles/materials.css'), source('components/MobileDock.vue')])
  assert.match(dock, /class="mobile-dock dock-glass"\s+data-glass-rim/)
  assert.match(dock, /class="mobile-sub-dock dock-glass"\s+data-glass-rim/)
  const rim = css.match(/\[data-glass-rim\]::before\s*\{([^}]+)\}/)?.[1] || ''
  assert.match(rim, /pointer-events:\s*none/)
  assert.match(rim, /border-radius:\s*inherit/)
  assert.match(rim, /mask-composite:\s*exclude/)
  assert.match(rim, /-webkit-mask-composite:\s*xor/)
  assert.doesNotMatch(rim, /backdrop-filter|animation|radial-gradient/)
  assert.doesNotMatch(css, /feDisplacementMap|@keyframes|will-change|transition:\s*all/)
})

test('transparency, contrast, forced colors and unsupported filters have opaque fallbacks', async () => {
  const css = await source('styles/materials.css')
  assert.match(css, /@supports not \(\(backdrop-filter:/)
  assert.match(css, /@media \(prefers-reduced-transparency:\s*reduce\), \(prefers-contrast:\s*more\)/)
  assert.match(css, /--glass-floating-fill:\s*var\(--glass-solid\)/)
  assert.match(css, /--glass-floating-filter:\s*none/)
  assert.match(css, /@media \(forced-colors:\s*active\)/)
  assert.match(css, /\[data-glass-rim\]::before\s*\{\s*display:\s*none/)
})

test('reader title shares optics and preserves its transition-time flattening', async () => {
  const reader = await source('views/Reader.vue')
  assert.match(reader, /heading\.setAttribute\('data-glass', 'scroll-edge'\)/)
  assert.match(reader, /attachGlassLens\(heading, \{ mode: 'css' \}\)/)
  assert.match(reader, /\.reader-page\.is-fs-transitioning[\s\S]+?backdrop-filter:\s*none\s*!important/)
})

test('app and fullscreen reading have a uniform backdrop without light pools or grain', async () => {
  for (const file of ['App.vue', 'views/Reader.vue']) {
    const text = await source(file)
    assert.doesNotMatch(text, /radial-gradient|ambient-bg|bg-grain|--orb-opacity/, `${file}: no independent ambient lighting`)
  }
})

test('every component filter consumes a semantic material token instead of a local blur recipe', async () => {
  const root = new URL('../src/', import.meta.url)
  for (const file of [...await readdir(root, { recursive: true }), '../index.html']) {
    if (!/\.(vue|css|html)$/.test(file) || file === 'styles/materials.css') continue
    const text = await source(file)
    for (const match of text.matchAll(/(?:-webkit-)?backdrop-filter:\s*([^;\n}]+)/g)) {
      assert.match(match[1], /^(?:none|var\(--glass-(?:floating|panel|structural|content|control|scrim)-filter\))(?:\s*!important)?$/, `${file}: ${match[0]}`)
    }
  }
})

test('calendar, secondary toolbars, previews, snackbar and scrims participate in shared materials', async () => {
  const css = await source('styles/materials.css')
  for (const selector of ['.glass-panel', '.config-actions', '.mobile-detail-nav', '.toolbar-row', '.published-notice', '.undo-snackbar', '.reader-toast', '.path-trigger', '.mv-mobile-close', '.canvas-hint', '.motion-modal', '.ppm-overlay']) {
    assert.ok(css.includes(selector), `${selector}: shared material coverage`)
  }
  assert.match(css, /--glass-scrim-filter:\s*blur\(4px\)/)
  assert.match(css, /--material-sheen:\s*none/)
  assert.match(css, /background:\s*var\(--material-sheen\),\s*var\(--material-fill\)/)
})

test('media inspection keeps its canvas unfiltered and its controls legible in every theme', async () => {
  const [css, viewer] = await Promise.all([source('styles/materials.css'), source('components/reader/MermaidViewer.vue')])
  assert.match(viewer, /\.mermaid-viewer\s*\{[^}]+background:\s*var\(--bg-base\);[^}]+backdrop-filter:\s*var\(--glass-content-filter\)/)
  for (const selector of ['.viewer-close', '.viewer-nav', '.viewer-zoom-btn', '.viewer-counter']) assert.ok(css.includes(selector))
  assert.match(css, /--glass-media-fill:/)
  assert.match(css, /--glass-media-label:/)
})

test('primary actions share neutral optical controls without filtering every button', async () => {
  const css = await source('styles/materials.css')
  for (const selector of ['.el-button--primary', 'button.primary', '.page-create', '.reader-shelf-add', '.mobile-compose-action', '.mobile-create-task', '.chat-composer button', '.artifact-main-row > a']) {
    assert.ok(css.includes(selector), `${selector}: neutral glass action coverage`)
  }
  assert.match(css, /--glass-action-fill:/)
  assert.match(css, /--glass-action-sheen:/)
  assert.match(css, /\[data-glass-action\][\s\S]+?backdrop-filter:\s*none\s*!important/)
  assert.match(css, /\.el-button--danger[\s\S]+?--control-label:/)
  assert.match(css, /:focus-visible[\s\S]+?outline:/)
  assert.match(css, /:disabled[\s\S]+?opacity:/)
})

test('page sidebars opt into structure, with modal sidebars avoiding a second filter', async () => {
  for (const [file, cls] of [['views/Reader.vue', 'pane pane-left'], ['views/Reader.vue', 'pane pane-right'], ['views/Tasks.vue', 'task-list-panel glass-surface'], ['views/Timeline.vue', 'time-nav'], ['views/knowledge/WikiWorkspace.vue', 'entry-pane knowledge-surface'], ['views/knowledge/KnowledgeChat.vue', 'chat-context knowledge-surface'], ['views/knowledge/WikiSettings.vue', 'settings-nav knowledge-surface'], ['views/knowledge/WikiSettings.vue', 'skill-detail-sidebar']]) {
    assert.match(await source(file), new RegExp(`class="${cls}"[^>]*data-glass="structural"`), `${file}: ${cls}`)
  }
  assert.match(await source('styles/materials.css'), /\.knowledge-modal-card[\s\S]+?\[data-glass="structural"\][\s\S]+?--material-filter:\s*none/)
})

test('Wiki settings fields shrink within their own grid and Skill file buttons expose state', async () => {
  const settings = await source('views/knowledge/WikiSettings.vue')
  assert.match(settings, /\.provider-mode\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/)
  assert.match(settings, /\.usage-filters\s+\.knowledge-select\s*\{[^}]*width:\s*100%/)
  assert.match(settings, /\.skill-file-tabs button\s*\{[^}]*max-width:\s*100%/)
  assert.match(settings, /:aria-pressed="file.relative_path === activeSkillFilePath"/)
  assert.match(settings, /:title="file.relative_path"/)
  assert.match(settings, /class="skill-file-button"[^>]*data-glass-action/)
})

test('file uploads expose styled triggers rather than unstyled native file buttons', async () => {
  const settings = await source('views/knowledge/WikiSettings.vue')
  for (const ref of ['skillArchiveInput', 'backupUploadInput']) {
    const input = settings.match(new RegExp(`<input ref="${ref}"[^>]+>`))?.[0] || ''
    assert.match(input, /\s+hidden(?:\s|\/?>)/, `${ref}: native picker is hidden`)
    assert.match(settings, new RegExp(`<el-button[^>]*@click="${ref}\\?\\.click\\(\\)"`), `${ref}: system styled trigger remains available`)
  }
})
