<template>
  <KnowledgePageShell title="知识问答" subtitle="在单本书的证据边界内提问，答案与来源会持续保留">
    <div class="chat-layout">
      <aside class="chat-context knowledge-surface">
        <span class="context-label">当前知识边界</span>
        <el-select
          v-model="activeBaseId"
          class="knowledge-select is-fluid"
          popper-class="system-select-popper"
          placement="bottom-start"
          :offset="0"
          :fit-input-width="true"
          :disabled="searching"
          placeholder="选择一本书"
          @change="changeKnowledgeBase"
        >
          <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
        </el-select>
        <div v-if="activeBase" class="context-book">
          <div class="context-book-icon">{{ activeBase.book_kind === 'pdf' ? 'PDF' : 'MD' }}</div>
          <div><strong>{{ activeBase.book_name }}</strong><span>{{ activeBase.entry_count }} 个可检索实体</span></div>
        </div>

        <section class="conversation-history" aria-label="问答历史">
          <header>
            <span>历史问答</span>
            <button type="button" :disabled="!activeBaseId || searching" @click="newConversation()">
              <el-icon><Plus /></el-icon><span>新对话</span>
            </button>
          </header>
          <div class="conversation-list">
            <button
              v-for="conversation in conversations"
              :key="conversation.id"
              type="button"
              :class="{ active: conversation.id === activeConversationId }"
              :disabled="searching"
              @click="openConversation(conversation.id)"
            >
              <strong>{{ conversation.title }}</strong>
              <span>{{ conversation.message_count }} 条消息 · {{ formatConversationTime(conversation.updated_at) }}</span>
            </button>
            <p v-if="historyLoading">正在恢复历史…</p>
            <p v-else-if="!conversations.length">首次提问后，会话会自动保存在本地数据库。</p>
          </div>
        </section>

        <div class="runtime-state">
          <span class="knowledge-status" :class="runtimeReady ? 'is-healthy' : 'is-warning'">
            {{ runtimeReady ? 'Harness 启动器可用' : '证据检索模式' }}
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
          <div v-if="!messages.length && !historyLoading" class="chat-welcome">
            <div class="welcome-symbol"><el-icon><ChatDotRound /></el-icon></div>
            <h2>从这本书开始思考</h2>
            <p>问题会先在当前书籍的数据库实体中召回证据，再交给 DeepSeek Harness 生成带来源编号的回答。</p>
            <button v-for="question in starterQuestions" :key="question" @click="ask(question)">{{ question }}</button>
          </div>
          <div v-else-if="historyLoading && !messages.length" class="history-loading">
            <el-icon class="is-loading"><Loading /></el-icon><span>正在恢复历史会话</span>
          </div>
          <template v-for="message in messages" :key="message.id">
            <div class="message" :class="message.role">
              <span class="message-role">{{ message.role === 'user' ? '你' : '知识库' }}</span>
              <p v-if="message.role === 'user'">{{ message.content }}</p>
              <KnowledgeAnswerMarkdown
                v-else
                :content="message.content"
                :evidence-count="message.evidence?.length ?? 0"
                :streaming="streamingMessageId === message.id"
                @citation="sourceIndex => previewEvidence(message, sourceIndex)"
              />
            </div>
            <div v-if="message.evidence?.length" class="evidence-grid">
              <button
                v-for="(entry, evidenceIndex) in message.evidence"
                :key="entry.id"
                type="button"
                @click="previewSource(entry)"
              >
                <span><b>S{{ evidenceIndex + 1 }}</b>{{ entry.source_path || '数据库实体' }}</span>
                <strong>{{ entry.title }}</strong>
                <p>{{ entry.summary || '预览完整正文与引用' }}</p>
                <em>预览来源 <el-icon><ArrowRight /></el-icon></em>
              </button>
            </div>
          </template>
          <div v-if="searching && !streamingMessageId" class="searching-bubble" role="status" aria-live="polite">
            <i></i><i></i><i></i><span>{{ thinkingText }}<b aria-hidden="true"></b></span>
          </div>
        </div>

        <form class="chat-composer" @submit.prevent="ask(draft)">
          <textarea v-model="draft" :disabled="!activeBaseId" rows="1" placeholder="询问本书中的概念、章节或观点…" @keydown.enter.exact.prevent="ask(draft)"></textarea>
          <button type="submit" :disabled="!activeBaseId || !draft.trim() || searching" aria-label="发送问题">
            <el-icon><Top /></el-icon>
          </button>
        </form>
      </section>
    </div>

    <MotionModal v-model="sourceVisible" aria-label="来源预览" size="wide">
      <div class="knowledge-modal-card source-preview-modal">
        <header class="source-preview-head">
          <div>
            <span>{{ sourceDetail?.entry_type === 'source_section' ? '来源章节' : '知识实体' }}</span>
            <h3>{{ sourceDetail?.title || '来源预览' }}</h3>
            <p>{{ sourceDetail?.source_path || '数据库实体' }}</p>
          </div>
          <button type="button" aria-label="关闭来源预览" @click="sourceVisible = false">
            <el-icon><Close /></el-icon>
          </button>
        </header>
        <div class="source-preview-body">
          <div v-if="sourceLoading" class="source-loading">
            <el-icon class="is-loading"><Loading /></el-icon><span>正在读取来源</span>
          </div>
          <template v-else-if="sourceDetail">
            <div ref="sourceMarkdownRef" class="source-markdown markdown-body" v-html="sourceHtml"></div>
            <section v-if="sourceDetail.citations.length" class="source-citations">
              <h4>原文引用</h4>
              <article v-for="citation in sourceDetail.citations" :key="citation.id">
                <strong>{{ citation.source_path }}</strong>
                <span v-if="citation.line_start">第 {{ citation.line_start }}–{{ citation.line_end || citation.line_start }} 行</span>
                <p v-if="citation.quote_text">{{ citation.quote_text }}</p>
              </article>
            </section>
          </template>
        </div>
        <footer class="knowledge-modal-actions">
          <el-button @click="sourceVisible = false">关闭</el-button>
          <el-button v-if="sourceDetail" type="primary" @click="openSourceWorkspace">在 Wiki 工作台打开</el-button>
        </footer>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowRight, ChatDotRound, Close, Loading, Lock, Plus, Top } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { useTypewriterLoop } from '@/composables/useTypewriterLoop'
