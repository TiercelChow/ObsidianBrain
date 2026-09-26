<template>
  <KnowledgePageShell title="Wiki 工作台" subtitle="浏览数据库实体，并沿引用回到书中原文">
    <template #actions>
      <el-button v-if="activeBase?.pending_review_count" type="primary" @click="openReviews"><el-icon><Checked /></el-icon>审核 {{ activeBase.pending_review_count }}</el-button>
      <el-button v-if="activeBase" @click="openGraph"><el-icon><Connection /></el-icon>关系洞察</el-button>
      <el-button v-if="activeBase" :loading="linting" @click="runLint"><el-icon><DataAnalysis /></el-icon>知识体检</el-button>
      <el-button v-if="activeBase" @click="compileReportVisible = true"><el-icon><Tickets /></el-icon>编译报告</el-button>
      <el-button v-if="activeBase" :loading="syncing" :disabled="activeBase.lifecycle !== 'active' || !activeBase.source_available" @click="syncActive"><el-icon><Refresh /></el-icon>同步来源</el-button>
    </template>

    <div class="wiki-toolbar knowledge-toolbar">
      <el-select v-model="activeBaseId" class="knowledge-select is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择一本书" @change="onBaseChanged">
        <el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" />
      </el-select>
      <label class="knowledge-search">
        <el-icon><Search /></el-icon>
        <input v-model="query" placeholder="搜索标题、摘要与正文" @input="scheduleSearch" />
      </label>
      <button class="mobile-actions-button" type="button" :disabled="!activeBase" aria-label="Wiki 操作" @click="mobileActionsVisible = true">
        <el-icon><MoreFilled /></el-icon>
      </button>
    </div>

    <div v-if="activeBase && (activeBase.lifecycle !== 'active' || !activeBase.source_available)" class="workspace-alert knowledge-surface">
      <strong>{{ activeBase.lifecycle !== 'active' ? `知识库${activeBase.lifecycle === 'paused' ? '已暂停' : '已归档'}` : '原书目录已失效' }}</strong>
      <span>{{ activeBase.lifecycle !== 'active' ? '当前可以继续浏览历史实体，但不能同步、问答、运行任务或提交变更。' : '当前可以浏览已有知识，但同步与智能编译已停止；请从阅境轩重新添加正确目录。' }}</span>
    </div>

    <div v-if="loadingBases" class="knowledge-empty knowledge-surface">
      <el-icon class="is-loading" :size="24"><Loading /></el-icon><span>正在加载知识库…</span>
    </div>
    <div v-else-if="!bases.length" class="knowledge-empty knowledge-surface">
      <strong>还没有可浏览的知识库</strong>
      <span>先从知识库页面选择一本书并完成初始化。</span>
      <el-button type="primary" @click="$router.push('/knowledge')">前往初始化</el-button>
    </div>

    <section v-else class="wiki-workspace" :class="{ 'show-detail': Boolean(selectedEntry) }">
      <aside class="entry-pane knowledge-surface">
        <div class="entry-pane-head">
          <div>
            <strong>{{ activeBase?.book_name }}</strong>
            <span>已加载 {{ entries.length }} / {{ entryTotal }} 个结果</span>
          </div>
          <div v-if="activeBase" class="entry-statuses">
            <span class="knowledge-status" :class="`is-${activeBase.sync_state}`">
              来源 · {{ syncLabel(activeBase.sync_state) }}
            </span>
            <span class="knowledge-status" :class="`is-${activeBase.compile_state}`">
              Wiki · {{ compileLabel(activeBase) }}
            </span>
          </div>
        </div>
        <div class="entry-list">
          <button
            v-for="entry in entries"
            :key="entry.id"
            class="entry-row"
            :class="{ active: entry.id === selectedEntry?.id }"
            @click="selectEntry(entry)"
          >
            <span class="entry-type">{{ entryTypeLabel(entry.entry_type) }}</span>
            <span v-if="entry.status === 'stale'" class="knowledge-status is-warning">来源需复核</span>
            <strong>{{ entry.title }}</strong>
            <span class="entry-summary">{{ entry.summary || '此章节没有摘要' }}</span>
            <span class="entry-source">{{ entry.source_path || '数据库实体' }}</span>
          </button>
          <div v-if="!loadingEntries && !entries.length" class="entry-empty">没有匹配的实体</div>
          <div v-if="loadingEntries" class="entry-loading"><el-icon class="is-loading"><Loading /></el-icon>检索中</div>
          <button v-if="entryHasMore && !loadingEntries" class="entry-load-more" type="button" @click="loadMoreEntries">继续加载</button>
        </div>
      </aside>

      <article class="entry-detail knowledge-surface">
        <template v-if="selectedEntry">
          <button class="mobile-back" type="button" @click="selectedEntry = null">
            <el-icon><ArrowLeft /></el-icon>返回实体列表
          </button>
          <header class="entry-detail-head">
            <div>
              <span class="entry-type">{{ entryTypeLabel(selectedEntry.entry_type) }}</span>
              <h2>{{ selectedEntry.title }}</h2>
              <p>{{ selectedEntry.source_path || '数据库实体' }}</p>
            </div>
            <div class="entry-detail-actions">
              <el-button v-if="detail && detail.entry_type !== 'source_section' && activeBase?.lifecycle === 'active' && !needsSourceReview(detail)" circle title="调整实体" @click="openEntryEdit"><el-icon><Edit /></el-icon></el-button>
              <span class="knowledge-status" :class="needsSourceReview(detail || selectedEntry) ? 'is-warning' : 'is-healthy'">{{ knowledgeStatusLabel(detail?.status || selectedEntry.status) }}</span>
            </div>
          </header>
          <div v-if="detailLoading" class="detail-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <template v-else-if="detail">
            <div class="entity-meta-strip">
              <span>Revision {{ detail.revision }}</span>
              <span>{{ detail.edit_policy === 'human_protected' ? '人工保护' : '可由审核变更' }}</span>
              <span v-if="detail.confidence != null">置信度 {{ Math.round(detail.confidence * 100) }}%</span>
              <span v-for="alias in detail.aliases" :key="alias">别名 · {{ alias }}</span>
            </div>
            <section v-if="needsSourceReview(detail)" class="source-review-notice" aria-label="来源复核提示">
              <strong>这份知识的依据需要重新复核</strong>
              <p>旧正文与引用保留供回看，不代表内容已被证伪，也不再作为当前问答证据。请重新编译并审核实际依据。</p>
              <ul v-if="detail.source_impacts?.length">
                <li v-for="impact in detail.source_impacts" :key="`${impact.previous_version_id}:${impact.affected_via_entry_id || 'direct'}`">
                  <span>{{ sourceImpactLabel(impact.reason) }} · {{ impact.source_path }}</span>
                  <button v-if="impact.affected_via_entry_id" type="button" @click="selectRelatedEntry(impact.affected_via_entry_id)">依赖主题：{{ impact.affected_via_entry_title }}</button>
                </li>
              </ul>
              <small v-if="(detail.source_impact_count || 0) > (detail.source_impacts?.length || 0)">已展示前 {{ detail.source_impacts?.length || 0 }} 项，共 {{ detail.source_impact_count }} 项来源影响。</small>
              <div class="source-review-actions">
                <el-button v-if="activeBase?.lifecycle === 'active' && detail.status !== 'archived'" :loading="retryingSources" :disabled="activeBase.compile_state === 'compiling' || !activeBase.source_available || activeBase.sync_state !== 'clean' || activeBase.pending_review_count > 0" @click="retrySourceReview">重新分析当前依据</el-button>
                <el-button v-if="activeBase?.lifecycle === 'active' && detail.status !== 'archived'" @click="openHistoricalArchive">提交历史归档</el-button>
                <el-button @click="$router.push({ path: '/knowledge', query: { base: activeBaseId } })">前往知识库处理</el-button>
              </div>
              <small v-if="activeBase?.pending_review_count">已有候选待审核，请先处理审核；重复分析不会自动批准或解除来源影响。</small>
            </section>
            <div ref="markdownRef" class="entity-markdown markdown-body" v-html="renderedHtml"></div>
            <section v-if="detail.claims.length" class="entity-structure-section">
              <h3>可核验论断 <span>{{ detail.claims.length }}</span></h3>
              <div class="claim-list">
                <article v-for="claim in detail.claims" :key="claim.id" class="structure-card">
                  <header><strong>{{ claim.predicate }}</strong><span>{{ claim.verification_status }}</span></header>
                  <p>{{ claim.claim_text }}</p>
                  <footer><span v-if="claim.object_text">对象 · {{ claim.object_text }}</span><span>{{ claim.citation_count }} 条证据</span></footer>
                </article>
              </div>
            </section>
            <section v-if="detail.relations.length" class="entity-structure-section">
              <h3>知识关系 <span>{{ detail.relations.length }}</span></h3>
              <div class="relation-list">
                <button v-for="relation in detail.relations" :key="relation.id" type="button" class="structure-card relation-card" @click="selectRelatedEntry(relation.related_entry_id)">
                  <header><span>{{ relation.direction === 'outgoing' ? '指向' : '来自' }}</span><strong>{{ relation.related_entry_title }}</strong></header>
                  <p>{{ relation.relation_type }}<template v-if="relation.evidence"> · {{ relation.evidence }}</template></p>
                </button>
              </div>
            </section>
            <section class="citation-section">
              <h3>来源引用 <span>{{ detail.citations.length }}</span></h3>
              <div v-for="citation in detail.citations" :key="citation.id" class="citation-card">
                <div><el-icon><Document /></el-icon><strong>{{ citation.source_path }}</strong></div>
                <span v-if="citation.line_start">第 {{ citation.line_start }}–{{ citation.line_end }} 行</span>
                <p v-if="citation.quote_text">{{ citation.quote_text }}</p>
              </div>
            </section>
            <section v-if="detail.versions.length" class="entity-structure-section version-section">
              <h3>版本历史 <span>{{ detail.versions.length }}</span></h3>
              <ol class="version-list">
                <li v-for="version in detail.versions" :key="version.revision">
                  <b>Revision {{ version.revision }}</b>
                  <span>{{ version.title }}</span>
                  <time>{{ formatTime(version.created_at) }}</time>
                </li>
              </ol>
            </section>
          </template>
        </template>
        <div v-else class="knowledge-empty">
          <div class="detail-symbol"><el-icon><Tickets /></el-icon></div>
          <strong>选择一个实体开始阅读</strong>
          <span>正文、公式、表格与 Mermaid 将沿用阅境轩的 Markdown 渲染链路。</span>
        </div>
      </article>
    </section>

    <KnowledgeCompileReport v-model="compileReportVisible" :base-id="activeBaseId" :book-name="activeBase?.book_name" @review="openCompileReportReview" @base-updated="updateCompileBase" />
    <MotionModal v-model="mobileActionsVisible" aria-label="Wiki 操作">
      <div class="knowledge-modal-card wiki-action-sheet">
        <div class="knowledge-modal-head"><div><h3>Wiki 操作</h3><p>{{ activeBase?.book_name || '当前知识库' }}</p></div></div>
        <div class="wiki-action-list">
          <button type="button" :disabled="!activeBase?.pending_review_count" @click="runMobileAction('review')"><el-icon><Checked /></el-icon><span><strong>审核候选</strong><small>{{ activeBase?.pending_review_count || 0 }} 项待处理变更</small></span></button>
          <button type="button" :disabled="!activeBase" @click="runMobileAction('graph')"><el-icon><Connection /></el-icon><span><strong>关系洞察</strong><small>查看连接与孤立实体</small></span></button>
          <button type="button" :disabled="!activeBase || linting" @click="runMobileAction('lint')"><el-icon><DataAnalysis /></el-icon><span><strong>知识体检</strong><small>检查证据与结构</small></span></button>
          <button type="button" :disabled="!activeBase" @click="runMobileAction('report')"><el-icon><Tickets /></el-icon><span><strong>编译报告</strong><small>分析范围、主题归并与冲突</small></span></button>
          <button type="button" :disabled="syncing || activeBase?.lifecycle !== 'active' || !activeBase?.source_available" @click="runMobileAction('sync')"><el-icon><Refresh /></el-icon><span><strong>同步来源</strong><small>读取原书的最新 Markdown</small></span></button>
        </div>
        <div class="knowledge-modal-actions"><el-button @click="mobileActionsVisible = false">完成</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="reviewVisible" aria-label="审核 Wiki 变更" size="wide">
      <div class="knowledge-modal-card review-modal">
        <div class="knowledge-modal-head"><div><h3>审核语义 Wiki 变更</h3><p>{{ activeReview?.citation_audit.historical_archive ? '仅归档状态，历史正文、论断、关系和引用保留；不代表旧依据已通过当前事实校验。' : '批准后在一个事务中写入正式条目、论断、关系、版本和引用。' }}</p></div><span v-if="activeReview" class="knowledge-status" :class="`is-${activeReview.risk_level === 'high' ? 'warning' : 'draft'}`">{{ activeReview.risk_level }} risk</span></div>
        <div v-if="activeReview" class="review-content">
          <div class="review-summary"><strong>{{ activeReview.title }}</strong><span>{{ activeReview.reason }}</span></div>
          <div class="review-audit-grid">
            <section><span>变更分类</span><strong>{{ activeReview.classification_summary.new }} 新增 · {{ activeReview.classification_summary.update }} 更新 · {{ activeReview.classification_summary.disputed }} 争议</strong></section>
            <section><span>引用审计</span><strong :class="{ warning: !activeReview.citation_audit.passed }">{{ activeReview.citation_audit.historical_archive ? '历史保留 · 不作事实核验' : activeReview.citation_audit.passed ? '通过' : `${activeReview.citation_audit.issues.length} 项问题` }}</strong><small>{{ activeReview.citation_audit.historical_archive ? '不刷新或重分配任何历史证据' : `${activeReview.citation_audit.entry_citations} 条目引用 · ${activeReview.citation_audit.claim_citations} 论断引用` }}</small></section>
            <section><span>级联影响</span><strong>{{ activeReview.impact_summary.entries }} 个实体</strong><small>{{ activeReview.impact_summary.claims }} 论断 · {{ activeReview.impact_summary.relations }} 关系 · {{ activeReview.impact_summary.citations }} 历史引用</small></section>
          </div>
          <article v-for="change in activeReview.changes" :key="change.id" class="review-change">
            <header><span>{{ changeClassificationLabel(change.classification) }}</span><span>{{ changeOperationLabel(change.operation, change.expected_revision) }}</span><strong>{{ String(change.after.title || change.object_id) }}</strong><code>{{ String(change.after.entry_type || '') }}</code></header>
            <p>{{ String(change.after.summary || '') }}</p>
            <div><span>引用 {{ Array.isArray(change.after.citations) ? change.after.citations.length : 0 }}</span><span>影响 {{ change.impact.claims }} 论断 / {{ change.impact.relations }} 关系</span><span v-if="change.citation_audit.inherited_claims">{{ change.citation_audit.inherited_claims }} 条论断继承条目引用</span><span v-if="change.expected_revision">基于 Revision {{ change.expected_revision }}</span></div>
            <details v-if="change.before" class="review-diff"><summary>查看变更前后</summary><div><section><b>修改前</b><pre>{{ JSON.stringify(change.before, null, 2) }}</pre></section><section><b>修改后</b><pre>{{ JSON.stringify(change.after, null, 2) }}</pre></section></div></details>
          </article>
          <el-input v-model="reviewNote" type="textarea" :rows="2" :maxlength="2000" placeholder="可选：记录审核说明" />
        </div>
        <div v-else class="knowledge-empty"><strong>没有待审核变更</strong><span>智能编译生成的候选会显示在这里。</span></div>
        <div class="knowledge-modal-actions"><el-button @click="reviewVisible = false">稍后处理</el-button><template v-if="activeReview"><el-button :loading="resolvingReview" @click="resolveReview('reject')">驳回</el-button><el-button type="primary" :loading="resolvingReview" :disabled="!activeReview.citation_audit.passed" @click="resolveReview('approve')">批准并应用</el-button></template></div>
      </div>
    </MotionModal>

    <MotionModal v-model="archiveVisible" aria-label="提交历史归档">
      <div class="knowledge-modal-card">
        <div class="knowledge-modal-head"><div><h3>归档历史主题</h3><p>{{ archiveTarget?.title }}</p></div><span class="knowledge-status is-warning">审核后生效</span></div>
        <div class="knowledge-modal-body archive-explanation"><p>仅归档该主题，不删除正文、论断、关系和引用。归档后不再作为当前问答证据，智能编译也不会自动恢复它。</p><p>历史依据仍保留原来的来源版本和验证状态，不会因此变成当前可用事实。依赖它的其他主题仍可能需要复核。</p></div>
        <div class="knowledge-modal-actions"><el-button @click="archiveVisible = false">取消</el-button><el-button type="primary" :loading="savingArchive" @click="submitHistoricalArchive">生成归档审核</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="editVisible" aria-label="调整知识实体" size="wide">
      <div class="knowledge-modal-card entry-edit-modal">
        <div class="knowledge-modal-head"><div><h3>调整知识实体</h3><p>{{ structureModeHint }}</p></div><span class="knowledge-status is-warning">审核后生效</span></div>
        <nav class="structure-mode-switch" aria-label="实体调整方式">
          <button v-for="mode in structureModes" :key="mode.value" type="button" :class="{ active: editMode === mode.value }" @click="setEditMode(mode.value)">{{ mode.label }}</button>
        </nav>
        <template v-if="editMode === 'edit' || editMode === 'merge'">
          <label v-if="editMode === 'merge'"><span>合并来源</span><el-select v-model="mergeSourceIds" multiple filterable remote :remote-method="searchMergeOptions" :loading="mergeOptionsLoading" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="搜索并选择要并入当前实体的知识"><el-option v-for="entry in mergeOptions" :key="entry.id" :label="`${entry.title} · ${entry.entry_type}`" :value="entry.id" /></el-select></label>
          <label><span>{{ editMode === 'merge' ? '合并后标题' : '标题' }}</span><el-input v-model="editDraft.title" :maxlength="1000" /></label>
          <label><span>摘要</span><el-input v-model="editDraft.summary" type="textarea" :rows="3" :maxlength="1000" /></label>
          <label><span>别名</span><el-input v-model="editDraft.aliases" placeholder="多个别名用逗号分隔" /></label>
          <label><span>状态</span><el-select v-model="editDraft.status" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true"><el-option label="草稿" value="draft" /><el-option label="已核验" value="verified" /><el-option v-if="editMode === 'edit'" label="归档实体" value="archived" /></el-select></label>
          <label><span>Markdown 正文</span><el-input v-model="editDraft.contentMd" type="textarea" :rows="editMode === 'merge' ? 9 : 12" :maxlength="30000" /></label>
        </template>
        <template v-else>
          <div class="split-note">原实体会在批准后归档；每个新实体继承原始来源引用，但论断与关系需后续分别核验。</div>
          <div class="split-parts">
            <article v-for="(part, index) in splitParts" :key="part.key">
              <header><strong>新实体 {{ index + 1 }}</strong><button v-if="splitParts.length > 2" type="button" @click="removeSplitPart(index)">移除</button></header>
              <label><span>标题</span><el-input v-model="part.title" :maxlength="1000" /></label>
              <label><span>摘要</span><el-input v-model="part.summary" type="textarea" :rows="2" :maxlength="1000" /></label>
              <label><span>别名</span><el-input v-model="part.aliases" placeholder="多个别名用逗号分隔" /></label>
              <label><span>Markdown 正文</span><el-input v-model="part.contentMd" type="textarea" :rows="5" :maxlength="30000" /></label>
            </article>
          </div>
          <button v-if="splitParts.length < 12" class="add-split-part" type="button" @click="addSplitPart">＋ 添加一个拆分实体</button>
        </template>
        <div class="knowledge-modal-actions"><el-button @click="editVisible = false">取消</el-button><el-button type="primary" :loading="savingEdit" @click="submitStructureChange">生成审核候选</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="graphVisible" aria-label="知识关系洞察" size="wide">
      <div class="knowledge-modal-card graph-modal">
        <div class="knowledge-modal-head"><div><h3>知识关系洞察</h3><p>定位高连接枢纽、孤立实体，并查找两个实体之间的最短路径。</p></div><span v-if="graphOverview" class="knowledge-status is-healthy">{{ graphOverview.relation_count }} 条关系</span></div>
        <div v-if="graphLoading" class="knowledge-empty"><el-icon class="is-loading"><Loading /></el-icon><span>正在分析关系…</span></div>
        <template v-else-if="graphOverview">
          <KnowledgeGraphCanvas v-if="graphSnapshot" :snapshot="graphSnapshot" :selected-id="selectedEntry?.id" :path-ids="graphPath.map(entry => entry.id)" @select="openGraphEntry" />
          <section class="graph-path-builder"><strong>关系路径</strong><div><el-select v-model="graphFromId" filterable popper-class="system-select-popper" placeholder="起点"><el-option v-for="entry in graphCandidates" :key="entry.id" :label="entry.title" :value="entry.id" /></el-select><span>→</span><el-select v-model="graphToId" filterable popper-class="system-select-popper" placeholder="终点"><el-option v-for="entry in graphCandidates" :key="entry.id" :label="entry.title" :value="entry.id" /></el-select><el-button :disabled="!graphFromId || !graphToId" @click="findPath">查找</el-button></div><div v-if="graphPath.length" class="graph-path"><button v-for="entry in graphPath" :key="entry.id" type="button" @click="openGraphEntry(entry)">{{ entry.title }}</button></div><p v-else-if="pathSearched">在 5 层关系内没有找到连接路径。</p></section>
          <div class="graph-columns"><section><h4>连接枢纽</h4><button v-for="entry in graphOverview.bridge_entries" :key="entry.id" type="button" @click="openGraphEntry(entry)"><span>{{ entry.title }}</span><b>{{ entry.degree }} 连接</b></button><p v-if="!graphOverview.bridge_entries.length">尚无关系实体</p></section><section><h4>孤立实体</h4><button v-for="entry in graphOverview.orphan_entries" :key="entry.id" type="button" @click="openGraphEntry(entry)"><span>{{ entry.title }}</span><b>{{ entry.entry_type }}</b></button><p v-if="!graphOverview.orphan_entries.length">没有孤立实体</p></section></div>
        </template>
        <div class="knowledge-modal-actions"><el-button type="primary" @click="graphVisible = false">完成</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="healthVisible" aria-label="知识库体检结果">
      <div class="knowledge-modal-card health-modal">
        <div class="knowledge-modal-head"><div><h3>知识库体检</h3><p>检查数据库结构与证据完整性，不调用模型，也不会修改知识。</p></div><span v-if="healthReport" class="knowledge-status" :class="healthReport.state === 'healthy' ? 'is-healthy' : 'is-warning'">{{ healthReport.state === 'healthy' ? '健康' : '需处理' }}</span></div>
        <template v-if="healthReport">
          <div class="health-counts"><span><b>{{ healthReport.semantic_entry_count }}</b>主题</span><span><b>{{ healthReport.source_span_count }}</b>片段</span><span><b>{{ healthReport.pending_review_count }}</b>待审核</span></div>
          <div v-if="healthReport.issues.length" class="health-issues">
            <article v-for="issue in healthReport.issues" :key="issue.code + issue.title" :class="`is-${issue.severity}`">
              <strong>{{ issue.title }}</strong><p>{{ issue.detail }}</p>
              <div v-if="issue.code === 'expired-source-evidence'" class="health-entry-links">
                <button v-for="(id, index) in issue.object_ids.slice(0, 8)" :key="id" type="button" @click="openHealthEntry(id)">{{ entries.find(entry => entry.id === id)?.title || `查看主题 ${index + 1}` }}</button>
              </div>
            </article>
          </div>
          <div v-else class="knowledge-empty"><strong>没有发现结构问题</strong><span>主题知识、引用和论断证据结构完整。</span></div>
        </template>
        <div class="knowledge-modal-actions"><el-button type="primary" @click="healthVisible = false">完成</el-button></div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { ArrowLeft, Checked, Connection, DataAnalysis, Document, Edit, Loading, MoreFilled, Refresh, Search, Tickets } from '@element-plus/icons-vue'
