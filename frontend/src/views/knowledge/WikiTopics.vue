<template>
  <KnowledgePageShell title="主题 Wiki">
    <div class="topic-toolbar knowledge-toolbar">
      <el-select v-model="baseId" class="knowledge-select is-responsive" popper-class="system-select-popper" aria-label="选择书籍" placeholder="选择一本书" :disabled="working || syncing || startingBuild || selectingTopic" @change="changeBook">
        <el-option v-for="book in books" :key="book.id" :label="book.book_name" :value="book.id" />
      </el-select>
      <div class="topic-actions">
        <el-button v-if="!selectingTopic" type="primary" :disabled="!writable || working || syncing || buildRunning" :loading="startingBuild" @click="startBuild">{{ buildActionLabel }}</el-button>
        <el-button v-if="writebackRunId" :disabled="working" @click="leaveWritebackDraft">取消整理</el-button>
        <el-button :disabled="!baseId || base?.lifecycle !== 'active'" @click="askBook">向本书提问</el-button>
        <el-button :disabled="!writable || working || buildRunning" :loading="syncing" @click="syncSources">同步来源</el-button>
        <el-button @click="$router.push('/knowledge')">返回书籍知识库</el-button>
      </div>
    </div>

    <p v-if="error" class="topic-alert" role="alert">{{ error }}</p>
    <section v-if="bookBuild && !selectingTopic" class="book-build knowledge-surface" aria-label="全库编译">
      <div class="book-build-header">
        <div>
          <h2>全库编译</h2>
          <p class="topic-note" role="status">{{ buildStatus }}</p>
        </div>
        <div class="topic-actions">
          <div v-if="bookBuild.sources.length" class="topic-note">{{ bookBuild.sources.length }} 份文档 · {{ builtPages }}/{{ bookBuild.pages.length }} 篇主题已生成</div>
          <el-button v-if="buildRunning" size="small" :loading="stoppingBuild" :disabled="bookBuild.stop_requested" @click="stopBuild">停止编译</el-button>
          <el-button v-if="readyDrafts.length" type="primary" size="small" :loading="publishingBuild" :disabled="!canMaintain || working || syncing || editingMarkdown !== null" @click="publishBuild">发布本轮 {{ readyDrafts.length }} 篇草稿</el-button>
        </div>
      </div>
      <p v-if="bookBuild.error" class="topic-alert" role="alert">{{ bookBuild.error }}</p>
      <p v-if="bookBuild.outdated && !buildRunning" class="topic-note">来源已有变化，可以更新全库 Wiki。</p>
      <p v-if="bookBuild.maintenance?.reused_pages" class="topic-note">复用 {{ bookBuild.maintenance.reused_pages }} 篇已有主题</p>
      <details v-if="bookBuild.maintenance?.changes.length">
        <summary>查看 {{ bookBuild.maintenance.changes.length }} 份来源的变化和处理结果</summary>
        <ul class="build-coverage">
          <li v-for="item in bookBuild.maintenance.changes" :key="item.path">
            <span>{{ item.path }}</span>
            <small>{{ sourceChangeLabels[item.change] }} · {{ maintenanceLabels[item.disposition] }} · {{ item.reason }}</small>
            <div v-if="item.page_ids.length" class="topic-actions">
              <el-button v-for="id in item.page_ids" :key="id" text :disabled="!pageById.has(id)" @click="openPage(id)">{{ topicTitle(id) }}</el-button>
            </div>
          </li>
        </ul>
      </details>
      <ul v-if="bookBuild.failed_groups.length || buildPageFailures.length" class="build-issues">
        <li v-for="group in bookBuild.failed_groups" :key="group.group">{{ outline.length ? '全书主题规划' : `第 ${group.group} 组资料` }} · {{ group.error }}</li>
        <li v-for="page in buildPageFailures" :key="page.page_id">{{ page.title }} · {{ page.error }}</li>
      </ul>
      <details v-if="bookBuild.sources.length">
        <summary>查看每份文档的处理结果</summary>
        <ul class="build-coverage">
          <li v-for="source in bookBuild.sources" :key="source.path">
            <span>{{ source.path }}</span>
            <small>{{ sourceStatus(source) }}</small>
            <div v-if="source.page_ids.length" class="topic-actions">
              <el-button v-for="id in source.page_ids" :key="id" text @click="openPage(id)">{{ topicTitle(id) }}</el-button>
            </div>
          </li>
        </ul>
      </details>
    </section>
    <p v-if="loading" role="status" class="topic-note">正在读取主题 Wiki…</p>
    <div v-else-if="!baseId" class="knowledge-empty knowledge-surface">
      <p>请选择已初始化的书籍知识库。</p>
      <el-button @click="$router.push('/knowledge')">查看书籍知识库</el-button>
    </div>

    <div v-else class="topic-layout">
      <nav class="topic-directory knowledge-surface" data-glass="structural" aria-label="主题目录">
        <header><h2>主题目录</h2><el-button :disabled="!writable || working || syncing || buildRunning || selectingTopic" @click="editPage()">补充主题</el-button></header>
        <button v-if="!selectingTopic" type="button" class="topic-link" :aria-current="!selected && !form ? 'page' : undefined" :disabled="working" @click="openWikiIndex">全书导览</button>
        <p v-if="!directoryPages.length && !outline.length" class="topic-note">{{ selectingTopic ? '没有可选的已发布主题。' : buildRunning ? '正在准备整书阅读顺序。' : '编译整个知识库后，可在这里阅读和审核主题页。' }}</p>
        <template v-if="outline.length">
          <button v-for="item in outline" :key="item.page_id" type="button" class="topic-link outline-link" :aria-current="selected?.page.id === item.page_id ? 'page' : undefined" :disabled="working || !pageById.has(item.page_id)" @click="openPage(item.page_id)">
            <span>{{ item.title }}</span>
            <span v-if="item.focus" class="topic-focus" :title="item.focus">{{ item.focus }}</span>
            <small v-if="item.error">可重试</small>
            <small v-else-if="!item.revision_id">{{ buildRunning ? '待生成' : '待继续' }}</small>
            <small v-else-if="pageById.get(item.page_id)?.needs_review">来源待复核</small>
            <small v-else-if="pageById.get(item.page_id)?.has_draft">有草稿</small>
          </button>
          <details v-if="otherPages.length" class="topic-other-pages">
            <summary>历史与补充主题（{{ otherPages.length }}）</summary>
            <button v-for="page in otherPages" :key="page.id" type="button" class="topic-link" :aria-current="selected?.page.id === page.id ? 'page' : undefined" :disabled="working" @click="openPage(page.id)">{{ page.title }}</button>
          </details>
        </template>
        <template v-else>
          <button v-for="page in directoryPages" :key="page.id" type="button" :aria-current="selected?.page.id === page.id ? 'page' : undefined" class="topic-link" :disabled="working" @click="openPage(page.id)">
            <span>{{ page.title }}</span>
            <small v-if="page.compiling">编写中</small>
            <small v-else-if="page.needs_review">来源待复核</small>
            <small v-else-if="page.last_error">可重试</small>
            <small v-else-if="page.has_draft">有草稿</small>
          </button>
        </template>
      </nav>

      <section v-if="form" class="topic-content knowledge-surface" data-glass="panel" aria-label="编译主题页">
        <header><h2>{{ form.page_id ? '修订主题页' : '新建主题页' }}</h2></header>
        <form class="topic-form" @submit.prevent="compile">
          <label>主题标题<el-input v-model="form.title" :disabled="Boolean(form.page_id) || working" maxlength="120" placeholder="例如 淘汰策略在不同负载下的取舍" /></label>
          <label>编辑要求（可选）<el-input v-model="form.guidance" type="textarea" :rows="3" :disabled="working" maxlength="2000" placeholder="例如 保留实验条件，说明是否测得尾延迟" /></label>
          <fieldset class="topic-source-picker" :disabled="working">
            <legend>选取原文</legend>
            <label>来源文件<el-select v-model="documentId" filterable class="knowledge-select is-fluid" popper-class="system-select-popper" aria-label="来源文件" placeholder="选择文件" @change="loadSections(false)">
              <el-option v-for="document in documents" :key="document.id" :label="document.relative_path" :value="document.id" />
            </el-select></label>
            <template v-if="documentId">
              <label>范围<el-select v-model="sectionId" class="knowledge-select is-fluid" popper-class="system-select-popper" aria-label="范围" :loading="sectionsLoading" :disabled="sectionsLoading">
                <el-option label="整个文件" value="" />
                <el-option v-for="section in sections" :key="section.id" :value="section.id" :label="sectionLabel(section)" />
              </el-select></label>
              <el-button v-if="hasMoreSections" :loading="sectionsLoading" @click="loadSections(true)">加载更多章节</el-button>
              <div v-if="chosenSection && (chosenSection.total_chars > 12000 || useRange)" class="topic-range-fields">
                <label>起点（从 0 开始）<el-input-number v-model="sourceOffset" :min="0" :max="chosenSection.total_chars - 1" :step="1" step-strictly controls-position="right" /></label>
                <label>字符长度<el-input-number v-model="sourceLength" :min="1" :max="Math.min(12000, chosenSection.total_chars - (sourceOffset || 0))" :step="1" step-strictly controls-position="right" /></label>
              </div>
              <el-checkbox v-if="chosenSection && chosenSection.total_chars <= 12000" v-model="useRange">只取其中一段</el-checkbox>
              <el-button :disabled="sectionsLoading || form.sources.length >= 12" @click="addSource">添加来源</el-button>
            </template>
            <ul v-if="form.sources.length" class="topic-selected-sources">
              <li v-for="(source, index) in form.sources" :key="`${source.document_id}-${source.span_id || ''}-${source.range?.offset ?? 'full'}-${source.range?.length ?? 0}`">
                <span>{{ sourceName(source) }}</span>
                <el-button text :disabled="working" :aria-label="`移除 ${sourceName(source)}`" @click="form.sources.splice(index, 1)">移除</el-button>
              </li>
            </ul>
            <p v-else-if="!documents.length" class="topic-note">同步原文后即可选取来源。</p>
          </fieldset>
          <div class="topic-actions">
            <el-button native-type="submit" type="primary" :loading="working" :disabled="!writable || syncing || buildRunning || startingBuild || (form.page_id && selected?.page.compiling) || !form.title.trim() || !form.sources.length">生成草稿</el-button>
            <el-button :disabled="working" @click="form = null">取消</el-button>
          </div>
          <p v-if="working" class="topic-note" role="status">正在编写这一页。可返回工作台，稍后回来查看草稿。</p>
        </form>
      </section>

      <article v-else-if="selected" ref="readingPane" tabindex="-1" class="topic-content knowledge-surface" data-glass="card">
        <header>
          <h2>{{ selected.page.title }}</h2>
          <div class="topic-actions">
            <el-button v-if="writebackRunId" type="primary" :disabled="!canDraftWriteback || reading || syncing" :loading="working" @click="saveWritebackDraft">添加为本页草稿</el-button>
            <el-button v-else-if="questionRunId" type="primary" :disabled="!canMaintain || !selected.published" :loading="working" @click="saveQuestion">加入本页待验证问题</el-button>
            <el-button v-else-if="selected.page.compiling" :loading="reading" @click="openPage(selected.page.id)">刷新进度</el-button>
            <el-button v-else :disabled="!writable || working || syncing || buildRunning || editingMarkdown !== null" @click="editPage(selected)">{{ selected.page.last_error ? '调整来源并重试' : '修订本页' }}</el-button>
            <template v-if="!selectingTopic && selected.draft && shown?.id === selected.draft.id && editingMarkdown === null">
              <el-button :disabled="!canMaintain || selected.page.compiling || working" @click="editingMarkdown = shown.markdown">编辑草稿</el-button>
              <el-button type="primary" :disabled="!canMaintain || selected.page.compiling" :loading="working" @click="publish">发布草稿</el-button>
            </template>
          </div>
        </header>
        <section v-if="writebackRunId" class="topic-cross-references" aria-label="整理知识片段">
          <p v-if="writebackMaterialLoading" class="topic-note" role="status">正在读取片段和出处。</p>
          <p v-else-if="writebackMaterialError" class="topic-alert" role="alert">{{ writebackMaterialError }}</p>
          <template v-else-if="writebackMaterial">
            <h3>{{ writebackMaterial.title }}</h3>
            <p v-if="selected.draft" class="topic-note">请先审核本页已有的草稿。</p>
            <template v-if="writebackMaterial.passages.length">
              <label class="topic-form">选择要加入的片段
                <el-select v-model="selectedPassages" multiple aria-label="知识片段" class="knowledge-select is-fluid" :disabled="working">
                  <el-option v-for="passage in writebackMaterial.passages" :key="passage.index" :label="passage.title" :value="passage.index" />
                </el-select>
              </label>
              <details v-if="chosenPassages.length">
                <summary>预览所选片段</summary>
                <KnowledgeAnswerMarkdown v-for="passage in chosenPassages" :key="passage.index" :content="passage.markdown" :evidence-count="writebackMaterial.evidence_count" @citation="viewWritebackCitation" />
              </details>
            </template>
            <p v-else class="topic-note">没有找到引用本书原文的片段。{{ writebackMaterial.kind === 'answer' ? '可以把问题留作待验证事项。' : '' }}</p>
          </template>
        </section>
        <p v-else-if="questionRunId" class="topic-note">从目录选择相关的已发布主题，再把刚才的问题存入该页。</p>
        <p v-if="selected.page.needs_review" class="topic-alert" role="status">引用的原文已更新。原版供查阅，发布修订后问答会采用新版本。</p>
        <p v-if="selected.page.last_error" class="topic-alert" role="alert">{{ selected.page.last_error }}</p>
        <p v-if="selected.page.compiling" class="topic-note" role="status">正在编写主题页。</p>
        <label v-if="selected.revisions.length" class="topic-versions">阅读版本
          <el-select :model-value="shown?.id" class="knowledge-select" popper-class="system-select-popper" aria-label="阅读版本" :loading="reading" :disabled="editingMarkdown !== null || working" @change="showRevision">
            <el-option v-for="version in selected.revisions" :key="version.id" :value="version.id" :label="versionLabel(version)" />
          </el-select>
        </label>
        <form v-if="editingMarkdown !== null" class="topic-form" @submit.prevent="saveMarkdown">
          <label>Markdown 正文<el-input v-model="editingMarkdown" type="textarea" :rows="14" maxlength="12000" :disabled="working" /></label>
          <div class="topic-actions">
            <el-button native-type="submit" type="primary" :loading="working" :disabled="!editingMarkdown.trim()">保存草稿</el-button>
            <el-button :disabled="working" @click="editingMarkdown = null">取消编辑</el-button>
          </div>
        </form>
        <KnowledgeAnswerMarkdown v-else-if="shown" :content="shown.markdown" :evidence-count="shown.evidence_count" @citation="viewCitation" />
        <p v-else-if="!selected.page.compiling" class="topic-note">这篇主题页还没有可阅读的版本。</p>
        <section v-if="shown?.links?.length && editingMarkdown === null" class="topic-cross-references">
          <h3>延伸阅读</h3>
          <ul>
            <li v-for="link in shown.links" :key="link.page_id">
              <button v-if="link.published" type="button" class="topic-text-link" :disabled="working" @click="openPage(link.page_id)">{{ link.title }}</button>
              <span v-else>{{ link.title }} · 待发布</span>
              <p>{{ link.reason }}</p>
            </li>
          </ul>
        </section>
        <section v-if="selected.backlinks?.length && editingMarkdown === null" class="topic-cross-references">
          <h3>关联到本页的主题</h3>
          <ul>
            <li v-for="link in selected.backlinks" :key="link.page_id">
              <button type="button" class="topic-text-link" :disabled="working" @click="openPage(link.page_id)">{{ link.title }}</button>
              <p>{{ link.reason }}</p>
            </li>
          </ul>
        </section>
        <section v-if="selected.questions.length" class="topic-questions">
          <h3>待验证问题</h3>
          <ul>
            <li v-for="item in selected.questions" :key="item.id">
              <span>{{ item.question }}</span>
              <small v-if="item.resolved_at">已解决</small>
              <el-button v-else text :disabled="!canMaintain || working" @click="resolveQuestion(item.id)">标为已解决</el-button>
            </li>
          </ul>
        </section>
      </article>

      <section v-else-if="writebackRunId" class="topic-content knowledge-surface" data-glass="card" aria-label="选择主题">
        <h2>选择要补充的主题</h2>
        <p v-if="writebackMaterial" class="topic-note">{{ writebackMaterial.title }}</p>
        <p v-if="writebackMaterialError" class="topic-alert" role="alert">{{ writebackMaterialError }}</p>
        <p v-else class="topic-note">从目录打开相关主题，再选择要保留的知识片段。</p>
      </section>
      <section v-else class="topic-content knowledge-surface" data-glass="card" aria-label="全书导览">
        <h2>全书导览</h2>
        <p v-if="wikiIndex?.overview" class="wiki-overview">{{ wikiIndex.overview }}</p>
        <ol v-if="wikiIndex?.pages.length" class="wiki-index">
          <li v-for="page in wikiIndex.pages" :key="page.page_id">
            <button type="button" class="topic-text-link" :disabled="reading" @click="openPage(page.page_id)">{{ page.title }}</button>
            <small v-if="page.needs_review" class="topic-note">来源待复核</small>
            <p v-if="page.summary || page.scope">{{ page.summary || page.scope }}</p>
          </li>
        </ol>
        <p v-else class="topic-note">{{ reading ? '正在读取主题…' : '主题页发布后会显示在这里。' }}</p>
        <details v-if="wikiIndex?.issues.length" class="wiki-index-details">
          <summary>结构体检（{{ wikiIndex.issues.length }} 项待检查）</summary>
          <ul>
            <li v-for="(issue, index) in wikiIndex.issues" :key="`${issue.page_id}-${index}`">
              <button type="button" class="topic-text-link" @click="openPage(issue.page_id)">{{ topicTitle(issue.page_id) }}</button>
              <p>{{ issue.detail }}</p>
            </li>
          </ul>
        </details>
        <WikiTopicReview v-if="baseId && !selectingTopic" :base-id="baseId" :can-run="canMaintain && !buildRunning && Boolean(wikiIndex?.pages.length)" :revision-key="JSON.stringify(wikiIndex?.pages.map(page => [page.page_id, page.needs_review])) + (wikiIndex?.publications[0]?.revision_id ?? '')" @open-page="openPage" />
        <details v-if="wikiIndex?.builds.length || wikiIndex?.publications.length" class="wiki-index-details">
          <summary>最近维护记录</summary>
          <ul>
            <li v-for="build in wikiIndex?.builds" :key="build.attempt_id">
              {{ dateLabel(build.recorded_at) }} · {{ build.status === 'ready' ? '编译完成' : build.status === 'partial' ? '编译部分完成' : '编译失败' }}
              <p>{{ build.source_count }} 份原文，{{ build.linked_sources }} 份被引用，生成 {{ build.written_pages }} 篇主题<span v-if="build.maintenance">，复用 {{ build.maintenance.reused_pages }} 篇</span></p>
              <ul v-if="build.maintenance?.changes.length">
                <li v-for="item in build.maintenance.changes" :key="item.path">{{ item.path }} · {{ maintenanceLabels[item.disposition] }} · {{ item.reason }}</li>
              </ul>
            </li>
            <li v-for="release in wikiIndex?.publications" :key="release.revision_id">
              {{ dateLabel(release.published_at) }} · 发布 <button type="button" class="topic-text-link" @click="openPage(release.page_id)">{{ release.title }}</button>
            </li>
          </ul>
        </details>
      </section>
    </div>

    <KnowledgeCitationPreview v-model="citationVisible" :entry="null" :run-id="citationRunId" :source-index="citationIndex" />
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { listBookKnowledgeBases, syncBookKnowledgeBase, type KnowledgeBaseSummary } from '@/api/knowledge'
import {
  compileTopicPage, draftTopicFromMaterial, editTopicMarkdown, getTopicWritebackMaterial, getTopicBookBuild, getTopicPage, getTopicRevision, getTopicWikiIndex, listTopicPages, listTopicSources, listTopicSourceSections,
  publishTopicBookBuild, publishTopicPage, resolveTopicQuestion, saveTopicQuestion, startTopicBookBuild, stopTopicBookBuild, topicErrorMessage, type TopicBookBuild, type TopicCompileRequest, type TopicPage, type TopicPageSummary,
  type TopicWritebackMaterial, type TopicRevision, type TopicSourceDocument, type TopicSourceSection, type TopicSourceSelection, type TopicWikiIndex,
} from '@/api/wikiTopics'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import KnowledgeAnswerMarkdown from '@/components/knowledge/KnowledgeAnswerMarkdown.vue'
import KnowledgeCitationPreview from '@/components/knowledge/KnowledgeCitationPreview.vue'
import WikiTopicReview from '@/components/knowledge/WikiTopicReview.vue'

