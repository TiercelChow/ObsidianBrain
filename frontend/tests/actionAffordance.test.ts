import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (name: string) => readFile(new URL(`../src/${name}`, import.meta.url), 'utf8')

test('shared icon actions name their purpose and never hide loading or disabled state', async () => {
  const action = await source('components/motion/UiAction.ts')
  assert.match(action, /'aria-label': props.label/)
  assert.match(action, /title: props.label/)
  assert.match(action, /props.disabled \|\| props.loading/)
  assert.match(action, /props.loading \? Loading : props.icon/)
})

test('content switches overlap immediately, preserve outgoing contents and reduce motion', async () => {
  const swap = await source('components/motion/MotionSwap.vue')
  assert.match(swap, /:key="viewKey"/)
  assert.doesNotMatch(swap, /mode="out-in"/)
  assert.match(swap, /element.inert = true/)
  assert.match(swap, /prefers-reduced-motion: reduce/)
  assert.match(swap, /<MotionContent :live="true"><slot \/><\/MotionContent>/)
})

test('collection transitions outrank scoped tile positioning and hover transitions only while active', async () => {
  const css = await source('styles/motion.css')
  assert.match(css, /\.collection-flow-leave-active\s*\{[^}]*position:\s*absolute\s*!important/)
  for (const state of ['move','enter-active','leave-active']) {
    assert.match(css, new RegExp(`\\.collection-flow-${state}\\s*\\{[^}]*transition:[^}]*!important`))
  }
})

test('every page family participates beyond creation buttons', async () => {
  for (const page of ['Tasks.vue','CodeRepo.vue','knowledge/WikiSettings.vue','knowledge/KnowledgeChat.vue','knowledge/WikiWorkspace.vue','knowledge/KnowledgeTasks.vue']) {
    const text = await source(`views/${page}`)
    assert.match(text, /<UiAction/, page)
  }
  for (const page of ['knowledge/WikiSettings.vue','knowledge/KnowledgeTasks.vue','knowledge/WikiWorkspace.vue']) {
    assert.match(await source(`views/${page}`), /<MotionSwap/, page)
  }
  const reader = await source('views/Reader.vue')
  assert.match(reader, /Transition name="reader-view"/)
  assert.match(reader, /v-show="viewMode === 'read'"/)
  assert.match(await source('components/tasks/TaskTree.vue'), /<TransitionGroup/)
  assert.match(await source('components/reader/BookshelfView.vue'), /<TransitionGroup/)
})

test('routine loading is a named spinner and provider creation does not waste a separate row', async () => {
  for (const file of ['PdfViewer.vue', 'PathPickerModal.vue', 'PathPreviewModal.vue']) {
    const text = await source(`components/reader/${file}`)
    assert.doesNotMatch(text, /<span>(PDF )?加载中…<\/span>/, file)
    assert.match(text, /role="status" aria-label="加载/, file)
  }
  const settings = await source('views/knowledge/WikiSettings.vue')
  assert.match(settings, /\.settings-section-head\.provider-section-head\s*\{[^}]*display: flex;[^}]*flex-wrap: nowrap/)
})
