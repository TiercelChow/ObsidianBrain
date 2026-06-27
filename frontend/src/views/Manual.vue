<template>
  <div class="manual-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">使用手册</h1>
        <p class="page-subtitle">ObsidianBrain 知识引擎使用指南</p>
      </div>
    </header>

    <div class="manual-layout">
      <!-- 左侧目录 -->
      <aside class="toc-sidebar">
        <div class="toc-inner">
          <div class="toc-header">📋 目录</div>
          <a
            v-for="s in sections"
            :key="s.id"
            :href="'#' + s.id"
            class="toc-link"
            :class="{ active: activeSection === s.id }"
            @click.prevent="scrollTo(s.id)"
          >
            <span class="toc-num">{{ s.num }}</span>
            <span class="toc-text">{{ s.title }}</span>
          </a>
        </div>
      </aside>

      <!-- 右侧内容 -->
      <div class="manual-content">
        <!-- 1. 什么是 ObsidianBrain -->
        <section id="intro" class="manual-section">
          <h2>1. 什么是 ObsidianBrain</h2>
          <p>ObsidianBrain 是一个运行在你电脑上的知识助手。它连接你的 Obsidian 笔记本，帮你管理知识的完整生命周期：</p>

          <div class="flow-diagram">
            <div class="flow-step">
              <div class="flow-icon">📥</div>
              <div class="flow-label">获取</div>
              <div class="flow-desc">从外部源抓取<br>或手动录入资料</div>
            </div>
            <div class="flow-arrow">→</div>
            <div class="flow-step">
              <div class="flow-icon">🧠</div>
              <div class="flow-label">整理</div>
              <div class="flow-desc">AI 自动摘要<br>提取概念和实体</div>
            </div>
            <div class="flow-arrow">→</div>
            <div class="flow-step">
              <div class="flow-icon">🔗</div>
              <div class="flow-label">连接</div>
              <div class="flow-desc">交叉引用<br>构建知识图谱</div>
            </div>
            <div class="flow-arrow">→</div>
            <div class="flow-step">
              <div class="flow-icon">💡</div>
              <div class="flow-label">洞察</div>
              <div class="flow-desc">发现缺口<br>生成新问题</div>
            </div>
          </div>
        </section>

        <!-- 2. 时光机 -->
        <section id="timeline" class="manual-section">
          <h2>2. 时光机 — 记录碎片想法</h2>
          <p>类似微信朋友圈，随时记录碎片化的想法。</p>

          <div class="feature-grid">
            <div class="feature-card">
              <div class="feature-icon">✏️</div>
              <h3>写小记</h3>
              <p>点击「写小记」→ 输入内容（支持 Markdown）→ 可选添加标签 → 发布</p>
            </div>
            <div class="feature-card">
              <div class="feature-icon">🔍</div>
              <h3>搜索筛选</h3>
              <p>关键词搜索 + 时间快捷筛选（今天/7天/30天）+ 标签点击搜索</p>
            </div>
            <div class="feature-card">
              <div class="feature-icon">🖼️</div>
              <h3>图片</h3>
              <p>最多 9 张图片，自动压缩缩略图，点击查看大图左右切换</p>
            </div>
            <div class="feature-card">
              <div class="feature-icon">🔄</div>
              <h3>同步</h3>
              <p>在 Obsidian 中改了小记？点「同步」按钮更新系统数据</p>
            </div>
          </div>

          <div class="callout">
            <span class="callout-icon">💡</span>
            <span>小记保存在 Obsidian 的 <code>Timeline/</code> 文件夹，你可以在 Obsidian 中直接查看和编辑。</span>
          </div>
        </section>

        <!-- 3. Wiki 看板 -->
        <section id="wiki-dashboard" class="manual-section">
          <h2>3. Wiki 看板 — 知识库仪表盘</h2>
          <p>Wiki 看板是你的知识库「仪表盘」，一眼掌握全局。</p>

          <div class="dashboard-mock">
            <div class="mock-row">
              <div class="mock-stat">📄 10<br><small>总页数</small></div>
              <div class="mock-stat">👤 4<br><small>实体</small></div>
              <div class="mock-stat">💡 5<br><small>概念</small></div>
              <div class="mock-stat">📚 1<br><small>源摘要</small></div>
              <div class="mock-stat">🔗 0<br><small>综合论述</small></div>
            </div>
            <div class="mock-section">
              <div class="mock-title">🏥 知识健康度</div>
              <div class="mock-bar"><span class="mock-bar-fill" style="width: 20%; background: #ef4444;"></span><span class="mock-bar-label">🔴 孤岛页 2</span></div>
              <div class="mock-bar"><span class="mock-bar-fill" style="width: 10%; background: #3b82f6;"></span><span class="mock-bar-label">🔵 缺失页面 1</span></div>
              <div class="mock-bar"><span class="mock-bar-fill" style="width: 60%; background: #8b5cf6;"></span><span class="mock-bar-label">🕸️ 枢纽 3</span></div>
            </div>
          </div>

          <table class="info-table">
            <thead><tr><th>指标</th><th>含义</th><th>建议</th></tr></thead>
            <tbody>
              <tr><td>🔴 孤岛页</td><td>没有其他页面引用的页面</td><td>补充交叉引用</td></tr>
              <tr><td>🔵 缺失页面</td><td>被提及但无独立页面</td><td>创建存根页面</td></tr>
              <tr><td>🕸️ 知识枢纽</td><td>被引用最多的核心页面</td><td>保持更新</td></tr>
              <tr><td>💡 建议</td><td>AI 生成的改进方向</td><td>参考探索</td></tr>
            </tbody>
          </table>

          <div class="callout">
            <span class="callout-icon">⚡</span>
            <span>健康度分析较慢（约 10 秒），只在点击「刷新」时执行，不会每次进入页面都等待。</span>
          </div>
        </section>

        <!-- 4. Wiki 工作台 -->
        <section id="wiki-workbench" class="manual-section">
          <h2>4. Wiki 工作台 — 操作知识库</h2>
          <p>四个功能页签，是你操作知识库的控制中心。</p>

          <div class="tab-flow">
            <div class="tab-card ingest">
              <div class="tab-icon">📥</div>
              <h3>Ingest 摄入</h3>
              <div class="tab-steps">
                <span class="step">① 输入资料路径</span>
                <span class="step-arrow">↓</span>
                <span class="step">② AI 读取全文</span>
                <span class="step-arrow">↓</span>
                <span class="step">③ 生成摘要</span>
                <span class="step-arrow">↓</span>
                <span class="step">④ 提取实体概念</span>
                <span class="step-arrow">↓</span>
                <span class="step">⑤ 创建/更新页面</span>
              </div>
            </div>
            <div class="tab-card query">
              <div class="tab-icon">💬</div>
              <h3>Query 查询</h3>
              <div class="tab-steps">
                <span class="step">① 输入问题</span>
                <span class="step-arrow">↓</span>
                <span class="step">② AI 读索引找相关页</span>
                <span class="step-arrow">↓</span>
                <span class="step">③ 综合回答+引用</span>
                <span class="step-arrow">↓</span>
                <span class="step">④ 可归档为论述</span>
              </div>
            </div>
            <div class="tab-card lint">
              <div class="tab-icon">🔍</div>
              <h3>Lint 检查</h3>
              <div class="tab-steps">
                <span class="step">① 扫描所有页面</span>
                <span class="step-arrow">↓</span>
                <span class="step">② 构建引用图谱</span>
                <span class="step-arrow">↓</span>
                <span class="step">③ 发现问题</span>
                <span class="step-arrow">↓</span>
                <span class="step">④ 可自动修复</span>
              </div>
            </div>
            <div class="tab-card schema">
              <div class="tab-icon">⚙️</div>
              <h3>Schema 规则</h3>
              <p class="tab-desc">编辑 AI 维护 Wiki 的规则手册：命名规范、摄入流程、查询流程</p>
            </div>
          </div>
        </section>

        <!-- 5. 知识探索 -->
        <section id="explore" class="manual-section">
          <h2>5. 知识探索 — 发现新视角</h2>
          <p>AI 基于你的 Wiki 主动发现你看不到的盲区。</p>

          <div class="explore-grid">
            <div class="explore-card gap">
              <div class="explore-icon">🔍</div>
              <h3>知识缺口</h3>
              <p>发现应该关联但没有交叉引用的概念对</p>
              <div class="explore-example">
                <span class="concept-pair"><span class="cp a">LLM OS</span> ✕ <span class="cp b">Second Brain</span></span>
                <span class="example-hint">→ "LLM OS 如何改变个人知识管理？"</span>
              </div>
            </div>
            <div class="explore-card question">
              <div class="explore-icon">❓</div>
              <h3>研究问题</h3>
              <p>AI 生成值得深入研究的开放性问题</p>
              <div class="explore-example">
                <span class="example-hint">"RAG 和 LLM Wiki 在大规模下谁更优？"</span>
              </div>
            </div>
            <div class="explore-card collision">
              <div class="explore-icon">⚡</div>
              <h3>概念碰撞</h3>
              <p>随机选两个概念，AI 分析交叉点和新洞察</p>
              <div class="explore-example">
                <span class="concept-pair"><span class="cp a">Memex</span> ✕ <span class="cp b">Markdown</span></span>
              </div>
            </div>
          </div>
        </section>

        <!-- 6. 外部摄入 -->
        <section id="ingest" class="manual-section">
          <h2>6. 外部摄入 — 从外部获取知识</h2>
          <p>从 RSS、arXiv 等源自动抓取文章，AI 预筛后一键摄入 Wiki。</p>

          <div class="pipeline">
            <div class="pipe-stage">
              <div class="pipe-icon">📡</div>
              <div class="pipe-label">抓取</div>
              <div class="pipe-desc">RSS/arXiv<br>最新文章</div>
            </div>
            <div class="pipe-arrow">→</div>
            <div class="pipe-stage">
              <div class="pipe-icon">🤖</div>
              <div class="pipe-label">AI 预筛</div>
              <div class="pipe-desc">评估与 Wiki<br>的相关性</div>
            </div>
            <div class="pipe-arrow">→</div>
            <div class="pipe-stage">
              <div class="pipe-icon">👀</div>
              <div class="pipe-label">人工确认</div>
              <div class="pipe-desc">高相关一键<br>摄入 / 跳过</div>
            </div>
            <div class="pipe-arrow">→</div>
            <div class="pipe-stage">
              <div class="pipe-icon">📚</div>
              <div class="pipe-label">入 Wiki</div>
              <div class="pipe-desc">自动摘要<br>更新知识库</div>
            </div>
          </div>

          <div class="callout">
            <span class="callout-icon">💡</span>
            <span>高相关文章支持「批量摄入」，一次整理多篇。</span>
          </div>
        </section>

        <!-- 7. 知识库 -->
        <section id="knowledge" class="manual-section">
          <h2>7. 知识库 — 笔记结构洞察</h2>
          <p>分析你整个 Obsidian 笔记本的结构和链接关系。</p>

          <div class="insight-grid">
            <div class="insight-card"><div class="insight-emoji">🏝️</div><h3>知识孤岛</h3><p>没有入链的笔记</p></div>
            <div class="insight-card"><div class="insight-emoji">🕸️</div><h3>知识枢纽</h3><p>被引用最多的笔记</p></div>
            <div class="insight-card"><div class="insight-emoji">📅</div><h3>尘封笔记</h3><p>最久未修改</p></div>
            <div class="insight-card"><div class="insight-emoji">🌱</div><h3>新生知识</h3><p>最近创建</p></div>
            <div class="insight-card"><div class="insight-emoji">📊</div><h3>知识领域</h3><p>文件夹分布</p></div>
          </div>
        </section>

        <!-- 8. 代码仓 -->
        <section id="code-repo" class="manual-section">
          <h2>8. 代码仓 — 管理代码仓库</h2>
          <table class="info-table">
            <thead><tr><th>操作</th><th>说明</th></tr></thead>
            <tbody>
              <tr><td>注册仓库</td><td>输入路径和名称，添加到管理列表</td></tr>
              <tr><td>查看详情</td><td>分支、HEAD、提交数、贡献者、提交历史</td></tr>
              <tr><td>打开 VSCode</td><td>一键在编辑器中打开仓库</td></tr>
            </tbody>
          </table>
        </section>

        <!-- 9. 首页配置 -->
        <section id="config" class="manual-section">
          <h2>9. 首页 — 系统配置</h2>
          <div class="config-grid">
            <div class="config-item">
              <div class="config-emoji">📁</div>
              <h3>Obsidian Vault</h3>
              <p>笔记本路径和名称</p>
            </div>
            <div class="config-item">
              <div class="config-emoji">🔌</div>
              <h3>Obsidian API</h3>
              <p>REST API 地址和密钥，保存后立即生效</p>
            </div>
            <div class="config-item">
              <div class="config-emoji">🤖</div>
              <h3>LLM 配置</h3>
              <p>AI 提供商、模型、参数</p>
            </div>
            <div class="config-item">
              <div class="config-emoji">🌙</div>
              <h3>深色模式</h3>
              <p>右上角月亮/太阳图标切换，自动保存</p>
            </div>
          </div>
        </section>

        <!-- 10. 典型工作流 -->
        <section id="workflow" class="manual-section">
          <h2>10. 典型工作流</h2>

          <div class="workflow-card">
            <h3>📝 日常记录</h3>
            <div class="workflow-flow">
              <span class="wf-step">写小记</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">定期同步</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">保持一致</span>
            </div>
          </div>

          <div class="workflow-card">
            <h3>📚 读书 / 学习</h3>
            <div class="workflow-flow">
              <span class="wf-step">保存资料到 Raw/</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">Wiki 工作台 Ingest</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">AI 自动摘要提取</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">Query 验证理解</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">知识探索发现新问题</span>
            </div>
          </div>

          <div class="workflow-card">
            <h3>📈 持续积累</h3>
            <div class="workflow-flow">
              <span class="wf-step">外部摄入获取文章</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">定期 Lint 保持健康</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">知识库查看结构</span>
              <span class="wf-arrow">→</span>
              <span class="wf-step">归档有价值的查询</span>
            </div>
          </div>

          <div class="callout success">
            <span class="callout-icon">🎯</span>
            <span>核心理念：你负责提供资料和提问，AI 负责摘要、分类、交叉引用。知识持续复利增长。</span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'

