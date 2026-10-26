<template>
  <span class="task-status-mark" :class="`is-${status}`" aria-hidden="true">
    <el-icon v-if="status === 'completed'"><CircleCheckFilled /></el-icon>
    <el-icon v-else-if="status === 'cancelled'" class="cancel-mark"><Minus /></el-icon>
    <span v-else-if="status === 'in_progress'" class="progress-mark"></span>
  </span>
</template>

<script setup lang="ts">
import { CircleCheckFilled, Minus } from '@element-plus/icons-vue'
import type { TaskStatus } from '@/api/tasks'

defineProps<{ status: TaskStatus }>()
</script>

<style scoped>
.task-status-mark {
  box-sizing: border-box;
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: 1.5px solid var(--border-subtle);
  border-radius: 50%;
  color: var(--text-muted);
  pointer-events: none;
}
.task-status-mark > .el-icon { width: 100%; height: 100%; font-size: 22px; line-height: 1; }
.task-status-mark :deep(svg) { display: block; }
.task-status-mark.is-completed { border: 0; color: var(--success); }
.task-status-mark.is-cancelled { background: color-mix(in srgb, var(--text-muted) 8%, transparent); }
.task-status-mark > .cancel-mark { font-size: 13px; }
.task-status-mark.is-in_progress { border-color: var(--accent); }
.progress-mark { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
.task-status-mark.is-blocked { border-color: var(--warning); }
</style>
