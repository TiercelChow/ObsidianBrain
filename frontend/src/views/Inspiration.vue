<template>
  <div class="inspiration-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">灵感熔炉</h1>
        <p class="page-subtitle">用你自己的知识制造新想法：概念碰撞、反向提问、对立观点</p>
      </div>
      <div class="header-actions">
        <el-button @click="loadHistory" :loading="loadingHistory">
          <el-icon><Refresh /></el-icon> 历史
        </el-button>
      </div>
    </header>

    <!-- 模式选择 -->
    <div class="mode-selector">
      <div
        v-for="mode in modes"
        :key="mode.value"
        class="mode-option"
        :class="{ active: selectedMode === mode.value }"
        @click="selectedMode = mode.value"
      >
        <div class="mode-icon">{{ mode.icon }}</div>
        <div class="mode-name">{{ mode.label }}</div>
        <div class="mode-desc">{{ mode.desc }}</div>
      </div>
    </div>

    <!-- 笔记选择（反向提问/对立观点需要） -->
    <div class="note-selector" v-if="selectedMode !== 'concept_combo'">
      <el-input
        v-model="notePath"
        placeholder="笔记路径（如：notes/my-note.md）"
        size="large"
        clearable
      >
        <template #prefix>
          <el-icon><Document /></el-icon>
        </template>
      </el-input>
      <el-button
        type="primary"
        size="large"
        @click="generateInspiration"
        :loading="generating"
        :disabled="selectedMode === 'counterpoint' && !notePath"
      >
        生成灵感
      </el-button>
    </div>

    <!-- 概念组合模式直接生成 -->
    <div class="action-bar" v-else>
      <el-button
        type="primary"
        size="large"
        @click="generateInspiration"
        :loading="generating"
        class="generate-btn"
      >
        <el-icon><MagicStick /></el-icon>
        随机组合生成
      </el-button>
    </div>

    <!-- 生成结果 -->
    <div class="result-section" v-if="result">
      <!-- 概念组合 -->
      <div v-if="result.type === 'concept_combo'" class="result-card combo-result">
        <div class="concepts-row">
          <div class="concept-chip">
            <span class="concept-icon">🅰️</span>
            <span class="concept-term">{{ result.concept_a?.term }}</span>
            <el-tag effect="plain">{{ result.concept_a?.source }}</el-tag>
          </div>
          <div class="concept-connector">✕</div>
          <div class="concept-chip">
            <span class="concept-icon">🅱️</span>
            <span class="concept-term">{{ result.concept_b?.term }}</span>
            <el-tag effect="plain">{{ result.concept_b?.source }}</el-tag>
          </div>
        </div>

        <div class="inspiration-text">
          <h3>💡 灵感</h3>
          <p>{{ result.inspiration }}</p>
        </div>

        <div class="suggestions" v-if="result.suggestions?.length">
          <h3>🎯 实践建议</h3>
          <ul>
            <li v-for="(s, i) in result.suggestions" :key="i">{{ s }}</li>
          </ul>
        </div>

        <div class="experiment" v-if="result.experiment_idea">
          <h3>🧪 实验方案</h3>
          <p>{{ result.experiment_idea }}</p>
        </div>
      </div>

      <!-- 反向提问 -->
      <div v-else-if="result.type === 'reverse_question'" class="result-card question-result">
        <div class="note-info">
          <el-icon><Document /></el-icon>
          <span>{{ result.note?.title }}</span>
          <span class="note-path">{{ result.note?.path }}</span>
        </div>

        <div class="questions-list">
          <div
            v-for="(q, i) in result.questions"
            :key="i"
            class="question-item"
          >
            <div class="question-number">{{ i + 1 }}</div>
            <div class="question-content">
              <div class="question-text">{{ q.question }}</div>
              <div class="question-why" v-if="q.why_it_matters">
                <strong>为什么值得思考：</strong>{{ q.why_it_matters }}
              </div>
              <el-tag effect="plain" class="question-type">
                {{ formatQuestionType(q.question_type) }}
              </el-tag>
            </div>
          </div>
        </div>
      </div>

      <!-- 对立观点 -->
      <div v-else-if="result.type === 'counterpoint'" class="result-card counterpoint-result">
        <div class="note-info">
          <el-icon><Document /></el-icon>
          <span>{{ result.note?.title }}</span>
          <span class="note-path">{{ result.note?.path }}</span>
        </div>

        <div class="counterpoints-list">
          <div
            v-for="(cp, i) in result.counterpoints"
            :key="i"
            class="counterpoint-item"
          >
            <div class="cp-header">
              <span class="cp-number">观点 {{ i + 1 }}</span>
            </div>
            <div class="cp-section">
              <div class="cp-label">📋 原始主张</div>
              <p>{{ cp.claim }}</p>
            </div>
            <div class="cp-section">
              <div class="cp-label">🔄 反方观点</div>
              <p>{{ cp.counter }}</p>
            </div>
            <div class="cp-section">
              <div class="cp-label">⚠️ 逻辑漏洞</div>
              <p>{{ cp.weakness }}</p>
            </div>
            <div class="cp-section">
              <div class="cp-label">💡 完善建议</div>
              <p>{{ cp.suggestion }}</p>
            </div>
          </div>
        </div>

        <div class="assessment" v-if="result.overall_assessment">
          <h3>📊 整体评估</h3>
          <p>{{ result.overall_assessment }}</p>
        </div>
      </div>
    </div>

    <el-empty v-else-if="!generating" description="选择模式并生成灵感" :image-size="80" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { getInspiration } from '@/api'
