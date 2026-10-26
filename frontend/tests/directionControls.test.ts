import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import test from 'node:test'

const source = (file: string) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8')

test('navigation and disclosure controls use SVG icons instead of font-dependent chevron text', async () => {
  for (const file of await readdir(new URL('../src/', import.meta.url), { recursive: true })) {
    if (!file.endsWith('.vue')) continue
    const template = (await source(file)).split('<script')[0]
    assert.doesNotMatch(template, />\s*[‹›«»〈〉❮❯❬❭⟨⟩]\s*<|>\s*(?:&lt;|&gt;)\s*</, file)
  }
  for (const file of ['views/Tasks.vue', 'components/tasks/TaskCalendar.vue']) {
    const text = await source(file)
    assert.match(text, /ArrowLeft/)
    assert.match(text, /ArrowRight/)
    assert.match(text, /data-direction-button/)
  }
  assert.match(await source('components/tasks/TaskTree.vue'), /<ArrowRight/)
})

test('direction buttons center a real icon without baseline or asymmetric default padding', async () => {
  const css = await source('styles/controls.css')
  const button = css.match(/:root\[data-theme\] :is\(\[data-direction-button\][^{]+\{([^}]+)\}/)?.[1] || ''
  for (const rule of ['display: inline-flex', 'align-items: center', 'justify-content: center', 'padding: 0', 'line-height: 1']) assert.ok(button.includes(rule), rule)
  assert.match(css, /\.ui-chevron\s*\{[^}]*flex:\s*none[^}]*line-height:\s*1/)
  assert.match(css, /\.ui-chevron > svg\s*\{[^}]*display:\s*block/)
})

test('disclosure rotation belongs to the centered icon and retains accessible state and reduced motion', async () => {
  const tree = await source('components/tasks/TaskTree.vue')
  const calendar = await source('components/tasks/TaskCalendar.vue')
  assert.match(tree, /:aria-expanded="expanded.has\(item.node.id\)"/)
  assert.match(tree, /@click.stop="toggle\(item.node.id\)"/)
  assert.match(calendar, /:aria-expanded="expandedAgendaIds.has\(entry.task.id\)"/)
  assert.doesNotMatch(calendar, /\.agenda-expand\.expanded\s*\{[^}]*transform:/)
  const css = await source('styles/controls.css')
  assert.match(css, /\[data-direction-button\]\[aria-expanded="true"\] > \.ui-chevron\s*\{[^}]*transform:\s*rotate\(90deg\)/)
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)\s*\{\s*\[data-direction-button\]\[aria-expanded\] > \.ui-chevron\s*\{\s*transition:\s*none/)
})
