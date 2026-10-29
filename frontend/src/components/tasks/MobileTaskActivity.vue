<template>
  <div class="mobile-task-activity">
    <div v-if="parts.progress.length" class="mobile-progress-feed">
      <article v-for="item in parts.progress" :key="item.id" class="mobile-progress-entry">
        <header><button v-if="attributed" type="button" class="progress-task" :title="item.taskTitle" @click="$emit('select', item.taskId)">{{ item.taskTitle }}</button><span v-else>进展</span><time>{{ formatTimestamp(item.time) }}</time></header>
        <button type="button" class="progress-note" :aria-label="`查看记录详情：${item.taskTitle} ${item.title}`" @click="$emit('inspect', item)">
          <p>{{ item.note }}</p><small v-if="item.detail">{{ item.detail }}</small>
        </button>
      </article>
    </div>
    <p v-else-if="!auditOnly" class="mobile-progress-empty">还没有进展，记录迈出的第一步。</p>
    <details v-if="parts.audit.length" class="mobile-audit" :open="auditOnly">
      <summary>状态与操作记录 <span>{{ parts.audit.length }}</span><el-icon class="ui-chevron" aria-hidden="true"><ArrowRight /></el-icon></summary>
      <div class="mobile-audit-list">
        <button v-for="item in parts.audit" :key="item.id" type="button" :aria-label="`查看记录详情：${item.taskTitle} ${item.title}`" @click="$emit('inspect', item)">
          <span class="audit-top"><strong>{{ item.title }}</strong><time>{{ formatTimestamp(item.time) }}</time></span>
          <span v-if="attributed" class="audit-task">{{ item.taskTitle }}</span>
          <span v-if="item.detail" class="audit-detail">{{ item.detail }}</span>
          <span v-if="item.note" class="audit-note">{{ item.note }}</span>
        </button>
      </div>
    </details>
    <p v-else-if="auditOnly" class="mobile-progress-empty">关闭说明与状态变化会保留在这里。</p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ArrowRight } from '@element-plus/icons-vue'
import type { TaskActivityEntry } from '@/utils/taskActivity'
import { partitionTaskActivity } from '@/utils/taskMobile'
import { formatTimestamp } from '@/utils/taskDates'
const props = defineProps<{ entries: TaskActivityEntry[]; attributed?: boolean; auditOnly?: boolean }>()
defineEmits<{ inspect: [entry: TaskActivityEntry]; select: [id: string] }>()
const parts = computed(() => partitionTaskActivity(props.entries))
</script>

<style scoped>
.mobile-progress-feed { display:grid; gap:14px; }
.mobile-progress-entry { padding:14px; border-radius:17px; background:var(--bg-hover); }
.mobile-progress-entry header { display:flex; align-items:center; justify-content:space-between; gap:10px; min-width:0; margin-bottom:8px; color:var(--text-muted); font-size:12px; }
time { flex:none; margin-left:auto; color:var(--text-faint); font-size:10px; font-variant-numeric:tabular-nums; }
.progress-task { max-width:52%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; border:0; background:transparent; padding:8px 0; margin:-6px 0; min-height:44px; color:var(--text-secondary); font:inherit; text-align:left; }
.progress-note { display:block; width:100%; min-height:44px; border:0; padding:0; background:transparent; color:var(--text-primary); font:inherit; text-align:left; cursor:pointer; }
.progress-note p { display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; overflow-wrap:anywhere; white-space:pre-wrap; margin:0; font-size:14px; line-height:1.75; }
.progress-note small { display:block; margin-top:9px; color:var(--accent); font-size:12px; }
.mobile-progress-empty { margin:8px 0 22px; font-size:13px; line-height:1.8; color:var(--text-muted); }
.mobile-audit { margin-top:16px; color:var(--text-secondary); }
.mobile-audit summary { display:flex; align-items:center; gap:7px; min-height:44px; list-style:none; font-size:12px; cursor:pointer; }
.mobile-audit summary::-webkit-details-marker { display:none; }
.mobile-audit summary span { color:var(--text-faint); }
.mobile-audit summary .el-icon { margin-left:auto; transition:transform 180ms var(--ease-emphasized); }
.mobile-audit[open] summary .el-icon { transform:rotate(90deg); }
.mobile-audit-list { display:grid; gap:0; }
.mobile-audit-list > button { display:grid; gap:5px; width:100%; min-height:60px; padding:12px 0; border:0; border-bottom:1px solid var(--border-faint); background:transparent; text-align:left; color:var(--text-secondary); font:inherit; cursor:pointer; }
.audit-top { display:flex; align-items:center; gap:10px; }
.audit-top strong { font-size:12px; font-weight:550; }
.audit-task,.audit-detail { font-size:12px; color:var(--text-muted); overflow-wrap:anywhere; }
.audit-note { display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden; color:var(--text-secondary); font-size:13px; line-height:1.7; white-space:pre-wrap; overflow-wrap:anywhere; }
button:focus-visible,summary:focus-visible { outline:2px solid var(--accent); outline-offset:2px; border-radius:8px; }
@media(prefers-reduced-motion:reduce) { .mobile-audit summary .el-icon { transition:none; } }
</style>