const route = useRoute()
const router = useRouter()
const books = ref<KnowledgeBaseSummary[]>([])
const baseId = ref('')
const base = computed(() => books.value.find(book => book.id === baseId.value))
const canMaintain = computed(() => base.value?.lifecycle === 'active')
const writable = computed(() => canMaintain.value && base.value?.source_available)
const questionRunId = computed(() => typeof route.query.questionRun === 'string' && route.query.base === baseId.value ? route.query.questionRun : '')
const writebackRunId = computed(() => typeof route.query.writebackRun === 'string' && route.query.base === baseId.value ? route.query.writebackRun : '')
const selectingTopic = computed(() => Boolean(questionRunId.value || writebackRunId.value))
const writebackMaterial = ref<TopicWritebackMaterial | null>(null)
const writebackMaterialError = ref('')
const writebackMaterialLoading = ref(false)
const selectedPassages = ref<number[]>([])
const chosenPassages = computed(() => writebackMaterial.value?.passages.filter(passage => selectedPassages.value.includes(passage.index)) ?? [])
const pages = ref<TopicPageSummary[]>([])
const directoryPages = computed(() => selectingTopic.value ? pages.value.filter(page => page.has_publication) : pages.value)
const wikiIndex = ref<TopicWikiIndex | null>(null)
const bookBuild = ref<TopicBookBuild | null>(null)
const buildRunning = computed(() => bookBuild.value?.status === 'running')
const startingBuild = ref(false)
const stoppingBuild = ref(false)
const publishingBuild = ref(false)
const pageById = computed(() => new Map(pages.value.map(page => [page.id, page])))
const outline = computed(() => selectingTopic.value ? [] : bookBuild.value?.pages.filter(page => page.focus) ?? [])
const otherPages = computed(() => {
  const planned = new Set(outline.value.map(item => item.page_id))
  return pages.value.filter(page => !planned.has(page.id))
})
const readyDrafts = computed(() => bookBuild.value?.phase === 'finished' && !bookBuild.value.outdated && !bookBuild.value.error
  ? outline.value.filter(item => !item.error && item.revision_id && pageById.value.get(item.page_id)?.draft_revision_id === item.revision_id)
  : [])
