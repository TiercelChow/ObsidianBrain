/** Explicit, isolated acceptance steps. No secret reads, automatic approvals or model retries. */
import { createHash } from 'node:crypto'
import { cp, mkdir, readFile, realpath, stat, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const fixture = new URL('../../docs/superpowers/fixtures/wiki-acceptance/', import.meta.url)
const defaultTarget = 'http://127.0.0.1:19988'

export function assertLocalTarget(value) {
  const url = new URL(value)
  if (url.origin !== defaultTarget || url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error('拒绝访问非隔离目标；只允许 http://127.0.0.1:19988')
  }
  return url.origin
}

export function acceptanceBookId(workspace) {
  return 'wiki-acceptance-' + createHash('sha256').update(path.resolve(workspace)).digest('hex').slice(0, 16)
}

export async function canonicalHealth(health) {
  return { ...health, vault: { ...health.vault, path: await realpath(health.vault.path) } }
}

export function compilationNeedsPolling(state) {
  return state.compile_state === 'compiling' && state.compile_phase !== 'waiting_review'
}

export function assertTaskScope(task, knowledgeBaseId, taskId) {
  if (!task || task.id !== taskId || task.knowledge_base_id !== knowledgeBaseId) {
    throw new Error('研究任务不属于本次隔离知识库，拒绝访问')
  }
}

export function taskNeedsPolling(task) {
  return ['queued', 'running'].includes(task.status)
}

export function assertIsolatedTarget(health, books, workspace) {
  if (!path.isAbsolute(workspace) || !path.basename(workspace).startsWith('obsidianbrain-wiki-acceptance.')) {
    throw new Error('必须使用专门创建的验收临时目录')
  }
  if (health?.vault?.path !== path.join(workspace, 'vault')) throw new Error('服务 Vault 与隔离目录不匹配，拒绝写入')
  if (health?.components?.obsidian !== 'disabled') throw new Error('测试实例必须禁用 Obsidian 连接')
  if (!Array.isArray(books) || books.length > 1 || books.some(book => book.id !== acceptanceBookId(workspace) || book.path !== path.join(workspace, 'sources'))) {
    throw new Error('书架包含非本次验收的书籍，拒绝整体替换')
  }
}

export function selectQaCase(manifest, id) {
  const item = manifest.cases.find(item => item.id === id && item.kind === 'qa')
  if (!item?.turns?.length) throw new Error('必须明确选择一个问答案例：Q1、Q2、Q3 或 Q4')
  return item
}

export function parseSseEvents() {
  let buffer = ''
  const parse = block => {
    const data = block.split(/\r?\n/).filter(line => line.startsWith('data:')).map(line => line.slice(5).replace(/^ /, '')).join('\n')
    return data ? [JSON.parse(data)] : []
  }
  return {
    push(text) {
      buffer += text
      const events = []
      let match
      while ((match = /\r?\n\r?\n/.exec(buffer))) {
        events.push(...parse(buffer.slice(0, match.index)))
        buffer = buffer.slice(match.index + match[0].length)
      }
      return events
    },
    finish() {
      const events = parse(buffer)
      buffer = ''
      return events
    },
  }
}

export function summarizeStream(timeline) {
  const terminal = timeline.find(item => item.event.type === 'completed')
  const failure = timeline.find(item => item.event.type === 'error')
  const deltas = timeline.filter(item => item.event.type === 'text_delta' && item.event.delta)
  const phase = timeline.find(item => ['phase', 'planning_ready', 'tool_started'].includes(item.event.type))
  const firstEvidence = timeline.findIndex(item => item.event.type === 'evidence')
  const lastDelta = timeline.findLastIndex(item => item.event.type === 'text_delta' && item.event.delta)
  return {
    completed: Boolean(terminal) && !failure,
    duration_ms: timeline.at(-1)?.elapsed_ms ?? null,
    first_status_ms: phase?.elapsed_ms ?? null,
    first_content_ms: deltas[0]?.elapsed_ms ?? null,
    delta_count: deltas.length,
    streamed_characters: deltas.reduce((total, item) => total + item.event.delta.length, 0),
    stream_matches_final: terminal ? deltas.map(item => item.event.delta).join('') === terminal.event.result?.answer : null,
    evidence_before_content_end: firstEvidence !== -1 && lastDelta !== -1 && firstEvidence < lastDelta,
    delivery: deltas.length > 1 ? 'incremental' : deltas.length === 1 ? 'single_delta' : 'completion_only',
    run_id: terminal?.event.result?.run_id ?? timeline.find(item => item.event.run_id)?.event.run_id ?? null,
    conversation_id: terminal?.event.result?.conversation_id ?? null,
    error: failure?.event.message ?? null,
    semantic_review: 'not_reviewed',
  }
}

async function request(origin, tool, args = {}) {
  const response = await fetch(origin + '/v1/tools/call', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ tool, arguments: args }), signal: AbortSignal.timeout(60_000),
  })
  const envelope = await response.json()
  if (!response.ok || envelope.status !== 'success') throw new Error(envelope.error?.message ?? `${tool}: HTTP ${response.status}`)
  return envelope.result
}

