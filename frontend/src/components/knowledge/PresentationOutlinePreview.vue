<template>
  <section class="outline-preview" aria-label="演示分页预览">
    <p v-if="preview.tight" role="status">
      当前优先 {{ preview.paging.preferred_total }} 页，按论点密度参考 {{ preview.reference_total }} 页。可增加页数或时长，让以下依据有足够的讲述空间。
    </p>
    <p v-else>
      <template v-if="preview.paging.min_total === preview.paging.max_total">按 {{ preview.paging.preferred_total }} 页规划。</template>
      <template v-else>预计 {{ preview.paging.min_total }}–{{ preview.paging.max_total }} 页，优先 {{ preview.paging.preferred_total }} 页。</template>
      页数含封面。
    </p>
    <details v-for="(section, index) in preview.sections" :key="`${index}-${section.title}`" :open="preview.tight || index === 0">
      <summary>{{ section.title }}</summary>
      <ol>
        <li v-for="(point, pointIndex) in preview.findings.slice(section.finding_start, section.finding_end)" :key="pointIndex">
          <span>{{ point.finding }}</span>
          <small v-if="point.limitations.length">{{ point.limitations.join(' · ') }}</small>
          <button v-for="ordinal in point.citation_indices" :key="ordinal" type="button" :title="sourceTitle(ordinal)" @click="emit('citation', ordinal - 1)">S{{ ordinal }}</button>
        </li>
      </ol>
    </details>
  </section>
</template>

<script setup lang="ts">
import type { PresentationOutlinePreview } from '@/api/wikiPresentation'

const props = defineProps<{ preview: PresentationOutlinePreview }>()
const emit = defineEmits<{ citation: [index: number] }>()

function sourceTitle(ordinal: number) {
  return props.preview.sources.find(source => source.index === ordinal)?.title || `来源 S${ordinal}`
}
</script>

<style scoped>
.outline-preview { display: grid; gap: 6px; padding: 10px 12px; border: 1px solid var(--accent-border); border-radius: 10px; }
.outline-preview p, .outline-preview ol { margin: 0; }
.outline-preview p, .outline-preview li, .outline-preview summary { color: var(--text-secondary); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
.outline-preview summary { font-weight: 650; cursor: pointer; }
.outline-preview li { margin: 5px 0; }
.outline-preview li small { display: block; color: var(--text-muted); }
.outline-preview li button {
  min-height: 28px; margin: 3px 5px 0 0; padding: 2px 6px;
  border: 1px solid var(--accent-border); border-radius: 7px; background: var(--accent-light);
  color: var(--accent); font: inherit; cursor: pointer;
}
</style>
