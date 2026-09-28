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
  assert.match(chat, /let answerEvidenceDelivered = false/)
  assert.match(chat, /if \(answerEvidenceDelivered\) \{/)
  assert.match(chat, /assistantMessage\.runId = event\.run_id/)
  assert.match(chat, /正在理解问题与选择知识/)
  assert.match(chat, /const assistantMessage = messages\.value\[assistantIndex\]/)
  assert.match(chat, /createStreamedTextBuffer/)
  assert.match(chat, /if \(receivedText\) \{\s*textBuffer\.cancel\(\)\s*assistantMessage\.content = ''\s*receivedText = ''/)
  assert.match(chat, /await textBuffer\.drain\(\)/)
  assert.match(chat.split('function newConversation')[1]?.split('function replaceChatQuery')[0] ?? '', /followOutput\.value = true/)
  assert.match(chat.split('async function openConversation')[1]?.split('async function changeKnowledgeBase')[0] ?? '', /followOutput\.value = true/)
  assert.match(chat, /message\.id !== streamingMessageId/)
  assert.match(chat, /chat-run-status/)
  assert.match(chat, /停止生成/)
  assert.doesNotMatch(chat, /revealAssistantAnswer/)
  assert.match(chat, /listKnowledgeConversations/)
  assert.match(chat, /getKnowledgeConversation/)
  assert.match(chat, /activeConversationId\.value \|\| undefined/)
  assert.match(api, /list_knowledge_conversations/)
  assert.match(api, /get_knowledge_conversation/)
  assert.match(api, /conversation_id: string/)
  assert.match(api, /knowledge\/chat\/stream/)
  assert.match(api, /text\/event-stream/)
  assert.match(api, /createSseDataParser/)
  assert.match(api, /parser\.push\(decoder\.decode\(value, \{ stream: !done \}\)\)/)
})

test('research inspector distinguishes native skills and lists evidence actually read', async () => {
  const [tasks, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])
  assert.match(tasks, /harness_native_available/)
  assert.match(tasks, /Harness 可按需读取/)
  assert.match(tasks, /实际读取的证据/)
  assert.match(tasks, /activeInspection\.evidence/)
  assert.match(api, /evidence: Array<\{/)
})

test('research creation confirms an analyzed brief before persisting or running', async () => {
  const [tasks, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])
  assert.match(tasks, /previewKnowledgeTaskBrief\(draft\)/)
  assert.match(tasks, /createStep\.value = 'preferences'/)
  assert.match(tasks, /brief: \{ \.\.\.brief, confirmed: true \}/)
  assert.match(tasks, /briefDecision\('purpose'\)/)
  assert.match(tasks, /briefDecision\('purpose'\)\?\.question/)
  assert.match(tasks, /briefDecision\('purpose'\)\?\.impact/)
  assert.match(tasks, /requestId !== briefPreviewRequestId \|\| !createVisible\.value/)
  assert.match(api, /preview_knowledge_task_brief/)
  assert.match(api, /brief: input\.brief/)
})

test('research workspace distinguishes the internal goal from the reader-facing report title', async () => {
  const [workspace, api] = await Promise.all([
    source('src/components/knowledge/KnowledgeResearchWorkspace.vue'),
    source('src/api/knowledge.ts'),
  ])
  assert.match(api, /report_title\?: string \| null/)
  assert.match(workspace, /workspace\.plan\.report_title/)
  assert.match(workspace, /材料标题/)
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
  assert.match(chat, /loadKnowledgeCitationPreview/)
  assert.match(chat, /message\.runId, sourceIndex/)
  assert.match(chat, /answer-interruption/)
  assert.doesNotMatch(chat.split('async function ask')[1]?.split('async function saveAnswer')[0] || '', /runtimeReady\.value = false/)
  assert.match(chat, /class="source-markdown markdown-body" v-html="sourceHtml"/)
  assert.match(chat, /保存到 Wiki/)
  assert.match(chat, /saveKnowledgeAnswer/)
  assert.match(api, /save_knowledge_answer/)
})

test('knowledge chat uses an uninterrupted text shimmer and an independent three-dot wave', async () => {
  const [chat, answer] = await Promise.all([
    source('src/views/knowledge/KnowledgeChat.vue'),
    source('src/components/knowledge/KnowledgeAnswerMarkdown.vue'),
  ])

  assert.match(chat, /chat-run-label/)
  assert.match(chat, /chat-run-dots/)
  assert.match(chat, /@keyframes chat-dot-wave/)
  assert.match(chat, /background-clip: text/)
  assert.match(chat, /background-repeat: no-repeat/)
  assert.match(chat, /@keyframes chat-status-shimmer/)
  assert.match(chat, /chat-dot-wave 900ms/)
  assert.match(chat, /chat-status-shimmer 2\.4s/)
  assert.match(chat, /translateY\(-2px\)/)
  assert.doesNotMatch(chat, /0%, 26%|72%, 100%/)
  assert.match(chat, /prefers-reduced-motion: reduce/)
  assert.doesNotMatch(chat, /useTypewriterLoop|activitySteps|chat-run-mark|chat-run-pulse/)
  assert.match(answer, /is-entering/)
  assert.match(answer, /@keyframes answer-arrive/)
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

test('wiki settings manage latest skills per book and keep execution instruction-only', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /新增 Skill/)
  assert.match(settings, /当前版本只允许 Markdown 指令/)
  assert.match(settings, /导入 ZIP/)
  assert.match(settings, /importWikiSkillArchive/)
  assert.match(settings, /skill\.usage_scope/)
  assert.match(settings, /仅智能编译/)
  assert.match(settings, /全部场景/)
  assert.match(settings, /setWikiSkillBinding/)
  assert.match(api, /list_wiki_skills/)
  assert.match(api, /save_custom_wiki_skill/)
  assert.match(api, /set_wiki_skill_binding/)
  assert.match(api, /knowledge\/skills\/import/)
})

