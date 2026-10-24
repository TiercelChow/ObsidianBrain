<template>
  <main class="diagram-lab">
    <header class="lab-heading"><div><p>阅境轩 · 图表阅读</p><h1>同一张图，三种呈现</h1></div><router-link to="/reader">返回书架</router-link></header>
    <div class="lab-options">
      <el-select v-model="sampleId" aria-label="图表示例" popper-class="glass-select-popper"><el-option v-for="sample in mermaidSamples" :key="sample.id" :label="sample.title" :value="sample.id" /></el-select>
      <div class="lab-themes"><button v-for="theme in themes" :key="theme.value" type="button" :aria-pressed="app.theme === theme.value" @click="app.setTheme(theme.value)">{{ theme.label }}</button></div>
    </div>
    <div class="lab-comparisons">
      <section class="lab-panel" data-engine="optimized"><header><h2>系统优化版</h2><span>推荐</span></header><p>系统配色 · 完整适宽 · 点击放大</p><div ref="optimizedRef" class="lab-diagram" v-html="optimizedHtml"></div></section>
      <section class="lab-panel" data-engine="original"><header><h2>原版</h2></header><p>默认主题与适宽显示，作为对照</p><div class="lab-diagram lab-original" :data-state="originalState" v-html="originalHtml"></div></section>
      <section class="lab-panel" data-engine="beautiful"><header><h2>beautiful-mermaid</h2></header><p>仅供比较，部分语法及公式尚不兼容</p><div class="lab-diagram lab-alternative" :data-state="alternativeState"><img v-if="alternativeUrl" :src="alternativeUrl" alt="beautiful-mermaid 对比图" /><span v-else>{{ alternativeError || '…' }}</span></div></section>
    </div>
    <details class="lab-source"><summary>查看图表源码</summary><pre>{{ selected.source }}</pre></details>
    <MermaidViewer v-if="viewerSvg" :svg-html="viewerSvg" :source="selected.source" :title="selected.title" @close="viewerSvg = ''" />
  </main>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { getMermaidPalette } from '@/markdown/mermaidTheme'
import { readDiagramTokens, renderMermaidSvg, type DiagramTheme } from '@/markdown/mermaidRuntime'
import { mermaidSamples } from '@/dev/mermaidSamples'
import MermaidViewer from '@/components/reader/MermaidViewer.vue'

const app = useAppStore(), route = useRoute(), router = useRouter()
const sampleId = computed({ get: () => String(route.query.sample || 'overview'), set: sample => { void router.replace({ query: { sample } }) } })
const selected = computed(() => mermaidSamples.find(sample => sample.id === sampleId.value) || mermaidSamples[0])
const themes: { label: string; value: DiagramTheme }[] = [{ label: '浅色', value: 'light' }, { label: '深色', value: 'dark' }, { label: '护眼', value: 'eye-care' }]
const optimizedRef = ref<HTMLElement | null>(null), optimizedHtml = ref(''), originalHtml = ref(''), viewerSvg = ref('')
const alternativeUrl = ref(''), alternativeError = ref('')
const originalState = ref('loading'), alternativeState = ref('loading')
let generation = 0
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(svg => { viewerSvg.value = svg.outerHTML })

