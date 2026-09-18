<template>
  <KnowledgePageShell title="Wiki 配置" subtitle="集中管理 Agent Runtime、每本书的规则文档与能力边界">
    <div class="settings-layout">
      <aside class="settings-nav knowledge-surface">
        <button :class="{ active: section === 'runtime' }" @click="section = 'runtime'"><el-icon><Cpu /></el-icon><span><strong>Agent Runtime</strong><small>执行器与模型</small></span></button>
        <button :class="{ active: section === 'usage' }" @click="section = 'usage'"><el-icon><DataAnalysis /></el-icon><span><strong>Token 用量</strong><small>调用趋势与来源</small></span></button>
        <button :class="{ active: section === 'protection' }" @click="section = 'protection'"><el-icon><Lock /></el-icon><span><strong>数据保护</strong><small>备份、下载与恢复</small></span></button>
        <button :class="{ active: section === 'documents' }" @click="section = 'documents'"><el-icon><Document /></el-icon><span><strong>配置文档</strong><small>数据库中的 Markdown</small></span></button>
        <button :class="{ active: section === 'skills' }" @click="section = 'skills'"><el-icon><MagicStick /></el-icon><span><strong>Skills</strong><small>版本化能力</small></span></button>
      </aside>

      <section class="settings-main knowledge-surface">
        <template v-if="section === 'runtime'">
          <header class="settings-section-head"><div><span>执行环境</span><h2>Agent Runtime</h2><p>DeepSeek Harness 负责执行，模型可连接任意受支持的供应商；业务数据仍由 Rust 与 SQLite 管理。</p></div></header>
          <div v-if="loading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <article v-for="item in runtimeHealth" v-else :key="item.profile.id" class="runtime-card">
            <div class="runtime-title">
              <div class="runtime-logo">DS</div>
              <div><h3>{{ item.profile.name }}</h3><p>{{ item.profile.runtime === 'deepseek_harness' ? 'ACP stdio sidecar' : item.profile.runtime }}</p></div>
              <span class="knowledge-status" :class="runtimeVerification[item.profile.id] ? 'is-healthy' : (item.available ? '' : 'is-warning')">{{ runtimeVerification[item.profile.id] ? '连接已验证' : (item.available ? '启动器可用' : '不可用') }}</span>
            </div>
            <div class="runtime-message" :class="{ 'is-verified': runtimeVerification[item.profile.id] }">{{ runtimeVerification[item.profile.id] || item.message }}<span v-if="item.version"> · {{ item.version }}</span></div>
            <label><span>ACP 启动命令</span><el-input v-model="item.profile.executable" placeholder="npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp" /></label>
            <div class="provider-mode">
              <div><strong>第三方模型供应商</strong><span>为当前 Runtime 注入独立的模型路由</span></div>
              <el-switch :model-value="Boolean(item.profile.provider_config)" @change="toggleProviderConfig(item.profile, Boolean($event))" />
            </div>
            <div v-if="item.profile.provider_config" class="provider-fields">
              <label><span>供应商名称</span><el-input v-model="item.profile.provider_config.display_name" placeholder="例如：阿里云百炼" /></label>
              <label><span>供应商 ID</span><el-input v-model="item.profile.provider_config.provider_id" placeholder="例如：aliyun-bailian" /></label>
              <label><span>API 协议</span><el-select v-model="item.profile.provider_config.api_protocol" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true"><el-option label="OpenAI Chat Completions" value="openai-completions" /><el-option label="OpenAI Responses" value="openai-responses" /><el-option label="Anthropic Messages" value="anthropic-messages" /></el-select></label>
              <label class="is-wide"><span>API Base URL</span><el-input v-model="item.profile.provider_config.base_url" placeholder="https://example.com/v1" /></label>
              <label><span>模型 ID</span><el-input v-model="item.profile.model" placeholder="例如：glm-5.2" /></label>
              <label><span>API Key 环境变量</span><el-input v-model="item.profile.provider_config.api_key_env" placeholder="CUSTOM_LLM_API_KEY" /></label>
            </div>
            <label v-else><span>模型覆盖（可选）</span><el-input v-model="item.profile.model" placeholder="留空则使用 Harness 默认模型" /></label>
            <p class="credential-hint">这里只保存环境变量名，不保存密钥。<template v-if="item.profile.provider_config">启动 ObsidianBrain 前请设置 <code>{{ item.profile.provider_config.api_key_env || 'CUSTOM_LLM_API_KEY' }}</code>。</template><template v-else>凭据由 Harness 默认 Profile 或 Harness Web 的 Models 页面管理。</template></p>
            <div class="runtime-actions">
              <el-switch v-model="item.profile.enabled" active-text="启用" />
              <div>
                <el-button :loading="verifyingRuntimeId === item.profile.id" :disabled="!item.profile.enabled || savingRuntime" @click="verifyRuntime(item.profile)">验证已保存配置</el-button>
                <el-button type="primary" :loading="savingRuntime" @click="saveRuntime(item.profile)">保存</el-button>
              </div>
            </div>
          </article>
        </template>

        <template v-else-if="section === 'usage'">
          <header class="settings-section-head split usage-head">
            <div><span>运行可观测性</span><h2>Token 用量</h2><p>按时间与调用方查看输入、输出和调用趋势。</p></div>
            <div class="usage-filters">
              <el-date-picker v-model="usageDateRange" type="daterange" value-format="YYYY-MM-DD" format="YYYY/MM/DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" :clearable="false" unlink-panels popper-class="glass-picker" @change="loadUsage" />
              <el-select v-model="usageCaller" class="knowledge-select is-compact" popper-class="system-select-popper system-toolbar-popper" placement="bottom-start" :offset="0" :fit-input-width="true" @change="loadUsage">
                <el-option label="全部调用方" value="" />
                <el-option label="知识问答" value="knowledge_qa" />
                <el-option label="研究任务" value="knowledge_task" />
              </el-select>
            </div>
          </header>
          <div v-if="usageLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <div v-else class="usage-dashboard">
            <div class="usage-disclosure" :class="`is-${usageStats?.usage_source || 'unavailable'}`">
              <el-icon><InfoFilled /></el-icon>
              <span><strong>{{ usageSourceLabel }}</strong>当前 Harness ACP 未上报精确 token，现有问答和研究任务使用本地文本估算；未来运行时上报后会自动标记为实测。</span>
            </div>
            <div class="usage-metrics">
              <article><span>总 Token</span><strong>{{ formatTokenCount(usageStats?.totals.total_tokens || 0) }}</strong><small>{{ usageStats?.totals.runs || 0 }} 次有记录调用</small></article>
              <article><span>输入</span><strong>{{ formatTokenCount(usageStats?.totals.input_tokens || 0) }}</strong><small>提示词与检索证据</small></article>
              <article><span>输出</span><strong>{{ formatTokenCount(usageStats?.totals.output_tokens || 0) }}</strong><small>模型回答与任务结果</small></article>
              <article><span>未统计</span><strong>{{ usageStats?.totals.unreported_runs || 0 }}</strong><small>升级前的历史运行</small></article>
            </div>
            <section class="usage-chart-card">
              <header><div><span>每日趋势</span><strong>{{ usageDateRange[0] }} — {{ usageDateRange[1] }}</strong></div><em>输入 + 输出</em></header>
              <div v-if="usageDailyBars.length" class="usage-bars" aria-label="每日 Token 用量柱状图">
                <div v-for="point in usageDailyBars" :key="point.date" class="usage-bar-column" :title="`${point.date} · ${point.total_tokens} tokens`">
                  <span>{{ formatTokenCount(point.total_tokens) }}</span>
                  <i :style="{ height: `${point.height}%` }"></i>
                  <small>{{ point.label }}</small>
                </div>
              </div>
              <div v-else class="usage-empty">当前筛选范围内还没有可统计的调用。</div>
            </section>
            <section class="usage-callers">
              <header><span>调用方分布</span><strong>用于定位问答与研究任务的上下文成本</strong></header>
              <article v-for="item in usageStats?.by_caller || []" :key="item.caller">
                <div><strong>{{ callerLabel(item.caller) }}</strong><span>{{ item.runs }} 次 · {{ formatTokenCount(item.total_tokens) }} tokens</span></div>
                <i><b :style="{ width: `${callerShare(item.total_tokens)}%` }"></b></i>
              </article>
            </section>
          </div>
        </template>

        <template v-else-if="section === 'protection'">
          <header class="settings-section-head split protection-head">
            <div><span>本地数据安全</span><h2>数据保护</h2><p>快照使用 SQLite Online Backup，能够一致地包含 WAL 中尚未合并的写入。</p></div>
            <div class="protection-actions">
              <input ref="backupUploadInput" class="visually-hidden" type="file" accept=".sqlite,.sqlite3,.db,application/vnd.sqlite3" @change="selectRestoreUpload" />
              <el-button @click="backupUploadInput?.click()"><el-icon><UploadFilled /></el-icon>上传恢复</el-button>
              <el-button type="primary" :loading="creatingBackup" @click="createBackup"><el-icon><Plus /></el-icon>立即备份</el-button>
            </div>
          </header>
          <div class="protection-note">
            <el-icon><InfoFilled /></el-icon>
            <span><strong>自动保留最近 {{ backupRetention }} 份快照。</strong>数据库迁移和恢复前也会自动创建安全备份；有运行中的问答或研究任务时，系统会拒绝恢复。</span>
          </div>
          <section class="wiki-export-card">
            <div><span>单书可迁移导出</span><h3>导出可检查的知识资产</h3><p>JSON 包适合程序迁移与审计；Markdown Wiki 可直接解压浏览，包含实体、来源引用、问答与研究任务。</p></div>
            <div class="wiki-export-actions">
              <el-select v-model="activeBaseId" class="knowledge-select is-compact is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select>
              <a v-if="activeBaseId" :href="bookWikiExportDownloadUrl(activeBaseId, 'json')" download><el-button><el-icon><Download /></el-icon>结构化 JSON</el-button></a>
              <a v-if="activeBaseId" :href="bookWikiExportDownloadUrl(activeBaseId, 'markdown')" download><el-button><el-icon><Download /></el-icon>Markdown Wiki</el-button></a>
            </div>
          </section>
          <div v-if="backupLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <div v-else-if="!backups.length" class="knowledge-empty"><strong>还没有数据库快照</strong><span>创建第一份一致性备份，之后可随时下载到其他位置保存。</span></div>
          <div v-else class="backup-list">
            <article v-for="backup in backups" :key="backup.filename" class="backup-card">
              <div class="backup-symbol"><el-icon><Lock /></el-icon></div>
              <div class="backup-copy">
                <h3>{{ backupReasonLabel(backup.reason) }}</h3>
                <p>{{ formatBackupDate(backup.created_at) }} · {{ formatBytes(backup.size_bytes) }}</p>
                <code>{{ backup.filename }}</code>
              </div>
              <div class="backup-card-actions">
                <a :href="knowledgeBackupDownloadUrl(backup.filename)" download><el-button text><el-icon><Download /></el-icon>下载</el-button></a>
                <el-button text type="danger" @click="openManagedRestore(backup)">恢复</el-button>
              </div>
            </article>
          </div>
        </template>

        <template v-else-if="section === 'documents'">
          <header class="settings-section-head split"><div><span>提示词与规则</span><h2>配置文档</h2><p>内容保存在 SQLite，需要运行时才会物化为临时文件。</p></div><el-select v-model="activeBaseId" class="knowledge-select is-compact is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库" @change="loadSettings"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select></header>
          <div v-if="!activeBaseId" class="knowledge-empty"><strong>选择一本书</strong><span>查看并调整它的知识建模、问答与任务规则。</span></div>
          <div v-else class="document-editor">
            <nav class="document-tabs"><button v-for="document in documents" :key="document.id" :class="{ active: document.id === activeDocument?.id }" @click="activeDocumentId = document.id"><el-icon><Document /></el-icon>{{ document.name }}</button></nav>
            <template v-if="activeDocument">
              <div class="document-meta"><span>{{ activeDocument.scope === 'book' ? '书籍级配置' : '全局配置' }}</span><span>Revision {{ activeDocument.revision }}</span></div>
              <textarea v-model="activeDocument.content_md" spellcheck="false"></textarea>
              <div class="document-actions"><span>Markdown 内容仅作为配置载荷，数据库是唯一事实来源。</span><el-button type="primary" :loading="savingDocument" @click="saveDocument">保存文档</el-button></div>
            </template>
          </div>
        </template>

        <template v-else>
          <header class="settings-section-head split skills-head">
            <div><span>能力扩展</span><h2>Skills</h2><p>指令保存在 SQLite 并按版本审计；启用范围严格绑定当前书籍。</p></div>
            <div class="skills-actions">
              <el-select v-model="activeBaseId" class="knowledge-select is-compact is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库" @change="loadSkills"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select>
              <input ref="skillArchiveInput" class="visually-hidden" type="file" accept=".zip,application/zip" @change="importSkillArchive" />
              <el-button :loading="importingSkill" @click="skillArchiveInput?.click()"><el-icon><UploadFilled /></el-icon>导入 ZIP</el-button>
              <el-button type="primary" @click="openSkillEditor()"><el-icon><Plus /></el-icon>新增 Skill</el-button>
            </div>
          </header>
          <div v-if="!activeBaseId" class="knowledge-empty"><strong>选择一本书</strong><span>Skill 的启用状态不会跨知识库共享。</span></div>
          <div v-else-if="skillsLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <div v-else class="skills-grid">
            <article v-for="skill in skills" :key="skill.id" class="skill-card">
              <header>
                <div class="skill-symbol"><el-icon><MagicStick /></el-icon></div>
                <div><h3>{{ skill.name }}</h3><code>{{ skill.slug }}</code></div>
                <el-switch :model-value="skill.enabled" :loading="savingSkillId === skill.id" @change="toggleSkill(skill, Boolean($event))" />
              </header>
              <p>{{ skill.description || '没有补充说明' }}</p>
              <div class="skill-badges"><span>{{ skill.source_type === 'builtin' ? '内置' : '自定义' }}</span><span>Revision {{ skill.revision }}</span><span>只读知识</span></div>
              <footer>
                <el-select v-model="skill.usage_scope" class="knowledge-select is-compact" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" :disabled="savingSkillId === skill.id" @change="updateSkillScope(skill)">
                  <el-option label="问答与研究" value="both" /><el-option label="仅问答" value="qa" /><el-option label="仅研究" value="research" /><el-option label="仅智能编译" value="ingest" /><el-option label="全部场景" value="all" />
                </el-select>
                <div class="skill-card-actions">
                  <el-button text @click="openSkillDetail(skill)">查看内容</el-button>
                  <el-button v-if="skill.source_type === 'custom'" text @click="openSkillEditor(skill)">编辑</el-button>
                </div>
              </footer>
            </article>
          </div>
        </template>
      </section>
    </div>

    <MotionModal v-model="skillEditorVisible" aria-label="编辑 Wiki Skill">
      <div class="knowledge-modal-card">
        <div class="knowledge-modal-head"><h3>{{ skillDraft.id ? '编辑 Skill' : '新增 Skill' }}</h3><p>当前版本只允许 Markdown 指令，不执行脚本，也不会获得文件或 Shell 权限。</p></div>
        <div class="knowledge-modal-body skill-editor-form">
          <el-input v-model="skillDraft.name" :maxlength="100" placeholder="名称，例如：论证图谱" />
          <el-input v-model="skillDraft.slug" :disabled="Boolean(skillDraft.id)" :maxlength="64" placeholder="唯一标识，例如：argument-map" />
          <el-input v-model="skillDraft.description" type="textarea" :rows="2" :maxlength="500" show-word-limit placeholder="说明这个 Skill 解决什么问题" />
          <el-input v-model="skillDraft.instructions" type="textarea" :rows="8" :maxlength="12000" show-word-limit placeholder="写明分析步骤、质量要求与输出格式" />
        </div>
        <div class="knowledge-modal-actions"><el-button @click="skillEditorVisible = false">取消</el-button><el-button type="primary" :loading="savingSkill" :disabled="!skillDraft.name.trim() || !skillDraft.slug.trim() || !skillDraft.instructions.trim()" @click="saveSkill">保存版本</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="skillDetailVisible" aria-label="Skill 内容与版本" size="wide">
      <div class="knowledge-modal-card skill-detail-modal">
        <div class="knowledge-modal-head">
          <div><h3>Skill 内容与版本</h3><p>这里展示可供运行选用的指令、资源、权限声明和全部历史版本；是否实际注入以运行检查器为准。</p></div>
          <span v-if="skillDetail" class="knowledge-status" :class="skillDetail.skill.source_type === 'builtin' ? 'is-healthy' : 'is-draft'">{{ skillDetail.skill.source_type === 'builtin' ? '系统内置' : '自定义' }}</span>
        </div>
        <div v-if="skillDetailLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
        <div v-else-if="skillDetail" class="skill-detail-layout">
          <aside class="skill-detail-sidebar">
            <section class="skill-detail-summary">
              <span>{{ skillDetail.skill.slug }}</span>
              <strong>{{ skillDetail.skill.name }}</strong>
              <p>{{ skillDetail.skill.description || '没有补充说明' }}</p>
            </section>
            <section class="skill-detail-access">
              <h4>权限与依赖</h4>
              <div><span v-for="permission in skillDetail.skill.permissions" :key="permission">{{ permission }}</span><span v-if="!skillDetail.skill.permissions.length">无扩展权限</span></div>
              <small v-if="skillDetail.skill.requirements.length">依赖：{{ skillDetail.skill.requirements.join('、') }}</small>
              <small v-else>没有外部依赖，不能扩大运行工具权限。</small>
            </section>
            <section class="skill-detail-versions">
              <h4>版本历史</h4>
              <button v-for="version in skillDetail.versions" :key="version.id" type="button" :class="{ active: version.id === activeSkillVersionId }" @click="selectSkillVersion(version.id)">
                <span>Revision {{ version.revision }}<em :class="`is-${version.release_state}`">{{ skillReleaseLabel(version.release_state) }}</em></span><small>{{ formatSkillDate(version.created_at) }}</small>
              </button>
            </section>
          </aside>
          <section class="skill-detail-content">
            <nav class="skill-file-tabs" aria-label="Skill 文件">
              <button v-for="file in activeSkillVersion?.files || []" :key="file.relative_path" type="button" :class="{ active: file.relative_path === activeSkillFilePath }" @click="activeSkillFilePath = file.relative_path">{{ file.relative_path }}</button>
            </nav>
            <header v-if="activeSkillFile"><div><strong>{{ activeSkillFile.relative_path }}</strong><span>{{ activeSkillFile.media_type }} · {{ formatBytes(activeSkillFile.size_bytes) }}</span></div><code>{{ activeSkillFile.content_hash }}</code></header>
            <div v-if="activeSkillVersion" class="skill-version-audit">
              <section>
                <strong>版本说明</strong>
                <p>{{ activeSkillVersion.changelog || '没有版本说明' }}</p>
                <template v-if="activeSkillVersion.origin">
                  <a :href="activeSkillVersion.origin.repository_url" target="_blank" rel="noreferrer">{{ activeSkillVersion.origin.attribution || '查看上游来源' }}</a>
                  <small>{{ activeSkillVersion.origin.license_spdx }} · {{ activeSkillVersion.origin.source_ref || '未固定引用' }}</small>
                  <p>{{ activeSkillVersion.origin.adaptation_notes }}</p>
                </template>
                <small v-else>项目自有或用户创建内容，没有外部来源声明。</small>
              </section>
              <section>
                <strong>固定离线评测</strong>
                <template v-if="activeSkillVersion.latest_evaluation">
                  <div class="skill-eval-score" :class="{ passed: activeSkillVersion.latest_evaluation.passed }"><b>{{ Math.round(activeSkillVersion.latest_evaluation.score * 100) }}</b><span>/ 100</span><em>{{ activeSkillVersion.latest_evaluation.passed ? '通过' : '未通过' }}</em></div>
                  <small v-if="activeSkillVersion.latest_evaluation.baseline_score != null">当前基线 {{ Math.round(activeSkillVersion.latest_evaluation.baseline_score * 100) }} 分</small>
                  <p>{{ activeSkillVersion.latest_evaluation.findings.filter(item => !item.passed).map(item => item.name).join('、') || '所有结构与安全检查均已通过。' }}</p>
                </template>
                <p v-else>尚未运行固定评测。评测只检查结构与安全规则，不代表真实模型回答质量。</p>
              </section>
              <section>
                <strong>真实模型基准</strong>
                <template v-if="activeSkillVersion.latest_benchmark">
                  <div v-if="['queued', 'running'].includes(activeSkillVersion.latest_benchmark.status)" class="skill-benchmark-running">
                    <el-icon class="is-loading"><Loading /></el-icon><span>{{ benchmarkProgressLabel(activeSkillVersion.latest_benchmark) }}</span>
                  </div>
                  <template v-else-if="activeSkillVersion.latest_benchmark.status === 'completed'">
                    <div class="skill-eval-score" :class="{ passed: activeSkillVersion.latest_benchmark.passed }"><b>{{ Math.round((activeSkillVersion.latest_benchmark.candidate_score || 0) * 100) }}</b><span>/ 100</span><em>{{ activeSkillVersion.latest_benchmark.passed ? '通过' : '需改进' }}</em></div>
                    <small>当前版 {{ Math.round((activeSkillVersion.latest_benchmark.baseline_score || 0) * 100) }} 分 · 差异 {{ formatBenchmarkDelta(activeSkillVersion.latest_benchmark.score_delta) }}</small>
                    <p>{{ benchmarkSummary(activeSkillVersion.latest_benchmark) }}</p>
                    <details v-if="activeSkillVersion.latest_benchmark.results.length" class="skill-benchmark-details">
                      <summary>查看候选版逐样例结果</summary>
                      <div><article v-for="result in activeSkillVersion.latest_benchmark.results.filter(item => item.variant === 'candidate')" :key="result.case_id"><header><span>{{ result.name }}</span><em :class="{ passed: result.passed }">{{ Math.round(result.score * 100) }}</em></header><p>{{ result.response_text || result.error || '没有返回内容' }}</p><small>{{ result.citations.length ? `引用：${result.citations.join('、')}` : '没有引用' }}</small></article></div>
                    </details>
                  </template>
                  <p v-else class="skill-benchmark-error">{{ activeSkillVersion.latest_benchmark.error || '真实模型基准运行失败' }}</p>
                </template>
                <p v-else>尚未运行。它会在只读、无工具权限的 Harness 会话中对比当前版和候选版。</p>
              </section>
            </div>
            <pre v-if="activeSkillFile">{{ activeSkillFile.content_text }}</pre>
            <div v-else class="knowledge-empty"><strong>当前版本没有可显示文件</strong></div>
          </section>
        </div>
        <div class="knowledge-modal-actions">
          <el-button v-if="skillDetail?.skill.source_type === 'builtin'" @click="cloneSkillToCustom">复制为自定义</el-button>
          <el-button v-if="skillDetail?.skill.source_type === 'custom'" @click="editDetailedSkill">编辑当前版本</el-button>
          <el-button v-if="activeSkillVersion?.release_state === 'candidate'" :loading="skillVersionAction === 'evaluate'" @click="evaluateActiveSkillVersion">运行固定评测</el-button>
          <el-button v-if="activeSkillVersion?.release_state === 'candidate'" :loading="skillVersionAction === 'benchmark'" :disabled="!activeSkillVersion.latest_evaluation?.passed || ['queued', 'running'].includes(activeSkillVersion.latest_benchmark?.status || '')" @click="benchmarkActiveSkillVersion">运行真实基准</el-button>
          <el-button v-if="activeSkillVersion?.release_state === 'candidate'" type="primary" :loading="skillVersionAction === 'publish'" :disabled="!activeSkillVersion.latest_evaluation?.passed || !activeSkillVersion.latest_benchmark?.passed" @click="publishActiveSkillVersion">发布此版本</el-button>
          <el-button v-if="activeSkillVersion?.release_state === 'published' && activeSkillVersion.id !== skillDetail?.current_version_id" :loading="skillVersionAction === 'rollback'" @click="rollbackActiveSkillVersion">回滚到此版本</el-button>
          <el-button type="primary" @click="skillDetailVisible = false">完成</el-button>
        </div>
      </div>
    </MotionModal>

    <MotionModal v-model="restoreVisible" aria-label="恢复知识数据库">
      <div class="knowledge-modal-card restore-modal-card">
        <div class="knowledge-modal-head"><h3>恢复知识数据库</h3><p>这会用所选快照替换当前数据库。执行前系统会再创建一份安全备份。</p></div>
        <div class="knowledge-modal-body restore-form">
          <div class="restore-target"><span>{{ restoreFile ? '上传文件' : '应用快照' }}</span><strong>{{ restoreFile?.name || restoreTarget?.filename }}</strong><small v-if="restoreFile">{{ formatBytes(restoreFile.size) }}</small><small v-else-if="restoreTarget">{{ formatBackupDate(restoreTarget.created_at) }} · {{ formatBytes(restoreTarget.size_bytes) }}</small></div>
          <div class="restore-warning"><el-icon><InfoFilled /></el-icon><span>恢复期间请勿关闭应用。运行中的知识任务会阻止本次操作，恢复完成后页面将自动刷新。</span></div>
          <label><span>输入 RESTORE 确认</span><el-input v-model="restoreConfirmation" autocomplete="off" placeholder="RESTORE" /></label>
        </div>
        <div class="knowledge-modal-actions"><el-button :disabled="restoring" @click="restoreVisible = false">取消</el-button><el-button type="danger" :loading="restoring" :disabled="restoreConfirmation !== 'RESTORE'" @click="performRestore">确认恢复</el-button></div>
      </div>
    </MotionModal>
  </KnowledgePageShell>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Cpu, DataAnalysis, Document, Download, InfoFilled, Loading, Lock, MagicStick, Plus, UploadFilled } from '@element-plus/icons-vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import {
  getBookWikiSettings,
  getAgentUsageStats,
  getWikiSkillBenchmark,
  bookWikiExportDownloadUrl,
  createKnowledgeBackup,
  evaluateWikiSkillVersion,
  getWikiSkillDetail,
  importWikiSkillArchive,
  knowledgeBackupDownloadUrl,
  listBookKnowledgeBases,
  listKnowledgeBackups,
  listWikiSkills,
  restoreKnowledgeBackup,
  rollbackWikiSkillVersion,
  saveAgentRuntimeProfile,
  saveBookWikiConfigDocument,
  saveCustomWikiSkill,
  startWikiSkillBenchmark,
  publishWikiSkillVersion,
  setWikiSkillBinding,
  uploadAndRestoreKnowledgeBackup,
  verifyAgentRuntime,
  type ConfigDocument,
  type DatabaseBackup,
  type AgentUsageStats,
  type KnowledgeBaseSummary,
  type RuntimeHealth,
  type RuntimeProfile,
  type WikiSkill,
  type WikiSkillDetail,
  type WikiSkillBenchmarkRun,
} from '@/api/knowledge'

