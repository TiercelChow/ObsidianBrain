// Run-scoped Cordis plugin. No tools, credentials, global state or private SDK APIs.
export const name = 'obsidianbrain-stream'
export const inject = ['llm', 'agents', 'sessions']
export function installBridge(ctx, send) {
  const calls = new Map()
  const emit = (sessionId, data) => send({
    jsonrpc: '2.0', method: 'session/update', params: {
      sessionId,
      update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: '' } },
      _meta: { 'obsidianbrain/stream': { version: 1, ...data } },
    },
  })
  // Agent frames exclude title/compaction calls and carry exact retry identity.
  ctx.on('agent/assistant-stream', ({ agent, frame }) => {
    const sessionId = agent.session.id, id = String(frame.attemptId)
    if (frame.type === 'start') {
      calls.set(sessionId, { id, thinking: false })
      emit(sessionId, { op: 'begin', call_id: id })
      return
    }
    const call = calls.get(sessionId)
    if (!call || call.id !== id || frame.type !== 'chunk') return
    const chunk = frame.chunk
    if (chunk.type === 'text-delta' && chunk.text) emit(sessionId, { op: 'append', call_id: id, text: chunk.text })
    if (chunk.type === 'reasoning-delta' && !call.thinking) {
      call.thinking = true
      emit(sessionId, { op: 'thinking', call_id: id })
    }
    if (chunk.type === 'finish') emit(sessionId, { op: 'finish', call_id: id, tool_round: chunk.reason.kind === 'tool-calls' })
  })
  ctx.on('session/event', (session, event) => {
    if (event.type !== 'assistant/message') return
    const sessionId = session.header.id, call = calls.get(sessionId)
    if (!call) return
    const content = event.data.message.content
    emit(sessionId, {
      op: 'commit', call_id: call.id,
      tool_round: content.some(b => b.type === 'tool-call'),
      text: content.filter(b => b.type === 'text').map(b => b.text).join(''),
    })
    calls.delete(sessionId)
  })
}
export function apply(ctx) {
  // ACP is newline-delimited JSON-RPC. One atomic write per frame; no logging on stdout.
  installBridge(ctx, frame => process.stdout.write(JSON.stringify(frame) + '\n'))
}
