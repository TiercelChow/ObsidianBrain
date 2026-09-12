<template>
  <KnowledgePageShell title="知识问答" subtitle="在单本书的证据边界内提问，答案将始终可追溯">
    <div class="chat-layout">
      <aside class="chat-context knowledge-surface">
        <span class="context-label">当前知识边界</span>
        <el-select v-model="activeBaseId" placeholder="选择一本书" @change="resetSession">
          <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
        </el-select>
        <div v-if="activeBase" class="context-book">
          <div class="context-book-icon">{{ activeBase.book_kind === 'pdf' ? 'PDF' : 'MD' }}</div>
          <div><strong>{{ activeBase.book_name }}</strong><span>{{ activeBase.entry_count }} 个可检索实体</span></div>
        </div>
        <div class="runtime-state">
          <span class="knowledge-status" :class="runtimeReady ? 'is-healthy' : 'is-warning'">
            {{ runtimeReady ? 'Harness 可用' : '证据检索模式' }}
          </span>
          <p>{{ runtimeMessage }}</p>
        </div>
        <div class="boundary-note">
          <el-icon><Lock /></el-icon>
          <span>不会跨书检索，也不会直接改写原始文件。</span>
        </div>
      </aside>

      <section class="chat-panel knowledge-surface">
        <div ref="messageListRef" class="message-list">
          <div v-if="!messages.length" class="chat-welcome">
            <div class="welcome-symbol"><el-icon><ChatDotRound /></el-icon></div>
            <h2>从这本书开始思考</h2>
            <p>当前体验版已经接通数据库证据召回，并能从自然问题中提取检索词。生成式回答与自动引用会在 DeepSeek Harness ACP 适配器接入后开启。</p>
            <button v-for="question in starterQuestions" :key="question" @click="ask(question)">{{ question }}</button>
          </div>
          <template v-for="message in messages" :key="message.id">
            <div class="message" :class="message.role">
              <span class="message-role">{{ message.role === 'user' ? '你' : '知识库' }}</span>
              <p>{{ message.content }}</p>
            </div>
            <div v-if="message.evidence?.length" class="evidence-grid">
              <button
                v-for="entry in message.evidence"
                :key="entry.id"
                @click="openEntry(entry)"
              >
                <span>{{ entry.source_path || '数据库实体' }}</span>
                <strong>{{ entry.title }}</strong>
                <p>{{ entry.summary || '打开查看完整正文' }}</p>
                <em>查看来源 <el-icon><ArrowRight /></el-icon></em>
              </button>
            </div>
          </template>
          <div v-if="searching" class="searching-bubble"><i></i><i></i><i></i><span>正在检索书内证据</span></div>
        </div>

        <form class="chat-composer" @submit.prevent="ask(draft)">
          <textarea v-model="draft" :disabled="!activeBaseId" rows="1" placeholder="询问本书中的概念、章节或观点…" @keydown.enter.exact.prevent="ask(draft)"></textarea>
          <button type="submit" :disabled="!activeBaseId || !draft.trim() || searching" aria-label="发送问题">
            <el-icon><Top /></el-icon>
          </button>
        </form>
      </section>
    </div>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowRight, ChatDotRound, Lock, Top } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  getBookWikiSettings,
  listBookKnowledgeBases,
  listKnowledgeEntries,
  type KnowledgeBaseSummary,
  type KnowledgeEntrySummary,
} from '@/api/knowledge'

interface ChatMessage {
  id: number
  role: 'user' | 'assistant'
  content: string
  evidence?: KnowledgeEntrySummary[]
}

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const draft = ref('')
const messages = ref<ChatMessage[]>([])
const searching = ref(false)
const runtimeReady = ref(false)
const runtimeMessage = ref('DeepSeek Harness 尚未连接；当前只返回真实命中的书内证据，不生成答案。')
const messageListRef = ref<HTMLElement | null>(null)
let messageId = 0

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const starterQuestions = ['这本书的核心主题是什么？', '找出与架构相关的章节', '有哪些内容提到了性能优化？']