import {
  askBookKnowledge,
  getBookWikiSettings,
  getKnowledgeConversation,
  getKnowledgeEntry,
  listBookKnowledgeBases,
  listKnowledgeConversations,
  listKnowledgeEntries,
  type KnowledgeBaseSummary,
  type KnowledgeConversationSummary,
  type KnowledgeEntryDetail,
  type KnowledgeEntrySummary,
} from '@/api/knowledge'

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  evidence?: KnowledgeEntrySummary[]
}

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const conversations = ref<KnowledgeConversationSummary[]>([])
const activeConversationId = ref('')
const draft = ref('')
const messages = ref<ChatMessage[]>([])
const searching = ref(false)
const streamingMessageId = ref('')
const historyLoading = ref(false)
const runtimeReady = ref(false)
const runtimeMessage = ref('DeepSeek Harness 尚未连接；当前只返回真实命中的书内证据。')
const messageListRef = ref<HTMLElement | null>(null)
const sourceVisible = ref(false)
const sourceLoading = ref(false)
const sourceDetail = ref<KnowledgeEntryDetail | null>(null)
const sourceHtml = ref('')
const sourceMarkdownRef = ref<HTMLElement | null>(null)
let localMessageId = 0
let historyRequestId = 0
let sourceRequestId = 0
let revealFrame = 0

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const starterQuestions = ['这本书的核心主题是什么？', '找出与架构相关的章节', '有哪些内容提到了性能优化？']
const { text: thinkingText } = useTypewriterLoop(searching, () => runtimeReady.value
  ? ['正在检索书内证据', '正在梳理关键线索', '正在生成可追溯回答']
  : ['正在检索书内证据', '正在整理匹配结果'])
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

