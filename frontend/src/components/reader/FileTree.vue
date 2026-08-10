<template>
  <div class="file-tree">
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
import { ref, watch } from 'vue'
import { CaretBottom, CaretRight, Folder, FolderOpened, Document, Files } from '@element-plus/icons-vue'
import type { DirEntry } from '@/api/reader'

const props = withDefaults(
  defineProps<{ entries: DirEntry[]; activePath?: string; level?: number }>(),
  { activePath: '', level: 0 },
)
const emit = defineEmits<{ select: [path: string] }>()

// Expand top-level directories by default for a useful initial view.
const expanded = ref<Set<string>>(new Set())
if (props.level === 0) {
  props.entries.filter((e) => e.is_dir).forEach((e) => expanded.value.add(e.path))
}

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
watch(() => props.activePath, (p) => { if (p) expandPathToActive(props.entries, p) }, { immediate: true })

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
</style>