async function loadContext() {
  try {
    const [cardsResponse, settingsResponse] = await Promise.all([
      listBookKnowledgeBases(),
      getBookWikiSettings(),
    ])
    if (cardsResponse.status !== 'success' || !cardsResponse.result) throw new Error(cardsResponse.error?.message || '知识库加载失败')
    bases.value = cardsResponse.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    const requested = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requested) ? requested : (bases.value[0]?.id || '')
    const runtime = settingsResponse.result?.runtime_profiles.find(item => item.profile.runtime === 'deepseek_harness')
    runtimeReady.value = Boolean(runtime?.available)
    if (runtime?.available) {
      runtimeMessage.value = `${runtime.version || 'DeepSeek Harness'} 已检测到；ACP 对话适配器将在下一阶段接入。当前仍使用证据检索模式。`
    }
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

function resetSession() {
  messages.value = []
  router.replace({ query: { base: activeBaseId.value } })
}

async function ask(question: string) {
  const value = question.trim()
  if (!value || !activeBaseId.value || searching.value) return
  draft.value = ''
  messages.value.push({ id: ++messageId, role: 'user', content: value })
  searching.value = true
  await scrollToBottom()
  try {
    const response = await listKnowledgeEntries(activeBaseId.value, { query: value, limit: 6 })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '检索失败')
    const evidence = response.result.entries
    messages.value.push({
      id: ++messageId,
      role: 'assistant',
      content: evidence.length
        ? `在《${activeBase.value?.book_name || '当前书籍'}》中找到 ${evidence.length} 条相关证据。体验版暂不拼接生成式答案，你可以直接打开下方来源核对原文。`
        : '当前书籍知识库中没有找到足够相关的证据。可以换一个关键词，或先返回知识库页面同步来源。',
      evidence,
    })
  } catch (error) {
    messages.value.push({ id: ++messageId, role: 'assistant', content: `检索失败：${(error as Error).message}` })
  } finally {
    searching.value = false
    await scrollToBottom()
  }
}

function openEntry(entry: KnowledgeEntrySummary) {
  router.push({ path: '/knowledge/wiki', query: { base: activeBaseId.value, entry: entry.id } })
}

async function scrollToBottom() {
  await nextTick()
  messageListRef.value?.scrollTo({ top: messageListRef.value.scrollHeight, behavior: 'smooth' })
}

onMounted(loadContext)
</script>

