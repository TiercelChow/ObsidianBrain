<template>
  <KnowledgePageShell title="Wiki 配置" subtitle="集中管理 Agent Runtime、每本书的规则文档与能力边界">
    <div class="settings-layout">
      <el-select v-model="section" class="mobile-settings-section knowledge-select is-fluid" aria-label="配置分区" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true">
        <el-option label="Agent Runtime · 执行器与模型" value="runtime" />
        <el-option label="模型供应商 · 路由与 API Key" value="providers" />
        <el-option label="Token 用量 · 调用趋势" value="usage" />
        <el-option label="数据保护 · 备份与恢复" value="protection" />
        <el-option label="配置文档 · 提示词与规则" value="documents" />
        <el-option label="Skills · 可配置能力" value="skills" />
      </el-select>
      <aside class="settings-nav knowledge-surface" data-glass="structural">
        <button :class="{ active: section === 'runtime' }" @click="section = 'runtime'"><el-icon><Cpu /></el-icon><span><strong>Agent Runtime</strong><small>执行器与模型</small></span></button>
        <button :class="{ active: section === 'providers' }" @click="section = 'providers'"><el-icon><Connection /></el-icon><span><strong>模型供应商</strong><small>路由与 API Key</small></span></button>
        <button :class="{ active: section === 'usage' }" @click="section = 'usage'"><el-icon><DataAnalysis /></el-icon><span><strong>Token 用量</strong><small>调用趋势与来源</small></span></button>
        <button :class="{ active: section === 'protection' }" @click="section = 'protection'"><el-icon><Lock /></el-icon><span><strong>数据保护</strong><small>备份、下载与恢复</small></span></button>
        <button :class="{ active: section === 'documents' }" @click="section = 'documents'"><el-icon><Document /></el-icon><span><strong>配置文档</strong><small>数据库中的 Markdown</small></span></button>
        <button :class="{ active: section === 'skills' }" @click="section = 'skills'"><el-icon><MagicStick /></el-icon><span><strong>Skills</strong><small>可配置能力</small></span></button>
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
              <el-select
                v-model="item.profile.provider_id"
                class="knowledge-select is-fluid"
                popper-class="system-select-popper"
                placement="bottom-start"
                :offset="0"
                :fit-input-width="true"
                aria-label="第三方模型供应商"
                :title="selectedProvider(item.profile) ? `${selectedProvider(item.profile)!.display_name} · ${selectedProvider(item.profile)!.model}` : '不使用，走 Harness 默认'"
                placeholder="不使用，走 Harness 默认"
                clearable
                @change="delete runtimeVerification[item.profile.id]"
              >
                <el-option v-for="provider in enabledProviders" :key="provider.provider_id" :label="`${provider.display_name} · ${provider.model}`" :value="provider.provider_id" />
              </el-select>
            </div>
            <label><span>模型覆盖（可选）</span><el-input v-model="item.profile.model" :placeholder="item.profile.provider_id ? '留空则使用所选供应商的模型' : '留空则使用 Harness 默认模型'" /></label>
            <p class="credential-hint">供应商与 API Key 在「模型供应商」分区集中管理。<template v-if="selectedProvider(item.profile)">{{ selectedProvider(item.profile)!.display_name }}：<code>{{ selectedProvider(item.profile)!.api_key_configured ? '已配置密钥' : '未配置密钥' }}</code></template><template v-else>未选择供应商时，凭据由 Harness 默认 Profile 管理。</template></p>
            <div class="runtime-actions">
              <el-switch v-model="item.profile.enabled" active-text="启用" />
              <div>
                <el-button :loading="verifyingRuntimeId === item.profile.id" :disabled="!item.profile.enabled || savingRuntime" @click="verifyRuntime(item.profile)">验证已保存配置</el-button>
                <el-button type="primary" :loading="savingRuntime" @click="saveRuntime(item.profile)">保存</el-button>
              </div>
            </div>
          </article>
        </template>

        <template v-else-if="section === 'providers'">
          <header class="settings-section-head split">
            <div><span>模型路由</span><h2>模型供应商</h2><p>集中管理第三方模型供应商；keychain 模式的 API Key 写入系统凭据库，不落盘。environment 模式仍由启动进程的环境变量提供。</p></div>
            <el-button type="primary" :icon="Plus" @click="openNewProvider">新增供应商</el-button>
          </header>
          <div v-if="loading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
          <article v-else v-for="provider in modelProviders" :key="provider.provider_id" class="provider-card">
            <div class="provider-head">
              <div><h3>{{ provider.display_name }}</h3><p>{{ provider.provider_id }} · {{ provider.model }}</p></div>
              <span class="knowledge-status" :class="provider.api_key_configured ? 'is-healthy' : 'is-warning'">{{ credentialStatusLabel(provider) }}</span>
            </div>
            <div class="provider-meta">
              <span>协议：{{ protocolLabel(provider.api_protocol) }}</span>
              <span>Base URL：{{ provider.base_url }}</span>
              <span>凭据来源：{{ provider.credential_source === 'keychain' ? '系统凭据库' : `环境变量 ${provider.api_key_env || '—'}` }}</span>
              <span>上下文：{{ provider.context_window == null ? '未声明' : `${formatTokenCount(provider.context_window)} tokens` }}</span>
              <span>最大输出：{{ provider.max_output_tokens == null ? '未声明' : `${formatTokenCount(provider.max_output_tokens)} tokens` }}</span>
              <span>推理：{{ reasoningPolicyLabel(provider.reasoning_policy || 'auto') }}</span>
            </div>
            <div class="runtime-actions">
              <el-switch v-model="provider.enabled" active-text="启用" @change="toggleProviderEnabled(provider)" />
              <div>
                <el-button :loading="updatingProviderId === provider.provider_id" @click="openEditProvider(provider)">编辑</el-button>
                <el-button type="danger" :loading="deletingProviderId === provider.provider_id" @click="removeProvider(provider)">删除</el-button>
              </div>
            </div>
          </article>
          <p v-if="!loading && !modelProviders.length" class="settings-empty">尚未配置任何模型供应商，点击右上角「新增供应商」开始。</p>
        </template>

        <template v-else-if="section === 'usage'">
          <header class="settings-section-head split usage-head">
            <div><span>运行可观测性</span><h2>Token 用量</h2><p>按时间与调用方查看输入、输出和调用趋势。</p></div>
            <div class="usage-filters">
              <el-date-picker v-model="usageDateRange" type="daterange" value-format="YYYY-MM-DD" format="YYYY/MM/DD" range-separator="至" start-placeholder="开始日期" end-placeholder="结束日期" :clearable="false" unlink-panels popper-class="glass-picker wiki-usage-picker" @change="loadUsage" />
              <el-select v-model="usageCaller" class="knowledge-select is-fluid" placeholder="全部调用方" aria-label="Token 用量调用方" popper-class="system-select-popper system-toolbar-popper" placement="bottom-start" :offset="0" :fit-input-width="true" @change="loadUsage">
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
              <span><strong>{{ usageSourceLabel }}</strong>当前 Harness ACP 未上报精确 token，估算仅覆盖初始 Prompt 与最终输出，不包含工具历史、模型内部多轮重发、压缩和推理消耗，不能作为供应商账单。上下文占用与 ACP 费用独立记录，不换算为计费 token。</span>
            </div>
            <div class="usage-metrics">
              <article><span>已记录 Token</span><strong>{{ formatTokenCount(usageStats?.totals.total_tokens || 0) }}</strong><small>{{ usageStats?.totals.runs || 0 }} 次有记录调用，非完整账单</small></article>
              <article><span>输入</span><strong>{{ formatTokenCount(usageStats?.totals.input_tokens || 0) }}</strong><small>已记录的提示词与可见证据</small></article>
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
              <input ref="backupUploadInput" hidden type="file" accept=".sqlite,.sqlite3,.db,application/vnd.sqlite3" @change="selectRestoreUpload" />
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
            <div><span>能力扩展</span><h2>Skills</h2><p>指令保存在 SQLite；当前开发阶段仅保留最新内容，启用范围严格绑定当前书籍。</p></div>
            <div class="skills-actions">
              <el-select v-model="activeBaseId" class="knowledge-select is-compact is-responsive" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true" placeholder="选择知识库" @change="loadSkills"><el-option v-for="base in bases" :key="base.id" :label="base.book_name" :value="base.id" /></el-select>
              <input ref="skillArchiveInput" hidden type="file" accept=".zip,application/zip" @change="importSkillArchive" />
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
        <div class="knowledge-modal-actions"><el-button @click="skillEditorVisible = false">取消</el-button><el-button type="primary" :loading="savingSkill" :disabled="!skillDraft.name.trim() || !skillDraft.slug.trim() || !skillDraft.instructions.trim()" @click="saveSkill">保存修改</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="providerEditorVisible" aria-label="编辑模型供应商">
      <div class="knowledge-modal-card">
        <div class="knowledge-modal-head">
          <div><h3>{{ providerDraft.provider_id ? '编辑供应商' : '新增供应商' }}</h3><p>keychain 模式：API Key 写入系统凭据库，数据库只存配置元数据。environment 模式：启动 ObsidianBrain 前设置好对应环境变量。</p></div>
          <span v-if="providerDraft.api_key_configured" class="knowledge-status is-healthy">已配置密钥</span>
        </div>
        <div class="knowledge-modal-body provider-editor-form">
          <el-input v-model="providerDraft.display_name" :maxlength="100" placeholder="供应商名称，例如：阿里云百炼" />
          <el-input v-model="providerDraft.provider_id" :disabled="Boolean(providerDraft.provider_id)" :maxlength="64" placeholder="供应商 ID，留空自动生成，例如：aliyun-bailian" />
          <el-select v-model="providerDraft.api_protocol" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true">
            <el-option label="OpenAI Chat Completions" value="openai-completions" />
            <el-option label="OpenAI Responses" value="openai-responses" />
            <el-option label="Anthropic Messages" value="anthropic-messages" />
          </el-select>
          <el-input v-model="providerDraft.base_url" placeholder="API Base URL，例如：https://dashscope.aliyuncs.com/compatible-mode/v1" />
          <el-input v-model="providerDraft.model" placeholder="模型 ID，例如：glm-5.2" />
          <section class="provider-capabilities">
            <header><strong>模型能力</strong><span>按供应商实际限制填写；未知时留空，不推测容量。</span></header>
            <label><span>上下文容量（tokens）</span><el-input v-model="contextWindowInput" inputmode="numeric" placeholder="留空使用 1M 应用默认值" /></label>
            <label><span>最大输出（tokens）</span><el-input v-model="maxOutputInput" inputmode="numeric" placeholder="留空按阶段动态分配" /></label>
            <label class="is-wide"><span>推理策略</span><el-select v-model="providerDraft.reasoning_policy" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true"><el-option v-for="policy in modelReasoningPolicies" :key="policy" :label="reasoningPolicyLabel(policy)" :value="policy" /></el-select></label>
            <p v-if="capabilityError" class="capability-error is-wide" role="alert">{{ capabilityError }}</p>
            <p v-else class="is-wide">未声明上下文时，问答与研究使用 1M 应用默认值，Runtime 上报容量后按实际值校验；不表示所有模型都支持 1M。最大输出与上下文不同，研究按正文、JSON 和推理分别预留，截断后仅扩容当前阶段；填写的供应商上限始终优先。</p>
          </section>
          <el-select v-model="providerDraft.credential_source" class="knowledge-select is-fluid" popper-class="system-select-popper" placement="bottom-start" :offset="0" :fit-input-width="true">
            <el-option label="系统凭据库（页面填写 API Key）" value="keychain" />
            <el-option label="环境变量（启动进程注入）" value="environment" />
          </el-select>
          <el-input v-if="providerDraft.credential_source === 'environment'" v-model="providerDraft.api_key_env" placeholder="API Key 环境变量，例如：CUSTOM_LLM_API_KEY" />
          <template v-else>
            <el-input v-model="providerDraft.api_key" type="password" show-password :placeholder="providerDraft.api_key_configured ? '留空保留现有密钥' : '粘贴 API Key，保存后写入系统凭据库'" />
            <el-checkbox v-if="providerDraft.api_key_configured" v-model="providerDraft.clear_api_key">保存时清除已配置的密钥</el-checkbox>
          </template>
          <el-switch v-model="providerDraft.enabled" active-text="启用" />
        </div>
        <div class="knowledge-modal-actions"><el-button @click="providerEditorVisible = false">取消</el-button><el-button type="primary" :loading="savingProvider" :disabled="!providerDraft.display_name.trim() || !providerDraft.base_url.trim() || !providerDraft.model.trim() || Boolean(capabilityError)" @click="saveProviderDraft">保存供应商</el-button></div>
      </div>
    </MotionModal>

    <MotionModal v-model="skillDetailVisible" aria-label="Skill 内容" size="wide">
      <div class="knowledge-modal-card skill-detail-modal">
        <div class="knowledge-modal-head">
          <div><h3>Skill 内容</h3><p>这里展示当前指令、资源与权限声明；是否实际注入以运行检查器为准。</p></div>
          <span v-if="skillDetail" class="knowledge-status" :class="skillDetail.skill.source_type === 'builtin' ? 'is-healthy' : 'is-draft'">{{ skillDetail.skill.source_type === 'builtin' ? '系统内置' : '自定义' }}</span>
        </div>
        <div v-if="skillDetailLoading" class="settings-loading"><el-icon class="is-loading"><Loading /></el-icon></div>
        <button v-if="skillDetail && !skillDetailLoading" class="mobile-skill-metadata-toggle" type="button" :aria-expanded="skillMetadataExpanded" @click="skillMetadataExpanded = !skillMetadataExpanded">{{ skillMetadataExpanded ? '收起权限与版本说明' : '权限、版本与来源说明' }}<el-icon><ArrowDown /></el-icon></button>
        <div v-if="skillDetail && !skillDetailLoading" class="skill-detail-layout" :class="{ 'show-metadata': skillMetadataExpanded }">
          <aside class="skill-detail-sidebar" data-glass="structural">
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
              <h4>当前内容</h4>
              <button v-for="version in skillDetail.versions" :key="version.id" type="button" :class="{ active: version.id === activeSkillVersionId }" @click="selectSkillVersion(version.id)">
                <span>Revision {{ version.revision }}<em :class="`is-${version.release_state}`">{{ skillReleaseLabel(version.release_state) }}</em></span><small>{{ formatSkillDate(version.created_at) }}</small>
              </button>
            </section>
          </aside>
          <section class="skill-detail-content">
            <nav class="skill-file-tabs" aria-label="Skill 文件">
              <button v-for="file in activeSkillVersion?.files || []" :key="file.relative_path" type="button" class="skill-file-button" data-glass-action :class="{ active: file.relative_path === activeSkillFilePath }" :aria-pressed="file.relative_path === activeSkillFilePath" :title="file.relative_path" @click="activeSkillFilePath = file.relative_path"><el-icon><Document /></el-icon><span>{{ file.relative_path }}</span></button>
            </nav>
            <header v-if="activeSkillFile"><div><strong :title="activeSkillFile.relative_path">{{ activeSkillFile.relative_path }}</strong><span>{{ activeSkillFile.media_type }} · {{ formatBytes(activeSkillFile.size_bytes) }}</span></div><code :title="activeSkillFile.content_hash">{{ activeSkillFile.content_hash }}</code></header>
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
import { ArrowDown, Connection, Cpu, DataAnalysis, Document, Download, InfoFilled, Loading, Lock, MagicStick, Plus, UploadFilled } from '@element-plus/icons-vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import KnowledgePageShell from '@/components/knowledge/KnowledgePageShell.vue'
import { modelReasoningPolicies, validateModelCapabilities } from '@/utils/knowledgeRuntimePolicy'
import {
  getBookWikiSettings,
  getAgentUsageStats,
  getWikiSkillBenchmark,
  bookWikiExportDownloadUrl,
  createKnowledgeBackup,
  deleteModelProvider,
  evaluateWikiSkillVersion,
  getWikiSkillDetail,
  importWikiSkillArchive,
  knowledgeBackupDownloadUrl,
  listBookKnowledgeBases,
  listKnowledgeBackups,
  listWikiSkills,
  restoreKnowledgeBackup,
  saveAgentRuntimeProfile,
  saveBookWikiConfigDocument,
  saveCustomWikiSkill,
  saveModelProvider,
  startWikiSkillBenchmark,
  publishWikiSkillVersion,
  setWikiSkillBinding,
  uploadAndRestoreKnowledgeBackup,
  verifyAgentRuntime,
  type ConfigDocument,
  type DatabaseBackup,
  type AgentUsageStats,
  type KnowledgeBaseSummary,
  type ModelProviderProfile,
  type RuntimeHealth,
  type RuntimeProfile,
  type SaveModelProviderRequest,
  type WikiSkill,
  type WikiSkillDetail,
  type WikiSkillBenchmarkRun,
} from '@/api/knowledge'

