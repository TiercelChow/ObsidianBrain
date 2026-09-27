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

  for (const moduleName of ['阅境轩', '时光机', '任务中枢', '书籍知识库', 'Wiki 工作台', '书籍问答', '研究任务', 'Wiki 配置']) {
    assert.match(html, new RegExp(moduleName), `missing ${moduleName}`)
  }
})

test('product site describes installation, configuration and module usage', async () => {
  const html = `${await read('index.html')}\n${await read('manual/index.html')}`

  for (const phrase of ['make build', 'obsidian-brain start', 'Obsidian Local REST API', 'DeepSeek Harness', 'CUSTOM_LLM_API_KEY', '局域网访问']) {
    assert.match(html, new RegExp(phrase), `missing usage phrase: ${phrase}`)
  }
})

test('product site describes the database-native Book Wiki workflow and current limits', async () => {
  const html = await read('index.html')

  for (const phrase of ['每本书一个独立知识库', '跨章节归并', '人工审核', '真实样例基准', 'PPTX', 'SQLite', '外部研究默认关闭', '授权域名']) {
    assert.match(html, new RegExp(phrase), `missing Book Wiki phrase: ${phrase}`)
  }

  assert.match(html, /书籍知识库只处理 Markdown 文件夹/)
  assert.match(html, /PPTX[^。]*下载/)
  assert.doesNotMatch(html, /Claude Code/)
  assert.doesNotMatch(html, /观察领域分布、孤岛、枢纽、尘封和新生内容/)
  assert.match(html, /href="\.\/manual\/#wiki"/)
  assert.match(html, /7 个内置 Skill/)
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

test('motion choreography progressively reveals groups and respects reduced motion', async () => {
  const [html, script, styles] = await Promise.all([
    read('index.html'),
    read('src/main.js'),
    read('src/style.css'),
  ])

  assert.match(html, /data-reveal-sequence/)
  assert.match(html, /data-reveal-group/)
  assert.match(script, /--reveal-delay/)
  assert.match(script, /requestAnimationFrame/)
  assert.match(script, /prefers-reduced-motion:\s*reduce/)
  assert.match(styles, /transition-delay:\s*var\(--reveal-delay/)
  assert.match(styles, /filter:\s*blur\(var\(--reveal-blur/)
  assert.match(styles, /@media \(prefers-reduced-motion:\s*reduce\)/)
})

test('scroll reveals reset offscreen and feature cards use a calm optical surface', async () => {
  const [script, styles] = await Promise.all([
    read('src/main.js'),
    read('src/style.css'),
  ])

  assert.match(script, /entry\.intersectionRatio\s*===\s*0/)
  assert.match(script, /dataset\.visible\s*=\s*String\(visible\)/)
  assert.doesNotMatch(script, /observer\.unobserve/)
  assert.match(styles, /\.feature-card\s*\{[^}]*var\(--glass-content-fill\)/s)
  assert.doesNotMatch(styles, /radial-gradient/)
  assert.doesNotMatch(styles, /\.feature-card::after\s*\{/)
})
