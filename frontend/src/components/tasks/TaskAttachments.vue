<template>
  <section
    class="task-attachments"
    :class="{ 'task-attachments--compact': compact }"
    aria-label="附件"
    :aria-busy="loading || Boolean(uploading)"
  >
    <header>
      <span
        >附件 <small v-if="items.length">{{ items.length }}</small></span
      ><UiAction
        :icon="uploading ? Close : Paperclip"
        :label="uploading ? '取消上传' : '上传附件'"
        :disabled="loading"
        @click="uploading ? request?.abort() : picker?.click()"
      /><input
        ref="picker"
        type="file"
        multiple
        hidden
        :disabled="loading"
        @change="choose"
      />
    </header>
    <p v-if="loading" class="attachment-muted" role="status">正在读取…</p>
    <p v-else-if="!items.length && !uploading" class="attachment-muted">
      暂无附件
    </p>
    <TransitionGroup name="collection-flow" tag="div" class="attachment-list">
      <div
        v-for="item in items"
        :key="item.id"
        class="attachment-row"
        data-glass="inset"
      >
        <el-icon class="attachment-file"><Document /></el-icon
        ><span class="attachment-copy"
          ><strong>{{ item.filename }}</strong
          ><small>{{ attachmentSize(item.size_bytes) }}</small></span
        >
        <a
          data-icon-only
          class="attachment-download ui-icon-action"
          :href="taskAttachmentUrl(item.id)"
          :download="item.filename"
          :aria-label="`下载 ${item.filename}`"
          ><el-icon><Download /></el-icon
        ></a>
        <UiAction
          :icon="Delete"
          :label="`移除 ${item.filename}`"
          :disabled="deleting === item.id"
          @click="confirmDelete = item"
        />
      </div>
    </TransitionGroup>
    <div v-if="uploading" class="attachment-upload" role="status">
      <el-icon class="is-loading"><Loading /></el-icon
      ><span>{{ uploading }} · {{ percent }}%</span
      ><progress :value="percent" max="100" />
    </div>
    <p v-if="error" class="attachment-error" role="alert">
      {{ error }} <button type="button" @click="load">重新读取</button>
    </p>
    <p class="attachment-hint">单个文件 ≤ 50 MB · 最多 20 个</p>
    <MotionModal
      :model-value="Boolean(confirmDelete)"
      aria-label="移除附件"
      @update:model-value="!$event && (confirmDelete = null)"
    >
      <section class="attachment-confirm glass-surface-heavy">
        <h3>移除附件？</h3>
        <p>{{ confirmDelete?.filename }}</p>
        <p>文件将从本机存储中删除，此操作无法撤销。</p>
        <footer>
          <button type="button" @click="confirmDelete = null">取消</button
          ><button type="button" class="danger" @click="remove">移除</button>
        </footer>
      </section>
    </MotionModal>
  </section>
</template>
<script setup lang="ts">
import { ref, watch, onBeforeUnmount } from 'vue'
import {
  Paperclip,
  Close,
  Delete,
  Document,
  Download,
  Loading,
} from '@element-plus/icons-vue'
import UiAction from '@/components/motion/UiAction'
import MotionModal from '@/components/motion/MotionModal.vue'
import {
  listTaskAttachments,
  uploadTaskAttachment,
  deleteTaskAttachment,
  taskAttachmentUrl,
  type TaskAttachment,
} from '@/api/taskAttachments'
import { attachmentSize, attachmentValidation } from '@/utils/taskAttachments'
const props = defineProps<{
  taskId: string
  progressId?: string
  compact?: boolean
}>()
const items = ref<TaskAttachment[]>([]),
  picker = ref<HTMLInputElement | null>(null),
  loading = ref(false),
  uploading = ref(''),
  percent = ref(0),
  error = ref(''),
  deleting = ref(''),
  confirmDelete = ref<TaskAttachment | null>(null)
let generation = 0,
  request: AbortController | null = null