const sections = [
  { id: 'intro', num: 1, title: '什么是 ObsidianBrain' },
  { id: 'timeline', num: 2, title: '时光机' },
  { id: 'wiki-dashboard', num: 3, title: 'Wiki 看板' },
  { id: 'wiki-workbench', num: 4, title: 'Wiki 工作台' },
  { id: 'explore', num: 5, title: '知识探索' },
  { id: 'ingest', num: 6, title: '外部摄入' },
  { id: 'knowledge', num: 7, title: '知识库' },
  { id: 'code-repo', num: 8, title: '代码仓' },
  { id: 'config', num: 9, title: '首页配置' },
  { id: 'workflow', num: 10, title: '典型工作流' },
]

const activeSection = ref('intro')

function scrollTo(id: string) {
  const el = document.getElementById(id)
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'start' })
    activeSection.value = id
  }
}

function onScroll() {
  const scrollEl = document.querySelector('.manual-content')
  if (!scrollEl) return
  const scrollTop = scrollEl.scrollTop
  for (const s of sections) {
    const el = document.getElementById(s.id)
    if (el) {
      const offsetTop = el.offsetTop - scrollEl.offsetTop
      if (scrollTop >= offsetTop - 100) {
        activeSection.value = s.id
      }
    }
  }
}

onMounted(() => {
  const el = document.querySelector('.manual-content')
  if (el) el.addEventListener('scroll', onScroll)
})
onUnmounted(() => {
  const el = document.querySelector('.manual-content')
  if (el) el.removeEventListener('scroll', onScroll)
})
</script>

