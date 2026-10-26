import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (file: string) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8')

test('native field paint excludes component-owned date and select inputs, including focus', async () => {
  const css = await source('styles/controls.css')
  const rules = [...css.matchAll(/:root\[data-theme\] :is\(([^{}]*\.field input[^{}]*)\)\s*(?::focus)?\s*\{/g)]
  assert.equal(rules.length, 2)
  for (const rule of rules) {
    assert.match(rule[1], /:not\(\.el-input__inner\)/)
    assert.match(rule[1], /:not\(\.el-select__input\)/)
    assert.match(rule[1], /\.field textarea:not\(\.el-textarea__inner\)/)
  }
})

test('task dates allocate their real control height and separate icon, value and edges', async () => {
  const tasks = await source('views/Tasks.vue')
  assert.match(tasks, /\.form-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/)
  assert.match(tasks, /\.field\s*\{[^}]*min-width:\s*0/)
  assert.match(tasks, /\.field :deep\(\.task-date-input\)\s*\{[^}]*height:\s*44px/)
  assert.match(tasks, /\.field :deep\(\.task-date-input \.el-input__wrapper\)\s*\{[^}]*height:\s*100%[^}]*padding:\s*0 14px[^}]*gap:\s*10px/)
  assert.match(tasks, /\.field :deep\(\.task-date-input \.el-input__prefix-inner > :last-child\)\s*\{\s*margin-right:\s*0/)
  assert.match(tasks, /\.field :deep\(\.task-date-input \.mobile-date-trigger\)\s*\{[^}]*padding:\s*0 14px[^}]*gap:\s*10px/)
  assert.match(tasks, /@media \(max-width: 768px\)[\s\S]*?\.field :deep\(\.task-date-input\)\s*\{\s*height:\s*48px/)
})