const section = ref<'runtime' | 'usage' | 'protection' | 'documents' | 'skills'>('runtime')
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const documents = ref<ConfigDocument[]>([])
const activeDocumentId = ref('')
const runtimeHealth = ref<RuntimeHealth[]>([])
const loading = ref(false)
const savingRuntime = ref(false)
const verifyingRuntimeId = ref('')
const runtimeVerification = ref<Record<string, string>>({})
const savingDocument = ref(false)
const usageLoading = ref(false)
const usageCaller = ref<'' | 'knowledge_qa' | 'knowledge_task'>('')
const usageDateRange = ref<[string, string]>(defaultUsageRange())
const usageStats = ref<AgentUsageStats | null>(null)
const backups = ref<DatabaseBackup[]>([])
const backupRetention = ref(7)
const backupLoading = ref(false)
const creatingBackup = ref(false)
const backupUploadInput = ref<HTMLInputElement | null>(null)
const restoreVisible = ref(false)
const restoreTarget = ref<DatabaseBackup | null>(null)
const restoreFile = ref<File | null>(null)
const restoreConfirmation = ref('')
const restoring = ref(false)
const skills = ref<WikiSkill[]>([])
const skillsLoading = ref(false)
const savingSkillId = ref('')
const savingSkill = ref(false)
const importingSkill = ref(false)
const skillArchiveInput = ref<HTMLInputElement | null>(null)
const skillEditorVisible = ref(false)
const skillDetailVisible = ref(false)
const skillDetailLoading = ref(false)
const skillDetail = ref<WikiSkillDetail | null>(null)
const activeSkillVersionId = ref('')
const activeSkillFilePath = ref('')
const skillVersionAction = ref('')
const skillBenchmarkPollingRunId = ref('')
const skillDraft = reactive({ id: '', slug: '', name: '', description: '', instructions: '', revision: 0 })
const activeDocument = computed(() => documents.value.find(document => document.id === activeDocumentId.value))
const activeSkillVersion = computed(() => skillDetail.value?.versions.find(version => version.id === activeSkillVersionId.value) || null)
const activeSkillFile = computed(() => activeSkillVersion.value?.files.find(file => file.relative_path === activeSkillFilePath.value) || activeSkillVersion.value?.files[0] || null)
const usageDailyBars = computed(() => {
  const points = usageStats.value?.daily || []
  const maximum = Math.max(...points.map(point => point.total_tokens), 1)
  return points.map(point => ({
    ...point,
    height: Math.max(8, Math.round((point.total_tokens / maximum) * 100)),
    label: new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric' }).format(new Date(`${point.date}T00:00:00`)),
  }))
})
const usageSourceLabel = computed(() => ({
  measured: '实测数据',
  mixed: '混合数据',
  estimated: '估算数据',
  unavailable: '暂无数据',
}[usageStats.value?.usage_source || 'unavailable']))

