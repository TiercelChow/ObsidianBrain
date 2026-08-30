<template>
  <div class="bookshelf">
    <div class="shelf-toolbar">
      <div class="shelf-chips" role="tablist" aria-label="类别筛选">
        <button
          v-for="c in categories"
          :key="c"
          type="button"
          class="shelf-chip"
          :class="{ active: activeCategory === c }"
          @click="activeCategory = c"
        >
          {{ c }}
        </button>
      </div>
      <el-button type="primary" class="shelf-add" @click="openAdd">+ 添加</el-button>
    </div>

    <div v-if="loadError" class="shelf-state error">⚠️ {{ loadError }}</div>
    <div v-else-if="!books.length" class="shelf-state">
      <el-icon class="ss-icon"><Collection /></el-icon>
      <p>书架还是空的</p>
      <p class="ss-hint">把一个 Markdown 文件夹或 PDF 登记为书，点击即回到上次读到的位置</p>
      <el-button type="primary" @click="openAdd">添加第一本书</el-button>
    </div>
    <div v-else-if="!visibleBooks.length" class="shelf-state">
      <p>没有匹配的书籍</p>
      <p class="ss-hint">换个关键词，或清空搜索看看全部</p>
    </div>
    <div v-else class="shelf-grid">
      <div
        v-for="b in visibleBooks"
        :key="b.id"
        class="book-cover"
        :class="`tone-${coverTone(b.name)}`"
        :title="coverTooltip(b)"
        @click="emit('open', b)"
      >
        <span class="bc-kind">
          <el-icon><FolderOpened v-if="b.kind === 'folder'" /><Document v-else /></el-icon>
        </span>
        <button type="button" class="bc-more" title="书籍操作" aria-label="书籍操作" @click.stop="openEdit(b)">
          <el-icon><MoreFilled /></el-icon>
        </button>
        <span class="bc-actions" @click.stop>
          <button type="button" title="编辑" aria-label="编辑" @click="openEdit(b)">
            <el-icon><EditPen /></el-icon>
          </button>
          <button type="button" title="删除" aria-label="删除" @click="confirmRemove(b)">
            <el-icon><Delete /></el-icon>
          </button>
        </span>
        <h3 class="bc-title">{{ b.name }}</h3>
        <div class="bc-meta">
          <span v-if="b.category" class="bc-cat">{{ b.category }}</span>
          <span class="bc-track">
            <span class="bc-fill" :style="{ width: `${Math.round(bookProgressRatio(b) * 100)}%` }"></span>
          </span>
          <span class="bc-label" :class="{ dim: !b.progress }">{{ progressLabel(b) }}</span>
        </div>
      </div>
    </div>

    <el-dialog
      v-model="formVisible"
      class="book-form-dialog"
      modal-class="book-form-overlay"
      :title="editingId ? '编辑书籍' : '添加书籍'"
      width="min(480px, 92vw)"
      :close-on-click-modal="false"
      append-to-body
    >
      <el-form label-position="top" @submit.prevent>
        <el-form-item label="路径（文件夹或 PDF 文件）" :error="formError || undefined">
          <el-input v-model="form.path" placeholder="/Users/you/Documents/book.pdf" @blur="onPathBlur" />
        </el-form-item>
        <el-form-item label="书名">
          <el-input v-model="form.name" placeholder="留空则使用文件名" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="2" placeholder="这本书讲什么（可空）" />
        </el-form-item>
        <el-form-item label="类别">
          <el-input v-model="form.category" placeholder="如：技术 / 论文 / 小说（可空）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="form-footer">
          <el-button v-if="editingId" class="danger" @click="removeFromDialog">删除</el-button>
          <span v-else></span>
          <span class="form-footer-main">
            <el-button @click="formVisible = false">取消</el-button>
            <el-button type="primary" :loading="saving" @click="submit">保存</el-button>
          </span>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Collection, Delete, Document, EditPen, FolderOpened, MoreFilled } from '@element-plus/icons-vue'
import { statLocalPath, type PathStat, type ReaderBook } from '@/api/reader'
import { useBookshelf } from '@/composables/useBookshelf'
import {
  bookProgressLabel,
  bookProgressRatio,
  coverTone,
  defaultBookName,
  deriveKind,
  makeBookId,
  matchesBookQuery,
  sortBooks,
  validateBookForm,
} from '@/utils/readerBooks'

