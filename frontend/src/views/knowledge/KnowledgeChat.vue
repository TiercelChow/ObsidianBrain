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
          <div class="context-book-icon">MD</div>
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
        <div ref="messageListRef" class="message-list" @scroll="onMessageScroll">
          <div v-if="!messages.length && !historyLoading" class="chat-welcome">
            <div class="welcome-symbol"><el-icon><ChatDotRound /></el-icon></div>
            <h2>从这本书开始思考</h2>
            <p>先结合会话理解问题，再从本书编译知识目录选择证据；必要时继续核对原文。</p>
            <button v-for="question in starterQuestions" :key="question" @click="ask(question)">{{ question }}</button>
          </div>
          <div v-else-if="historyLoading && !messages.length" class="history-loading">
            <el-icon class="is-loading"><Loading /></el-icon><span>正在恢复历史会话</span>
          </div>
          <template v-for="message in messages" :key="message.id">
            <div class="message" :class="message.role">
              <span class="message-role">{{ message.role === 'user' ? '你' : '知识库' }}</span>
              <p v-if="message.role === 'user'">{{ message.content }}</p>
              <template v-else>
                <div v-if="streamingMessageId === message.id && !message.content" class="chat-run-status" role="status" aria-live="polite" aria-atomic="true">
                  <span class="chat-run-dots" aria-hidden="true"><i></i><i></i><i></i></span>
                  <span class="chat-run-label">{{ streamPhase || '正在处理你的问题' }}</span>
                </div>
                <KnowledgeAnswerMarkdown
                  v-if="message.content"
                  :content="message.content"
                  :evidence-count="message.evidence?.length ?? 0"
                  :streaming="streamingMessageId === message.id"
                  @citation="sourceIndex => previewEvidence(message, sourceIndex)"
                  @rendered="followAnswer"
                />
              </template>
              <div v-if="message.interruption" class="answer-interruption" role="status">
                <strong>{{ message.interruption.notice }}</strong>
                <span v-if="message.interruption.kind !== 'cancelled'">{{ message.interruption.detail }}</span>
              </div>
              <button v-if="message.role === 'assistant' && message.runId && !message.interruption && message.id !== streamingMessageId" class="save-answer" type="button" :disabled="savingRunId === message.runId" @click="saveAnswer(message)">
                <el-icon :class="{ 'is-loading': savingRunId === message.runId }"><Loading v-if="savingRunId === message.runId" /><Checked v-else /></el-icon>{{ savingRunId === message.runId ? '正在生成候选' : '保存到 Wiki' }}
              </button>
            </div>
            <section v-if="message.evidence?.length && message.id !== streamingMessageId" class="evidence-section" aria-label="回答参考来源">
              <header><strong>参考来源</strong><span>{{ message.evidence.length }} 条{{ message.runId ? '已读证据' : '检索候选' }}</span></header>
              <div class="evidence-grid">
                <button
                  v-for="(entry, evidenceIndex) in message.evidence"
                  :key="`${message.id}-${evidenceIndex}-${entry.id}`"
                  type="button"
                  @click="previewEvidence(message, evidenceIndex)"
                >
                  <span><b>S{{ evidenceIndex + 1 }}</b>{{ entry.source_path || '数据库实体' }}</span>
                  <strong>{{ entry.title }}</strong>
                  <p>{{ entry.summary || '预览证据正文与引用' }}</p>
                  <em>预览来源 <el-icon><ArrowRight /></el-icon></em>
                </button>
              </div>
            </section>
          </template>
        </div>

        <form class="chat-composer" @submit.prevent="ask(draft)">
          <textarea v-model="draft" :disabled="!activeBaseId" rows="1" placeholder="询问本书中的概念、章节或观点…" @compositionstart="composerIsComposing = true" @compositionend="finishComposerComposition" @keydown="handleComposerKeydown"></textarea>
          <button v-if="searching && runtimeReady" type="button" class="is-stop" aria-label="停止生成" @click="stopAnswer"><span class="stop-square"></span></button>
          <button v-else type="submit" :disabled="!activeBaseId || !draft.trim() || searching" aria-label="发送问题">
            <el-icon><Top /></el-icon>
          </button>
        </form>
      </section>
    </div>

    <MotionModal v-model="sourceVisible" aria-label="来源预览" size="wide">
      <div class="knowledge-modal-card source-preview-modal">
        <header class="source-preview-head">
          <div>
            <span>{{ sourceDetail?.entry_type === 'external' ? '外部资料' : sourceDetail?.entry_type === 'source_section' ? '来源章节' : '知识实体' }}</span>
            <h3>{{ sourceDetail?.title || '来源预览' }}</h3>
            <p>{{ sourceDetail?.source_path || '数据库实体' }}</p>
            <p v-if="sourceHistorical" class="source-snapshot-note">本轮实际读取的历史快照 · {{ sourceVersion }}</p>
            <p v-else-if="sourceDetail" class="source-snapshot-note">当前实体版本（非本轮历史证据）</p>
            <p v-if="sourceRanges.length" class="source-snapshot-note">已读范围：{{ sourceRanges.map(range => `${range.offset_chars}–${range.offset_chars + range.returned_chars} 字符`).join('、') }}</p>
          </div>
          <button type="button" aria-label="关闭来源预览" @click="sourceVisible = false">
            <el-icon><Close /></el-icon>
          </button>
        </header>
        <div class="source-preview-body">
          <div v-if="sourceLoading" class="source-loading">
            <el-icon class="is-loading"><Loading /></el-icon><span>正在读取来源</span>
          </div>
          <p v-else-if="sourceError" class="source-preview-error" role="alert">{{ sourceError }}</p>
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
          <el-button v-if="sourceDetail && sourceDetail.entry_type !== 'external'" type="primary" @click="openSourceWorkspace">在 Wiki 工作台打开当前版本</el-button>
          <el-button v-else-if="sourceCurrentEntry && sourceError && sourceCurrentEntry.entry_type !== 'external'" @click="openCurrentSource">查看当前版本（非本轮证据）</el-button>
        </footer>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowRight, ChatDotRound, Checked, Close, Loading, Lock, Plus, Top } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { shouldSendComposerOnEnter } from '@/utils/chatComposer'