watch([selected, () => app.theme], async () => {
  const version = ++generation, source = selected.value.source, theme = app.theme
  const isCurrent = () => version === generation
  cleanup()
  originalHtml.value = ''
  originalState.value = alternativeState.value = 'loading'
  if (alternativeUrl.value) URL.revokeObjectURL(alternativeUrl.value)
  alternativeUrl.value = alternativeError.value = ''
  optimizedHtml.value = await renderMarkdown(`\`\`\`mermaid\n${source}\n\`\`\``, undefined, `mermaid-lab-${sampleId.value}`)
  if (!isCurrent()) return
  await nextTick()
  if (optimizedRef.value) await enhance(optimizedRef.value)
  try {
    const svg = await renderMermaidSvg(source, { theme, profile: 'original', isCurrent })
    if (svg && isCurrent()) { originalHtml.value = svg; originalState.value = 'ready' }
  } catch { if (isCurrent()) { originalHtml.value = '原版也无法解析此示例'; originalState.value = 'error' } }
  if (!isCurrent()) return
  try {
    const { renderMermaidSVG } = await import('beautiful-mermaid')
    if (!isCurrent()) return
    const colors = getMermaidPalette(readDiagramTokens())
    const svg = renderMermaidSVG(source, { bg: colors.background, fg: colors.text, accent: colors.accent, font: readDiagramTokens().fontFamily })
    alternativeUrl.value = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }))
    alternativeState.value = 'ready'
  } catch { if (isCurrent()) { alternativeError.value = '该渲染器暂不支持此示例'; alternativeState.value = 'error' } }
}, { immediate: true, flush: 'post' })
onBeforeUnmount(() => { generation += 1; if (alternativeUrl.value) URL.revokeObjectURL(alternativeUrl.value) })
</script>

<style scoped>
.diagram-lab { max-width: 1600px; padding: 20px var(--page-gutter) 120px; margin-inline: auto; color: var(--text-primary); }
.lab-heading { display: flex; justify-content: space-between; align-items: center; gap: 20px; margin-bottom: 28px; }
.lab-heading p { margin: 0 0 7px; font-size: 12px; color: var(--text-muted); }
.lab-heading h1 { margin: 0; font-size: 25px; font-weight: 700; letter-spacing: var(--tracking-tight); }
.lab-heading a { color: var(--text-muted); font-size: 13px; text-decoration: none; white-space: nowrap; }
.lab-options { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 16px; margin-bottom: 20px; }
.lab-options .el-select { width: 230px; }
.lab-themes { display: flex; gap: 4px; }
.lab-themes button { min-height: 40px; padding: 0 14px; background: transparent; border: 0; border-radius: 999px; font: 13px var(--font-sans); color: var(--text-muted); cursor: pointer; }
.lab-themes button[aria-pressed="true"] { background: var(--bg-hover); color: var(--text-primary); }
.lab-comparisons { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.lab-panel[data-engine="optimized"] { grid-column: 1 / -1; }
.lab-panel { min-width: 0; padding: 20px; border: 1px solid var(--border-faint); border-radius: var(--radius-content); background: color-mix(in srgb, var(--bg-base) 88%, var(--glass-solid)); }
.lab-panel > header { display: flex; align-items: center; gap: 10px; }
.lab-panel h2 { margin: 0; font-size: 16px; font-weight: 650; }
.lab-panel > header > span { font-size: 10px; padding: 3px 7px; border-radius: 999px; color: var(--accent); background: var(--accent-light); }
.lab-panel > p { font-size: 12px; color: var(--text-muted); margin: 8px 0 16px; }
.lab-diagram { min-height: 240px; min-width: 0; max-height: 540px; overflow: auto; }
.lab-panel[data-engine="optimized"] .lab-diagram { max-height: none; }
.lab-diagram :deep(.mermaid) { margin: 0; min-height: 240px; border: 0; background: transparent; }
.lab-original { display: grid; place-items: center; }
.lab-original :deep(svg) { max-width: 100%; height: auto; }
.lab-alternative { display: grid; place-items: center; color: var(--text-muted); font-size: 13px; }
.lab-alternative img { max-width: 100%; max-height: 500px; }
.lab-source { margin-top: 24px; font-size: 13px; color: var(--text-muted); }
.lab-source summary { cursor: pointer; }
.lab-source pre { padding: 20px; overflow: auto; border-radius: 16px; background: var(--code-bg); font: 12px/1.7 var(--font-mono); }
@media (max-width: 1000px) { .lab-comparisons { grid-template-columns: 1fr; } }
@media (max-width: 768px) { .diagram-lab { padding: 12px var(--page-gutter) 140px; } .lab-heading h1 { font-size: 21px; } .lab-heading { gap: 12px; } .lab-options { gap: 8px; } .lab-options .el-select { width: 100%; } .lab-panel { padding: 16px; } }
</style>
