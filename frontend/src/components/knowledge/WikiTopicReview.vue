<template>
  <details class="wiki-index-details topic-review" @toggle="onToggle">
    <summary>内容体检<span v-if="findings.length">（{{ findings.length }} 项待核对）</span></summary>
    <p v-if="loading" role="status">正在读取体检结果…</p>
    <p v-if="error" class="topic-alert" role="alert">{{ error }}</p>
    <p v-if="report" class="topic-note">
      已用模型核对 {{ report.reviewed_pages }}/{{ report.published_pages }} 篇已发布主题，
      共 {{ report.reviewed_passages }}/{{ report.eligible_passages }} 段可核查正文。
    </p>
    <el-button v-else size="small" :disabled="!canRun || loading" :loading="running" @click="review">用模型核对结论与冲突</el-button>
    <p v-if="report && findings.length === 0" class="topic-note">所查范围暂未发现待核对项。</p>
    <ul v-if="findings.length">
      <li v-for="(finding, index) in findings" :key="index">
        <p>{{ labels[finding.kind] }} · {{ finding.detail }}</p>
        <button type="button" class="topic-text-link" @click="$emit('open-page', finding.location.page_id)">
          {{ finding.location.title }} · {{ finding.location.heading }} · 第 {{ finding.location.line }} 行
        </button>
        <blockquote>{{ finding.location.quote }}</blockquote>
        <template v-if="finding.related">
          <button type="button" class="topic-text-link" @click="$emit('open-page', finding.related.page_id)">
            {{ finding.related.title }} · {{ finding.related.heading }} · 第 {{ finding.related.line }} 行
          </button>
          <blockquote>{{ finding.related.quote }}</blockquote>
        </template>
      </li>
    </ul>
  </details>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getTopicReview, runTopicReview, type TopicReviewIssue, type TopicReviewReport } from '@/api/wikiTopicReview'
import { topicErrorMessage } from '@/api/wikiTopics'

const props = defineProps<{ baseId: string; canRun: boolean; revisionKey: string }>()
defineEmits<{ 'open-page': [pageId: string] }>()

const suspected = ref<TopicReviewIssue[]>([])
const report = ref<TopicReviewReport | null>(null)
const loading = ref(false)
const running = ref(false)
const expanded = ref(false)
const error = ref('')
const findings = computed(() => [...suspected.value, ...(report.value?.issues ?? [])])
let generation = 0
const labels = {
  number: '数字待核对',
  quotation: '引语待核对',
  unsupported: '证据不足',
  missing_condition: '遗漏条件',
  contradiction: '跨页冲突',
}

async function refresh() {
  const current = ++generation
  error.value = ''
  loading.value = true
  try {
    const result = await getTopicReview(props.baseId)
    if (current !== generation) return
    suspected.value = result.suspected
    report.value = result.latest
  } catch (failure) {
    if (current === generation) error.value = topicErrorMessage(failure)
  } finally {
    if (current === generation) loading.value = false
  }
}

function onToggle(event: Event) {
  expanded.value = (event.currentTarget as HTMLDetailsElement).open
  if (expanded.value && !running.value) void refresh()
}

watch(() => [props.baseId, props.revisionKey], () => {
  ++generation
  error.value = ''
  suspected.value = []
  report.value = null
  running.value = false
  loading.value = false
  if (expanded.value && !running.value) void refresh()
})

async function review() {
  ++generation
  const current = generation
  running.value = true
  error.value = ''
  try {
    const result = await runTopicReview(props.baseId)
    if (current === generation) report.value = result
  } catch (failure) {
    if (current === generation) error.value = topicErrorMessage(failure)
  } finally {
    if (current === generation) running.value = false
  }
}
</script>

<style scoped>
.topic-review blockquote {
  margin: 0.35rem 0 0.75rem;
  padding-inline-start: 0.75rem;
  border-inline-start: 2px solid var(--el-border-color);
}
</style>
