<template>
  <section class="epub-viewer" :aria-busy="loading || chapterLoading">
    <div v-if="loading" class="epub-state" role="status" aria-label="加载 EPUB">
      <el-icon class="is-loading"><Loading /></el-icon>
    </div>
    <p v-if="error" class="epub-error" role="alert">{{ error }}</p>
    <template v-if="book">
      <nav class="epub-chapter-bar" aria-label="EPUB 章节">
        <UiAction
          :icon="ArrowLeft"
          label="上一章"
          :disabled="index === 0 || chapterLoading"
          @click="change(index - 1)"
        />
        <span
          ><strong>{{ book.chapters[index]?.title }}</strong
          ><small>{{ index + 1 }} / {{ book.chapters.length }}</small></span
        >
        <UiAction
          :icon="ArrowRight"
          label="下一章"
          :disabled="index === book.chapters.length - 1 || chapterLoading"
          @click="change(index + 1)"
        />
      </nav>
      <div
        v-if="chapterLoading"
        class="epub-state"
        role="status"
        aria-label="加载章节"
      >
        <el-icon class="is-loading"><Loading /></el-icon>
      </div>
      <iframe
        v-if="src && !error"
        ref="frame"
        :key="src"
        :src="src"
        class="epub-frame"
        :data-epub-chapter="chapter"
        :style="{
          height: `${height}px`,
          visibility: chapterLoading ? 'hidden' : 'visible',
        }"
        sandbox="allow-same-origin"
        :title="book.chapters[index]?.title || 'EPUB 章节'"
        @load="loaded"
        @error="fail('章节读取失败')"
      />
      <nav v-if="!chapterLoading" class="epub-bottom" aria-label="翻章">
        <button v-if="index > 0" type="button" @click="change(index - 1)">
          <el-icon><ArrowLeft /></el-icon>上一章</button
        ><button
          v-if="index < book.chapters.length - 1"
          type="button"
          @click="change(index + 1)"
        >
          下一章<el-icon><ArrowRight /></el-icon>
        </button>
      </nav>
    </template>
  </section>
</template>
<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref } from 'vue'
import { ArrowLeft, ArrowRight, Loading } from '@element-plus/icons-vue'
import UiAction from '@/components/motion/UiAction'
import { epubPosition, epubRestore, epubResourceUrl } from '@/utils/epub'
import { normalizeEpubInlinePaint, syncEpubPalette } from '@/utils/epubChapterTheme'
import chapterCss from './epubChapter.css?inline'
interface Chapter {
  href: string
  title: string
}
interface Book {
  title: string
  chapters: Chapter[]
  toc: Chapter[]
}
const props = defineProps<{
  file: string
  bookId: string
  initialPosition: number
}>()
const emit = defineEmits<{
  outline: [toc: Chapter[], file: string]
  ready: [chapter: string, file: string]
  loaderror: [message: string, file: string]
  location: [value: { file: string; position: number }]
  point: [point: { x: number; y: number }]
  interact: []
  beforechange: []
}>()
const frame = ref<HTMLIFrameElement | null>(null),
  book = ref<Book | null>(null),
  base = ref(''),
  index = ref(0),
  height = ref(1),
  loading = ref(true),
  chapterLoading = ref(true),
  error = ref('')
const chapter = computed(() => book.value?.chapters[index.value]?.href || '')
const src = computed(() =>
  chapter.value ? epubResourceUrl(base.value, chapter.value) : '',
)
let disposed = false,
  ready = false,
  restoreRatio: number | null = null,
  anchor = '',
  resize: ResizeObserver | null = null,
  theme: MutationObserver | null = null,
  events: AbortController | null = null,
  loadTimer: ReturnType<typeof setTimeout> | null = null,
  restoreTimer: ReturnType<typeof setTimeout> | null = null,
  layoutFrame = 0