const sourceChangeLabels = { added: '新增', modified: '修改', removed: '移除', unreadable: '读取失败', empty: '正文为空' }
const maintenanceLabels = { update: '更新主题', add: '补充主题', no_change: '无需改页', review: '待核查' }
const builtPages = computed(() => bookBuild.value?.pages.filter(page => page.revision_id).length ?? 0)
const buildPageFailures = computed(() => bookBuild.value?.pages.filter(page => page.error) ?? [])
const buildActionLabel = computed(() => {
  if (buildRunning.value) return '全库编译中'
  if (!bookBuild.value) return '编译整个知识库'
  return bookBuild.value.status === 'ready' ? '更新全库 Wiki' : '继续全库编译'
})
const buildStatus = computed(() => {
  const build = bookBuild.value
  if (!build) return ''
  if (build.status === 'running' && build.stop_requested) return '当前步骤完成后将停止编译。'
  if (build.phase === 'stopped') return '编译已停止，已完成的结果可以继续使用。'
  if (build.phase === 'syncing') return '正在同步知识库中的 Markdown 文档'
  if (build.status === 'running' && build.phase === 'planning') return '正在组织整书主题与阅读顺序。'
  if (build.status === 'running') return '正在生成主题页，可以先阅读已完成的草稿。'
  if (build.status === 'failed') return '全库编译未完成，可以继续。'
  if (build.status === 'partial') return '部分完成，可继续处理失败项。'
  if (build.maintenance?.reused_pages && build.maintenance.reused_pages === build.pages.length) return '已同步原文，现有主题可继续使用。'
  return build.pages.length ? '全库编译完成。审核草稿并发布后，问答将使用这些主题。' : '原文已完整索引，本次没有生成主题页。'
})
const documents = ref<TopicSourceDocument[]>([])
const selected = ref<TopicPage | null>(null)
const canDraftWriteback = computed(() => canMaintain.value && !buildRunning.value && Boolean(selected.value?.published)
  && !selected.value?.page.needs_review && !selected.value?.page.compiling && !selected.value?.draft && chosenPassages.value.length > 0)
