<template>
  <div class="knowledge-page">
    <header class="page-header knowledge-heading">
      <div>
        <div class="knowledge-eyebrow">BOOK WIKI</div>
        <h1 class="page-title">{{ title }}</h1>
        <p class="page-subtitle">{{ subtitle }}</p>
      </div>
      <div class="header-actions"><slot name="actions" /></div>
    </header>

    <nav class="knowledge-tabs" aria-label="书籍知识功能">
      <router-link v-for="item in tabs" :key="item.path" :to="item.path">
        <el-icon><component :is="item.icon" /></el-icon>
        <span>{{ item.label }}</span>
      </router-link>
    </nav>

    <main class="knowledge-content"><slot /></main>
  </div>
</template>

<script setup lang="ts">
import { ChatDotRound, Collection, Operation, Setting, Tickets } from '@element-plus/icons-vue'

defineProps<{ title: string; subtitle: string }>()

const tabs = [
  { path: '/knowledge', label: '知识库', icon: Collection },
  { path: '/knowledge/wiki', label: 'Wiki', icon: Tickets },
  { path: '/knowledge/chat', label: '问答', icon: ChatDotRound },
  { path: '/knowledge/tasks', label: '研究任务', icon: Operation },
  { path: '/knowledge/settings', label: '配置', icon: Setting },
]
</script>
