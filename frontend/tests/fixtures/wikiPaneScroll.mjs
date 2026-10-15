// Isolated UI verification: synthetic books/history, no real storage or model calls.
// Run from frontend: node tests/fixtures/wikiPaneScroll.mjs
import { createServer as createHttpServer } from 'node:http'
import { fileURLToPath } from 'node:url'
import { createServer as createViteServer } from 'vite'

const timestamp = '2026-10-14T08:00:00Z'
const bases = ['active', 'paused'].map(lifecycle => ({
  id: `preview-${lifecycle}`, book_id: `book-${lifecycle}`, book_name: lifecycle === 'active' ? '独立滚动验收' : '暂停知识库验收',
  book_path: '/preview/book', book_kind: 'folder', book_description: '', book_category: '',
  lifecycle, source_available: true, sync_state: 'clean', compile_mode: 'smart', compile_state: 'ready',
  compile_phase: 'completed', compile_message: null, compile_current_batch: 0, compile_total_batches: 0,
  compile_active_run_id: null, compile_change_set_id: null, compile_started_at: null, compile_heartbeat_at: null,
  compile_cancel_requested: false, health_state: 'healthy', last_error: null, compile_processed_sources: 8,
  compile_total_sources: 8, pending_review_count: 0, source_count: 8, entry_count: 70, claim_count: 0, task_count: 0,
}))
const entries = baseId => Array.from({ length: 70 }, (_, i) => ({
  id: `${baseId}-entry-${i}`, knowledge_base_id: baseId, entry_type: 'concept', slug: `concept-${i}`,
  title: `概念 ${String(i + 1).padStart(2, '0')} · 知识结构与证据`,
  summary: '用于验证实体列表长内容滚动，正文与列表不应互相影响。',
  status: 'verified', source_path: `chapters/chapter-${i + 1}.md`, updated_at: timestamp,
}))
const conversations = baseId => Array.from({ length: 30 }, (_, i) => ({
  id: `${baseId}-chat-${i}`, knowledge_base_id: baseId, title: `知识讨论 ${i + 1} · 如何组织可靠的证据？`,
  message_count: 2, created_at: timestamp, updated_at: timestamp,
}))

function readOnlyResult(tool, args, referer) {
  const baseId = args.knowledge_base_id || bases[0].id
  switch (tool) {
    case 'list_book_knowledge_bases': return { items: referer.includes('preview=empty') ? [] : bases.map(base => ({
      book: { id: base.book_id, name: base.book_name, path: base.book_path, kind: 'folder', addedAt: 1 }, knowledge_base: base,
    })) }
    case 'list_knowledge_entries': {
      const matches = entries(baseId).filter(entry => !args.query || `${entry.title} ${entry.summary}`.includes(args.query))
      const offset = args.offset || 0
      const limit = args.limit || 60
      return { entries: matches.slice(offset, offset + limit), offset, limit, total: matches.length, has_more: offset + limit < matches.length }
    }
    case 'get_knowledge_entry': {
      const entry = bases.flatMap(base => entries(base.id)).find(item => item.id === args.entry_id)
      if (!entry) throw new Error('Unknown preview entity')
      return { ...entry, revision: 1, edit_policy: 'agent_managed', aliases: [], claims: [], relations: [], citations: [], versions: [],
        content_md: Array.from({ length: 60 }, (_, i) => `## 证据段落 ${i + 1}\n\n独立阅读区域应保持稳定，左侧实体列表滑动时，本段正文的位置不应改变。通过明确的来源和论断建立知识之间的联系。`).join('\n\n'),
      }
    }
    case 'list_knowledge_change_sets': return { change_sets: [] }
    case 'list_knowledge_tasks': return { tasks: [] }
    case 'list_wiki_skills': return { skills: [] }
    case 'get_book_wiki_settings': return { runtime_profiles: [], model_providers: [], documents: [] }
    case 'list_knowledge_conversations': return { conversations: conversations(baseId) }
    case 'list_unfinished_qa_runs': return { runs: [] }
    case 'get_knowledge_conversation': {
      const conversation = bases.flatMap(base => conversations(base.id)).find(item => item.id === args.conversation_id)
      if (!conversation) throw new Error('Unknown preview conversation')
      const evidence = /-chat-(0|2|3)$/.test(conversation.id) ? entries(conversation.knowledge_base_id).slice(0, 3) : []
      const messages = [
        { id: 'preview-question', role: 'user', content: '如何组织可靠的证据？', evidence: [], created_at: timestamp },
        { id: 'preview-answer', role: 'assistant', run_id: conversation.id.endsWith('-3') ? null : 'preview-run', content: '## 先明确来源\n\n将知识整理成可核验的论断，并保留来源引用。\n\n- 先读证据，再形成结论。\n- 有疑问时明确标注。', evidence, created_at: timestamp },
      ]
      if (conversation.id.endsWith('-2')) messages.push(
        { id: 'preview-followup', role: 'user', content: '不同回答的参考来源能分别展开吗？', evidence: [], created_at: timestamp },
        { id: 'preview-followup-answer', role: 'assistant', run_id: 'preview-run', content: '可以，每条回答的参考来源独立展开，不会影响其他回答。', evidence, created_at: timestamp },
      )
      return { ...conversation, messages }
    }
    case 'get_agent_run_citation': {
      const entry = entries(bases[0].id)[args.source_index]
      if (!entry) throw new Error('Unknown preview citation')
      return { run_id: 'preview-run', citation_index: args.source_index + 1, kind: 'entry', object_id: entry.id,
        version_id: '1', entry, content_md: '## 来源快照\n\n这是隔离验收使用的原始证据，不会访问实际文件。',
        citations: [], historical: true, read_ranges: [],
      }
    }
    default: throw new Error('Preview blocks all other operations')
  }
}

const mock = createHttpServer(async (request, response) => {
  try {
    if (request.method !== 'POST' || request.url !== '/v1/tools/call') throw new Error('Read-only tool preview only')
    const parts = []
    for await (const part of request) parts.push(part)
    const call = JSON.parse(Buffer.concat(parts).toString())
    const result = readOnlyResult(call.tool, call.arguments || {}, request.headers.referer || '')
    response.writeHead(200, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify({ status: 'success', result }))
  } catch {
    response.writeHead(400, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify({ status: 'error', error: { message: '仅支持布局验收的只读模拟操作' } }))
  }
})
await new Promise((resolve, reject) => {
  mock.once('error', reject)
  mock.listen(0, '127.0.0.1', resolve)
})
const vite = await createViteServer({
  root: fileURLToPath(new URL('../../', import.meta.url)),
  configFile: fileURLToPath(new URL('../../vite.config.ts', import.meta.url)),
  server: {
    host: '127.0.0.1', port: 5231, strictPort: true,
    proxy: { '/v1': { target: `http://127.0.0.1:${mock.address().port}`, changeOrigin: true } },
  },
})
await vite.listen()
console.log('Isolated Wiki preview: http://127.0.0.1:5231/knowledge/wiki')
let stopping = false
async function stop() {
  if (stopping) return
  stopping = true
  await vite.close()
  mock.close(() => process.exit(0))
}
process.on('SIGINT', stop)
process.on('SIGTERM', stop)
