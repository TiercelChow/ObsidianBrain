<template>
  <MotionModal v-model="visible" aria-label="来源预览" size="wide">
    <div class="knowledge-modal-card citation-preview">
      <header class="knowledge-modal-head">
        <div><span>{{ preview?.kind === 'external' ? '外部资料' : '来源预览' }}</span><h3>{{ preview?.entry.title || entry?.title || '来源预览' }}</h3><p>{{ preview?.entry.source_path || entry?.source_path || '数据库实体' }}</p><p v-if="preview?.historical">本轮实际读取的历史快照 · {{ preview.version_id }}</p></div>
        <button type="button" aria-label="关闭来源预览" @click="visible = false"><el-icon><Close /></el-icon></button>
      </header>
      <div class="knowledge-modal-body">
        <p v-if="preview && !preview.historical" class="preview-status">当前实体版本（非本轮历史证据）</p>
        <p v-if="preview?.read_ranges.length" class="preview-status">已读范围：{{ preview.read_ranges.map(range => `${range.offset_chars}–${range.offset_chars + range.returned_chars} 字符`).join('、') }}</p>
        <div v-if="loading" class="preview-status" role="status"><el-icon class="is-loading"><Loading /></el-icon>正在读取来源</div>
        <p v-else-if="error" class="preview-status" role="alert">{{ error }}</p>
        <template v-else-if="preview">
          <KnowledgeAnswerMarkdown :content="preview.content_md" :evidence-count="0" />
          <section v-if="preview.citations.length" class="preview-citations"><h4>原文引用</h4><article v-for="citation in preview.citations" :key="citation.id"><strong>{{ citation.source_path }}</strong><small v-if="citation.line_start">第 {{ citation.line_start }}–{{ citation.line_end || citation.line_start }} 行</small><p v-if="citation.quote_text">{{ citation.quote_text }}</p></article></section>
        </template>
      </div>
      <footer class="knowledge-modal-actions">
        <el-button @click="visible = false">关闭</el-button>
        <el-button v-if="preview && preview.kind !== 'external'" type="primary" @click="openWorkspace">在 Wiki 工作台打开当前版本</el-button>
        <el-button v-else-if="error && entry && entry.entry_type !== 'external'" @click="loadCurrent">查看当前版本（非本轮证据）</el-button>
      </footer>
    </div>
  </MotionModal>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Close, Loading } from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import { getAgentRunCitation, getKnowledgeEntry, type AgentRunCitationPreview, type KnowledgeEntrySummary } from '@/api/knowledge'
import { loadKnowledgeCitationPreview } from '@/utils/knowledgeCitationPreview'

const props = defineProps<{ modelValue: boolean; entry: KnowledgeEntrySummary | null; runId?: string; sourceIndex: number }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const router = useRouter()
const visible = computed({ get: () => props.modelValue, set: value => emit('update:modelValue', value) })
const preview = ref<AgentRunCitationPreview | null>(null)
const loading = ref(false)
const error = ref('')
let requestId = 0

async function load(runId?: string) {
  const currentRequest = ++requestId
  if (!props.entry && !runId) return
  preview.value = null
  error.value = ''
  loading.value = true
  try {
    let result: AgentRunCitationPreview
    if (props.entry) result = await loadKnowledgeCitationPreview(props.entry, runId, props.sourceIndex, { snapshot: getAgentRunCitation, current: getKnowledgeEntry })
    else {
      const response = await getAgentRunCitation(runId!, props.sourceIndex)
      if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '历史来源读取失败')
      result = response.result
    }
    if (currentRequest === requestId) preview.value = result
  } catch (failure) {
    if (currentRequest === requestId) error.value = (failure as Error).message
  } finally {
    if (currentRequest === requestId) loading.value = false
  }
}
function loadCurrent() { void load() }
function openWorkspace() {
  if (!preview.value || preview.value.kind === 'external') return
  visible.value = false
  void router.push({ path: '/knowledge/wiki', query: { base: preview.value.entry.knowledge_base_id, entry: preview.value.entry.id } })
}
watch(() => [props.modelValue, props.entry?.id, props.runId, props.sourceIndex], () => {
  if (props.modelValue) void load(props.runId)
  else { ++requestId; preview.value = null; loading.value = false }
}, { immediate: true })
onBeforeUnmount(() => { ++requestId })
</script>

<style scoped>
.citation-preview { height: min(720px, calc(100dvh - 48px)); }
.knowledge-modal-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.knowledge-modal-head > div { min-width: 0; }
.knowledge-modal-head span { color: var(--accent); font-size: 11px; font-weight: 650; }
.knowledge-modal-head h3, .knowledge-modal-head p { overflow-wrap: anywhere; }
.knowledge-modal-head button { width: 36px; height: 36px; flex: none; border: 0; border-radius: 11px; color: var(--text-muted); background: var(--bg-glass-subtle); font-size: 18px; cursor: pointer; }
.preview-status { display: flex; align-items: center; gap: 8px; color: var(--text-muted); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.preview-citations { display: grid; gap: 10px; }
.preview-citations h4 { margin: 0; font-size: 12px; }
.preview-citations article { display: grid; gap: 4px; padding: 10px 12px; border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-muted); overflow-wrap: anywhere; }
.preview-citations strong { font-size: 12px; font-weight: 600; }
.preview-citations small, .preview-citations p { font-size: 11px; line-height: 1.65; }
@media (max-width: 720px) { .citation-preview { height: min(86dvh, 760px); } }
</style>