import { useRoute, useRouter } from 'vue-router'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import KnowledgeGraphCanvas from '@/components/knowledge/KnowledgeGraphCanvas.vue'
import KnowledgeCompileReport from '@/components/knowledge/KnowledgeCompileReport.vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import { useMarkdownRender } from '@/composables/useMarkdownRender'
import { knowledgeStatusLabel, needsSourceReview, sourceImpactLabel } from '@/utils/knowledgeSourceImpacts'
import {
  findKnowledgeGraphPath,
  getKnowledgeGraphOverview,
  getKnowledgeGraphSnapshot,
  getKnowledgeEntry,
  lintBookKnowledgeBase,
  listBookKnowledgeBases,
  listKnowledgeChangeSets,
  listKnowledgeEntries,
  proposeKnowledgeEntryEdit,
  proposeKnowledgeEntryArchive,
  proposeKnowledgeEntryMerge,
  proposeKnowledgeEntrySplit,
  resolveKnowledgeChangeSet,
  syncBookKnowledgeBase,
  retryKnowledgeSourceReview,
  type KnowledgeBaseSummary,
  type KnowledgeEntryDetail,
  type KnowledgeEntrySummary,
  type KnowledgeGraphOverview,
  type KnowledgeGraphSnapshot,
  type KnowledgeChangeSet,
  type KnowledgeHealthReport,
} from '@/api/knowledge'

