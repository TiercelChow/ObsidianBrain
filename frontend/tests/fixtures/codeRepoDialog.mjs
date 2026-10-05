// Isolated UI preview: synthetic metadata only, no database or real repository calls.
// Run from frontend: node tests/fixtures/codeRepoDialog.mjs
import { createServer as createHttpServer } from 'node:http'
import { fileURLToPath } from 'node:url'
import { createServer as createViteServer } from 'vite'

const makeRepo = (name, long = false) => ({
  name,
  path: long ? `/workspace/${'very-long-path-segment/'.repeat(45)}project` : '/workspace/demo',
  current_branch: long ? `feature/${'long-branch-name-'.repeat(16)}` : 'main',
  is_dirty: false,
  languages: long ? { Rust: .6, TypeScript: .3, ['VeryLongLanguageName'.repeat(8)]: .1 } : { Rust: 1 },
  linked_notes_count: 0,
  head_hash: 'a1234567890abcdef',
  total_commits: long ? 12500 : 0,
  contributors: long ? Array.from({ length: 48 }, (_, i) => `Contributor${i}-${'LongName'.repeat(8)}`) : ['Demo'],
  branches: ['main'],
  recent_commits: long ? Array.from({ length: 20 }, (_, i) => ({
    hash: `${String(i + 1).padStart(7, '0')}abc`,
    message: `提交 ${i + 1}：${'长内容应完整换行，不应撑大弹窗。'.repeat(16)}\n${'NoWhitespaceText'.repeat(24)}`,
    author: 'LongAuthorName'.repeat(12),
    timestamp: '2026-10-04T00:00:00Z',
  })) : [],
})
const repos = [makeRepo('长内容布局验收', true), makeRepo('短内容布局验收')]
const mock = createHttpServer(async (request, response) => {
  const parts = []
  for await (const part of request) parts.push(part)
  try {
    const call = JSON.parse(Buffer.concat(parts).toString())
    let result
    if (call.tool === 'list_code_repos') result = { repos }
    else if (call.tool === 'get_repo_detail') result = repos.find(repo => repo.name === call.arguments?.name)
    else throw new Error('Preview does not perform this operation')
    if (!result) throw new Error('Unknown preview repository')
    response.writeHead(200, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify({ status: 'success', result }))
  } catch {
    response.writeHead(400, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify({ status: 'error', error: { message: '仅支持布局验收的只读操作' } }))
  }
})
await new Promise((resolve, reject) => {
  mock.once('error', reject)
  mock.listen(22989, '127.0.0.1', resolve)
})
const vite = await createViteServer({
  root: fileURLToPath(new URL('../../', import.meta.url)),
  configFile: fileURLToPath(new URL('../../vite.config.ts', import.meta.url)),
  server: {
    host: '127.0.0.1', port: 5219, strictPort: true,
    proxy: { '/v1': { target: 'http://127.0.0.1:22989', changeOrigin: true } },
  },
})
await vite.listen()
console.log('Isolated repo dialog preview: http://127.0.0.1:5219/code-repo')
let stopping = false
async function stop() {
  if (stopping) return
  stopping = true
  await vite.close()
  mock.close(() => process.exit(0))
}
process.on('SIGINT', stop)
process.on('SIGTERM', stop)
