<template>
  <div class="page">
    <header class="page-header">
      <h1 class="page-title">灵感熔炉</h1>
      <p class="page-subtitle">用你自己的知识制造新想法：概念碰撞、反向提问、对立观点</p>
    </header>

    <div class="modes-grid">
      <div
        v-for="(mode, i) in modes"
        :key="mode.title"
        class="mode-card"
        :style="{ '--delay': `${i * 0.08}s` }"
      >
        <div class="mode-icon" :style="{ background: mode.bg, color: mode.color }">
          <el-icon :size="22"><component :is="mode.icon" /></el-icon>
        </div>
        <h3 class="mode-title">{{ mode.title }}</h3>
        <p class="mode-desc">{{ mode.desc }}</p>
        <div class="mode-tags">
          <span v-for="tag in mode.tags" :key="tag" class="tag">{{ tag }}</span>
        </div>
      </div>
    </div>

    <div class="empty-state">
      <el-empty description="功能开发中..." :image-size="80" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { Coin, QuestionFilled, SortDown } from '@element-plus/icons-vue'

const modes = [
  {
    icon: Coin, title: '随机概念组合',
    desc: '从标签与仓库名中选取两个距离较远的概念，LLM 生成跨界联想。',
    tags: ['TF-IDF', '概念距离'],
    bg: '#fdf2f8', color: '#ec4899',
  },
  {
    icon: QuestionFilled, title: '反向提问引擎',
    desc: '选取一篇笔记，LLM 生成 3 个你可能没想过的深入问题。',
    tags: ['笔记分析', '深层提问'],
    bg: '#f0f0ff', color: '#6366f1',
  },
  {
    icon: SortDown, title: '对立观点生成器',
    desc: '对指定笔记生成反方观点和逻辑漏洞，帮助完善论证。',
    tags: ['逻辑分析', '论证完善'],
    bg: '#fdf4ff', color: '#a855f7',
  },
]
</script>

<style scoped>
.page-header { margin-bottom: 32px; }
.page-title { font-size: 22px; font-weight: 600; color: #18181b; letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: #a1a1aa; font-size: 14px; }

.modes-grid {
  display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-bottom: 40px;
}
.mode-card {
  padding: 24px; background: #fff; border: 1px solid #f0f0f0; border-radius: 18px;
  text-align: center;
  animation: fade-in 0.4s ease both; animation-delay: var(--delay, 0s);
  transition: box-shadow 0.2s ease;
}
.mode-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.04); }

.mode-icon {
  width: 48px; height: 48px; border-radius: 16px;
  display: flex; align-items: center; justify-content: center;
  margin: 0 auto 16px;
}
.mode-title { font-size: 15px; font-weight: 600; color: #18181b; margin-bottom: 8px; }
.mode-desc { font-size: 13px; color: #71717a; line-height: 1.6; margin-bottom: 16px; }

.mode-tags { display: flex; gap: 6px; justify-content: center; flex-wrap: wrap; }
.tag {
  font-size: 11px; padding: 3px 8px; border-radius: 8px;
  background: #f4f4f5; color: #52525b; font-weight: 500;
}

.empty-state {
  padding: 48px; background: #fff; border: 1px solid #f0f0f0; border-radius: 16px;
  animation: fade-in 0.4s ease both; animation-delay: 0.35s;
}

@keyframes fade-in {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
