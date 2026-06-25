import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'Home',
      component: () => import('@/views/Home.vue'),
      meta: { title: '首页' },
    },
    {
      path: '/memory',
      name: 'Memory',
      component: () => import('@/views/Memory.vue'),
      meta: { title: '知识库' },
    },
    {
      path: '/wiki',
      name: 'WikiWorkbench',
      component: () => import('@/views/WikiWorkbench.vue'),
      meta: { title: 'Wiki 工作台' },
    },
    {
      path: '/wiki-dashboard',
      name: 'WikiDashboard',
      component: () => import('@/views/WikiDashboard.vue'),
      meta: { title: 'Wiki 看板' },
    },
    {
      path: '/explore',
      name: 'Explore',
      component: () => import('@/views/Explore.vue'),
      meta: { title: '知识探索' },
    },
    {
      path: '/ingest',
      name: 'Ingest',
      component: () => import('@/views/Ingest.vue'),
      meta: { title: '外部摄入' },
    },
    {
      path: '/code-repo',
      name: 'CodeRepo',
      component: () => import('@/views/CodeRepo.vue'),
      meta: { title: '代码仓' },
    },
    {
      path: '/timeline',
      name: 'Timeline',
      component: () => import('@/views/Timeline.vue'),
      meta: { title: '时光机' },
    },
    {
      path: '/inspiration',
      name: 'Inspiration',
      component: () => import('@/views/Inspiration.vue'),
      meta: { title: '灵感熔炉' },
    },
    {
      path: '/radar',
      name: 'Radar',
      component: () => import('@/views/Radar.vue'),
      meta: { title: '智识雷达' },
    },
  ],
})

export default router
