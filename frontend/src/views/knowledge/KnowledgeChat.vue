<template>
  <KnowledgePageShell title="知识问答" subtitle="在单本书的证据边界内提问，答案与来源会持续保留">
    <div class="chat-layout">
      <div class="chat-mobile-context">
        <el-select v-model="activeBaseId" class="knowledge-select is-fluid" aria-label="当前问答知识库" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" :disabled="searching" placeholder="选择一本书" @change="changeKnowledgeBase">
          <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
        </el-select>
        <button type="button" aria-label="问答历史与设置" @click="contextVisible = true"><el-icon><ChatLineSquare /></el-icon></button>
      </div>
      <aside class="chat-context knowledge-surface" data-glass="structural">
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

        <section v-if="unfinishedRuns.length" class="conversation-history unfinished-history" aria-label="待恢复的问答">
          <header><span>待恢复的问答</span><span>{{ unfinishedRuns.length }} 轮</span></header>
          <div class="conversation-list">
            <button v-for="run in unfinishedRuns" :key="run.run_id" type="button" :disabled="searching || !!openingRunId" @click="openUnfinishedRun(run)">
              <strong>{{ run.question }}</strong><span>{{ run.completed_without_history ? '回答待恢复' : run.has_partial_answer ? '草稿待完成' : '未收到正文' }} · {{ formatConversationTime(run.created_at) }}</span>
            </button>
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
                <div v-if="message.restoredFailed && message.content" class="restored-answer-draft"><strong>未完成草稿 · 引用待重新核对</strong><p>{{ message.content }}</p></div>
                <div v-else-if="message.unsavedAnswer && message.content" class="restored-answer-draft"><strong>回答已生成，但未写入会话</strong><span>这段正文从运行记录找回；引用需在运行检查器核对。补存成功前，后续追问不会自动继承它。</span><KnowledgeAnswerMarkdown :content="message.content" :evidence-count="0" @rendered="followAnswer" /></div>
                <KnowledgeAnswerMarkdown
                  v-else-if="message.content"
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
                <button v-if="message.interruption.canRetry && message.runId && message.originalQuestion && message.content" type="button" :disabled="searching || !runtimeReady" @click="ask(message.originalQuestion, message)">继续完成完整答案</button>
                <button v-else-if="message.interruption.canRetry && message.originalQuestion && !message.content" type="button" :disabled="searching || !runtimeReady" @click="ask(message.originalQuestion, message.runId ? message : undefined)">{{ message.runId ? '继续生成完整答案' : '重新提问' }}</button>
                <button v-else-if="message.interruption.kind === 'truncated' && message.runId && message.originalQuestion" type="button" :disabled="searching || !runtimeReady" @click="ask(message.originalQuestion, message)">{{ message.interruption.detail.includes('qa_output_retry_exhausted') ? '仍要继续（会重新生成）' : '调整配置后继续' }}</button>
                <button v-else-if="['failed', 'credentials', 'stalled'].includes(message.interruption.kind) && message.originalQuestion" type="button" :disabled="searching || !runtimeReady" @click="ask(message.originalQuestion)">重新提问</button>
                <button v-if="message.interruption.kind === 'truncated' && !message.interruption.canRetry" type="button" @click="router.push({ path: '/knowledge/settings' })">调整模型配置</button>
                <button v-if="message.originalQuestion && activeBaseId && message.interruption.researchSuggested" type="button" :disabled="searching || !!researchTransferMessageId" @click="openResearchTask(message)">{{ researchTransferMessageId === message.id ? '正在恢复问题…' : '转为分阶段研究任务' }}</button>
              </div>
              <button v-if="message.role === 'assistant' && message.runId && message.id !== streamingMessageId" type="button" class="save-answer" @click="inspectRun(message.runId)">查看本轮目标与取证预算</button>
              <button v-if="message.unsavedAnswer && message.runId" class="save-answer" type="button" :disabled="searching || savingHistoryRunId === message.runId" @click="persistRecoveredAnswer(message)">{{ savingHistoryRunId === message.runId ? '正在补存…' : '补存到会话历史' }}</button>
              <button v-if="message.role === 'assistant' && message.runId && !message.interruption && !message.unsavedAnswer && message.id !== streamingMessageId" class="save-answer" type="button" :disabled="savingRunId === message.runId" @click="saveAnswer(message)">
                <el-icon :class="{ 'is-loading': savingRunId === message.runId }"><Loading v-if="savingRunId === message.runId" /><Checked v-else /></el-icon>{{ savingRunId === message.runId ? '正在生成候选' : '保存到 Wiki' }}
              </button>
              <button v-if="message.role === 'assistant' && message.runId && !message.interruption && message.originalQuestion && message.id !== streamingMessageId" class="save-answer" type="button" :disabled="searching || !!researchTransferMessageId" @click="openResearchTask(message)">{{ researchTransferMessageId === message.id ? '正在准备研究…' : '深入研究此问题' }}</button>
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

        <form v-glass-lens class="chat-composer" data-glass-rim @submit.prevent="ask(draft)">
          <button class="chat-short-context-button" type="button" aria-label="问答历史与设置" @click="contextVisible = true"><el-icon><ChatLineSquare /></el-icon></button>
          <textarea v-model="draft" :disabled="!activeBaseId" rows="1" placeholder="询问本书中的概念、章节或观点…" @compositionstart="composerIsComposing = true" @compositionend="finishComposerComposition" @keydown="handleComposerKeydown"></textarea>
          <button v-if="searching && runtimeReady" type="button" class="is-stop" aria-label="停止生成" @click="stopAnswer"><span class="stop-square"></span></button>
          <button v-else type="submit" :disabled="!activeBaseId || !draft.trim() || searching" aria-label="发送问题">
            <el-icon><Top /></el-icon>
          </button>
        </form>
      </section>
    </div>

    <MotionModal v-model="contextVisible" aria-label="问答历史与设置">
      <div class="knowledge-modal-card chat-context-sheet">
        <header class="knowledge-modal-head"><h3>历史问答</h3><p>{{ activeBase?.book_name || '请先选择一本书' }}</p></header>
        <div class="knowledge-modal-body">
          <el-select v-model="activeBaseId" class="knowledge-select is-fluid" aria-label="当前问答知识库" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" :disabled="searching" placeholder="选择一本书" @change="changeKnowledgeBase">
            <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
          </el-select>
          <button class="new-conversation-button" type="button" :disabled="!activeBaseId || searching" @click="contextVisible = false; newConversation()"><el-icon><Plus /></el-icon>新对话</button>
          <section class="conversation-history" aria-label="手机端问答历史">
            <div class="conversation-list">
              <button v-for="conversation in conversations" :key="conversation.id" type="button" :class="{ active: conversation.id === activeConversationId }" :disabled="searching" @click="contextVisible = false; openConversation(conversation.id)">
                <strong>{{ conversation.title }}</strong><span>{{ conversation.message_count }} 条消息 · {{ formatConversationTime(conversation.updated_at) }}</span>
              </button>
              <p v-if="historyLoading">正在恢复历史…</p>
              <p v-else-if="!conversations.length">首次提问后，会话会自动保存在本地数据库。</p>
            </div>
          </section>
          <section v-if="unfinishedRuns.length" class="conversation-history unfinished-history" aria-label="手机端待恢复的问答">
            <header><span>待恢复的问答</span><span>{{ unfinishedRuns.length }} 轮</span></header>
            <div class="conversation-list">
              <button v-for="run in unfinishedRuns" :key="run.run_id" type="button" :disabled="searching || !!openingRunId" @click="openUnfinishedRun(run)">
                <strong>{{ run.question }}</strong><span>{{ run.completed_without_history ? '回答待恢复' : run.has_partial_answer ? '草稿待完成' : '未收到正文' }} · {{ formatConversationTime(run.created_at) }}</span>
              </button>
            </div>
          </section>
          <div class="runtime-state"><span class="knowledge-status" :class="runtimeReady ? 'is-healthy' : 'is-warning'">{{ runtimeReady ? 'Harness 启动器可用' : '证据检索模式' }}</span><p>{{ runtimeMessage }}</p></div>
          <p class="boundary-note"><el-icon><Lock /></el-icon>不会跨书检索，也不会直接改写原始文件。</p>
        </div>
        <footer class="knowledge-modal-actions"><el-button @click="contextVisible = false">关闭</el-button></footer>
      </div>
    </MotionModal>

    <MotionModal v-model="inspectorVisible" aria-label="问答运行检查器" size="wide">
      <div class="knowledge-modal-card qa-inspector-modal">
        <header class="source-preview-head"><h3>本轮问答</h3><button type="button" aria-label="关闭问答运行检查器" @click="inspectorVisible = false"><el-icon><Close /></el-icon></button></header>
        <div class="source-preview-body">
          <p v-if="inspectorLoading" role="status">正在读取运行记录…</p>
          <p v-else-if="inspectorError" role="alert">{{ inspectorError }}</p>
          <KnowledgeRunInspector v-else :inspection="inspection" />
        </div>
        <footer class="knowledge-modal-actions"><el-button @click="inspectorVisible = false">关闭</el-button><el-button :disabled="inspectorLoading" @click="inspectRun(inspectedRunId)">刷新状态</el-button></footer>
      </div>
    </MotionModal>

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
import { ArrowRight, ChatDotRound, ChatLineSquare, Checked, Close, Loading, Lock, Plus, Top } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import KnowledgeRunInspector from '@/components/knowledge/KnowledgeRunInspector.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { shouldSendComposerOnEnter } from '@/utils/chatComposer'
import { createStreamedTextBuffer, type StreamedTextBuffer } from '@/utils/streamedText'
import { interruptedKnowledgeAnswer, knowledgeRuntimeMode, labelForRuntimePhase, originalQuestionForSavedAnswer, researchTaskQuestionFromPlanning, researchTaskQuestionFromRun, researchTaskRouteFromQuestion } from '@/utils/knowledgeRuntimePolicy'
import { recoverInterruptedQaRun } from '@/utils/qaRunRecovery'
import { loadKnowledgeCitationPreview } from '@/utils/knowledgeCitationPreview'
import {
  getBookWikiSettings,
  getKnowledgeConversation,
  getKnowledgeEntry,
  getAgentRunCitation,
  getAgentRunInspection,
  listBookKnowledgeBases,
  listKnowledgeConversations,
  listUnfinishedQaRuns,
  listKnowledgeEntries,
  recoverCompletedQaAnswer,
  saveKnowledgeAnswer,
  streamBookKnowledge,
  type KnowledgeBaseSummary,
  type KnowledgeConversationSummary,
  type UnfinishedQaRun,
  type KnowledgeEntryDetail,
  type KnowledgeEntrySummary,
  type AgentRunInspection,
} from '@/api/knowledge'

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  knowledgeBaseId?: string
  evidence?: KnowledgeEntrySummary[]
  runId?: string
  originalQuestion?: string
  requestConversationId?: string
  planningContext?: { knowledge_base_id: string; question: string; standalone_question: string }
  interruption?: ReturnType<typeof interruptedKnowledgeAnswer>
  restoredFailed?: boolean
  unsavedAnswer?: boolean
}

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const contextVisible = ref(false)
const conversations = ref<KnowledgeConversationSummary[]>([])
const unfinishedRuns = ref<UnfinishedQaRun[]>([])
const openingRunId = ref('')
const savingHistoryRunId = ref('')
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
const researchTransferMessageId = ref('')
const inspectorVisible = ref(false)
const inspectorLoading = ref(false)
const inspectorError = ref('')
const inspection = ref<AgentRunInspection | null>(null)
const inspectedRunId = ref('')
let inspectorRequestId = 0
let localMessageId = 0
let historyRequestId = 0
let unfinishedRequestId = 0
let sourceRequestId = 0
let lastCompositionEndAt = 0
let askController: AbortController | null = null
let activeTextBuffer: StreamedTextBuffer | null = null
let viewActive = true

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const starterQuestions = ['这本书的核心主题是什么？', '找出与架构相关的章节', '有哪些内容提到了性能优化？']
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

