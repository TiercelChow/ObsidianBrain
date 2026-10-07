<template>
  <div ref="pageRef" class="home-page">
    <header class="page-header">
      <div><h1 class="page-title">首页</h1><p class="page-subtitle">{{ dateLabel }}</p></div>
      <div class="header-actions">
        <el-button class="home-theme-action ui-icon-action" @click="appStore.toggleTheme()" :title="`切换主题，当前${themeName}`" :aria-label="`切换主题，当前${themeName}`"><el-icon><component :is="themeIcon" /></el-icon></el-button>
        <el-button class="ui-icon-action" @click="loadOverview()" :loading="loading" aria-label="刷新工作台" title="刷新工作台"><el-icon v-if="!loading"><Refresh /></el-icon></el-button>
      </div>
    </header>
    <div class="workbench-heading"><span>今日关注</span><small v-if="overview">{{ error ? '更新失败 · 显示上次结果' : `更新于 ${updatedLabel}` }}</small></div>
    <p v-if="error" class="home-notice" role="status">{{ error }} <button @click="loadOverview()">重试</button></p>
    <p v-else-if="systemProblem" class="home-notice" role="alert">本地服务或数据库状态异常，请刷新检查。</p>
    <div class="attention-grid" aria-label="今日关注">
      <RouterLink v-for="tile in attention" :key="tile.label" class="attention-tile glass-surface" :to="tile.to">
        <span class="attention-top"><el-icon><component :is="tile.icon" /></el-icon><el-icon class="entry-chevron"><ArrowRight /></el-icon></span>
        <strong>{{ tile.value }}</strong><span>{{ tile.label }}</span><small>{{ tile.hint }}</small>
      </RouterLink>
    </div>
    <RouterLink v-if="overview?.wiki.data?.failed_count" :to="{path:'/knowledge',query:{activity:'attention'}}" class="home-notice">有 {{ overview.wiki.data.failed_count }} 项 Wiki 工作需要检查 <el-icon><ArrowRight /></el-icon></RouterLink>
    <div class="workbench-grid">
      <div class="workbench-main">
      <HomePanel class="task-panel" title="我的任务" :subtitle="overview?.tasks.data ? `${overview.tasks.data.active_count} 项未关闭` : undefined" to="/tasks" :loading="loading" :ready="!!overview?.tasks.data" :error="overview?.tasks.error" @retry="loadOverview()">
        <div v-if="overview?.tasks.data?.items.length" class="home-list">
          <div v-for="task in shownTasks" :key="task.id" class="task-row">
            <RouterLink class="home-row" :to="{path:'/tasks',query:{view:'tasks',task:task.id}}">
              <span class="row-icon"><el-icon><component :is="task.kind === 'long' ? Flag : CircleCheck" /></el-icon></span>
              <span class="row-content"><strong>{{ task.title }}</strong><span class="row-meta"><span :class="{'is-warning':task.end_date < today}">{{ task.end_date < today ? '已逾期' : task.end_date === today ? '今天截止' : `${task.end_date} 截止` }}</span><span>{{ importanceLabel(task.importance) }}</span></span>
                <span v-if="task.kind === 'long'" class="home-progress"><span :style="{width:`${task.progress_percent}%`}"></span></span>
              </span>
              <span class="row-end"><small>{{ task.kind === 'long' ? `${task.progress_percent}%` : '待办' }}</small><el-icon><ArrowRight /></el-icon></span>
            </RouterLink>
            <RouterLink v-if="task.child_risk_id" class="child-risk" :to="{path:'/tasks',query:{view:'tasks',task:task.id,child:task.child_risk_id}}">子任务需关注：{{ task.child_risk_title }}<el-icon><ArrowRight /></el-icon></RouterLink>
          </div>
        </div>
        <div v-else class="panel-empty"><el-icon><CircleCheck /></el-icon><p>没有未关闭的任务</p><RouterLink to="/tasks">去安排下一件事</RouterLink></div>
      </HomePanel>
      <HomePanel class="reading-panel" title="最近阅读" :to="{path:'/reader',query:{view:'shelf'}}" :loading="loading" :ready="!!overview?.reading.data" :error="overview?.reading.error" @retry="loadOverview()">
        <div v-if="overview?.reading.data?.length" class="home-list">
          <RouterLink v-for="book in shownBooks" :key="book.id" class="home-row" :to="{path:'/reader',query:{book:book.id}}">
            <span class="book-spine"><el-icon><component :is="book.kind === 'pdf' ? Document : Reading" /></el-icon></span>
            <span class="row-content"><strong>{{ book.name }}</strong><span class="row-meta reading-position">{{ readingPosition(book) }}</span><small>{{ recentReadLabel(book.read_at) }}阅读 · {{ book.kind === 'pdf' ? 'PDF' : 'Markdown 文集' }}</small></span><el-icon class="entry-chevron"><ArrowRight /></el-icon>
          </RouterLink>
        </div>
        <div v-else class="panel-empty"><el-icon><Reading /></el-icon><p>还没有阅读记录</p><RouterLink :to="{path:'/reader',query:{view:'shelf'}}">从书架选一本书</RouterLink></div>
      </HomePanel>
      </div>
      <div class="workbench-side">
      <HomePanel class="wiki-panel" title="Wiki 动态" to="/knowledge" :loading="loading" :ready="!!overview?.wiki.data" :error="overview?.wiki.error" @retry="loadOverview()">
        <div v-if="overview?.wiki.data?.items.length" class="home-list">
          <RouterLink v-for="item in overview.wiki.data.items" :key="`${item.kind}-${item.id}`" class="home-row wiki-row" :to="wikiItemRoute(item)">
            <span class="row-icon" :class="{'is-warning':item.status === 'failed' || item.artifact_state === 'failed'}"><el-icon><component :is="item.kind === 'review' ? EditPen : item.kind === 'compile' ? Collection : DataAnalysis" /></el-icon></span>
            <span class="row-content"><strong>{{ item.title }}</strong><span class="row-meta">{{ item.book_name }}</span><small :class="{'is-warning':item.status === 'failed' || item.artifact_state === 'failed'}">{{ wikiStatus(item) }}</small><span v-if="item.detail && item.kind !== 'research'" class="wiki-detail">{{ item.detail }}</span></span><el-icon class="entry-chevron"><ArrowRight /></el-icon>
          </RouterLink>
        </div>
        <div v-else class="panel-empty"><el-icon><Collection /></el-icon><p>暂时没有 Wiki 动态</p><RouterLink to="/knowledge">查看书籍知识库</RouterLink></div>
      </HomePanel>
      <HomePanel class="memo-panel" title="最近小记" to="/timeline" :loading="loading" :ready="!!overview?.memos.data" :error="overview?.memos.error" @retry="loadOverview()">
        <RouterLink v-if="overview?.memos.data" class="week-link" :to="{path:'/timeline',query:{start:overview.week_start,end:today}}">本周 {{ overview.memos.data.week_count }} 条 <span>查看本周<el-icon><ArrowRight /></el-icon></span></RouterLink>
        <div v-if="overview?.memos.data?.items.length" class="home-list">
          <RouterLink v-for="memo in overview.memos.data.items" :key="memo.id" class="home-row memo-row" :to="{path:'/timeline',query:{memo:memo.id}}">
            <span class="row-content"><small>{{ memoTime(memo.timestamp) }}</small><strong class="memo-excerpt">{{ plainExcerpt(memo.excerpt) || '一段影像记录' }}</strong><span v-if="memo.tags.length" class="row-meta memo-tags">{{ memo.tags.map(tag=>`#${tag}`).join('  ') }}</span></span>
            <img v-if="memo.thumbnail && !failedImages.has(memo.id)" :src="memoImageUrl(memo.thumbnail,true)" loading="lazy" decoding="async" class="memo-thumb" alt="小记图片" @error="failedImages.add(memo.id)" /><el-icon v-else class="entry-chevron"><ArrowRight /></el-icon>
          </RouterLink>
        </div>
        <div v-else class="panel-empty"><el-icon><EditPen /></el-icon><p>还没有小记</p><RouterLink to="/timeline">记下今天的第一段想法</RouterLink></div>
      </HomePanel>
      </div>
    </div>
    <section class="home-system glass-surface" aria-label="系统概况">
      <header><h2>系统概况</h2><span v-if="overview">v{{ overview.system.version }}</span></header>
      <div v-if="overview" class="system-grid">
        <div><span>本地服务 / 数据库</span><strong :class="{'is-warning':systemProblem}">{{ systemProblem ? '状态异常' : '运行正常' }}</strong></div>
        <div><span>持续运行</span><strong>{{ formatUptime(overview.system.uptime_seconds) }}</strong></div>
        <RouterLink :to="{path:'/timeline',query:{storage:'1'}}"><span>原图存储 · 登记用量</span><strong>{{ overview.storage.data ? formatHomeBytes(overview.storage.data.originals_bytes) : '—' }}<el-icon><ArrowRight /></el-icon></strong></RouterLink>
        <RouterLink :to="{path:'/timeline',query:{storage:'1'}}"><span>图片缓存 / 上限</span><strong>{{ overview.storage.data ? `${formatHomeBytes(overview.storage.data.cache_bytes)} / ${formatHomeBytes(overview.storage.data.cache_limit_bytes)}` : '—' }}<el-icon><ArrowRight /></el-icon></strong></RouterLink>
      </div>
      <p v-else class="system-unavailable">{{ loading ? '正在读取系统状态…' : '系统状态暂时无法读取' }}</p>
      <footer><span v-if="overview?.storage.error">{{ overview.storage.error }}</span><span v-else-if="overview?.storage.data?.pending_cleanup">{{ overview.storage.data.pending_cleanup }} 张图片等待清理</span><span v-else>数据保存在本机 · 原图不参与缓存淘汰</span><RouterLink :to="{path:'/knowledge/settings',query:{section:'usage'}}">查看 AI 用量<el-icon><ArrowRight /></el-icon></RouterLink></footer>
    </section>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { ArrowRight, Refresh, Sunny, Moon, View, Calendar, Warning, Collection, EditPen, Flag, CircleCheck, Reading, Document, DataAnalysis } from '@element-plus/icons-vue'