const emit = defineEmits<{ open: [book: ReaderBook] }>()
const props = defineProps<{ query?: string }>()

const shelf = useBookshelf()
const books = computed(() => sortBooks(shelf.books.value))
const loadError = computed(() => shelf.loadError.value)

const activeCategory = ref('全部')
const categories = computed(() => [
  '全部',
  ...new Set(shelf.books.value.map((b) => b.category).filter(Boolean)),
])
const visibleBooks = computed(() =>
  books.value.filter(
    (b) =>
      (activeCategory.value === '全部' || b.category === activeCategory.value) &&
      matchesBookQuery(b, props.query ?? ''),
  ),
)

function progressLabel(b: ReaderBook) {
  return bookProgressLabel(b)
}

/** Native tooltip: description + path live here — covers show only name/category/progress. */
function coverTooltip(b: ReaderBook) {
  return `${b.name}${b.description ? `\n${b.description}` : ''}\n${b.path}`
}

// ── add / edit dialog ────────────────────────────────────────────────
const formVisible = ref(false)
const saving = ref(false)
const editingId = ref<string | null>(null)
const editingBook = ref<ReaderBook | null>(null)
const form = reactive({ path: '', name: '', description: '', category: '' })
const formError = ref('')
const pathStat = ref<PathStat | null>(null)

function resetForm() {
  editingId.value = null
  editingBook.value = null
  form.path = ''
  form.name = ''
  form.description = ''
  form.category = ''
  formError.value = ''
  pathStat.value = null
}

function openAdd() {
  resetForm()
  formVisible.value = true
}

function openEdit(b: ReaderBook) {
  resetForm()
  editingId.value = b.id
  editingBook.value = b
  form.path = b.path
  form.name = b.name
  form.description = b.description
  form.category = b.category
  formVisible.value = true
  void refreshStat()
}

/** Delete from the edit sheet — the mobile ⋯ entry lands here too. */
function removeFromDialog() {
  const b = editingBook.value
  formVisible.value = false
  if (b) void confirmRemove(b)
}

async function refreshStat() {
  if (!form.path.trim()) {
    pathStat.value = null
    return
  }
  try {
    const res = await statLocalPath(form.path.trim())
    pathStat.value = res.status === 'success' && res.result ? res.result : null
  } catch {
    pathStat.value = null
  }
  validate()
}

function onPathBlur() {
  // 书名留空时，随路径默认取文件名
  if (!editingId.value && !form.name.trim()) form.name = defaultBookName(form.path.trim())
  void refreshStat()
}

function validate() {
  formError.value =
    validateBookForm(form.path.trim(), editingId.value, shelf.books.value, pathStat.value) ?? ''
}

watch(
  () => form.path,
  () => {
    if (formVisible.value) validate()
  },
)

async function submit() {
  await refreshStat()
  validate()
  if (formError.value) return
  const path = form.path.trim()
  const base = {
    path,
    kind: deriveKind(path),
    name: form.name.trim() || defaultBookName(path),
    description: form.description.trim(),
    category: form.category.trim(),
  }
  saving.value = true
  try {
    let ok: boolean
    if (editingId.value) {
      const current = shelf.books.value.find((b) => b.id === editingId.value)
      if (!current) {
        ElMessage.error('书籍不存在，请刷新后重试')
        return
      }
      ok = await shelf.updateBook({ ...current, ...base })
    } else {
      ok = await shelf.addBook({ ...base, id: makeBookId(), addedAt: Date.now() })
    }
    if (!ok) {
      ElMessage.error('保存失败，请重试')
      return
    }
    formVisible.value = false
  } finally {
    saving.value = false
  }
}