<style scoped>
.manual-page { max-width: 100%; min-height: 100%; }
.page-header { margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-muted); font-size: 14px; }

.manual-layout { display: flex; gap: 24px; }

/* 左侧目录 */
.toc-sidebar { width: 200px; flex-shrink: 0; position: sticky; top: 0; align-self: flex-start; max-height: calc(100vh - 100px); overflow-y: auto; }
.toc-inner { padding: 16px 12px; border-radius: 14px; }
.toc-header { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-bottom: 10px; padding-left: 8px; }
.toc-link { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: 8px; text-decoration: none; font-size: 13px; color: var(--text-muted); transition: all 0.15s ease; cursor: pointer; }
.toc-link:hover { background: var(--bg-glass-subtle); color: var(--text-secondary); }
.toc-link.active { background: var(--accent-light); color: var(--accent); font-weight: 600; }
.toc-num { width: 18px; text-align: center; font-weight: 600; }
.toc-text { flex: 1; }

/* 右侧内容 */
.manual-content { flex: 1; min-width: 0; overflow-y: auto; max-height: calc(100vh - 100px); padding-right: 8px; }
.manual-section { margin-bottom: 40px; scroll-margin-top: 20px; }
.manual-section h2 { font-size: 18px; font-weight: 700; color: var(--text-primary); margin-bottom: 16px; padding-bottom: 8px; border-bottom: 2px solid var(--accent-light); }
.manual-section h3 { font-size: 15px; font-weight: 600; color: var(--text-primary); margin: 16px 0 8px; }
.manual-section p { font-size: 14px; color: var(--text-secondary); line-height: 1.75; margin-bottom: 12px; }
.manual-section ul, .manual-section ol { padding-left: 22px; margin-bottom: 12px; }
.manual-section li { font-size: 14px; color: var(--text-secondary); line-height: 1.75; margin-bottom: 4px; }
.manual-section code { background: var(--bg-glass-subtle); padding: 2px 6px; border-radius: 4px; font-size: 0.9em; color: var(--text-tertiary); }

