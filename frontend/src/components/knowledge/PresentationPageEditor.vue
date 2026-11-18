<template>
  <MotionModal v-model="visible" aria-label="演示逐页编辑" size="wide" :dismissible="!saving">
    <div class="knowledge-modal-card page-editor">
      <header class="knowledge-modal-head">
        <h3>演示大纲</h3>
      </header>
      <div v-if="loading" class="editor-state" role="status">正在读取演示大纲</div>
      <div v-else-if="outline" class="knowledge-modal-body editor-body">
        <nav aria-label="幻灯片列表" class="page-list">
          <button type="button" :disabled="hasChanges || saving" :aria-current="pageNumber === 1 ? 'page' : undefined" @click="pageNumber = 1">
            <span>1</span>{{ outline.cover_title }}
          </button>
          <button v-for="page in outline.pages" :key="page.page" :disabled="hasChanges || saving" type="button"
            :aria-current="pageNumber === page.page ? 'page' : undefined" @click="pageNumber = page.page">
            <span>{{ page.page }}</span>{{ page.title }}
          </button>
        </nav>
        <section v-if="selectedPage" class="page-content">
          <h4>第 {{ selectedPage.page }} 页 · {{ layoutLabels[selectedPage.layout] || '正文' }}</h4>
          <p class="editor-state">
            文字容量 {{ characterCount }} / {{ selectedPage.character_limit }}
            <template v-if="selectedPage.estimated_seconds"> · {{ selectedPage.estimated_seconds }} 秒</template>
          </p>
          <p v-if="editBlocked" class="editor-state" role="status">{{ editBlocked }}</p>
          <p v-else class="editor-state">保存为新 PPTX，仅更新这一页文字。</p>
          <div v-if="visibleSources.length" class="page-sources">
            <span>本页出处</span>
            <button v-for="source in visibleSources" :key="source.index" type="button" :title="source.source_path || source.title" @click="openSource(source.index)">
              S{{ source.index }} · {{ source.title }}
            </button>
          </div>
          <ul v-if="selectedPage.fixed_text.length" class="locked-content" aria-label="原文引语或图表数据">
            <li v-for="(content, index) in selectedPage.fixed_text" :key="index">{{ content }}</li>
          </ul>
          <label v-for="field in selectedPage.fields" :key="field.path" class="edit-field">
            <span>{{ field.label }}</span>
            <el-input v-model="draft[field.path]" :readonly="Boolean(editBlocked)" :disabled="saving"
              :type="field.path === '/title' || field.path === '/eyebrow' ? 'text' : 'textarea'"
              :autosize="{ minRows: 1, maxRows: 5 }" />
          </label>
        </section>
        <section v-else class="page-content cover-copy">
          <h4>{{ outline.cover_title }}</h4>
          <p>{{ outline.cover_subtitle }}</p>
          <p>{{ outline.core_message }}</p>
        </section>
      </div>
      <p v-if="error" class="editor-state editor-error" role="alert">{{ error }}</p>
      <footer class="knowledge-modal-actions">
        <el-button :disabled="saving" @click="visible = false">关闭</el-button>
        <el-button v-if="hasChanges" :disabled="saving" @click="resetDraft">放弃本页修改</el-button>
        <el-button v-if="selectedPage && !editBlocked" type="primary" :loading="saving" :disabled="!hasChanges" @click="savePage">保存新版 PPTX</el-button>
      </footer>
    </div>
  </MotionModal>
  <KnowledgeCitationPreview v-model="sourceVisible" :entry="null" :run-id="outline?.report_run_id" :source-index="sourceIndex" />
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { isAxiosError } from 'axios'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgeCitationPreview from '@/components/knowledge/KnowledgeCitationPreview.vue'
import type { KnowledgeArtifact } from '@/api/knowledge'
import { editPresentationSlide, getSavedPresentationOutline, type SavedPresentationOutline } from '@/api/wikiPresentation'

const props = defineProps<{ modelValue: boolean; artifactId: string; canEdit: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [artifact: KnowledgeArtifact] }>()
const visible = computed({ get: () => props.modelValue, set: value => emit('update:modelValue', value) })
const outline = ref<SavedPresentationOutline | null>(null)
const draft = ref<Record<string, string>>({})
const pageNumber = ref(2)
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const sourceVisible = ref(false)
const sourceIndex = ref(0)
let revision = 0