function setActivity(phase: string) {
  if (streamPhase.value !== phase) streamPhase.value = phase
}

async function openResearchTask(message: ChatMessage) {
  const baseId = activeBaseId.value
  if (!baseId || !message.originalQuestion || researchTransferMessageId.value) return
  if (message.knowledgeBaseId && message.knowledgeBaseId !== baseId) {
    ElMessage.warning('这轮问答属于另一知识库，请先切回对应书籍')
    return
  }
  researchTransferMessageId.value = message.id
  let resolved = researchTaskQuestionFromPlanning(message.originalQuestion, baseId, message.planningContext || null)
  let inspectionUnavailable = false
  try {
    if (message.runId) {
      try {
        const response = await getAgentRunInspection(message.runId)
        if (response.status === 'success' && response.result) {
          const fromRun = researchTaskQuestionFromRun(message.originalQuestion, baseId, response.result.run)
          if (fromRun.contextRecovered) resolved = fromRun
        } else {
          inspectionUnavailable = true
        }
      } catch { inspectionUnavailable = true }
    }
    if (activeBaseId.value !== baseId) return
    if (!resolved.contextRecovered && (inspectionUnavailable || message.requestConversationId)) {
      ElMessage.warning('未能从运行记录补全追问，请在创建任务前确认上下文')
    }
    await router.push(researchTaskRouteFromQuestion(baseId, resolved.question))
  } catch (error) {
    ElMessage.error((error as Error).message || '无法打开研究任务创建页')
  } finally {
    researchTransferMessageId.value = ''
  }
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
      await loadUnfinishedRuns()
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

async function loadUnfinishedRuns() {
  if (!viewActive) return
  const baseId = activeBaseId.value
  const requestId = ++unfinishedRequestId
  if (!baseId) {
    unfinishedRuns.value = []
    return
  }
  try {
    const response = await listUnfinishedQaRuns(baseId)
    if (!viewActive || requestId !== unfinishedRequestId || baseId !== activeBaseId.value) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '未完成问答读取失败')
    unfinishedRuns.value = response.result.runs
  } catch (error) {
    if (!viewActive || requestId !== unfinishedRequestId || baseId !== activeBaseId.value) return
    unfinishedRuns.value = []
    ElMessage.warning((error as Error).message)
  }
}

