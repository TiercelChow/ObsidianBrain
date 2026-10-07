import assert from 'node:assert/strict'
import test from 'node:test'
import { createRenderer, defineComponent, h, nextTick, ref } from 'vue'
import MotionContent from '../src/components/motion/MotionContent.ts'

interface HostNode { text: string; children: HostNode[]; parent: HostNode | null }
const node = (text = ''): HostNode => ({ text, children: [], parent: null })

function hostRenderer() {
  return createRenderer<HostNode, HostNode>({
    createElement: () => node(), createText: node, createComment: () => node(),
    setText: (el, text) => { el.text = text },
    setElementText: (el, text) => { el.text = text; el.children = [] },
    patchProp: () => {}, parentNode: el => el.parent,
    nextSibling: el => {
      const siblings = el.parent?.children ?? []
      return siblings[siblings.indexOf(el) + 1] ?? null
    },
    insert: (el, parent, anchor = null) => {
      if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1)
      el.parent = parent
      const index = anchor ? parent.children.indexOf(anchor) : -1
      parent.children.splice(index < 0 ? parent.children.length : index, 0, el)
    },
    remove: el => {
      if (el.parent) el.parent.children.splice(el.parent.children.indexOf(el), 1)
      el.parent = null
    },
  })
}
const text = (el: HostNode): string => el.text + el.children.map(text).join('')

test('closing contents survive cleared parent selection and refresh on reopen', async () => {
  const renderer = hostRenderer()
  const open = ref(true), selection = ref('原有内容')
  const root = node()
  const app = renderer.createApp(defineComponent({
    setup: () => () => h(MotionContent, {live:open.value}, {default:() => h('section', selection.value)}),
  }))
  app.mount(root)
  try {
    assert.equal(text(root), '原有内容')
    open.value = false; selection.value = ''
    await nextTick()
    assert.equal(text(root), '原有内容', 'exit must not shrink to an empty panel')
    open.value = true; selection.value = '新的内容'
    await nextTick()
    assert.equal(text(root), '新的内容', 'reopened sheet must not show a stale selection')
  } finally { app.unmount() }
  assert.equal(root.children.length, 0)
})

test('nested content switches freeze while the enclosing sheet exits', async () => {
  const renderer = hostRenderer()
  const open = ref(true), selection = ref('子任务详情')
  const Nested = defineComponent({
    setup: () => () => h(MotionContent, { live: true }, { default: () => h('section', selection.value) }),
  })
  const root = node()
  const app = renderer.createApp(defineComponent({
    setup: () => () => h(MotionContent, { live: open.value }, { default: () => h(Nested) }),
  }))
  app.mount(root)
  try {
    assert.equal(text(root), '子任务详情')
    open.value = false; selection.value = ''
    await nextTick()
    assert.equal(text(root), '子任务详情', 'nested slots must not erase a closing sheet')
    open.value = true; selection.value = '另一个子任务'
    await nextTick()
    assert.equal(text(root), '另一个子任务')
  } finally { app.unmount() }
})