const readingPane = ref<HTMLElement | null>(null)
const shown = ref<TopicRevision | null>(null)
const form = ref<TopicCompileRequest | null>(null)
const editingMarkdown = ref<string | null>(null)
const loading = ref(true)
const reading = ref(false)
const working = ref(false)
const syncing = ref(false)
const error = ref('')
const documentId = ref('')
const sectionId = ref('')
const sections = ref<TopicSourceSection[]>([])
const sectionNames = new Map<string, string>()
const sectionsLoading = ref(false)
const hasMoreSections = ref(false)
const chosenSection = computed(() => sections.value.find(section => section.id === sectionId.value))
const useRange = ref(false)
const sourceOffset = ref<number | undefined>(0)
const sourceLength = ref<number | undefined>(3000)
const citationVisible = ref(false)
const citationIndex = ref(0)
const citationRunId = ref<string>()
let bookRequest = 0
let pageRequest = 0
let sourceRequest = 0
let active = true
let polling = false
let buildTimer: number | undefined

function sectionLabel(section: TopicSourceSection) {
  const position = section.line_start ? ` · 第 ${section.line_start} 行` : ''
  return `${section.heading || '正文'}${position} · ${section.total_chars} 字符`
}

function sourceName(source: TopicSourceSelection) {
  const path = documents.value.find(document => document.id === source.document_id)?.relative_path || '已移除的来源'
  const section = source.span_id ? sectionNames.get(source.span_id) || '已选章节' : '整个文件'
  const range = source.range ? ` · ${source.range.offset}–${source.range.offset + source.range.length} 字符` : ''
  return `${path} · ${section}${range}`
}

