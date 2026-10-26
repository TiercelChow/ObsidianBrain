import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import test from 'node:test'
import { iconActions } from './helpers/iconActions.mjs'

const source = (file: string) => readFile(new URL(`../src/${file}`, import.meta.url), 'utf8')

test('all icon-only actions use vectors rather than close, plus or zoom font characters', async () => {
  const textGlyphs: string[] = []
  let count = 0
  for (const file of await readdir(new URL('../src/', import.meta.url), { recursive: true })) {
    if (!file.endsWith('.vue')) continue
    for (const action of iconActions(await source(file))) {
      ++count
      if (action.textGlyphs.length) textGlyphs.push(`${file}:${action.line} ${action.textGlyphs.join(' ')}`)
    }
  }
  assert.ok(count > 85, 'audit actual pages and components, not just a new helper')
  assert.deepEqual(textGlyphs, [])
})

test('all standalone icon shells and library close controls share baseline-independent centering', async () => {
  const css = await source('styles/controls.css')
  const shell = css.match(/:root\[data-theme\] :is\(\[data-direction-button\][^{]+\{([^}]+)\}/)?.[0] || ''
  for (const selector of ['[data-icon-only]', '.ui-action', '.el-dialog__headerbtn', '.el-drawer__close-btn', '.el-message-box__headerbtn']) assert.ok(shell.includes(selector), selector)
  for (const rule of ['display: inline-flex', 'align-items: center', 'justify-content: center', 'padding: 0', 'line-height: 1']) assert.ok(shell.includes(rule), rule)
  assert.match(css, /\.ui-action-icon\s*\{[^}]*pointer-events:\s*none/)
  assert.match(css, /\.ui-action__glyph[^{}]*\{[^}]*display:\s*block/)
  assert.match(css, /\.el-button\[data-icon-only\]\.is-loading > span\s*\{\s*display:\s*none/)
})

test('research disclosures and task state glyphs do not rely on font glyph placement either', async () => {
  const research = await source('components/knowledge/KnowledgeResearchWorkspace.vue')
  assert.match(research, /class="ui-chevron stage-directory-icon"/)
  assert.doesNotMatch(research, /summary::after\s*\{[^}]*content:\s*['"][+−]/)
  const tree = await source('components/tasks/TaskTree.vue')
  assert.match(tree, /<TaskStatusMark/)
  const mark = await source('components/tasks/TaskStatusMark.vue')
  assert.match(mark, /<CircleCheckFilled/)
  assert.match(mark, /<Minus/)
  assert.doesNotMatch(tree.split('<script')[0], /[✓—]/)
})

test('built-in warning, empty and preview illustrations use the same vector library, not platform emoji', async () => {
  for (const file of ['views/Reader.vue', 'views/Timeline.vue', 'views/Tasks.vue', 'components/reader/BookshelfView.vue', 'components/reader/PdfViewer.vue', 'components/reader/PathPickerModal.vue', 'components/reader/PathPreviewModal.vue']) {
    assert.doesNotMatch((await source(file)).split('<script')[0], /\p{Extended_Pictographic}/u, file)
  }
})
