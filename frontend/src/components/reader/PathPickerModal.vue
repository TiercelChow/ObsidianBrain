<template>
  <el-dialog
    v-model="visible"
    class="path-picker-dialog"
    modal-class="path-picker-overlay"
    title="选择文件夹、PDF 或 EPUB"
    width="min(720px, 92vw)"
    :close-on-click-modal="false"
    append-to-body
  >
    <div class="pp-bar">
      <UiAction :icon="Top" label="上一级" :disabled="!currentPath" @click="goUp" />
      <form class="pp-address" data-glass="inset" @submit.prevent="openAddress(address)">
        <UiAction :icon="Monitor" label="浏览位置" :disabled="loading && !locations" @click="showLocations" />
        <input
          v-if="editingAddress"
          ref="addressInput"
          v-model="address"
          class="pp-input"
          type="text"
          aria-label="目录或书籍路径"
          :placeholder="locations?.platform === 'windows' ? 'D:\\藏书' : '输入或粘贴绝对路径'"
          autocomplete="off"
          autocapitalize="off"
          :spellcheck="false"
          @keydown.esc.stop.prevent="editingAddress = false"
        />
        <nav v-else class="pp-crumbs" aria-label="当前路径" :title="currentPath || undefined">
          <template v-if="segments.length > 1">
            <button type="button" class="pp-crumb pp-root" :title="segments[0].path" @click="navigate(segments[0].path)">
              {{ segments[0].name === '/' ? '根目录' : segments[0].name }}
            </button>
            <el-icon class="pp-divider" aria-hidden="true"><ArrowRight /></el-icon>
          </template>
          <ElDropdown v-if="ancestors.length" trigger="click" placement="bottom-start" popper-class="pp-ancestor-popover" @command="navigate">
            <button type="button" class="pp-current-trigger" :aria-label="`上级目录：${currentLabel}`">
              <span class="pp-current" aria-current="location">{{ currentLabel }}</span>
              <el-icon aria-hidden="true"><ArrowDown /></el-icon>
            </button>
            <template #dropdown>
              <ElDropdownMenu>
                <ElDropdownItem v-for="seg in ancestors" :key="seg.path" :command="seg.path" :title="seg.path">
                  <span class="pp-ancestor">{{ seg.name }}</span>
                </ElDropdownItem>
              </ElDropdownMenu>
            </template>
          </ElDropdown>
          <span v-else class="pp-current" aria-current="location">{{ currentLabel }}</span>
        </nav>
        <UiAction v-if="editingAddress" :icon="Check" label="前往路径" @click="openAddress(address)" />
        <UiAction v-else :icon="EditPen" label="输入路径" @click="editAddress" />
      </form>
    </div>

    <!-- Entry list -->
    <div ref="list" class="pp-list" :aria-busy="loading">
      <div v-if="loading" class="pp-state" role="status" aria-label="加载目录">
        <el-icon class="is-loading" aria-hidden="true"><Loading /></el-icon>
      </div>
      <template v-else>
      <div v-if="error" class="pp-error" role="alert"><el-icon aria-hidden="true"><WarningFilled /></el-icon><span>{{ error }}</span></div>
      <div v-if="currentPath && !pickable.length && !error" class="pp-state">没有可选的文件夹或书籍</div>
      <div v-if="!currentPath && !places.length && !error" class="pp-state">输入路径以打开本地目录</div>
      <button v-for="place in currentPath ? [] : places" :key="place.path" type="button" class="pp-item" :title="place.path" @click="navigate(place.path)">
        <el-icon class="pp-item-icon" aria-hidden="true"><component :is="place.icon" /></el-icon>
        <span class="pp-item-name">{{ place.name }}</span>
        <el-icon class="pp-next" aria-hidden="true"><ArrowRight /></el-icon>
      </button>
      <button
        v-for="e in currentPath ? pickable : []"
        :key="e.path"
        type="button"
        class="pp-item"
        :class="{ selected: e.path === selectedPath }"
        :title="e.path"
        :aria-pressed="e.is_dir ? undefined : e.path === selectedPath"
        @click="onItemClick(e)"
      >
        <el-icon class="pp-item-icon" aria-hidden="true">
          <Folder v-if="e.is_dir" />
          <Files v-else />
        </el-icon>
        <span class="pp-item-name">{{ e.name }}</span>
        <span v-if="!e.is_dir" class="pp-item-tag">
          {{ e.is_epub ? 'EPUB' : 'PDF' }}
        </span>
        <el-icon v-if="e.is_dir" class="pp-next" aria-hidden="true"><ArrowRight /></el-icon>
        <el-icon v-else-if="e.path === selectedPath" class="pp-check" aria-hidden="true"><Check /></el-icon>
      </button>
      </template>
    </div>

    <!-- Footer: select-current-folder on the left, cancel/confirm on the right -->
    <template #footer>
      <div class="pp-footer">
        <span v-if="selectedPath" class="pp-sel-status" :title="selectedPath" role="status">
          {{ selectedPath === currentPath ? '已选当前文件夹' : `已选 ${localPathName(selectedPath)}` }}
        </span>
        <el-button v-if="currentPath" class="pp-select-dir" :disabled="loading" @click="selectedPath = currentPath">
          选此文件夹
        </el-button>
        <span class="pp-footer-main">
          <el-button @click="visible = false">取消</el-button>
          <el-button type="primary" :disabled="loading || !selectedPath" @click="confirm">
            确定
          </el-button>
        </span>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ElDropdown, ElDropdownItem, ElDropdownMenu } from 'element-plus'
