<template>
  <div class="explore-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">知识探索</h1>
        <p class="page-subtitle">发现知识缺口，生成研究问题，碰撞概念交叉</p>
      </div>
      <div class="header-actions">
        <el-button size="small" @click="loadAll" :loading="loading">
          <el-icon v-if="!loading"><Refresh /></el-icon>
          刷新
        </el-button>
      </div>
    </header>

    <!-- 知识缺口 -->
    <section class="explore-section">
      <div class="section-header">
        <h2 class="section-title">🔍 知识缺口</h2>
        <el-button size="small" @click="loadGaps" :loading="gapsLoading">分析</el-button>
      </div>
      <div class="gap-list" v-if="gaps.length > 0">
        <div v-for="(gap, i) in gaps" :key="i" class="gap-card">
          <div class="gap-concepts">
            <span class="concept-tag a">{{ gap.concept_a }}</span>
            <span class="gap-x">✕</span>
            <span class="concept-tag b">{{ gap.concept_b }}</span>
          </div>
          <div class="gap-reason">{{ gap.reason }}</div>
          <div class="gap-direction">💡 {{ gap.direction }}</div>
        </div>
      </div>
      <div v-else-if="!gapsLoading" class="empty-hint">点击「分析」发现概念间的缺失连接</div>
    </section>

    <!-- 研究问题 -->
    <section class="explore-section">
      <div class="section-header">
        <h2 class="section-title">❓ 研究问题</h2>
        <el-button size="small" @click="loadQuestions" :loading="questionsLoading">生成</el-button>
      </div>
      <div class="question-list" v-if="questions.length > 0">
        <div v-for="(q, i) in questions" :key="i" class="question-card">
          <div class="question-num">{{ i + 1 }}</div>
          <div class="question-body">
            <div class="question-text">{{ q.question }}</div>
            <div class="question-why">{{ q.why }}</div>
            <div class="question-related">
              <span v-for="r in q.related" :key="r" class="related-tag">{{ r }}</span>
            </div>
          </div>
        </div>
      </div>
      <div v-else-if="!questionsLoading" class="empty-hint">点击「生成」获取基于 Wiki 的研究问题</div>
    </section>

    <!-- 概念碰撞 -->
    <section class="explore-section">
      <div class="section-header">
        <h2 class="section-title">⚡ 概念碰撞</h2>
        <el-button size="small" @click="loadCollision" :loading="collisionLoading">碰撞</el-button>
      </div>
      <div v-if="collision" class="collision-card">
        <div class="collision-concepts">
          <div class="collision-concept a">
            <span>{{ collision.concept_a }}</span>
          </div>
          <div class="collision-x">✕</div>
          <div class="collision-concept b">
            <span>{{ collision.concept_b }}</span>
          </div>
        </div>
        <div class="collision-analysis" v-if="collision.analysis">
          <div class="analysis-row">
            <span class="analysis-label">交叉点</span>
            <span>{{ collision.analysis.intersection }}</span>
          </div>
          <div class="analysis-row">
            <span class="analysis-label">新洞察</span>
            <span>{{ collision.analysis.insight }}</span>
          </div>
          <div class="analysis-row" v-if="collision.analysis.should_link">
            <span class="analysis-label">建议引用</span>
            <span>{{ collision.analysis.link_reason }}</span>
          </div>
        </div>
      </div>
      <div v-else-if="!collisionLoading" class="empty-hint">点击「碰撞」随机选取两个概念分析交叉点</div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { Refresh } from '@element-plus/icons-vue'
import { discoverGaps, generateQuestions, conceptCollision } from '@/api/explore'

interface Gap {
  concept_a: string
  concept_b: string
  reason: string
  direction: string
}

interface Question {
  question: string
  related: string[]
  why: string
}

interface Collision {
  concept_a: string
  concept_b: string
  analysis: {
    intersection: string
    insight: string
    should_link: boolean
    link_reason: string
  } | null
}

const loading = ref(false)
const gapsLoading = ref(false)
const questionsLoading = ref(false)
const collisionLoading = ref(false)

const gaps = ref<Gap[]>([])
const questions = ref<Question[]>([])
const collision = ref<Collision | null>(null)

