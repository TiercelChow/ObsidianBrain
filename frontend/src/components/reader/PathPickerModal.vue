<template>
  <el-dialog
    v-model="visible"
    class="path-picker-dialog"
    modal-class="path-picker-overlay"
    title="选择文件夹或 PDF"
    width="min(720px, 92vw)"
    :close-on-click-modal="false"
    append-to-body
    @open="init"
    @closed="onClosed"
  >
    <!-- Breadcrumb + up -->
    <div class="pp-bar">
      <el-button
        class="pp-up"
        :disabled="currentPath === '/'"
        title="上一级"
        aria-label="上一级"
        @click="goUp"
      >
        <el-icon><Top /></el-icon>
      </el-button>
      <nav class="pp-crumbs" aria-label="路径">
        <button
          v-for="seg in segments"
          :key="seg.path"
          type="button"
          class="pp-crumb"
          :class="{ active: seg.path === currentPath }"
          :title="seg.path"
          @click="navigate(seg.path)"
        >
          <span class="pp-crumb-name">{{ seg.name }}</span>
        </button>
      </nav>
    </div>

    <!-- Entry list -->
    <div class="pp-list">
      <div v-if="loading" class="pp-state">
        <el-icon class="is-loading"><Loading /></el-icon><span>加载中…</span>
      </div>
      <div v-else-if="error" class="pp-state error">⚠️ {{ error }}</div>
      <div v-else-if="!pickable.length" class="pp-state">此文件夹下没有可选内容（文件夹或 PDF）</div>
      <div
        v-for="e in pickable"
        :key="e.path"
        class="pp-item"
        :class="{ dir: e.is_dir, selected: e.path === selectedPath }"
        :title="e.path"
        @click="onItemClick(e)"
      >
        <el-icon class="pp-item-icon">
          <Folder v-if="e.is_dir" />
          <Files v-else />
        </el-icon>
        <span class="pp-item-name">{{ e.name }}</span>
        <span class="pp-item-tag" :class="e.is_dir ? 'tag-dir' : 'tag-pdf'">
          {{ e.is_dir ? '文件夹' : 'PDF' }}
        </span>
        <el-icon v-if="e.path === selectedPath" class="pp-check"><Check /></el-icon>
      </div>
    </div>

    <!-- Footer: select-current-folder on the left, cancel/confirm on the right -->
    <template #footer>
      <div class="pp-footer">
        <div class="pp-footer-left">
          <el-button
            class="pp-select-dir"
            :type="selectedPath === currentPath ? 'primary' : 'default'"
            @click="selectCurrentDir"
          >
            选择当前文件夹
          </el-button>
          <span v-if="selectedPath" class="pp-sel-status">
            <el-icon><Check /></el-icon>
            {{ selectedPath === currentPath ? '当前文件夹' : selectedPath.split('/').pop() }}
          </span>
        </div>
        <span class="pp-footer-main">
          <el-button @click="visible = false">取消</el-button>
          <el-button type="primary" :disabled="!selectedPath" @click="confirm">
            确定
          </el-button>
        </span>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, Files, Folder, Loading, Top } from '@element-plus/icons-vue'
import { listLocalDir, type DirEntry } from '@/api/reader'
import { parentPath, pathSegments, pickableEntries } from '@/utils/pathPicker'

const props = defineProps<{ modelValue: boolean; initialPath?: string }>()
const emit = defineEmits<{
  'update:modelValue': [v: boolean]
  select: [path: string]
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v) => emit('update:modelValue', v),
})

const currentPath = ref('/')
const entries = ref<DirEntry[]>([])
const loading = ref(false)
const error = ref('')
const selectedPath = ref<string | null>(null)

const segments = computed(() => pathSegments(currentPath.value))
const pickable = computed(() => pickableEntries(entries.value))

/** On open: start from initialPath (or its parent if it's a file), load. */
function init() {
  const start = props.initialPath?.trim() || '/'
  selectedPath.value = null
  // If the initial path is likely a file (has an extension), start at its parent.
  currentPath.value = /\.[a-z0-9]+$/i.test(start) ? parentPath(start) : start
  load()
}