const route = useRoute()
const router = useRouter()
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const query = ref('')
const entries = ref<KnowledgeEntrySummary[]>([])
const entryTotal = ref(0)
const entryHasMore = ref(false)
const selectedEntry = ref<KnowledgeEntrySummary | null>(null)
const detail = ref<KnowledgeEntryDetail | null>(null)
const renderedHtml = ref('')
const markdownRef = ref<HTMLElement | null>(null)
const loadingBases = ref(false)
const loadingEntries = ref(false)
const detailLoading = ref(false)
const syncing = ref(false)
const retryingSources = ref(false)
const archiveVisible = ref(false)
const savingArchive = ref(false)
const archiveTarget = ref<{ id: string; title: string; revision: number; baseId: string } | null>(null)
const pendingReviews = ref<KnowledgeChangeSet[]>([])
const activeReview = ref<KnowledgeChangeSet | null>(null)
const reviewVisible = ref(false)
const compileReportVisible = ref(false)
const mobileActionsVisible = ref(false)
const reviewNote = ref('')
const resolvingReview = ref(false)
const linting = ref(false)
const healthVisible = ref(false)
const healthReport = ref<KnowledgeHealthReport | null>(null)
const editVisible = ref(false)
const savingEdit = ref(false)
const editDraft = reactive({ title: '', summary: '', aliases: '', status: 'draft' as 'draft' | 'verified' | 'archived', contentMd: '' })
type EditMode = 'edit' | 'merge' | 'split'
interface SplitPartDraft { key: number; title: string; summary: string; aliases: string; contentMd: string }
const editMode = ref<EditMode>('edit')
const structureModes: Array<{ value: EditMode; label: string }> = [
  { value: 'edit', label: '编辑' },
  { value: 'merge', label: '合并' },
  { value: 'split', label: '拆分' },
]
const mergeSourceIds = ref<string[]>([])
const mergeOptions = ref<KnowledgeEntrySummary[]>([])
const mergeOptionsLoading = ref(false)
const splitParts = ref<SplitPartDraft[]>([])
let splitPartKey = 0
const graphVisible = ref(false)
const graphLoading = ref(false)
const graphOverview = ref<KnowledgeGraphOverview | null>(null)
const graphSnapshot = ref<KnowledgeGraphSnapshot | null>(null)
const graphFromId = ref('')
const graphToId = ref('')
const graphPath = ref<KnowledgeEntrySummary[]>([])
const pathSearched = ref(false)
let searchTimer = 0
let graphReleaseTimer = 0

