<template>
  <el-date-picker v-if="!isMobile" v-bind="$attrs" :model-value="modelValue" :type="type"
    :format="format" :value-format="valueFormat" :clearable="clearable" :disabled="disabled"
    :placeholder="placeholder" :start-placeholder="startPlaceholder" :end-placeholder="endPlaceholder"
    @update:model-value="desktopUpdate" @change="desktopChange" @clear="emit('clear')"
  />
  <div v-else v-bind="$attrs" class="app-date-picker mobile-date-picker">
    <button type="button" class="mobile-date-trigger" :class="{ 'is-empty': !hasValue }"
      :disabled="disabled" :aria-label="pickerLabel" aria-haspopup="dialog" :aria-expanded="open" @click="show"
    ><el-icon><Calendar /></el-icon><span class="mobile-date-value">{{ displayValue }}</span></button>
    <MotionModal v-model="open" :aria-label="pickerLabel">
      <section class="date-wheel-sheet glass-surface-heavy">
        <header class="date-wheel-header">
          <button type="button" @click="open = false">取消</button>
          <h3>{{ pickerLabel }}</h3>
          <button type="button" class="date-wheel-confirm" @click="confirm">完成</button>
        </header>
        <div v-if="isRange" class="date-wheel-range" aria-label="编辑日期范围">
          <button v-for="side in endpoints" :key="side.index" type="button" :aria-pressed="endpoint === side.index"
            :class="{ active: endpoint === side.index }" @click="switchEndpoint(side.index)"
          ><span>{{ side.label }}</span><strong>{{ displayDate(draft[side.index]) }}</strong></button>
        </div>
        <p v-else class="date-wheel-summary" aria-live="polite">{{ displayDate(activeDate) }}</p>
        <div class="date-wheels" aria-label="年月日滚轮">
          <span class="date-wheel-selection" aria-hidden="true"></span>
          <DateWheelColumn ref="yearWheel" :model-value="activeDate.year" :options="years" label="年" @update:model-value="choose('year', $event)" />
          <DateWheelColumn ref="monthWheel" :model-value="activeDate.month" :options="months" label="月" @update:model-value="choose('month', $event)" />
          <DateWheelColumn ref="dayWheel" :model-value="activeDate.day" :options="days" label="日" @update:model-value="choose('day', $event)" />
        </div>
        <footer class="date-wheel-footer">
          <button type="button" @click="chooseToday">今天</button>
          <button v-if="clearable" type="button" @click="clear">清除日期</button>
        </footer>
      </section>
    </MotionModal>
  </div>
</template>

<script setup lang="ts" generic="T extends DatePickerModel">
import { computed, ref, useAttrs, watch } from 'vue'
import { Calendar } from '@element-plus/icons-vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import DateWheelColumn from './DateWheelColumn.vue'
import { usePhoneViewport } from '@/composables/usePhoneViewport'
import { clampDateParts, createDateDraft, dateKey, dateParts, datePickerValue, replaceDraftDate, type DateDraft, type DateParts, type DatePickerModel } from '@/utils/dateWheel'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<{
  modelValue: T
  type?: 'date' | 'daterange'
  format?: string
  valueFormat?: string
  clearable?: boolean
  disabled?: boolean
  placeholder?: string
  startPlaceholder?: string
  endPlaceholder?: string
}>(), { type: 'date', clearable: true, disabled: false })
const emit = defineEmits<{ 'update:modelValue': [value: T]; change: [value: T]; clear: [] }>()
const attrs = useAttrs()
const { isMobile } = usePhoneViewport()
const open = ref(false)
const endpoint = ref<0 | 1>(0)
const isRange = computed(() => props.type === 'daterange')
const pickerLabel = computed(() => String(attrs['aria-label'] || (isRange.value ? '选择日期范围' : '选择日期')))
const draft = ref<DateDraft>(createDateDraft(props.modelValue, isRange.value))
const activeDate = computed(() => draft.value[endpoint.value])
const yearWheel = ref<InstanceType<typeof DateWheelColumn> | null>(null)
const monthWheel = ref<InstanceType<typeof DateWheelColumn> | null>(null)
const dayWheel = ref<InstanceType<typeof DateWheelColumn> | null>(null)
const endpoints = [{ index: 0 as const, label: '开始日期' }, { index: 1 as const, label: '结束日期' }]
const months = Array.from({ length: 12 }, (_, index) => index + 1)
const yearBounds = ref<[number, number]>([1900, new Date().getFullYear() + 100])
const years = computed(() => {
  const [start, end] = yearBounds.value
  return Array.from({ length: end - start + 1 }, (_, index) => start + index)
})
const days = computed(() => Array.from({ length: clampDateParts({ ...activeDate.value, day: 31 }).day }, (_, index) => index + 1))
const hasValue = computed(() => Array.isArray(props.modelValue) ? props.modelValue.length === 2 : !!props.modelValue)
const displayDate = (parts: DateParts) => dateKey(parts).replace(/-/g, '/')
const displayValue = computed(() => {
  if (!hasValue.value) return props.placeholder || (isRange.value ? `${props.startPlaceholder || '开始日期'} 至 ${props.endPlaceholder || '结束日期'}` : '选择日期')
  const value = createDateDraft(props.modelValue, isRange.value)
  return isRange.value ? `${displayDate(value[0])} 至 ${displayDate(value[1])}` : displayDate(value[0])
})