function defaultUsageRange(): [string, string] {
  const end = new Date()
  const start = new Date(end)
  start.setDate(start.getDate() - 29)
  return [formatDateValue(start), formatDateValue(end)]
}

function formatDateValue(value: Date) {
  const year = value.getFullYear()
  const month = String(value.getMonth() + 1).padStart(2, '0')
  const day = String(value.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function formatTokenCount(value: number) {
  return new Intl.NumberFormat('zh-CN', { notation: value >= 10_000 ? 'compact' : 'standard', maximumFractionDigits: 1 }).format(value)
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB`
  if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MB`
  return `${(value / 1024 ** 3).toFixed(2)} GB`
}

function formatBackupDate(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  }).format(new Date(value))
}

function formatSkillDate(value: string) {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function skillReleaseLabel(state: 'candidate' | 'published' | 'retired') {
  return state === 'candidate' ? '候选' : state === 'published' ? '已发布' : '已退役'
}

function skillEvaluationSuite(slug: string) {
  if (slug === 'book-ingest') return 'semantic-ingest'
  if (slug === 'book-query') return 'grounded-query'
  if (slug === 'book-research') return 'evidence-research'
  if (slug === 'book-presentation') return 'evidence-presentation'
  return 'grounded-query'
}

function formatBenchmarkDelta(value?: number | null) {
  const points = Math.round((value || 0) * 100)
  return `${points > 0 ? '+' : ''}${points} 分`
}

function benchmarkProgressLabel(run: WikiSkillBenchmarkRun) {
  if (run.status === 'queued') return `等待运行 · ${run.total_cases} 个固定样例`
  if (run.completed_cases >= run.total_cases) return '候选版已完成，正在运行当前版对照'
  return `正在运行候选版 · ${run.total_cases} 个固定样例`
}

function benchmarkSummary(run: WikiSkillBenchmarkRun) {
  const candidate = run.results.filter(result => result.variant === 'candidate')
  const failed = candidate.filter(result => !result.passed)
  if (!failed.length) return '候选版通过全部真实样例。发布仍需满足总分及相对当前版的回退门槛。'
  return `未通过：${failed.slice(0, 3).map(result => result.name).join('、')}${failed.length > 3 ? ` 等 ${failed.length} 项` : ''}`
}

function backupReasonLabel(reason: string) {
  if (reason === 'manual') return '手动备份'
  if (reason === 'pre-migration') return '迁移前自动备份'
  if (reason === 'pre-restore') return '恢复前安全备份'
  return reason
}

async function loadBackups() {
  backupLoading.value = true
  try {
    const response = await listKnowledgeBackups()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '备份列表加载失败')
    backups.value = response.result.backups
    backupRetention.value = response.result.retention
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    backupLoading.value = false
  }
}