const section = ref<'runtime' | 'providers' | 'usage' | 'protection' | 'documents' | 'skills'>('runtime')
const bases = ref<KnowledgeBaseSummary[]>([])
const activeBaseId = ref('')
const skillMetadataExpanded = ref(false)
const documents = ref<ConfigDocument[]>([])
const activeDocumentId = ref('')
const runtimeHealth = ref<RuntimeHealth[]>([])
const modelProviders = ref<ModelProviderProfile[]>([])
const providerEditorVisible = ref(false)
const updatingProviderId = ref('')
const savingProvider = ref(false)
const deletingProviderId = ref('')
const providerDraft = reactive<SaveModelProviderRequest & { api_key_configured: boolean }>({
  provider_id: '',
  display_name: '',
  api_protocol: 'openai-completions',
  base_url: '',
  model: '',
  context_window: null,
  max_output_tokens: null,
  reasoning_policy: 'auto',
  credential_source: 'keychain',
  api_key_env: '',
  api_key: '',
  clear_api_key: false,
  enabled: true,
  expected_revision: 0,
  api_key_configured: false,
})
const contextWindowInput = computed({
  get: () => providerDraft.context_window == null ? '' : String(providerDraft.context_window),
  set: (value: string) => { providerDraft.context_window = value.trim() ? Number(value) : null },
})
const maxOutputInput = computed({
  get: () => providerDraft.max_output_tokens == null ? '' : String(providerDraft.max_output_tokens),
  set: (value: string) => { providerDraft.max_output_tokens = value.trim() ? Number(value) : null },
})
const capabilityError = computed(() => validateModelCapabilities(providerDraft))
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
const enabledProviders = computed(() => modelProviders.value.filter(provider => provider.enabled))
function selectedProvider(profile: RuntimeProfile): ModelProviderProfile | null {
  const id = profile.provider_id
  if (!id) return null
  return modelProviders.value.find(provider => provider.provider_id === id) || null
}
function credentialStatusLabel(provider: ModelProviderProfile): string {
  if (provider.credential_source === 'environment') {
    return provider.api_key_env ? '环境变量' : '未配置变量'
  }
  return provider.api_key_configured ? '已配置密钥' : '未配置密钥'
}
function protocolLabel(protocol: ModelProviderProfile['api_protocol']): string {
  return protocol === 'openai-completions'
    ? 'OpenAI Completions'
    : protocol === 'openai-responses'
      ? 'OpenAI Responses'
      : 'Anthropic Messages'
}
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

