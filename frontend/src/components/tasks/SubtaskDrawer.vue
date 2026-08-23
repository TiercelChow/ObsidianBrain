<template>
  <div class="drawer-backdrop" :class="{ open: !!node }" @click="emit('close')"></div>
  <div class="subtask-drawer-slot" :class="{ open: !!node }">
    <aside v-if="node" class="subtask-drawer" role="dialog" aria-label="子任务详情">
      <header class="drawer-header">
        <button v-if="parent" type="button" class="drawer-back" :title="`返回 ${parent.title}`" @click="emit('select', parent.id)">
          <span class="drawer-back-arrow" aria-hidden="true">‹</span>
          <span class="drawer-back-title">{{ parent.title }}</span>
        </button>
        <div class="drawer-row">
          <div class="drawer-title-group">
            <h3>{{ node.title }}</h3>
          </div>
          <div class="drawer-corner">
            <button type="button" class="corner-button" aria-label="编辑任务" title="编辑" @click="emit('edit', node)"><el-icon><EditPen /></el-icon></button>
            <button type="button" class="corner-button" aria-label="更改状态" title="更改状态" @click="emit('status', node)"><el-icon><Refresh /></el-icon></button>
            <button type="button" class="corner-button" aria-label="移动任务" title="移动" @click="emit('move', node)"><el-icon><Rank /></el-icon></button>
          </div>
        </div>
      </header>

      <div class="drawer-scroll">
        <p v-if="node.description" class="drawer-description">{{ node.description }}</p>

        <div class="drawer-facts">
          <span class="task-pill" :class="`status-${node.status}`">{{ taskStatusLabel(node.status) }}</span>
          <span class="task-pill" :class="`importance-${node.importance}`">{{ taskImportanceLabel(node.importance) }}</span>
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
        </div>

        <section class="drawer-section">
          <div class="drawer-section-heading">
            <div><span>子任务</span><strong>{{ children.length }} 个</strong></div>
            <button type="button" class="drawer-add" aria-label="添加子任务" title="添加子任务" @click="emit('add', node)">＋</button>
          </div>
          <div v-if="children.length" class="drawer-children">
            <button
              v-for="child in children"
              :key="child.id"
              type="button"
              class="drawer-child"
              @click="emit('select', child.id)"
            >
              <span class="status-dot" :class="`status-${child.status}`"></span>
              <strong>{{ child.title }}</strong>
              <span class="task-pill" :class="`importance-${child.importance}`">{{ taskImportanceLabel(child.importance) }}</span>
              <span class="drawer-child-chevron" aria-hidden="true">›</span>
            </button>
          </div>
          <p v-else class="drawer-section-empty">还没有子任务</p>
        </section>

        <section ref="activityRef" class="drawer-section">
          <div class="drawer-section-heading">
            <div><span>进展与记录</span><strong>{{ activity.length }} 条</strong></div>
            <button type="button" class="drawer-add" aria-label="记录进展" title="记录进展" @click="emit('progress', node)">＋</button>
          </div>
          <div v-if="activity.length" class="drawer-activity">
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item">
              <span class="activity-dot" :class="item.type"></span>
              <div class="drawer-activity-copy">
                <div class="drawer-activity-head">
                  <strong><span class="task-pill" :class="`type-${item.type}`">{{ item.title }}</span></strong>
                  <time>{{ formatTimestamp(item.time) }}</time>
                </div>
                <p v-if="item.detail" class="drawer-activity-detail">{{ item.detail }}</p>
                <p v-if="item.note" class="drawer-activity-note">{{ item.note }}</p>
              </div>
            </article>
          </div>
          <p v-else class="drawer-section-empty">暂无进展记录</p>
        </section>
      </div>

      <button ref="collapseRef" type="button" class="drawer-collapse" aria-label="收起子任务详情" title="收起" @click="emit('close')">‹</button>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { EditPen, Rank, Refresh } from '@element-plus/icons-vue'
import type { TaskNode } from '@/api/tasks'
import { taskImportanceLabel, taskStatusLabel, type TaskActivityEntry } from '@/utils/taskActivity'
import { formatTimestamp } from '@/utils/taskDates'

const props = defineProps<{
  node: TaskNode | null
  activity: TaskActivityEntry[]
  children: TaskNode[]
  parent: TaskNode | null
}>()

const emit = defineEmits<{
  close: []
  select: [taskId: string]
  progress: [task: TaskNode]
  add: [task: TaskNode]
  status: [task: TaskNode]
  edit: [task: TaskNode]
  move: [task: TaskNode]
}>()

const activityRef = ref<HTMLElement | null>(null)
const collapseRef = ref<HTMLButtonElement | null>(null)

// Move focus into the drawer when it opens so keyboard users land on the collapse affordance.
watch(() => props.node?.id, async (id) => {
  await nextTick()
  if (id) collapseRef.value?.focus()
})

function revealActivity() {
  activityRef.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ revealActivity })
</script>

<style scoped>
.drawer-backdrop { display: none; }

/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone.
   The 24px padding strip left of the drawer hosts the collapse handle. */
