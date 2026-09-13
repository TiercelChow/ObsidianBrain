import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const frontendRoot = new URL('../', import.meta.url)

async function source(path: string) {
  return readFile(new URL(path, frontendRoot), 'utf8')
}

test('knowledge chat persists conversations and restores their cited messages', async () => {
  const [chat, answer, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/components/knowledge/KnowledgeAnswerMarkdown.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(chat, /KnowledgeAnswerMarkdown/)
  assert.match(answer, /renderMarkdownDocument/)
  assert.match(answer, /data-source-index/)
  assert.match(chat, /revealAssistantAnswer/)
  assert.match(chat, /requestAnimationFrame/)
  assert.match(chat, /listKnowledgeConversations/)
  assert.match(chat, /getKnowledgeConversation/)
  assert.match(chat, /activeConversationId\.value \|\| undefined/)
  assert.match(api, /list_knowledge_conversations/)
  assert.match(api, /get_knowledge_conversation/)
  assert.match(api, /conversation_id: string/)
})

test('knowledge chat previews numbered sources before an explicit workspace navigation', async () => {
  const chat = await source('src/views/knowledge/KnowledgeChat.vue')

  assert.match(chat, /previewEvidence\(message, sourceIndex\)/)
  assert.match(chat, /S\{\{ evidenceIndex \+ 1 \}\}/)
  assert.match(chat, /<MotionModal[^>]+aria-label="来源预览"/)
  assert.match(chat, /getKnowledgeEntry/)
  assert.match(chat, /在 Wiki 工作台打开/)
  assert.match(chat, /本次会话已切换为书内证据检索模式/)
  assert.match(chat, /class="source-markdown markdown-body" v-html="sourceHtml"/)
})

test('knowledge thinking label loops as a reduced-motion aware typewriter', async () => {
  const [chat, typewriter] = await Promise.all([
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/composables/useTypewriterLoop.ts'),
  ])

  assert.match(chat, /useTypewriterLoop/)
  assert.match(chat, /正在梳理关键线索/)
  assert.match(typewriter, /prefers-reduced-motion: reduce/)
  assert.match(typewriter, /phrase\.slice\(0, characterIndex\)/)
})

test('knowledge dropdowns use shared system visuals with layout-only utility classes', async () => {
  const [motion, tasks, ...files] = await Promise.all([
    source('src/styles/motion.css'),
    source('src/views/Tasks.vue'),
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/views/knowledge/WikiWorkspace.vue'),
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/views/knowledge/WikiSettings.vue'),
  ])

  assert.ok(files.every(file => file.includes('knowledge-select')))
  assert.ok(files.every(file => file.includes('system-select-popper')))
  assert.match(tasks, /system-select-popper/)
  assert.match(motion, /\.system-select-popper\.el-popper/)
  assert.ok(files.every(file => !file.includes(':deep(.el-select)')))
})

test('wiki settings expose token usage filters and estimated usage disclosure', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /Token 用量/)
  assert.match(settings, /usageDateRange/)
  assert.match(settings, /usageCaller/)
  assert.match(settings, /当前 Harness ACP 未上报精确 token/)
  assert.match(api, /get_agent_usage_stats/)
  assert.match(api, /usage_source/)
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

test('runtime settings support an OpenAI-compatible provider without storing its key', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /第三方模型供应商/)
  assert.match(settings, /API Base URL/)
  assert.match(settings, /API Key 环境变量/)
  assert.match(settings, /CUSTOM_LLM_API_KEY/)
  assert.doesNotMatch(settings, /DEEPSEEK_API_KEY/)
  assert.match(api, /provider_config: profile\.provider_config \?\? null/)
  assert.doesNotMatch(api, /api_key:/)
})