function versionLabel(version: TopicPage['revisions'][number]) {
  const label = version.id === selected.value?.published?.id ? '已发布' : version.id === selected.value?.draft?.id ? '待审核草稿' : version.published_at ? '历史发布版本' : '历史草稿'
  return `${label} · ${new Date(version.published_at || version.created_at).toLocaleString('zh-CN')}`
}

function viewCitation(index: number, runId = shown.value?.run_id) {
  citationRunId.value = runId
  citationIndex.value = index
  citationVisible.value = true
}

function viewWritebackCitation(index: number) {
  viewCitation(index, writebackRunId.value)
}

watch(writebackRunId, async runId => {
  const book = baseId.value
  writebackMaterial.value = null
  writebackMaterialError.value = ''
  writebackMaterialLoading.value = Boolean(runId)
  selectedPassages.value = []
  if (!runId) return
  const stillCurrent = () => active && runId === writebackRunId.value && book === baseId.value
  try {
    const material = await getTopicWritebackMaterial(book, runId)
    if (!stillCurrent()) return
    writebackMaterial.value = material
    if (material.passages.length === 1) selectedPassages.value = [material.passages[0].index]
  } catch (failure) {
    if (stillCurrent()) writebackMaterialError.value = topicErrorMessage(failure)
  } finally {
    if (stillCurrent()) writebackMaterialLoading.value = false
  }
})

function askBook() {
  void router.push({ path: '/knowledge/chat', query: { base: baseId.value } })
}

function topicTitle(id: string) {
  return pages.value.find(page => page.id === id)?.title || bookBuild.value?.pages.find(page => page.page_id === id)?.title || '打开主题'
}

function sourceStatus(source: TopicBookBuild['sources'][number]) {
  if (source.error) return source.error
  if (source.status === 'empty') return '空文档'
  if (source.status === 'linked') return `已关联 ${source.page_ids.length} 篇主题 · ${source.linked_sections}/${source.total_sections} 节提供依据`
  return '原文可查，尚无主题引用'
}

function firstReadablePage() {
  if (questionRunId.value) return pages.value.find(page => page.has_publication)?.id
  if (route.query.pending === '1') return pages.value.find(page => page.has_draft)?.id
}

function dateLabel(value: string) {
  return new Date(value).toLocaleString('zh-CN', { dateStyle: 'short', timeStyle: 'short' })
}

