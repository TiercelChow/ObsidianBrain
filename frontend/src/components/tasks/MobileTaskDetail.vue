<template>
  <section class="mobile-task-detail" :class="{ 'is-child': child, 'is-read-only-short': node.kind === 'short' && isTaskClosed(node) }" aria-label="任务详情">
    <header class="mobile-task-detail-nav" v-glass-lens="'css'" data-glass="scroll-edge" :class="{ 'header-scrolled': scrolled }">
      <UiAction :icon="ArrowLeft" :label="child ? '返回上一级' : '返回任务列表'" @click="$emit('back')" />
      <h1 class="mobile-detail-page-title">任务详情</h1>
      <UiAction v-if="child" :icon="Close" label="关闭子任务详情" @click="$emit('close')" />
    </header>
    <div class="mobile-task-detail-scroll" @scroll="onScroll">
      <div class="mobile-task-hero">
        <p v-if="child && parentTitle" class="mobile-parent-path" :title="parentTitle">{{ parentTitle }}</p>
        <div class="mobile-detail-title-row">
          <h2>{{ node.title }}</h2>
          <UiAction :icon="EditPen" label="编辑任务" @click="$emit('edit', node)" />
        </div>
        <div class="mobile-detail-meta"><TaskStatusMark :status="node.status" /><span>{{ taskStatusLabel(node.status) }}</span><span class="mobile-detail-kind">{{ child ? '子任务' : node.kind === 'long' ? '长期任务' : '短期待办' }}</span><span v-if="node.importance !== 'normal'" class="task-pill" :class="`importance-${node.importance}`">{{ taskImportanceLabel(node.importance) }}</span></div>
        <div class="mobile-detail-dates"><span>{{ formatTaskDateRange(node.start_date, node.end_date) }}</span><strong v-if="!isTaskClosed(node)" :class="taskTiming(node, today).tone">{{ taskTiming(node, today).label }}</strong></div>
        <p v-if="node.description" class="mobile-detail-description">{{ node.description }}</p>
        <div v-if="!child && node.kind === 'long'" class="mobile-detail-progress" :aria-label="`完成度 ${progressPercent}%，已完成 ${completedLeaves} / ${effectiveLeaves} 项`">
          <span><strong>{{ progressPercent }}<small>%</small></strong><small>{{ completedLeaves }} / {{ effectiveLeaves }} 项完成</small></span>
          <span class="mobile-progress-track" aria-hidden="true"><i :style="{ transform: `scaleX(${progressPercent / 100})` }"></i></span>
        </div>
      </div>
      <MotionSwap :view-key="node.kind === 'short' ? 'progress' : section" :direction="section === 'progress' ? 1 : -1" horizontal>
        <div v-if="node.kind === 'long' && section === 'breakdown'" class="mobile-child-rows" aria-label="直属子任务">
          <button v-for="task in children" :key="task.id" type="button" class="mobile-child-row" :aria-label="`${task.title}，${taskStatusLabel(task.status)}`" @click="$emit('select', task.id)">
            <TaskStatusMark :status="task.status" />
            <span class="mobile-child-copy"><strong>{{ task.title }}</strong><small :class="{ danger: !isTaskClosed(task) && taskTiming(task, today).tone === 'danger' }">{{ isTaskClosed(task) ? taskStatusLabel(task.status) : taskTiming(task, today).label }}<template v-if="childCounts.get(task.id)"> · {{ childCounts.get(task.id) }} 项子任务</template></small></span>
            <el-icon class="ui-chevron" aria-hidden="true"><ArrowRight /></el-icon>
          </button>
          <p v-if="!children.length" class="mobile-detail-empty">{{ isTaskClosed(node) ? '没有子任务。' : '把目标拆成小步骤，点击下方 + 开始。' }}</p>
        </div>
        <MobileTaskActivity v-else :key="node.id" :entries="entries" :attributed="!child && node.kind === 'long'" :audit-only="node.kind === 'short'" @inspect="$emit('inspect', $event)" @select="$emit('select', $event)" />
      </MotionSwap>
    </div>
    <footer class="mobile-detail-context" aria-label="详情栏目与新增" v-glass-lens="'css'" data-glass="scroll-edge" data-glass-edge="bottom" :class="{ 'is-empty': node.kind === 'short' && isTaskClosed(node) }">
      <template v-if="node.kind === 'long'">
      <div class="mobile-detail-tabs" data-glass="inset" role="tablist" aria-label="任务详情栏目">
        <span class="mobile-detail-tab-indicator" data-glass-indicator :class="section" aria-hidden="true"></span>
        <button type="button" role="tab" :aria-selected="section === 'breakdown'" :class="{ active: section === 'breakdown' }" @click="$emit('update:section', 'breakdown')">子任务 <small>{{ children.length }}</small></button>
        <button type="button" role="tab" :aria-selected="section === 'progress'" :class="{ active: section === 'progress' }" @click="$emit('update:section', 'progress')">进展 <small>{{ progressCount }}</small></button>
      </div>
      <UiAction v-if="!isTaskClosed(node)" :icon="Plus" :label="section === 'breakdown' ? '添加子任务' : '记录进展'" class="mobile-context-create" @click="section === 'breakdown' ? $emit('add-child', node) : $emit('add-progress', node)" />
      </template>
      <button v-else-if="!isTaskClosed(node)" type="button" class="mobile-complete-task" @click="$emit('complete', node)"><el-icon aria-hidden="true"><Check /></el-icon>完成待办</button>
    </footer>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowLeft, ArrowRight, Check, Close, EditPen, Plus } from '@element-plus/icons-vue'
