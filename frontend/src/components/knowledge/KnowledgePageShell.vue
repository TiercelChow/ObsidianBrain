<template>
  <div class="knowledge-page">
    <header class="page-header knowledge-heading">
      <div>
        <h1 class="page-title">{{ title }}</h1>
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
import { useRoute } from 'vue-router'
import { useMobileSubnav } from '@/composables/useMobileSubnav'

defineProps<{ title: string }>()

const tabs = [
  { path: '/knowledge', label: '知识库', icon: Collection },
  { path: '/knowledge/wiki', label: 'Wiki', icon: Tickets },
  { path: '/knowledge/chat', label: '问答', icon: ChatDotRound },
  { path: '/knowledge/tasks', label: '研究任务', icon: Operation },
  { path: '/knowledge/settings', label: '配置', icon: Setting },
]

const route = useRoute()
useMobileSubnav(() => ({
  label: '书籍知识功能',
  items: tabs.map(item => ({ id: item.path, label: item.label, compactLabel: item.path === '/knowledge/tasks' ? '研究' : undefined, to: item.path, active: route.path === item.path })),
}))
</script>
