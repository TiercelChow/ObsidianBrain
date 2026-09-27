import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('website consumes application optics without duplicating theme recipes', async () => {
  const [materials, main, base] = await Promise.all([read('src/materials.css'), read('src/main.js'), read('src/style.css')])
  assert.match(materials, /@import ['"]\.\.\/\.\.\/frontend\/src\/styles\/materials\.css['"]/)
  assert.match(main, /import ['"]\.\/materials\.css['"]/)
  assert.doesNotMatch(base, /--(?:bg-glass(?:-strong|-subtle)?|border-glass|glass-blur|orb-opacity):/)
})

test('every website entry marks floating and structural chrome consistently', async () => {
  for (const path of ['index.html', 'manual/index.html', 'llm-wiki/index.html']) {
    const html = await read(path)
    assert.match(html, /class="header-inner"[^>]*data-glass="floating"[^>]*data-glass-rim/)
    if (path !== 'index.html') {
      assert.match(html, /class="docs-sidebar"[^>]*data-glass="structural"/)
      assert.match(html, /class="mobile-chapters"[^>]*data-glass="floating"[^>]*data-glass-rim/)
    }
  }
})

test('website filters use semantic roles, with no ambient or card light pools', async () => {
  for (const path of ['src/style.css', 'src/home.css', 'src/manual.css', 'src/llm-wiki.css', 'src/materials.css']) {
    const source = await read(path)
    assert.doesNotMatch(source, /radial-gradient|ambient-orb|cta-orb|--orb-opacity/)
    for (const match of source.matchAll(/(?:-webkit-)?backdrop-filter:\s*([^;\n}]+)/g)) {
      assert.match(match[1], /^(?:none|var\(--glass-(?:floating|panel|structural|content|control)-filter\))(?:\s*!important)?$/)
    }
  }
})

test('neutral actions preserve accessible interaction and avoid nested filters', async () => {
  const material = await read('src/materials.css')
  assert.match(material, /\.button-primary[\s\S]*?var\(--glass-action-fill\)/)
  assert.match(material, /var\(--glass-action-sheen\)/)
  assert.match(material, /var\(--glass-action-pressed\)/)
  assert.match(material, /var\(--glass-action-label\)/)
  assert.match(material, /focus-visible/)
  assert.match(material, /\.mock-sidebar[\s\S]*?backdrop-filter:\s*none/)
  assert.match(material, /@media print/)
})