/* 流程图 */
.flow-diagram { display: flex; align-items: stretch; gap: 0; margin: 20px 0; flex-wrap: wrap; }
.flow-step { flex: 1; min-width: 120px; padding: 16px 12px; border-radius: 12px; text-align: center; background: var(--bg-glass); }
.flow-icon { font-size: 28px; margin-bottom: 6px; }
.flow-label { font-size: 14px; font-weight: 600; color: var(--text-primary); margin-bottom: 4px; }
.flow-desc { font-size: 12px; color: var(--text-muted); line-height: 1.5; }
.flow-arrow { display: flex; align-items: center; padding: 0 8px; font-size: 18px; color: var(--text-faint); }

/* 功能卡片网格 */
.feature-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; margin: 16px 0; }
.feature-card { padding: 16px; border-radius: 12px; }
.feature-icon { font-size: 24px; margin-bottom: 8px; }
.feature-card h3 { margin: 0 0 6px; font-size: 14px; }
.feature-card p { font-size: 12px; margin: 0; color: var(--text-muted); }

/* 仪表盘模拟 */
.dashboard-mock { padding: 16px; border-radius: 14px; margin: 16px 0; }
.mock-row { display: flex; gap: 8px; margin-bottom: 12px; }
.mock-stat { flex: 1; padding: 10px; border-radius: 10px; text-align: center; background: var(--bg-glass-subtle); font-size: 18px; font-weight: 700; color: var(--text-primary); }
.mock-stat small { display: block; font-size: 11px; font-weight: 400; color: var(--text-muted); margin-top: 2px; }
.mock-section { margin-top: 8px; }
.mock-title { font-size: 13px; font-weight: 600; color: var(--text-secondary); margin-bottom: 6px; }
.mock-bar { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; height: 24px; }
.mock-bar-fill { height: 8px; border-radius: 4px; }
.mock-bar-label { font-size: 12px; color: var(--text-muted); }