async function refreshWikiIndex(book: string) {
  const request = bookRequest
  try {
    const index = await getTopicWikiIndex(book)
    if (active && book === baseId.value && request === bookRequest) wikiIndex.value = index
  } catch (failure) {
    if (active && book === baseId.value && request === bookRequest) error.value = topicErrorMessage(failure)
  }
}

function openWikiIndex() {
  ++pageRequest
  selected.value = null
  shown.value = null
  form.value = null
  reading.value = false
  editingMarkdown.value = null
  error.value = ''
  void router.replace({ query: { base: baseId.value } })
  void refreshWikiIndex(baseId.value)
}

async function startBuild() {
  if (!writable.value || startingBuild.value || buildRunning.value) return
  const id = baseId.value
  startingBuild.value = true
  error.value = ''
  try {
    const build = await startTopicBookBuild(id)
    if (active && id === baseId.value) {
      bookBuild.value = build
      await refreshBuild(true)
    }
  } catch (failure) {
    if (active && id === baseId.value) error.value = topicErrorMessage(failure)
  } finally { startingBuild.value = false }
}

async function stopBuild() {
  const id = baseId.value
  const attempt = bookBuild.value?.id
  if (!id || !attempt || !buildRunning.value || stoppingBuild.value) return
  stoppingBuild.value = true
  try {
    const build = await stopTopicBookBuild(id, attempt)
    if (active && id === baseId.value && bookBuild.value?.id === build.id && buildRunning.value) bookBuild.value = build
  } catch (failure) {
    if (active && id === baseId.value) error.value = topicErrorMessage(failure)
  } finally { stoppingBuild.value = false }
}

async function publishBuild() {
  const id = baseId.value
  const attempt = bookBuild.value?.id
  if (!attempt || publishingBuild.value || !readyDrafts.value.length || !canMaintain.value) return
  publishingBuild.value = true
  error.value = ''
  try {
    await publishTopicBookBuild(id, attempt)
    if (!active || id !== baseId.value) return
    const selectedId = selected.value?.page.id
    await refreshBuild(true)
    if (selectedId && active && id === baseId.value) await openPage(selectedId, false)
  } catch (failure) {
    if (active && id === baseId.value) error.value = topicErrorMessage(failure)
  } finally { publishingBuild.value = false }
}

async function refreshBuild(force = false) {
  if ((!force && !buildRunning.value) || polling || loading.value) return
  const id = baseId.value
  const request = bookRequest
  polling = true
  try {
    const [build, result, nextSources] = await Promise.all([getTopicBookBuild(id), listTopicPages(id), listTopicSources(id)])
    if (!active || request !== bookRequest || id !== baseId.value) return
    bookBuild.value = build
    pages.value = result
    documents.value = nextSources
    if (build?.status !== 'running') await refreshWikiIndex(id)
    if (!active || request !== bookRequest || id !== baseId.value) return
    if (form.value || editingMarkdown.value !== null || reading.value || working.value) return
    const selectedPage = selected.value
    const target = firstReadablePage()
    if (!selectedPage && target) await openPage(target, false)
    else if (selectedPage?.page.compiling) {
      const current = result.find(page => page.id === selectedPage.page.id)
      if (current && !current.compiling) await openPage(current.id, false)
    }
  } catch (failure) {
    if (active && request === bookRequest) error.value = topicErrorMessage(failure)
  } finally { polling = false }
}

async function loadBook(preferredPage = '') {
  const id = baseId.value
  const request = ++bookRequest
  ++pageRequest
  ++sourceRequest
  loading.value = true
  reading.value = false
  selected.value = null
  shown.value = null
  form.value = null
  editingMarkdown.value = null
  pages.value = []
  wikiIndex.value = null
  bookBuild.value = null
  documents.value = []
  sectionNames.clear()
  error.value = ''
  if (!id) { loading.value = false; return }
  try {
    const [nextPages, nextSources, nextBuild, index] = await Promise.all([listTopicPages(id), listTopicSources(id), getTopicBookBuild(id), getTopicWikiIndex(id)])
    if (request !== bookRequest) return
    pages.value = nextPages
    documents.value = nextSources
    bookBuild.value = nextBuild
    wikiIndex.value = index
    const target = preferredPage || firstReadablePage()
    if (target) await openPage(target, false)
  } catch (failure) {
    if (request === bookRequest) error.value = topicErrorMessage(failure)
  } finally {
    if (request === bookRequest) loading.value = false
  }
}

async function changeBook() {
  void router.replace({ query: { base: baseId.value || undefined } })
  await loadBook()
}

function receivePage(page: TopicPage, draft = false) {
  selected.value = page
  shown.value = draft ? page.draft || page.published : page.published || page.draft
  const index = pages.value.findIndex(item => item.id === page.page.id)
  if (index === -1) pages.value.push(page.page)
  else pages.value[index] = page.page
  pages.value.sort((a, b) => a.title.localeCompare(b.title, 'zh-CN'))
}

async function openPage(id: string, updateUrl = true) {
  const request = ++pageRequest
  const book = baseId.value
  error.value = ''
  reading.value = true
  try {
    const page = await getTopicPage(book, id)
    if (request !== pageRequest) return
    form.value = null
    editingMarkdown.value = null
    const currentDraft = page.draft?.id
    receivePage(page, Boolean(currentDraft && (route.query.pending === '1' || outline.value.some(item => item.page_id === id && item.revision_id === currentDraft))))
    if (updateUrl) {
      void router.replace({ query: { base: book, page: id, questionRun: questionRunId.value || undefined, writebackRun: writebackRunId.value || undefined } })
      await nextTick()
      if (request !== pageRequest || !readingPane.value) return
      readingPane.value.scrollTop = 0
      if (window.matchMedia('(max-width: 1100px)').matches) {
        readingPane.value.scrollIntoView({ block: 'start' })
        readingPane.value.focus({ preventScroll: true })
      }
    }
  } catch (failure) {
    if (request === pageRequest) error.value = topicErrorMessage(failure)
  } finally {
    if (request === pageRequest) reading.value = false
  }
}