async function openUnfinishedRun(summary: UnfinishedQaRun) {
  const baseId = activeBaseId.value
  if (!viewActive || !baseId || openingRunId.value || searching.value) return
  if (messages.value.some(message => message.runId === summary.run_id)) {
    contextVisible.value = false
    await scrollToBottom(false)
    return
  }
  openingRunId.value = summary.run_id
  try {
    const response = await getAgentRunInspection(summary.run_id)
    if (!viewActive || baseId !== activeBaseId.value) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行记录读取失败')
    const recovered = recoverInterruptedQaRun(response.result.run, baseId, summary.run_id, summary.completed_without_history)
    if (!recovered) throw new Error('这轮运行已无法在当前书籍恢复，请刷新未完成列表')
    if (recovered.conversationId) {
      await openConversation(recovered.conversationId)
      if (activeConversationId.value !== recovered.conversationId) throw new Error('原会话不可用，不能把追问转入其他会话')
    } else {
      newConversation()
    }
    if (!viewActive || baseId !== activeBaseId.value) return
    messages.value.push({ id: `local-${++localMessageId}`, role: 'user', content: recovered.question })
    messages.value.push({
      id: `local-${++localMessageId}`,
      role: 'assistant',
      content: recovered.draft,
      knowledgeBaseId: baseId,
      evidence: [],
      runId: recovered.runId,
      originalQuestion: recovered.question,
      requestConversationId: recovered.conversationId,
      interruption: recovered.completedWithoutHistory ? undefined : interruptedKnowledgeAnswer(recovered.draft, recovered.draft, new Error(recovered.error)),
      restoredFailed: !recovered.completedWithoutHistory,
      unsavedAnswer: recovered.completedWithoutHistory,
    })
    contextVisible.value = false
    followOutput.value = true
    await scrollToBottom(false)
  } catch (error) {
    if (viewActive) ElMessage.error((error as Error).message)
  } finally {
    openingRunId.value = ''
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
    const savedConversation = response.result
    if (savedConversation.knowledge_base_id !== activeBaseId.value) throw new Error('该会话不属于当前知识库')
    const conversationBaseId = savedConversation.knowledge_base_id
    activeConversationId.value = savedConversation.id
    followOutput.value = true
    messages.value = savedConversation.messages.map((message, index, history) => ({
      id: message.id,
      role: message.role,
      content: message.content,
      knowledgeBaseId: conversationBaseId,
      evidence: message.evidence,
      runId: message.run_id || undefined,
      originalQuestion: message.run_id ? originalQuestionForSavedAnswer(history, index) : undefined,
      requestConversationId: savedConversation.id,
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
  ++unfinishedRequestId
  activeConversationId.value = ''
  conversations.value = []
  unfinishedRuns.value = []
  messages.value = []
  replaceChatQuery()
  await Promise.all([loadConversations(), loadUnfinishedRuns()])
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

async function inspectRun(runId: string) {
  const request = ++inspectorRequestId
  inspectedRunId.value = runId
  inspectorVisible.value = true
  inspectorLoading.value = true
  inspectorError.value = ''
  inspection.value = null
  try {
    const response = await getAgentRunInspection(runId)
    if (request !== inspectorRequestId) return
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行记录读取失败')
    inspection.value = response.result
  } catch (error) {
    if (request === inspectorRequestId) inspectorError.value = (error as Error).message
  } finally {
    if (request === inspectorRequestId) inspectorLoading.value = false
  }
}

async function recoverUnsavedAnswer(target: ChatMessage, baseId: string): Promise<boolean> {
  if (!target.runId || !viewActive || activeBaseId.value !== baseId) return false
  try {
    const pending = await listUnfinishedQaRuns(baseId)
    if (pending.status !== 'success' || !pending.result?.runs.some(run => run.run_id === target.runId && run.completed_without_history)) return false
    const inspection = await getAgentRunInspection(target.runId)
    if (inspection.status !== 'success' || !inspection.result || !viewActive || activeBaseId.value !== baseId) return false
    const recovered = recoverInterruptedQaRun(inspection.result.run, baseId, target.runId, true)
    if (!recovered?.completedWithoutHistory) return false
    target.content = recovered.draft
    target.evidence = []
    target.unsavedAnswer = true
    target.interruption = undefined
    return true
  } catch {
    return false
  }
}

async function ask(question: string, recovery?: ChatMessage) {
  const value = question.trim()
  const baseId = activeBaseId.value
  if (!value || !baseId || searching.value) return
  draft.value = ''
  const requestConversationId = recovery ? recovery.requestConversationId : activeConversationId.value || undefined
  messages.value.push({ id: `local-${++localMessageId}`, role: 'user', content: recovery ? `继续完成：${value}` : value })
  const assistantIndex = messages.value.length
  messages.value.push({ id: `local-${++localMessageId}`, role: 'assistant', content: '', knowledgeBaseId: baseId, evidence: [], originalQuestion: value, requestConversationId })
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
      let textBuffer = createStreamedTextBuffer(chunk => { assistantMessage.content += chunk })
      activeTextBuffer = textBuffer
      let answerEvidenceDelivered = false
      const result = await streamBookKnowledge(
        baseId,
        value,
        requestConversationId,
        (event) => {
          if (event.type === 'evidence') {
            answerEvidenceDelivered = true
            assistantMessage.evidence = event.evidence
          } else if (event.type === 'planning_ready') {
            assistantMessage.planningContext = event
          } else if (event.type === 'run_started') {
            if (answerEvidenceDelivered) {
              if (receivedText) {
                textBuffer.cancel()
                assistantMessage.content = ''
                receivedText = ''
                textBuffer = createStreamedTextBuffer(chunk => { assistantMessage.content += chunk })
                activeTextBuffer = textBuffer
              }
              assistantMessage.runId = event.run_id
              setActivity('正在连接模型')
            }
          } else if (event.type === 'text_delta') {
            receivedText += event.delta
            textBuffer.push(event.delta)
          } else if (event.type === 'text_replace') {
            textBuffer.cancel()
            receivedText = event.text
            assistantMessage.content = event.text
            textBuffer = createStreamedTextBuffer(chunk => { assistantMessage.content += chunk })
            activeTextBuffer = textBuffer
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
        recovery?.runId,
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
    const recovered = await recoverUnsavedAnswer(assistantMessage, baseId)
    if (recovered) {
      runtimeMessage.value = '完整回答已从运行记录找回，但尚未写入会话历史。'
    } else {
      const interruption = interruptedKnowledgeAnswer(assistantMessage.content, receivedText, error)
      assistantMessage.content = interruption.content
      assistantMessage.interruption = interruption
      // A failed request does not change launcher availability or silently replace an answer with search results.
      if (runtimeReady.value) runtimeMessage.value = interruption.kind === 'cancelled'
        ? '本轮已停止；下一次提问仍可使用 Harness。'
        : interruption.notice
    }
  } finally {
    askController = null
    activeTextBuffer?.cancel()
    activeTextBuffer = null
    streamPhase.value = ''
    streamingMessageId.value = ''
    searching.value = false
    followAnswer()
    void loadUnfinishedRuns()
  }
}

async function persistRecoveredAnswer(message: ChatMessage) {
  const baseId = activeBaseId.value
  if (!baseId || !message.runId || !message.unsavedAnswer || savingHistoryRunId.value) return
  const conversationAtStart = activeConversationId.value
  savingHistoryRunId.value = message.runId
  try {
    const response = await recoverCompletedQaAnswer(baseId, message.runId)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '补存失败')
    if (!viewActive || activeBaseId.value !== baseId || response.result.run_id !== message.runId) return
    if (activeConversationId.value !== conversationAtStart || !messages.value.includes(message)) {
      await Promise.allSettled([refreshConversationList(), loadUnfinishedRuns()])
      return
    }
    message.unsavedAnswer = false
    message.content = response.result.answer
    message.evidence = response.result.evidence
    activeConversationId.value = response.result.conversation_id
    replaceChatQuery()
    ElMessage.success('回答已补存到会话历史')
    try {
      await Promise.all([refreshConversationList(), loadUnfinishedRuns()])
    } catch {
      ElMessage.warning('回答已补存，但历史列表暂未刷新')
    }
  } catch (error) {
    if (viewActive && activeBaseId.value === baseId) ElMessage.error((error as Error).message)
  } finally {
    savingHistoryRunId.value = ''
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
  viewActive = false
  ++historyRequestId
  ++unfinishedRequestId
  ++sourceRequestId
  ++inspectorRequestId
  askController?.abort()
  activeTextBuffer?.cancel()
  cleanup()
})
</script>

<style scoped>
.chat-mobile-context { display: none; }
.new-conversation-button { min-height: 44px; display: flex; align-items: center; justify-content: center; gap: 8px; border: 0; border-radius: 12px; color: var(--accent); background: var(--accent-light); font: inherit; cursor: pointer; }
.new-conversation-button:disabled { opacity: .45; cursor: default; }
.chat-context-sheet .conversation-list { display: grid; max-height: none; }
.chat-context-sheet .conversation-list > button { width: 100%; min-height: 54px; padding: 10px 12px; }
.chat-context-sheet .conversation-list strong { font-size: 14px; }
.chat-context-sheet .conversation-list span, .chat-context-sheet .conversation-list > p { font-size: 12px; }
.chat-context-sheet .runtime-state p, .chat-context-sheet .boundary-note { display: flex; font-size: 12px; }
.chat-layout { min-width: 0; min-height: 590px; display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 12px; }
.answer-interruption { display: grid; gap: 5px; margin-top: 10px; padding: 10px 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
.answer-interruption strong { font-weight: 600; }
.answer-interruption span { color: var(--text-faint); }
.answer-interruption button { justify-self: start; min-height: 40px; padding: 8px 12px; border: 0; border-radius: 10px; color: var(--accent); background: var(--accent-soft); cursor: pointer; font: inherit; }
.answer-interruption button:disabled { opacity: .45; cursor: default; }
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
.unfinished-history { padding-top: 12px; border-top: 1px solid var(--border-faint); }
.unfinished-history .conversation-list > button { background: var(--bg-glass-subtle); }
.runtime-state { display: grid; justify-items: start; gap: 8px; }
.runtime-state p { color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.boundary-note { display: flex; align-items: flex-start; gap: 7px; padding-top: 14px; border-top: 1px solid var(--border-faint); color: var(--text-faint); font-size: 10px; line-height: 1.5; }
.chat-panel { position: relative; min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); overflow: hidden; }
.message-list { grid-area: 1 / 1; min-width: 0; max-height: calc(100vh - 210px); min-height: 480px; overflow: auto; padding: 28px clamp(16px, 5vw, 70px) 100px; scroll-padding-bottom: 100px; }
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
.restored-answer-draft { display: grid; gap: 8px; padding: 12px 14px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--text-secondary); line-height: 1.7; overflow-wrap: anywhere; }
.restored-answer-draft strong { color: var(--text-muted); font-size: 11px; font-weight: 700; }
.restored-answer-draft p { white-space: pre-wrap; }
.restored-answer-draft > span { color: var(--text-muted); font-size: 11px; line-height: 1.5; }
.restored-answer-draft :deep(.knowledge-answer-markdown) { padding: 0; border-radius: 0; background: transparent; }
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
.chat-composer { grid-area: 1 / 1; align-self: end; z-index: 1; display: flex; align-items: flex-end; gap: 8px; margin: 0 16px 16px; padding: 9px 9px 9px 15px; border: 1px solid var(--border-subtle); border-radius: 28px; background: var(--bg-glass-strong); box-shadow: var(--shadow-md), var(--inset-highlight); }
.chat-composer:focus-within { border-color: var(--accent-border); }
.chat-composer textarea { min-width: 0; flex: 1; min-height: 38px; max-height: 120px; padding: 8px 0; resize: none; border: 0; outline: none; background: transparent; color: var(--text-primary); font: inherit; line-height: 1.5; }
.chat-composer button { width: 38px; height: 38px; display: grid; place-items: center; flex: none; border: 0; border-radius: 50%; cursor: pointer; }
.chat-composer button:disabled { opacity: .35; cursor: default; }
.chat-composer .chat-short-context-button { display: none; }
.chat-composer button.is-stop { background: var(--text-primary); }
.stop-square { width: 11px; height: 11px; border-radius: 3px; background: var(--bg-base); }
.source-preview-modal, .qa-inspector-modal { width: 100%; max-height: calc(100dvh - 48px); display: flex; flex-direction: column; }
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
  .message-list { min-height: 0; max-height: none; padding: 20px clamp(16px, 3vw, 40px) 100px; }
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
  .chat-context { display: none; }
  .chat-mobile-context { min-width: 0; flex: none; display: grid; grid-template-columns: minmax(0, 1fr) 44px; align-items: stretch; gap: 8px; margin-bottom: 10px; }
  .chat-mobile-context > button { min-height: 44px; display: grid; place-items: center; border: 1px solid var(--border-subtle); border-radius: 13px; background: var(--bg-glass); color: var(--accent); font-size: 20px; cursor: pointer; }
  .chat-panel { min-height: 0; flex: 1; }
  .message-list { min-height: 0; max-height: none; padding: 16px 12px 100px; overscroll-behavior-y: contain; }
  .chat-welcome { min-height: 100%; }
  .message { max-width: 92%; }
  .evidence-grid { grid-template-columns: 1fr; }
  .chat-composer { flex: none; margin: 0 8px 8px; }
  .chat-composer textarea { font-size: 16px; }
  .chat-composer button { width: 44px; height: 44px; }
  .save-answer { min-height: 44px; max-width: 100%; text-align: left; font-size: 12px; }
  .message > p { font-size: 15px; }
  .chat-welcome h2 { font-size: 19px; }
  .chat-welcome > button { min-height: 44px; font-size: 13px; }
  .source-preview-modal, .qa-inspector-modal { width: 100%; max-height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); border-radius: 24px 24px 0 0; }
  .source-preview-head { padding: 34px 16px 13px; }
  .source-preview-head h3 { font-size: 19px; }
  .source-preview-head > button { width: 44px; height: 44px; }
  .source-preview-body { padding: 18px 16px; }
  .source-preview-modal .knowledge-modal-actions { padding: 10px 16px 16px; }
}
@media (prefers-reduced-motion: reduce) {
  .chat-run-dots i { animation: none; opacity: .6; }
  .chat-run-label { animation: none; background: none; -webkit-text-fill-color: currentColor; }
}
@media (max-width: 768px) and (max-height: 500px) {
  .chat-mobile-context { display: none; }
  .chat-composer .chat-short-context-button { display: grid; background: var(--bg-glass-subtle); color: var(--accent); }
  .chat-welcome { gap: 6px; }
  .welcome-symbol { display: none; }
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