.subtask-drawer-slot {
  flex: 0 0 auto;
  width: 0;
  padding-left: 0;
  overflow: hidden;
  transition: width var(--motion-normal) var(--ease-emphasized), padding-left var(--motion-normal) var(--ease-emphasized);
}
.subtask-drawer-slot.open { width: calc(min(400px, 30vw) + 24px); padding-left: 24px; }
.subtask-drawer {
  position: relative;
  width: min(400px, 30vw);
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-glass);
  border-radius: 22px;
  background: var(--bg-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
}

.drawer-header { display: flex; flex-direction: column; gap: 10px; padding: 16px 18px 0; }
.drawer-back {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 570;
  cursor: pointer;
}
.drawer-back:hover { color: var(--accent); }
.drawer-back-arrow { flex: none; font-size: 14px; line-height: 1; }
.drawer-back-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.drawer-row { display: flex; align-items: flex-start; gap: 10px; }
.drawer-title-group { min-width: 0; flex: 1; }
.drawer-title-group h3 { margin: 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }
.drawer-corner { flex: none; display: flex; gap: 5px; }
.corner-button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: 11px;
  background: var(--bg-glass);
  color: var(--text-muted);
  font-size: 15px;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease;
}
.corner-button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

.drawer-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 0 18px max(18px, env(safe-area-inset-bottom)); }
.drawer-description { margin: 16px 0 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.drawer-facts { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 18px 0 0; }
.drawer-facts .task-pill { flex: none; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts div span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }

.drawer-section { margin: 20px 0 0; padding: 18px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.drawer-section-heading { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 12px; }
.drawer-section-heading > div { display: flex; align-items: baseline; gap: 8px; }
.drawer-section-heading span { font-size: 12px; font-weight: 650; }
.drawer-section-heading strong { color: var(--text-faint); font-size: 11px; font-weight: 600; }
.drawer-add {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-glass);
  color: var(--accent);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
  transition: background var(--motion-fast) ease;
}
.drawer-add:hover { background: var(--bg-glass-strong); }
.drawer-section-empty { margin: 2px 0 0; color: var(--text-faint); font-size: 12px; }

.drawer-children { display: grid; gap: 6px; }
.drawer-child {
  display: grid;
  grid-template-columns: 10px minmax(0, 1fr) auto 16px;
  align-items: center;
  gap: 9px;
  min-height: 42px;
  padding: 8px 11px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: color-mix(in srgb, var(--text-primary) 3%, transparent);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  transition: background var(--motion-fast) ease, border-color var(--motion-fast) ease;
}
.drawer-child:hover { background: var(--bg-glass-strong); border-color: var(--border-subtle); }
.drawer-child strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 570; }
.drawer-child-chevron { color: var(--text-faint); font-size: 15px; text-align: right; }

.status-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: #8e8e93; }
.status-dot.status-in_progress { background: var(--accent); }
.status-dot.status-blocked { background: #ff9500; }
.status-dot.status-completed { background: #34c759; }
.status-dot.status-cancelled { background: #8e8e93; }

.drawer-activity { display: grid; gap: 0; }
/* Dividers breathe on both sides: the next entry carries the top border plus its own padding. */
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 12px 0; }
.drawer-activity-item + .drawer-activity-item { border-top: 1px solid var(--border-subtle); }
.drawer-activity-copy { min-width: 0; }
.drawer-activity-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.drawer-activity-head strong { min-width: 0; display: flex; align-items: center; }
.drawer-activity-head time { flex: none; padding-top: 1px; text-align: right; color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }
.drawer-activity-item .activity-dot { width: 9px; height: 9px; margin-top: 5px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.drawer-activity-item .activity-dot.audit { border-color: var(--text-faint); }
.drawer-activity-detail { margin: 6px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.5; }
.drawer-activity-note { margin: 5px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }

/* Collapse handle: rides the drawer's left edge, vertically centered. */
.drawer-collapse {
  position: absolute;
  left: -24px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 64px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-glass);
  border-right: 0;
  border-radius: 12px 0 0 12px;
  background: var(--bg-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  color: var(--text-muted);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease;
}
.drawer-collapse:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

/* Below 1150px: no room to push — overlay from the right with a backdrop. */
@media (max-width: 1149px) {
  .drawer-backdrop { display: block; position: fixed; inset: 0; z-index: 2300; background: color-mix(in srgb, #0f121a 32%, transparent); opacity: 0; pointer-events: none; transition: opacity var(--motion-normal) ease; }
  .drawer-backdrop.open { opacity: 1; pointer-events: auto; }
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; padding: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
  .subtask-drawer-slot.open { transform: translateX(0); }
  .subtask-drawer { width: 100%; height: 100%; border-radius: 22px 0 0 22px; }
}

@media (max-width: 768px) {
  .subtask-drawer-slot { width: min(88vw, 400px); }
  .drawer-header, .drawer-scroll { padding-left: 16px; padding-right: 16px; }
  .drawer-corner .corner-button { width: 38px; height: 38px; }
  .drawer-add { width: 36px; height: 36px; }
}

@media (prefers-reduced-motion: reduce) {
  .subtask-drawer-slot, .drawer-backdrop { transition-duration: 1ms !important; }
}
</style>
