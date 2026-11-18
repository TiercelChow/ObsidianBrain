<template>
  <div class="presentation-pagination-fields">
    <label class="research-brief-field">
      <span>演示分页</span>
      <el-select
        :model-value="pagination.mode"
        class="knowledge-select is-fluid"
        popper-class="system-select-popper"
        aria-label="演示分页方式"
        @update:model-value="chooseMode"
      >
        <el-option label="按内容自动分页" value="auto" />
        <el-option label="指定总页数" value="fixed" />
        <el-option label="按讲述时长" value="duration" />
      </el-select>
    </label>
    <label v-if="pagination.mode === 'fixed'" class="research-brief-field">
      <span>总页数（含封面和附录）</span>
      <el-input-number :model-value="pagination.total_slides" :min="3" :max="24" :precision="0"
        aria-label="演示总页数" @update:model-value="changePages" />
    </label>
    <label v-if="pagination.mode === 'duration'" class="research-brief-field">
      <span>讲述时间上限（分钟）</span>
      <el-input-number :model-value="pagination.minutes" :min="1" :max="45" :precision="0"
        aria-label="演示时长" @update:model-value="changeMinutes" />
    </label>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { PresentationPagination } from '@/api/knowledge'

const props = defineProps<{ modelValue?: PresentationPagination }>()
const emit = defineEmits<{ 'update:modelValue': [value: PresentationPagination] }>()
const pagination = computed<PresentationPagination>(() => props.modelValue ?? { mode: 'auto' })

function chooseMode(mode: PresentationPagination['mode']) {
  if (mode === 'fixed') emit('update:modelValue', { mode, total_slides: 6 })
  else if (mode === 'duration') emit('update:modelValue', { mode, minutes: 8 })
  else emit('update:modelValue', { mode: 'auto' })
}

function changePages(total_slides: number | undefined) {
  if (total_slides !== undefined) emit('update:modelValue', { mode: 'fixed', total_slides })
}

function changeMinutes(minutes: number | undefined) {
  if (minutes !== undefined) emit('update:modelValue', { mode: 'duration', minutes })
}
</script>

<style scoped>
.presentation-pagination-fields {
  display: grid;
  gap: 12px;
}
</style>