test('wiki skills expose current resources, permissions, and a safe clone flow', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /查看内容/)
  assert.match(settings, /Skill 内容/)
  assert.match(settings, /当前内容/)
  assert.match(settings, /skillDetail\.versions/)
  assert.match(settings, /权限与依赖/)
  assert.match(settings, /复制为自定义/)
  assert.match(settings, /固定离线评测/)
  assert.match(settings, /真实模型基准/)
  assert.match(settings, /benchmarkActiveSkillVersion/)
  assert.match(settings, /license_spdx/)
  assert.match(settings, /publishActiveSkillVersion/)
  assert.match(api, /get_wiki_skill_detail/)
  assert.match(api, /WikiSkillDetail/)
  assert.match(api, /evaluate_wiki_skill_version/)
  assert.match(api, /start_wiki_skill_benchmark/)
  assert.match(api, /get_wiki_skill_benchmark/)
  assert.match(api, /WikiSkillBenchmarkRun/)
  assert.match(api, /publish_wiki_skill_version/)
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
  assert.match(view, /artifactQuality/)
  assert.match(view, /引用覆盖/)
  assert.match(view, /策划过程/)
  assert.match(view, /inspectArtifact/)
  assert.match(view, /PPTX 生成失败，研究报告已保留/)
  assert.match(view, /task\.artifact_state === 'failed'/)
  assert.match(api, /PresentationQualityReport/)
  assert.match(api, /validation_details/)
  assert.match(view, /任务已进入后台队列，可以离开此页面/)
  assert.match(api, /execute_knowledge_task/)
  assert.match(api, /get_knowledge_task_result/)
  assert.match(api, /cancel_knowledge_task/)
  assert.match(api, /get_knowledge_task_activity/)
  assert.match(api, /knowledge\/artifacts/)
})

test('knowledge task results expose an auditable run inspector', async () => {
  const [view, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeTasks.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(view, /运行检查器/)
  assert.match(view, /有效 Prompt/)
  assert.match(view, /配置快照/)
  assert.match(view, /activeInspection/)
  assert.match(view, /getAgentRunInspection/)
  assert.match(api, /get_agent_run_inspection/)
  assert.match(api, /AgentRunInspection/)
  assert.match(api, /AgentRunConfigSnapshot/)
  assert.match(view, /resultTab === 'inspector'/)
  assert.match(view, /inspectionLoading/)
  assert.match(view, /inspectionError/)
  assert.match(view, /重新加载检查器/)
  assert.match(view, /canFocusDocument\(document\)/)
})

test('failed research tasks open their failed stage before the legacy run inspector', async () => {
  const view = await source('src/views/knowledge/KnowledgeTasks.vue')
  const handler = view.split('async function openFailedInspection(task: KnowledgeTask) {')[1]?.split('\nasync function loadRunInspection')[0] || ''
  assert.match(handler, /getKnowledgeResearchWorkspace\(task\.id\)/)
  assert.match(handler, /hasFailedResearchStage\(workspace\.result\?\.stages\)/)
  assert.match(handler, /openStages\(task\)/)
  assert.match(handler, /getKnowledgeTaskActivity\(task\.id\)/)
  assert.ok(handler.indexOf('openStages(task)') < handler.indexOf('getKnowledgeTaskActivity(task.id)'))
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

test('smart wiki compilation runs in background and exposes recoverable live progress', async () => {
  const [bases, api] = await Promise.all([
    source('src/views/knowledge/KnowledgeBases.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(api, /compile_phase:/)
  assert.match(api, /compile_message:/)
  assert.match(api, /compile_current_batch:/)
  assert.match(api, /compile_total_batches:/)
  assert.match(api, /compile_active_run_id/)
  assert.match(api, /compile_change_set_id/)
  assert.match(api, /cancel_book_knowledge_compile/)
  assert.match(bases, /任务已进入后台，可以离开当前页面/)
  assert.match(bases, /cancelBookKnowledgeCompile/)
  assert.match(bases, /setInterval/)
  assert.match(bases, /compile-phase/)
  assert.match(bases, /查看并审核/)
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
  assert.match(workspace, /变更分类/)
  assert.match(workspace, /引用审计/)
  assert.match(workspace, /级联影响/)
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

test('runtime settings select a model provider and manage API keys on the page', async () => {
  const [settings, api] = await Promise.all([
    source('src/views/knowledge/WikiSettings.vue'),
    source('src/api/knowledge.ts'),
  ])

  assert.match(settings, /第三方模型供应商/)
  assert.match(settings, /API Base URL/)
  assert.match(settings, /API Key 环境变量/)
  assert.match(settings, /CUSTOM_LLM_API_KEY/)
  assert.doesNotMatch(settings, /DEEPSEEK_API_KEY/)
  // the runtime profile references a provider by id instead of an inline provider_config object
  assert.match(api, /provider_id: profile\.provider_id \?\? ''/)
  assert.doesNotMatch(api, /provider_config: profile\.provider_config/)
  // providers are managed through dedicated tools; keychain keys are written to the system credential store
  assert.match(api, /save_model_provider/)
  assert.match(api, /delete_model_provider/)
  assert.match(api, /list_model_providers/)
  assert.match(api, /model_providers/)
})