async function loadContext() {
  try {
    const [cardsResponse, settingsResponse] = await Promise.all([
      listBookKnowledgeBases(),
      getBookWikiSettings(),
    ])
    if (cardsResponse.status !== 'success' || !cardsResponse.result) throw new Error(cardsResponse.error?.message || '知识库加载失败')
    bases.value = cardsResponse.result.items.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    const requestedBase = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requestedBase) ? requestedBase : (bases.value[0]?.id || '')
    const runtime = settingsResponse.result?.runtime_profiles.find(item => item.profile.runtime === 'deepseek_harness')
    runtimeReady.value = Boolean(runtime?.available)
    if (runtime?.available) {
      runtimeMessage.value = `${runtime.version || '本地启动器'} 可用；ACP 会话与模型凭据将在问答时验证。`
    } else if (runtime) {
      runtimeMessage.value = runtime.message
    }
    if (activeBaseId.value) await loadConversations(String(route.query.conversation || ''))
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function loadConversations(preferredConversationId = '') {
  const requestId = ++historyRequestId
  historyLoading.value = true
  try {
    const response = await listKnowledgeConversations(activeBaseId.value)
    if (requestId !== historyRequestId) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '历史会话加载失败')
    conversations.value = response.result.conversations
    const selected = conversations.value.find(item => item.id === preferredConversationId)
      || conversations.value.find(item => item.id === activeConversationId.value)
      || conversations.value[0]
    if (selected) await openConversation(selected.id, requestId)
    else newConversation(false)
  } catch (error) {
    if (requestId === historyRequestId) ElMessage.error((error as Error).message)
  } finally {
    if (requestId === historyRequestId) historyLoading.value = false
  }
}

async function refreshConversationList() {
  const response = await listKnowledgeConversations(activeBaseId.value)
  if (response.status === 'success' && response.result) conversations.value = response.result.conversations
}

async function openConversation(conversationId: string, parentRequestId?: number) {
  if (conversationId === activeConversationId.value && messages.value.length) return
  const requestId = parentRequestId ?? ++historyRequestId
  historyLoading.value = true
  try {
    const response = await getKnowledgeConversation(conversationId)
    if (requestId !== historyRequestId) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '会话读取失败')
    if (response.result.knowledge_base_id !== activeBaseId.value) throw new Error('该会话不属于当前知识库')
    activeConversationId.value = response.result.id
    messages.value = response.result.messages.map(message => ({
      id: message.id,
      role: message.role,
      content: message.content,
      evidence: message.evidence,
    }))
    replaceChatQuery()
    await scrollToBottom(false)
  } catch (error) {
    if (requestId === historyRequestId) ElMessage.error((error as Error).message)
  } finally {
    if (requestId === historyRequestId) historyLoading.value = false
  }
}

async function changeKnowledgeBase() {
  ++historyRequestId
  activeConversationId.value = ''
  conversations.value = []
  messages.value = []
  replaceChatQuery()
  await loadConversations()
}

function newConversation(updateRoute = true) {
  ++historyRequestId
  activeConversationId.value = ''
  messages.value = []
  historyLoading.value = false
  if (updateRoute) replaceChatQuery()
}

function replaceChatQuery() {
  router.replace({
    query: {
      base: activeBaseId.value || undefined,
      conversation: activeConversationId.value || undefined,
    },
  })
}

async function ask(question: string) {
  const value = question.trim()
  if (!value || !activeBaseId.value || searching.value) return
  draft.value = ''
  messages.value.push({ id: `local-${++localMessageId}`, role: 'user', content: value })
  searching.value = true
  await scrollToBottom()
  try {
    if (runtimeReady.value) {
      const response = await askBookKnowledge(activeBaseId.value, value, activeConversationId.value || undefined)
      if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Harness 回答失败')
      activeConversationId.value = response.result.conversation_id
      await revealAssistantAnswer(
        `run-${response.result.run_id}`,
        response.result.answer,
        response.result.evidence,
      )
      replaceChatQuery()
      await refreshConversationList()
      return
    }
    await appendEvidenceFallback(value)
  } catch (error) {
    const detail = (error as Error).message
    if (runtimeReady.value) {
      runtimeReady.value = false
      runtimeMessage.value = 'Harness 本次调用失败，本次会话已切换为书内证据检索模式。'
      try {
        await appendEvidenceFallback(value, detail)
      } catch {
        messages.value.push({ id: `local-${++localMessageId}`, role: 'assistant', content: `Harness 调用失败：${detail}` })
      }
    } else {
      messages.value.push({ id: `local-${++localMessageId}`, role: 'assistant', content: `检索失败：${detail}` })
    }
  } finally {
    searching.value = false
    await scrollToBottom()
  }
}