async function showRevision(id: string) {
  if (!selected.value) return
  const request = ++pageRequest
  reading.value = true
  error.value = ''
  try {
    const page = selected.value
    const revision = page.published?.id === id ? page.published : page.draft?.id === id ? page.draft : await getTopicRevision(baseId.value, page.page.id, id)
    if (request === pageRequest) shown.value = revision
  } catch (failure) {
    if (request === pageRequest) error.value = topicErrorMessage(failure)
  } finally {
    if (request === pageRequest) reading.value = false
  }
}

function editPage(page?: TopicPage) {
  ++pageRequest
  ++sourceRequest
  sectionsLoading.value = false
  hasMoreSections.value = false
  reading.value = false
  error.value = ''
  documentId.value = ''
  sectionId.value = ''
  sections.value = []
  form.value = {
    page_id: page?.page.id,
    title: page?.page.title || '',
    guidance: page?.last_request?.guidance || '',
    sources: page?.last_request?.sources.map(source => ({ ...source })) || [],
  }
}

async function loadSections(append: boolean) {
  const request = ++sourceRequest
  if (!append) { sections.value = []; sectionId.value = ''; hasMoreSections.value = false }
  if (!documentId.value) return
  sectionsLoading.value = true
  try {
    const result = await listTopicSourceSections(baseId.value, documentId.value, sections.value.length)
    if (request !== sourceRequest) return
    for (const section of result) sectionNames.set(section.id, sectionLabel(section))
    sections.value = [...sections.value, ...result]
    hasMoreSections.value = result.length === 50
  } catch (failure) {
    if (request === sourceRequest) error.value = topicErrorMessage(failure)
  } finally {
    if (request === sourceRequest) sectionsLoading.value = false
  }
}

watch(chosenSection, section => {
  useRange.value = false
  sourceOffset.value = 0
  sourceLength.value = Math.min(3000, section?.total_chars ?? 3000)
})

function addSource() {
  if (!form.value || !documentId.value || form.value.sources.length >= 12) return
  const section = chosenSection.value
  const selection: TopicSourceSelection = { document_id: documentId.value, span_id: section?.id }
  if (section && (useRange.value || section.total_chars > 12000)) {
    const offset = sourceOffset.value
    const length = sourceLength.value
    if (offset === undefined || length === undefined || length < 1 || offset + length > section.total_chars) {
      error.value = '请填写有效的原文起点和长度。'
      return
    }
    selection.range = { offset, length }
  }
  const existing = form.value.sources.filter(source => source.document_id === selection.document_id)
  if (existing.some(source => !source.span_id || !selection.span_id || source.span_id === selection.span_id)) {
    error.value = '这个文件或章节已被选中。更换范围前请移除原选择。'
    return
  }
  error.value = ''
  form.value.sources.push(selection)
}

async function compile() {
  if (!form.value || working.value || !writable.value) return
  const book = baseId.value
  const request = form.value
  working.value = true
  error.value = ''
  try {
    const page = await compileTopicPage(book, request)
    if (!active || book !== baseId.value) return
    receivePage(page, true)
    form.value = null
    void router.replace({ query: { base: book, page: page.page.id } })
  } catch (failure) {
    if (!active || book !== baseId.value) return
    error.value = topicErrorMessage(failure)
    try { pages.value = await listTopicPages(book) } catch { /* Keep the compilation error and the current form. */ }
  } finally {
    working.value = false
  }
}

async function publish() {
  const revision = selected.value?.draft?.id
  if (revision && await maintainPage((book, id) => publishTopicPage(book, id, revision))) {
    void refreshWikiIndex(baseId.value)
  }
}

async function maintainPage(action: (book: string, id: string) => Promise<TopicPage>, draft = false) {
  if (!selected.value || working.value || !canMaintain.value) return false
  const book = baseId.value
  const id = selected.value.page.id
  working.value = true
  error.value = ''
  try {
    const page = await action(book, id)
    if (!active || book !== baseId.value) return false
    receivePage(page, draft)
    return true
  } catch (failure) {
    if (active && book === baseId.value) error.value = topicErrorMessage(failure)
    return false
  } finally { working.value = false }
}

async function saveMarkdown() {
  const markdown = editingMarkdown.value
  const revisionId = selected.value?.draft?.id
  if (!markdown?.trim() || !revisionId) return
  if (await maintainPage((book, id) => editTopicMarkdown(book, id, revisionId, markdown), true)) editingMarkdown.value = null
}

async function saveQuestion() {
  const runId = questionRunId.value
  if (!runId) return
  if (await maintainPage((book, id) => saveTopicQuestion(book, id, runId))) {
    void router.replace({ query: { base: baseId.value, page: selected.value?.page.id } })
  }
}

function leaveWritebackDraft() {
  void router.replace({ query: { base: baseId.value, page: selected.value?.page.id } })
}

async function saveWritebackDraft() {
  const runId = writebackRunId.value
  const revision = selected.value?.published?.id
  if (!runId || !revision || !canDraftWriteback.value) return
  const passages = chosenPassages.value.map(passage => passage.index)
  if (await maintainPage((book, id) => draftTopicFromMaterial(book, id, runId, revision, passages), true)) {
    leaveWritebackDraft()
  }
}

function resolveQuestion(questionId: string) {
  return maintainPage((book, id) => resolveTopicQuestion(book, id, questionId))
}

async function syncSources() {
  const book = baseId.value
  syncing.value = true
  error.value = ''
  try {
    const response = await syncBookKnowledgeBase(book)
    if (response.status !== 'success') throw new Error(response.error?.message || '来源同步失败')
    if (!active || book !== baseId.value) return
    if (form.value) {
      const [nextPages, nextSources] = await Promise.all([listTopicPages(book), listTopicSources(book)])
      if (!active || book !== baseId.value) return
      pages.value = nextPages
      documents.value = nextSources
      if (documentId.value) await loadSections(false)
    } else await loadBook(selected.value?.page.id)
  } catch (failure) {
    if (active && book === baseId.value) error.value = topicErrorMessage(failure)
  } finally { syncing.value = false }
}