function message(e: unknown) {
  const value = e as {
    response?: { data?: { message?: string } }
    message?: string
  }
  return value.response?.data?.message || value.message || '附件操作失败'
}
async function load() {
  if (uploading.value) return
  const current = ++generation,
    task = props.taskId,
    progress = props.progressId
  loading.value = true
  error.value = ''
  try {
    const result = await listTaskAttachments(task, progress)
    if (current === generation) items.value = result
  } catch (e) {
    if (current === generation) error.value = message(e)
  } finally {
    if (current === generation) loading.value = false
  }
}
watch(
  () => [props.taskId, props.progressId],
  () => {
    request?.abort()
    request = null
    uploading.value = ''
    deleting.value = ''
    items.value = []
    confirmDelete.value = null
    void load()
  },
  { immediate: true },
)
async function choose(event: Event) {
  const input = event.target as HTMLInputElement,
    files = Array.from(input.files || [])
  input.value = ''
  if (uploading.value) return
  const task = props.taskId,
    progress = props.progressId,
    current = generation
  const controller = new AbortController()
  request = controller
  error.value = ''
  for (const file of files) {
    if (controller.signal.aborted || current !== generation) break
    const invalid = attachmentValidation(file)
    if (invalid) {
      error.value = `${file.name}：${invalid}`
      break
    }
    uploading.value = file.name
    percent.value = 0
    try {
      const item = await uploadTaskAttachment(
        task,
        progress,
        file,
        controller.signal,
        (p) => {
          if (current === generation) percent.value = p
        },
      )
      if (current === generation) items.value = [...items.value, item]
    } catch (e) {
      if (current === generation)
        error.value = controller.signal.aborted
          ? '上传已取消；已完成的附件仍保留。'
          : message(e)
      break
    }
  }
  if (current === generation) {
    uploading.value = ''
    request = null
  }
}
async function remove() {
  const item = confirmDelete.value
  if (!item) return
  confirmDelete.value = null
  deleting.value = item.id
  const current = generation
  try {
    await deleteTaskAttachment(item.id)
    if (current === generation)
      items.value = items.value.filter((v) => v.id !== item.id)
  } catch (e) {
    if (current === generation) error.value = message(e)
  } finally {
    if (current === generation) deleting.value = ''
  }
}
onBeforeUnmount(() => {
  generation++
  request?.abort()
})
</script>
<style scoped>
.task-attachments {
  min-width: 0;
  margin-top: 18px;
}
.task-attachments--compact {
  margin-top: 0;
}
.task-attachments--compact .attachment-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 44px;
  gap: 2px;
  padding: 6px 8px;
}
.task-attachments--compact .attachment-file {
  display: none;
}
.task-attachments--compact .attachment-copy {
  grid-column: 1 / -1;
}
.task-attachments--compact .attachment-copy strong {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.task-attachments--compact .attachment-download {
  justify-self: end;
}
.task-attachments--compact .attachment-hint {
  display: none;
}
.task-attachments--compact .attachment-list {
  max-height: 184px;
  overflow: auto;
  overscroll-behavior: contain;
}
.task-attachments > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 13px;
  font-weight: 600;
}
.task-attachments small,
.attachment-muted,
.attachment-hint {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 400;
}
.attachment-muted {
  margin: 4px 0 8px;
}
.attachment-hint {
  margin: 8px 0 0;
  font-size: 11px;
}
.attachment-list {
  display: grid;
  gap: 6px;
}
.attachment-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 56px;
  padding: 6px 8px;
  border-radius: 14px;
  min-width: 0;
}
.attachment-file {
  flex: none;
  color: var(--text-muted);
  font-size: 22px;
}
.attachment-copy {
  display: grid;
  gap: 4px;
  min-width: 0;
  flex: 1;
}
.attachment-copy strong {
  font-size: 13px;
  font-weight: 500;
  overflow-wrap: anywhere;
  line-height: 1.5;
}
.attachment-download {
  display: grid;
  place-items: center;
  flex: none;
  width: 44px;
  height: 44px;
  color: var(--text-primary);
  font-size: 20px;
  text-decoration: none;
}
.attachment-row :deep(.ui-action) {
  flex: none;
}
.attachment-upload {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  margin-top: 8px;
}
.attachment-upload span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.attachment-upload progress {
  width: 60px;
  flex: none;
  accent-color: var(--accent);
}
.attachment-error {
  color: var(--danger);
  font-size: 12px;
  overflow-wrap: anywhere;
}
.attachment-error button {
  border: 0;
  background: none;
  color: var(--accent);
  font: inherit;
  cursor: pointer;
}
.attachment-confirm {
  padding: 24px;
  border-radius: 24px;
  width: min(420px, 92vw);
}
.attachment-confirm p {
  font-size: 14px;
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}
.attachment-confirm footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}
.attachment-confirm button {
  min-height: 44px;
  padding: 8px 16px;
  border: 0;
  border-radius: 12px;
  background: var(--bg-hover);
  color: var(--text-primary);
  font: inherit;
  cursor: pointer;
}
.attachment-confirm .danger {
  color: var(--danger);
}
</style>
