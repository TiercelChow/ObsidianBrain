<template>
  <section class="task-calendar glass-panel" aria-label="任务日历">
    <header class="calendar-header">
      <div class="calendar-heading">
        <h2>{{ monthTitle(anchor) }}</h2>
        <span>{{ topLevelTasks.length }} 项安排</span>
      </div>
      <div class="calendar-nav">
        <button type="button" aria-label="上个月" @click="$emit('shift', -1)">‹</button>
        <button type="button" class="today-button" @click="$emit('today')">今天</button>
        <button type="button" aria-label="下个月" @click="$emit('shift', 1)">›</button>
      </div>
    </header>

    <div class="weekday-row" aria-hidden="true">
      <span v-for="weekday in weekdays" :key="weekday">{{ weekday }}</span>
    </div>

    <div class="calendar-grid" :class="{ loading }">
      <button
        v-for="day in days"
        :key="day.date"
        type="button"
        class="calendar-day"
        :class="{
          muted: !day.inCurrentMonth,
          today: day.isToday,
          selected: selectedDate === day.date,
          busy: topLevelEventsFor(day.date).length > 0,
        }"
        :aria-label="dayAriaLabel(day.date)"
        @click="$emit('select-date', day.date)"
        @dblclick="$emit('create', day.date)"
      >
        <span class="date-badge">
          <span class="day-number">{{ day.day }}</span>
          <span class="lunar-date">{{ formatLunarDate(day.date) }}</span>
        </span>
        <span class="mobile-dots" aria-hidden="true">
          <i
            v-for="task in topLevelEventsFor(day.date).slice(0, 3)"
            :key="task.id"
            :class="`importance-${task.importance}`"
          ></i>
        </span>
        <span class="day-events">
          <span
            v-for="task in topLevelEventsFor(day.date).slice(0, 3)"
            :key="task.id"
            class="event-pill"
            :class="[
              `importance-${task.importance}`,
              { closed: task.status === 'completed' || task.status === 'cancelled' },
            ]"
            @click.stop="$emit('open-task', task.id)"
          >
            {{ task.title }}
          </span>
          <span v-if="topLevelEventsFor(day.date).length > 3" class="more-events">
            +{{ topLevelEventsFor(day.date).length - 3 }}
          </span>
        </span>
      </button>
    </div>

    <div class="agenda">
      <div class="agenda-header">
        <div>
          <strong>{{ selectedDateLabel }}</strong>
          <span>{{ selectedRootCount ? `${selectedRootCount} 项任务` : '暂无安排' }}</span>
        </div>
        <button type="button" @click="$emit('create', selectedDate)">＋ 添加</button>
      </div>
      <TransitionGroup name="agenda-item" tag="div" class="agenda-list">
        <article
          v-for="entry in selectedEntries"
          :key="entry.task.id"
          class="agenda-card"
          :class="{ subtask: entry.depth > 0 }"
          :style="{ '--agenda-depth': entry.depth }"
        >
          <span class="agenda-accent" :class="`importance-${entry.task.importance}`"></span>
          <button type="button" class="agenda-open" @click="$emit('open-task', entry.task.id)">
            <span class="agenda-copy">
              <strong>{{ entry.task.title }}</strong>
              <span>{{ entry.depth > 0 ? '子任务 · ' : '' }}{{ formatTaskDateRange(entry.task.start_date, entry.task.end_date) }}</span>
            </span>
          </button>
          <span class="agenda-progress">{{ entry.task.progress_percent }}%</span>
          <button
            v-if="entry.depth === 0 && entry.hasChildren"
            type="button"
            class="agenda-expand"
            :class="{ expanded: expandedAgendaIds.has(entry.task.id) }"
            :aria-expanded="expandedAgendaIds.has(entry.task.id)"
            :aria-label="expandedAgendaIds.has(entry.task.id) ? `收起 ${entry.task.title} 的子任务` : `展开 ${entry.task.title} 的子任务`"
            @click="toggleAgenda(entry.task.id)"
          >›</button>
          <span v-else class="agenda-arrow" aria-hidden="true">›</span>
        </article>
      </TransitionGroup>
      <button v-if="selectedEntries.length === 0" type="button" class="agenda-empty" @click="$emit('create', selectedDate)">
        这一天还很空，安排一项任务
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TaskSummary } from '@/api/tasks'
import {
  buildMonthGrid,
  dateInRange,
  formatLunarDate,
  formatShortDate,
  formatTaskDateRange,
  monthTitle,
  parseLocalDate,
} from '@/utils/taskDates'
import { calendarAgendaEntries, calendarTopLevelTasks } from '@/utils/taskHierarchy'