import { Refresh, Document, MagicStick } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

const selectedMode = ref('concept_combo')
const notePath = ref('')
const generating = ref(false)
const loadingHistory = ref(false)
const result = ref<any>(null)

const modes = [
  { value: 'concept_combo', label: '随机概念组合', icon: '🎲', desc: '两个距离较远的概念 → 跨界联想' },
  { value: 'reverse_question', label: '反向提问', icon: '❓', desc: '对你的一篇笔记生成深层问题' },
  { value: 'counterpoint', label: '对立观点', icon: '⚔️', desc: '对笔记生成反方观点和逻辑漏洞' },
]

async function generateInspiration() {
  if (selectedMode.value === 'counterpoint' && !notePath.value) {
    ElMessage.warning('对立观点模式需要指定笔记路径')
    return
  }

  generating.value = true
  try {
    const res = await getInspiration(
      selectedMode.value,
      notePath.value || undefined
    ) as unknown as { result: any }
    result.value = res.result
  } catch (e: any) {
    ElMessage.error('生成失败: ' + (e.message || '未知错误'))
    result.value = null
  } finally {
    generating.value = false
  }
}

function formatQuestionType(type: string): string {
  const map: Record<string, string> = {
    counterfactual: '假设反事实',
    extension: '延伸应用',
    logic_check: '逻辑一致性',
    temporal_projection: '时间维度',
  }
  return map[type] || type
}

function loadHistory() {
  ElMessage.info('历史记录功能开发中')
}
</script>

<style scoped>
.inspiration-page {
  min-height: 100%;
  max-width: 100%;
}
.inspiration-page .mode-option {
  animation: fade-in var(--duration-normal) var(--ease-out) both;
}
.inspiration-page .mode-option:nth-child(2) { animation-delay: 0.06s; }
.inspiration-page .mode-option:nth-child(3) { animation-delay: 0.12s; }
.inspiration-page .result-card { animation: fade-in var(--duration-normal) var(--ease-out) both; }
.inspiration-page .question-item { animation: fade-in var(--duration-normal) var(--ease-out) both; }
.inspiration-page .question-item:nth-child(2) { animation-delay: 0.06s; }
.inspiration-page .question-item:nth-child(3) { animation-delay: 0.12s; }
.inspiration-page .counterpoint-item { animation: fade-in var(--duration-normal) var(--ease-out) both; }
.inspiration-page .counterpoint-item:nth-child(2) { animation-delay: 0.06s; }
.inspiration-page .counterpoint-item:nth-child(3) { animation-delay: 0.12s; }