function resetProviderDraft() {
  Object.assign(providerDraft, {
    provider_id: '',
    display_name: '',
    api_protocol: 'openai-completions',
    base_url: '',
    model: '',
    context_window: null,
    max_output_tokens: null,
    reasoning_policy: 'auto',
    credential_source: 'keychain',
    api_key_env: '',
    api_key: '',
    clear_api_key: false,
    enabled: true,
    expected_revision: 0,
    api_key_configured: false,
  })
}

function openNewProvider() {
  resetProviderDraft()
  providerEditorVisible.value = true
}

function openEditProvider(provider: ModelProviderProfile) {
  Object.assign(providerDraft, {
    provider_id: provider.provider_id,
    display_name: provider.display_name,
    api_protocol: provider.api_protocol,
    base_url: provider.base_url,
    model: provider.model,
    context_window: provider.context_window ?? null,
    max_output_tokens: provider.max_output_tokens ?? null,
    reasoning_policy: provider.reasoning_policy || 'auto',
    credential_source: provider.credential_source,
    api_key_env: provider.api_key_env,
    api_key: '',
    clear_api_key: false,
    enabled: provider.enabled,
    expected_revision: provider.revision,
    api_key_configured: provider.api_key_configured,
  })
  providerEditorVisible.value = true
}

