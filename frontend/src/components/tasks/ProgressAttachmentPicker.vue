<template>
  <section class="progress-attachment-picker" aria-label="进展附件">
    <header>
      <span
        >附件 <small v-if="items.length">{{ items.length }} / 20</small></span
      ><UiAction
        :icon="Paperclip"
        label="选择进展附件"
        :disabled="busy || items.length >= 20"
        @click="picker?.click()"
      /><input
        ref="picker"
        type="file"
        multiple
        hidden
        :disabled="busy"
        @change="choose"
      />
    </header>
    <p v-if="!items.length" class="draft-hint">
      可同时选择多个文件，随本次进展提交。单个文件 ≤ 50 MB。
    </p>
    <TransitionGroup
      name="collection-flow"
      tag="div"
      class="draft-attachment-list"
    >
      <div
        v-for="item in items"
        :key="item.id"
        class="draft-attachment-row"
        data-glass="inset"
      >
        <el-icon class="draft-file"><Document /></el-icon>
        <div class="draft-file-copy">
          <strong>{{ item.file.name }}</strong
          ><small
            >{{ attachmentSize(item.file.size) }} ·
            {{
              item.status === 'uploaded'
                ? '已上传'
                : item.status === 'uploading'
                  ? `上传中 ${item.percent}%`
                  : item.status === 'failed'
                    ? '上传失败'
                    : '待提交'
            }}</small
          >
          <p v-if="item.error" role="alert">{{ item.error }}</p>
          <progress
            v-if="item.status === 'uploading'"
            :value="item.percent"
            max="100"
          />
        </div>
        <el-icon
          v-if="item.status === 'uploaded'"
          class="draft-complete"
          aria-label="已上传"
          ><Check
        /></el-icon>
        <UiAction
          v-else
          :icon="Close"
          :label="`移除待上传附件 ${item.file.name}`"
          :disabled="busy"
          @click="$emit('remove', item.id)"
        />
      </div>
    </TransitionGroup>
  </section>
</template>
<script setup lang="ts">
import { ref } from 'vue'
import { Paperclip, Close, Document, Check } from '@element-plus/icons-vue'
import UiAction from '@/components/motion/UiAction'
import type { DraftFile } from '@/utils/progressAttachmentDraft'
import { attachmentSize } from '@/utils/taskAttachments'
defineProps<{ items: DraftFile<File>[]; busy: boolean }>()
const emit = defineEmits<{ select: [files: File[]]; remove: [id: string] }>()
const picker = ref<HTMLInputElement | null>(null)
function choose(event: Event) {
  const input = event.target as HTMLInputElement
  emit('select', Array.from(input.files || []))
  input.value = ''
}
</script>
<style scoped>
.progress-attachment-picker {
  min-width: 0;
}
.progress-attachment-picker > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 13px;
  font-weight: 600;
}
.progress-attachment-picker small,
.draft-hint {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 400;
  line-height: 1.6;
}
.draft-hint {
  margin: 0;
}
.draft-attachment-list {
  display: grid;
  gap: 8px;
  max-height: 220px;
  overflow: auto;
  overscroll-behavior: contain;
}
.draft-attachment-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 8px 10px;
  border-radius: 14px;
}
.draft-file {
  color: var(--text-muted);
  font-size: 22px;
  flex: none;
}
.draft-file-copy {
  display: grid;
  gap: 4px;
  flex: 1;
  min-width: 0;
}
.draft-file-copy strong {
  font-size: 13px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.draft-file-copy p {
  font-size: 12px;
  color: var(--danger);
  margin: 0;
  overflow-wrap: anywhere;
}
.draft-file-copy progress {
  width: 100%;
  height: 4px;
  accent-color: var(--accent);
}
.draft-complete {
  color: var(--accent);
  width: 44px;
  height: 44px;
  flex: none;
  font-size: 20px;
}
</style>
