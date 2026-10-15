import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/KnowledgeTasks.vue', import.meta.url), 'utf8')

test('research task viewing is one quiet eye beside the title, not a duplicate footer action', async () => {
  const page = await source()
  const titleRow = page.match(/<div class="task-title-row">[\s\S]*?<\/div>/)?.[0] ?? ''
  assert.match(titleRow, /<h2>\{\{ task\.title \}\}<\/h2>[\s\S]*?<UiAction class="task-view-action" :icon="View"/)
  assert.match(titleRow, /:loading="loadingResultId === task\.id" @click="openTaskDetails\(task\)"/)
  assert.doesNotMatch(titleRow, /appearance="glass"|<span>查看<\/span>/)
  assert.match(page, /<div v-if="task\.status !== 'completed' && task\.status !== 'failed'" class="task-card-actions">/)
  const actions = page.match(/class="task-card-actions">[\s\S]*?<\/div>/)?.[0] ?? ''
  assert.doesNotMatch(actions, /<UiAction|<View/)
  assert.match(page, /class="task-action"[\s\S]*?@click="runOrOpen\(task\)"/)
  assert.match(page, /function openTaskDetails\(task: KnowledgeTask\)[\s\S]*?task\.status === 'completed' \|\| \(task\.status === 'failed' && task\.artifact_state === 'failed'\)\) void openResult\(task\)/)
  assert.match(page, /else if \(task\.status === 'failed'\) void openFailedInspection\(task\)\s*else openStages\(task\)/)
  assert.match(page, /task\.status === 'running' \|\| task\.status === 'queued'\) \{\s*void cancelTask\(task\)/)
  assert.match(page, /\.task-title-row\s*\{[^}]*display: flex;[^}]*align-items: center/)
  assert.match(page, /\.task-title-row h2\s*\{[^}]*min-width: 0;[^}]*flex: 1;[^}]*margin: 0/)
  assert.match(page, /\.task-view-action\s*\{[^}]*flex: none/)
  assert.match(page, /\.research-task\s*\{[^}]*grid-template-columns: 52px minmax\(0, 1fr\);/)
})

test('presentation file cards keep their content and icon download but omit the duplicate inspector action', async () => {
  const page = await source()
  const artifacts = page.match(/<div v-if="activeArtifacts\.length" class="task-artifacts"[\s\S]*?<\/article>/)?.[0] ?? ''
  assert.match(artifacts, /<article v-for="artifact in activeArtifacts" :key="artifact\.id" class="artifact-card"/)
  assert.match(artifacts, /artifact\.title/)
  assert.match(artifacts, /formatBytes\(artifact\.size_bytes\)/)
  assert.match(artifacts, /artifact\.validation_message/)
  assert.match(artifacts, /class="artifact-icon"/)
  assert.match(artifacts, /<a data-icon-only class="ui-icon-action"/)
  assert.match(artifacts, /:href="knowledgeArtifactDownloadUrl\(artifact\.id\)" download/)
  assert.match(artifacts, /:aria-label="`下载演示文稿：\$\{artifact\.title\}`" :title="artifact\.title"/)
  assert.match(artifacts, /class="artifact-quality"/)
  assert.match(artifacts, /class="artifact-checks"/)
  assert.doesNotMatch(artifacts, /<UiAction|inspectArtifact|策划过程|下载 PPTX/)
  assert.doesNotMatch(page, /function inspectArtifact/)
  assert.match(page, /\.artifact-main-row\s*\{[^}]*grid-template-columns: 38px minmax\(0, 1fr\) auto/)
  assert.match(page, /PPTX 生成失败，研究报告已保留/)
  assert.match(page, /@click="openTaskInspection"/)
})
