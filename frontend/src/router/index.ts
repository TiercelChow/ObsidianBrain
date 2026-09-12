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
      path: '/knowledge',
      name: 'KnowledgeBases',
      component: () => import('@/views/knowledge/KnowledgeBases.vue'),
      meta: { title: '书籍知识库' },
    },
    {
      path: '/knowledge/wiki',
      name: 'KnowledgeWiki',
      component: () => import('@/views/knowledge/WikiWorkspace.vue'),
      meta: { title: 'Wiki 工作台' },
    },
    {
      path: '/knowledge/chat',
      name: 'KnowledgeChat',
      component: () => import('@/views/knowledge/KnowledgeChat.vue'),
      meta: { title: '知识问答' },
    },
    {
      path: '/knowledge/tasks',
      name: 'KnowledgeTasks',
      component: () => import('@/views/knowledge/KnowledgeTasks.vue'),
      meta: { title: '研究任务' },
    },
    {
      path: '/knowledge/settings',
      name: 'WikiSettings',
      component: () => import('@/views/knowledge/WikiSettings.vue'),
      meta: { title: 'Wiki 配置' },
    },
    { path: '/memory', redirect: '/knowledge' },
    { path: '/wiki-dashboard', redirect: '/knowledge' },
    { path: '/wiki', redirect: '/knowledge/wiki' },
    { path: '/explore', redirect: '/knowledge/chat' },
    { path: '/ingest', redirect: '/knowledge/tasks' },
    {
      path: '/reader',
      name: 'Reader',
      component: () => import('@/views/Reader.vue'),
      meta: { title: '阅境轩' },
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
      path: '/tasks',
      name: 'Tasks',
      component: () => import('@/views/Tasks.vue'),
      meta: { title: '任务中枢' },
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