/* 模式选择 */
.mode-selector { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin-bottom: 24px; }
.mode-option {
  padding: 20px; border: 2px solid rgba(255, 255, 255, 0.6); border-radius: 16px;
  text-align: center; cursor: pointer; transition: var(--transition-interactive);
}
.mode-option:hover { border-color: #e4e4e7; }
.mode-option.active { border-color: #6366f1; background: rgba(99, 102, 241, 0.06); }
.mode-option:active { transform: scale(0.97); transition: transform 100ms ease-out; }
.mode-icon { font-size: 28px; margin-bottom: 8px; }
.mode-name { font-size: 15px; font-weight: 600; color: var(--text-primary); margin-bottom: 4px; }
.mode-desc { font-size: 12px; color: var(--text-faint); }

/* 笔记选择 */
.note-selector { display: flex; gap: 12px; margin-bottom: 24px; }
.action-bar { margin-bottom: 24px; }
.generate-btn { min-width: 160px; }

/* 结果区域 */
.result-section { animation: slide-up var(--duration-normal) var(--ease-out); }

.result-card {
  border-radius: 16px;
  padding: 24px; margin-bottom: 20px;
}

/* 概念组合 */
.concepts-row { display: flex; align-items: center; justify-content: center; gap: 16px; margin-bottom: 24px; }
.concept-chip {
  display: flex; align-items: center; gap: 8px; padding: 10px 16px;
  border-radius: 12px;
}
.concept-icon { font-size: 18px; }
.concept-term { font-size: 16px; font-weight: 600; color: var(--text-primary); }
.concept-connector { font-size: 20px; color: var(--text-faint); font-weight: 300; }

.inspiration-text h3, .suggestions h3, .experiment h3, .assessment h3 {
  font-size: 16px; font-weight: 600; color: var(--text-primary); margin-bottom: 8px;
}
.inspiration-text p, .experiment p, .assessment p {
  font-size: 14px; color: var(--text-tertiary); line-height: 1.7;
}
.suggestions ul { padding-left: 20px; }
.suggestions li { font-size: 14px; color: var(--text-tertiary); line-height: 1.7; margin-bottom: 4px; }

/* 反向提问 */
.note-info {
  display: flex; align-items: center; gap: 8px;
  font-size: 14px; color: var(--text-tertiary); margin-bottom: 16px;
}
.note-path { font-size: 12px; color: var(--text-faint); font-family: var(--font-mono); }

.questions-list { display: flex; flex-direction: column; gap: 16px; }
.question-item { display: flex; gap: 16px; padding: 16px; border-radius: 12px; }
.question-number {
  width: 32px; height: 32px; border-radius: 50%; background: #6366f1; color: #fff;
  display: flex; align-items: center; justify-content: center; font-weight: 600; flex-shrink: 0;
}
.question-text { font-size: 15px; font-weight: 500; color: var(--text-primary); margin-bottom: 6px; }
.question-why { font-size: 13px; color: var(--text-tertiary); margin-bottom: 8px; }
.question-why strong { color: var(--text-primary); }
.question-type { font-size: 11px; }

/* 对立观点 */
.counterpoints-list { display: flex; flex-direction: column; gap: 16px; }
.counterpoint-item { padding: 16px; border-radius: 12px; }
.cp-header { margin-bottom: 12px; }
.cp-number { font-size: 14px; font-weight: 600; color: #6366f1; }
.cp-section { margin-bottom: 12px; }
.cp-section:last-child { margin-bottom: 0; }
.cp-label { font-size: 13px; font-weight: 600; color: var(--text-primary); margin-bottom: 4px; }
.cp-section p { font-size: 13px; color: var(--text-tertiary); line-height: 1.6; }

.assessment { margin-top: 20px; padding: 16px; border-radius: 12px; }
.assessment h3 { color: #166534; }
.assessment p { color: #15803d; }

@media (max-width: 768px) {
  .mode-selector {
    display: flex;
    gap: 8px;
    margin-inline: -2px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
  }
  .mode-selector::-webkit-scrollbar { display: none; }
  .mode-option { flex: 0 0 min(78vw, 260px); padding: 14px; scroll-snap-align: start; text-align: left; }
  .mode-icon { float: left; margin: 0 10px 0 0; }
  .result-card { padding: 16px; }
  .note-selector { flex-direction: column; gap: 10px; }
  .note-selector :deep(.el-button), .action-bar :deep(.el-button) { width: 100%; }
  .concepts-row { flex-direction: column; gap: 10px; }
  .concept-chip { padding: 8px 12px; }
}
</style>
