import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/CodeRepo.vue', import.meta.url), 'utf8')

test('repository detail has a bounded fixed panel and one keyboard-scrollable content region', async () => {
  const text = await source()
  assert.match(text, /<el-dialog[^>]*class="repo-detail-dialog"[^>]*align-center/)
  assert.match(text, /<el-dialog[^>]*destroy-on-close/)
  assert.match(text, /class="repo-detail-content"[^>]*tabindex="0"[^>]*role="region"[^>]*aria-label="仓库详情内容"/)
  assert.match(text, /:global\(:root\[data-theme\] \.repo-detail-dialog\)\s*\{[^}]*height: min\(640px, calc\(var\(--visual-viewport-height, 100dvh\) - 64px\)\)/)
  assert.match(text, /:global\(:root\[data-theme\] \.repo-detail-dialog \.el-dialog__header\)\s*\{[^}]*flex: none/)
  assert.match(text, /:global\(:root\[data-theme\] \.repo-detail-dialog \.el-dialog__body\)\s*\{[^}]*min-height: 0;[^}]*overflow: hidden/)
  assert.match(text, /\.repo-detail-content\s*\{[^}]*min-height: 0;[^}]*overflow-y: auto;[^}]*overscroll-behavior: contain/)
})

test('long repo fields wrap, recent commits are not truncated, and mobile spans match the column count', async () => {
  const text = await source()
  assert.match(text, /:span="isMobile \? 1 : 2"/)
  assert.doesNotMatch(text, /recent_commits\.slice\(/)
  assert.match(text, /\.commit-msg\s*\{[^}]*min-width: 0;[^}]*overflow-wrap: anywhere;[^}]*white-space: pre-wrap/)
  assert.match(text, /:global\(:root\[data-theme\] \.repo-detail-dialog \.el-descriptions__table\)\s*\{[^}]*table-layout: fixed/)
  assert.match(text, /@media \(max-width: 768px\)[\s\S]*\.repo-detail-dialog\)[^{]*\{[^}]*height: min\(720px,/)
  assert.ok(text.includes('<style scoped>'))
  assert.doesNotMatch(text, /:global\(\.el-dialog(?:__body)?\)/)
})

test('long branch and language names do not expand the underlying repo list on phones', async () => {
  const text = await source()
  assert.match(text, /\.repo-card\s*\{[^}]*min-width: 0/)
  assert.match(text, /class="repo-branch-name"/)
  assert.match(text, /\.repo-branch-name\s*\{[^}]*min-width: 0;[^}]*overflow-wrap: anywhere/)
  assert.match(text, /\.lang-tag\s*\{[^}]*max-width: 100%;[^}]*overflow-wrap: anywhere/)
  assert.match(text, /@media \(max-width: 768px\)[\s\S]*\.repo-grid\s*\{[^}]*grid-template-columns: minmax\(0, 1fr\)/)
})