const request = new AbortController()
function pane() {
  return frame.value?.closest<HTMLElement>('.pane-center') || null
}
function ratio() {
  const root = pane()
  return root
    ? Math.max(
        0,
        Math.min(
          1,
          root.scrollTop / Math.max(1, root.scrollHeight - root.clientHeight),
        ),
      )
    : 0
}
function getProgress() {
  return ready && book.value
    ? epubPosition(index.value, ratio(), book.value.chapters.length)
    : null
}
function cleanupChapter() {
  resize?.disconnect()
  events?.abort()
  if (loadTimer) clearTimeout(loadTimer)
  if (restoreTimer) clearTimeout(restoreTimer)
  cancelAnimationFrame(layoutFrame)
}
function fail(message: string) {
  if (disposed) return
  cleanupChapter()
  ready = false
  loading.value = false
  chapterLoading.value = false
  error.value = message
  emit('loaderror', message, props.file)
}
function change(target: number, position = 0, fragment = '') {
  if (!book.value || target < 0 || target >= book.value.chapters.length)
    return false
  if (target === index.value && ready) {
    restoreRatio = position
    anchor = fragment
    restore()
    return true
  }
  if (ready) emit('beforechange')
  cleanupChapter()
  index.value = target
  height.value = 1
  ready = false
  chapterLoading.value = true
  error.value = ''
  restoreRatio = position
  anchor = fragment
  loadTimer = setTimeout(() => fail('章节加载超时，请重新打开这本书。'), 30_000)
  return true
}
function goTo(href: string, position = 0) {
  const [path, ...hash] = href.split('#'),
    target =
      book.value?.chapters.findIndex(
        (item) => item.href.split('#')[0] === path,
      ) ?? -1
  return change(target, position, hash.join('#'))
}
function restore() {
  const root = pane(),
    doc = frame.value?.contentDocument
  if (!root || !doc) return
  if (anchor) {
    const target =
      doc.getElementById(anchor) || doc.getElementsByName(anchor)[0]
    if (target && frame.value) {
      root.scrollTop +=
        target.getBoundingClientRect().top +
        frame.value.getBoundingClientRect().top -
        root.getBoundingClientRect().top -
        24
      anchor = ''
      restoreRatio = null
      return
    }
  }
  if (restoreRatio !== null)
    root.scrollTop =
      restoreRatio * Math.max(0, root.scrollHeight - root.clientHeight)
}
function layout() {
  cancelAnimationFrame(layoutFrame)
  layoutFrame = requestAnimationFrame(() => {
    const doc = frame.value?.contentDocument
    if (disposed || !doc?.body) return
    const content = Math.ceil(doc.body.getBoundingClientRect().height)
    if (content > 2_000_000) {
      fail('此章节过长或存在不稳定的页面布局，请拆分章节后阅读。')
      return
    }
    height.value = Math.max(80, content)
    void nextTick(() => {
      restore()
      if (ready) {
        emit('ready', `${chapter.value}:${height.value}`, props.file)
        const position = getProgress()
        if (position !== null) emit('location', { file: props.file, position })
      }
    })
  })
}
function updatePalette(doc: Document) {
  syncEpubPalette(doc, getComputedStyle(document.documentElement))
}
async function loaded() {
  if (disposed) return
  const doc = frame.value?.contentDocument,
    root = pane()
  if (!doc?.head || !doc.body || !root || doc.contentType === 'application/json') {
    fail(doc?.body?.textContent || '章节读取失败')
    return
  }
  if (loadTimer) clearTimeout(loadTimer)
  events = new AbortController()
  const signal = events.signal
  normalizeEpubInlinePaint(doc)
  updatePalette(doc)
  const style = doc.createElement('style')
  style.textContent = chapterCss
  doc.head.prepend(style)
  doc.addEventListener(
    'click',
    (event) => {
      const target = (event.target as Element).closest('a[href]')
      if (!target) return
      event.preventDefault()
      const href = target.getAttribute('href') || ''
      try {
        const url = new URL(href, frame.value!.src),
          prefix = new URL(base.value, location.origin)
        if (!url.href.startsWith(prefix.href)) return
        goTo(
          decodeURIComponent(url.pathname.slice(prefix.pathname.length)) +
            decodeURIComponent(url.hash),
        )
      } catch {
        /* invalid links remain inert */
      }
    },
    { signal },
  )
  const interact = () => {
    restoreRatio = null
    anchor = ''
    emit('interact')
  }
  doc.addEventListener(
    'pointerdown',
    (event) => {
      interact()
      const rect = frame.value?.getBoundingClientRect()
      if (rect)
        emit('point', {
          x: rect.left + event.clientX,
          y: rect.top + event.clientY,
        })
    },
    { signal },
  )
  for (const type of ['wheel', 'touchstart', 'keydown']) {
    doc.addEventListener(type, interact, { signal, passive: true })
    root.addEventListener(type, interact, { signal, passive: true })
  }
  resize = new ResizeObserver(layout)
  resize.observe(doc.body)
  ready = true
  chapterLoading.value = false
  await nextTick()
  layout()
  restoreTimer = setTimeout(() => {
    restoreRatio = null
    anchor = ''
  }, 1800)
  emit('ready', chapter.value, props.file)
}
onMounted(async () => {
  try {
    const response = await fetch(
      `/v1/reader/epub/${encodeURIComponent(props.bookId)}/metadata?path=${encodeURIComponent(props.file)}`,
      { signal: request.signal },
    )
    const data = await response.json()
    if (!response.ok) throw new Error(data.message || 'EPUB 读取失败')
    if (disposed) return
    book.value = data.book
    base.value = data.resource_base
    if (!book.value?.chapters.length) throw new Error('书中没有可读章节')
    emit('outline', book.value.toc, props.file)
    const saved = epubRestore(props.initialPosition, book.value.chapters.length)
    change(saved.index, saved.ratio)
    theme = new MutationObserver(() => {
      const doc = frame.value?.contentDocument
      if (ready && doc) updatePalette(doc)
    })
    theme.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
  } catch (e) {
    if (!request.signal.aborted) fail((e as Error).message)
  } finally {
    if (!disposed) loading.value = false
  }
})
onBeforeUnmount(() => {
  disposed = true
  request.abort()
  cleanupChapter()
  theme?.disconnect()
})
defineExpose({ getProgress, goTo })
</script>
<style scoped>
.epub-viewer {
  min-width: 0;
  max-width: 100%;
  width: min(100%, 900px);
  margin: 0 auto;
  color: var(--text-primary);
}
.epub-frame {
  display: block;
  border: 0;
  width: 100%;
  background: transparent;
  color-scheme: inherit;
}
.epub-chapter-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
}
.epub-chapter-bar > span {
  display: grid;
  gap: 4px;
  flex: 1;
  min-width: 0;
  text-align: center;
}
.epub-chapter-bar strong {
  font-size: 13px;
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.epub-chapter-bar small {
  font-size: 11px;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.epub-state {
  display: grid;
  place-items: center;
  min-height: 100px;
  color: var(--text-muted);
  font-size: 24px;
}
.epub-error {
  padding: 24px;
  color: var(--danger);
  font-size: 14px;
  overflow-wrap: anywhere;
}
.epub-bottom {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 20px 32px;
}
.epub-bottom button {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 0;
  border-radius: 12px;
  background: var(--bg-hover);
  color: var(--text-primary);
  padding: 10px 16px;
  min-height: 44px;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.epub-bottom button:last-child {
  margin-left: auto;
}
</style>