async function revealAssistantAnswer(id: string, answer: string, evidence: KnowledgeEntrySummary[]) {
  window.cancelAnimationFrame(revealFrame)
  messages.value.push({ id, role: 'assistant', content: '', evidence })
  const message = messages.value[messages.value.length - 1]
  streamingMessageId.value = id
  const characters = Array.from(answer)
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  if (reduceMotion || characters.length < 12) {
    message.content = answer
    streamingMessageId.value = ''
    return
  }

  await new Promise<void>((resolve) => {
    let cursor = 0
    let frameCount = 0
    const reveal = () => {
      const remaining = characters.length - cursor
      const batchSize = Math.min(28, Math.max(2, Math.ceil(remaining / 72)))
      cursor = Math.min(characters.length, cursor + batchSize)
      message.content = characters.slice(0, cursor).join('')
      frameCount += 1
      if (frameCount % 5 === 0) void scrollToBottom(false)
      if (cursor < characters.length) {
        revealFrame = window.requestAnimationFrame(reveal)
      } else {
        streamingMessageId.value = ''
        resolve()
      }
    }
    revealFrame = window.requestAnimationFrame(reveal)
  })
}

async function appendEvidenceFallback(question: string, harnessError = '') {
  const response = await listKnowledgeEntries(activeBaseId.value, { query: question, limit: 6 })
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '检索失败')
  const evidence = response.result.entries
  const prefix = harnessError ? `Harness 暂时不可用：${harnessError}\n\n` : ''
  messages.value.push({
    id: `local-${++localMessageId}`,
    role: 'assistant',
    content: evidence.length
      ? `${prefix}在《${activeBase.value?.book_name || '当前书籍'}》中找到 ${evidence.length} 条相关证据。你可以在下方弹窗中核对原文。`
      : `${prefix}当前书籍知识库中没有找到足够相关的证据。可以换一个关键词，或先返回知识库页面同步来源。`,
    evidence,
  })
}

async function previewSource(entry: KnowledgeEntrySummary) {
  const requestId = ++sourceRequestId
  cleanup()
  sourceVisible.value = true
  sourceLoading.value = true
  sourceDetail.value = null
  sourceHtml.value = ''
  try {
    const response = await getKnowledgeEntry(entry.id)
    if (requestId !== sourceRequestId) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '来源读取失败')
    sourceDetail.value = response.result
    sourceHtml.value = await renderMarkdown(response.result.content_md, undefined, entry.id)
    await nextTick()
    if (sourceMarkdownRef.value) await enhance(sourceMarkdownRef.value)
  } catch (error) {
    if (requestId === sourceRequestId) {
      sourceVisible.value = false
      ElMessage.error((error as Error).message)
    }
  } finally {
    if (requestId === sourceRequestId) sourceLoading.value = false
  }
}

function previewEvidence(message: ChatMessage, sourceIndex: number) {
  const entry = message.evidence?.[sourceIndex]
  if (entry) void previewSource(entry)
}

function openSourceWorkspace() {
  if (!sourceDetail.value) return
  router.push({ path: '/knowledge/wiki', query: { base: activeBaseId.value, entry: sourceDetail.value.id } })
}

function formatConversationTime(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const today = new Date()
  return date.toDateString() === today.toDateString()
    ? new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' }).format(date)
    : new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric' }).format(date)
}

async function scrollToBottom(smooth = true) {
  await nextTick()
  messageListRef.value?.scrollTo({
    top: messageListRef.value.scrollHeight,
    behavior: smooth ? 'smooth' : 'auto',
  })
}

watch(sourceVisible, (visible) => {
  if (!visible) {
    ++sourceRequestId
    cleanup()
  }
})

onMounted(loadContext)
onBeforeUnmount(() => {
  ++historyRequestId
  ++sourceRequestId
  window.cancelAnimationFrame(revealFrame)
  cleanup()
})
</script>

