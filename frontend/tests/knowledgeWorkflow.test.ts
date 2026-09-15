import assert from 'node:assert/strict'
import { access, readFile } from 'node:fs/promises'
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
  assert.match(chat, /streamBookKnowledge/)
  assert.match(chat, /event\.type === 'text_delta'/)
  assert.doesNotMatch(chat, /revealAssistantAnswer/)
  assert.match(chat, /listKnowledgeConversations/)
  assert.match(chat, /getKnowledgeConversation/)
  assert.match(chat, /activeConversationId\.value \|\| undefined/)
  assert.match(api, /list_knowledge_conversations/)
  assert.match(api, /get_knowledge_conversation/)
  assert.match(api, /conversation_id: string/)
  assert.match(api, /knowledge\/chat\/stream/)
  assert.match(api, /text\/event-stream/)
})

test('knowledge chat previews numbered sources before an explicit workspace navigation', async () => {
  const [chat, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(chat, /previewEvidence\(message, sourceIndex\)/)
  assert.match(chat, /S\{\{ evidenceIndex \+ 1 \}\}/)
  assert.match(chat, /<MotionModal[^>]+aria-label="来源预览"/)
  assert.match(chat, /getKnowledgeEntry/)
  assert.match(chat, /在 Wiki 工作台打开/)
  assert.match(chat, /本次会话已切换为书内证据检索模式/)
  assert.match(chat, /class="source-markdown markdown-body" v-html="sourceHtml"/)
  assert.match(chat, /保存到 Wiki/)
  assert.match(chat, /saveKnowledgeAnswer/)
  assert.match(api, /save_knowledge_answer/)
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

test('wiki settings manage versioned skills per book and keep execution instruction-only', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /新增 Skill/)
  assert.match(settings, /当前版本只允许 Markdown 指令/)
  assert.match(settings, /导入 ZIP/)
  assert.match(settings, /importWikiSkillArchive/)
  assert.match(settings, /skill\.usage_scope/)
  assert.match(settings, /setWikiSkillBinding/)
  assert.match(api, /list_wiki_skills/)
  assert.match(api, /save_custom_wiki_skill/)
  assert.match(api, /set_wiki_skill_binding/)
  assert.match(api, /knowledge\/skills\/import/)
})

test('wiki settings expose consistent database snapshots and guarded restore flows', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /SQLite Online Backup/)
  assert.match(settings, /listKnowledgeBackups/)
  assert.match(settings, /createKnowledgeBackup/)
  assert.match(settings, /knowledgeBackupDownloadUrl/)
  assert.match(settings, /uploadAndRestoreKnowledgeBackup/)
  assert.match(settings, /restoreConfirmation !== 'RESTORE'/)
  assert.match(api, /list_knowledge_backups/)
  assert.match(api, /create_knowledge_backup/)
  assert.match(api, /restore_knowledge_backup/)
  assert.match(api, /knowledge\/backups\/restore\/upload/)
})

