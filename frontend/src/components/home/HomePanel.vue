<template>
  <section class="home-panel glass-surface" :aria-busy="loading && !ready" :aria-label="title">
    <header><div><h2>{{ title }}</h2><p v-if="subtitle">{{ subtitle }}</p></div><RouterLink class="panel-link" :to="to">查看全部<el-icon><ArrowRight /></el-icon></RouterLink></header>
    <div v-if="loading && !ready" class="home-skeleton" aria-label="加载中"><i v-for="n in 3" :key="n"></i></div>
    <div v-else-if="!ready && error" class="home-empty"><span>{{ error }}</span><button type="button" @click="$emit('retry')">重试</button></div>
    <template v-else>
      <p v-if="error" class="home-stale">更新失败，暂时显示上次结果 <button type="button" @click="$emit('retry')">重试</button></p>
      <slot v-if="ready" />
      <p v-else class="home-empty">暂时无法读取</p>
    </template>
  </section>
</template>
<script setup lang="ts">
import { ArrowRight } from '@element-plus/icons-vue'
import type { RouteLocationRaw } from 'vue-router'
defineProps<{title:string; subtitle?:string; to:RouteLocationRaw; loading:boolean; ready:boolean; error?:string|null}>()
defineEmits<{retry:[]}>()
</script>
<style scoped>
.home-panel { min-width:0; padding:20px; border-radius:var(--radius-lg,20px); }
header { display:flex; justify-content:space-between; align-items:flex-start; gap:12px; margin-bottom:10px; }
header > div { min-width:0; }
h2 { margin:0; font-size:17px; font-weight:680; letter-spacing:-.02em; line-height:1.4; }
header p { margin:5px 0 0; color:var(--text-muted); font-size:12px; line-height:1.5; }
.panel-link { display:inline-flex; align-items:center; justify-content:flex-end; gap:3px; min-height:44px; flex:none; margin-top:-10px; color:var(--text-muted); font-size:12px; text-decoration:none; }
.panel-link:hover { color:var(--text-primary); }
.home-empty { display:grid; justify-items:start; gap:6px; margin:0; padding:20px 0; color:var(--text-muted); font-size:14px; line-height:1.6; }
button { min-height:44px; border:0; background:transparent; color:var(--text-primary); cursor:pointer; padding:0 8px; font:inherit; }
.home-stale { color:var(--text-muted); font-size:12px; }
.home-skeleton { display:grid; gap:12px; padding:6px 0; }
.home-skeleton i { display:block; height:58px; border-radius:12px; background:var(--bg-hover); }
@media(max-width:768px) { .home-panel { padding:16px; } h2 { font-size:17px; } }
</style>
