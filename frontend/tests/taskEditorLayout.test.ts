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
  assert.match(tasks, /\.field :deep\(\.task-date-input \.mobile-date-trigger\)\s*\{\s*height:\s*100%;\s*\}/)
  assert.doesNotMatch(tasks, /\.field :deep\(\.task-date-input\)\s*\{\s*height:\s*48px/)
})

test('task start/end dates follow the Timeline two-field layout and shared phone date appearance', async () => {
  const tasks = await source('views/Tasks.vue')
  assert.match(tasks, /class="task-date-fields field full" role="group" aria-label="任务日期"/)
  assert.match(tasks, /\.task-date-fields\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)[^}]*gap:\s*12px/)
  for (const name of ['开始日期', '结束日期']) assert.match(tasks, new RegExp(`aria-label="${name}"`))
  assert.equal((tasks.match(/class="task-date-input"/g) || []).length, 2)
  assert.doesNotMatch(tasks, /formDateRange|任务起止日期/)
  const trigger = tasks.match(/\.field :deep\(\.task-date-input \.mobile-date-trigger\)\s*\{([^}]+)\}/)?.[1] || ''
  assert.doesNotMatch(trigger, /padding|gap|font-size|background|box-shadow|border-radius/)
  const picker = await source('components/controls/AppDatePicker.vue')
  const reference = await source('views/Timeline.vue')
  assert.match(reference, /class="filter-sheet-dates"/)
  assert.match(picker, /\.mobile-date-trigger\s*\{[^}]*gap:8px[^}]*min-height:44px[^}]*padding:8px 10px/)
})