<style scoped>
.chat-layout { min-height: 590px; display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: 12px; }
.chat-context { align-self: start; display: grid; gap: 16px; padding: 18px; }
.context-label { color: var(--text-faint); font-size: 10px; font-weight: 720; letter-spacing: .08em; }
.context-book { display: flex; align-items: center; gap: 11px; padding: 12px; border-radius: 14px; background: var(--bg-glass-subtle); }
.context-book-icon { width: 42px; height: 52px; display: grid; place-items: center; flex: none; border-radius: 9px 7px 7px 9px; background: linear-gradient(145deg, var(--accent), #4447bd); color: white; font-size: 9px; font-weight: 760; }
.context-book > div:last-child { min-width: 0; display: grid; gap: 3px; }
.context-book strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.context-book span { color: var(--text-faint); font-size: 10px; }
.runtime-state { display: grid; justify-items: start; gap: 8px; }
.runtime-state p { color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.boundary-note { display: flex; align-items: flex-start; gap: 7px; padding-top: 14px; border-top: 1px solid var(--border-faint); color: var(--text-faint); font-size: 10px; line-height: 1.5; }
.chat-panel { min-width: 0; display: flex; flex-direction: column; overflow: hidden; }
.message-list { flex: 1; max-height: calc(100vh - 290px); min-height: 480px; overflow: auto; padding: 28px clamp(16px, 5vw, 70px); }
.chat-welcome { min-height: 390px; display: grid; place-content: center; justify-items: center; gap: 10px; text-align: center; }
.welcome-symbol { width: 65px; height: 65px; display: grid; place-items: center; margin-bottom: 5px; border-radius: 22px; background: radial-gradient(circle at 32% 24%, color-mix(in srgb, white 42%, transparent), transparent 42%), var(--accent-light); color: var(--accent); font-size: 28px; box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-welcome h2 { font-size: 23px; }
.chat-welcome > p { max-width: 520px; margin-bottom: 10px; color: var(--text-muted); font-size: 13px; line-height: 1.7; }
.chat-welcome > button { width: min(100%, 380px); min-height: 40px; padding: 8px 14px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-secondary); font: inherit; font-size: 12px; cursor: pointer; transition: var(--transition-interactive); }
.chat-welcome > button:hover { border-color: var(--accent-border); color: var(--accent); transform: translateY(-1px); }
.message { max-width: 78%; display: grid; gap: 5px; margin-bottom: 16px; }
.message.user { margin-left: auto; justify-items: end; }
.message-role { color: var(--text-faint); font-size: 10px; }
.message p { padding: 11px 14px; border-radius: 16px 16px 16px 5px; background: var(--bg-glass-subtle); color: var(--text-secondary); font-size: 13px; line-height: 1.65; }
.message.user p { border-radius: 16px 16px 5px 16px; background: var(--accent); color: white; }
.evidence-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; margin: -6px 0 22px; }
.evidence-grid button { min-width: 0; display: grid; gap: 5px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); color: var(--text-primary); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.evidence-grid button:hover { border-color: var(--accent-border); transform: translateY(-1px); }
.evidence-grid span { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid strong { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid p { display: -webkit-box; overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.5; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.evidence-grid em { display: flex; align-items: center; gap: 3px; margin-top: 3px; color: var(--accent); font-size: 10px; font-style: normal; }
.searching-bubble { display: flex; align-items: center; gap: 4px; color: var(--text-faint); font-size: 11px; }
.searching-bubble i { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: bubble 800ms ease-in-out infinite alternate; }
.searching-bubble i:nth-child(2) { animation-delay: 120ms; }.searching-bubble i:nth-child(3) { animation-delay: 240ms; }.searching-bubble span { margin-left: 5px; }
.chat-composer { display: flex; align-items: flex-end; gap: 8px; margin: 0 16px 16px; padding: 9px 9px 9px 15px; border: 1px solid var(--border-subtle); border-radius: 18px; background: var(--bg-glass-strong); box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-composer:focus-within { border-color: var(--accent-border); }
.chat-composer textarea { flex: 1; min-height: 38px; max-height: 120px; padding: 8px 0; resize: none; border: 0; background: transparent; color: var(--text-primary); font: inherit; line-height: 1.5; }
.chat-composer button { width: 38px; height: 38px; display: grid; place-items: center; flex: none; border: 0; border-radius: 12px; background: var(--accent); color: white; cursor: pointer; }
.chat-composer button:disabled { opacity: .35; cursor: default; }
@keyframes bubble { to { transform: translateY(-4px); opacity: .45; } }
@media (max-width: 768px) {
  .chat-layout { min-height: 0; display: block; }
  .chat-context { margin-bottom: 10px; padding: 13px; }
  .context-label, .context-book, .boundary-note { display: none; }
  .runtime-state { display: flex; align-items: center; }
  .runtime-state p { flex: 1; }
  .message-list { min-height: calc(100dvh - 430px); max-height: none; padding: 20px 12px; }
  .chat-welcome { min-height: 360px; }
  .message { max-width: 92%; }
  .evidence-grid { grid-template-columns: 1fr; }
  .chat-composer { position: sticky; bottom: 0; margin: 0 8px 8px; }
}
</style>