import { createStreamedTextBuffer, type StreamedTextBuffer } from '@/utils/streamedText'
import { interruptedKnowledgeAnswer, knowledgeRuntimeMode } from '@/utils/knowledgeRuntimePolicy'
import { loadKnowledgeCitationPreview } from '@/utils/knowledgeCitationPreview'
import {
  getBookWikiSettings,
  getKnowledgeConversation,
  getKnowledgeEntry,
  getAgentRunCitation,
  listBookKnowledgeBases,
  listKnowledgeConversations,
  listKnowledgeEntries,
  saveKnowledgeAnswer,
  streamBookKnowledge,
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
  runId?: string
  interruption?: ReturnType<typeof interruptedKnowledgeAnswer>
}

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const conversations = ref<KnowledgeConversationSummary[]>([])
const activeConversationId = ref('')
const draft = ref('')
const composerIsComposing = ref(false)
const messages = ref<ChatMessage[]>([])
const searching = ref(false)
const streamingMessageId = ref('')
const streamPhase = ref('')
const followOutput = ref(true)
const historyLoading = ref(false)
const runtimeReady = ref(false)
const runtimeMessage = ref('DeepSeek Harness 尚未连接；当前只返回真实命中的书内证据。')
const messageListRef = ref<HTMLElement | null>(null)
const sourceVisible = ref(false)
const sourceLoading = ref(false)
const sourceDetail = ref<Pick<KnowledgeEntryDetail, 'id' | 'title' | 'entry_type' | 'source_path' | 'content_md' | 'citations'> | null>(null)
const sourceHistorical = ref(false)
const sourceVersion = ref('')
const sourceRanges = ref<Array<{ offset_chars: number; returned_chars: number }>>([])
const sourceError = ref('')
const sourceCurrentEntry = ref<KnowledgeEntrySummary | null>(null)
const sourceHtml = ref('')
const sourceMarkdownRef = ref<HTMLElement | null>(null)
const savingRunId = ref('')
let localMessageId = 0
let historyRequestId = 0
let sourceRequestId = 0
let lastCompositionEndAt = 0
let askController: AbortController | null = null
let activeTextBuffer: StreamedTextBuffer | null = null

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const starterQuestions = ['这本书的核心主题是什么？', '找出与架构相关的章节', '有哪些内容提到了性能优化？']
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

function setActivity(phase: string) {
  if (streamPhase.value !== phase) streamPhase.value = phase
}

