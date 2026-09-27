import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const root = new URL('../', import.meta.url)
const read = (path) => readFile(new URL(path, root), 'utf8')

const chapters = ['overview', 'sources', 'compile', 'review', 'workspace', 'qa', 'budgets', 'tasks', 'config', 'security', 'limits']

function section(html, id) {
  const match = html.match(new RegExp(`<section id="${id}"[^>]*>([\\s\\S]*?)</section>`))
  assert.ok(match, `missing section #${id}`)
  return match[1]
}

test('llm wiki page contains every architecture chapter with directory links', async () => {
  const html = await read('llm-wiki/index.html')
  for (const id of chapters) {
    assert.match(html, new RegExp(`id="${id}"`), `missing chapter #${id}`)
    assert.match(html, new RegExp(`href="#${id}"`), `missing directory link to #${id}`)
  }
})

test('llm wiki page documents the four knowledge pages as one pipeline', async () => {
  const html = await read('llm-wiki/index.html')
  for (const page of ['书籍知识库', 'Wiki 工作台', '知识问答', '研究任务']) {
    assert.match(html, new RegExp(page), `missing knowledge page: ${page}`)
  }
  for (const phrase of ['proposal', '审核', '单事务', '租约']) {
    assert.match(html, new RegExp(phrase), `missing pipeline concept: ${phrase}`)
  }
})

test('llm wiki page cites verified backend mechanisms and storage contracts', async () => {
  const html = await read('llm-wiki/index.html')
  for (const phrase of [
    'source_versions', 'source_spans', 'knowledge_compile_checkpoints', 'compile_fingerprint',
    'knowledge_change_sets', 'knowledge_entries', 'knowledge_claims', 'knowledge_relations',
    'knowledge_citations', 'agent_runs', 'agent_run_capabilities', 'agent_run_inspections',
    'knowledge_tasks', 'FTS5', 'cjk_terms', 'ACP', 'SSE', 'MCP', 'PPTX', 'OOXML',
    'knowledge_propose_changes', 'book_fetch_external', 'BEGIN IMMEDIATE',
  ]) {
    assert.match(html, new RegExp(phrase), `missing mechanism fact: ${phrase}`)
  }
  assert.match(html, /驳回[^。]*检查点/)
  assert.match(html, /外部研究[^。]*默认关闭/)
  assert.match(html, /环境变量名/)
  assert.match(html, /自动应答[^。]*Cancelled|Cancelled[^。]*自动应答/)
  assert.doesNotMatch(html, /Claude Code/)
})

test('llm wiki page renders flows and state machines as accessible diagrams', async () => {
  const html = await read('llm-wiki/index.html')
  const figures = html.match(/<figure class="diagram"/g) ?? []
  assert.ok(figures.length >= 8, `expected >= 8 diagrams, got ${figures.length}`)
  const captions = html.match(/<figcaption>/g) ?? []
  assert.ok(captions.length >= figures.length, 'every diagram needs a figcaption')
  const roles = html.match(/role="img"/g) ?? []
  assert.ok(roles.length >= figures.length, 'every diagram svg needs role="img" with aria-label')
  assert.match(html, /<marker id="dg-arrow"/)
})

test('llm wiki page explains how each stage works with real data shapes', async () => {
  const html = await read('llm-wiki/index.html')
  const hows = html.match(/<h3 class="how">/g) ?? []
  assert.ok(hows.length >= 6, `expected >= 6 how-sections, got ${hows.length}`)
  const whys = html.match(/class="callout why"/g) ?? []
  assert.ok(whys.length >= 6, `expected >= 6 why-callouts, got ${whys.length}`)
  for (const shape of [
    '&lt;output_schema&gt;', '&lt;source_spans&gt;', '&lt;existing_wiki&gt;', '&lt;evidence&gt;',
    'no_material_reason', '_classification', 'claim_text', 'to_slug', 'relation_type',
    'run_started', 'text_delta', 'bm25', 'repair_context', 'schema_version', 'core_message',
  ]) {
    assert.match(html, new RegExp(shape.replace(/[&<>]/g, (c) => `\\${c}`)), `missing real data shape: ${shape}`)
  }
})

