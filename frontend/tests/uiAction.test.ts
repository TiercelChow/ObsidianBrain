import assert from 'node:assert/strict'
import test from 'node:test'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { readFile } from 'node:fs/promises'
import { Edit } from '@element-plus/icons-vue'
import UiAction from '../src/components/motion/UiAction.ts'

test('icon actions render a named native button, including caller attributes', async () => {
  const html = await renderToString(createSSRApp({ render: () => h(UiAction, {icon:Edit, label:'编辑任务', class:'custom-action'}) }))
  assert.match(html, /<button[^>]+type="button"/)
  assert.match(html, /aria-label="编辑任务"/)
  assert.match(html, /title="编辑任务"/)
  assert.match(html, /custom-action/)
  assert.match(html, /ui-action--quiet/)
  assert.doesNotMatch(html, /ui-icon-action|data-glass-action/)
  assert.match(html, /<svg[^>]+aria-hidden="true"/)
  assert.doesNotMatch(html, /disabled|aria-busy/)
})

test('busy actions remain identifiable and are disabled rather than disappearing', async () => {
  const html = await renderToString(createSSRApp({ render: () => h(UiAction, {icon:Edit, label:'编辑任务', loading:true}) }))
  assert.match(html, /aria-label="编辑任务"/)
  assert.match(html, /disabled/)
  assert.match(html, /aria-busy="true"/)
  assert.match(html, /ui-action__glyph is-loading/)
})

test('dangerous icon actions remain borderless and identifiable without losing their semantic tone', async () => {
  const html = await renderToString(createSSRApp({ render: () => h(UiAction, { icon: Edit, label: '删除记录', danger: true }) }))
  assert.match(html, /ui-action--quiet is-danger/)
  assert.match(html, /aria-label="删除记录"/)
  assert.doesNotMatch(html, /data-glass-action/)
  const css = await readFile(new URL('../src/styles/motion.css', import.meta.url), 'utf8')
  assert.match(css, /\.ui-action--quiet\.is-danger:not\(:disabled\):active\s*\{[^}]*color:\s*var\(--glass-danger-label\)/)
})

test('quiet actions keep semantics and loading but opt out of raised glass paint', async () => {
  const html = await renderToString(createSSRApp({ render: () => h(UiAction, { icon: Edit, label: '查看本轮运行', appearance: 'quiet', loading: true }) }))
  assert.match(html, /ui-action--quiet/)
  assert.match(html, /aria-label="查看本轮运行"/)
  assert.match(html, /title="查看本轮运行"/)
  assert.match(html, /disabled/)
  assert.match(html, /aria-busy="true"/)
  assert.doesNotMatch(html, /ui-icon-action|data-glass-action|appearance=/)
})

test('quiet actions retain a 44px target, borderless resting state and accessible interaction feedback', async () => {
  const css = await readFile(new URL('../src/styles/motion.css', import.meta.url), 'utf8')
  assert.match(css, /\.ui-icon-action,\s*\.ui-action\s*\{[^}]*min-width:\s*44px;[^}]*min-height:\s*44px/)
  assert.match(css, /\.ui-action--quiet\s*\{[^}]*border:\s*0;[^}]*border-radius:\s*50%;[^}]*background:\s*transparent;[^}]*box-shadow:\s*none/)
  assert.match(css, /\.ui-action--quiet:not\(:disabled\):hover/)
  assert.match(css, /\.ui-action--quiet:not\(:disabled\):active/)
  assert.match(css, /\.ui-action--quiet:focus-visible/)
  assert.match(css, /\.ui-action--quiet:disabled/)
})

test('disabled and busy actions never emit a click', () => {
  for (const appearance of ['glass', 'quiet'] as const) {
    for (const state of [{disabled:true,loading:false},{disabled:false,loading:true},{disabled:false,loading:false}]) {
      let clicks = 0
      const render = UiAction.setup!({icon:Edit,label:'编辑',appearance,danger:false,...state}, {emit:() => {clicks++}} as any) as () => any
      render().props.onClick({})
      assert.equal(clicks, state.disabled || state.loading ? 0 : 1)
    }
  }
})
