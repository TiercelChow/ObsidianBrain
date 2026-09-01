<template>
  <div ref="rootEl" class="file-tree">
    <div v-if="!entries.length" class="ft-empty">暂无文件</div>
    <div v-for="entry in entries" :key="entry.path" class="ft-node">
      <!-- Directory -->
      <div
        v-if="entry.is_dir"
        class="ft-row ft-dir"
        :style="{ paddingLeft: indent }"
        @click="toggle(entry)"
      >
        <el-icon class="ft-caret">
          <CaretBottom v-if="isExpanded(entry)" />
          <CaretRight v-else />
        </el-icon>
        <el-icon class="ft-icon"><FolderOpened v-if="isExpanded(entry)" /><Folder v-else /></el-icon>
        <span class="ft-name">{{ entry.name }}</span>
      </div>
      <!-- File -->
      <div
        v-else
        class="ft-row ft-file"
        :class="{ active: entry.path === activePath, disabled: !entry.is_markdown && !entry.is_pdf }"
        :style="{ paddingLeft: indent }"
        :title="entry.path"
        @click="onFileClick(entry)"
      >
        <span class="ft-caret-spacer"></span>
        <el-icon class="ft-icon">
          <Document v-if="entry.is_markdown" />
          <Files v-else-if="entry.is_pdf" />
          <Document v-else />
        </el-icon>
        <span class="ft-name">{{ entry.name }}</span>
      </div>
      <!-- Children (recursive) -->
      <div v-if="entry.is_dir && isExpanded(entry) && entry.children?.length" class="ft-children">
        <FileTree
          :entries="entry.children"
          :active-path="activePath"
          :level="level + 1"
          @select="$emit('select', $event)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from 'vue'
import { CaretBottom, CaretRight, Folder, FolderOpened, Document, Files } from '@element-plus/icons-vue'
import type { DirEntry } from '@/api/reader'

const props = withDefaults(
  defineProps<{ entries: DirEntry[]; activePath?: string; level?: number }>(),
  { activePath: '', level: 0 },
)
const emit = defineEmits<{ select: [path: string] }>()

// Only the directories along the path to the active file are expanded — not
// every top-level dir. Opening a folder used to expand everything, burying
// the file being read under unrelated branches; now the tree stays compact
// and just the active file's ancestors unfold.
const expanded = ref<Set<string>>(new Set())
const rootEl = ref<HTMLElement | null>(null)

// Auto-expand all ancestor directories of the active file so it's visible.
function expandPathToActive(entries: DirEntry[], active: string) {
  for (const e of entries) {
    if (e.is_dir && e.children?.length) {
      // If the active path is this dir itself or starts with it, expand.
      if (active === e.path || active.startsWith(e.path + '/')) {
        expanded.value.add(e.path)
        expandPathToActive(e.children, active)
      }
    }
  }
}

/** Nearest scrollable ancestor (the sidebar pane-scroll or the mobile drawer). */
function scrollableAncestorOf(el: HTMLElement | null): HTMLElement | null {
  let node = el?.parentElement ?? null
  while (node) {
    const style = getComputedStyle(node)
    if (node.scrollHeight > node.clientHeight && /auto|scroll/.test(style.overflowY)) {
      return node
    }
    node = node.parentElement
  }
  return null
}

/** Bring the active file row into view, centered — only if it's off-screen. */
function scrollToActive() {
  const root = rootEl.value
  if (!root) return
  const active = root.querySelector<HTMLElement>('.ft-file.active')
  if (!active) return
  const scroller = scrollableAncestorOf(active)
  if (!scroller) return
  const sc = scroller.getBoundingClientRect()
  const ac = active.getBoundingClientRect()
  const margin = 40
  // Already visible — don't jump, so navigating within the tree stays calm.
  if (ac.top >= sc.top + margin && ac.bottom <= sc.bottom - margin) return
  const delta = (ac.top + ac.height / 2) - (sc.top + sc.height / 2)
  scroller.scrollBy({ top: delta, behavior: 'smooth' })
}

function scheduleScrollToActive() {
  // nextTick lets the cascade of nested-instance expansions mount the active
  // row; rAF ensures layout has settled before measuring its position.
  nextTick(() => requestAnimationFrame(() => scrollToActive()))
}

watch(
  () => props.activePath,
  (p, old) => {
    if (!p) return
    expandPathToActive(props.entries, p)
    // Skip the immediate (pre-mount) pass — onMounted handles the initial
    // scroll once the root element exists.
    if (props.level === 0 && old !== undefined) scheduleScrollToActive()
  },
  { immediate: true },
)

onMounted(() => {
  if (props.level === 0 && props.activePath) scheduleScrollToActive()
})

defineExpose({ scrollToActive })

const indent = `${10 + props.level * 14}px`

function isExpanded(entry: DirEntry) {
  return expanded.value.has(entry.path)
}
function toggle(entry: DirEntry) {
  if (isExpanded(entry)) {
    expanded.value.delete(entry.path)
  } else {
    expanded.value.add(entry.path)
  }
}
function onFileClick(entry: DirEntry) {
  if (entry.is_markdown || entry.is_pdf) emit('select', entry.path)
}
</script>

<style scoped>
.file-tree { user-select: none; }
.ft-empty { padding: 16px 12px; font-size: 12px; color: var(--text-faint); text-align: center; }

.ft-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px 4px 0;
  margin: 1px 2px;
  border-radius: 8px;
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.12s var(--ease-out), color 0.12s var(--ease-out);
  white-space: nowrap;
  overflow: hidden;
}
.ft-row:hover { background: var(--bg-glass-subtle); color: var(--text-primary); }
.ft-dir { font-weight: 500; color: var(--text-secondary); }

.ft-file.disabled { color: var(--text-faint); cursor: default; }
.ft-file.disabled:hover { background: transparent; color: var(--text-faint); }
.ft-file.active { background: var(--accent-light); color: var(--accent); font-weight: 600; }

.ft-caret { width: 12px; font-size: 12px; color: var(--text-muted); flex-shrink: 0; }
.ft-caret-spacer { width: 12px; flex-shrink: 0; }
.ft-icon { width: 15px; font-size: 15px; color: var(--text-muted); flex-shrink: 0; }
.ft-dir:hover .ft-icon, .ft-file.active .ft-icon { color: var(--accent); }
.ft-name { overflow: hidden; text-overflow: ellipsis; }

.ft-children { animation: ft-expand var(--duration-fast) var(--ease-out); }
@keyframes ft-expand {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

@media (max-width: 768px), (pointer: coarse) {
  .ft-row {
    min-height: var(--tap-target);
    padding-top: 8px;
    padding-bottom: 8px;
    font-size: 14px;
  }
  .ft-caret { width: 18px; }
  .ft-caret-spacer { width: 18px; }
  .ft-icon { width: 18px; font-size: 17px; }
}
</style>