const activeBase = computed(() => bases.value.find(base => base.id === activeBaseId.value))
const graphCandidates = computed(() => graphSnapshot.value?.entries || entries.value.filter(entry => entry.entry_type !== 'source_section'))
const structureModeHint = computed(() => ({
  edit: '修改标题、别名、状态和正文；保存后先进入审核。',
  merge: '把重复知识并入当前实体，批准后归档来源并迁移关系。',
  split: '将复合知识拆成多个独立实体，同时保留原始引用。',
})[editMode.value])
const { renderMarkdown, enhance, cleanup } = useMarkdownRender(() => {})

function compileLabel(base: KnowledgeBaseSummary) {
  if (base.compile_mode === 'chapter' && base.compile_state === 'not_started') return '章节索引'
  if (base.compile_phase === 'waiting_review') return '待审核'
  if (base.compile_phase === 'cancelling') return '停止中'
  const labels: Record<string, string> = {
    not_started: '待编译', outdated: '待更新', compiling: '编译中', ready: '智能 Wiki', failed: '编译失败',
  }
  return labels[base.compile_state] || base.compile_state
}

async function loadBases() {
  loadingBases.value = true
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '加载失败')
    bases.value = response.result.items.flatMap(card => (
      card.book.kind === 'folder' && card.knowledge_base ? [card.knowledge_base] : []
    ))
    const requested = String(route.query.base || '')
    activeBaseId.value = bases.value.some(base => base.id === requested) ? requested : (bases.value[0]?.id || '')
    if (activeBaseId.value) {
      await Promise.all([loadEntries(String(route.query.entry || '')), loadReviews()])
      const requestedReview = String(route.query.review || '')
      if (requestedReview) {
        activeReview.value = pendingReviews.value.find(item => item.id === requestedReview) || null
        reviewVisible.value = Boolean(activeReview.value)
      }
    }
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loadingBases.value = false
  }
}

