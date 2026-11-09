<template>
  <div v-if="error" role="alert">{{ error }}</div>
  <article ref="root" class="markdown-body bookmark-markdown" style="padding:0;max-width:none" v-html="html"></article>
  <MermaidViewer v-if="svg" :svg-html="svg" :source="source" @close="svg = ''" />
</template>
<script setup lang="ts">
import { defineAsyncComponent, nextTick, ref, watch } from 'vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
const props = defineProps<{content:string}>()
const root = ref<HTMLElement|null>(null), html = ref(''), error = ref(''), svg = ref(''), source = ref('')
const MermaidViewer = defineAsyncComponent(() => import('./MermaidViewer.vue'))
const {renderMarkdown,enhance,cleanup} = useMarkdownRender((element, raw) => {svg.value=element.outerHTML;source.value=raw})
let generation=0
watch(() => props.content, async content => {
  const current=++generation
  error.value=''
  cleanup()
  try {
    const rendered=await renderMarkdown(content,undefined,'bookmark-note')
    if (current!==generation) return
    html.value=rendered
    await nextTick()
    if (current===generation && root.value) await enhance(root.value)
  } catch (e) {if (current===generation && (e as Error).name!=='AbortError') error.value='笔记预览失败，原文仍可编辑'}
},{immediate:true})
</script>
<style scoped>
.bookmark-markdown { font-size: 15px; line-height: 1.8; overflow-wrap: anywhere; min-width: 0; }
.bookmark-markdown :deep(> :first-child) { margin-top: 0; }
.bookmark-markdown :deep(h1) { position: static !important; font-size: 1.5em; }
.bookmark-markdown :deep(img) { max-width: 100%; }
</style>
