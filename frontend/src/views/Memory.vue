<template>
  <div class="page">
    <header class="page-header">
      <h1 class="page-title">记忆管理</h1>
      <p class="page-subtitle">自动索引 Obsidian 笔记，提供全文与语义混合检索</p>
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
import { Document, Search, ScaleToOriginal, Link } from '@element-plus/icons-vue'

const features = [
  { icon: Document, title: '自动提取', desc: 'Vault 文件变更时自动分割、向量化并建立索引' },
  { icon: Search, title: '混合检索', desc: 'Tantivy 全文检索 + Qdrant 语义搜索，RRF 融合排序' },
  { icon: ScaleToOriginal, title: '智能分块', desc: '按标题层级与段落边界切分，300~800 token/块' },
  { icon: Link, title: '引用溯源', desc: '搜索结果附带 obsidian:// 链接，直达原始笔记' },
]
</script>

<style scoped>
.page-header { margin-bottom: 32px; }
.page-title {
  font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px;
}
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
  background: #f0f0ff; color: #6366f1;
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