const props = defineProps<{
  anchor: string
  selectedDate: string
  tasks: TaskSummary[]
  loading?: boolean
}>()

defineEmits<{
  shift: [months: number]
  today: []
  'select-date': [date: string]
  'open-task': [id: string]
  create: [date: string]
}>()

const weekdays = ['一', '二', '三', '四', '五', '六', '日']
const days = computed(() => buildMonthGrid(props.anchor))
const topLevelTasks = computed(() => calendarTopLevelTasks(props.tasks))
const expandedAgendaIds = ref<Set<string>>(new Set())
const selectedEntries = computed(() => calendarAgendaEntries(props.tasks, props.selectedDate, expandedAgendaIds.value))
const selectedRootCount = computed(() => selectedEntries.value.filter(entry => entry.depth === 0).length)
const selectedDateLabel = computed(() => {
  const parts = parseLocalDate(props.selectedDate)
  const weekday = new Date(parts.year, parts.month - 1, parts.day, 12).toLocaleDateString('zh-CN', { weekday: 'long' })
  return `${formatShortDate(props.selectedDate)} · ${weekday}`
})

function topLevelEventsFor(date: string) {
  return topLevelTasks.value
    .filter((task) => dateInRange(date, task.start_date, task.end_date))
    .sort((a, b) => importanceRank(b.importance) - importanceRank(a.importance) || a.end_date.localeCompare(b.end_date))
}

function importanceRank(value: TaskSummary['importance']) {
  return { low: 0, normal: 1, high: 2, urgent: 3 }[value]
}

function dayAriaLabel(date: string) {
  const count = topLevelEventsFor(date).length
  return `${date}，农历${formatLunarDate(date)}，${count ? `${count} 项任务` : '无任务'}`
}