import type { TaskNode } from '@/api/tasks'
import type { TaskActivityEntry } from '@/utils/taskActivity'
import { taskImportanceLabel, taskStatusLabel } from '@/utils/taskActivity'
import { formatTaskDateRange } from '@/utils/taskDates'
import { directTaskChildren } from '@/utils/taskMobile'
import { isTaskClosed, taskTiming } from '@/utils/taskWorkspace'
import MotionSwap from '@/components/motion/MotionSwap.vue'
import UiAction from '@/components/motion/UiAction'
import TaskStatusMark from './TaskStatusMark.vue'
import MobileTaskActivity from './MobileTaskActivity.vue'
const props = withDefaults(defineProps<{
  node: TaskNode; tasks: TaskNode[]; entries: TaskActivityEntry[]; today: string; child?: boolean; parentTitle?: string
  section: 'breakdown' | 'progress'; progressPercent?: number; completedLeaves?: number; effectiveLeaves?: number
}>(), { progressPercent: 0, completedLeaves: 0, effectiveLeaves: 0 })
const emit = defineEmits<{
  back: []; close: []; edit: [node: TaskNode]; complete: [node: TaskNode]; select: [id: string]; inspect: [entry: TaskActivityEntry]
  'update:section': [section: 'breakdown' | 'progress']; 'add-child': [node: TaskNode]; 'add-progress': [node: TaskNode]; scroll: [event: Event]
}>()
const scrolled = ref(false)
function onScroll(event: Event) {
  scrolled.value = (event.target as HTMLElement).scrollTop > 8
  emit('scroll', event)
}
const children = computed(() => directTaskChildren(props.tasks, props.node.id))
const childCounts = computed(() => {
  const counts = new Map<string, number>()
  for (const task of props.tasks) if (task.parent_id) counts.set(task.parent_id, (counts.get(task.parent_id) || 0) + 1)
  return counts
})
const progressCount = computed(() => props.entries.filter(item => item.type === 'progress').length)
</script>