onMounted(async () => {
  buildTimer = window.setInterval(() => { void refreshBuild() }, 1_300)
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    if (!active) return
    const cards = response.result.items.filter(card => card.book.kind === 'folder' && card.knowledge_base)
    const pending = route.query.pending === '1'
    books.value = cards.flatMap(card => card.knowledge_base ? [card.knowledge_base] : [])
    const wanted = String(route.query.base || '')
    const firstPending = pending ? cards.find(card => card.topic_draft_count > 0)?.knowledge_base?.id : undefined
    baseId.value = books.value.find(book => book.id === wanted)?.id || firstPending || books.value[0]?.id || ''
    await loadBook(wanted === baseId.value ? String(route.query.page || '') : '')
  } catch (failure) {
    if (active) error.value = topicErrorMessage(failure)
  } finally { loading.value = false }
})

onBeforeUnmount(() => {
  if (buildTimer !== undefined) window.clearInterval(buildTimer)
  active = false
  ++bookRequest
  ++pageRequest
  ++sourceRequest
})
</script>

<style scoped>
.topic-toolbar, .topic-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.topic-actions :deep(.el-button + .el-button) { margin-left: 0; }
.topic-layout { display: grid; grid-template-columns: minmax(210px, 270px) minmax(0, 1fr); align-items: start; gap: 14px; }
.topic-directory, .topic-content { display: grid; gap: 14px; padding: clamp(14px, 2vw, 24px); }
.topic-directory { align-content: start; }
.topic-directory header, .topic-content header { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 10px; }
.topic-directory h2, .topic-content h2 { margin: 0; overflow-wrap: anywhere; }
.topic-directory h2 { font-size: 1rem; }
.book-build { display: grid; gap: 12px; padding: 14px; margin-bottom: 14px; }
.book-build-header { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; justify-content: space-between; }
.book-build-header h2 { margin: 0 0 4px; font-size: 1rem; }
.book-build summary { width: fit-content; cursor: pointer; color: var(--accent); }
.build-coverage, .build-issues { display: grid; gap: 6px; margin: 6px 0 0; padding: 0; list-style: none; overflow-wrap: anywhere; }
.build-coverage { max-height: 360px; overflow: auto; }
.build-coverage li, .build-issues li { display: grid; gap: 2px; padding: 6px; border-bottom: 1px solid var(--border-faint); }
.build-coverage small { color: var(--text-muted); }
.topic-link { display: grid; justify-items: start; gap: 4px; padding: 10px 12px; border: 1px solid transparent; border-radius: 10px; background: transparent; text-align: left; color: var(--text-primary); cursor: pointer; overflow-wrap: anywhere; }
.topic-link:hover, .topic-link[aria-current='page'] { background: var(--bg-glass-subtle); border-color: var(--accent-border); }
.topic-link small, .topic-note { color: var(--text-muted); }
.topic-link:disabled { cursor: default; }
.topic-focus { color: var(--text-muted); font-size: .82em; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.topic-other-pages summary { padding: 6px; color: var(--text-muted); cursor: pointer; }
.topic-other-pages .topic-link { width: 100%; }
.topic-note, .topic-alert { margin: 0; line-height: 1.6; }
.topic-alert { padding: 10px 12px; border: 1px solid var(--accent-border); border-radius: 10px; color: var(--text-primary); overflow-wrap: anywhere; }
.topic-form, .topic-source-picker { display: grid; gap: 12px; }
.topic-form label, .topic-versions { display: grid; gap: 6px; }
.topic-versions { max-width: 300px; }
.topic-source-picker { margin: 0; padding: 12px; min-width: 0; border: 1px solid var(--accent-border); border-radius: 10px; }
.topic-source-picker > .el-button { justify-self: start; }
.topic-range-fields { display: flex; flex-wrap: wrap; gap: 8px; }
.topic-selected-sources { list-style: none; display: grid; gap: 4px; padding: 0; margin: 0; }
.topic-selected-sources li { display: flex; justify-content: space-between; align-items: center; gap: 8px; overflow-wrap: anywhere; }
.topic-questions h3 { margin: 0; font-size: 1rem; }
.topic-questions ul { list-style: none; display: grid; gap: 4px; padding: 0; margin: 6px 0 0; }
.topic-questions li { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
.topic-questions small { color: var(--text-muted); }
.wiki-overview { margin: 0; line-height: 1.7; white-space: pre-line; overflow-wrap: anywhere; }
.wiki-index, .wiki-index-details ul, .topic-cross-references ul { display: grid; gap: 12px; margin: 0; padding-inline-start: 24px; }
.wiki-index p, .wiki-index-details p, .topic-cross-references p { margin: 4px 0 0; color: var(--text-muted); line-height: 1.6; overflow-wrap: anywhere; }
.wiki-index-details, .topic-cross-references { border-top: 1px solid var(--border-faint); padding-top: 12px; }
.wiki-index-details summary { cursor: pointer; }
.wiki-index-details ul, .topic-cross-references ul { margin-top: 10px; }
.topic-cross-references h3 { margin: 0; font-size: 1rem; }
.topic-text-link { padding: 0; margin-inline-end: 8px; border: 0; background: none; color: var(--accent); cursor: pointer; font: inherit; text-align: start; overflow-wrap: anywhere; }
.topic-text-link:hover { text-decoration: underline; }
@media (min-width: 1101px) {
  .topic-layout { grid-template-rows: minmax(0, 1fr); align-items: stretch; }
  .topic-directory, .topic-content { min-height: 0; overflow: auto; overscroll-behavior-y: contain; }
  .topic-content { align-content: start; }
  .build-coverage { max-height: min(200px, 24dvh); }
}
@media (max-width: 768px) {
  .topic-layout { grid-template-columns: minmax(0, 1fr); }
  .topic-directory { max-height: 260px; overflow: auto; }
  .topic-toolbar { align-items: stretch; }
}
</style>
