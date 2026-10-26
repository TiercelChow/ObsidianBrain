import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = (path: string) => readFile(new URL(`../src/${path}`, import.meta.url), 'utf8')

test('nested task headings have a stable full-width row, independent of the parent return button', async () => {
  const tasks = await source('views/Tasks.vue')
  const header = tasks.match(/<header class="child-dialog-header">([\s\S]*?)<\/header>/)?.[1] || ''
  assert.ok(header.includes('class="child-header-toolbar"'))
  assert.ok(header.includes('class="child-parent-path"'))
  assert.match(header, /<h3 class="child-task-title">\{\{ drawerNode.title \}\}<\/h3>/)
  assert.ok(header.indexOf('class="child-task-title"') > header.indexOf('class="child-header-actions"'))
  assert.doesNotMatch(tasks, /child-dialog-header\.has-back|child-dialog-actions/)
})

test('subtask and progress creation live beside their own section headings, including an empty child list', async () => {
  const tasks = await source('views/Tasks.vue')
  const dialog = tasks.slice(tasks.indexOf('class="child-task-dialog'), tasks.indexOf('<MotionModal v-model="sheetOpen"'))
  assert.doesNotMatch(dialog, /<footer|<section v-if="drawerChildren.length"/)
  for (const [section, label, action] of [['子任务', '添加子任务', 'openSubtask'], ['进展与记录', '记录进展', 'openProgress']]) {
    const content = dialog.match(new RegExp(`<section class="child-section" aria-label="${section}">([\\s\\S]*?)</section>`))?.[1] || ''
    const heading = content.match(/<div class="column-heading">[\s\S]*?<UiAction[^>]+\/>/)?.[0] || ''
    assert.ok(heading.includes(`label="${label}"`), label)
    assert.ok(heading.includes(`@click="${action}(drawerNode)"`), action)
    assert.ok(heading.includes('!isTaskClosed(drawerNode)'), 'closed tasks retain their existing write restrictions')
  }
  assert.match(dialog, /v-else class="activity-empty-copy">还没有子任务/)
})

test('the tree and nested child list share a compact vector status mark with a filled completion check', async () => {
  const tree = await source('components/tasks/TaskTree.vue')
  const tasks = await source('views/Tasks.vue')
  const mark = await source('components/tasks/TaskStatusMark.vue')
  assert.match(tree, /<TaskStatusMark :status="item.node.status"/)
  assert.match(tasks, /<TaskStatusMark :status="child.status"/)
  assert.match(mark, /<CircleCheckFilled/)
  assert.match(mark, /aria-hidden="true"/)
  assert.match(tree, /:aria-label="`\$\{item.node.title\}，\$\{statusLabel\(item.node.status\)\}`"/)
  assert.match(tree, /\.status-orb\s*\{[^}]*width: 44px;[^}]*height: 44px;/)
})