<style scoped>
.mobile-task-detail { --task-detail-header-height:52px; --task-detail-footer-height:calc(64px + max(12px,var(--safe-bottom))); position:relative; isolation:isolate; display:flex; flex-direction:column; min-height:0; height:100%; color:var(--text-primary); }
.mobile-task-detail-nav { position:absolute; top:0; left:0; right:0; z-index:2; display:grid; grid-template-columns:44px minmax(0,1fr) 44px; align-items:center; height:var(--task-detail-header-height); }
.mobile-detail-page-title { min-width:0; margin:0; text-align:center; font-size:19px; font-weight:700; letter-spacing:-.2px; line-height:1.3; }
.mobile-parent-path { min-width:0; margin:0 0 8px; color:var(--text-muted); font-size:12px; text-overflow:ellipsis; overflow:hidden; white-space:nowrap; }
.mobile-task-detail-scroll { flex:1; min-height:0; overflow-y:auto; overscroll-behavior:contain; padding:calc(var(--task-detail-header-height) + 6px) 4px calc(var(--task-detail-footer-height) + var(--glass-scroll-edge-fade) + 12px); scroll-padding-block:calc(var(--task-detail-header-height) + var(--glass-scroll-edge-fade)) calc(var(--task-detail-footer-height) + var(--glass-scroll-edge-fade)); scrollbar-width:none; }
.mobile-task-detail-scroll::-webkit-scrollbar { display:none; }
.mobile-task-hero { padding:2px 0 22px; }
.mobile-detail-title-row { display:flex; align-items:flex-start; gap:10px; margin:4px 0 10px; }
.mobile-detail-title-row h2 { flex:1; min-width:0; margin:5px 0 0; font-size:24px; font-weight:680; line-height:1.4; letter-spacing:-.025em; overflow-wrap:anywhere; }
.mobile-detail-title-row > :deep(.ui-action) { flex:none; }
.mobile-detail-meta { display:flex; align-items:center; flex-wrap:wrap; gap:7px; margin-bottom:12px; color:var(--text-secondary); font-size:12px; }
.mobile-detail-kind { color:var(--text-muted); }
.mobile-detail-meta .task-pill { font-size:10px; }
.mobile-detail-dates { display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:6px 12px; color:var(--text-muted); font-size:12px; line-height:1.6; font-variant-numeric:tabular-nums; }
.mobile-detail-dates strong { font-weight:500; font-size:11px; }
.danger { color:var(--danger); }
.accent { color:var(--accent); }
.mobile-detail-description { white-space:pre-wrap; overflow-wrap:anywhere; font-size:14px; line-height:1.8; color:var(--text-secondary); margin:16px 0 0; }
.mobile-detail-progress { display:grid; gap:8px; margin-top:18px; }
.mobile-detail-progress > span:first-child { display:flex; align-items:baseline; justify-content:space-between; gap:12px; }
.mobile-detail-progress strong { font-size:26px; font-weight:600; letter-spacing:-.05em; font-variant-numeric:tabular-nums; }
.mobile-detail-progress small { color:var(--text-muted); font-size:11px; font-weight:500; }
.mobile-detail-progress strong small { margin-left:2px; font-size:13px; }
.mobile-progress-track { height:4px; overflow:hidden; border-radius:3px; background:var(--border-faint); }
.mobile-progress-track i { display:block; height:100%; background:var(--accent); transform-origin:left; transition:transform 240ms var(--ease-emphasized); }
.mobile-child-row { display:flex; align-items:center; gap:12px; min-height:74px; width:100%; padding:12px 0; border:0; border-bottom:1px solid var(--border-faint); color:var(--text-primary); background:transparent; font:inherit; text-align:left; cursor:pointer; transition:transform 160ms var(--ease-emphasized); }
.mobile-child-row:active { transform:scale(.985); }
.mobile-child-copy { display:grid; flex:1; min-width:0; gap:6px; }
.mobile-child-row strong { overflow-wrap:anywhere; font-size:15px; font-weight:580; line-height:1.5; }
.mobile-child-row small { font-size:12px; line-height:1.5; color:var(--text-muted); }
.mobile-child-row small.danger { color:var(--danger); }
.mobile-child-row .el-icon { flex:none; color:var(--text-faint); font-size:14px; }
.mobile-detail-empty { margin:6px 0 20px; font-size:13px; line-height:1.8; color:var(--text-muted); }
.mobile-detail-context { position:absolute; bottom:0; left:0; right:0; z-index:2; display:flex; align-items:center; gap:10px; min-height:var(--task-detail-footer-height); padding:12px 0 max(12px,var(--safe-bottom)); }
.is-read-only-short { --task-detail-footer-height:max(12px,var(--safe-bottom)); }
.mobile-detail-context.is-empty { padding:0; pointer-events:none; }
.mobile-detail-tabs { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); position:relative; flex:1; padding:4px; border-radius:18px; isolation:isolate; }
.mobile-detail-tab-indicator { position:absolute; top:4px; bottom:4px; left:4px; width:calc(50% - 4px); border-radius:14px; z-index:-1; transition:transform 280ms var(--ease-spring-gentle); }
.mobile-detail-tab-indicator.progress { transform:translateX(100%); }
.mobile-detail-tabs button { display:flex; align-items:center; justify-content:center; gap:6px; min-height:44px; padding:7px; border:0; border-radius:14px; background:transparent; color:var(--text-muted); font:inherit; font-size:14px; cursor:pointer; }
.mobile-detail-tabs button.active { color:var(--text-primary); font-weight:650; }
.mobile-detail-tabs small { font-size:11px; color:var(--text-faint); font-variant-numeric:tabular-nums; }
.mobile-context-create { flex:none; width:52px; height:52px; }
.mobile-complete-task { display:flex; align-items:center; justify-content:center; gap:8px; min-height:48px; width:100%; border:0; border-radius:16px; background:var(--bg-hover); color:var(--text-primary); font:inherit; font-size:15px; font-weight:600; cursor:pointer; }
.is-child { height:min(80dvh,720px); max-height:inherit; padding:22px 20px 0; border-radius:24px 24px 0 0; background:var(--bg-glass-strong); }
.is-child .mobile-task-detail-nav { top:22px; left:20px; right:20px; }
.is-child .mobile-detail-context { left:20px; right:20px; }
button:focus-visible { outline:2px solid var(--accent); outline-offset:-2px; border-radius:12px; }
@media(prefers-reduced-motion:reduce) { .mobile-child-row,.mobile-detail-tab-indicator,.mobile-progress-track i { transition:none; } .mobile-child-row:active { transform:none; } }
</style>