async function createBackup() {
  creatingBackup.value = true
  try {
    const response = await createKnowledgeBackup()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '数据库备份失败')
    ElMessage.success('一致性数据库快照已创建')
    await loadBackups()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    creatingBackup.value = false
  }
}

function openManagedRestore(backup: DatabaseBackup) {
  restoreTarget.value = backup
  restoreFile.value = null
  restoreConfirmation.value = ''
  restoreVisible.value = true
}

function selectRestoreUpload(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  restoreTarget.value = null
  restoreFile.value = file
  restoreConfirmation.value = ''
  restoreVisible.value = true
}

async function performRestore() {
  if (restoreConfirmation.value !== 'RESTORE' || (!restoreTarget.value && !restoreFile.value)) return
  restoring.value = true
  try {
    if (restoreFile.value) {
      await uploadAndRestoreKnowledgeBackup(restoreFile.value, restoreConfirmation.value)
    } else if (restoreTarget.value) {
      const response = await restoreKnowledgeBackup(restoreTarget.value.filename, restoreConfirmation.value)
      if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '数据库恢复失败')
    }
    ElMessage.success('数据库已恢复并通过完整性检查')
    window.setTimeout(() => window.location.reload(), 500)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    restoring.value = false
  }
}

function callerLabel(caller: string) {
  return caller === 'knowledge_qa' ? '知识问答' : caller === 'knowledge_task' ? '研究任务' : caller
}

function callerShare(tokens: number) {
  const total = usageStats.value?.totals.total_tokens || 0
  return total ? Math.max(3, Math.round((tokens / total) * 100)) : 0
}

function toggleProviderConfig(profile: RuntimeProfile, enabled: boolean) {
  profile.provider_config = enabled
    ? {
        provider_id: 'custom-provider',
        display_name: '自定义供应商',
        api_protocol: 'openai-completions',
        base_url: '',
        api_key_env: 'CUSTOM_LLM_API_KEY',
      }
    : null
  delete runtimeVerification.value[profile.id]
}

