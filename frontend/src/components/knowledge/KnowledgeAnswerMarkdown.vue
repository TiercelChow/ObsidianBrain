<template>
  <div
    ref="rootRef"
    class="knowledge-answer-markdown markdown-body"
    :class="{ 'is-streaming': streaming }"
    v-html="html"
    @click="handleClick"
  ></div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { renderMarkdownDocument } from '@/markdown/renderMarkdown'

const props = defineProps<{
  content: string
  evidenceCount: number
  streaming?: boolean
}>()

const emit = defineEmits<{
  citation: [sourceIndex: number]
}>()

const rootRef = ref<HTMLElement | null>(null)
const html = ref('')
let renderTimer: number | undefined

function render() {
  window.clearTimeout(renderTimer)
  renderTimer = undefined
  html.value = renderMarkdownDocument(props.content || '').html
  void nextTick(decorateCitations)
}

function scheduleRender() {
  if (!props.streaming) {
    render()
    return
  }
  if (renderTimer !== undefined) return
  renderTimer = window.setTimeout(render, 72)
}

function decorateCitations() {
  const root = rootRef.value
  if (!root || !props.evidenceCount) return
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const textNodes: Text[] = []
  let node: Node | null
  while ((node = walker.nextNode())) {
    const parent = node.parentElement
    if (!parent?.closest('a, button, code, pre, .katex, .mermaid')) textNodes.push(node as Text)
  }
  const pattern = /\[S(\d+)\]/g
  for (const textNode of textNodes) {
    const value = textNode.data
    pattern.lastIndex = 0
    if (!pattern.test(value)) continue
    pattern.lastIndex = 0
    const fragment = document.createDocumentFragment()
    let cursor = 0
    for (const match of value.matchAll(pattern)) {
      const position = match.index ?? 0
      const sourceIndex = Number(match[1]) - 1
      fragment.append(value.slice(cursor, position))
      if (sourceIndex >= 0 && sourceIndex < props.evidenceCount) {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'answer-citation'
        button.dataset.sourceIndex = String(sourceIndex)
        button.setAttribute('aria-label', `预览来源 ${sourceIndex + 1}`)
        button.textContent = match[0]
        fragment.append(button)
      } else {
        fragment.append(match[0])
      }
      cursor = position + match[0].length
    }
    fragment.append(value.slice(cursor))
    textNode.replaceWith(fragment)
  }
}

function handleClick(event: MouseEvent) {
  const target = (event.target as HTMLElement).closest<HTMLElement>('[data-source-index]')
  if (!target) return
  const sourceIndex = Number(target.dataset.sourceIndex)
  if (Number.isInteger(sourceIndex)) emit('citation', sourceIndex)
}

watch(() => [props.content, props.streaming, props.evidenceCount], scheduleRender, { immediate: true })
onBeforeUnmount(() => window.clearTimeout(renderTimer))
</script>

<style scoped>
.knowledge-answer-markdown {
  min-width: 0;
  padding: 12px 15px;
  border-radius: 16px 16px 16px 5px;
  background: var(--bg-glass-subtle);
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.72;
  overflow-wrap: anywhere;
}
.knowledge-answer-markdown.is-streaming::after {
  width: 2px;
  height: 1em;
  display: inline-block;
  margin-left: 3px;
  border-radius: 99px;
  background: var(--accent);
  content: '';
  vertical-align: -.12em;
  animation: answer-caret 760ms step-end infinite;
}
.knowledge-answer-markdown :deep(> :first-child) { margin-top: 0; }
.knowledge-answer-markdown :deep(> :last-child) { margin-bottom: 0; }
.knowledge-answer-markdown :deep(p),
.knowledge-answer-markdown :deep(ul),
.knowledge-answer-markdown :deep(ol),
.knowledge-answer-markdown :deep(blockquote),
.knowledge-answer-markdown :deep(pre),
.knowledge-answer-markdown :deep(.table-scroll) { margin: .62em 0; }
.knowledge-answer-markdown :deep(h1),
.knowledge-answer-markdown :deep(h2),
.knowledge-answer-markdown :deep(h3),
.knowledge-answer-markdown :deep(h4) { margin: 1em 0 .45em; color: var(--text-primary); line-height: 1.3; }
.knowledge-answer-markdown :deep(h1) { font-size: 1.45em; }
.knowledge-answer-markdown :deep(h2) { font-size: 1.28em; }
.knowledge-answer-markdown :deep(h3) { font-size: 1.14em; }
.knowledge-answer-markdown :deep(ul), .knowledge-answer-markdown :deep(ol) { padding-left: 1.45em; }
.knowledge-answer-markdown :deep(li + li) { margin-top: .28em; }
.knowledge-answer-markdown :deep(blockquote) { padding: .35em .8em; border-left: 3px solid var(--accent-border); color: var(--text-muted); }
.knowledge-answer-markdown :deep(code) { padding: .14em .38em; border-radius: 6px; background: color-mix(in srgb, var(--text-primary) 7%, transparent); font-family: var(--font-mono); font-size: .9em; }
.knowledge-answer-markdown :deep(pre) { max-width: 100%; overflow-x: auto; padding: 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: color-mix(in srgb, var(--bg-base) 72%, transparent); }
.knowledge-answer-markdown :deep(pre code) { padding: 0; background: transparent; }
.knowledge-answer-markdown :deep(.table-scroll),
.knowledge-answer-markdown :deep(.katex-display) { max-width: 100%; overflow-x: auto; }
.knowledge-answer-markdown :deep(table) { min-width: max-content; border-collapse: collapse; }
.knowledge-answer-markdown :deep(th), .knowledge-answer-markdown :deep(td) { padding: 7px 10px; border: 1px solid var(--border-faint); }
.knowledge-answer-markdown :deep(a) { color: var(--accent); text-decoration-thickness: 1px; text-underline-offset: 2px; }
.knowledge-answer-markdown :deep(.answer-citation) { display: inline-flex; align-items: center; margin: 0 2px; padding: 1px 5px; border: 0; border-radius: 6px; background: var(--accent-light); color: var(--accent); font: inherit; font-size: 11px; font-weight: 720; line-height: 1.5; vertical-align: .08em; cursor: pointer; }
.knowledge-answer-markdown :deep(.answer-citation:hover) { box-shadow: inset 0 0 0 1px var(--accent-border); }
@keyframes answer-caret { 50% { opacity: 0; } }
@media (prefers-reduced-motion: reduce) {
  .knowledge-answer-markdown.is-streaming::after { animation: none; }
}
</style>