test('knowledge tasks expose durable execution controls and real presentation artifacts', async () => {
  const [view, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(view, /executeKnowledgeTask/)
  assert.match(view, /getKnowledgeTaskResult/)
  assert.match(view, /重新运行/)
  assert.match(view, /cancelKnowledgeTask/)
  assert.match(view, /getKnowledgeTaskActivity/)
  assert.match(view, /task-live/)
  assert.match(view, /演示文稿/)
  assert.match(view, /knowledgeArtifactDownloadUrl/)
  assert.match(view, /任务已进入后台队列，可以离开此页面/)
  assert.match(api, /execute_knowledge_task/)
  assert.match(api, /get_knowledge_task_result/)
  assert.match(api, /cancel_knowledge_task/)
  assert.match(api, /get_knowledge_task_activity/)
  assert.match(api, /knowledge\/artifacts/)
})

test('external research stays opt-in, domain scoped, and rate limited per task', async () => {
  const [view, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(view, /授权外部资料研究/)
  assert.match(view, /仅本次任务有效，默认关闭/)
  assert.match(view, /draft\.externalResearchEnabled = false/)
  assert.match(view, /externalRequestLimit/)
  assert.match(view, /只允许 HTTPS 文本/)
  assert.match(api, /external_research_enabled/)
  assert.match(api, /external_domains/)
  assert.match(api, /external_request_limit/)
})

test('knowledge pages distinguish source synchronization from wiki compilation', async () => {
  const [bases, workspace, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/views/knowledge/WikiWorkspace.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(bases, /来源 ·/)
  assert.match(bases, /Wiki ·/)
  assert.match(bases, /章节索引/)
  assert.match(workspace, /compileLabel/)
  assert.match(workspace, /可核验论断/)
  assert.match(workspace, /知识关系/)
  assert.match(workspace, /版本历史/)
  assert.match(workspace, /知识体检/)
  assert.match(workspace, /批准并应用/)
  assert.match(api, /compile_mode: 'chapter' \| 'smart'/)
  assert.match(api, /compile_state: 'not_started' \| 'outdated' \| 'compiling' \| 'ready' \| 'failed'/)
})

test('wiki workspace supports reviewed structure changes, paging, and an on-demand graph canvas', async () => {
  const [bases, workspace, graphCanvas, reader, chat, tasks, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/views/knowledge/WikiWorkspace.vue'),
    source('src/components/knowledge/KnowledgeGraphCanvas.vue'),
    source('src/views/Reader.vue'),
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(bases, /管理知识库/)
  assert.match(bases, /setBookKnowledgeBaseLifecycle/)
  assert.match(bases, /原书目录已失效/)
  assert.match(workspace, /继续加载/)
  assert.match(workspace, /proposeKnowledgeEntryEdit/)
  assert.match(workspace, /proposeKnowledgeEntryMerge/)
  assert.match(workspace, /proposeKnowledgeEntrySplit/)
  assert.match(workspace, /调整知识实体/)
  assert.match(workspace, /查看变更前后/)
  assert.match(workspace, /关系洞察/)
  assert.match(workspace, /findKnowledgeGraphPath/)
  assert.match(workspace, /getKnowledgeGraphSnapshot/)
  assert.match(graphCanvas, /@wheel\.prevent="onWheel"/)
  assert.match(graphCanvas, /@pointermove="movePan"/)
  assert.match(graphCanvas, /@media \(max-width: 768px\) \{ \.graph-canvas-shell \{ display: none;/)
  assert.match(reader, /proposeReaderSelection/)
  assert.match(reader, /加入知识/)
  assert.match(reader, /fileKind === 'md'/)
  assert.match(chat, /route\.query\.question/)
  assert.match(tasks, /route\.query\.create === '1'/)
  assert.match(api, /source_available: boolean/)
  assert.match(api, /has_more: boolean/)
  assert.match(api, /propose_knowledge_entry_merge/)
  assert.match(api, /propose_knowledge_entry_split/)
  assert.match(api, /get_knowledge_graph_snapshot/)
})

test('Book Wiki only exposes Markdown folders while Reader keeps PDF support', async () => {
  const [bases, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(bases, /filter\(card => card\.book\.kind === 'folder'\)/)
  assert.match(bases, /只显示阅境轩中的 Markdown 文件夹/)
  assert.doesNotMatch(bases, /card\.book\.kind === 'pdf'/)
  assert.match(api, /runtime: 'deepseek_harness'/)
  assert.doesNotMatch(api, /runtime: 'deepseek_harness' \| 'claude_code'/)
})

test('legacy knowledge pages are retired behind one-release compatibility redirects', async () => {
  const router = await source('src/router/index.ts')
  for (const [legacyPath, destination] of [
    ['/memory', '/knowledge'],
    ['/wiki-dashboard', '/knowledge'],
    ['/wiki', '/knowledge/wiki'],
    ['/explore', '/knowledge/chat'],
    ['/ingest', '/knowledge/tasks'],
  ]) {
    assert.match(router, new RegExp(`path: '${legacyPath}'.*redirect: '${destination}'`))
  }
  for (const file of ['Memory.vue', 'WikiDashboard.vue', 'WikiWorkbench.vue', 'Explore.vue', 'Ingest.vue']) {
    await assert.rejects(access(new URL(`src/views/${file}`, frontendRoot)), { code: 'ENOENT' })
  }
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