import { ArrowDown, ArrowRight, Check, EditPen, Files, Folder, House, Loading, Monitor, Top, WarningFilled } from '@element-plus/icons-vue'
import { getReaderLocations, listLocalDir, statLocalPath, type DirEntry, type ReaderLocations } from '@/api/reader'
import { localPathName, normalizePickerPath, parentPath, pathSegments, pickableEntries } from '@/utils/pathPicker'
import UiAction from '@/components/motion/UiAction'

const props = defineProps<{ modelValue: boolean; initialPath?: string }>()
const emit = defineEmits<{
  'update:modelValue': [v: boolean]
  select: [path: string]
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (v) => emit('update:modelValue', v),
})

const locations = ref<ReaderLocations | null>(null)
const currentPath = ref('')
const entries = ref<DirEntry[]>([])
const loading = ref(false)
const error = ref('')
const selectedPath = ref<string | null>(null)
const editingAddress = ref(false)
const address = ref('')
const addressInput = ref<HTMLInputElement | null>(null)
const list = ref<HTMLElement | null>(null)
let request: AbortController | null = null

const segments = computed(() => currentPath.value ? pathSegments(currentPath.value) : [])
const ancestors = computed(() => segments.value.slice(1, -1))
const currentLabel = computed(() => {
  const name = segments.value[segments.value.length - 1]?.name
  return name === '/' ? '根目录' : name || (locations.value?.platform === 'windows' ? '此电脑' : '位置')
})
const pickable = computed(() => pickableEntries(entries.value))
const places = computed(() => {
  const home = locations.value?.home
  const roots = locations.value?.roots ?? []
  return [
    ...(home && !roots.includes(home) ? [{ path: home, name: '个人目录', icon: House }] : []),
    ...roots.map((path) => ({ path, name: path === '/' ? '根目录' : localPathName(path), icon: Monitor })),
  ]
})

function cancelRequest() {
  request?.abort()
  request = null
  loading.value = false
}

async function run(action: (signal: AbortSignal) => Promise<void>) {
  cancelRequest()
  const controller = new AbortController()
  request = controller
  loading.value = true
  error.value = ''
  try {
    await action(controller.signal)
  } catch (e) {
    if (!controller.signal.aborted) error.value = e instanceof Error ? e.message : '读取目录失败'
  } finally {
    if (request === controller) {
      request = null
      loading.value = false
    }
  }
}

async function loadDirectory(path: string, signal: AbortSignal, selection: string | null = null) {
  const response = await listLocalDir(path, 0, signal)
  if (signal.aborted) return
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '无法读取此文件夹')
  currentPath.value = path
  entries.value = response.result.entries
  selectedPath.value = selection
  editingAddress.value = false
  if (list.value) list.value.scrollTop = 0
}

async function openPath(path: string, signal: AbortSignal) {
  const stat = await statLocalPath(path, signal)
  if (signal.aborted) return
  if (stat.status !== 'success' || !stat.result?.exists) throw new Error('找不到此路径，或当前没有访问权限')
  if (stat.result.is_dir) return loadDirectory(path, signal)
  if (!stat.result.is_file || !/\.(pdf|epub)$/i.test(path)) throw new Error('请选择文件夹、PDF 或 EPUB')
  return loadDirectory(parentPath(path), signal, path)
}

function showLocations() {
  cancelRequest()
  currentPath.value = ''
  entries.value = []
  selectedPath.value = null
  editingAddress.value = false
  error.value = ''
}

async function editAddress() {
  address.value = currentPath.value
  editingAddress.value = true
  await nextTick()
  addressInput.value?.select()
  addressInput.value?.focus()
}

function openAddress(value: string) {
  selectedPath.value = null
  const path = normalizePickerPath(value)
  if (!path) {
    cancelRequest()
    error.value = '请输入完整的本地或共享路径'
    return
  }
  void run((signal) => openPath(path, signal))
}

function navigate(path: string) {
  if (path === currentPath.value) return
  selectedPath.value = null
  void run((signal) => loadDirectory(path, signal))
}