function desktopUpdate(value: unknown) { emit('update:modelValue', value as T) }
function desktopChange(value: unknown) { emit('change', value as T) }
function show() {
  if (props.disabled) return
  draft.value = createDateDraft(props.modelValue, isRange.value)
  // Keep the rail stable throughout a gesture, including dates outside the
  // default range. Shrinking it on each passing year would cancel momentum.
  yearBounds.value = [Math.min(1900, draft.value[0].year, draft.value[1].year), Math.max(new Date().getFullYear() + 100, draft.value[0].year, draft.value[1].year)]
  endpoint.value = 0
  open.value = true
}
function settleWheels() {
  yearWheel.value?.settle()
  monthWheel.value?.settle()
  dayWheel.value?.settle()
}
function choose(part: keyof DateParts, value: number) {
  draft.value = replaceDraftDate(draft.value, endpoint.value, { ...activeDate.value, [part]: value }, isRange.value)
}
function switchEndpoint(value: 0 | 1) { settleWheels(); endpoint.value = value }
function chooseToday() {
  draft.value = replaceDraftDate(draft.value, endpoint.value, dateParts(new Date()), isRange.value)
}
function commit(value: T) { emit('update:modelValue', value); emit('change', value); open.value = false }
function confirm() {
  settleWheels()
  commit(datePickerValue(draft.value, isRange.value, !!props.valueFormat) as T)
}
function clear() {
  if (!props.clearable) return
  commit(null as T)
  emit('clear')
}
watch(isMobile, mobile => { if (!mobile) open.value = false })
</script>

<style scoped>
.mobile-date-picker { min-width:0; width:100%; }
.mobile-date-trigger { display:flex; align-items:center; gap:8px; width:100%; min-width:0; min-height:44px; padding:8px 10px; border:0; border-radius:var(--radius-control, 12px); background:var(--glass-field-fill); box-shadow:inset 0 0 0 1px var(--glass-field-edge); color:var(--text-primary); font:inherit; font-size:14px; line-height:1.5; text-align:left; cursor:pointer; }
.mobile-date-trigger .el-icon { flex:none; color:var(--text-muted); font-size:17px; }
.mobile-date-trigger.is-empty { color:var(--text-faint); }
.mobile-date-trigger:active { background:var(--bg-hover); }
.mobile-date-trigger:focus-visible { outline:2px solid var(--accent-border); outline-offset:2px; }
.mobile-date-value { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-variant-numeric:tabular-nums; }
.date-wheel-sheet { max-height:inherit; padding:24px 12px 10px; overflow-y:auto; overscroll-behavior:contain; border-radius:24px 24px 0 0; color:var(--text-primary); }
.date-wheel-header { display:grid; grid-template-columns:60px minmax(0, 1fr) 60px; align-items:center; gap:6px; }
.date-wheel-header h3 { min-width:0; margin:0; font-size:17px; font-weight:650; text-align:center; }
.date-wheel-sheet button { min-height:44px; padding:8px; border:0; border-radius:12px; background:transparent; color:var(--text-secondary); font:inherit; font-size:15px; cursor:pointer; }
.date-wheel-sheet button:active { background:var(--bg-hover); }
.date-wheel-sheet button:focus-visible { outline:2px solid var(--accent-border); outline-offset:-2px; }
.date-wheel-header .date-wheel-confirm { color:var(--accent); font-weight:650; }
.date-wheel-summary { margin:4px 0 0; color:var(--text-muted); font-size:14px; text-align:center; font-variant-numeric:tabular-nums; }
.date-wheel-range { display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px; margin:6px 0; padding:3px; border-radius:16px; background:var(--bg-hover); }
.date-wheel-range button { display:grid; gap:3px; min-width:0; padding:7px; }
.date-wheel-range button.active { background:var(--bg-glass-strong); box-shadow:var(--shadow-sm); color:var(--text-primary); }
.date-wheel-range span { font-size:12px; color:var(--text-muted); }
.date-wheel-range strong { font-size:14px; font-weight:600; font-variant-numeric:tabular-nums; }
.date-wheels { position:relative; display:grid; grid-template-columns:1.25fr 1fr 1fr; }
.date-wheel-selection { position:absolute; top:calc(50% - 22px); right:0; left:0; height:44px; border:1px solid var(--border-faint); border-radius:14px; background:var(--bg-hover); pointer-events:none; }
.date-wheel-footer { display:flex; justify-content:space-between; }
.date-wheel-footer button { min-width:60px; color:var(--text-muted); font-size:14px; }
@media (max-height: 500px) { .date-wheel-sheet { --date-wheel-rows:3; padding-top:20px; } .date-wheel-summary { display:none; } .date-wheel-range { margin:0; } }
</style>