async function loadEntries(preselectId = '', append = false) {
  if (!activeBaseId.value) return
  loadingEntries.value = true
  try {
    const offset = append ? entries.value.length : 0
    const response = await listKnowledgeEntries(activeBaseId.value, { query: query.value, offset, limit: 60, includeStale: true })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '实体加载失败')
    entries.value = append ? [...entries.value, ...response.result.entries] : response.result.entries
    entryTotal.value = response.result.total
    entryHasMore.value = response.result.has_more
    const preselect = entries.value.find(entry => entry.id === preselectId)
    if (preselect) await selectEntry(preselect)
    else if (selectedEntry.value && !entries.value.some(entry => entry.id === selectedEntry.value?.id)) selectedEntry.value = null
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loadingEntries.value = false
  }
}

function loadMoreEntries() {
  void loadEntries('', true)
}

function scheduleSearch() {
  window.clearTimeout(searchTimer)
  searchTimer = window.setTimeout(() => loadEntries(), 220)
}

async function selectEntry(entry: KnowledgeEntrySummary) {
  selectedEntry.value = entry
  detail.value = null
  detailLoading.value = true
  router.replace({ query: { ...route.query, base: activeBaseId.value, entry: entry.id } })
  try {
    const response = await getKnowledgeEntry(entry.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '实体读取失败')
    detail.value = response.result
    renderedHtml.value = await renderMarkdown(response.result.content_md, undefined, entry.id)
    await nextTick()
    if (markdownRef.value) await enhance(markdownRef.value)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    detailLoading.value = false
  }
}

async function selectRelatedEntry(entryId: string) {
  const known = entries.value.find(entry => entry.id === entryId)
  if (known) return selectEntry(known)
  detailLoading.value = true
  try {
    const response = await getKnowledgeEntry(entryId)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '关联实体读取失败')
    const entry: KnowledgeEntrySummary = {
      id: response.result.id,
      knowledge_base_id: response.result.knowledge_base_id,
      entry_type: response.result.entry_type,
      slug: response.result.slug,
      title: response.result.title,
      summary: response.result.summary,
      status: response.result.status,
      confidence: response.result.confidence,
      source_path: response.result.source_path,
      updated_at: response.result.updated_at,
    }
    await selectEntry(entry)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    detailLoading.value = false
  }
}

async function onBaseChanged() {
  selectedEntry.value = null
  detail.value = null
  router.replace({ query: { base: activeBaseId.value } })
  await Promise.all([loadEntries(), loadReviews()])
}

async function loadReviews() {
  if (!activeBaseId.value) return
  const response = await listKnowledgeChangeSets(activeBaseId.value, 'proposed')
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '审核列表加载失败')
  pendingReviews.value = response.result.change_sets
}

async function retrySourceReview() {
  const baseId = activeBaseId.value
  if (!baseId || retryingSources.value) return
  retryingSources.value = true
  try {
    const response = await retryKnowledgeSourceReview(baseId)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '来源复核启动失败')
    if (activeBaseId.value !== baseId) return
    const index = bases.value.findIndex(base => base.id === baseId)
    if (index >= 0) bases.value[index] = response.result.knowledge_base
    ElMessage.success(response.result.message)
    compileReportVisible.value = true
  } catch (error) { ElMessage.error((error as Error).message) }
  finally { retryingSources.value = false }
}

function updateCompileBase(base: KnowledgeBaseSummary) {
  const index = bases.value.findIndex(item => item.id === base.id)
  if (index >= 0) bases.value[index] = base
}

function openHistoricalArchive() {
  if (!detail.value || detail.value.status === 'archived') return
  archiveTarget.value = { id: detail.value.id, title: detail.value.title, revision: detail.value.revision, baseId: activeBaseId.value }
  archiveVisible.value = true
}

async function submitHistoricalArchive() {
  const target = archiveTarget.value
  if (!target || savingArchive.value) return
  savingArchive.value = true
  try {
    const response = await proposeKnowledgeEntryArchive(target.id, target.revision)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '归档审核生成失败')
    const shouldOpenReview = archiveVisible.value && archiveTarget.value === target
    archiveVisible.value = false
    if (activeBaseId.value !== target.baseId) { ElMessage.success('归档候选已保存，可在对应知识库审核'); return }
    await loadReviews()
    const base = bases.value.find(item => item.id === target.baseId)
    if (base) base.pending_review_count = pendingReviews.value.length
    if (!shouldOpenReview || activeBaseId.value !== target.baseId) { ElMessage.success('归档候选已保存，请在审核入口处理'); return }
    activeReview.value = response.result
    reviewNote.value = ''
    reviewVisible.value = true
  } catch (error) { ElMessage.error((error as Error).message) }
  finally { savingArchive.value = false }
}

function openReviews() {
  activeReview.value = pendingReviews.value[0] || null
  reviewNote.value = ''
  reviewVisible.value = true
}

async function openCompileReportReview(changeSetId: string) {
  try {
    await loadReviews()
    activeReview.value = pendingReviews.value.find(item => item.id === changeSetId) || null
    if (!activeReview.value) { ElMessage.info('该变更已处理，请刷新报告查看结果'); return }
    reviewNote.value = ''
    reviewVisible.value = true
  } catch (error) { ElMessage.error((error as Error).message) }
}

function runMobileAction(action: 'review' | 'graph' | 'lint' | 'sync' | 'report') {
  mobileActionsVisible.value = false
  if (action === 'review') openReviews()
  else if (action === 'graph') void openGraph()
  else if (action === 'lint') void runLint()
  else if (action === 'report') compileReportVisible.value = true
  else void syncActive()
}

async function resolveReview(decision: 'approve' | 'reject') {
  if (!activeReview.value) return
  resolvingReview.value = true
  try {
    const response = await resolveKnowledgeChangeSet(activeReview.value.id, decision, reviewNote.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '审核处理失败')
    ElMessage.success(decision === 'approve' ? '知识变更已原子应用' : '知识变更已驳回')
    reviewVisible.value = false
    await loadBases()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    resolvingReview.value = false
  }
}

async function runLint() {
  if (!activeBaseId.value) return
  linting.value = true
  try {
    const response = await lintBookKnowledgeBase(activeBaseId.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识体检失败')
    healthReport.value = response.result
    healthVisible.value = true
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    linting.value = false
  }
}

async function openHealthEntry(entryId: string) {
  healthVisible.value = false
  await selectRelatedEntry(entryId)
}

async function syncActive() {
  if (!activeBaseId.value) return
  syncing.value = true
  try {
    const response = await syncBookKnowledgeBase(activeBaseId.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '同步失败')
    ElMessage.success(response.result.message)
    await loadBases()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    syncing.value = false
  }
}

function openEntryEdit() {
  if (!detail.value) return
  editMode.value = 'edit'
  editDraft.title = detail.value.title
  editDraft.summary = detail.value.summary
  editDraft.aliases = detail.value.aliases.join('，')
  editDraft.status = detail.value.status === 'verified' || detail.value.status === 'archived' ? detail.value.status : 'draft'
  editDraft.contentMd = detail.value.content_md
  mergeSourceIds.value = []
  mergeOptions.value = []
  resetSplitParts()
  editVisible.value = true
}

function setEditMode(mode: EditMode) {
  editMode.value = mode
  if (mode === 'merge') {
    if (editDraft.status === 'archived') editDraft.status = 'draft'
    void searchMergeOptions('')
  }
}

async function searchMergeOptions(search: string) {
  if (!activeBaseId.value || !detail.value) return
  mergeOptionsLoading.value = true
  try {
    const response = await listKnowledgeEntries(activeBaseId.value, { query: search.trim(), offset: 0, limit: 100 })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '合并候选加载失败')
    const selected = mergeOptions.value.filter(entry => mergeSourceIds.value.includes(entry.id))
    const options = [...selected, ...response.result.entries]
      .filter(entry => entry.entry_type !== 'source_section' && entry.id !== detail.value?.id)
    mergeOptions.value = [...new Map(options.map(entry => [entry.id, entry])).values()]
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    mergeOptionsLoading.value = false
  }
}