test('llm wiki page is a standalone Pages entry reachable from every page nav', async () => {
  const [home, manual, page, config] = await Promise.all([
    read('index.html'), read('manual/index.html'), read('llm-wiki/index.html'), read('vite.config.js'),
  ])
  assert.match(home, /href="\.\/llm-wiki\/"/)
  assert.match(manual, /href="\.\.\/llm-wiki\/"/)
  assert.match(page, /href="\.\.\/"[^>]*aria-label="ObsidianBrain 首页"|aria-label="ObsidianBrain 首页"[^>]*href="\.\.\/"/)
  assert.match(page, /href="\.\.\/manual\/"/)
  assert.match(page, /aria-current="page"/)
  assert.match(config, /llm-wiki\/index\.html/)
})

test('llm wiki page uses repository-relative paths and never calls the local API', async () => {
  const html = await read('llm-wiki/index.html')
  assert.doesNotMatch(html, /(?:src|href)=["']\//)
  assert.doesNotMatch(html, /(?:fetch|axios)\s*\(/)
  assert.doesNotMatch(html, /\/v1\//)
})

test('llm wiki page has valid same-page anchors and unique ids', async () => {
  const html = await read('llm-wiki/index.html')
  const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1])
  assert.equal(new Set(ids).size, ids.length, 'duplicate ids')
  for (const [, id] of html.matchAll(/\bhref="#([^"]+)"/g)) {
    assert.ok(ids.includes(id), `missing anchor target #${id}`)
  }
})

test('llm wiki page reuses the shared docs shell scripts and styles', async () => {
  const html = await read('llm-wiki/index.html')
  assert.match(html, /src="\.\.\/src\/main\.js"/)
  assert.match(html, /src="\.\.\/src\/llm-wiki\.js"/)
  assert.match(html, /class="manual-page/)
  assert.match(html, /data-chapter-link/)
  assert.match(html, /data-theme-toggle/)
})

test('architecture documents adaptive QA and clean conversation context instead of legacy top-k', async () => {
  const html = await read('llm-wiki/index.html')
  const qa = section(html, 'qa')
  for (const phrase of ['意图记忆', '子问题', 'token', '目录', '独立问题', 'FTS', '已读', '继续完成完整答案']) {
    assert.ok(qa.includes(phrase), `missing adaptive QA concept: ${phrase}`)
  }
  assert.doesNotMatch(html, /FTS 预召回 ≤8 条|预召回至多 8 条|上一轮问题 \+ 本轮问题的拼接|至多 20 条已有主题|机械限长（句末截断）/)
  const budgets = section(html, 'budgets')
  for (const tool of ['knowledge_get_run_budget', 'knowledge_request_budget_extension', 'knowledge_report_evidence_coverage']) {
    assert.ok(budgets.includes(tool), `missing native budget tool: ${tool}`)
  }
  for (const phrase of ['软预算', '硬上限', '新增实际已读证据', '不扩大权限', '应用护栏']) {
    assert.ok(budgets.includes(phrase), `missing budget boundary: ${phrase}`)
  }
})

test('architecture explains full-body compilation and current-source review with honest states', async () => {
  const html = await read('llm-wiki/index.html')
  const compile = section(html, 'compile')
  for (const phrase of ['semantic-contract-v5-knowledge-body', 'content_md', '完整正文', '原子论断', '归并', 'knowledge_compile_reports', 'no_material_reason']) {
    assert.ok(compile.includes(phrase), `missing current compile contract: ${phrase}`)
  }
  assert.doesNotMatch(html, /模型不输出 content_md|正文由服务端生成而非模型撰写|模型不输出.*正文|超限句末截断并标注/)
  for (const phrase of ['Markdown AST', 'source_span_structures', 'knowledge_source_impacts', '命中附近', 'retry_knowledge_source_review', 'propose_knowledge_entry_archive']) {
    assert.ok(html.includes(phrase), `missing structured source/review workflow: ${phrase}`)
  }
  assert.match(html, /无实质(?:新增|变化|结果)[^。]*不能清除过期/)
})

test('architecture separates retrieved candidates, frozen evidence and factual validation', async () => {
  const html = await read('llm-wiki/index.html')
  for (const phrase of ['agent_run_evidence', '冻结', '实际读取范围', '同一对象同一版本', '当前版本', '不等于逐句事实核验']) {
    assert.ok(html.includes(phrase), `missing evidence boundary: ${phrase}`)
  }
  assert.match(html, /候选[^。]*不是[^。]*引用证据/)
  const limits = section(html, 'limits')
  for (const phrase of ['向量', '全局语义候选重排', '会话对象记忆', '真实模型', '未实现']) {
    assert.ok(limits.includes(phrase), `missing truthful capability limit: ${phrase}`)
  }
})

test('architecture documents durable research, true baselines, synthesis and whole-report PPT materialization', async () => {
  const html = await read('llm-wiki/index.html')
  const tasks = section(html, 'tasks')
  for (const phrase of ['plan', 'section', 'synthesis', 'report', 'validation', 'presentation', 'content_run_id', 'research_execution_epoch', 'knowledge_get_research_baseline', 'knowledge_get_research_section', '完整报告', '选材范围']) {
    assert.ok(tasks.includes(phrase), `missing durable research concept: ${phrase}`)
  }
  assert.match(tasks, /只(?:恢复|重做)[^。]*未完成/)
  assert.match(tasks, /不是[^。]*前[^。]*截取/)
  assert.match(tasks, /找不到基线[^。]*partial\/missing/)
})

test('architecture distinguishes native Skill availability and model capacity from billing usage', async () => {
  const html = await read('llm-wiki/index.html')
  const config = section(html, 'config')
  for (const phrase of ['harness_native_available', 'prompt_injected', 'context_window', 'max_output_tokens', 'reasoning_policy', '按需读取', '计费', '压缩']) {
    assert.ok(config.includes(phrase), `missing runtime/config boundary: ${phrase}`)
  }
  assert.doesNotMatch(html, /Harness 只是文本进出|拿到全部磁盘文件也拿不到密钥|零供应链依赖/)
})

test('complete compilation example follows the actual required data shape', async () => {
  const [html, schemaText] = await Promise.all([
    read('llm-wiki/index.html'), read('../backend/prompts/wiki/semantic-output.schema.json'),
  ])
  const example = html.match(/<code id="compile-example">([\s\S]*?)<\/code>/)
  assert.ok(example, 'missing complete JSON example')
  const data = JSON.parse(example[1])
  const schema = JSON.parse(schemaText)
  assert.deepEqual(Object.keys(data), ['entries'])
  assert.ok(data.entries.length > 0 && data.entries.length <= schema.properties.entries.maxItems)
  for (const entry of data.entries) {
    assert.deepEqual(Object.keys(entry).sort(), [...schema.definitions.entry.required].sort())
    assert.ok(schema.definitions.entry.properties.entry_type.enum.includes(entry.entry_type))
    assert.ok(schema.definitions.entry.properties._classification.enum.includes(entry._classification))
    assert.ok(entry.content_md.includes('\n'), 'Markdown newlines must be legal JSON escapes')
    assert.ok(entry.content_md.length <= schema.definitions.entry.properties.content_md.maxLength)
    for (const claim of entry.claims) {
      assert.deepEqual(Object.keys(claim).sort(), [...schema.definitions.claim.required].sort())
      assert.ok(claim.citations.every(id => entry.citations.includes(id)))
    }
  }
})
