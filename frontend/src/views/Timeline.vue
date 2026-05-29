<template>
  <div class="page">
    <header class="page-header">
      <h1 class="page-title">时间线</h1>
      <p class="page-subtitle">知识演变的时间维度可视化，每日回顾与周报生成</p>
    </header>

    <div class="features-grid">
      <div
        v-for="(feat, i) in features"
        :key="feat.title"
        class="feature-card"
        :style="{ '--delay': `${i * 0.06}s` }"
      >
        <div class="feature-icon">
          <el-icon :size="18"><component :is="feat.icon" /></el-icon>
        </div>
        <div class="feature-content">
          <h3>{{ feat.title }}</h3>
          <p>{{ feat.desc }}</p>
        </div>
      </div>
    </div>

    <div class="empty-state">
      <el-empty description="功能开发中..." :image-size="80" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { Collection, DataAnalysis, ChatDotRound, Clock } from '@element-plus/icons-vue'

const features = [
  { icon: Collection, title: '多源收集', desc: '从 Frontmatter、文件名、#date 标签、Git 提交中提取事件' },
  { icon: DataAnalysis, title: '统计聚合', desc: '计数、频率、趋势分析，支持时段对比' },
  { icon: ChatDotRound, title: 'LLM 摘要', desc: '自动生成时间段摘要与知识周报' },
  { icon: Clock, title: '去年今日', desc: '回顾历史同期的知识活动，发现知识演变轨迹' },
]
</script>

<style scoped>
.page-header { margin-bottom: 32px; }
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }

.features-grid {
  display: grid; grid-template-columns: repeat(2, 1fr); gap: 14px; margin-bottom: 40px;
}
.feature-card {
  display: flex; gap: 14px; padding: 18px 20px;
  background: #fff; border: 1px solid #f0f0f0; border-radius: 16px;
  animation: fade-in 0.4s ease both; animation-delay: var(--delay, 0s);
  transition: box-shadow 0.2s ease;
}
.feature-card:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.04); }

.feature-icon {
  width: 38px; height: 38px; border-radius: 12px; flex-shrink: 0;
  display: flex; align-items: center; justify-content: center;
  background: #fffbeb; color: #f59e0b;
}
.feature-content h3 { font-size: 14px; font-weight: 600; color: #18181b; margin-bottom: 4px; }
.feature-content p { font-size: 13px; color: #71717a; line-height: 1.5; }

.empty-state {
  padding: 48px; background: #fff; border: 1px solid #f0f0f0; border-radius: 16px;
  animation: fade-in 0.4s ease both; animation-delay: 0.3s;
}

@keyframes fade-in {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
