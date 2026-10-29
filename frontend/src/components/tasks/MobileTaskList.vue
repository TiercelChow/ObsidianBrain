<template>
  <section class="mobile-task-list" aria-label="任务列表">
    <div class="mobile-task-filters" aria-label="快速筛选当前列表">
      <button v-for="option in focuses" :key="option.value" type="button"
        :class="{ active: focus === option.value }" :aria-pressed="focus === option.value" @click="$emit('focus', option.value)">
        {{ option.label }}<span>{{ counts[option.value] }}</span>
      </button>
    </div>
    <div ref="scrollRef" class="mobile-task-list-scroll" @scroll="$emit('scroll', $event)" :aria-busy="loading">
      <div v-if="loading" class="mobile-task-loading" role="status" aria-label="加载任务中"><i v-for="n in 5" :key="n"></i></div>
      <div v-else-if="tasks.length" class="mobile-task-groups">
        <section v-for="group in groups" :key="group.key" :aria-label="group.label">
          <h2>{{ group.label }}<small>{{ group.tasks.length }}</small></h2>
          <TransitionGroup name="compact-task" tag="div" class="mobile-task-rows">
            <button v-for="task in group.tasks" :key="task.id" type="button" class="mobile-task-row" data-glass="subcard"
              :class="{ closed: isTaskClosed(task) }" :aria-label="`${task.title}，${taskStatusLabel(task.status)}，${taskImportanceLabel(task.importance)}，${mobileTaskMeta(task, today)}`"
              @click="$emit('open', task.id)">
              <TaskStatusMark :status="task.status" />
              <span class="mobile-task-copy">
                <span class="mobile-task-title"><strong>{{ task.title }}</strong><span v-if="task.importance === 'high' || task.importance === 'urgent'" class="mobile-priority" :class="task.importance">{{ taskImportanceLabel(task.importance) }}</span></span>
                <span class="mobile-task-meta-row">
                  <span class="mobile-task-meta" :class="{ overdue: !isTaskClosed(task) && taskTiming(task, today).tone === 'danger' }">{{ mobileTaskMeta(task, today) }}</span>
                  <span v-if="task.kind === 'long'" class="mobile-task-percent" :aria-label="`完成度 ${task.progress_percent}%`">{{ task.progress_percent }}<small>%</small></span>
                </span>
                <span v-if="task.kind === 'long' && task.progress_percent > 0" class="mobile-card-progress" aria-hidden="true"><i :style="{ transform: `scaleX(${task.progress_percent / 100})` }"></i></span>
              </span>
              <el-icon class="ui-chevron mobile-row-chevron" aria-hidden="true"><ArrowRight /></el-icon>
            </button>
          </TransitionGroup>
        </section>
      </div>
      <div v-else class="mobile-task-empty" role="status">
        <el-icon aria-hidden="true"><CircleCheck /></el-icon>
        <strong>{{ filtered ? '没有符合条件的任务' : '从一件小事开始' }}</strong>
        <p>{{ filtered ? '调整搜索或筛选条件。' : '点击下方 +，记下一件待办或长期目标。' }}</p>
        <button v-if="filtered" type="button" @click="$emit('reset')">重置筛选</button>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ArrowRight, CircleCheck } from '@element-plus/icons-vue'
import type { TaskSummary } from '@/api/tasks'
import TaskStatusMark from './TaskStatusMark.vue'
import { groupWorkspaceTasks, isTaskClosed, taskTiming, type TaskFocus } from '@/utils/taskWorkspace'
import { mobileTaskMeta } from '@/utils/taskMobile'
import { taskImportanceLabel, taskStatusLabel } from '@/utils/taskActivity'
const props = withDefaults(defineProps<{ tasks: TaskSummary[]; counts: Record<TaskFocus, number>; focus: TaskFocus; today: string; loading: boolean; filtered: boolean; scrollTop?: number; restoreReady?: boolean }>(), { scrollTop: 0, restoreReady: true })
defineEmits<{ open: [id: string]; focus: [focus: TaskFocus]; reset: []; scroll: [event: Event] }>()
const focuses: { value: TaskFocus; label: string }[] = [{ value: 'all', label: '全部' }, { value: 'overdue', label: '逾期' }, { value: 'today', label: '今日' }, { value: 'blocked', label: '受阻' }]
const groups = computed(() => groupWorkspaceTasks(props.tasks, props.today))
const scrollRef = ref<HTMLElement | null>(null)
// The root route restores the Dock reserve after leaving a focused detail.
// Restoring before that layout update would clamp the saved position too early.
function restoreScroll() { if (scrollRef.value && !props.loading && props.restoreReady) scrollRef.value.scrollTop = props.scrollTop }
onMounted(restoreScroll)
watch([() => props.loading, () => props.restoreReady], restoreScroll, { flush: 'post' })
</script>

