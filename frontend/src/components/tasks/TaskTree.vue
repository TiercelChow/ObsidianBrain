<template>
  <div class="task-tree" role="tree" aria-label="任务拆解">
    <div
      v-for="item in flattened"
      :key="item.node.id"
      class="tree-row"
      :class="{
        selected: item.node.id === selectedId,
        closed: isClosed(item.node.status),
        dragging: item.node.id === draggingId,
      }"
      :style="{ '--tree-depth': item.depth }"
      role="treeitem"
      :aria-level="item.depth + 1"
      :aria-expanded="item.hasChildren ? expanded.has(item.node.id) : undefined"
      :draggable="item.node.role === 'subtask'"
      @click="$emit('select', item.node.id)"
      @dragstart="startDrag(item.node.id)"
      @dragend="draggingId = null"
      @dragover.prevent
      @drop.prevent="dropOn(item.node.id)"
    >
      <button
        v-if="item.hasChildren"
        type="button"
        class="disclosure"
        :aria-label="expanded.has(item.node.id) ? '收起子任务' : '展开子任务'"
        @click.stop="toggle(item.node.id)"
      >
        <span :class="{ expanded: expanded.has(item.node.id) }">›</span>
      </button>
      <span v-else class="disclosure-spacer"></span>

      <button
        type="button"
        class="status-orb"
        :class="`status-${item.node.status}`"
        :aria-label="`${item.node.title}，${statusLabel(item.node.status)}`"
        @click.stop="$emit('status', item.node)"
      >
        <span v-if="item.node.status === 'completed'">✓</span>
        <span v-else-if="item.node.status === 'cancelled'">—</span>
      </button>

      <div class="tree-copy">
        <div class="tree-title">{{ item.node.title }}</div>
        <div class="tree-meta">
          <span class="importance" :class="`importance-${item.node.importance}`">
            {{ importanceLabel(item.node.importance) }}
          </span>
          <span class="tree-dates">
            <span class="dates-full">{{ item.node.start_date }} – {{ item.node.end_date }}</span>
            <span class="dates-compact">{{ formatTaskDateRangeCompact(item.node.start_date, item.node.end_date) }}</span>
          </span>
        </div>
      </div>

      <div class="tree-actions">
        <button type="button" title="添加进展" @click.stop="$emit('progress', item.node)">进展</button>
        <button type="button" title="添加子任务" @click.stop="$emit('add', item.node)">＋</button>
        <button
          v-if="item.node.role === 'subtask'"
          type="button"
          title="移动任务"
          @click.stop="$emit('move', item.node)"
        >移动</button>
      </div>
    </div>

    <div v-if="flattened.length === 0" class="tree-empty">还没有子任务</div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TaskImportance, TaskNode, TaskStatus } from '@/api/tasks'
import { flattenVisibleSubtasks } from '@/utils/taskHierarchy'
import { formatTaskDateRangeCompact } from '@/utils/taskDates'

const props = defineProps<{
  tasks: TaskNode[]
  selectedId?: string | null
}>()

const emit = defineEmits<{
  select: [id: string]
  add: [task: TaskNode]
  progress: [task: TaskNode]
  status: [task: TaskNode]
  move: [task: TaskNode]
  reorder: [taskId: string, parentId: string]
}>()

const expanded = ref(new Set<string>())
const draggingId = ref<string | null>(null)

watch(
  () => props.tasks,
  (tasks) => {
    const next = new Set(expanded.value)
    for (const task of tasks) {
      if (task.role === 'root' || tasks.some((candidate) => candidate.parent_id === task.id)) {
        next.add(task.id)
      }
    }
    expanded.value = next
  },
  { immediate: true },
)

const flattened = computed(() => flattenVisibleSubtasks(props.tasks, expanded.value))

