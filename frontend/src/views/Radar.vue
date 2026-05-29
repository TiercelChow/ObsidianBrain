<template>
  <div class="page">
    <header class="page-header">
      <h1 class="page-title">智识雷达</h1>
      <p class="page-subtitle">让外部信息来找你，基于个人知识图谱的个性化推荐</p>
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
import { Connection, TrendCharts, Download, Refresh } from '@element-plus/icons-vue'

const features = [
  { icon: Connection, title: '多源聚合', desc: 'RSS、arXiv、HackerNews、Reddit 定时拉取更新' },
  { icon: TrendCharts, title: '语义排序', desc: '与近期活跃笔记的向量相似度计算 + 多因子加权' },
  { icon: Download, title: '一键纳藏', desc: '文章正文提取后生成 Obsidian 笔记，写入 Vault' },
  { icon: Refresh, title: '状态管理', desc: 'New / Read / Saved / Dismissed 四态流转' },
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
  background: #ecfeff; color: #06b6d4;
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
