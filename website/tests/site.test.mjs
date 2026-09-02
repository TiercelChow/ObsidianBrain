import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)

async function read(path) {
  return readFile(new URL(path, root), 'utf8')
}

test('product site contains the complete introduction and onboarding journey', async () => {
  const html = await read('index.html')

  for (const id of ['product', 'workflow', 'features', 'quick-start', 'guide', 'privacy', 'faq']) {
    assert.match(html, new RegExp(`id=["']${id}["']`), `missing #${id}`)
  }

  for (const moduleName of ['阅境轩', '时光机', '任务中枢', 'Wiki 工作台', '知识库', '代码仓', '灵感熔炉', '智识雷达']) {
    assert.match(html, new RegExp(moduleName), `missing ${moduleName}`)
  }
})

test('product site describes installation, configuration and module usage', async () => {
  const html = await read('index.html')

  for (const phrase of ['make build', 'obsidian-brain start', 'Obsidian Local REST API', 'LLM 配置', '局域网访问']) {
    assert.match(html, new RegExp(phrase), `missing usage phrase: ${phrase}`)
  }
})

test('site assets use repository-relative paths and never call the local API', async () => {
  const [html, script] = await Promise.all([read('index.html'), read('src/main.js')])

  assert.doesNotMatch(html, /(?:src|href)=["']\//)
  assert.doesNotMatch(`${html}\n${script}`, /(?:fetch|axios)\s*\(|\/v1\//)
})

test('Vite and Pages workflow publish the website subproject', async () => {
  const [config, workflow] = await Promise.all([
    read('vite.config.js'),
    read('../.github/workflows/deploy-pages.yml'),
  ])

  assert.match(config, /base:\s*['"]\/ObsidianBrain\/['"]/)
  assert.match(workflow, /working-directory:\s*website/)
  assert.match(workflow, /path:\s*website\/dist/)
  assert.match(workflow, /pages:\s*write/)
  assert.match(workflow, /id-token:\s*write/)
})
