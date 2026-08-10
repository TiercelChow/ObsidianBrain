<template>
  <div class="ppm-overlay" @click.self="close">
    <div class="ppm-panel">
      <header class="ppm-head">
        <el-icon class="ppm-head-icon"><Document /></el-icon>
        <span class="ppm-path" :title="currentPath">{{ currentPath }}</span>
        <button
          v-if="canOpenInReader"
          class="ppm-reader-btn"
          title="在阅读器中打开"
          @click="emit('open-in-reader', currentPath)"
        >
          <el-icon><FullScreen /></el-icon><span class="rbtn-label">阅读器</span>
        </button>
        <button class="ppm-close-btn" title="关闭" @click="close">
          <el-icon><Close /></el-icon>
        </button>
      </header>

      <div ref="bodyRef" class="ppm-body">
        <div v-if="loading" class="ppm-state">
          <el-icon class="is-loading"><Loading /></el-icon><span>加载中…</span>
        </div>
        <div v-else-if="error" class="ppm-state error">⚠️ {{ error }}</div>
        <div v-else-if="kind === 'notfound'" class="ppm-state">路径不存在</div>
        <div v-else-if="kind === 'folder'" class="ppm-folder">
          <FileTree :entries="entries" :active-path="''" @select="onTreeSelect" />
        </div>
        <article v-else-if="kind === 'md'" ref="mdRef" class="markdown-body ppm-md" v-html="mdHtml"></article>
        <div v-else-if="kind === 'code'" class="code-block ppm-code-block">
          <pre class="code-gutter">{{ codeGutter }}</pre>
          <pre class="code-content"><code ref="codeRef" :class="codeClass">{{ codeContent }}</code></pre>
        </div>
        <div v-else-if="kind === 'image'" class="ppm-state">🖼️ 暂不支持图片预览</div>
        <div v-else-if="kind === 'pdf'" class="ppm-state">
          <el-icon><Document /></el-icon><span>PDF 预览请点击右上「阅读器」按钮打开</span>
        </div>
        <div v-else class="ppm-state">该文件类型暂不支持预览</div>
      </div>
    </div>

    <MermaidViewer
      v-if="mermaidSvg"
      :svg-html="mermaidSvg"
      :source="mermaidSource"
      @close="mermaidSvg = ''"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onMounted, computed } from 'vue'
import { Document, Close, Loading, FullScreen } from '@element-plus/icons-vue'
import hljs from 'highlight.js'
import {
  statLocalPath, listLocalDir, readLocalFile,
  type DirEntry, type PathStat,
} from '@/api/reader'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import FileTree from './FileTree.vue'
import MermaidViewer from './MermaidViewer.vue'

const props = defineProps<{ path: string; root: string; anchor?: string }>()
const emit = defineEmits<{
  close: []
  'open-in-reader': [path: string, anchor?: string]
}>()

const currentPath = ref(props.path)
// Anchor (line/symbol/heading) to scroll to after the content renders.
const pendingAnchor = ref(props.anchor || '')
const bodyRef = ref<HTMLElement | null>(null)
const loading = ref(false)
const error = ref('')
const kind = ref('')
const entries = ref<DirEntry[]>([])
const mdHtml = ref('')
const codeContent = ref('')
const codeClass = ref('hljs')
const codeGutter = computed(() =>
  codeContent.value
    ? codeContent.value.split('\n').map((_, i) => i + 1).join('\n')
    : '',
)
const mdRef = ref<HTMLElement | null>(null)
const codeRef = ref<HTMLElement | null>(null)
const mermaidSvg = ref('')
const mermaidSource = ref('')

const canOpenInReader = computed(
  () => kind.value === 'pdf' || (kind.value === 'md' && isUnderRoot(currentPath.value)),
)

function isUnderRoot(p: string): boolean {
  const root = props.root
  if (!root) return false
  const r = root.endsWith('/') ? root : root + '/'
  return p === root || p.startsWith(r)
}