const layoutLabels: Record<string, string> = {
  statement: '论点', split: '对照', process: '步骤', metric: '指标', chart: '图表',
  relationship: '关系图', quote: '引语', evidence: '证据', summary: '收束',
}
const selectedPage = computed(() => outline.value?.pages.find(page => page.page === pageNumber.value))
const visibleSources = computed(() => outline.value?.sources.filter(source =>
  selectedPage.value?.citations.some(citation => Number(citation.slice(1)) === source.index)) ?? [])
const editBlocked = computed(() => outline.value?.read_only_reason || (!props.canEdit ? '报告待复核，当前可查看演示大纲。' : ''))
const changes = computed(() => Object.fromEntries(selectedPage.value?.fields
  .filter(field => (draft.value[field.path] ?? field.text).trim() !== field.text.trim())
  .map(field => [field.path, draft.value[field.path]]) ?? []))
const hasChanges = computed(() => Object.keys(changes.value).length > 0)
const characterCount = computed(() => {
  const page = selectedPage.value
  if (!page) return 0
  return page.fields.reduce((count, field) => {
    const value = changes.value[field.path]
    return value === undefined ? count : count + Array.from(value.trim()).length - Array.from(field.text).length
  }, page.used_characters)
})

function message(failure: unknown) {
  return isAxiosError(failure) ? failure.response?.data?.message || failure.message : (failure as Error).message
}

function resetDraft() {
  draft.value = Object.fromEntries(selectedPage.value?.fields.map(field => [field.path, field.text]) ?? [])
  error.value = ''
}

function openSource(ordinal: number) {
  sourceIndex.value = ordinal - 1
  sourceVisible.value = true
}

watch(selectedPage, resetDraft, { immediate: true })
watch(() => [props.modelValue, props.artifactId] as const, async ([open, artifactId]) => {
  const request = ++revision
  outline.value = null
  sourceVisible.value = false
  error.value = ''
  loading.value = false
  saving.value = false
  pageNumber.value = 2
  if (!open || !artifactId) return
  loading.value = true
  try {
    const result = await getSavedPresentationOutline(artifactId)
    if (request === revision) outline.value = result
  } catch (failure) {
    if (request === revision) error.value = message(failure)
  } finally {
    if (request === revision) loading.value = false
  }
}, { immediate: true })

async function savePage() {
  const current = outline.value
  const page = selectedPage.value
  if (!current || !page || editBlocked.value || !hasChanges.value || saving.value) return
  const request = revision
  saving.value = true
  error.value = ''
  try {
    const artifact = await editPresentationSlide(current.artifact_id, current.report_run_id, page.page, changes.value)
    if (request === revision) {
      emit('saved', artifact)
      visible.value = false
    }
  } catch (failure) {
    if (request === revision) error.value = message(failure)
  } finally {
    if (request === revision) saving.value = false
  }
}

onBeforeUnmount(() => { ++revision })
</script>

<style scoped>
.page-editor { max-height: min(760px, calc(100dvh - 40px)); }
.editor-body { display: grid; grid-template-columns: minmax(145px, 28%) minmax(0, 1fr); gap: 16px; }
.page-list, .page-content, .edit-field { display: grid; align-content: start; gap: 8px; min-width: 0; }
.page-list button, .page-sources button {
  min-height: 36px; border: 1px solid var(--accent-border); border-radius: 10px;
  background: var(--bg-glass-subtle); color: var(--text-primary); font: inherit; font-size: 11px;
  text-align: left; padding: 7px 9px; cursor: pointer; overflow-wrap: anywhere;
}
.page-list button span { display: inline-block; color: var(--text-muted); min-width: 24px; }
.page-list button[aria-current='page'] { background: var(--accent-light); color: var(--accent); }
.page-list button:disabled:not([aria-current='page']) { opacity: .55; cursor: default; }
.page-content h4, .cover-copy p { margin: 0; overflow-wrap: anywhere; }
.cover-copy p, .edit-field span, .editor-state, .page-sources > span, .locked-content { color: var(--text-muted); font-size: 12px; line-height: 1.6; }
.editor-state { margin: 0; overflow-wrap: anywhere; }
.editor-error { color: var(--danger); }
.page-sources { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
.page-sources button { color: var(--accent); }
.locked-content { margin: 0; padding: 8px 12px 8px 28px; border-radius: 9px; background: var(--bg-glass-subtle); }
@media (max-width: 720px) {
  .editor-body { grid-template-columns: minmax(0, 1fr); }
  .page-list { display: flex; overflow-x: auto; }
  .page-list button { min-width: 112px; flex: 0 0 auto; max-width: 190px; min-height: 44px; }
}
</style>
