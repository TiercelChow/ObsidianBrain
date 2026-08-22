<template>
  <div class="drawer-backdrop" :class="{ open: !!node }" @click="emit('close')"></div>
  <div class="subtask-drawer-slot" :class="{ open: !!node }">
    <aside v-if="node" class="subtask-drawer" role="dialog" aria-label="子任务详情">
      <header class="drawer-header">
        <button ref="closeButtonRef" type="button" aria-label="关闭子任务详情" @click="emit('close')">✕</button>
        <div class="drawer-title-group">
          <div class="drawer-kicker">
            <span class="status-dot" :class="`status-${node.status}`"></span>
            子任务 · {{ taskStatusLabel(node.status) }} · {{ taskImportanceLabel(node.importance) }}
          </div>
          <h3>{{ node.title }}</h3>
        </div>
      </header>

      <div class="drawer-scroll">
        <p v-if="node.description" class="drawer-description">{{ node.description }}</p>

        <div class="drawer-facts">
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
          <div><span>优先级</span><strong>{{ taskImportanceLabel(node.importance) }}</strong></div>
        </div>

        <div class="drawer-actions">
          <button type="button" @click="emit('progress', node)">记录进展</button>
          <button type="button" @click="emit('add', node)">添加子任务</button>
          <button type="button" @click="emit('status', node)">更改状态</button>
          <button type="button" @click="emit('edit', node)">编辑</button>
          <button type="button" @click="emit('move', node)">移动</button>
        </div>

        <section ref="activityRef" class="drawer-section">
          <div class="drawer-section-heading">
            <span>进展与记录</span>
            <strong>{{ activity.length }} 条</strong>
          </div>
          <div v-if="activity.length" class="drawer-activity">
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item">
              <span class="activity-dot" :class="item.type"></span>
              <div class="drawer-activity-copy">
                <div class="drawer-activity-head">
                  <strong>{{ item.title }}</strong>
                  <time>{{ formatTimestamp(item.time) }}</time>
                </div>
                <p v-if="item.note">{{ item.note }}</p>
              </div>
            </article>
          </div>
          <p v-else class="drawer-activity-empty">暂无进展记录</p>
        </section>
      </div>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import type { TaskNode } from '@/api/tasks'
import { taskImportanceLabel, taskStatusLabel, type TaskActivityEntry } from '@/utils/taskActivity'
import { formatTimestamp } from '@/utils/taskDates'

const props = defineProps<{
  node: TaskNode | null
  activity: TaskActivityEntry[]
}>()

const emit = defineEmits<{
  close: []
  progress: [task: TaskNode]
  add: [task: TaskNode]
  status: [task: TaskNode]
  edit: [task: TaskNode]
  move: [task: TaskNode]
}>()

const activityRef = ref<HTMLElement | null>(null)
const closeButtonRef = ref<HTMLButtonElement | null>(null)

// Move focus into the drawer when it opens so keyboard users land on the close affordance.
watch(() => props.node, async (node) => {
  await nextTick()
  if (node) closeButtonRef.value?.focus()
})

function revealActivity() {
  activityRef.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ revealActivity })
</script>

<style scoped>
.drawer-backdrop { display: none; }

/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone. */
.subtask-drawer-slot { flex: 0 0 auto; width: 0; overflow: hidden; transition: width var(--motion-normal) var(--ease-emphasized), margin var(--motion-normal) var(--ease-emphasized); }
.subtask-drawer-slot.open { width: min(400px, 30vw); margin-left: 14px; }
.subtask-drawer {
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

.drawer-header { display: flex; align-items: flex-start; gap: 12px; padding: 18px 18px 0; }
.drawer-header > button { flex: none; width: 34px; height: 34px; border: 0; border-radius: 11px; background: transparent; color: var(--text-faint); font-size: 15px; cursor: pointer; }
.drawer-header > button:hover { background: var(--bg-glass-strong); color: var(--text-primary); }
.drawer-title-group { min-width: 0; }
.drawer-kicker { display: flex; align-items: center; gap: 6px; color: var(--text-muted); font-size: 11px; font-weight: 580; }
.drawer-kicker .status-dot { flex: none; width: 7px; height: 7px; border-radius: 50%; background: #8e8e93; box-shadow: 0 0 0 4px color-mix(in srgb, #8e8e93 10%, transparent); }
.drawer-kicker .status-dot.status-in_progress { background: var(--accent); box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 11%, transparent); }
.drawer-kicker .status-dot.status-blocked { background: #ff9500; }
.drawer-kicker .status-dot.status-completed { background: #34c759; }
.drawer-kicker .status-dot.status-cancelled { background: #8e8e93; }
.drawer-title-group h3 { margin: 7px 0 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }

.drawer-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 0 18px max(18px, env(safe-area-inset-bottom)); }
.drawer-description { margin: 14px 0 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.drawer-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 16px 0 0; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
.drawer-actions { display: flex; flex-wrap: wrap; gap: 7px; margin: 16px 0 0; }
.drawer-actions button { min-height: 38px; padding: 0 11px; border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--text-secondary); font-weight: 580; cursor: pointer; }
.drawer-actions button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }

.drawer-section { margin: 18px 0 0; padding: 16px; border: 1px solid var(--border-subtle); border-radius: 17px; background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.drawer-section-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 6px; }
.drawer-section-heading span { font-size: 12px; font-weight: 650; }
.drawer-section-heading strong { color: var(--text-faint); font-size: 11px; font-weight: 600; }
.drawer-activity { display: grid; gap: 0; }
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; min-height: 56px; }
.drawer-activity-copy { min-width: 0; padding: 0 0 14px 3px; border-bottom: 1px solid var(--border-subtle); }
.drawer-activity-item:last-child .drawer-activity-copy { border-bottom: 0; }
.drawer-activity-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
.drawer-activity-head strong { min-width: 0; font-size: 12px; }
.drawer-activity-head time { flex: none; padding-top: 1px; text-align: right; color: var(--text-faint); font-size: 10px; font-variant-numeric: tabular-nums; }
.drawer-activity-item .activity-dot { width: 9px; height: 9px; margin-top: 4px; border: 2px solid var(--accent); border-radius: 50%; background: var(--bg-base); }
.drawer-activity-item .activity-dot.audit { border-color: var(--text-faint); }
.drawer-activity-item p { margin: 5px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }
.drawer-activity-empty { margin: 6px 0 0; color: var(--text-faint); font-size: 12px; }

/* Below 1150px: no room to push — overlay from the right with a backdrop. */
@media (max-width: 1149px) {
  .drawer-backdrop { display: block; position: fixed; inset: 0; z-index: 2300; background: color-mix(in srgb, #0f121a 32%, transparent); opacity: 0; pointer-events: none; transition: opacity var(--motion-normal) ease; }
  .drawer-backdrop.open { opacity: 1; pointer-events: auto; }
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
  .subtask-drawer-slot.open { transform: translateX(0); }
  .subtask-drawer { width: 100%; height: 100%; border-radius: 22px 0 0 22px; }
}

@media (max-width: 768px) {
  .subtask-drawer-slot { width: min(88vw, 400px); }
  .drawer-header, .drawer-scroll { padding-left: 16px; padding-right: 16px; }
  .drawer-actions button { min-height: 44px; }
  .drawer-facts { grid-template-columns: 1fr 1fr; }
  .drawer-facts div:last-child { grid-column: 1 / -1; }
}

@media (prefers-reduced-motion: reduce) {
  .subtask-drawer-slot, .drawer-backdrop { transition-duration: 1ms !important; }
}
</style>
