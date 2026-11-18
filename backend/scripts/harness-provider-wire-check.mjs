/** Local-only test of the installed Harness adapter's actual HTTP payload.
 * Production-generated patches arrive on stdin. No supplier requests, real
 * credentials, persistent writes, or printing of headers/prompts.
 */
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { pathToFileURL } from 'node:url'

const { apply, Config } = await import(pathToFileURL(process.argv[2]).href)
const input = []
for await (const chunk of process.stdin) input.push(chunk)
const patches = JSON.parse(Buffer.concat(input).toString('utf8'))
const payloads = []
const server = createServer(async (request, response) => {
  assert.equal(request.url, '/v1/chat/completions')
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  payloads.push(JSON.parse(Buffer.concat(chunks).toString('utf8')))
  response.writeHead(200, { 'Content-Type': 'text/event-stream' })
  const common = { id: 'local-only', object: 'chat.completion.chunk', created: 1, model: 'deepseek-v4.1-flash' }
  response.write('data: ' + JSON.stringify({ ...common, choices: [{ index: 0, delta: { role: 'assistant', content: 'READY' }, finish_reason: null }] }) + '\n\n')
  response.end('data: ' + JSON.stringify({ ...common, choices: [{ index: 0, delta: {}, finish_reason: 'stop' }] }) + '\n\ndata: [DONE]\n\n')
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
try {
  for (const [label, patch] of Object.entries(patches)) {
    const config = patch.find(entry => entry.id === 'llm-pi-ai').config
    const route = Object.keys(config.providers)[0]
    const profile = config.providers[route]
    profile.baseURL = `http://127.0.0.1:${server.address().port}/v1`
    let adapter
    const credentials = { resolve: async () => ({ value: 'local-wire-test-only' }), readRecord: async () => undefined, listRecords: async () => [] }
    apply({
      get: name => name === 'credentials' ? credentials : undefined,
      inject() {},
      fiber: {},
      on() {},
      waterfall: (...args) => args.at(-1)(),
      logger: { warn() {} },
      llm: {
        registerAdapter: (_, value) => { adapter = value; return { replace() {} } },
        registerConfigurableProviders: () => ({ replace() {} }),
        registerModelDiscovery() {},
      },
    }, Config ? Config(config) : config)
    assert.ok(adapter)
    let completed = false
    for await (const event of adapter.stream({
      provider: route, model: profile.models[0].id,
      system: 'Local test system instructions',
      messages: [{ role: 'user', content: [{ type: 'text', text: 'Reply READY' }] }],
      maxTokens: 10016, signal: AbortSignal.timeout(15_000),
    })) {
      if (event.type === 'error') throw new Error('Local Harness adapter failed')
      if (event.type === 'finish') {
        assert.equal(event.reason.kind, 'stop', 'Harness must finish successfully')
        completed = true
      }
    }
    assert.equal(payloads.length, Object.keys(patches).indexOf(label) + 1)
    const wire = payloads.at(-1)
    if (label.endsWith('auto')) {
      assert.equal(wire.enable_thinking, undefined)
      assert.equal(wire.thinking, undefined, 'QA auto must keep the supplier default')
    } else {
      assert.equal(wire.messages[0].role, 'system')
      if (label === 'deepseek_compile') {
        assert.equal(wire.enable_thinking, undefined)
        assert.deepEqual(wire.thinking, { type: 'disabled' })
      } else {
        assert.equal(wire.enable_thinking, false, `${label} must disable thinking on the wire`)
      }
    }
    assert.equal(wire.max_tokens ?? wire.max_completion_tokens, 10016)
    assert.ok(completed, 'Adapter must finish, not merely submit the HTTP request')
    console.log(JSON.stringify({ case: label, local_only: true, thinking: wire.thinking?.type ?? wire.enable_thinking ?? 'supplier_default', passed: true }))
  }
} finally {
  server.closeAllConnections()
  await new Promise(resolve => server.close(resolve))
}