function resolveRelative(baseDir: string, rel: string): string {
  const parts = baseDir ? baseDir.split('/') : []
  for (const seg of rel.replace(/^\.\//, '').split('/')) {
    if (seg === '' || seg === '.') continue
    if (seg === '..') parts.pop()
    else parts.push(seg)
  }
  return parts.join('/')
}

function onMermaidClick(svg: SVGElement, source: string) {
  mermaidSource.value = source
  mermaidSvg.value = svg.outerHTML
}

function onLinkClick(href: string) {
  const baseDir = currentPath.value.substring(0, currentPath.value.lastIndexOf('/'))
  const [pathPartRaw, anchor] = href.split('#')
  const resolved = resolveRelative(baseDir, decodeURIComponent(pathPartRaw))
  if (!resolved) return
  pendingAnchor.value = anchor ? decodeURIComponent(anchor) : ''
  if (/\.(md|markdown|pdf)$/i.test(resolved) && isUnderRoot(resolved)) {
    emit('open-in-reader', resolved, pendingAnchor.value)
  } else {
    currentPath.value = resolved
  }
}

const { renderMarkdown, enhance } = useMarkdownRender(onMermaidClick, onLinkClick)

function classify(s: PathStat): string {
  if (!s.exists) return 'notfound'
  if (s.is_dir) return 'folder'
  const ext = s.ext.toLowerCase()
  if (ext === 'md' || ext === 'markdown') return 'md'
  if (ext === 'pdf') return 'pdf'
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico'].includes(ext)) return 'image'
  return 'code'
}

async function load() {
  loading.value = true
  error.value = ''
  kind.value = ''
  mdHtml.value = ''
  codeContent.value = ''
  entries.value = []
  try {
    const sres = await statLocalPath(currentPath.value)
    if (sres.status !== 'success' || !sres.result) {
      error.value = sres.error?.message || '查询失败'
      return
    }
    const s = sres.result
    kind.value = classify(s)

    if (kind.value === 'folder') {
      const lres = await listLocalDir(currentPath.value, 2)
      if (lres.status === 'success' && lres.result) entries.value = lres.result.entries
    } else if (kind.value === 'md') {
      const rres = await readLocalFile(currentPath.value)
      if (rres.status === 'success' && rres.result) {
        mdHtml.value = renderMarkdown(rres.result.content)
      } else {
        error.value = rres.error?.message || '读取失败'
      }
    } else if (kind.value === 'code') {
      const rres = await readLocalFile(currentPath.value)
      if (rres.status === 'success' && rres.result) {
        codeContent.value = rres.result.content
        codeClass.value = s.ext ? `hljs language-${s.ext}` : 'hljs'
      } else {
        error.value = rres.error?.message || '读取失败'
      }
    }
  } catch (e) {
    error.value = (e as Error)?.message || '加载失败'
  } finally {
    loading.value = false
  }

  // Highlight/enhance AFTER loading=false so the content div is mounted (the refs
  // are null while the loading state shows). No further reactive change after this,
  // so the highlighted DOM is not reset by a re-render.
  await nextTick()
  if (kind.value === 'md' && mdRef.value) {
    await enhance(mdRef.value)
  } else if (kind.value === 'code' && codeRef.value) {
    try { hljs.highlightElement(codeRef.value) } catch (e) { console.warn(e) }
  }
  await scrollToAnchor()
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/** Find the 1-based line number of the first line containing the symbol (word-boundary). */
function findSymbolLine(symbol: string): number {
  const re = new RegExp(`\\b${escapeRegex(symbol)}\\b`)
  const lines = codeContent.value.split('\n')
  for (let i = 0; i < lines.length; i++) {
    if (re.test(lines[i])) return i + 1
  }
  return 0
}

/**
 * Smoothly scroll the modal body from the top down to `target` (absolute scroll
 * position). Resets to 0 first so the animation always starts at the file top.
 */
function smoothScrollTo(target: number) {
  const body = bodyRef.value
  if (!body) return
  body.scrollTop = 0
  requestAnimationFrame(() => {
    body.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
  })
}

/** Smooth-scroll so code line `n` lands ~1/3 from the top. */
function scrollToLine(n: number) {
  const body = bodyRef.value
  const code = codeRef.value
  if (!body || !code || n < 1) return
  const pre = code.parentElement // <pre class="code-content">
  if (!pre) return
  const cs = getComputedStyle(pre)
  const fs = parseFloat(cs.fontSize)
  const lhRaw = cs.lineHeight
  const lh = lhRaw.endsWith('px') ? parseFloat(lhRaw) : parseFloat(lhRaw) * fs
  const paddingTop = parseFloat(cs.paddingTop) || 0
  const bodyRect = body.getBoundingClientRect()
  const preRect = pre.getBoundingClientRect()
  const preTopInBody = preRect.top - bodyRect.top + body.scrollTop
  const lineTop = preTopInBody + paddingTop + (n - 1) * lh
  smoothScrollTo(lineTop - body.clientHeight / 3)
}

/** Scroll to a pending anchor: `L42`/`42` (line) or a symbol, or an md heading id. */
async function scrollToAnchor() {
  const a = pendingAnchor.value
  pendingAnchor.value = ''
  if (!a) return
  if (kind.value === 'code') {
    const m = a.match(/^L?(\d+)$/i)
    if (m) {
      scrollToLine(parseInt(m[1], 10))
      return
    }
    const line = findSymbolLine(a)
    if (line > 0) scrollToLine(line)
    return
  }
  if (kind.value === 'md') {
    await nextTick()
    const body = bodyRef.value
    const el = document.getElementById(a)
    if (body && el) {
      const bodyRect = body.getBoundingClientRect()
      const elRect = el.getBoundingClientRect()
      const target = elRect.top - bodyRect.top + body.scrollTop - 40
      smoothScrollTo(target)
    }
  }
}

function onTreeSelect(path: string) {
  if (/\.(md|markdown|pdf)$/i.test(path) && isUnderRoot(path)) {
    emit('open-in-reader', path)
  } else {
    currentPath.value = path
  }
}

function close() {
  emit('close')
}

watch(currentPath, () => { void load() })
// Parent opened a new target (path prop changed) — sync currentPath + reset the anchor.
watch(() => props.path, (p) => {
  currentPath.value = p
  pendingAnchor.value = props.anchor || ''
})
onMounted(() => { void load() })
</script>

<style scoped>
.ppm-overlay {
  position: fixed;
  inset: 0;
  z-index: 2900;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(12px) saturate(150%);
  -webkit-backdrop-filter: blur(12px) saturate(150%);
  animation: fade-in var(--duration-normal) var(--ease-out) both;
}
:root[data-theme="dark"] .ppm-overlay {
  background: rgba(0, 0, 0, 0.6);
}

.ppm-panel {
  width: min(960px, 100%);
  height: min(85vh, 100%);
  display: flex;
  flex-direction: column;
  background: var(--bg-glass-strong);
  backdrop-filter: blur(32px) saturate(180%);
  -webkit-backdrop-filter: blur(32px) saturate(180%);
  border: 1px solid var(--border-glass);
  border-radius: 18px;
  box-shadow: var(--shadow-lg), var(--inset-highlight);
  overflow: hidden;
  animation: ppm-panel-enter var(--duration-slow) var(--ease-spring);
}
@keyframes ppm-panel-enter {
  from { opacity: 0; transform: scale(0.95) translateY(10px); }
  to { opacity: 1; transform: scale(1) translateY(0); }
}

.ppm-head {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-faint);
}
.ppm-head-icon { color: var(--text-muted); flex-shrink: 0; }
.ppm-path {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}
.ppm-reader-btn,
.ppm-close-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 5px;
  height: 30px;
  padding: 0 12px;
  border-radius: 9px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s var(--ease-out);
}
.ppm-reader-btn:hover { color: var(--accent); border-color: var(--accent-border); }
.ppm-close-btn { padding: 0; width: 30px; justify-content: center; }
.ppm-close-btn:hover { color: #f87171; border-color: rgba(248, 113, 113, 0.3); background: rgba(248, 113, 113, 0.1); }
.rbtn-label { line-height: 1; }

.ppm-body {
  flex: 1;
  overflow: auto;
  padding: 20px 24px;
}

.ppm-state {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-faint);
  font-size: 14px;
}
.ppm-state.error { color: #f87171; }
.ppm-state .is-loading { animation: spin 1s linear infinite; color: var(--accent); }

.ppm-folder { user-select: none; }
.ppm-md { max-width: 100%; padding: 0; }
/* .ppm-code-block uses the global .code-block styles (shared with the reader). */

@media (max-width: 768px) {
  .ppm-overlay { padding: 0; }
  .ppm-panel { width: 100%; height: 100vh; border-radius: 0; border: none; }
  .rbtn-label { display: none; }
  .ppm-reader-btn { padding: 0; width: 30px; justify-content: center; }
  .ppm-body { padding: 14px; }
}
</style>