async function load() {
  loading.value = true
  error.value = ''
  try {
    const res = await listLocalDir(currentPath.value, 1)
    if (res.status !== 'success' || !res.result) {
      error.value = res.error?.message || '无法读取该目录'
      entries.value = []
    } else {
      entries.value = res.result.entries
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : '读取目录失败'
    entries.value = []
  } finally {
    loading.value = false
  }
}

function navigate(path: string) {
  if (path === currentPath.value) return
  selectedPath.value = null
  currentPath.value = path
  load()
}

function goUp() {
  const parent = parentPath(currentPath.value)
  if (parent !== currentPath.value) navigate(parent)
}

/** Folder click → drill in; PDF click → select for confirm. */
function onItemClick(e: DirEntry) {
  if (e.is_dir) {
    navigate(e.path)
  } else {
    selectedPath.value = e.path
  }
}

function selectCurrentDir() {
  selectedPath.value = currentPath.value
}

function confirm() {
  if (!selectedPath.value) return
  emit('select', selectedPath.value)
  visible.value = false
}

function onClosed() {
  entries.value = []
  selectedPath.value = null
  error.value = ''
}

// Reload if the open prop flips to true without an @open (e.g. re-open after close).
watch(
  () => props.modelValue,
  (v) => {
    if (v && currentPath.value === '/' && !entries.value.length) init()
  },
)
</script>

<style>
/* Path picker dialog — same glass modal language as the book form dialog:
   dark frosted scrim + spring scale-in (panel glass comes from index.html). */
.el-overlay.path-picker-overlay {
  background-color: rgba(0, 0, 0, 0.45) !important;
  backdrop-filter: blur(12px) saturate(150%) !important;
  -webkit-backdrop-filter: blur(12px) saturate(150%) !important;
}

.dialog-fade-enter-from .path-picker-dialog,
.dialog-fade-leave-to .path-picker-dialog {
  opacity: 0;
}
.dialog-fade-enter-from .path-picker-dialog {
  transform: scale(0.95) translateY(10px);
}
.dialog-fade-leave-to .path-picker-dialog {
  transform: scale(0.98) translateY(6px);
}

/* Mobile: top-anchored sheet radius (overrides index.html's 20px). */
@media (max-width: 768px) {
  .el-dialog.path-picker-dialog {
    border-radius: 22px 22px 0 0 !important;
  }
}
</style>

<style scoped>
/* Internal layout — scoped works because these elements are inside the
   dialog's default/footer slots, rendered within the component subtree
   (el-dialog with append-to-body teleports the *wrapper*, but slot
   content keeps the component's scoped data-v attribute). */
.pp-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: -4px 0 12px;
}
.pp-up {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  padding: 0;
  border-radius: 8px;
}
.pp-crumbs {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  scrollbar-width: thin;
  font-size: 13px;
}
.pp-crumbs::-webkit-scrollbar { height: 4px; }
.pp-crumb {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 13px;
  cursor: pointer;
  padding: 4px 6px;
  border-radius: 6px;
  white-space: nowrap;
  transition: color var(--motion-fast) var(--ease-emphasized),
    background-color var(--motion-fast) var(--ease-emphasized);
}
.pp-crumb::after {
  content: '/';
  margin: 0 1px 0 0;
  color: var(--text-faint);
}
.pp-crumb:last-child::after { content: ''; }
.pp-crumb:hover { color: var(--text-primary); background: var(--bg-glass-subtle); }
.pp-crumb.active { color: var(--accent); font-weight: 600; }

.pp-list {
  min-height: 200px;
  max-height: min(52vh, 460px);
  overflow-y: auto;
  overscroll-behavior: contain;
  border: 1px solid var(--border-faint);
  border-radius: 12px;
  padding: 4px;
}
.pp-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 200px;
  color: var(--text-faint);
  font-size: 13px;
}
.pp-state.error { color: #f87171; }
.pp-state .is-loading { animation: pp-spin 1s linear infinite; color: var(--accent); }
@keyframes pp-spin { to { transform: rotate(360deg); } }

.pp-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 9px;
  cursor: pointer;
  transition: background-color var(--motion-fast) var(--ease-emphasized);
}
.pp-item + .pp-item { margin-top: 1px; }
.pp-item:hover { background: var(--bg-glass-subtle); }
.pp-item.selected {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
.pp-item-icon {
  flex-shrink: 0;
  font-size: 16px;
  color: var(--text-muted);
}
.pp-item.selected .pp-item-icon { color: var(--accent); }
.pp-item-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13.5px;
  color: var(--text-secondary);
}
.pp-item.selected .pp-item-name { color: var(--text-primary); font-weight: 550; }
.pp-item-tag {
  flex-shrink: 0;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.03em;
  padding: 2px 7px;
  border-radius: 999px;
}
.pp-item-tag.tag-dir {
  color: var(--text-muted);
  background: var(--bg-glass-subtle);
}
.pp-item-tag.tag-pdf {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
.pp-check {
  flex-shrink: 0;
  font-size: 15px;
  color: var(--accent);
}

.pp-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.pp-footer-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.pp-sel-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12.5px;
  color: var(--accent);
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pp-sel-status .el-icon { font-size: 14px; }
.pp-footer-main {
  display: flex;
  gap: 10px;
}

@media (max-width: 768px) {
  .pp-up { width: var(--tap-target); height: var(--tap-target); }
  .pp-crumb { padding: 6px 8px; font-size: 14px; }
  .pp-item { min-height: var(--tap-target); }
  .pp-item-name { font-size: 14px; }
  .pp-list { max-height: 56vh; }
}
</style>
