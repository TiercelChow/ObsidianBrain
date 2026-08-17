<template>
  <section class="task-calendar glass-panel" aria-label="任务日历">
    <header class="calendar-header">
      <div class="calendar-heading">
        <h2>{{ monthTitle(anchor) }}</h2>
        <span>{{ tasks.length }} 项安排</span>
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
          busy: eventsFor(day.date).length > 0,
        }"
        :aria-label="dayAriaLabel(day.date)"
        @click="$emit('select-date', day.date)"
        @dblclick="$emit('create', day.date)"
      >
        <span class="day-number">{{ day.day }}</span>
        <span class="mobile-dots" aria-hidden="true">
          <i
            v-for="task in eventsFor(day.date).slice(0, 3)"
            :key="task.id"
            :class="`importance-${task.importance}`"
          ></i>
        </span>
        <span class="day-events">
          <span
            v-for="task in eventsFor(day.date).slice(0, 3)"
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
          <span v-if="eventsFor(day.date).length > 3" class="more-events">
            +{{ eventsFor(day.date).length - 3 }}
          </span>
        </span>
      </button>
    </div>

    <div class="agenda">
      <div class="agenda-header">
        <div>
          <strong>{{ selectedDateLabel }}</strong>
          <span>{{ selectedTasks.length ? `${selectedTasks.length} 项任务` : '暂无安排' }}</span>
        </div>
        <button type="button" @click="$emit('create', selectedDate)">＋ 添加</button>
      </div>
      <TransitionGroup name="agenda-item" tag="div" class="agenda-list">
        <button
          v-for="task in selectedTasks"
          :key="task.id"
          type="button"
          class="agenda-card"
          @click="$emit('open-task', task.id)"
        >
          <span class="agenda-accent" :class="`importance-${task.importance}`"></span>
          <span class="agenda-copy">
            <strong>{{ task.title }}</strong>
            <span>{{ formatTaskDateRange(task.start_date, task.end_date) }}</span>
          </span>
          <span class="agenda-progress">{{ task.progress_percent }}%</span>
          <span class="agenda-arrow">›</span>
        </button>
      </TransitionGroup>
      <button v-if="selectedTasks.length === 0" type="button" class="agenda-empty" @click="$emit('create', selectedDate)">
        这一天还很空，安排一项任务
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TaskSummary } from '@/api/tasks'
import {
  buildMonthGrid,
  dateInRange,
  formatShortDate,
  formatTaskDateRange,
  monthTitle,
  parseLocalDate,
} from '@/utils/taskDates'

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
const selectedTasks = computed(() => eventsFor(props.selectedDate))
const selectedDateLabel = computed(() => {
  const parts = parseLocalDate(props.selectedDate)
  const weekday = new Date(parts.year, parts.month - 1, parts.day, 12).toLocaleDateString('zh-CN', { weekday: 'long' })
  return `${formatShortDate(props.selectedDate)} · ${weekday}`
})

function eventsFor(date: string) {
  return props.tasks
    .filter((task) => dateInRange(date, task.start_date, task.end_date))
    .sort((a, b) => importanceRank(b.importance) - importanceRank(a.importance) || a.end_date.localeCompare(b.end_date))
}

function importanceRank(value: TaskSummary['importance']) {
  return { low: 0, normal: 1, high: 2, urgent: 3 }[value]
}

function dayAriaLabel(date: string) {
  const count = eventsFor(date).length
  return `${date}，${count ? `${count} 项任务` : '无任务'}`
}
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
.day-number { width: 28px; height: 28px; display: grid; place-items: center; border-radius: 9px; font-size: 12px; font-weight: 560; transition: color var(--motion-fast) ease, background var(--motion-fast) ease, transform var(--motion-fast) var(--ease-spring-gentle); }
.calendar-day.today:not(.selected) .day-number { background: color-mix(in srgb, var(--accent) 11%, transparent); color: var(--accent); font-weight: 680; }
.calendar-day.selected .day-number { background: var(--accent); color: white; font-weight: 680; box-shadow: 0 5px 14px color-mix(in srgb, var(--accent) 28%, transparent); transform: scale(1.04); }
.day-events { display: grid; gap: 3px; margin-top: 4px; }
.event-pill { display: block; min-width: 0; padding: 4px 7px; border-radius: 7px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; background: color-mix(in srgb, var(--accent) 10%, transparent); color: var(--text-secondary); font-size: 10px; line-height: 1.3; }
.event-pill.importance-low { background-color: color-mix(in srgb, #8e8e93 10%, transparent); }
.event-pill.importance-high { background-color: color-mix(in srgb, #ff9500 12%, transparent); }
.event-pill.importance-urgent { background-color: color-mix(in srgb, #ff3b30 12%, transparent); }
.event-pill.closed { opacity: .5; }
.more-events { padding-left: 7px; color: var(--text-faint); font-size: 10px; }
.mobile-dots { display: none; }
.agenda { margin-top: 20px; padding: 16px; border-radius: 18px; background: color-mix(in srgb, var(--text-primary) 2.5%, transparent); }
.agenda-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.agenda-header div { display: flex; align-items: baseline; gap: 9px; }
.agenda-header strong { color: var(--text-primary); font-size: 14px; }
.agenda-header span { color: var(--text-faint); font-size: 11px; }
.agenda-header button { min-height: 38px; padding: 0 12px; border: 1px solid var(--border-subtle); border-radius: 11px; background: var(--bg-glass); color: var(--accent); font-weight: 600; cursor: pointer; }
.agenda-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.agenda-card { display: grid; grid-template-columns: 4px minmax(0, 1fr) auto 18px; align-items: center; gap: 10px; min-height: 58px; padding: 8px 10px 8px 0; border: 1px solid var(--border-subtle); border-radius: 14px; background: var(--bg-glass); color: var(--text-primary); text-align: left; cursor: pointer; transition: transform var(--motion-instant) ease, background var(--motion-fast) ease; overflow: hidden; }
.agenda-card:hover { background: var(--bg-glass-strong); }
.agenda-card:active { transform: scale(.99); }
.agenda-accent { align-self: stretch; background: var(--accent); border: 0; }
.agenda-accent.importance-low { background: #8e8e93; }
.agenda-accent.importance-high { background: #ff9500; }
.agenda-accent.importance-urgent { background: #ff3b30; }
.agenda-copy { min-width: 0; display: grid; gap: 4px; }
.agenda-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
.agenda-copy span, .agenda-progress { color: var(--text-faint); font-size: 11px; }
.agenda-arrow { color: var(--text-faint); font-size: 20px; }
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
  .day-number { margin: 0 auto; width: 28px; height: 28px; }
  .day-events { display: none; }
  .mobile-dots { height: 6px; display: flex; align-items: center; justify-content: center; gap: 2px; }
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
  .calendar-nav button, .calendar-grid, .calendar-day, .day-number, .agenda-card, .agenda-item-enter-active, .agenda-item-leave-active { transition-duration: 1ms !important; }
}

@media (prefers-reduced-transparency: reduce) {
  .glass-panel { backdrop-filter: none; -webkit-backdrop-filter: none; background: var(--bg-primary); }
}
</style>