function labelForRuntimePhase(message: string): string | null {
  if (/理解追问|编译知识目录/.test(message)) return '正在理解问题与选择知识'
  if (/结束|校验/.test(message)) return '正在整理回答'
  if (/启动|连接|会话/.test(message)) return '正在连接模型'
  if (/请求|等待|分析|思考/.test(message)) return '正在分析书内证据'
  return null
}

function stopAnswer() {
  askController?.abort()
  activeTextBuffer?.cancel()
}

function finishComposerComposition() {
  composerIsComposing.value = false
  lastCompositionEndAt = Date.now()
}

function handleComposerKeydown(event: KeyboardEvent) {
  if (!shouldSendComposerOnEnter(event, composerIsComposing.value, Date.now() - lastCompositionEndAt)) return
  event.preventDefault()
  void ask(draft.value)
}

async function loadContext() {
  try {
    const [cardsResponse, settingsResponse] = await Promise.all([
      listBookKnowledgeBases(),
      getBookWikiSettings(),
    ])
    if (cardsResponse.status !== 'success' || !cardsResponse.result) throw new Error(cardsResponse.error?.message || '知识库加载失败')
    bases.value = cardsResponse.result.items.flatMap(card => (
      card.book.kind === 'folder' && card.knowledge_base ? [card.knowledge_base] : []
    ))
    const requestedBase = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requestedBase) ? requestedBase : (bases.value[0]?.id || '')
    const runtime = settingsResponse.result?.runtime_profiles.find(item => item.profile.runtime === 'deepseek_harness')
    runtimeReady.value = Boolean(runtime?.available)
    if (runtime?.available) {
      runtimeMessage.value = `${runtime.version || '本地启动器'} 可用；ACP 会话与模型凭据将在问答时验证。`
    } else if (runtime) {
      runtimeMessage.value = runtime.message
    }
    if (activeBaseId.value) {
      const requestedQuestion = String(route.query.question || '').trim()
      if (requestedQuestion) {
        newConversation(false)
        draft.value = requestedQuestion
      } else {
        await loadConversations(String(route.query.conversation || ''))
      }
    }
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
    followOutput.value = true
    messages.value = response.result.messages.map(message => ({
      id: message.id,
      role: message.role,
      content: message.content,
      evidence: message.evidence,
      runId: message.run_id || undefined,
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
  followOutput.value = true
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
  const assistantIndex = messages.value.length
  messages.value.push({ id: `local-${++localMessageId}`, role: 'assistant', content: '', evidence: [] })
  // Always mutate the proxy stored in the reactive array, not the raw object passed to push().
  const assistantMessage = messages.value[assistantIndex]
  streamingMessageId.value = assistantMessage.id
  followOutput.value = true
  searching.value = true
  setActivity('正在检索书内证据')
  await scrollToBottom(false)
  let receivedText = ''
  try {
    if (knowledgeRuntimeMode(runtimeReady.value) === 'runtime') {
      askController = new AbortController()
      const textBuffer = createStreamedTextBuffer(chunk => { assistantMessage.content += chunk })
      activeTextBuffer = textBuffer
      let answerEvidenceDelivered = false
      const result = await streamBookKnowledge(
        activeBaseId.value,
        value,
        activeConversationId.value || undefined,
        (event) => {
          if (event.type === 'evidence') {
            answerEvidenceDelivered = true
            assistantMessage.evidence = event.evidence
          } else if (event.type === 'run_started') {
            if (answerEvidenceDelivered) {
              assistantMessage.runId = event.run_id
              setActivity('正在连接模型')
            }
          } else if (event.type === 'text_delta') {
            receivedText += event.delta
            textBuffer.push(event.delta)
          } else if (event.type === 'phase') {
            const label = labelForRuntimePhase(event.message)
            if (label) setActivity(label)
          } else if (event.type === 'tool_started') {
            setActivity('正在查阅相关知识')
          } else if (event.type === 'tool_finished') {
            setActivity(event.status === 'failed' ? '正在调整检索方式' : '正在分析书内证据')
          }
        },
        askController.signal,
      )
      if (result.answer.startsWith(receivedText)) textBuffer.push(result.answer.slice(receivedText.length))
      else if (!receivedText) textBuffer.push(result.answer)
      await textBuffer.drain()
      if (askController.signal.aborted) throw new DOMException('已停止生成', 'AbortError')
      assistantMessage.content = result.answer
      assistantMessage.evidence = result.evidence
      assistantMessage.runId = result.run_id
      runtimeMessage.value = '模型已完成本轮回答；下一轮会继续使用 Harness。'
      streamingMessageId.value = ''
      activeConversationId.value = result.conversation_id
      replaceChatQuery()
      try {
        await refreshConversationList()
      } catch {
        ElMessage.warning('回答已保存，但历史列表暂未刷新')
      }
      return
    }
    await appendEvidenceFallback(value, assistantMessage)
  } catch (error) {
    activeTextBuffer?.cancel()
    const interruption = interruptedKnowledgeAnswer(assistantMessage.content, receivedText, error)
    assistantMessage.content = interruption.content
    assistantMessage.interruption = interruption
    // A failed request does not change launcher availability or silently replace an answer with search results.
    if (runtimeReady.value) runtimeMessage.value = interruption.kind === 'cancelled'
      ? '本轮已停止；下一次提问仍可使用 Harness。'
      : interruption.notice
  } finally {
    askController = null
    activeTextBuffer?.cancel()
    activeTextBuffer = null
    streamPhase.value = ''
    streamingMessageId.value = ''
    searching.value = false
    followAnswer()
  }
}

async function saveAnswer(message: ChatMessage) {
  if (!message.runId || !activeBaseId.value) return
  savingRunId.value = message.runId
  try {
    const response = await saveKnowledgeAnswer(activeBaseId.value, message.runId)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '保存失败')
    ElMessage.success('已生成待审核知识候选')
    await router.push({ path: '/knowledge/wiki', query: { base: activeBaseId.value, review: response.result.id } })
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingRunId.value = ''
  }
}

async function appendEvidenceFallback(question: string, target: ChatMessage) {
  const response = await listKnowledgeEntries(activeBaseId.value, { query: question, limit: 6 })
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '检索失败')
  const evidence = response.result.entries
  target.content = evidence.length
    ? `当前为证据检索模式（未生成模型回答）。在《${activeBase.value?.book_name || '当前书籍'}》中找到 ${evidence.length} 条相关候选，你可以在下方弹窗中核对当前正文。`
    : '当前为证据检索模式（未生成模型回答）。没有找到足够相关的候选，可以换一个关键词，或先返回知识库页面同步来源。'
  target.evidence = evidence
}

async function previewSource(entry: KnowledgeEntrySummary, runId?: string, sourceIndex = 0) {
  const requestId = ++sourceRequestId
  cleanup()
  sourceVisible.value = true
  sourceLoading.value = true
  sourceDetail.value = null
  sourceHtml.value = ''
  sourceHistorical.value = false
  sourceVersion.value = ''
  sourceRanges.value = []
  sourceError.value = ''
  sourceCurrentEntry.value = entry
  try {
    const snapshot = await loadKnowledgeCitationPreview(entry, runId, sourceIndex, { snapshot: getAgentRunCitation, current: getKnowledgeEntry })
    if (requestId !== sourceRequestId) return
    sourceDetail.value = { ...snapshot.entry, content_md: snapshot.content_md, citations: snapshot.citations }
    sourceHistorical.value = snapshot.historical
    sourceVersion.value = snapshot.version_id
    sourceRanges.value = snapshot.read_ranges
    const rendered = await renderMarkdown(snapshot.content_md, undefined, `${runId || 'current'}-${entry.id}`)
    if (requestId !== sourceRequestId) return
    sourceHtml.value = rendered
    await nextTick()
    if (sourceMarkdownRef.value) await enhance(sourceMarkdownRef.value)
  } catch (error) {
    if (requestId === sourceRequestId) {
      sourceError.value = (error as Error).message
    }
  } finally {
    if (requestId === sourceRequestId) sourceLoading.value = false
  }
}

function previewEvidence(message: ChatMessage, sourceIndex: number) {
  const entry = message.evidence?.[sourceIndex]
  if (entry) void previewSource(entry, message.runId, sourceIndex)
}

function openCurrentSource() {
  if (sourceCurrentEntry.value) void previewSource(sourceCurrentEntry.value)
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

function onMessageScroll(event: Event) {
  const element = event.target as HTMLElement
  followOutput.value = element.scrollHeight - element.clientHeight - element.scrollTop < 96
}

function followAnswer() {
  if (followOutput.value) void scrollToBottom(false)
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
  askController?.abort()
  activeTextBuffer?.cancel()
  cleanup()
})
</script>

<style scoped>
.chat-layout { min-width: 0; min-height: 590px; display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 12px; }
.answer-interruption { display: grid; gap: 5px; margin-top: 10px; padding: 10px 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
.answer-interruption strong { font-weight: 600; }
.answer-interruption span { color: var(--text-faint); }
.source-preview-error, .source-snapshot-note { color: var(--text-muted); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
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
.message-list { min-width: 0; flex: 1; max-height: calc(100vh - 290px); min-height: 480px; overflow: auto; padding: 28px clamp(16px, 5vw, 70px); }
.chat-welcome { min-height: 390px; display: grid; place-content: center; justify-items: center; gap: 10px; text-align: center; }
.welcome-symbol { width: 65px; height: 65px; display: grid; place-items: center; margin-bottom: 5px; border-radius: 22px; background: radial-gradient(circle at 32% 24%, color-mix(in srgb, white 42%, transparent), transparent 42%), var(--accent-light); color: var(--accent); font-size: 28px; box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-welcome h2 { font-size: 20px; }
.chat-welcome > p { max-width: 520px; margin-bottom: 10px; color: var(--text-muted); font-size: 13px; line-height: 1.7; }
.chat-welcome > button { width: min(100%, 380px); min-height: 40px; padding: 8px 14px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-secondary); font: inherit; font-size: 12px; cursor: pointer; transition: var(--transition-interactive); }
.chat-welcome > button:hover { border-color: var(--accent-border); color: var(--accent); transform: translateY(-1px); }
.history-loading { min-height: 320px; display: grid; place-content: center; justify-items: center; gap: 9px; color: var(--text-faint); font-size: 12px; }
.message { min-width: 0; max-width: 78%; display: grid; gap: 5px; margin-bottom: 16px; }
.message.assistant { max-width: min(100%, 900px); }
.message.user { margin-left: auto; justify-items: end; }
.message-role { color: var(--text-faint); font-size: 10px; }
.message > p { padding: 11px 14px; overflow-wrap: anywhere; border-radius: 16px 16px 16px 5px; background: var(--bg-glass-subtle); color: var(--text-secondary); font-size: 13px; line-height: 1.65; white-space: pre-wrap; }
.message.user p { border-radius: 16px 16px 5px 16px; background: var(--accent); color: white; }
.save-answer { width: fit-content; display: inline-flex; align-items: center; gap: 5px; padding: 6px 9px; border: 0; border-radius: 9px; background: transparent; color: var(--accent); font: inherit; font-size: 10px; font-weight: 650; cursor: pointer; }
.save-answer:hover { background: var(--accent-light); }
.save-answer:disabled { opacity: .55; cursor: default; }
.chat-run-status { min-height: 32px; display: flex; align-items: center; gap: 9px; padding: 4px 2px; }
.chat-run-dots { display: inline-flex; align-items: center; gap: 3px; flex: none; }
.chat-run-dots i { width: 4px; height: 4px; border-radius: 50%; background: var(--text-secondary); opacity: .95; animation: chat-dot-wave 900ms ease-in-out infinite alternate; }
.chat-run-dots i:nth-child(2) { animation-delay: 130ms; }
.chat-run-dots i:nth-child(3) { animation-delay: 260ms; }
.chat-run-label { color: var(--text-muted); font-size: 13px; font-weight: 600; line-height: 1.6; background-image: linear-gradient(90deg, var(--text-muted) 0%, var(--text-muted) 42%, var(--text-secondary) 50%, var(--text-muted) 58%, var(--text-muted) 100%); background-size: 250% 100%; background-position: 100% 0; background-repeat: no-repeat; background-clip: text; -webkit-background-clip: text; -webkit-text-fill-color: transparent; animation: chat-status-shimmer 2.4s linear infinite; }
.evidence-section { max-width: min(100%, 900px); margin: 2px 0 22px; }
.evidence-section > header { display: flex; align-items: baseline; gap: 8px; margin: 0 0 9px; color: var(--text-muted); }
.evidence-section > header strong { color: var(--text-secondary); font-size: 11px; font-weight: 700; }
.evidence-section > header span { color: var(--text-faint); font-size: 10px; }
.evidence-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.evidence-grid button { min-width: 0; display: grid; gap: 5px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); color: var(--text-primary); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.evidence-grid button:hover { border-color: var(--accent-border); transform: translateY(-1px); }
.evidence-grid span { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid span b { display: inline-flex; margin-right: 7px; padding: 2px 5px; border-radius: 5px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.evidence-grid strong { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.evidence-grid p { display: -webkit-box; overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.5; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.evidence-grid em { display: flex; align-items: center; gap: 3px; margin-top: 3px; color: var(--accent); font-size: 10px; font-style: normal; }
.chat-composer { display: flex; align-items: flex-end; gap: 8px; margin: 0 16px 16px; padding: 9px 9px 9px 15px; border: 1px solid var(--border-subtle); border-radius: 18px; background: var(--bg-glass-strong); box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-composer:focus-within { border-color: var(--accent-border); }
.chat-composer textarea { min-width: 0; flex: 1; min-height: 38px; max-height: 120px; padding: 8px 0; resize: none; border: 0; outline: none; background: transparent; color: var(--text-primary); font: inherit; line-height: 1.5; }
.chat-composer button { width: 38px; height: 38px; display: grid; place-items: center; flex: none; border: 0; border-radius: 12px; background: var(--accent); color: white; cursor: pointer; }
.chat-composer button:disabled { opacity: .35; cursor: default; }
.chat-composer button.is-stop { background: var(--text-primary); }
.stop-square { width: 11px; height: 11px; border-radius: 3px; background: var(--bg-base); }
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
.source-citations strong { overflow-wrap: anywhere; font-size: 11px; }
.source-citations span { color: var(--text-faint); font-size: 9px; }
.source-citations p { color: var(--text-muted); font-size: 11px; line-height: 1.6; }
@keyframes chat-dot-wave { to { transform: translateY(-2px); opacity: .45; } }
@keyframes chat-status-shimmer { to { background-position: 0% 0; } }
@media (min-width: 1151px) {
  .chat-layout, .chat-panel { min-height: 0; }
  .message-list { min-height: 0; max-height: none; padding: 20px clamp(16px, 3vw, 40px); }
  .chat-welcome { min-height: 100%; }
  .chat-composer { margin: 0 12px 12px; }
}
@media (min-width: 769px) and (max-width: 1150px) {
  .chat-layout { min-height: 0; display: block; }
  .chat-context { display: grid; grid-template-columns: minmax(180px, 230px) minmax(0, 1fr); gap: 10px 12px; margin-bottom: 12px; }
  .context-label, .conversation-history, .runtime-state, .boundary-note { grid-column: 1 / -1; }
  .conversation-list { max-height: none; display: flex; overflow-x: auto; }
  .conversation-list > button { flex: 0 0 190px; }
  .message-list { max-height: min(65dvh, 800px); }
}
@media (max-width: 768px) {
  .chat-layout { min-height: 0; flex: 1; display: flex; flex-direction: column; }
  .chat-context { align-self: stretch; margin-bottom: 10px; padding: 13px; }
  .context-label, .context-book, .boundary-note, .runtime-state p { display: none; }
  .runtime-state { display: flex; align-items: center; }
  .conversation-history { grid-row: 3; }
  .conversation-history header > button span { display: none; }
  .conversation-list { max-height: none; display: flex; gap: 6px; overflow-x: auto; padding-bottom: 2px; }
  .conversation-list > button { width: 168px; flex: none; }
  .conversation-list > p { min-width: 220px; }
  .chat-panel { min-height: 0; flex: 1; }
  .message-list { min-height: 0; max-height: none; padding: 20px 12px; }
  .chat-welcome { min-height: 100%; }
  .message { max-width: 92%; }
  .evidence-grid { grid-template-columns: 1fr; }
  .chat-composer { flex: none; margin: 0 8px 8px; }
  .source-preview-modal { width: 100%; max-height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); border-radius: 24px 24px 0 0; }
  .source-preview-head { padding: 34px 16px 13px; }
  .source-preview-head h3 { font-size: 19px; }
  .source-preview-body { padding: 18px 16px; }
  .source-preview-modal .knowledge-modal-actions { padding: 10px 16px 16px; }
}
@media (prefers-reduced-motion: reduce) {
  .chat-run-dots i { animation: none; opacity: .6; }
  .chat-run-label { animation: none; background: none; -webkit-text-fill-color: currentColor; }
}
@media (prefers-contrast: more) {
  .chat-run-dots i { animation: none; opacity: 1; }
  .chat-run-label { animation: none; background: none; color: var(--text-secondary); -webkit-text-fill-color: currentColor; }
}
@media (forced-colors: active) {
  .chat-run-dots i { animation: none; background: CanvasText; opacity: 1; }
  .chat-run-label { animation: none; background: none; -webkit-text-fill-color: currentColor; }
}
</style>
