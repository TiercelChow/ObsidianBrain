import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const frontendRoot = new URL('../', import.meta.url)

async function source(path: string) {
  return readFile(new URL(path, frontendRoot), 'utf8')
}

test('knowledge chat maps answer citations to numbered evidence cards without v-html', async () => {
  const chat = await source('src/views/knowledge/KnowledgeChat.vue')

  assert.match(chat, /parseKnowledgeCitations/)
  assert.match(chat, /focusEvidence\(message, segment\.sourceIndex\)/)
  assert.match(chat, /S\{\{ evidenceIndex \+ 1 \}\}/)
  assert.match(chat, /本次会话已切换为书内证据检索模式/)
  assert.doesNotMatch(chat, /v-html/)
})

test('knowledge tasks expose explicit run retry and persisted result actions', async () => {
  const [view, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(view, /executeKnowledgeTask/)
  assert.match(view, /getKnowledgeTaskResult/)
  assert.match(view, /重新运行/)
  assert.match(api, /execute_knowledge_task/)
  assert.match(api, /get_knowledge_task_result/)
})

test('runtime settings keep save and paid connection verification as separate actions', async () => {
  const settings = await source('src/views/knowledge/WikiSettings.vue')

  assert.match(settings, />验证已保存配置</)
  assert.match(settings, />保存</)
  assert.match(settings, /verifyAgentRuntime/)
  assert.doesNotMatch(settings, /保存并检测/)
})