function toggle(id: string) {
  const next = new Set(expanded.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expanded.value = next
}

function startDrag(id: string) {
  draggingId.value = id
}

function dropOn(parentId: string) {
  const taskId = draggingId.value
  draggingId.value = null
  if (!taskId || taskId === parentId) return
  emit('reorder', taskId, parentId)
}

function isClosed(status: TaskStatus) {
  return status === 'completed' || status === 'cancelled'
}

function statusLabel(status: TaskStatus) {
  return ({
    open: '待处理',
    planned: '已计划',
    in_progress: '进行中',
    blocked: '受阻',
    completed: '已完成',
    cancelled: '已取消',
  } satisfies Record<TaskStatus, string>)[status]
}

function importanceLabel(importance: TaskImportance) {
  return ({ low: '低', normal: '普通', high: '重要', urgent: '紧急' } satisfies Record<TaskImportance, string>)[importance]
}
</script>

<style scoped>
.task-tree { display: grid; gap: 6px; }
.tree-row {
  --indent: calc(var(--tree-depth) * 22px);
  min-height: 54px;
  display: grid;
  grid-template-columns: 26px 28px minmax(0, 1fr) auto;
  align-items: center;
  gap: 7px;
  padding: 7px 8px 7px calc(8px + var(--indent));
  border: 1px solid transparent;
  border-radius: 14px;
  transition: background var(--motion-fast) var(--ease-emphasized),
              border-color var(--motion-fast) var(--ease-emphasized),
              transform var(--motion-instant) var(--ease-emphasized),
              opacity var(--motion-fast) var(--ease-emphasized);
}
.tree-row:hover, .tree-row.selected { background: var(--bg-glass); border-color: var(--border-subtle); }
.tree-row:active { transform: scale(.995); }
.tree-row.dragging { opacity: .45; }
.tree-row.closed .tree-title { color: var(--text-faint); text-decoration: line-through; }
.disclosure, .status-orb, .tree-actions button {
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}
.disclosure { width: 26px; height: 36px; border-radius: 10px; font-size: 25px; line-height: 1; }
.disclosure:hover { background: var(--bg-glass-strong); }
.disclosure span { display: inline-block; transform: rotate(0); transition: transform var(--motion-normal) var(--ease-spring-gentle); }
.disclosure span.expanded { transform: rotate(90deg); }
.disclosure-spacer { width: 26px; }
.status-orb {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1.5px solid var(--border-subtle);
  display: grid;
  place-items: center;
  font-size: 13px;
  transition: transform var(--motion-fast) var(--ease-spring), background var(--motion-fast) ease;
}
.status-orb:hover { transform: scale(1.08); }
.status-completed { background: color-mix(in srgb, #34c759 17%, transparent); border-color: #34c759; color: #248a3d; }
.status-cancelled { background: color-mix(in srgb, var(--text-faint) 12%, transparent); }
.status-in_progress { border-color: var(--accent); box-shadow: inset 0 0 0 4px color-mix(in srgb, var(--accent) 15%, transparent); }
.status-blocked { border-color: #ff9500; }
.tree-copy { min-width: 0; }
.tree-title { color: var(--text-primary); font-size: 15px; font-weight: 580; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-meta { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 3px; color: var(--text-faint); font-size: 12px; }
.importance { flex: none; font-weight: 600; }
/* Both range renderings exist; the media queries below pick one so the meta
   line stays single-row on phones without shipping a resize listener. */
.tree-dates { min-width: 0; }
.dates-compact { display: none; }
.importance-high { color: #ff9500; }
.importance-urgent { color: #ff3b30; }
.tree-actions { display: flex; opacity: 0; transform: translateX(4px); transition: opacity var(--motion-fast) ease, transform var(--motion-fast) ease; }
.tree-row:hover .tree-actions, .tree-row:focus-within .tree-actions { opacity: 1; transform: none; }
.tree-actions button { min-height: 36px; padding: 0 8px; border-radius: 9px; font-size: 13px; }
.tree-actions button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }
.tree-empty { padding: 28px; text-align: center; color: var(--text-faint); font-size: 14px; }

@media (max-width: 768px) {
  .tree-row { --indent: calc(var(--tree-depth) * 15px); grid-template-columns: 22px 30px minmax(0, 1fr) auto; padding-left: calc(3px + var(--indent)); }
  .disclosure, .disclosure-spacer { width: 22px; }
  .tree-actions { opacity: 1; transform: none; }
  .tree-actions button { min-width: 44px; min-height: 44px; }
  .tree-actions button:first-child { display: none; }
  .dates-full { display: none; }
  .dates-compact { display: inline; }
}

@media (prefers-reduced-motion: reduce) {
  .tree-row, .disclosure span, .status-orb, .tree-actions { transition-duration: 1ms !important; }
}
</style>