async function guard(origin, workspace) {
  const health = await canonicalHealth(await (await fetch(origin + '/v1/health', { signal: AbortSignal.timeout(5000) })).json())
  const { books } = await request(origin, 'get_reader_books')
  assertIsolatedTarget(health, books, workspace)
  return books
}

async function saveRecord(workspace, name, value) {
  await mkdir(path.join(workspace, 'results'), { recursive: true })
  await writeFile(path.join(workspace, 'results', name), JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 })
}

async function streamTurn(origin, knowledgeBaseId, question, conversationId) {
  const timeline = []
  const start = performance.now()
  const receive = events => {
    for (const event of events) timeline.push({ elapsed_ms: Math.round(performance.now() - start), event })
  }
  try {
    const response = await fetch(origin + '/v1/knowledge/chat/stream', {
      method: 'POST', headers: { 'Content-Type': 'application/json', Accept: 'text/event-stream' },
      body: JSON.stringify({ knowledge_base_id: knowledgeBaseId, question, ...(conversationId ? { conversation_id: conversationId } : {}) }),
      signal: AbortSignal.timeout(2_520_000),
    })
    if (!response.ok || !response.body) throw new Error(`问答流 HTTP ${response.status}`)
    const parser = parseSseEvents()
    const decoder = new TextDecoder()
    for await (const bytes of response.body) receive(parser.push(decoder.decode(bytes, { stream: true })))
    receive(parser.push(decoder.decode()))
    receive(parser.finish())
    if (!timeline.some(item => ['completed', 'error'].includes(item.event.type))) throw new Error('流意外结束，没有完成事件')
  } catch (error) {
    receive([{ type: 'error', message: error.message }])
  }
  return { question, timeline, metrics: summarizeStream(timeline) }
}

