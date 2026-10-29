/** Isolated UI fixture server. Never proxies to the real backend or runs a model.
 * All writes affect these in-memory objects only; closing the process discards them.
 * Open /tasks?fixtureTheme=light (dark / eye-care also supported).
 */
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer, loadConfigFromFile } from 'vite'

const root = fileURLToPath(new URL('../', import.meta.url))
const cache = await mkdtemp(join(tmpdir(), 'mobile-task-review-'))
const date = new Date()
const dateKey = value => `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, '0')}-${String(value.getDate()).padStart(2, '0')}`
const today = dateKey(date)
const offsetDate = days => { const value = new Date(date); value.setDate(value.getDate() + days); return dateKey(value) }
const timestamp = new Date().toISOString()
const node = (id, title, patch = {}) => ({ id, root_id: id, parent_id: null, kind: 'long', role: 'root', title, description: '', start_date: offsetDate(-5), end_date: offsetDate(7), importance: 'normal', status: 'planned', position: 0, revision: 1, archived_at: null, closed_at: null, closure_note: null, created_at: timestamp, updated_at: timestamp, ...patch })
const main = node('fixture-long', '完成个人知识工作台改版', { status: 'in_progress', description: '统一阅读、记录与任务的日常使用体验，让信息更清楚、操作更顺手。', importance: 'high' })
const child = node('fixture-child', '梳理任务的信息层级', { root_id: main.id, parent_id: main.id, role: 'subtask', status: 'in_progress', end_date: today, description: '先确定列表、详情和子任务的呈现方式，再逐步验证交互。' })
const grandchild = node('fixture-grandchild', '验证二级子任务：长标题换行后仍与正文左边缘保持一致', { root_id: main.id, parent_id: child.id, role: 'subtask', end_date: today })
const leaf = node('fixture-leaf', '检查三级任务与返回路径', { root_id: main.id, parent_id: grandchild.id, role: 'subtask', end_date: today })
const otherChild = node('fixture-other-child', '完成移动端交互走查', { root_id: main.id, parent_id: main.id, role: 'subtask', position: 1, end_date: offsetDate(3) })
const finishedChild = node('fixture-finished-child', '整理设计参考与约束', { root_id: main.id, parent_id: main.id, role: 'subtask', position: 2, status: 'completed', closed_at: timestamp })
const roots = [
  node('fixture-overdue', '确认下个月的阅读安排', { kind: 'short', status: 'open', importance: 'urgent', end_date: offsetDate(-2) }),
  node('fixture-short', '整理本周阅读笔记', { kind: 'short', status: 'open', start_date: today, end_date: today }),
  main,
  node('fixture-blocked', '等待设计资料补充', { status: 'blocked' }),
  ...['整理主题知识库', '完成阅读计划', '归纳研究结论', '优化日常工作流'].map((title, index) => node(`fixture-extra-${index}`, title, { end_date: offsetDate(2 + index) })),
  node('fixture-done', '归档已完成的学习专题', { status: 'completed', closed_at: timestamp, closure_note: '已整理所有成果。' }),
]
const documents = new Map(roots.map(task => [task.id, {
  root: task, tasks: task.id === main.id ? [task, child, grandchild, leaf, otherChild, finishedChild] : [task],
  progress: task.id === main.id ? [{ id: 'fixture-progress', root_id: task.id, task_id: child.id, recorded_at: timestamp, created_at: timestamp, percent_after: 40, note: '已完成首屏信息梳理，移除重复标签。\n下一步验证小屏和长标题，确保操作入口始终可见。' }] : [],
  audit: [{ id: `audit-${task.id}`, root_id: task.id, task_id: task.id, event_type: 'status_changed', from_status: 'planned', to_status: task.status, note: task.closure_note, occurred_at: timestamp }],
  storage_path: `fixture:${task.id}`, document_version: { revision: 1, content_hash: 'fixture:1' }, progress_percent: task.id === main.id ? 40 : task.status === 'completed' ? 100 : 0, completed_leaf_count: 1, effective_leaf_count: task.id === main.id ? 3 : 1,
}]))
const calls = [], unexpected = []
const summary = task => ({ ...task, ...Object.fromEntries(['storage_path', 'document_version', 'progress_percent', 'completed_leaf_count', 'effective_leaf_count'].map(key => [key, documents.get(task.root_id)[key]])), derived: { overdue: task.end_date < today, active_today: task.start_date <= today && task.end_date >= today, due_today: task.end_date === today } })
const find = id => [...documents.values()].find(doc => doc.tasks.some(task => task.id === id))
const matches = (task, args) => !task.archived_at && (!args.kinds || args.kinds.includes(task.kind)) && (!args.statuses || args.statuses.includes(task.status)) && (!args.query || `${task.title} ${task.description}`.includes(args.query))
function read(tool, args) {
  if (tool === 'browse_timeline') return { memos: [], total: 0, has_more: false }
  if (tool === 'list_tasks') return { tasks: [...documents.values()].map(doc => doc.root).filter(task => matches(task, args)).map(summary), next_cursor: null }
  if (tool === 'get_task') return find(args.task_id)
  if (tool === 'get_task_calendar') return [...documents.values()].flatMap(doc => doc.tasks).filter(task => matches(task, args) && task.start_date <= args.end_date && task.end_date >= args.start_date).map(summary)
}
const writes = ['create_task', 'update_task', 'add_subtask', 'add_task_progress', 'set_task_status', 'move_subtask', 'archive_task']
function write(tool, args) {
  if (tool === 'create_task') {
    const task = node(`fixture-created-${documents.size}`, args.title, { ...args, status: args.kind === 'short' ? 'open' : 'planned' })
    const doc = { root: task, tasks: [task], progress: [], audit: [], storage_path: `fixture:${task.id}`, document_version: { revision: 1, content_hash: 'fixture:1' }, progress_percent: 0, completed_leaf_count: 0, effective_leaf_count: 1 }
    documents.set(task.id, doc); return { task: doc }
  }
  const doc = find(args.task_id || args.parent_id)
  if (!doc) throw new Error('Fixture task not found')
  if (args.expected_version?.revision !== doc.document_version.revision || args.expected_version?.content_hash !== doc.document_version.content_hash) throw new Error('Fixture version conflict')
  if (args.note?.includes('模拟失败')) throw new Error('隔离测试：保存失败，输入应保留')
  const task = doc.tasks.find(task => task.id === args.task_id)
  if (tool === 'update_task') Object.assign(task, args.patch)
  if (tool === 'add_subtask') {
    if ('kind' in args) throw new Error('Subtasks must not send kind')
    doc.tasks.push(node(`fixture-child-${doc.tasks.length}`, args.title, { ...args, kind: 'long', role: 'subtask', root_id: doc.root.id, position: doc.tasks.length }))
  }
  if (tool === 'add_task_progress') doc.progress.unshift({ id: `fixture-progress-${doc.progress.length}`, root_id: doc.root.id, task_id: args.task_id, note: args.note, percent_after: args.percent_after ?? null, recorded_at: timestamp, created_at: timestamp })
  if (tool === 'set_task_status') {
    doc.audit.unshift({ id: `fixture-audit-${doc.audit.length}`, root_id: doc.root.id, task_id: task.id, event_type: 'status_changed', from_status: task.status, to_status: args.status, note: args.closure_note || null, occurred_at: timestamp })
    Object.assign(task, { status: args.status, closure_note: args.closure_note || null })
  }
  if (tool === 'move_subtask') Object.assign(task, { parent_id: args.new_parent_id, position: args.position })
  if (tool === 'archive_task') doc.root.archived_at = args.archived ? timestamp : null
  const revision = doc.document_version.revision + 1
  doc.document_version = { revision, content_hash: `fixture:${revision}` }
  return { task: doc }
}
const mock = { name: 'isolated-mobile-task-review',
  transformIndexHtml(html) { return html.replace('<head>', `<head><script>localStorage.setItem('theme', new URLSearchParams(location.search).get('fixtureTheme') || 'light')</script>`) },
  configureServer(server) { server.middlewares.use(async (request, response, next) => {
    if (request.url === '/__fixture/status') { response.setHeader('Content-Type', 'application/json'); return response.end(JSON.stringify({ calls, unexpected, documents: [...documents.values()] })) }
    if (!request.url?.startsWith('/v1')) return next()
    response.setHeader('Content-Type', 'application/json')
    if (request.url === '/v1/health') return response.end(JSON.stringify({ status: 'healthy', version: 'isolated-fixture', components: {}, uptime_seconds: 1, tools_count: 0 }))
    let body = ''; for await (const chunk of request) body += chunk
    const { tool, arguments: args = {} } = body ? JSON.parse(body) : {}
    calls.push({ tool, arguments: args })
    if (request.url !== '/v1/tools/call' || !['list_tasks', 'get_task', 'get_task_calendar', 'browse_timeline', ...writes].includes(tool)) {
      unexpected.push(tool || request.url); response.statusCode = 501
      return response.end(JSON.stringify({ status: 'error', result: null, error: { message: 'Real backend forbidden' } }))
    }
    try {
      const result = writes.includes(tool) ? write(tool, args) : read(tool, args)
      response.end(JSON.stringify({ status: 'success', result }))
    } catch (error) { response.end(JSON.stringify({ status: 'error', result: null, error: { code: 'FIXTURE_WRITE_FAILED', message: error.message } })) }
  }) },
}
const config = await loadConfigFromFile({ command: 'serve', mode: 'development' }, join(root, 'vite.config.ts'))
const server = await createServer({ ...config.config, root, configFile: false, cacheDir: cache, plugins: [mock, ...config.config.plugins], logLevel: 'error', server: { host: '127.0.0.1', port: 0, proxy: {} } })
await server.listen()
console.log(`Isolated task UI: http://127.0.0.1:${server.httpServer.address().port}/tasks`)
process.on('SIGTERM', async () => { await server.close(); process.exit(0) })
process.on('SIGINT', async () => { await server.close(); process.exit(0) })