<style scoped>
.mobile-task-list { display:flex; flex-direction:column; height:100%; min-height:0; }
.mobile-task-filters { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:4px; flex:none; padding-bottom:6px; }
.mobile-task-filters button { display:flex; align-items:center; justify-content:center; gap:5px; min-height:44px; padding:5px 4px; border:0; border-radius:14px; background:transparent; color:var(--text-muted); font:inherit; font-size:14px; cursor:pointer; }
.mobile-task-filters button.active { background:var(--bg-hover); color:var(--text-primary); font-weight:650; box-shadow:var(--inset-highlight); }
.mobile-task-filters span { font-size:11px; font-variant-numeric:tabular-nums; opacity:.72; }
.mobile-task-list-scroll { min-height:0; flex:1; overflow-y:auto; overscroll-behavior:contain; overflow-anchor:none; padding:0 2px 24px; scrollbar-width:none; }
.mobile-task-list-scroll::-webkit-scrollbar { display:none; }
.mobile-task-groups { display:grid; gap:14px; }
h2 { display:flex; align-items:center; gap:8px; margin:8px 8px 8px; font-size:12px; line-height:1.5; font-weight:600; color:var(--text-muted); }
h2 small { font-size:11px; font-weight:500; color:var(--text-faint); }
.mobile-task-rows { position:relative; display:grid; gap:10px; }
.mobile-task-row { display:flex; align-items:center; gap:11px; width:100%; min-height:90px; padding:14px 13px; border:1px solid var(--border-subtle); border-radius:18px; color:var(--text-primary); font:inherit; text-align:left; cursor:pointer; transition:transform 180ms var(--ease-emphasized),opacity 160ms ease; }
.mobile-task-row:active { transform:scale(.98); opacity:.88; }
.mobile-task-copy { display:grid; flex:1; gap:8px; min-width:0; }
.mobile-task-title { display:flex; align-items:center; gap:6px; min-width:0; }
.mobile-task-title strong { min-width:0; overflow:hidden; display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; font-size:16px; font-weight:600; line-height:1.45; overflow-wrap:anywhere; }
.mobile-priority { flex:none; padding:1px 5px; border-radius:5px; font-size:10px; line-height:1.7; color:var(--accent); background:var(--accent-light); }
.mobile-priority.urgent { color:var(--danger); background:color-mix(in srgb,var(--danger) 10%,transparent); }
.mobile-task-meta { color:var(--text-muted); font-size:12px; line-height:1.5; font-variant-numeric:tabular-nums; }
.mobile-task-meta.overdue { color:var(--danger); }
.mobile-task-meta-row { display:flex; align-items:baseline; justify-content:space-between; flex-wrap:wrap; gap:4px 10px; min-width:0; }
.mobile-task-percent { flex:none; margin-left:auto; color:var(--text-secondary); font-size:12px; font-weight:600; font-variant-numeric:tabular-nums; }
.mobile-task-percent small { margin-left:1px; font-size:10px; font-weight:500; color:var(--text-faint); }
.mobile-card-progress { height:3px; overflow:hidden; border-radius:3px; background:var(--border-faint); }
.mobile-card-progress i { display:block; height:100%; background:var(--accent); transform-origin:left; transition:transform 240ms var(--ease-emphasized); }
.mobile-row-chevron { flex:none; font-size:13px; color:var(--text-faint); }
.closed .mobile-task-title { color:var(--text-muted); }
.mobile-task-empty { display:grid; justify-items:center; gap:10px; padding:65px 12px; color:var(--text-secondary); text-align:center; }
.mobile-task-empty > .el-icon { font-size:30px; color:var(--text-faint); }
.mobile-task-empty strong { font-size:16px; }
.mobile-task-empty p { margin:0; font-size:13px; color:var(--text-muted); line-height:1.6; }
.mobile-task-empty button { min-height:44px; padding:8px 16px; border:0; border-radius:12px; background:var(--bg-hover); color:var(--text-primary); font:inherit; font-size:14px; }
.mobile-task-loading { display:grid; gap:10px; padding-top:12px; }
.mobile-task-loading i { height:90px; border-radius:18px; background:var(--bg-hover); }
button:focus-visible { outline:2px solid var(--accent); outline-offset:-2px; }
.compact-task-enter-active,.compact-task-leave-active { transition:opacity 180ms ease,transform 240ms var(--ease-spring-gentle); }
.compact-task-leave-active { position:absolute; }
.compact-task-enter-from,.compact-task-leave-to { opacity:0; transform:translateY(8px); }
@media(prefers-reduced-motion:reduce) { .mobile-task-row,.compact-task-enter-active,.compact-task-leave-active { transition:opacity 120ms ease; } .mobile-card-progress i { transition:none; } .mobile-task-row:active,.compact-task-enter-from,.compact-task-leave-to { transform:none; } }
</style>