function createSplitPart(index: number): SplitPartDraft {
  splitPartKey += 1
  return { key: splitPartKey, title: `${detail.value?.title || '新实体'} · ${index + 1}`, summary: '', aliases: '', contentMd: '' }
}

function resetSplitParts() {
  splitParts.value = [createSplitPart(0), createSplitPart(1)]
}

function addSplitPart() {
  splitParts.value.push(createSplitPart(splitParts.value.length))
}

function removeSplitPart(index: number) {
  if (splitParts.value.length > 2) splitParts.value.splice(index, 1)
}

async function submitEntryEdit() {
  if (!detail.value) return
  savingEdit.value = true
  try {
    let response
    if (editMode.value === 'edit') {
      response = await proposeKnowledgeEntryEdit({
        entryId: detail.value.id,
        title: editDraft.title,
        summary: editDraft.summary,
        contentMd: editDraft.contentMd,
        aliases: splitAliases(editDraft.aliases),
        status: editDraft.status,
        expectedRevision: detail.value.revision,
      })
    } else if (editMode.value === 'merge') {
      if (!mergeSourceIds.value.length) throw new Error('至少选择一个要合并的来源实体')
      const sourceDetails = await Promise.all(mergeSourceIds.value.map(async (entryId) => {
        const source = await getKnowledgeEntry(entryId)
        if (source.status !== 'success' || !source.result) throw new Error(source.error?.message || '合并来源读取失败')
        return { entryId, expectedRevision: source.result.revision }
      }))
      response = await proposeKnowledgeEntryMerge({
        targetEntryId: detail.value.id,
        title: editDraft.title,
        summary: editDraft.summary,
        contentMd: editDraft.contentMd,
        aliases: splitAliases(editDraft.aliases),
        status: editDraft.status === 'verified' ? 'verified' : 'draft',
        expectedRevision: detail.value.revision,
        sources: sourceDetails,
      })
    } else {
      if (splitParts.value.some(part => !part.title.trim() || !part.summary.trim() || !part.contentMd.trim())) {
        throw new Error('请完整填写每个拆分实体的标题、摘要和正文')
      }
      response = await proposeKnowledgeEntrySplit({
        entryId: detail.value.id,
        expectedRevision: detail.value.revision,
        parts: splitParts.value.map(part => ({
          title: part.title.trim(),
          summary: part.summary.trim(),
          contentMd: part.contentMd.trim(),
          aliases: splitAliases(part.aliases),
        })),
      })
    }
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '实体变更提交失败')
    editVisible.value = false
    await loadReviews()
    activeReview.value = pendingReviews.value.find(review => review.id === response.result?.id) || response.result
    reviewNote.value = ''
    reviewVisible.value = true
    ElMessage.success('已生成待审核变更，当前实体尚未被覆盖')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingEdit.value = false
  }
}

function submitStructureChange() {
  void submitEntryEdit()
}

function splitAliases(value: string) {
  return value.split(/[，,]/).map(alias => alias.trim()).filter(Boolean)
}

async function openGraph() {
  if (!activeBaseId.value) return
  graphVisible.value = true
  graphLoading.value = true
  graphPath.value = []
  pathSearched.value = false
  try {
    const desktopSnapshotRequest = window.matchMedia('(max-width: 768px)').matches
      ? Promise.resolve(null)
      : getKnowledgeGraphSnapshot(activeBaseId.value, 120)
    const [overviewResponse, snapshotResponse] = await Promise.all([
      getKnowledgeGraphOverview(activeBaseId.value, 30),
      desktopSnapshotRequest,
    ])
    if (overviewResponse.status !== 'success' || !overviewResponse.result) throw new Error(overviewResponse.error?.message || '关系洞察加载失败')
    if (snapshotResponse && (snapshotResponse.status !== 'success' || !snapshotResponse.result)) throw new Error(snapshotResponse.error?.message || '关系画布加载失败')
    graphOverview.value = overviewResponse.result
    graphSnapshot.value = snapshotResponse?.result || null
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    graphLoading.value = false
  }
}

async function findPath() {
  if (!graphFromId.value || !graphToId.value) return
  const response = await findKnowledgeGraphPath(activeBaseId.value, graphFromId.value, graphToId.value, 5)
  if (response.status !== 'success' || !response.result) {
    ElMessage.error(response.error?.message || '关系路径查找失败')
    return
  }
  graphPath.value = response.result.entries
  pathSearched.value = true
}

async function openGraphEntry(entry: KnowledgeEntrySummary) {
  graphVisible.value = false
  await selectRelatedEntry(entry.id)
}

function entryTypeLabel(type: string) {
  return type === 'source_section' ? '来源章节' : type
}

function changeOperationLabel(operation: string, expectedRevision?: number | null) {
  if (operation === 'merge') return '合并'
  if (operation === 'split') return expectedRevision ? '拆分原实体' : '拆分新增'
  if (operation === 'archive') return '归档'
  if (operation === 'restore') return '恢复'
  return operation === 'create' ? '新增' : '更新'
}

function changeClassificationLabel(classification: 'new' | 'update' | 'disputed') {
  return ({ new: '新增知识', update: '补充既有', disputed: '存在争议' })[classification]
}

function syncLabel(status: string) {
  return ({ clean: '已同步', outdated: '待同步', failed: '同步失败', scanning: '扫描中' } as Record<string, string>)[status] || status
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  }).format(new Date(value))
}

onMounted(loadBases)
watch(graphVisible, (visible) => {
  window.clearTimeout(graphReleaseTimer)
  if (!visible) {
    graphReleaseTimer = window.setTimeout(() => {
      graphOverview.value = null
      graphSnapshot.value = null
      graphPath.value = []
    }, 260)
  }
})
onBeforeUnmount(() => {
  window.clearTimeout(searchTimer)
  window.clearTimeout(graphReleaseTimer)
  graphSnapshot.value = null
  cleanup()
})
</script>