async function saveProviderDraft() {
  savingProvider.value = true
  try {
    const invalidCapabilities = validateModelCapabilities(providerDraft)
    if (invalidCapabilities) throw new Error(invalidCapabilities)
    const response = await saveModelProvider({
      provider_id: providerDraft.provider_id || undefined,
      display_name: providerDraft.display_name.trim(),
      api_protocol: providerDraft.api_protocol,
      base_url: providerDraft.base_url.trim(),
      model: providerDraft.model.trim(),
      context_window: providerDraft.context_window ?? null,
      max_output_tokens: providerDraft.max_output_tokens ?? null,
      reasoning_policy: providerDraft.reasoning_policy || 'auto',
      credential_source: providerDraft.credential_source,
      api_key_env: providerDraft.api_key_env?.trim() ?? '',
      api_key: providerDraft.api_key || undefined,
      clear_api_key: providerDraft.clear_api_key,
      enabled: providerDraft.enabled,
      expected_revision: providerDraft.expected_revision ?? 0,
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '供应商保存失败')
    providerEditorVisible.value = false
    ElMessage.success('供应商已保存')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    savingProvider.value = false
  }
}

async function removeProvider(provider: ModelProviderProfile) {
  deletingProviderId.value = provider.provider_id
  try {
    const response = await deleteModelProvider(provider.provider_id, provider.revision)
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '供应商删除失败')
    ElMessage.success('供应商已删除')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
  } finally {
    deletingProviderId.value = ''
  }
}