function goUp() {
  if (!currentPath.value) return
  const parent = parentPath(currentPath.value)
  if (parent === currentPath.value) showLocations()
  else navigate(parent)
}

function onItemClick(e: DirEntry) {
  if (e.is_dir) {
    navigate(e.path)
  } else {
    selectedPath.value = e.path
  }
}

function confirm() {
  if (!selectedPath.value || loading.value) return
  emit('select', selectedPath.value)
  visible.value = false
}

watch(visible, (open) => {
  showLocations()
  if (!open) return
  void run(async (signal) => {
    const host = await getReaderLocations(signal)
    if (signal.aborted) return
    locations.value = host
    const initial = normalizePickerPath(props.initialPath ?? '')
    if (initial) await openPath(initial, signal)
    else if (host.platform !== 'windows') await loadDirectory(host.home ?? host.roots[0] ?? '/', signal)
  })
}, { immediate: true })
onBeforeUnmount(cancelRequest)
</script>

<style>
/* Path picker dialog — same glass modal language as the book form dialog:
   dark frosted scrim + spring scale-in (panel glass comes from index.html). */
.el-overlay.path-picker-overlay {
  background-color: rgba(0, 0, 0, 0.45) !important;
  backdrop-filter: var(--glass-scrim-filter) !important;
  -webkit-backdrop-filter: var(--glass-scrim-filter) !important;
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
.pp-ancestor-popover .el-dropdown-menu {
  max-height: min(40dvh, 320px);
  overflow-y: auto;
  overscroll-behavior: contain;
  border-radius: 12px;
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
  gap: 4px;
  margin-bottom: 14px;
}
.pp-bar :deep(.ui-action) { flex: none; }
.pp-address {
  display: flex;
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: 3px;
  border-radius: 16px;
  padding: 2px;
}
.pp-crumbs {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
}
.pp-crumb, .pp-current, .pp-ancestor {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pp-crumb {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  padding: 0 6px;
  border-radius: 9px;
  min-height: 36px;
}
.pp-root { flex: none; max-width: 30%; }
.pp-crumbs :deep(.el-dropdown) { min-width: 0; }
.pp-current-trigger {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  min-height: 36px;
  padding: 0 3px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
}
.pp-current-trigger .el-icon { flex: none; font-size: 11px; }
.pp-divider { flex: none; color: var(--text-faint); font-size: 11px; }
.pp-current { color: var(--text-primary); font-weight: 650; }
.pp-ancestor { max-width: min(320px, 68vw); }
.pp-input {
  width: 0;
  flex: 1;
  min-height: 36px;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: 16px;
}
.pp-crumb:hover, .pp-current-trigger:hover, .pp-item:hover { color: var(--text-primary); background: var(--bg-glass-subtle); }
.pp-crumb:focus-visible, .pp-current-trigger:focus-visible, .pp-item:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }

.pp-list {
  min-height: 200px;
  max-height: min(49dvh, 460px);
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
.pp-error { display: flex; align-items: baseline; gap: 8px; padding: 10px; color: var(--danger); overflow-wrap: anywhere; }
.pp-error .el-icon { flex: none; }
.pp-state .is-loading { animation: pp-spin 1s linear infinite; color: var(--accent); }
@keyframes pp-spin { to { transform: rotate(360deg); } }

.pp-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  background: transparent;
  text-align: start;
  font: inherit;
  padding: 8px 10px;
  border-radius: 9px;
  cursor: pointer;
  transition: background-color var(--motion-fast) var(--ease-emphasized);
}
.pp-item + .pp-item { margin-top: 1px; }
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
  color: var(--text-muted);
  background: var(--bg-glass-subtle);
}
.pp-check, .pp-next {
  flex-shrink: 0;
  font-size: 15px;
  color: var(--accent);
}
.pp-next { color: var(--text-faint); }

.pp-footer {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
}
.pp-select-dir { grid-column: 1; justify-self: start; }
.pp-sel-status {
  grid-column: 1 / -1;
  text-align: start;
  font-size: 12.5px;
  color: var(--accent);
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pp-footer-main {
  grid-column: 2;
  display: flex;
  gap: 8px;
}
.pp-footer-main :deep(.el-button + .el-button) { margin: 0; }

@media (max-width: 768px) {
  .pp-crumb, .pp-current-trigger { min-height: var(--tap-target); }
  .pp-item { min-height: var(--tap-target); }
  .pp-item-name { font-size: 14px; }
  .pp-list { min-height: min(180px, 27dvh); max-height: 45dvh; }
  .pp-state { min-height: min(180px, 27dvh); }
  .pp-footer :deep(.el-button) { min-height: var(--tap-target); }
}
@media (prefers-reduced-motion: reduce) {
  .pp-item { transition: none; }
}
</style>