import { getHomeOverview, type HomeOverview } from '@/api/home'
import HomePanel from '@/components/home/HomePanel.vue'
import { useAppStore } from '@/stores/app'
import { usePhoneViewport } from '@/composables/usePhoneViewport'
import { todayLocal } from '@/utils/taskDates'
import { memoImageUrl } from '@/utils/timelineMemo'
import { readingPosition, wikiItemRoute, wikiStatus, mergeHomeOverview, shouldPollHome, formatHomeBytes, formatUptime, recentReadLabel, homeSnapshot, saveHomeSnapshot } from '@/utils/homeWorkbench'

const appStore = useAppStore()
const { isMobile } = usePhoneViewport()
const themeName = computed(() => ({light:'浅色',dark:'深色','eye-care':'护眼'}[appStore.theme]))
const themeIcon = computed(() => appStore.theme === 'dark' ? Moon : appStore.theme === 'eye-care' ? View : Sunny)
const snapshot = homeSnapshot()
const overview = ref<HomeOverview|null>(snapshot.data)
const loading = ref(!snapshot.data)
const error = ref('')
const today = ref(todayLocal())
const pageRef = ref<HTMLElement|null>(null)
const failedImages = reactive(new Set<string>())
let controller: AbortController | null = null
let poll: ReturnType<typeof setTimeout> | undefined
let active = true
let request = 0
const dateLabel = computed(() => new Date(`${today.value}T12:00:00`).toLocaleDateString('zh-CN',{month:'long',day:'numeric',weekday:'long'}))
const updatedLabel = computed(() => overview.value ? new Date(overview.value.generated_at).toLocaleTimeString('zh-CN',{hour:'2-digit',minute:'2-digit'}) : '')
const shownTasks = computed(() => overview.value?.tasks.data?.items.slice(0,isMobile.value ? 3 : 4) || [])
const shownBooks = computed(() => overview.value?.reading.data?.slice(0,isMobile.value ? 3 : 4) || [])
const systemProblem = computed(() => !!overview.value && Object.values(overview.value.system.components).some(status => status !== 'ok'))
const attention = computed(() => {
  const data = overview.value
  return [
    {label:'今日安排',icon:Calendar,value:data?.tasks.error ? '—' : data?.tasks.data?.today_count ?? '—',hint:'顶层任务 · 今日开始或截止',to:{path:'/tasks',query:{focus:'today'}}},
    {label:'已逾期',icon:Warning,value:data?.tasks.error ? '—' : data?.tasks.data?.overdue_count ?? '—',hint:'顶层任务 · 尚未关闭',to:{path:'/tasks',query:{focus:'overdue'}}},
    {label:'Wiki 运行中',icon:Collection,value:data?.wiki.error ? '—' : data?.wiki.data?.running_count ?? '—',hint:'编译与研究 · 含排队',to:{path:'/knowledge',query:{activity:'running'}}},
    {label:'等待审核',icon:EditPen,value:data?.wiki.error ? '—' : data?.wiki.data?.review_count ?? '—',hint:'知识候选 · 由你决定',to:{path:'/knowledge/wiki',query:{pending:'1'}}},
  ]
})
function importanceLabel(value:string) { return ({urgent:'紧急',high:'重要',normal:'普通',low:'低优先'} as Record<string,string>)[value] || '普通' }
function memoTime(value:string) { return new Date(value).toLocaleString('zh-CN',{month:'numeric',day:'numeric',hour:'2-digit',minute:'2-digit'}) }
function plainExcerpt(value:string) { return value.replace(/!\[[^\]]*\]\([^)]*\)|!\[\[[^\]]*\]\]/g,'').replace(/\[([^\]]+)\]\([^)]*\)/g,'$1').replace(/[#>*`_~]/g,'').replace(/\s+/g,' ').trim() }
function schedulePoll() {
  clearTimeout(poll)
  if (active && shouldPollHome(overview.value, document.visibilityState === 'visible')) poll = setTimeout(() => void loadOverview(true), 15000)
}
async function loadOverview(quiet = false) {
  if (!active || document.visibilityState !== 'visible') return
  const sequence = ++request
  controller?.abort(); clearTimeout(poll)
  controller = new AbortController()
  if (!quiet) loading.value = true
  today.value = todayLocal()
  try {
    const fresh = await getHomeOverview(today.value, controller.signal)
    if (!active || sequence !== request) return
    overview.value = mergeHomeOverview(overview.value, fresh)
    saveHomeSnapshot(overview.value); error.value = ''
  } catch {
    if (active && sequence === request) error.value = '工作台暂时无法更新，请检查本地服务。'
  } finally {
    if (active && sequence === request) { loading.value = false; if (!error.value) schedulePoll() }
  }
}
function visibilityChanged() {
  if (document.visibilityState === 'visible') void loadOverview(true)
  else { ++request; controller?.abort(); clearTimeout(poll); loading.value = false }
}
function readingSaved() { void loadOverview(true) }
onBeforeRouteLeave(() => saveHomeSnapshot(overview.value, pageRef.value?.closest('.app-main')?.scrollTop ?? 0))
onMounted(async () => {
  await nextTick()
  const main = pageRef.value?.closest('.app-main')
  if (main) main.scrollTop = snapshot.scroll
  document.addEventListener('visibilitychange', visibilityChanged)
  window.addEventListener('reader-progress-saved', readingSaved)
  void loadOverview()
})
onBeforeUnmount(() => { active = false; ++request; controller?.abort(); clearTimeout(poll); document.removeEventListener('visibilitychange', visibilityChanged); window.removeEventListener('reader-progress-saved', readingSaved) })
</script>
<style scoped>
.home-page { min-height:100%; max-width:100%; color:var(--text-primary); }
.workbench-heading { display:flex; justify-content:space-between; gap:12px; align-items:center; margin:0 0 12px; font-size:14px; font-weight:600; }
.workbench-heading small { font-size:12px; font-weight:400; color:var(--text-muted); }
.attention-grid { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:14px; margin-bottom:20px; }
.attention-tile { display:flex; flex-direction:column; padding:16px 18px; border-radius:18px; text-decoration:none; color:inherit; min-width:0; gap:5px; }
.attention-top { display:flex; justify-content:space-between; font-size:18px; color:var(--text-muted); margin-bottom:2px; }
.attention-tile strong { font-size:28px; font-weight:720; line-height:1.2; font-variant-numeric:tabular-nums; letter-spacing:-.03em; }
.attention-tile > span:not(.attention-top) { font-size:14px; font-weight:600; }
.attention-tile small { font-size:11px; color:var(--text-muted); line-height:1.5; }
.entry-chevron { flex-shrink:0; color:var(--text-faint); font-size:13px; }
.workbench-grid { display:grid; grid-template-columns:minmax(0,1.55fr) minmax(0,1fr); gap:18px; align-items:stretch; margin-bottom:18px; }
.workbench-main,.workbench-side { min-width:0; display:flex; flex-direction:column; gap:18px; }
.workbench-main > .home-panel,.workbench-side > .home-panel { flex:1 1 auto; }
.task-panel { grid-area:tasks; } .reading-panel { grid-area:reading; } .wiki-panel { grid-area:wiki; } .memo-panel { grid-area:memos; }
.home-list { display:grid; gap:3px; }
.home-row { display:flex; align-items:center; gap:12px; min-height:76px; padding:12px 8px; border-radius:12px; text-decoration:none; color:inherit; min-width:0; transition:background var(--duration-fast) var(--ease-out),transform var(--duration-fast) var(--ease-out); }
.home-row:hover,.attention-tile:hover { background-color:var(--bg-hover); }
.home-row:active,.attention-tile:active { transform:scale(.985); }
.home-row:focus-visible,.attention-tile:focus-visible,.child-risk:focus-visible { outline:2px solid var(--text-primary); outline-offset:3px; }
.row-icon { flex:none; display:grid; place-items:center; width:36px; height:36px; color:var(--text-tertiary); font-size:20px; }
.row-content { display:flex; flex:1; min-width:0; flex-direction:column; gap:5px; }
.row-content strong { font-size:15px; font-weight:600; line-height:1.45; overflow:hidden; text-overflow:ellipsis; display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:2; overflow-wrap:anywhere; }
.row-meta { display:flex; gap:10px; font-size:12px; line-height:1.5; color:var(--text-muted); }
.row-content small { color:var(--text-muted); font-size:12px; line-height:1.5; }
.row-end { display:flex; gap:10px; align-items:center; color:var(--text-muted); flex:none; font-variant-numeric:tabular-nums; }
.row-end small { font-size:12px; }
.home-progress { height:3px; border-radius:3px; background:var(--bg-hover); margin-top:2px; overflow:hidden; }
.home-progress > span { display:block; height:100%; background:var(--text-tertiary); border-radius:inherit; }
.child-risk { display:flex; align-items:center; justify-content:space-between; gap:8px; padding:4px 8px 4px 56px; min-height:44px; color:var(--glass-warning-label,var(--text-secondary)); font-size:12px; text-decoration:none; overflow-wrap:anywhere; }
.is-warning { color:var(--glass-warning-label,var(--text-secondary))!important; }
.book-spine { flex:none; display:grid; place-items:center; width:40px; height:50px; border-radius:6px 11px 11px 6px; background:var(--bg-hover); color:var(--text-secondary); box-shadow:inset 3px 0 0 var(--border-subtle); font-size:22px; }
.reading-position,.memo-tags { overflow:hidden; white-space:nowrap; text-overflow:ellipsis; display:block; }
.wiki-row { align-items:flex-start; } .wiki-row > .entry-chevron { align-self:center; }
.wiki-detail { font-size:12px; color:var(--text-muted); line-height:1.5; display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:2; overflow:hidden; overflow-wrap:anywhere; }
.week-link { display:flex; justify-content:space-between; gap:8px; align-items:center; min-height:44px; padding:0 8px; font-size:12px; color:var(--text-muted); text-decoration:none; }
.week-link > span { display:flex; align-items:center; gap:3px; }
.memo-thumb { width:56px; height:56px; object-fit:cover; border-radius:12px; flex:none; }
.panel-empty { display:flex; flex-direction:column; align-items:flex-start; justify-content:center; padding:20px 8px; min-height:116px; color:var(--text-muted); }
.panel-empty > .el-icon { font-size:25px; margin-bottom:4px; }
.panel-empty p { font-size:14px; margin:8px 0 0; }
.panel-empty a { display:inline-flex; align-items:center; min-height:44px; color:var(--text-secondary); font-size:13px; text-decoration:none; }
.home-notice { display:flex; align-items:center; justify-content:space-between; gap:12px; margin:0 0 16px; font-size:13px; color:var(--text-secondary); min-height:44px; text-decoration:none; }
.home-notice button { background:none; border:0; font:inherit; color:inherit; min-height:44px; cursor:pointer; }
.home-system { padding:18px 20px; border-radius:18px; }
.home-system header { display:flex; align-items:center; gap:10px; margin-bottom:14px; }
.home-system h2 { font-size:15px; margin:0; font-weight:650; }
.home-system header span { font-size:12px; color:var(--text-muted); }
.system-grid { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:18px; }
.system-grid > * { display:grid; gap:7px; text-decoration:none; color:inherit; min-width:0; min-height:44px; }
.system-grid span { color:var(--text-muted); font-size:12px; line-height:1.5; }
.system-grid strong { font-size:13px; font-weight:550; display:flex; gap:6px; align-items:center; flex-wrap:wrap; overflow-wrap:anywhere; }
.home-system footer { display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:8px; margin-top:12px; font-size:11px; color:var(--text-muted); }
.home-system footer a { display:flex; align-items:center; gap:4px; min-height:44px; text-decoration:none; color:var(--text-secondary); font-size:12px; }
.system-unavailable { font-size:13px; color:var(--text-muted); }
@media(max-width:1080px) { .workbench-grid { grid-template-columns:minmax(0,1.2fr) minmax(0,1fr); } .system-grid { grid-template-columns:repeat(2,minmax(0,1fr)); } }
@media (max-width: 768px) {
  .home-page .header-actions .home-theme-action { display: inline-flex; position: fixed; left: max(12px, var(--safe-left)); top:calc(var(--safe-top) + (var(--mobile-header-height) - var(--tap-target)) / 2); width: var(--tap-target); padding:0; }
  .workbench-heading { font-size:14px; margin-bottom:10px; } .workbench-heading small { font-size:11px; }
  .attention-grid { grid-template-columns:repeat(2,minmax(0,1fr)); gap:10px; margin-bottom:14px; }
  .attention-tile { padding:14px; border-radius:16px; } .attention-tile strong { font-size:27px; } .attention-tile small { font-size:10px; }
  .attention-tile { position:relative; gap:4px; }
  .attention-top { position:absolute; top:14px; right:14px; }
  .attention-top .entry-chevron { display:none; }
  .workbench-grid { grid-template-columns:minmax(0,1fr); grid-template-areas:'tasks' 'reading' 'wiki' 'memos'; gap:14px; }
  .workbench-main,.workbench-side { display:contents; }
  .home-row { padding:12px 2px; gap:10px; } .row-content strong { font-size:15px; } .child-risk { padding-left:48px; }
  .home-system { padding:16px; } .system-grid { gap:16px 10px; }
}
@media(prefers-reduced-motion:reduce) { .home-row,.attention-tile { transition:none; transform:none!important; } }
</style>