async function loadGaps() {
  gapsLoading.value = true
  try {
    const res = await discoverGaps() as unknown as { result: { gaps: Gap[] } }
    gaps.value = res.result?.gaps || []
  } catch (e) {
    console.error('分析缺口失败:', e)
  } finally {
    gapsLoading.value = false
  }
}

async function loadQuestions() {
  questionsLoading.value = true
  try {
    const res = await generateQuestions() as unknown as { result: { questions: Question[] } }
    questions.value = res.result?.questions || []
  } catch (e) {
    console.error('生成问题失败:', e)
  } finally {
    questionsLoading.value = false
  }
}

async function loadCollision() {
  collisionLoading.value = true
  try {
    const res = await conceptCollision() as unknown as { result: Collision }
    collision.value = res.result
  } catch (e) {
    console.error('概念碰撞失败:', e)
  } finally {
    collisionLoading.value = false
  }
}

async function loadAll() {
  loading.value = true
  await Promise.allSettled([loadGaps(), loadQuestions(), loadCollision()])
  loading.value = false
}
</script>

<style scoped>
.explore-page { max-width: 100%; min-height: 100%; }
.page-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-muted); font-size: 14px; }
.header-actions { display: flex; gap: 8px; }

.explore-section { margin-bottom: 28px; }
.section-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.section-title { font-size: 15px; font-weight: 600; color: var(--text-primary); }

.empty-hint { text-align: center; color: var(--text-faint); font-size: 13px; padding: 24px 0; }

.gap-list { display: flex; flex-direction: column; gap: 10px; }
.gap-card { padding: 16px; border-radius: 14px; }
.gap-concepts { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
.concept-tag { font-size: 13px; font-weight: 600; padding: 4px 12px; border-radius: 8px; }
.concept-tag.a { background: rgba(99, 102, 241, 0.15); color: #818cf8; }
.concept-tag.b { background: rgba(236, 72, 153, 0.15); color: #f472b6; }
.gap-x { color: var(--text-faint); font-size: 14px; }
.gap-reason { font-size: 13px; color: var(--text-secondary); line-height: 1.6; margin-bottom: 4px; }
.gap-direction { font-size: 13px; color: var(--text-tertiary); }

.question-list { display: flex; flex-direction: column; gap: 10px; }
.question-card { display: flex; gap: 14px; padding: 16px; border-radius: 14px; }
.question-num { width: 28px; height: 28px; border-radius: 50%; background: var(--accent); color: #fff; display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700; flex-shrink: 0; }
.question-body { flex: 1; }
.question-text { font-size: 14px; color: var(--text-primary); font-weight: 500; margin-bottom: 4px; }
.question-why { font-size: 12px; color: var(--text-muted); margin-bottom: 6px; }
.question-related { display: flex; gap: 4px; flex-wrap: wrap; }
.related-tag { font-size: 11px; color: var(--accent); background: var(--accent-light); padding: 2px 8px; border-radius: 6px; }

.collision-card { padding: 20px; border-radius: 14px; }
.collision-concepts { display: flex; align-items: center; justify-content: center; gap: 20px; margin-bottom: 16px; }
.collision-concept { padding: 12px 24px; border-radius: 14px; text-align: center; }
.collision-concept.a { background: rgba(99, 102, 241, 0.15); }
.collision-concept.b { background: rgba(236, 72, 153, 0.15); }
.collision-concept span { font-size: 15px; font-weight: 600; }
.collision-concept.a span { color: #818cf8; }
.collision-concept.b span { color: #f472b6; }
.collision-x { font-size: 20px; color: var(--text-faint); }
.collision-analysis { display: flex; flex-direction: column; gap: 10px; }
.analysis-row { display: flex; gap: 10px; font-size: 13px; }
.analysis-label { color: var(--text-muted); font-weight: 600; min-width: 70px; flex-shrink: 0; }

@media (max-width: 768px) {
  .page-header { margin-bottom: 16px; flex-wrap: wrap; gap: 8px; }
  .page-subtitle { width: 100%; order: 1; margin-top: 0; }
  .gap-card, .question-card, .collision-card { padding: 14px; }
  .collision-concepts { gap: 12px; }
  .collision-concept { padding: 10px 16px; }
}
</style>