async function toggleProviderEnabled(provider: ModelProviderProfile) {
  // optimistic inline toggle persisted immediately so disabled providers cannot stay selected by a runtime
  updatingProviderId.value = provider.provider_id
  try {
    const response = await saveModelProvider({
      provider_id: provider.provider_id,
      display_name: provider.display_name,
      api_protocol: provider.api_protocol,
      base_url: provider.base_url,
      model: provider.model,
      context_window: provider.context_window ?? null,
      max_output_tokens: provider.max_output_tokens ?? null,
      reasoning_policy: provider.reasoning_policy || 'auto',
      credential_source: provider.credential_source,
      api_key_env: provider.api_key_env,
      enabled: provider.enabled,
      expected_revision: provider.revision,
    })
    if (response.status !== 'success' || !response.result) throw new Error(response.error?.message || '供应商状态更新失败')
    await loadSettings()
  } catch (error) {
    ElMessage.error((error as Error).message)
    await loadSettings()
  } finally {
    updatingProviderId.value = ''
  }
}

function reasoningPolicyLabel(policy: string) {
  return ({ auto: '继承 Harness', off: '关闭推理', minimal: '最少推理', low: '低', medium: '中', high: '高', xhigh: '极高', max: '最大' } as Record<string, string>)[policy] || policy
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
  skillMetadataExpanded.value = false
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
    ElMessage.success(skillDraft.id ? 'Skill 已更新' : 'Skill 已创建')
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
    modelProviders.value = response.result.model_providers
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
.mobile-settings-section { display: none; }
.mobile-skill-metadata-toggle { display: none; }
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
.settings-section-head.split { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 16px; }
.settings-section-head > div { min-width: 0; max-width: 100%; }
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
.provider-mode { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr); gap: 9px; padding: 11px 13px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); }
.provider-mode > div { min-width: 0; display: grid; gap: 2px; }
.provider-mode .knowledge-select { min-width: 0; width: 100%; max-width: 100%; }
.provider-mode :deep(.el-select__wrapper), .usage-filters :deep(.el-select__wrapper) { min-width: 0; }
.provider-mode :deep(.el-select__selection), .usage-filters :deep(.el-select__selection) { min-width: 0; }
.provider-mode :deep(.el-select__selected-item) { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.provider-mode strong { font-size: 12px; }
.provider-mode span { color: var(--text-faint); font-size: 10px; }
.provider-fields { min-width: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; padding: 14px; border: 1px solid var(--accent-border); border-radius: 14px; background: var(--accent-light); }
.provider-fields label { min-width: 0; }
.provider-fields .is-wide { grid-column: 1 / -1; }
.credential-hint { color: var(--text-faint); font-size: 10px; line-height: 1.55; }
.credential-hint code { color: var(--text-muted); font-family: var(--font-mono); }
.provider-card { min-width: 0; display: grid; gap: 13px; padding: 18px; border: 1px solid var(--border-faint); border-radius: 17px; background: var(--bg-glass-subtle); }
.provider-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.provider-head h3 { margin: 0; font-size: 15px; font-weight: 660; }
.provider-head p { margin: 3px 0 0; color: var(--text-muted); font-size: 11px; font-family: var(--font-mono); }
.provider-meta { display: flex; flex-wrap: wrap; gap: 6px 18px; color: var(--text-faint); font-size: 11px; }
.settings-empty { color: var(--text-faint); font-size: 12px; padding: 28px 4px; }
.provider-editor-form { display: grid; gap: 11px; }
.provider-editor-form .el-select { width: 100%; }
.provider-capabilities { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px 12px; padding: 13px; border: 1px solid var(--border-faint); border-radius: 14px; background: var(--bg-glass-subtle); }
.provider-capabilities header { grid-column: 1 / -1; display: grid; gap: 3px; }
.provider-capabilities strong { font-size: 12px; font-weight: 650; }
.provider-capabilities header span, .provider-capabilities p { color: var(--text-faint); font-size: 11px; line-height: 1.6; }
.provider-capabilities label { min-width: 0; display: grid; gap: 5px; }
.provider-capabilities label > span { color: var(--text-muted); font-size: 11px; }
.provider-capabilities .is-wide { grid-column: 1 / -1; }
.provider-capabilities .capability-error { color: var(--danger, #d9342b); }
@media (max-width: 560px) { .provider-capabilities { grid-template-columns: minmax(0, 1fr); } }
.runtime-actions { display: flex; align-items: center; justify-content: space-between; }
.runtime-actions > div:not(.el-switch) { display: flex; gap: 8px; }
.usage-head { align-items: flex-start !important; }
.usage-filters { --usage-control-height: 40px; min-width: 0; flex: 0 1 520px; width: min(100%, 520px); display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 170px); gap: 8px; }
.usage-filters .knowledge-select { min-width: 0; width: 100%; max-width: 100%; }
.usage-filters :deep(.el-select__wrapper), .usage-filters :deep(.el-date-editor) { min-height: var(--usage-control-height); }
.usage-filters :deep(.el-date-editor) { min-width: 0; width: 100%; max-width: 100%; border-radius: 12px; background: var(--bg-glass-subtle); box-shadow: inset 0 0 0 1px var(--border-faint); }
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
.skill-file-tabs { min-width: 0; flex: none; display: flex; flex-wrap: wrap; gap: 7px; padding: 12px; border-bottom: 1px solid var(--border-faint); }
.skill-file-tabs button { min-width: 0; max-width: 100%; min-height: 38px; display: inline-flex; align-items: center; gap: 7px; padding: 8px 11px; border: 1px solid var(--border-faint); border-radius: 11px; background: transparent; color: var(--text-muted); font: inherit; font-family: var(--font-mono); font-size: 11px; cursor: pointer; transition: background var(--motion-fast), border-color var(--motion-fast); }
.skill-file-tabs button .el-icon { flex: none; font-size: 15px; }
.skill-file-tabs button span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
.skill-file-tabs button:active { transform: scale(.98); }
.skill-detail-content > header { min-width: 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 14px; border-bottom: 1px solid var(--border-faint); }
.skill-detail-content > header div { min-width: 0; flex: 1; display: grid; gap: 2px; }
.skill-detail-content > header strong { overflow: hidden; color: var(--text-primary); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.skill-detail-content > header code { flex: none; max-width: 12ch; }
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
  .runtime-actions > div:not(.el-switch) { display: grid; grid-template-columns: 1fr; gap: 7px; }
  .runtime-actions .el-button { min-width: 0; margin: 0; white-space: normal; }
  .provider-fields { grid-template-columns: 1fr; }
  .provider-fields .is-wide { grid-column: auto; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .skill-card footer .knowledge-select { width: 100%; }
  .skill-card-actions { width: 100%; justify-content: flex-end; }
}
@media (max-width: 768px) {
  .settings-layout { display: block; }
  .settings-nav { display: none; }
  .mobile-settings-section { display: block; margin-bottom: 10px; }
  .settings-nav button { min-height: 48px; justify-content: center; padding: 8px 12px; }
  .settings-nav button span { display: grid; }
  .settings-nav strong { font-size: 12px; }
  .settings-main { padding: 14px; }
  .settings-section-head { margin-bottom: 14px; padding-bottom: 12px; }
  .settings-section-head h2 { font-size: 19px; }
  .settings-section-head p { font-size: 13px; line-height: 1.6; }
  .runtime-card, .provider-card { gap: 12px; padding: 12px; }
  .runtime-title { gap: 8px; }
  .runtime-logo { width: 36px; height: 36px; border-radius: 11px; flex: none; }
  .provider-mode { display: grid; grid-template-columns: minmax(0, 1fr); gap: 9px; padding: 10px; }
  .provider-head { flex-wrap: wrap; gap: 8px; }
  .provider-head > div { min-width: 0; flex: 1 1 100%; }
  .provider-head h3, .provider-head p, .provider-meta span, .credential-hint, .document-meta { overflow-wrap: anywhere; }
  .provider-meta { display: grid; grid-template-columns: minmax(0, 1fr); font-size: 12px; }
  .runtime-actions .el-button, .skills-actions .el-button, .protection-actions .el-button, .skill-card-actions .el-button, .document-actions .el-button { min-height: 44px; height: auto; }
  .document-tabs { max-width: 100%; flex-wrap: nowrap; overflow-x: auto; }
  .document-tabs button { flex: none; min-height: 44px; }
  .document-editor textarea { font-size: 16px; padding: 14px; }
  .skill-card { padding: 12px; }
  .skill-card header { gap: 8px; }
  .skill-card header h3, .skill-card header code { overflow-wrap: anywhere; white-space: normal; }
  .settings-section-head.split { display: grid; }
  .runtime-title { flex-wrap: wrap; }
  .runtime-title > div:nth-child(2) { flex: 1 1 calc(100% - 44px); }
  .runtime-title .knowledge-status { margin-left: 44px; }
  .runtime-actions { align-items: stretch; flex-direction: column; gap: 12px; }
  .runtime-actions > div:not(.el-switch) { display: grid; grid-template-columns: 1fr; gap: 7px; }
  .runtime-actions .el-button { min-width: 0; margin: 0; white-space: normal; }
  .provider-fields { grid-template-columns: 1fr; }
  .provider-fields .is-wide { grid-column: auto; }
  .document-actions { align-items: stretch; flex-direction: column; }
  .usage-filters { --usage-control-height: 44px; width: 100%; grid-template-columns: 1fr; }
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
  .mobile-skill-metadata-toggle { flex: none; min-height: 44px; display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 0 16px 10px; padding: 8px 12px; border: 1px solid var(--border-faint); border-radius: 12px; background: var(--bg-glass-subtle); color: var(--accent); font: inherit; font-size: 13px; cursor: pointer; }
  .skill-detail-layout:not(.show-metadata) .skill-detail-sidebar,
  .skill-detail-layout:not(.show-metadata) .skill-version-audit { display: none; }
  .skill-detail-modal { height: calc(min(88dvh, 760px) - env(safe-area-inset-bottom)); }
  .skill-detail-sidebar { overflow: visible; border-right: 0; border-bottom: 1px solid var(--border-faint); }
  .skill-detail-versions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .skill-detail-versions h4 { grid-column: 1 / -1; }
  .skill-detail-content { overflow: visible; }
  .skill-detail-content > pre { min-height: 0; max-height: none; padding: 14px; font-size: 13px; }
  .skill-file-tabs { flex-wrap: wrap; }
  .skill-file-tabs button { min-height: 44px; }
  .skill-detail-content > header { flex-wrap: wrap; gap: 6px; }
  .skill-detail-content > header > div { min-width: 0; }
}
@media (prefers-reduced-motion: reduce) {
  .usage-bar-column > i { animation: none; }
}
</style>