<style scoped>
.wiki-toolbar { margin-bottom: 12px; }
.workspace-alert { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; padding: 11px 14px; border-color: color-mix(in srgb, #ff9f0a 24%, var(--border-faint)); background: color-mix(in srgb, #ff9f0a 7%, var(--bg-glass)); }
.workspace-alert strong { flex: 0 0 auto; color: #b56600; font-size: 12px; }
.workspace-alert span { color: var(--text-muted); font-size: 11px; }
.wiki-toolbar .knowledge-search { flex: 1; }
.wiki-workspace { min-height: 570px; display: grid; grid-template-columns: minmax(250px, 320px) minmax(0, 1fr); gap: 12px; }
.entry-pane, .entry-detail { min-height: 0; overflow: hidden; }
.entry-pane { display: flex; flex-direction: column; }
.entry-pane-head { min-height: 68px; display: grid; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--border-faint); }
.entry-pane-head > div { min-width: 0; display: grid; gap: 2px; }
.entry-pane-head > .entry-statuses { display: flex; flex-wrap: wrap; gap: 5px; }
.entry-pane-head strong { overflow: hidden; font-size: 14px; text-overflow: ellipsis; white-space: nowrap; }
.entry-pane-head span:not(.knowledge-status) { color: var(--text-faint); font-size: 11px; }
.entry-list { flex: 1; max-height: calc(100vh - 260px); overflow: auto; padding: 7px; }
.entry-row { width: 100%; display: grid; gap: 4px; margin: 0 0 5px; padding: 12px; border: 0; border-radius: 13px; background: transparent; color: var(--text-primary); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.entry-row:hover { background: var(--bg-hover); }
.entry-row.active { background: var(--accent-light); box-shadow: inset 0 0 0 1px var(--accent-border); }
.entry-row > .knowledge-status { justify-self: start; }
.entry-type { width: fit-content; color: var(--accent); font-size: 10px; font-weight: 700; letter-spacing: .04em; }
.entry-row strong { overflow: hidden; font-size: 14px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
.entry-summary { display: -webkit-box; overflow: hidden; color: var(--text-muted); font-size: 11px; line-height: 1.45; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.entry-source { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.entry-empty, .entry-loading { display: flex; justify-content: center; gap: 7px; padding: 32px 10px; color: var(--text-faint); font-size: 12px; }
.entry-load-more { width: calc(100% - 10px); min-height: 38px; margin: 5px; border: 0; border-radius: 11px; background: var(--bg-glass-subtle); color: var(--accent); cursor: pointer; }
.entry-detail { max-height: calc(100vh - 208px); overflow: auto; padding: 22px clamp(18px, 2vw, 32px); }
.entry-detail-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 24px; padding-bottom: 20px; border-bottom: 1px solid var(--border-faint); }
.entry-detail-head > div:first-child { min-width: 0; }
.entry-detail-head h2 { margin: 6px 0 4px; overflow-wrap: anywhere; font-size: clamp(20px, 2vw, 26px); font-weight: 690; }
.entry-detail-head p { overflow-wrap: anywhere; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; }
.entry-detail-actions { display: flex; align-items: center; gap: 9px; flex: none; }
.detail-loading { min-height: 300px; display: grid; place-content: center; color: var(--accent); font-size: 24px; }
.detail-symbol { width: 62px; height: 62px; display: grid; place-items: center; border-radius: 20px; background: var(--accent-light); color: var(--accent); font-size: 27px; }
.entity-meta-strip { display: flex; flex-wrap: wrap; gap: 6px; margin: -8px 0 18px; }
.entity-meta-strip span { padding: 5px 9px; border: 1px solid var(--border-faint); border-radius: 999px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 10px; }
.source-review-notice { display: grid; gap: 10px; margin-bottom: 20px; padding: 16px; border: 1px solid var(--accent-border); border-radius: 16px; background: var(--accent-light); overflow-wrap: anywhere; }
.source-review-notice strong { font-size: 14px; font-weight: 650; }
.source-review-notice p, .source-review-notice li, .source-review-notice small { margin: 0; color: var(--text-muted); font-size: 12px; line-height: 1.65; }
.source-review-notice ul { display: grid; gap: 8px; margin: 0; padding-left: 18px; }
.source-review-notice li > span { display: block; }
.source-review-notice > .el-button { justify-self: start; max-width: 100%; }
.source-review-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.source-review-actions :deep(.el-button) { margin-left: 0; }
.archive-explanation { color: var(--text-muted); line-height: 1.7; overflow-wrap: anywhere; }
.source-review-notice li button, .health-entry-links button { min-height: 36px; padding: 6px 8px; border: 0; border-radius: 9px; background: var(--bg-glass-subtle); color: var(--accent); text-align: left; overflow-wrap: anywhere; cursor: pointer; }
.source-review-notice li button:active, .health-entry-links button:active { background: var(--accent-light); }
.health-entry-links { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
.entity-markdown { color: var(--text-secondary); font-size: 15px; line-height: 1.8; overflow-wrap: anywhere; }
@media (min-width: 1101px) {
  .wiki-workspace { min-height: 0; overflow: hidden; }
  .entry-list { min-height: 0; max-height: none; }
  .entry-detail { max-height: none; }
}
.review-modal { max-height: min(780px, calc(100dvh - 48px)); }
.review-content { min-height: 0; display: grid; align-content: start; gap: 10px; flex: 1; overflow: auto; padding: 0 24px 12px; overscroll-behavior: contain; }
.review-summary { display: grid; gap: 4px; padding: 12px; border-radius: 13px; background: var(--accent-light); }
.review-summary strong { overflow-wrap: anywhere; font-size: 14px; }
.review-summary span { overflow-wrap: anywhere; color: var(--text-muted); font-size: 11px; }
.review-audit-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.review-audit-grid section { min-width: 0; display: grid; gap: 3px; padding: 11px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); }
.review-audit-grid span, .review-audit-grid small { color: var(--text-faint); font-size: 9px; }
.review-audit-grid strong { color: var(--text-primary); font-size: 11px; }
.review-audit-grid strong.warning { color: var(--danger, #dc2626); }
.review-change { display: grid; gap: 8px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.review-change header { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
.review-change header span { padding: 3px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent); font-size: 9px; }
.review-change header strong { min-width: min(100%, 160px); flex: 1; overflow-wrap: anywhere; font-size: 13px; }
.review-change header code { color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; }
.review-change p { overflow-wrap: anywhere; color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.review-change > div { display: flex; flex-wrap: wrap; gap: 5px 12px; color: var(--text-faint); font-size: 9px; }
.review-diff { color: var(--text-muted); font-size: 10px; }
.review-diff summary { width: fit-content; color: var(--accent); cursor: pointer; }
.review-diff > div { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-top: 8px; }
.review-diff section { min-width: 0; }
.review-diff pre { max-height: 210px; overflow: auto; margin-top: 5px; padding: 10px; border-radius: 10px; background: var(--code-block-bg); color: var(--code-block-text); font-size: 9px; white-space: pre-wrap; overflow-wrap: anywhere; }
.entry-edit-modal { display: grid; gap: 12px; max-height: min(820px, calc(100dvh - 40px)); overflow: auto; }
.entry-edit-modal > label { display: grid; gap: 6px; color: var(--text-muted); font-size: 11px; }
.structure-mode-switch { display: grid; grid-template-columns: repeat(3, 1fr); gap: 3px; padding: 3px; border: 1px solid var(--border-faint); border-radius: 13px; background: var(--bg-glass-subtle); }
.structure-mode-switch button { min-height: 34px; border: 0; border-radius: 10px; background: transparent; color: var(--text-muted); cursor: pointer; transition: var(--transition-interactive); }
.structure-mode-switch button.active { background: var(--bg-surface); color: var(--accent); box-shadow: var(--shadow-xs); font-weight: 650; }
.split-note { padding: 10px 12px; border-radius: 12px; background: color-mix(in srgb, #ff9f0a 8%, var(--bg-glass-subtle)); color: var(--text-muted); font-size: 10px; line-height: 1.55; }
.split-parts { display: grid; gap: 9px; }
.split-parts article { display: grid; gap: 9px; padding: 12px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.split-parts article > header { display: flex; align-items: center; justify-content: space-between; }
.split-parts article > header strong { font-size: 12px; }
.split-parts article > header button { border: 0; background: transparent; color: #d84a42; cursor: pointer; font-size: 10px; }
.split-parts label { display: grid; gap: 5px; color: var(--text-muted); font-size: 10px; }
.add-split-part { min-height: 38px; border: 1px dashed var(--accent-border); border-radius: 12px; background: var(--accent-light); color: var(--accent); cursor: pointer; }
.graph-modal { display: grid; gap: 14px; max-height: min(900px, calc(100dvh - 40px)); overflow: auto; }
.health-modal { overflow-y: auto; }
.graph-path-builder { display: grid; gap: 9px; padding: 13px; border-radius: 14px; background: var(--bg-glass-subtle); }
.graph-path-builder > strong { font-size: 12px; }
.graph-path-builder > div:first-of-type { display: grid; grid-template-columns: 1fr auto 1fr auto; align-items: center; gap: 7px; }
.graph-path-builder p { color: var(--text-faint); font-size: 10px; }
.graph-path { display: flex !important; flex-wrap: wrap; align-items: center; gap: 7px; }
.graph-path button, .graph-columns button { border: 1px solid var(--border-faint); border-radius: 10px; background: var(--bg-glass); color: var(--text-primary); cursor: pointer; }
.graph-path button { padding: 7px 10px; }
.graph-path button + button::before { content: '→'; margin-right: 8px; color: var(--text-faint); }
.graph-columns { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.graph-columns section { display: grid; align-content: start; gap: 6px; min-height: 150px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; }
.graph-columns h4 { margin-bottom: 4px; font-size: 12px; }
.graph-columns button { display: flex; justify-content: space-between; gap: 8px; padding: 9px 10px; text-align: left; }
.graph-columns button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.graph-columns button b { flex: 0 0 auto; color: var(--accent); font-size: 9px; }
.graph-columns p { color: var(--text-faint); font-size: 10px; }
.relation-card { width: 100%; color: var(--text-primary); text-align: left; cursor: pointer; }
.health-counts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.health-counts span { display: grid; gap: 3px; padding: 12px; border-radius: 13px; background: var(--bg-glass-subtle); color: var(--text-faint); font-size: 10px; }
.health-counts b { color: var(--text-primary); font-size: 20px; }
.health-issues { display: grid; gap: 8px; max-height: 48dvh; overflow: auto; }
.health-issues article { padding: 12px 14px; border: 1px solid color-mix(in srgb, #ff9f0a 25%, transparent); border-radius: 13px; background: color-mix(in srgb, #ff9f0a 7%, transparent); }
.health-issues article.is-error { border-color: color-mix(in srgb, #ff3b30 25%, transparent); background: color-mix(in srgb, #ff3b30 7%, transparent); }
.health-issues strong { font-size: 12px; }
.health-issues p { margin-top: 4px; color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.entity-markdown :deep(h1), .entity-markdown :deep(h2), .entity-markdown :deep(h3) { margin: 1.4em 0 .6em; color: var(--text-primary); }
.entity-markdown :deep(p) { margin: .8em 0; }
.entity-markdown :deep(pre) { overflow: auto; margin: 1em 0; padding: 15px; border-radius: 13px; background: var(--code-block-bg); color: var(--code-block-text); }
.entity-markdown :deep(code) { font-family: var(--font-mono); }
.entity-markdown :deep(.table-scroll), .entity-markdown :deep(.katex-display), .entity-markdown :deep(.mermaid) { max-width: 100%; overflow-x: auto; }
.entity-markdown :deep(table) { min-width: max-content; border-collapse: collapse; }
.entity-markdown :deep(th), .entity-markdown :deep(td) { padding: 8px 11px; border: 1px solid var(--border-faint); }
.citation-section { margin-top: 32px; padding-top: 22px; border-top: 1px solid var(--border-faint); }
.citation-section h3 { margin: 0 0 12px; font-size: 15px; }
.citation-section h3 span { color: var(--text-faint); font-size: 11px; }
.citation-card { display: grid; gap: 5px; margin-top: 8px; padding: 12px 14px; border-radius: 13px; background: var(--bg-glass-subtle); }
.citation-card > div { min-width: 0; display: flex; align-items: center; gap: 7px; font-size: 12px; }
.citation-card strong { min-width: 0; overflow-wrap: anywhere; }
.citation-card > span { color: var(--text-faint); font-size: 10px; }
.citation-card p { color: var(--text-muted); font-size: 11px; }
.entity-structure-section { margin-top: 30px; padding-top: 20px; border-top: 1px solid var(--border-faint); }
.entity-structure-section h3 { margin: 0 0 12px; color: var(--text-primary); font-size: 15px; }
.entity-structure-section h3 span { color: var(--text-faint); font-size: 11px; }
.claim-list, .relation-list { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr)); gap: 8px; }
.structure-card { display: grid; gap: 7px; padding: 12px 14px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.structure-card header, .structure-card footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.structure-card header strong { color: var(--text-primary); font-size: 12px; }
.structure-card header span, .structure-card footer { color: var(--text-faint); font-size: 9px; }
.structure-card p { color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.relation-card header { justify-content: flex-start; }
.relation-card header > span { color: var(--accent); }
.version-list { display: grid; gap: 0; padding: 0; list-style: none; }
.version-list li { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 9px 2px; border-bottom: 1px solid var(--border-faint); font-size: 10px; }
.version-list b { color: var(--accent); font-size: 10px; }
.version-list span { overflow: hidden; color: var(--text-secondary); text-overflow: ellipsis; white-space: nowrap; }
.version-list time { color: var(--text-faint); }
.mobile-back { display: none; }
.mobile-actions-button { display: none; }
.mobile-actions-button:disabled { opacity: .45; }
.wiki-action-list { min-height: 0; display: grid; gap: 6px; overflow-y: auto; padding: 0 16px 10px; }
.wiki-action-list button { min-width: 0; min-height: 58px; display: flex; align-items: center; gap: 12px; padding: 10px 12px; border: 1px solid var(--border-faint); border-radius: 13px; background: var(--bg-glass-subtle); color: var(--text-primary); text-align: left; cursor: pointer; }
.wiki-action-list button:active { background: var(--accent-light); }
.wiki-action-list button:disabled { opacity: .45; cursor: default; }
.wiki-action-list button > .el-icon { flex: none; color: var(--accent); font-size: 19px; }
.wiki-action-list button > span { min-width: 0; display: grid; gap: 2px; }
.wiki-action-list strong { font-size: 13px; }
.wiki-action-list small { color: var(--text-faint); font-size: 11px; }
@media (max-width: 1100px) {
  .wiki-workspace { min-height: 0; display: block; }
  .entry-pane { min-height: 500px; }
  .entry-detail { display: none; max-height: none; }
  .wiki-workspace.show-detail .entry-pane { display: none; }
  .wiki-workspace.show-detail .entry-detail { display: block; }
  .mobile-back { display: flex; align-items: center; gap: 5px; margin: -4px 0 14px; padding: 8px 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-size: 13px; cursor: pointer; }
}
@media (max-width: 768px) {
  .workspace-alert { align-items: flex-start; display: grid; }
  .review-diff > div, .graph-columns, .review-audit-grid { grid-template-columns: 1fr; }
  .graph-path-builder > div:first-of-type { grid-template-columns: 1fr auto; }
  .graph-path-builder > div:first-of-type > span { display: none; }
  .wiki-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) 46px; align-items: stretch; }
  .wiki-toolbar .knowledge-search { grid-column: 1 / -1; grid-row: 2; }
  .mobile-actions-button { grid-column: 2; grid-row: 1; width: 46px; min-height: 46px; display: grid; place-items: center; border: 1px solid var(--border-subtle); border-radius: 14px; background: var(--bg-glass); color: var(--accent); font-size: 19px; }
  .wiki-workspace { min-height: calc(100dvh - 230px); }
  .entry-pane, .entry-detail { min-height: calc(100dvh - 230px); }
  .entry-detail { padding: 16px; }
  .wiki-workspace.show-detail .entry-detail { animation: mobile-detail-in var(--motion-normal) var(--ease-spring-gentle) both; }
  .entry-list { max-height: none; }
  .entry-detail-head { display: block; }
  .entry-detail-head .knowledge-status { margin-top: 10px; }
  .review-content { padding: 0 16px 12px; }
  .review-modal { max-height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); }
  .entry-edit-modal, .graph-modal { max-height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); }
}
@keyframes mobile-detail-in { from { opacity: 0; transform: translateX(18px); } to { opacity: 1; transform: none; } }
</style>
