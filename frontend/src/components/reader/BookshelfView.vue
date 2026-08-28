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
    <div v-else class="shelf-grid">
      <div
        v-for="b in visibleBooks"
        :key="b.id"
        class="book-card glass-surface"
        :title="b.path"
        @click="emit('open', b)"
      >
        <div class="bc-head">
          <el-icon class="bc-kind-icon">
            <FolderOpened v-if="b.kind === 'folder'" />
            <Document v-else />
          </el-icon>
          <span class="bc-name">{{ b.name }}</span>
        </div>
        <span v-if="b.category" class="bc-category">{{ b.category }}</span>
        <p class="bc-desc">{{ b.description || '　' }}</p>
        <div class="bc-foot">
          <span class="bc-progress">{{ progressLabel(b) }}</span>
          <span class="bc-actions" @click.stop>
            <button type="button" title="编辑" aria-label="编辑" @click="openEdit(b)">
              <el-icon><EditPen /></el-icon>
            </button>
            <button type="button" title="删除" aria-label="删除" @click="confirmRemove(b)">
              <el-icon><Delete /></el-icon>
            </button>
          </span>
        </div>
      </div>
    </div>

    <el-dialog
      v-model="formVisible"
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
        <el-button @click="formVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="submit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Collection, Delete, Document, EditPen, FolderOpened } from '@element-plus/icons-vue'
import { statLocalPath, type PathStat, type ReaderBook } from '@/api/reader'
import { useBookshelf } from '@/composables/useBookshelf'
import {
  bookProgressLabel,
  defaultBookName,
  deriveKind,
  makeBookId,
  sortBooks,
  validateBookForm,
} from '@/utils/readerBooks'

const emit = defineEmits<{ open: [book: ReaderBook] }>()

const shelf = useBookshelf()
const books = computed(() => sortBooks(shelf.books.value))
const loadError = computed(() => shelf.loadError.value)

const activeCategory = ref('全部')
const categories = computed(() => [
  '全部',
  ...new Set(shelf.books.value.map((b) => b.category).filter(Boolean)),
])
const visibleBooks = computed(() =>
  activeCategory.value === '全部'
    ? books.value
    : books.value.filter((b) => b.category === activeCategory.value),
)

function progressLabel(b: ReaderBook) {
  return bookProgressLabel(b)
}

// ── add / edit dialog ────────────────────────────────────────────────
const formVisible = ref(false)
const saving = ref(false)
const editingId = ref<string | null>(null)
const form = reactive({ path: '', name: '', description: '', category: '' })
const formError = ref('')
const pathStat = ref<PathStat | null>(null)

function resetForm() {
  editingId.value = null
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
  form.path = b.path
  form.name = b.name
  form.description = b.description
  form.category = b.category
  formVisible.value = true
  void refreshStat()
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
.glass-surface {
  background: var(--bg-glass);
  border: 1px solid var(--border-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
}

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

.shelf-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 14px;
}
.book-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  border-radius: 16px;
  cursor: pointer;
  transition: transform var(--motion-normal) var(--ease-spring-gentle),
    box-shadow var(--motion-normal) var(--ease-emphasized);
}
.book-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}
.bc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.bc-kind-icon {
  flex: none;
  color: var(--accent);
  font-size: 18px;
}
.bc-name {
  font-weight: 620;
  font-size: 15px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bc-category {
  align-self: flex-start;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  font-weight: 560;
  padding: 2px 9px;
  border-radius: 999px;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.bc-desc {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-secondary);
  min-height: 2.9em;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.bc-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: auto;
}
.bc-progress {
  font-size: 12px;
  color: var(--text-faint);
}
.bc-actions {
  display: flex;
  gap: 2px;
  opacity: 0.75;
}
.bc-actions button {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: color var(--motion-fast) var(--ease-emphasized),
    background-color var(--motion-fast) var(--ease-emphasized);
}
.bc-actions button:hover {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

@media (max-width: 768px) {
  .shelf-grid {
    grid-template-columns: repeat(2, 1fr);
    gap: 10px;
  }
  .book-card {
    padding: 12px;
    border-radius: 14px;
  }
  .bc-desc {
    font-size: 12px;
  }
  .shelf-add {
    min-height: var(--tap-target);
  }
}
</style>
