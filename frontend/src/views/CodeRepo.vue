<template>
  <div class="page">
    <header class="page-header">
      <h1 class="page-title">代码仓管理</h1>
      <p class="page-subtitle">注册本地 Git 仓库，元信息展示，笔记关联与自动文档化</p>
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
import { FolderAdd, DataBoard, Connection, Document } from '@element-plus/icons-vue'

const features = [
  { icon: FolderAdd, title: '仓库注册', desc: '验证 Git 仓库路径，提取元数据并持久化配置' },
  { icon: DataBoard, title: '信息卡片', desc: '分支、最近提交、语言占比、工作区状态一览' },
  { icon: Connection, title: '笔记关联', desc: '在笔记中插入仓库引用块，建立双向链接' },
  { icon: Document, title: '自动文档化', desc: '调用 LLM 生成结构化项目文档笔记写入 Vault' },
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
  background: #ecfdf5; color: #10b981;
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