async function run(argv) {
  const [command, ...flags] = argv
  const options = {}
  for (let index = 0; index < flags.length; index += 2) {
    if (!['--workspace', '--case', '--task-id'].includes(flags[index]) || !flags[index + 1] || options[flags[index]]) throw new Error('参数：prepare|status|compile|qa|watch-task --workspace <隔离目录> [--case Q1] [--task-id ID]')
    options[flags[index]] = flags[index + 1]
  }
  if (!['prepare', 'status', 'compile', 'qa', 'watch-task'].includes(command) || !options['--workspace']) throw new Error('参数：prepare|status|compile|qa|watch-task --workspace <隔离目录> [--case Q1] [--task-id ID]')
  const workspace = await realpath(options['--workspace'])
  const origin = assertLocalTarget(defaultTarget)
  const books = await guard(origin, workspace)
  const manifest = JSON.parse(await readFile(new URL('cases.json', fixture), 'utf8'))
  const bookId = acceptanceBookId(workspace)

  if (command === 'prepare') {
    if (books.length) throw new Error('材料已准备；不覆盖来源副本，请使用 status 查看')
    try {
      await stat(path.join(workspace, 'sources'))
      throw new Error('来源副本已存在，拒绝覆盖')
    } catch (error) {
      if (error.code !== 'ENOENT') throw error
    }
    await cp(new URL('sources/', fixture), path.join(workspace, 'sources'), { recursive: true, errorOnExist: true, force: false })
    await guard(origin, workspace)
    await request(origin, 'save_reader_books', { books: [{
      id: bookId, path: path.join(workspace, 'sources'), kind: 'folder', name: manifest.book_name,
      description: '虚构、隔离、不可用于生产建议的 Wiki 验收材料', category: '隔离验收', addedAt: Date.now(),
    }] })
    const initialized = await request(origin, 'initialize_book_knowledge_base', { book_id: bookId, sync: true })
    await saveRecord(workspace, 'preparation.json', { prepared_at: new Date().toISOString(), book_id: bookId, initialized, manifest })
    console.log('来源已复制并同步；未调用模型或自动批准。结果：' + path.join(workspace, 'results/preparation.json'))
    return
  }

  if (!books.length) throw new Error('请先执行 prepare')
  const cards = await request(origin, 'list_book_knowledge_bases')
  const base = cards.items.find(item => item.book.id === bookId)?.knowledge_base
  if (!base) throw new Error('验收知识库不存在')
  if (command === 'status') {
    const settings = await request(origin, 'get_book_wiki_settings')
    console.log(JSON.stringify({ knowledge_base: base, runtime_profiles: settings.runtime_profiles, model_providers: settings.model_providers }, null, 2))
    return
  }

  if (command === 'watch-task') {
    const taskId = options['--task-id']
    if (!taskId) throw new Error('必须明确指定已启动的隔离任务 --task-id；不自动创建、执行或恢复任务')
    const started = performance.now()
    const timeline = []
    let signature
    let task
    let workspaceState
    do {
      await guard(origin, workspace)
      task = (await request(origin, 'list_knowledge_tasks', { knowledge_base_id: base.id })).tasks.find(item => item.id === taskId)
      assertTaskScope(task, base.id, taskId)
      workspaceState = await request(origin, 'get_knowledge_research_workspace', { task_id: taskId })
      const next = JSON.stringify([task.status, task.artifact_state, workspaceState?.stages])
      if (signature !== next) {
        const state = { elapsed_ms: Math.round(performance.now() - started), task, workspace: workspaceState }
        timeline.push(state)
        console.log(JSON.stringify({ elapsed_ms: state.elapsed_ms, status: task.status, artifact: task.artifact_state, stages: workspaceState?.stages?.map(stage => ({ key: stage.stage_key, status: stage.status, error: stage.error })) }))
        signature = next
      }
      if (!taskNeedsPolling(task)) break
      if (performance.now() - started > 1_800_000) throw new Error('任务观察已到30分钟上限；不自动取消或重新执行')
      await new Promise(resolve => setTimeout(resolve, 5000))
    } while (true)
    const stages = []
    for (const stage of workspaceState?.stages ?? []) {
      stages.push(await request(origin, 'get_knowledge_research_stage', { task_id: taskId, stage_key: stage.stage_key, revision: stage.revision }))
    }
    let result
    let resultError
    try { result = await request(origin, 'get_knowledge_task_result', { task_id: taskId }) }
    catch (error) { resultError = error.message }
    const name = 'task-' + taskId + '-' + new Date().toISOString().replaceAll(':', '-') + '.json'
    await saveRecord(workspace, name, { timeline, task, workspace: workspaceState, stages, result, result_error: resultError, semantic_review: 'not_reviewed', visual_review: 'not_reviewed' })
    console.log('已保存研究阶段与成果元数据，需人工核对内容与PPT：' + path.join(workspace, 'results', name))
    if (['failed', 'cancelled'].includes(task.status)) process.exitCode = 1
    return
  }

  if (command === 'compile') {
    if (base.compile_state === 'compiling' || base.pending_review_count > 0) throw new Error('已有编译或待审核候选，拒绝重复调用')
    const started = performance.now()
    const timeline = []
    let state = (await request(origin, 'compile_book_knowledge_base', { knowledge_base_id: base.id })).knowledge_base
    let signature
    do {
      const next = JSON.stringify([state.compile_state, state.compile_phase, state.compile_current_batch, state.compile_message, state.compile_error])
      if (signature !== next) {
        timeline.push({ elapsed_ms: Math.round(performance.now() - started), state })
        console.log(JSON.stringify({ elapsed_ms: Math.round(performance.now() - started), state: state.compile_state, phase: state.compile_phase, batch: state.compile_current_batch, message: state.compile_message, error: state.compile_error }))
        signature = next
      }
      if (!compilationNeedsPolling(state)) break
      if (performance.now() - started > 1_800_000) throw new Error('编译观察已到30分钟上限；不自动取消或重新排队，请查看阶段状态')
      await new Promise(resolve => setTimeout(resolve, 2000))
      await guard(origin, workspace)
      state = await request(origin, 'get_book_knowledge_base', { knowledge_base_id: base.id })
    } while (true)
    const changes = await request(origin, 'list_knowledge_change_sets', { knowledge_base_id: base.id })
    const report = await request(origin, 'get_knowledge_compile_report', { knowledge_base_id: base.id })
    const name = 'W1-' + new Date().toISOString().replaceAll(':', '-') + '.json'
    await saveRecord(workspace, name, { started_at: new Date(Date.now() - (performance.now() - started)).toISOString(), timeline, state, report, changes, semantic_review: 'not_reviewed' })
    console.log('已保存编译合同与候选，尚未批准：' + path.join(workspace, 'results', name))
    if (state.compile_state === 'failed') process.exitCode = 1
    return
  }

  const item = selectQaCase(manifest, options['--case'])
  if (base.compile_mode !== 'smart' || base.compile_state !== 'ready' || base.pending_review_count > 0) throw new Error('真实问答验收需要先完成智能编译并人工审核；不使用按章兜底结果代替')
  const record = { started_at: new Date().toISOString(), origin, case: item, knowledge_base: base, turns: [] }
  let conversationId
  for (const question of item.turns) {
    await guard(origin, workspace)
    const turn = await streamTurn(origin, base.id, question, conversationId)
    record.turns.push(turn)
    console.log(JSON.stringify(turn.metrics))
    if (!turn.metrics.completed) break
    conversationId = turn.metrics.conversation_id
    if (!conversationId) break
  }
  if (conversationId) {
    try {
      record.persisted_conversation = await request(origin, 'get_knowledge_conversation', { conversation_id: conversationId })
    } catch (error) { record.history_error = error.message }
  }
  const name = item.id + '-' + new Date().toISOString().replaceAll(':', '-') + '.json'
  await saveRecord(workspace, name, record)
  console.log('已保存原始输出与接收时间，需人工核对语义：' + path.join(workspace, 'results', name))
  if (record.turns.length !== item.turns.length || record.turns.some(turn => !turn.metrics.completed)) process.exitCode = 1
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(process.argv.slice(2)).catch(error => { console.error(error.message); process.exitCode = 1 })
}