/* 信息表格 */
.info-table { width: 100%; border-collapse: collapse; margin: 16px 0; font-size: 13px; }
.info-table th { text-align: left; padding: 8px 12px; background: var(--bg-glass-subtle); color: var(--text-muted); font-weight: 600; border-bottom: 1px solid var(--border-faint); }
.info-table td { padding: 8px 12px; color: var(--text-secondary); border-bottom: 1px solid var(--border-faint); }
.info-table tr:last-child td { border-bottom: none; }

/* 提示框 */
.callout { display: flex; align-items: flex-start; gap: 10px; padding: 12px 16px; border-radius: 10px; background: var(--accent-light); margin: 16px 0; font-size: 13px; color: var(--text-secondary); }
.callout.success { background: rgba(74, 222, 128, 0.1); }
.callout-icon { font-size: 16px; flex-shrink: 0; }

/* Tab 流程 */
.tab-flow { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; margin: 16px 0; }
.tab-card { padding: 16px; border-radius: 12px; }
.tab-icon { font-size: 24px; margin-bottom: 6px; }
.tab-card h3 { margin: 0 0 10px; font-size: 14px; }
.tab-steps { display: flex; flex-direction: column; gap: 4px; }
.step { font-size: 12px; color: var(--text-secondary); padding: 4px 8px; border-radius: 6px; background: var(--bg-glass-subtle); }
.step-arrow { text-align: center; color: var(--text-faint); font-size: 11px; }
.tab-desc { font-size: 12px; color: var(--text-muted); margin: 0; }

