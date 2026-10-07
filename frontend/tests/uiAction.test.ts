import assert from 'node:assert/strict'
import test from 'node:test'
import { createSSRApp, h } from 'vue'
import { renderToString } from '@vue/server-renderer'
import { Edit } from '@element-plus/icons-vue'
import UiAction from '../src/components/motion/UiAction.ts'

test('icon actions render a named native button, including caller attributes', async () => {
  const html = await renderToString(createSSRApp({ render: () => h(UiAction, {icon:Edit, label:'编辑任务', class:'custom-action'}) }))
  assert.match(html, /<button[^>]+type="button"/)
  assert.match(html, /aria-label="编辑任务"/)
  assert.match(html, /title="编辑任务"/)
  assert.match(html, /custom-action/)
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

test('disabled and busy actions never emit a click', () => {
  for (const state of [{disabled:true,loading:false},{disabled:false,loading:true},{disabled:false,loading:false}]) {
    let clicks = 0
    const render = UiAction.setup!({icon:Edit,label:'编辑',danger:false,...state}, {emit:() => {clicks++}} as any) as () => any
    render().props.onClick({})
    assert.equal(clicks, state.disabled || state.loading ? 0 : 1)
  }
})