<style scoped>
.chat-layout { min-height: 590px; display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 12px; }
.chat-context { align-self: start; display: grid; gap: 14px; padding: 18px; }
.context-label { color: var(--text-faint); font-size: 10px; font-weight: 720; letter-spacing: .08em; }
.context-book { display: flex; align-items: center; gap: 11px; padding: 12px; border-radius: 14px; background: var(--bg-glass-subtle); }
.context-book-icon { width: 42px; height: 52px; display: grid; place-items: center; flex: none; border-radius: 9px 7px 7px 9px; background: linear-gradient(145deg, var(--accent), #4447bd); color: white; font-size: 9px; font-weight: 760; }
.context-book > div:last-child { min-width: 0; display: grid; gap: 3px; }
.context-book strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.context-book span { color: var(--text-faint); font-size: 10px; }
.conversation-history { min-width: 0; display: grid; gap: 7px; padding-top: 2px; }
.conversation-history header { display: flex; align-items: center; justify-content: space-between; gap: 8px; color: var(--text-faint); font-size: 10px; font-weight: 720; }
.conversation-history header > button { display: inline-flex; align-items: center; gap: 3px; padding: 4px 6px; border: 0; border-radius: 7px; background: transparent; color: var(--accent); font: inherit; font-size: 10px; cursor: pointer; }
.conversation-history header > button:hover { background: var(--accent-light); }
.conversation-list { max-height: 208px; display: grid; gap: 4px; overflow: auto; }
.conversation-list > button { min-width: 0; display: grid; gap: 2px; padding: 8px 9px; border: 0; border-radius: 10px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; transition: background var(--motion-fast) var(--ease-emphasized), box-shadow var(--motion-fast) var(--ease-emphasized); }
.conversation-list > button:hover { background: var(--bg-hover); }
.conversation-list > button.active { background: var(--accent-light); box-shadow: inset 0 0 0 1px var(--accent-border); }
.conversation-list strong { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.conversation-list span, .conversation-list > p { color: var(--text-faint); font-size: 9px; line-height: 1.45; }
.conversation-list > p { padding: 8px 2px; }
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
.history-loading { min-height: 320px; display: grid; place-content: center; justify-items: center; gap: 9px; color: var(--text-faint); font-size: 12px; }
.message { max-width: 78%; display: grid; gap: 5px; margin-bottom: 16px; }
.message.user { margin-left: auto; justify-items: end; }
.message-role { color: var(--text-faint); font-size: 10px; }
.message > p { padding: 11px 14px; border-radius: 16px 16px 16px 5px; background: var(--bg-glass-subtle); color: var(--text-secondary); font-size: 13px; line-height: 1.65; white-space: pre-wrap; }
.message.user p { border-radius: 16px 16px 5px 16px; background: var(--accent); color: white; }
.evidence-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; margin: -6px 0 22px; }
.evidence-grid button { min-width: 0; display: grid; gap: 5px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); color: var(--text-primary); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.evidence-grid button:hover { border-color: var(--accent-border); transform: translateY(-1px); }
.evidence-grid span { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid span b { display: inline-flex; margin-right: 7px; padding: 2px 5px; border-radius: 5px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.evidence-grid strong { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid p { display: -webkit-box; overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.5; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.evidence-grid em { display: flex; align-items: center; gap: 3px; margin-top: 3px; color: var(--accent); font-size: 10px; font-style: normal; }
.searching-bubble { min-height: 22px; display: flex; align-items: center; gap: 4px; color: var(--text-faint); font-size: 11px; }
.searching-bubble i { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: bubble 800ms ease-in-out infinite alternate; }
.searching-bubble i:nth-child(2) { animation-delay: 120ms; }
.searching-bubble i:nth-child(3) { animation-delay: 240ms; }
.searching-bubble span { min-width: 12.5em; margin-left: 5px; }
.searching-bubble span b { display: inline-block; width: 1px; height: 1em; margin-left: 2px; background: currentColor; vertical-align: -.12em; animation: caret 700ms step-end infinite; }
.chat-composer { display: flex; align-items: flex-end; gap: 8px; margin: 0 16px 16px; padding: 9px 9px 9px 15px; border: 1px solid var(--border-subtle); border-radius: 18px; background: var(--bg-glass-strong); box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-composer:focus-within { border-color: var(--accent-border); }
.chat-composer textarea { flex: 1; min-height: 38px; max-height: 120px; padding: 8px 0; resize: none; border: 0; background: transparent; color: var(--text-primary); font: inherit; line-height: 1.5; }
.chat-composer button { width: 38px; height: 38px; display: grid; place-items: center; flex: none; border: 0; border-radius: 12px; background: var(--accent); color: white; cursor: pointer; }
.chat-composer button:disabled { opacity: .35; cursor: default; }
.source-preview-modal { width: 100%; max-height: calc(100dvh - 48px); display: flex; flex-direction: column; }
.source-preview-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; padding: 24px 24px 15px; border-bottom: 1px solid var(--border-faint); }
.source-preview-head > div { min-width: 0; }
.source-preview-head span { color: var(--accent); font-size: 10px; font-weight: 720; letter-spacing: .05em; }
.source-preview-head h3 { margin: 5px 0 4px; font-size: 22px; }
.source-preview-head p { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.source-preview-head > button { width: 34px; height: 34px; display: grid; place-items: center; flex: none; border: 0; border-radius: 10px; background: var(--bg-glass-subtle); color: var(--text-muted); cursor: pointer; }
.source-preview-body { min-height: 0; flex: 1; overflow: auto; padding: 24px; overscroll-behavior: contain; }
.source-loading { min-height: 260px; display: grid; place-content: center; justify-items: center; gap: 9px; color: var(--text-faint); font-size: 12px; }
.source-markdown { color: var(--text-secondary); font-size: 14px; line-height: 1.8; overflow-wrap: anywhere; }
.source-markdown :deep(.table-scroll), .source-markdown :deep(.katex-display), .source-markdown :deep(.mermaid), .source-markdown :deep(pre) { max-width: 100%; overflow-x: auto; }
.source-markdown :deep(table) { min-width: max-content; border-collapse: collapse; }
.source-markdown :deep(th), .source-markdown :deep(td) { padding: 8px 11px; border: 1px solid var(--border-faint); }
.source-citations { display: grid; gap: 8px; margin-top: 28px; padding-top: 20px; border-top: 1px solid var(--border-faint); }
.source-citations h4 { margin: 0 0 3px; font-size: 14px; }
.source-citations article { display: grid; gap: 4px; padding: 12px; border-radius: 12px; background: var(--bg-glass-subtle); }
.source-citations strong { font-size: 11px; }
.source-citations span { color: var(--text-faint); font-size: 9px; }
.source-citations p { color: var(--text-muted); font-size: 11px; line-height: 1.6; }
@keyframes bubble { to { transform: translateY(-4px); opacity: .45; } }
@keyframes caret { 50% { opacity: 0; } }
@media (max-width: 768px) {
  .chat-layout { min-height: 0; display: block; }
  .chat-context { margin-bottom: 10px; padding: 13px; }
  .context-label, .context-book, .boundary-note, .runtime-state p { display: none; }
  .runtime-state { display: flex; align-items: center; }
  .conversation-history { grid-row: 3; }
  .conversation-history header > button span { display: none; }
  .conversation-list { max-height: none; display: flex; gap: 6px; overflow-x: auto; padding-bottom: 2px; }
  .conversation-list > button { width: 168px; flex: none; }
  .conversation-list > p { min-width: 220px; }
  .message-list { min-height: calc(100dvh - 430px); max-height: none; padding: 20px 12px; }
  .chat-welcome { min-height: 330px; }
  .message { max-width: 92%; }
  .evidence-grid { grid-template-columns: 1fr; }
  .chat-composer { position: sticky; bottom: 0; margin: 0 8px 8px; }
  .source-preview-modal { width: 100%; max-height: min(88dvh, 760px); border-radius: 24px 24px 0 0; }
  .source-preview-head { padding: 34px 16px 13px; }
  .source-preview-head h3 { font-size: 19px; }
  .source-preview-body { padding: 18px 16px; }
  .source-preview-modal .knowledge-modal-actions { padding: 10px 16px 16px; }
}
@media (prefers-reduced-motion: reduce) {
  .searching-bubble i, .searching-bubble span b { animation: none; }
}
</style>