async function initialize() {
  try {
    const response = await listBookKnowledgeBases()
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '知识库加载失败')
    bases.value = response.result.items.flatMap(card => (
      card.book.kind === 'folder' && card.knowledge_base ? [card.knowledge_base] : []
    ))
    activeBaseId.value = bases.value[0]?.id || ''
    await Promise.all([loadSettings(), loadSkills()])
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

async function loadSkills() {
  if (!activeBaseId.value) {
    skills.value = []
    return
  }
  skillsLoading.value = true
  try {
    const response = await listWikiSkills(activeBaseId.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skills 加载失败')
    skills.value = response.result.skills
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    skillsLoading.value = false
  }
}

async function openSkillDetail(skill: WikiSkill) {
  skillDetailVisible.value = true
  skillDetailLoading.value = true
  skillDetail.value = null
  activeSkillVersionId.value = ''
  activeSkillFilePath.value = ''
  try {
    const response = await getWikiSkillDetail(skill.id, activeBaseId.value || undefined)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 详情加载失败')
    const detail = response.result
    skillDetail.value = detail
    const current = detail.versions.find(version => version.id === detail.current_version_id) || detail.versions[0]
    if (current) {
      selectSkillVersion(current.id)
      if (current.latest_benchmark && ['queued', 'running'].includes(current.latest_benchmark.status)) {
        void resumeSkillBenchmark(current.latest_benchmark, current.id)
      }
    }
  } catch (error) {
    skillDetailVisible.value = false
    ElMessage.error((error as Error).message)
  } finally {
    skillDetailLoading.value = false
  }
}

function selectSkillVersion(versionId: string) {
  activeSkillVersionId.value = versionId
  const version = skillDetail.value?.versions.find(item => item.id === versionId)
  activeSkillFilePath.value = version?.files.find(file => file.relative_path === 'SKILL.md')?.relative_path || version?.files[0]?.relative_path || ''
  if (version?.latest_benchmark && ['queued', 'running'].includes(version.latest_benchmark.status)) {
    void resumeSkillBenchmark(version.latest_benchmark, version.id)
  }
}

async function refreshSkillDetail(selectedVersionId?: string) {
  const skillId = skillDetail.value?.skill.id
  if (!skillId) return
  const response = await getWikiSkillDetail(skillId, activeBaseId.value || undefined)
  if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 详情刷新失败')
  const detail = response.result
  skillDetail.value = detail
  const selected = detail.versions.find(version => version.id === selectedVersionId)
    || detail.versions.find(version => version.id === detail.current_version_id)
    || detail.versions[0]
  if (selected) selectSkillVersion(selected.id)
}

async function evaluateActiveSkillVersion() {
  if (!skillDetail.value || !activeSkillVersion.value) return
  skillVersionAction.value = 'evaluate'
  try {
    const response = await evaluateWikiSkillVersion(skillDetail.value.skill.id, activeSkillVersion.value.id, skillEvaluationSuite(skillDetail.value.skill.slug))
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '固定评测失败')
    await refreshSkillDetail(activeSkillVersion.value.id)
    ElMessage.success(response.result.passed ? `固定评测通过：${Math.round(response.result.score * 100)} 分` : `固定评测未通过：${Math.round(response.result.score * 100)} 分`)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    skillVersionAction.value = ''
  }
}

async function benchmarkActiveSkillVersion() {
  if (!skillDetail.value || !activeSkillVersion.value || !activeBaseId.value) return
  const versionId = activeSkillVersion.value.id
  skillVersionAction.value = 'benchmark'
  try {
    const response = await startWikiSkillBenchmark(
      activeBaseId.value,
      skillDetail.value.skill.id,
      versionId,
      skillEvaluationSuite(skillDetail.value.skill.slug),
    )
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '真实模型基准启动失败')
    updateSkillBenchmark(versionId, response.result)
    const run = await pollSkillBenchmark(response.result, versionId)
    if (run.status === 'completed') {
      ElMessage.success(run.passed ? `真实模型基准通过：${Math.round((run.candidate_score || 0) * 100)} 分` : '真实模型基准完成，候选版本尚未达到发布门槛')
    } else if (run.status === 'failed') {
      throw new Error(run.error || '真实模型基准运行失败')
    }
    await refreshSkillDetail(versionId)
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    skillVersionAction.value = ''
  }
}

async function pollSkillBenchmark(initial: WikiSkillBenchmarkRun, versionId: string) {
  let run = initial
  skillBenchmarkPollingRunId.value = run.id
  try {
    while (['queued', 'running'].includes(run.status) && skillDetailVisible.value && skillBenchmarkPollingRunId.value === run.id) {
      await new Promise(resolve => window.setTimeout(resolve, 1500))
      if (!skillDetailVisible.value || skillBenchmarkPollingRunId.value !== run.id) break
      const polled = await getWikiSkillBenchmark(run.id)
      if (polled.status !== 'success' || !polled.result) throw new Error(polled.error?.message || '真实模型基准状态读取失败')
      run = polled.result
      updateSkillBenchmark(versionId, run)
    }
    return run
  } finally {
    if (skillBenchmarkPollingRunId.value === run.id) skillBenchmarkPollingRunId.value = ''
  }
}

async function resumeSkillBenchmark(run: WikiSkillBenchmarkRun, versionId: string) {
  if (skillBenchmarkPollingRunId.value === run.id) return
  try {
    const completed = await pollSkillBenchmark(run, versionId)
    if (!['queued', 'running'].includes(completed.status)) await refreshSkillDetail(versionId)
  } catch (error) {
    ElMessage.error((error as Error).message)
  }
}

function updateSkillBenchmark(versionId: string, benchmark: WikiSkillBenchmarkRun) {
  const version = skillDetail.value?.versions.find(item => item.id === versionId)
  if (version) version.latest_benchmark = benchmark
}