/* 探索网格 */
.explore-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin: 16px 0; }
.explore-card { padding: 16px; border-radius: 12px; }
.explore-icon { font-size: 24px; margin-bottom: 6px; }
.explore-card h3 { margin: 0 0 6px; font-size: 14px; }
.explore-card p { font-size: 12px; margin: 0 0 10px; color: var(--text-muted); }
.explore-example { padding: 8px; border-radius: 8px; background: var(--bg-glass-subtle); }
.concept-pair { display: block; font-size: 12px; margin-bottom: 4px; }
.cp { padding: 2px 8px; border-radius: 6px; font-weight: 600; }
.cp.a { background: rgba(99, 102, 241, 0.15); color: #818cf8; }
.cp.b { background: rgba(236, 72, 153, 0.15); color: #f472b6; }
.example-hint { font-size: 11px; color: var(--text-muted); }

/* 管道图 */
.pipeline { display: flex; align-items: stretch; gap: 0; margin: 20px 0; flex-wrap: wrap; }
.pipe-stage { flex: 1; min-width: 100px; padding: 14px 10px; border-radius: 12px; text-align: center; background: var(--bg-glass); }
.pipe-icon { font-size: 24px; margin-bottom: 4px; }
.pipe-label { font-size: 13px; font-weight: 600; color: var(--text-primary); }
.pipe-desc { font-size: 11px; color: var(--text-muted); margin-top: 4px; }
.pipe-arrow { display: flex; align-items: center; padding: 0 6px; font-size: 16px; color: var(--text-faint); }

/* 洞察网格 */
.insight-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin: 16px 0; }
.insight-card { padding: 16px; border-radius: 12px; text-align: center; }
.insight-emoji { font-size: 28px; margin-bottom: 6px; }
.insight-card h3 { margin: 0 0 4px; font-size: 14px; }
.insight-card p { font-size: 12px; margin: 0; color: var(--text-muted); }

/* 配置网格 */
.config-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; margin: 16px 0; }
.config-item { padding: 16px; border-radius: 12px; }
.config-emoji { font-size: 24px; margin-bottom: 6px; }
.config-item h3 { margin: 0 0 4px; font-size: 14px; }
.config-item p { font-size: 12px; margin: 0; color: var(--text-muted); }

/* 工作流卡片 */
.workflow-card { padding: 16px 20px; border-radius: 12px; margin-bottom: 12px; }
.workflow-card h3 { margin: 0 0 10px; font-size: 14px; }
.workflow-flow { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.wf-step { padding: 5px 12px; border-radius: 8px; background: var(--bg-glass-subtle); font-size: 12px; color: var(--text-secondary); }
.wf-arrow { color: var(--text-faint); font-size: 14px; }

@media (max-width: 768px) {
  .toc-sidebar { display: none; }
  .feature-grid, .tab-flow, .config-grid { grid-template-columns: 1fr; }
  .explore-grid, .insight-grid { grid-template-columns: 1fr; }
  .flow-diagram, .pipeline { flex-direction: column; }
  .flow-arrow, .pipe-arrow { transform: rotate(90deg); padding: 4px 0; }
}
</style>