function toggleAgenda(id: string) {
  const next = new Set(expandedAgendaIds.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expandedAgendaIds.value = next
}

watch(() => props.selectedDate, () => {
  expandedAgendaIds.value = new Set()
})
</script>

<style scoped>
.glass-panel {
  background: var(--bg-glass);
  border: 1px solid var(--border-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
}
.task-calendar { border-radius: 24px; padding: 20px; overflow: hidden; }
.calendar-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
.calendar-heading h2 { margin: 0; color: var(--text-primary); font-size: 22px; letter-spacing: var(--tracking-tight); }
.calendar-heading span { display: block; margin-top: 3px; color: var(--text-faint); font-size: 12px; }
.calendar-nav { display: flex; align-items: center; gap: 5px; padding: 4px; border: 1px solid var(--border-subtle); border-radius: 14px; background: var(--bg-glass); }
.calendar-nav button { min-width: 38px; height: 36px; border: 0; border-radius: 10px; background: transparent; color: var(--text-muted); font-size: 22px; cursor: pointer; transition: background var(--motion-fast) ease, transform var(--motion-instant) ease; }
.calendar-nav button:hover { color: var(--text-primary); background: var(--bg-glass-strong); }
.calendar-nav button:active { transform: scale(.94); }
.calendar-nav .today-button { width: auto; padding: 0 11px; font-size: 13px; font-weight: 600; }
.weekday-row, .calendar-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); }
.weekday-row { color: var(--text-faint); font-size: 11px; font-weight: 650; text-align: center; letter-spacing: .08em; }
.weekday-row span { padding: 7px; }
.calendar-grid { gap: 6px; margin-top: 4px; transition: opacity var(--motion-fast) ease; }
.calendar-grid.loading { opacity: .5; }
.calendar-day {
  position: relative;
  min-width: 0;
  min-height: 105px;
  padding: 8px;
  border: 0;
  border-radius: 15px;
  background: transparent;
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  overflow: hidden;
  transition: background var(--motion-fast) ease, transform var(--motion-instant) ease;
}
.calendar-day:hover { background: color-mix(in srgb, var(--text-primary) 4%, transparent); }
.calendar-day:active { transform: scale(.985); }
.calendar-day.selected { background: transparent; }
.calendar-day.muted { color: var(--text-faint); opacity: .58; }
.calendar-day.selected.muted { opacity: .82; }
.date-badge { display: contents; }
.lunar-date { position: absolute; top: 12px; left: 10px; max-width: calc(100% - 50px); overflow: hidden; color: var(--text-faint); font-size: 10px; line-height: 1; white-space: nowrap; }
.day-number { position: absolute; top: 7px; right: 7px; width: 32px; height: 32px; display: grid; place-items: center; border-radius: 50%; font-size: 15px; font-weight: 590; font-variant-numeric: tabular-nums; transition: color var(--motion-fast) ease, background var(--motion-fast) ease, transform var(--motion-fast) var(--ease-spring-gentle); }
.calendar-day.today .day-number { background: var(--accent); color: white; font-weight: 700; box-shadow: 0 5px 14px color-mix(in srgb, var(--accent) 28%, transparent); }
.calendar-day.selected:not(.today) .day-number { color: var(--accent); font-weight: 700; }
.day-events { display: grid; gap: 3px; margin-top: 36px; }
.event-pill { display: block; min-width: 0; padding: 4px 7px; border-radius: 7px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; background: color-mix(in srgb, var(--accent) 10%, transparent); color: var(--text-secondary); font-size: 11px; line-height: 1.35; }
.event-pill.importance-low { background-color: color-mix(in srgb, #8e8e93 10%, transparent); }
.event-pill.importance-high { background-color: color-mix(in srgb, #ff9500 12%, transparent); }
.event-pill.importance-urgent { background-color: color-mix(in srgb, #ff3b30 12%, transparent); }
.event-pill.closed { opacity: .5; }
.more-events { padding-left: 7px; color: var(--text-faint); font-size: 11px; }
.mobile-dots { display: none; }
.agenda { margin-top: 20px; padding: 16px; border-radius: 18px; background: color-mix(in srgb, var(--text-primary) 2.5%, transparent); }
.agenda-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.agenda-header div { display: flex; align-items: baseline; gap: 9px; }
.agenda-header strong { color: var(--text-primary); font-size: 15px; }
.agenda-header span { color: var(--text-faint); font-size: 12px; }
.agenda-header button { min-height: 38px; padding: 0 12px; border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--accent); font-weight: 600; cursor: pointer; }
.agenda-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.agenda-card { display: grid; grid-template-columns: 4px minmax(0, 1fr) auto 34px; align-items: center; gap: 10px; min-height: 58px; padding: 8px 7px 8px 0; border: 1px solid var(--border-subtle); border-radius: 14px; background: var(--bg-glass); color: var(--text-primary); text-align: left; transition: transform var(--motion-instant) ease, background var(--motion-fast) ease; overflow: hidden; }
.agenda-card.subtask { margin-left: min(calc(var(--agenda-depth) * 18px), 54px); background: color-mix(in srgb, var(--bg-glass) 72%, transparent); }
.agenda-card:hover { background: var(--bg-glass-strong); }
.agenda-card:active { transform: scale(.99); }
.agenda-accent { align-self: stretch; background: var(--accent); border: 0; }
.agenda-accent.importance-low { background: #8e8e93; }
.agenda-accent.importance-high { background: #ff9500; }
.agenda-accent.importance-urgent { background: #ff3b30; }
.agenda-open { min-width: 0; padding: 4px 0; border: 0; background: transparent; color: inherit; text-align: left; cursor: pointer; }
.agenda-copy { min-width: 0; display: grid; gap: 4px; }
.agenda-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
.agenda-copy span, .agenda-progress { color: var(--text-faint); font-size: 12px; }
.agenda-expand, .agenda-arrow { width: 32px; height: 32px; display: grid; place-items: center; border-radius: 10px; color: var(--text-faint); font-size: 20px; }
.agenda-expand { border: 0; background: transparent; cursor: pointer; transition: color var(--motion-fast) ease, background var(--motion-fast) ease, transform var(--motion-normal) var(--ease-spring-gentle); }
.agenda-expand:hover { background: color-mix(in srgb, var(--text-primary) 6%, transparent); color: var(--text-primary); }
.agenda-expand.expanded { color: var(--accent); transform: rotate(90deg); }
.agenda-empty { width: 100%; min-height: 58px; border: 1px dashed var(--border-subtle); border-radius: 14px; background: transparent; color: var(--text-faint); cursor: pointer; }
.agenda-item-enter-active, .agenda-item-leave-active { transition: opacity var(--motion-normal) ease, transform var(--motion-normal) var(--ease-spring-gentle); }
.agenda-item-enter-from, .agenda-item-leave-to { opacity: 0; transform: translateY(6px); }

@media (max-width: 768px) {
  .task-calendar { padding: 14px; border-radius: 20px; }
  .calendar-header { margin-bottom: 10px; }
  .calendar-heading h2 { font-size: 18px; }
  .calendar-nav button { min-width: 44px; height: 44px; }
  .calendar-nav .today-button { display: none; }
  .weekday-row span { padding: 5px 1px; }
  .calendar-grid { gap: 3px; }
  .calendar-day { min-height: 0; aspect-ratio: 1; padding: 4px 2px; border-radius: 12px; text-align: center; overflow: visible; }
  .date-badge { position: absolute; top: 1px; right: 1px; width: 40px; height: 40px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0; border-radius: 50%; transition: color var(--motion-fast) ease, background var(--motion-fast) ease, transform var(--motion-fast) var(--ease-spring-gentle); }
  .lunar-date { position: static; width: auto; max-width: 34px; color: var(--text-muted); font-size: 8px; line-height: 9px; text-align: center; }
  .day-number { position: static; width: auto; height: auto; border-radius: 0; font-size: 17px; line-height: 19px; }
  .calendar-day.today .date-badge { background: var(--accent); color: white; box-shadow: 0 5px 14px color-mix(in srgb, var(--accent) 28%, transparent); }
  .calendar-day.today .day-number { background: transparent; color: white; box-shadow: none; }
  .calendar-day.today .lunar-date { color: color-mix(in srgb, white 78%, transparent); }
  .calendar-day.selected:not(.today) .lunar-date { color: var(--accent); }
  .day-events { display: none; }
  .mobile-dots { position: absolute; top: 5px; right: auto; bottom: auto; left: -1px; display: flex; flex-direction: column; align-items: center; justify-content: flex-start; gap: 2px; }
  .mobile-dots i { width: 4px; height: 4px; border-radius: 50%; background: var(--accent); }
  .mobile-dots i.importance-low { background: #8e8e93; }
  .mobile-dots i.importance-high { background: #ff9500; }
  .mobile-dots i.importance-urgent { background: #ff3b30; }
  .agenda { margin-top: 14px; padding: 12px; border-radius: 16px; }
  .agenda-header button { min-height: 44px; }
  .agenda-list { grid-template-columns: 1fr; }
  .agenda-card { min-height: 64px; }
}

@media (prefers-reduced-motion: reduce) {
  .calendar-nav button, .calendar-grid, .calendar-day, .date-badge, .day-number, .agenda-card, .agenda-expand, .agenda-item-enter-active, .agenda-item-leave-active { transition-duration: 1ms !important; }
}

@media (prefers-reduced-transparency: reduce) {
  .glass-panel { backdrop-filter: none; -webkit-backdrop-filter: none; background: var(--bg-primary); }
}
</style>