async function publishActiveSkillVersion() {
  if (!skillDetail.value || !activeSkillVersion.value) return
  skillVersionAction.value = 'publish'
  try {
    const response = await publishWikiSkillVersion(skillDetail.value.skill.id, activeSkillVersion.value.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 发布失败')
    skillDetail.value = response.result
    selectSkillVersion(response.result.current_version_id)
    await loadSkills()
    ElMessage.success('候选版本已发布，后续运行会使用新版本')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    skillVersionAction.value = ''
  }
}

async function rollbackActiveSkillVersion() {
  if (!skillDetail.value || !activeSkillVersion.value) return
  skillVersionAction.value = 'rollback'
  try {
    const response = await rollbackWikiSkillVersion(skillDetail.value.skill.id, activeSkillVersion.value.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 回滚失败')
    skillDetail.value = response.result
    selectSkillVersion(response.result.current_version_id)
    await loadSkills()
    ElMessage.success('已回滚到所选版本')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    skillVersionAction.value = ''
  }
}

function customSkillSlug(slug: string) {
  const stem = `${slug}-custom`.slice(0, 58).replace(/-+$/g, '')
  let candidate = stem
  let sequence = 2
  while (skills.value.some(skill => skill.slug === candidate)) {
    candidate = `${stem}-${sequence}`
    sequence += 1
  }
  return candidate
}

function cloneSkillToCustom() {
  const skill = skillDetail.value?.skill
  if (!skill) return
  Object.assign(skillDraft, {
    id: '',
    slug: customSkillSlug(skill.slug),
    name: `${skill.name} · 自定义`,
    description: skill.description,
    instructions: skill.instructions,
    revision: 0,
  })
  skillDetailVisible.value = false
  skillEditorVisible.value = true
}

function editDetailedSkill() {
  const skill = skillDetail.value?.skill
  if (!skill || skill.source_type !== 'custom') return
  skillDetailVisible.value = false
  openSkillEditor(skill)
}

function openSkillEditor(skill?: WikiSkill) {
  Object.assign(skillDraft, skill ? {
    id: skill.id,
    slug: skill.slug,
    name: skill.name,
    description: skill.description,
    instructions: skill.instructions,
    revision: skill.revision,
  } : { id: '', slug: '', name: '', description: '', instructions: '', revision: 0 })
  skillEditorVisible.value = true
}

async function saveSkill() {
  savingSkill.value = true
  try {
    const response = await saveCustomWikiSkill({
      ...(skillDraft.id ? { skillId: skillDraft.id, expectedRevision: skillDraft.revision } : {}),
      slug: skillDraft.slug.trim(),
      name: skillDraft.name.trim(),
      description: skillDraft.description.trim(),
      instructions: skillDraft.instructions.trim(),
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 保存失败')
    skillEditorVisible.value = false
    ElMessage.success(skillDraft.id ? 'Skill 新版本已保存' : 'Skill 已创建')
    await loadSkills()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingSkill.value = false
  }
}

async function importSkillArchive(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  importingSkill.value = true
  try {
    const response = await importWikiSkillArchive(file)
    ElMessage.success(`Skill「${response.skill.name}」已导入；启用前请检查内容`)
    await loadSkills()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    importingSkill.value = false
  }
}

async function persistSkillBinding(skill: WikiSkill, enabled: boolean) {
  if (!activeBaseId.value) return
  savingSkillId.value = skill.id
  try {
    const response = await setWikiSkillBinding({
      knowledgeBaseId: activeBaseId.value,
      skillId: skill.id,
      enabled,
      usageScope: skill.usage_scope,
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Skill 配置保存失败')
    const index = skills.value.findIndex(item => item.id === skill.id)
    if (index >= 0) skills.value[index] = response.result
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadSkills()
  } finally {
    savingSkillId.value = ''
  }
}

function toggleSkill(skill: WikiSkill, enabled: boolean) {
  void persistSkillBinding(skill, enabled)
}

function updateSkillScope(skill: WikiSkill) {
  void persistSkillBinding(skill, skill.enabled)
}

async function loadSettings() {
  loading.value = true
  try {
    const response = await getBookWikiSettings(activeBaseId.value || undefined)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '配置加载失败')
    runtimeHealth.value = response.result.runtime_profiles
    documents.value = response.result.documents
    activeDocumentId.value = documents.value.some(document => document.id === activeDocumentId.value)
      ? activeDocumentId.value
      : (documents.value[0]?.id || '')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    loading.value = false
  }
}

async function loadUsage() {
  if (usageDateRange.value.length !== 2) return
  usageLoading.value = true
  try {
    const response = await getAgentUsageStats({
      startDate: usageDateRange.value[0],
      endDate: usageDateRange.value[1],
      ...(usageCaller.value ? { caller: usageCaller.value } : {}),
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || 'Token 用量加载失败')
    usageStats.value = response.result
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    usageLoading.value = false
  }
}

async function saveRuntime(profile: RuntimeProfile) {
  savingRuntime.value = true
  try {
    const response = await saveAgentRuntimeProfile(profile)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '运行配置保存失败')
    delete runtimeVerification.value[profile.id]
    ElMessage.success('运行配置已保存')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingRuntime.value = false
  }
}

async function verifyRuntime(profile: RuntimeProfile) {
  verifyingRuntimeId.value = profile.id
  try {
    const response = await verifyAgentRuntime(profile.id)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '连接验证失败')
    runtimeVerification.value[profile.id] = response.result.message
    ElMessage.success(response.result.message)
  } catch (error) {
    delete runtimeVerification.value[profile.id]
    ElMessage.error((error as Error).message)
  } finally {
    verifyingRuntimeId.value = ''
  }
}

async function saveDocument() {
  if (!activeDocument.value) return
  savingDocument.value = true
  try {
    const response = await saveBookWikiConfigDocument(activeDocument.value)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '文档保存失败')
    const index = documents.value.findIndex(document => document.id === response.result?.id)
    if (index >= 0) documents.value[index] = response.result
    ElMessage.success('配置文档已保存到数据库')
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingDocument.value = false
  }
}

watch(section, value => {
  if (value === 'usage' && !usageStats.value) void loadUsage()
  if (value === 'protection' && !backups.value.length) void loadBackups()
  if (value === 'skills' && !skills.value.length) void loadSkills()
})
onMounted(initialize)
</script>

<style scoped>
.settings-layout { min-height: 0; display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 12px; }
.settings-nav { align-self: start; display: grid; gap: 4px; padding: 7px; }
.settings-nav button { display: flex; align-items: center; gap: 11px; min-height: 58px; padding: 9px 11px; border: 0; border-radius: 13px; background: transparent; color: var(--text-muted); text-align: left; cursor: pointer; transition: var(--transition-interactive); }
.settings-nav button:hover { background: var(--bg-hover); color: var(--text-primary); }
.settings-nav button.active { background: var(--accent-light); color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent-border); }
.settings-nav .el-icon { flex: none; font-size: 19px; }
.settings-nav button span { display: grid; gap: 2px; }
.settings-nav strong { color: inherit; font-size: 13px; }
.settings-nav small { color: var(--text-faint); font-size: 10px; }
.settings-main { min-width: 0; container-type: inline-size; padding: clamp(16px, 1.8vw, 24px); }
.settings-section-head { margin-bottom: 18px; padding-bottom: 14px; border-bottom: 1px solid var(--border-faint); }
.settings-section-head.split { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; }
.settings-section-head span { color: var(--accent); font-size: 10px; font-weight: 720; letter-spacing: .08em; }
.settings-section-head h2 { margin: 5px 0 4px; font-size: 20px; font-weight: 680; }
.settings-section-head p { color: var(--text-muted); font-size: 12px; }
.settings-loading { min-height: 260px; display: grid; place-content: center; color: var(--accent); }
.runtime-card { min-width: 0; display: grid; gap: 15px; padding: 18px; border: 1px solid var(--border-faint); border-radius: 17px; background: var(--bg-glass-subtle); }
.runtime-title { display: flex; align-items: center; gap: 11px; }
.runtime-logo { width: 44px; height: 44px; display: grid; place-items: center; border-radius: 14px; background: linear-gradient(145deg, var(--accent), #3c3fae); color: white; font-size: 12px; font-weight: 760; box-shadow: 0 8px 18px color-mix(in srgb, var(--accent) 22%, transparent); }
.runtime-title > div:nth-child(2) { min-width: 0; flex: 1; }
.runtime-title h3 { margin: 0; overflow-wrap: anywhere; font-size: 16px; }
.runtime-title p { color: var(--text-faint); font-size: 10px; }
.runtime-message { padding: 9px 11px; overflow-wrap: anywhere; border-radius: 10px; background: var(--bg-glass); color: var(--text-muted); font-family: var(--font-mono); font-size: 10px; }
.runtime-message.is-verified { background: color-mix(in srgb, #34c759 11%, transparent); color: #248a3d; }
.runtime-card label { display: grid; gap: 6px; }
.runtime-card label > span { color: var(--text-muted); font-size: 11px; }
.provider-mode { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 11px 13px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); }
.provider-mode > div { display: grid; gap: 2px; }
.provider-mode strong { font-size: 12px; }
.provider-mode span { color: var(--text-faint); font-size: 10px; }
.provider-fields { min-width: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; padding: 14px; border: 1px solid var(--accent-border); border-radius: 14px; background: var(--accent-light); }
.provider-fields label { min-width: 0; }
.provider-fields .is-wide { grid-column: 1 / -1; }
.credential-hint { color: var(--text-faint); font-size: 10px; line-height: 1.55; }
.credential-hint code { color: var(--text-muted); font-family: var(--font-mono); }
.runtime-actions { display: flex; align-items: center; justify-content: space-between; }
.runtime-actions > div { display: flex; gap: 8px; }
.usage-head { align-items: flex-start !important; }
.usage-filters { width: min(100%, 520px); display: grid; grid-template-columns: minmax(250px, 1fr) 170px; gap: 8px; }
.usage-filters :deep(.el-date-editor) { min-width: 0; width: 100%; max-width: 100%; min-height: 40px; border-radius: 12px; background: var(--bg-glass-subtle); box-shadow: inset 0 0 0 1px var(--border-faint); }
.usage-dashboard { display: grid; gap: 13px; }
.usage-disclosure { display: flex; align-items: flex-start; gap: 9px; padding: 11px 13px; border: 1px solid var(--border-faint); border-radius: 13px; background: var(--bg-glass-subtle); color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.usage-disclosure .el-icon { flex: none; margin-top: 2px; color: var(--accent); font-size: 15px; }
.usage-disclosure strong { margin-right: 7px; color: var(--text-primary); }
.usage-disclosure.is-estimated { border-color: color-mix(in srgb, #ff9f0a 28%, var(--border-faint)); background: color-mix(in srgb, #ff9f0a 6%, var(--bg-glass-subtle)); }
.usage-metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 9px; }
.usage-metrics article { min-width: 0; display: grid; gap: 3px; padding: 15px; border: 1px solid var(--border-faint); border-radius: 15px; background: var(--bg-glass-subtle); box-shadow: var(--inset-highlight); }
.usage-metrics span { color: var(--text-faint); font-size: 10px; }
.usage-metrics strong { overflow: hidden; color: var(--text-primary); font-size: clamp(20px, 3vw, 28px); font-variant-numeric: tabular-nums; text-overflow: ellipsis; }
.usage-metrics small { color: var(--text-muted); font-size: 9px; }
.usage-chart-card, .usage-callers { padding: 16px; border: 1px solid var(--border-faint); border-radius: 16px; background: var(--bg-glass-subtle); }
.usage-chart-card > header, .usage-callers > header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.usage-chart-card > header div { display: grid; gap: 3px; }
.usage-chart-card > header span, .usage-callers > header span { color: var(--text-primary); font-size: 13px; font-weight: 700; }
.usage-chart-card > header strong, .usage-callers > header strong { color: var(--text-faint); font-size: 9px; font-weight: 500; }
.usage-chart-card > header em { color: var(--text-faint); font-size: 9px; font-style: normal; }
.usage-bars { height: 220px; display: flex; align-items: stretch; gap: clamp(3px, .8vw, 9px); margin-top: 18px; overflow-x: auto; padding-top: 22px; }
.usage-bar-column { min-width: 15px; flex: 1; display: grid; grid-template-rows: 14px 1fr 15px; justify-items: center; align-items: end; gap: 4px; }
.usage-bar-column > span { max-width: 46px; overflow: hidden; color: var(--text-faint); font-size: 8px; font-variant-numeric: tabular-nums; text-overflow: ellipsis; white-space: nowrap; opacity: 0; transition: opacity var(--motion-fast) var(--ease-emphasized); }
.usage-bar-column:hover > span { opacity: 1; }
.usage-bar-column > i { width: min(100%, 22px); min-height: 5px; border-radius: 7px 7px 3px 3px; background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 72%, white), var(--accent)); box-shadow: 0 5px 12px color-mix(in srgb, var(--accent) 15%, transparent); transform-origin: bottom; animation: usage-rise var(--motion-slow) var(--ease-emphasized) both; }
.usage-bar-column > small { color: var(--text-faint); font-size: 8px; white-space: nowrap; }
.usage-empty { min-height: 180px; display: grid; place-content: center; color: var(--text-faint); font-size: 11px; }
.usage-callers { display: grid; gap: 12px; }
.usage-callers > header { margin-bottom: 2px; }
.usage-callers article { display: grid; gap: 6px; }
.usage-callers article > div { display: flex; justify-content: space-between; gap: 12px; font-size: 11px; }
.usage-callers article span { color: var(--text-faint); font-size: 9px; }
.usage-callers article > i { height: 7px; overflow: hidden; border-radius: 999px; background: color-mix(in srgb, var(--text-primary) 6%, transparent); }
.usage-callers article > i b { height: 100%; display: block; border-radius: inherit; background: var(--accent); transition: width var(--motion-slow) var(--ease-emphasized); }
@keyframes usage-rise { from { transform: scaleY(.08); opacity: .35; } }
.protection-head { align-items: flex-start !important; }
.protection-actions { display: flex; gap: 8px; }
.protection-note { display: flex; align-items: flex-start; gap: 10px; margin-bottom: 14px; padding: 12px 14px; border: 1px solid var(--accent-border); border-radius: 14px; background: var(--accent-light); color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.protection-note .el-icon { flex: none; margin-top: 2px; color: var(--accent); font-size: 16px; }
.protection-note strong { display: block; color: var(--text-primary); }
.wiki-export-card { display: flex; align-items: center; justify-content: space-between; gap: 18px; margin-bottom: 14px; padding: 16px; border: 1px solid var(--border-faint); border-radius: 16px; background: var(--bg-glass-subtle); box-shadow: var(--inset-highlight); }
.wiki-export-card > div:first-child { min-width: 0; flex: 1; display: grid; gap: 3px; }
.wiki-export-card span { color: var(--accent); font-size: 9px; font-weight: 720; letter-spacing: .06em; }
.wiki-export-card h3 { margin: 0; color: var(--text-primary); font-size: 14px; }
.wiki-export-card p { max-width: 590px; color: var(--text-muted); font-size: 10px; line-height: 1.55; }
.wiki-export-actions { width: min(100%, 460px); display: grid; grid-template-columns: minmax(150px, 1fr) auto auto; gap: 7px; }
.wiki-export-actions a { text-decoration: none; }
.backup-list { display: grid; gap: 9px; }
.backup-card { min-width: 0; display: flex; align-items: center; gap: 12px; padding: 14px; border: 1px solid var(--border-faint); border-radius: 16px; background: var(--bg-glass-subtle); box-shadow: var(--inset-highlight); transition: var(--transition-interactive); }
.backup-card:hover { border-color: var(--accent-border); transform: translateY(-1px); }
.backup-symbol { width: 42px; height: 42px; flex: none; display: grid; place-items: center; border-radius: 13px; background: var(--accent-light); color: var(--accent); font-size: 18px; }
.backup-copy { min-width: 0; flex: 1; display: grid; gap: 2px; }
.backup-copy h3 { margin: 0; color: var(--text-primary); font-size: 13px; }
.backup-copy p { color: var(--text-muted); font-size: 10px; }
.backup-copy code { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.backup-card-actions { display: flex; align-items: center; gap: 2px; }
.backup-card-actions a { text-decoration: none; }
.restore-modal-card { width: 100%; }
.restore-form { display: grid; gap: 13px; }
.restore-target { display: grid; gap: 3px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 13px; background: var(--bg-glass-subtle); }
.restore-target span, .restore-form label > span { color: var(--text-faint); font-size: 9px; }
.restore-target strong { overflow: hidden; color: var(--text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.restore-target small { color: var(--text-muted); font-size: 10px; }
.restore-warning { display: flex; gap: 9px; padding: 11px 12px; border: 1px solid color-mix(in srgb, #ff453a 24%, var(--border-faint)); border-radius: 12px; background: color-mix(in srgb, #ff453a 6%, transparent); color: var(--text-muted); font-size: 10px; line-height: 1.55; }
.restore-warning .el-icon { flex: none; margin-top: 2px; color: #ff453a; }
.restore-form label { display: grid; gap: 6px; }
.document-editor { display: grid; gap: 12px; }
.document-tabs { display: flex; gap: 6px; overflow-x: auto; }
.document-tabs button { min-height: 38px; display: flex; align-items: center; gap: 6px; padding: 0 12px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-muted); font: inherit; font-size: 11px; white-space: nowrap; cursor: pointer; }
.document-tabs button.active { border-color: var(--accent-border); background: var(--accent-light); color: var(--accent); }
.document-meta { display: flex; justify-content: space-between; color: var(--text-faint); font-size: 10px; }
.document-editor textarea { width: 100%; min-height: 340px; resize: vertical; padding: 17px; border: 1px solid var(--border-subtle); border-radius: 15px; background: color-mix(in srgb, var(--bg-base) 55%, transparent); color: var(--text-secondary); font-family: var(--font-mono); font-size: 12px; line-height: 1.75; }
.document-editor textarea:focus { border-color: var(--accent-border); }
.document-actions { display: flex; align-items: center; justify-content: space-between; gap: 15px; }
.document-actions span { color: var(--text-faint); font-size: 10px; }
.skills-head { align-items: flex-start !important; }
.skills-actions { display: flex; align-items: center; gap: 8px; }
.skills-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 11px; }
.skill-card { min-width: 0; display: grid; gap: 13px; padding: 17px; border: 1px solid var(--border-faint); border-radius: 17px; background: var(--bg-glass-subtle); box-shadow: var(--inset-highlight); transition: var(--transition-interactive); }
.skill-card:hover { border-color: var(--accent-border); transform: translateY(-1px); }
.skill-card header { display: flex; align-items: center; gap: 10px; }
.skill-card header > div:nth-child(2) { min-width: 0; flex: 1; display: grid; gap: 2px; }
.skill-symbol { width: 38px; height: 38px; flex: none; display: grid; place-items: center; border-radius: 12px; background: var(--accent-light); color: var(--accent); font-size: 18px; }
.skill-card h3 { margin: 0; overflow: hidden; font-size: 14px; text-overflow: ellipsis; white-space: nowrap; }
.skill-card code { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.skill-card > p { min-height: 38px; color: var(--text-muted); font-size: 11px; line-height: 1.65; }
.skill-badges { display: flex; flex-wrap: wrap; gap: 5px; }
.skill-badges span { padding: 3px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent); font-size: 8px; font-weight: 650; }
.skill-card footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding-top: 12px; border-top: 1px solid var(--border-faint); }
.skill-card footer .knowledge-select { width: 142px; }
.skill-card-actions { display: flex; align-items: center; gap: 2px; }
.skill-editor-form { display: grid; gap: 11px; }
.skill-detail-modal { width: 100%; height: min(760px, calc(100dvh - 48px)); }
.skill-detail-layout { min-height: 0; display: grid; grid-template-columns: 205px minmax(0, 1fr); flex: 1; overflow: hidden; border: 1px solid var(--border-faint); border-radius: 17px; background: var(--bg-glass-subtle); }
.skill-detail-sidebar { min-width: 0; display: grid; align-content: start; gap: 15px; overflow-y: auto; padding: 16px; border-right: 1px solid var(--border-faint); }
.skill-detail-summary { display: grid; gap: 4px; }
.skill-detail-summary > span { overflow-wrap: anywhere; color: var(--accent); font-family: var(--font-mono); font-size: 10px; }
.skill-detail-summary > strong { color: var(--text-primary); font-size: 16px; }
.skill-detail-summary > p { color: var(--text-muted); font-size: 11px; line-height: 1.55; }
.skill-detail-access, .skill-detail-versions { display: grid; gap: 8px; }
.skill-detail-access h4, .skill-detail-versions h4 { margin: 0; color: var(--text-faint); font-size: 10px; letter-spacing: .06em; text-transform: uppercase; }
.skill-detail-access > div { display: flex; flex-wrap: wrap; gap: 5px; }
.skill-detail-access > div span { padding: 4px 7px; border-radius: 999px; background: var(--accent-light); color: var(--accent); font-size: 10px; }
.skill-detail-access small { overflow-wrap: anywhere; color: var(--text-faint); font-size: 10px; line-height: 1.5; }
.skill-detail-versions button { min-width: 0; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 4px 8px; min-height: 40px; padding: 6px 10px; border: 1px solid transparent; border-radius: 10px; background: transparent; color: var(--text-muted); font: inherit; font-size: 11px; cursor: pointer; }
.skill-detail-versions button > span { min-width: 0; display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
.skill-detail-versions button em { padding: 2px 5px; border-radius: 999px; background: var(--bg-soft); color: var(--text-faint); font-size: 9px; font-style: normal; }
.skill-detail-versions button em.is-candidate { background: color-mix(in srgb, var(--warning, #d97706) 13%, transparent); color: var(--warning, #b45309); }
.skill-detail-versions button em.is-published { background: var(--accent-light); color: var(--accent); }
.skill-detail-versions button:hover, .skill-detail-versions button.active { border-color: var(--accent-border); background: var(--accent-light); color: var(--accent); }
.skill-detail-versions small { color: var(--text-faint); font-size: 10px; }
.skill-detail-content { min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow-y: auto; }
.skill-file-tabs { display: flex; gap: 5px; overflow-x: auto; padding: 10px 12px; border-bottom: 1px solid var(--border-faint); }
.skill-file-tabs button { min-height: 36px; padding: 0 9px; border: 1px solid var(--border-faint); border-radius: 9px; background: transparent; color: var(--text-muted); font: inherit; font-family: var(--font-mono); font-size: 10px; white-space: nowrap; cursor: pointer; }
.skill-file-tabs button.active { border-color: var(--accent-border); background: var(--accent-light); color: var(--accent); }
.skill-detail-content > header { min-width: 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 14px; border-bottom: 1px solid var(--border-faint); }
.skill-detail-content > header div { min-width: 0; display: grid; gap: 2px; }
.skill-detail-content > header strong { color: var(--text-primary); font-size: 12px; }
.skill-detail-content > header span, .skill-detail-content > header code { overflow: hidden; color: var(--text-faint); font-family: var(--font-mono); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.skill-version-audit { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; padding: 11px 14px; border-bottom: 1px solid var(--border-faint); background: color-mix(in srgb, var(--bg-base) 42%, transparent); }
.skill-version-audit section:last-child:nth-child(odd) { grid-column: 1 / -1; }
.skill-version-audit section { min-width: 0; display: grid; align-content: start; gap: 4px; padding: 10px; border: 1px solid var(--border-faint); border-radius: 12px; }
.skill-version-audit strong { color: var(--text-primary); font-size: 11px; }
.skill-version-audit p, .skill-version-audit small { margin: 0; overflow-wrap: anywhere; color: var(--text-faint); font-size: 10px; line-height: 1.55; }
.skill-version-audit a { overflow: hidden; color: var(--accent); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
.skill-eval-score { display: flex; align-items: baseline; gap: 3px; color: var(--text-muted); }
.skill-eval-score b { color: var(--text-primary); font-size: 18px; }
.skill-eval-score span { font-size: 10px; }
.skill-eval-score em { margin-left: auto; padding: 3px 6px; border-radius: 999px; background: color-mix(in srgb, var(--danger, #dc2626) 12%, transparent); color: var(--danger, #dc2626); font-size: 10px; font-style: normal; }
.skill-eval-score.passed em { background: var(--accent-light); color: var(--accent); }
.skill-benchmark-running { display: flex; align-items: center; gap: 6px; color: var(--accent); font-size: 10px; line-height: 1.5; }
.skill-benchmark-error { color: var(--danger, #dc2626) !important; }
.skill-benchmark-details summary { color: var(--accent); font-size: 11px; cursor: pointer; }
.skill-benchmark-details > div { max-height: 180px; margin-top: 7px; overflow: auto; display: grid; gap: 6px; }
.skill-benchmark-details article { display: grid; gap: 3px; padding: 7px; border-radius: 8px; background: var(--bg-soft); }
.skill-benchmark-details article header { display: flex; align-items: center; justify-content: space-between; gap: 6px; }
.skill-benchmark-details article header span { overflow-wrap: anywhere; color: var(--text-secondary); font-size: 10px; font-weight: 650; }
.skill-benchmark-details article header em { color: var(--danger, #dc2626); font-size: 10px; font-style: normal; }
.skill-benchmark-details article header em.passed { color: var(--accent); }
.skill-benchmark-details article p, .skill-benchmark-details article small { overflow-wrap: anywhere; color: var(--text-faint); font-size: 10px; line-height: 1.45; }
.skill-detail-content > pre { min-height: 220px; max-height: min(48dvh, 420px); margin: 0; overflow: auto; padding: 18px; background: color-mix(in srgb, var(--bg-base) 56%, transparent); color: var(--text-secondary); font-family: var(--font-mono); font-size: 11px; line-height: 1.7; white-space: pre-wrap; overflow-wrap: anywhere; }
@media (max-width: 1200px) {
  .settings-layout { display: block; }
  .settings-nav { display: flex; gap: 5px; margin-bottom: 12px; overflow-x: auto; scrollbar-width: none; }
  .settings-nav::-webkit-scrollbar { display: none; }
  .settings-nav button { min-height: 50px; flex: none; white-space: nowrap; }
  .settings-nav small { display: none; }
}
@container (max-width: 740px) {
  .settings-section-head.split { display: grid; align-items: start; }
  .usage-filters { width: 100%; grid-template-columns: minmax(0, 1fr) minmax(145px, 190px); }
  .wiki-export-card { align-items: stretch; flex-direction: column; }
  .wiki-export-actions { width: 100%; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .wiki-export-actions .knowledge-select { width: 100%; grid-column: 1 / -1; }
  .wiki-export-actions a .el-button { width: 100%; margin: 0; }
  .skills-actions { width: 100%; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .skills-actions .knowledge-select { width: 100%; grid-column: 1 / -1; }
  .skills-actions .el-button { min-width: 0; width: 100%; margin: 0; }
  .skills-grid { grid-template-columns: 1fr; }
  .skill-card footer { flex-wrap: wrap; }
}
@container (max-width: 520px) {
  .usage-filters { grid-template-columns: 1fr; }
  .wiki-export-actions { grid-template-columns: 1fr; }
  .wiki-export-actions .knowledge-select { grid-column: auto; }
  .runtime-actions { align-items: stretch; flex-direction: column; gap: 12px; }
  .runtime-actions > div { display: grid; grid-template-columns: 1fr; gap: 7px; }
  .runtime-actions .el-button { min-width: 0; margin: 0; white-space: normal; }
  .provider-fields { grid-template-columns: 1fr; }
  .provider-fields .is-wide { grid-column: auto; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .skill-card footer .knowledge-select { width: 100%; }
  .skill-card-actions { width: 100%; justify-content: flex-end; }
}
@media (max-width: 768px) {
  .settings-layout { display: block; }
  .settings-nav { display: flex; margin-bottom: 10px; overflow-x: auto; }
  .settings-nav button { min-height: 48px; justify-content: center; padding: 8px 12px; }
  .settings-nav button span { display: grid; }
  .settings-nav strong { font-size: 12px; }
  .settings-main { padding: 16px; }
  .settings-section-head.split { display: grid; }
  .runtime-title { flex-wrap: wrap; }
  .runtime-title .knowledge-status { margin-left: 55px; }
  .runtime-actions { align-items: stretch; flex-direction: column; gap: 12px; }
  .runtime-actions > div { display: grid; grid-template-columns: 1fr; gap: 7px; }
  .runtime-actions .el-button { min-width: 0; margin: 0; white-space: normal; }
  .provider-fields { grid-template-columns: 1fr; }
  .provider-fields .is-wide { grid-column: auto; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .usage-filters { width: 100%; grid-template-columns: 1fr; }
  .usage-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .usage-bars { height: 190px; }
  .usage-chart-card, .usage-callers { padding: 13px; }
  .protection-actions { width: 100%; display: grid; grid-template-columns: 1fr 1fr; }
  .wiki-export-card { align-items: stretch; flex-direction: column; }
  .wiki-export-actions { width: 100%; grid-template-columns: 1fr; }
  .wiki-export-actions .knowledge-select { grid-column: auto; }
  .skill-version-audit { grid-template-columns: 1fr; }
  .wiki-export-actions a .el-button { width: 100%; }
  .backup-card { align-items: flex-start; flex-wrap: wrap; }
  .backup-copy { width: calc(100% - 58px); flex: auto; }
  .backup-card-actions { width: 100%; justify-content: flex-end; border-top: 1px solid var(--border-faint); padding-top: 7px; }
  .backup-card:hover { transform: none; }
  .skills-actions { width: 100%; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .skills-actions .knowledge-select { min-width: 0; grid-column: 1 / -1; }
  .skills-actions .el-button { min-width: 0; width: 100%; margin: 0; white-space: normal; }
  .skills-grid { grid-template-columns: 1fr; }
  .skill-card:hover { transform: none; }
  .skill-detail-layout { min-height: 0; display: block; overflow-y: auto; }
  .skill-detail-modal { height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); }
  .skill-detail-sidebar { overflow: visible; border-right: 0; border-bottom: 1px solid var(--border-faint); }
  .skill-detail-versions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .skill-detail-versions h4 { grid-column: 1 / -1; }
  .skill-detail-content { overflow: visible; }
  .skill-detail-content > pre { max-height: 38dvh; }
}
@media (prefers-reduced-motion: reduce) {
  .usage-bar-column > i { animation: none; }
}
</style>
