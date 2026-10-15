import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('phone vertical reading planes do not reserve a disappearing desktop scrollbar', async () => {
  const app = await source('src/App.vue')
  const policy = app.match(/\/\* Mobile scroll planes:[\s\S]*?\/\* Theme colors/)?.[0] ?? ''
  assert.match(policy, /@media \(max-width: 768px\)/)
  for (const pane of ['.app-main.mobile-full', '.pane-center', '.memo-scroll', '.task-list-scroll', '.task-detail-zone', '.mobile-detail-scroll', '.pdf-viewer', '.message-list', '.entry-list', '.entry-detail']) {
    assert.ok(policy.includes(pane), pane)
  }
  assert.match(policy, /scrollbar-width: none;/)
  assert.match(policy, /scrollbar-gutter: auto;/)
  assert.match(policy, /::-webkit-scrollbar\s*\{\s*width: 0;\s*height: 0;/)
  assert.doesNotMatch(policy, /overflow(?:-y)?:\s*hidden|padding-(?:left|right)|\.table-scroll|\.code-content|textarea/)
})

test('desktop scrollbar and drawer background scroll protection remain enabled', async () => {
  const [app, motion, modal] = await Promise.all([
    source('src/App.vue'), source('src/styles/motion.css'), source('src/composables/useModalEnvironment.ts'),
  ])
  assert.match(app, /::-webkit-scrollbar \{ width: 5px; \}/)
  assert.match(motion, /@media \(max-width: 768px\)\s*\{\s*html\.motion-overlay-open \.app-main,[\s\S]*?overflow: hidden !important/)
  assert.match(modal, /scrollLockCount \+= 1/)
  assert.match(modal, /if \(scrollLockCount !== 0\) return/)
  assert.match(modal, /target\.focus\(\{ preventScroll: true \}\)/)
})