async function confirmRemove(b: ReaderBook) {
  try {
    await ElMessageBox.confirm(`删除「${b.name}」？阅读进度会一并移除（不会动磁盘文件）。`, '删除书籍', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
  } catch {
    return
  }
  const ok = await shelf.removeBook(b.id)
  if (!ok) ElMessage.error('删除失败，请重试')
}
</script>

<style scoped>
.bookshelf {
  min-height: 100%;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.shelf-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
.shelf-chips {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  flex: 1;
  min-width: 0;
}
.shelf-chip {
  min-height: 32px;
  padding: 0 14px;
  border-radius: 999px;
  border: 1px solid var(--border-glass);
  background: var(--bg-glass);
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 550;
  cursor: pointer;
  transition: color var(--motion-fast) var(--ease-emphasized),
    border-color var(--motion-fast) var(--ease-emphasized),
    background-color var(--motion-fast) var(--ease-emphasized);
}
.shelf-chip:hover {
  color: var(--text-primary);
}
.shelf-chip.active {
  color: var(--accent);
  border-color: var(--accent-border);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.shelf-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--text-faint);
  padding: 60px 0;
  text-align: center;
}
.shelf-state.error {
  color: #f87171;
}
.ss-icon {
  font-size: 40px;
}
.ss-hint {
  font-size: 13px;
}

/* ── Book wall: covers on glass shelf boards ──
   Rows are a fixed pitch (cover height + row gap) so one repeating
   gradient paints a board under every row, including the last. */
.shelf-grid {
  --cover-h: 230px;
  --board: 10px;
  --row-gap: 26px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  grid-auto-rows: var(--cover-h);
  gap: var(--row-gap) 14px;
  padding-bottom: calc(var(--board) + 4px);
  background: repeating-linear-gradient(
    to bottom,
    transparent 0 calc(var(--cover-h) + 1px),
    rgba(255, 255, 255, 0.55) calc(var(--cover-h) + 1px) calc(var(--cover-h) + 2px),
    var(--bg-glass) calc(var(--cover-h) + 2px) calc(var(--cover-h) + var(--board)),
    rgba(15, 18, 25, 0.12) calc(var(--cover-h) + var(--board)) calc(var(--cover-h) + var(--board) + 3px),
    transparent calc(var(--cover-h) + var(--board) + 3px) calc(var(--cover-h) + var(--row-gap))
  );
}

.book-cover {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: 4px 8px 8px 4px;
  background: linear-gradient(155deg, var(--cover-b) 0%, var(--cover-a) 100%);
  cursor: pointer;
  overflow: hidden;
  box-shadow: var(--shadow-sm), inset 0 1px 0 rgba(255, 255, 255, 0.18);
  transition: transform var(--motion-normal) var(--ease-spring-gentle),
    box-shadow var(--motion-normal) var(--ease-emphasized);
}
/* Bound spine edge + a soft sheen on the fore-edge. */
.book-cover::before {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(to right, rgba(0, 0, 0, 0.38) 0, rgba(0, 0, 0, 0.14) 7px, transparent 16px),
    radial-gradient(ellipse 120% 60% at 85% -10%, rgba(255, 255, 255, 0.22), transparent 55%);
  pointer-events: none;
}
.book-cover:hover {
  transform: translateY(-6px);
  box-shadow: var(--shadow-md), inset 0 1px 0 rgba(255, 255, 255, 0.18);
}

/* Cover tones — deterministic per book name (coverTone). Deep, bookish. */
.tone-0 { --cover-a: #4338ca; --cover-b: #6366f1; } /* indigo */
.tone-1 { --cover-a: #115e59; --cover-b: #0d9488; } /* teal */
.tone-2 { --cover-a: #9f1239; --cover-b: #be123c; } /* rose */
.tone-3 { --cover-a: #92400e; --cover-b: #b45309; } /* amber */
.tone-4 { --cover-a: #1e3a5f; --cover-b: #2d5b8e; } /* navy */
.tone-5 { --cover-a: #166534; --cover-b: #15803d; } /* forest */
.tone-6 { --cover-a: #701a75; --cover-b: #86198f; } /* plum */
.tone-7 { --cover-a: #374151; --cover-b: #4b5563; } /* slate */

.bc-kind {
  position: absolute;
  top: 9px;
  left: 10px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 15px;
}
.bc-actions {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-emphasized);
}
.book-cover:hover .bc-actions,
.book-cover:focus-within .bc-actions {
  opacity: 1;
}
.bc-actions button {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border: 0;
  border-radius: 7px;
  background: rgba(0, 0, 0, 0.32);
  color: rgba(255, 255, 255, 0.92);
  cursor: pointer;
  transition: background-color var(--motion-fast) var(--ease-emphasized),
    transform var(--motion-fast) var(--ease-emphasized);
}
.bc-actions button:hover {
  background: rgba(0, 0, 0, 0.5);
}
.bc-actions button:active {
  transform: scale(0.92);
}

.bc-title {
  margin: 36px 0 0;
  padding: 0 12px 0 14px;
  color: #fff;
  font-size: 15px;
  font-weight: 650;
  line-height: 1.4;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  overflow-wrap: anywhere;
}

.bc-meta {
  margin-top: auto;
  padding: 0 12px 11px 14px;
}
.bc-cat {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: rgba(255, 255, 255, 0.78);
  margin-bottom: 6px;
}
.bc-track {
  display: block;
  height: 3px;
  border-radius: 2px;
  background: rgba(0, 0, 0, 0.28);
  overflow: hidden;
}
.bc-fill {
  display: block;
  height: 100%;
  border-radius: 2px;
  background: linear-gradient(90deg, rgba(255, 255, 255, 0.55), rgba(255, 255, 255, 0.95));
  transition: width var(--motion-normal) var(--ease-emphasized);
}
.bc-label {
  display: block;
  margin-top: 5px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.85);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}
.bc-label.dim {
  color: rgba(255, 255, 255, 0.6);
}

/* Touch devices: one subtle ⋯ instead of the hover action pair; the
   edit sheet it opens carries delete in its footer. */
.bc-more {
  display: none;
}
@media (hover: none) {
  .bc-actions {
    display: none;
  }
  .bc-more {
    display: grid;
    place-items: center;
    position: absolute;
    top: 7px;
    right: 7px;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 7px;
    background: rgba(0, 0, 0, 0.22);
    color: rgba(255, 255, 255, 0.88);
    font-size: 13px;
    cursor: pointer;
  }
  .bc-more:active {
    transform: scale(0.92);
  }
  .bc-title {
    padding-right: 38px;
  }
}

.form-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.form-footer-main {
  display: flex;
  gap: 10px;
}
.form-footer .danger {
  color: #ef4444;
}

@media (max-width: 768px) {
  .shelf-grid {
    /* Same height/width ratio as desktop (230/177): 165px columns capped
       and centered, instead of stretching covers to half the viewport. */
    --cover-h: 214px;
    grid-template-columns: repeat(2, minmax(0, 165px));
    justify-content: center;
    gap: var(--row-gap) 12px;
  }
  .bc-title {
    font-size: 13.5px;
    margin-top: 36px;
  }
  .shelf-add {
    min-height: var(--tap-target);
  }
}
</style>

<style>
/* 添加/编辑书籍弹窗 — 遮罩与入场动画对齐应用的自定义弹窗（PathPreviewModal 等）：
   暗色磨砂遮罩 + 弹簧缩放入场。面板玻璃样式由 index.html 的 Glass Dialog 全局
   规则提供，此处不重复覆盖。
   全局作用域：弹窗 teleport 到 body，scoped 样式够不到；!important 是为了压过
   index.html 的 .el-overlay 覆盖（浅色主题 --el-mask-color 为白色 0.9，不适合
   做弹窗遮罩）。 */
.el-overlay.book-form-overlay {
  background-color: rgba(0, 0, 0, 0.45) !important;
  backdrop-filter: blur(12px) saturate(150%) !important;
  -webkit-backdrop-filter: blur(12px) saturate(150%) !important;
}
/* 深色/护眼主题的遮罩底色由 App.vue 的主题规则接管（优先级更高）。 */

/* 入场/离场：借 EP 的 dialog-fade transition 给面板加缩放；时长与曲线由
   motion.css 的 .el-dialog 过渡（transform/opacity, spring-gentle）接管。 */
.dialog-fade-enter-from .book-form-dialog {
  transform: scale(0.95) translateY(10px);
  opacity: 0;
}
.dialog-fade-leave-to .book-form-dialog {
  transform: scale(0.98) translateY(6px);
  opacity: 0;
}

/* 移动端：保持 App.vue 的顶贴弹层造型（顶部圆角、底部直角）；更高优先级
   压过 index.html 的 20px 全局圆角（后者会盖过 App.vue 的同优先级规则）。 */
@media (max-width: 768px) {
  .el-dialog.book-form-dialog {
    border-radius: 22px 22px 0 0 !important;
  }
}
</style>
