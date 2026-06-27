<template>
  <div class="manual-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">使用手册</h1>
        <p class="page-subtitle">ObsidianBrain 知识引擎使用指南</p>
      </div>
    </header>

    <div class="manual-content">
      <!-- 目录 -->
      <div class="toc-card">
        <h3 class="toc-title">📋 目录</h3>
        <div class="toc-list">
          <a v-for="s in sections" :key="s.id" :href="'#' + s.id" class="toc-item">{{ s.num }}. {{ s.title }}</a>
        </div>
      </div>

      <!-- 1. 什么是 ObsidianBrain -->
      <section :id="sections[0].id" class="manual-section">
        <h2>{{ sections[0].num }}. {{ sections[0].title }}</h2>
        <p>ObsidianBrain 是一个运行在你电脑上的知识助手。它连接你的 Obsidian 笔记本，帮你：</p>
        <ul>
          <li>📝 <strong>记录碎片想法</strong>：随时写小记，支持文字和图片</li>
          <li>📚 <strong>构建知识 Wiki</strong>：把读过的文章、资料整理成结构化的知识库</li>
          <li>🔍 <strong>洞察知识健康度</strong>：发现被遗忘的笔记、知识盲区、核心枢纽</li>
          <li>🤖 <strong>AI 辅助维护</strong>：AI 帮你摘要、分类、交叉引用，你只需提供资料和提问</li>
        </ul>
      </section>

      <!-- 2. 时光机 -->
      <section :id="sections[1].id" class="manual-section">
        <h2>{{ sections[1].num }}. {{ sections[1].title }}</h2>
        <p>时光机是你的「碎片想法记录器」，类似微信朋友圈。</p>

        <h3>写小记</h3>
        <ol>
          <li>点击右上角「写小记」按钮</li>
          <li>输入内容（支持 Markdown 格式：加粗、列表、代码块等）</li>
          <li>可选：添加标签（逗号分隔，如「灵感,想法」）</li>
          <li>点击「发布小记」</li>
        </ol>
        <p>小记会保存到 Obsidian 的 <code>Timeline/</code> 文件夹，你可以在 Obsidian 中直接查看和编辑。</p>

        <h3>搜索和筛选</h3>
        <ul>
          <li><strong>搜索</strong>：在搜索框输入关键词，实时搜索小记内容</li>
          <li><strong>时间筛选</strong>：点击「今天」「7天」「30天」等快捷按钮，或选择自定义日期范围</li>
          <li><strong>标签筛选</strong>：点击小记下方的标签，快速搜索同标签的小记</li>
        </ul>

        <h3>图片</h3>
        <ul>
          <li>写小记时可以上传图片（最多 9 张）</li>
          <li>图片保存在 Obsidian 的 <code>Timeline/images/</code> 文件夹</li>
          <li>点击图片可查看大图，左右切换浏览</li>
        </ul>

        <h3>同步</h3>
        <p>如果你在 Obsidian 中直接修改或删除了小记，点击「同步」按钮可以把 Obsidian 中的最新内容同步到系统中。</p>
      </section>

      <!-- 3. Wiki 看板 -->
      <section :id="sections[2].id" class="manual-section">
        <h2>{{ sections[2].num }}. {{ sections[2].title }}</h2>
        <p>Wiki 看板是你的知识库「仪表盘」，展示 Wiki 的整体状态。</p>

        <h3>统计概览</h3>
        <p>顶部卡片显示：总页数、实体数、概念数、源摘要数、综合论述数。</p>

        <h3>知识健康度</h3>
        <ul>
          <li>🔴 <strong>孤岛页</strong>：没有任何其他笔记引用的页面，可能需要补充连接</li>
          <li>🔵 <strong>缺失页面</strong>：被提及但还没有独立页面的概念</li>
          <li>🕸️ <strong>知识枢纽</strong>：被引用最多的核心页面</li>
          <li>💡 <strong>建议</strong>：AI 给出的改进方向</li>
        </ul>
        <p>点击「刷新」按钮可以重新分析健康度（分析需要约 10 秒）。</p>

        <h3>最近操作</h3>
        <p>展示最近的摄入、查询、检查操作记录。</p>

        <h3>知识领域分布</h3>
        <p>按页面类型（实体/概念/源摘要/综合论述）的分布比例。</p>
      </section>

      <!-- 4. Wiki 工作台 -->
      <section :id="sections[3].id" class="manual-section">
        <h2>{{ sections[3].num }}. {{ sections[3].title }}</h2>
        <p>Wiki 工作台是你操作知识库的「控制中心」，有四个功能页签。</p>

        <h3>Ingest（摄入资料）</h3>
        <p>把一篇原始资料整理进 Wiki 的流程：</p>
        <ol>
          <li>在「原始资料路径」填入 Obsidian 中资料的位置（如 <code>Raw/articles/xxx.md</code>）</li>
          <li>选择资料类型（文章/论文/书籍章节等）</li>
          <li>点击「开始摄入」</li>
        </ol>
        <p>AI 会自动：读取全文 → 生成摘要 → 提取实体和概念 → 创建/更新相关 Wiki 页面 → 更新索引和日志。</p>
        <p>摄入后你会在 Obsidian 的 <code>Wiki/</code> 文件夹看到自动生成的知识页面。</p>

        <h3>Query（知识查询）</h3>
        <p>向你的知识库提问：</p>
        <ol>
          <li>在问题框输入你的问题</li>
          <li>AI 会先读取 Wiki 索引，找到相关页面</li>
          <li>基于这些页面的内容综合回答，附带引用来源</li>
        </ol>
        <p>勾选「归档为综合论述」可以把回答保存为新的 Wiki 页面，让知识持续积累。</p>

        <h3>Lint（健康检查）</h3>
        <p>检查 Wiki 的健康状态：</p>
        <ul>
          <li>找出孤岛页（没有入链的页面）</li>
          <li>找出缺失页面（被提及但没有独立页面的概念）</li>
          <li>识别知识枢纽（被引用最多的页面）</li>
          <li>AI 生成改进建议</li>
        </ul>
        <p>开启「自动修复」可以自动为缺失的概念创建存根页面。</p>

        <h3>Schema（维护规则）</h3>
        <p>编辑 Wiki 的维护规则文件，告诉 AI 应该如何组织和管理你的知识库。你可以自定义命名规范、摄入流程、查询流程等。</p>
      </section>

      <!-- 5. 知识探索 -->
      <section :id="sections[4].id" class="manual-section">
        <h2>{{ sections[4].num }}. {{ sections[4].title }}</h2>
        <p>基于已有 Wiki 主动发现新视角。</p>

        <h3>知识缺口</h3>
        <p>AI 分析你的 Wiki 中哪些概念之间应该有关联但目前没有交叉引用，提示你探索的交叉领域。</p>

        <h3>研究问题</h3>
        <p>AI 基于你的知识库生成值得深入研究的问题，每个问题附带相关概念和「为什么值得问」的理由。</p>

        <h3>概念碰撞</h3>
        <p>随机选取两个概念，AI 分析它们的交叉点和潜在新洞察。如果发现有价值的连接，可以添加交叉引用。</p>
      </section>

      <!-- 6. 外部摄入 -->
      <section :id="sections[5].id" class="manual-section">
        <h2>{{ sections[5].num }}. {{ sections[5].title }}</h2>
        <p>从外部信息源（RSS、arXiv 等）自动抓取文章，AI 预筛选后一键摄入 Wiki。</p>

        <h3>使用流程</h3>
        <ol>
          <li>系统自动从配置的信息源抓取最新文章</li>
          <li>AI 评估每篇文章与你的知识库的相关性</li>
          <li>点击「摄入到 Wiki」把文章整理进知识库</li>
          <li>不感兴趣的文章点「跳过」</li>
        </ol>
        <p>高相关文章可以「批量摄入」，一次性整理多篇。</p>
      </section>

      <!-- 7. 知识库 -->
      <section :id="sections[6].id" class="manual-section">
        <h2>{{ sections[6].num }}. {{ sections[6].title }}</h2>
        <p>分析你整个 Obsidian 笔记本的结构和链接关系。</p>

        <h3>五个洞察维度</h3>
        <ul>
          <li>🏝️ <strong>知识孤岛</strong>：没有被任何笔记引用的笔记，可能被遗忘</li>
          <li>🕸️ <strong>知识枢纽</strong>：被引用最多的核心笔记</li>
          <li>📅 <strong>尘封笔记</strong>：最久未修改的笔记，可能需要更新</li>
          <li>🌱 <strong>新生知识</strong>：最近创建的笔记</li>
          <li>📊 <strong>知识领域</strong>：按文件夹的笔记分布</li>
        </ul>
      </section>

      <!-- 8. 代码仓 -->
      <section :id="sections[7].id" class="manual-section">
        <h2>{{ sections[7].num }}. {{ sections[7].title }}</h2>
        <p>管理你本地的代码仓库，查看分支、提交历史、语言统计，一键在 VSCode 中打开。</p>
        <ol>
          <li>点击「注册仓库」，输入路径和名称</li>
          <li>查看仓库卡片：分支、语言占比、状态</li>
          <li>点击「详情」查看提交历史</li>
          <li>点击「VSCode」直接打开编辑器</li>
        </ol>
      </section>

      <!-- 9. 首页配置 -->
      <section :id="sections[8].id" class="manual-section">
        <h2>{{ sections[8].num }}. {{ sections[8].title }}</h2>
        <p>首页提供系统状态查看和配置管理。</p>

        <h3>系统配置</h3>
        <ul>
          <li><strong>Obsidian Vault</strong>：你的 Obsidian 笔记本路径和名称</li>
          <li><strong>Obsidian REST API</strong>：Obsidian 插件的 API 地址和密钥</li>
          <li><strong>LLM 配置</strong>：AI 模型的提供商、模型名、参数等</li>
        </ul>
        <p>修改配置后点击「保存配置」，Obsidian 相关配置会立即生效，其他配置可能需要重启服务。</p>

        <h3>深色模式</h3>
        <p>点击右上角的月亮/太阳图标切换深色/浅色主题，设置会自动保存。</p>
      </section>

      <!-- 10. 典型工作流 -->
      <section :id="sections[9].id" class="manual-section">
        <h2>{{ sections[9].num }}. {{ sections[9].title }}</h2>

        <h3>日常记录</h3>
        <ol>
          <li>在时光机写小记，记录当天的想法、灵感、见闻</li>
          <li>定期同步，确保系统和 Obsidian 内容一致</li>
        </ol>

        <h3>读书/学习</h3>
        <ol>
          <li>把文章/论文保存到 Obsidian 的 <code>Raw/articles/</code> 文件夹</li>
          <li>在 Wiki 工作台「Ingest」摄入这篇资料</li>
          <li>AI 自动摘要、提取概念、创建知识页面</li>
          <li>在 Wiki 工作台「Query」向知识库提问，验证理解</li>
          <li>定期在知识探索页面发现新问题和新连接</li>
        </ol>

        <h3>持续积累</h3>
        <ol>
          <li>通过外部摄入页面，从 RSS 等源获取与你知识库相关的文章</li>
          <li>定期在 Wiki 工作台执行 Lint，保持知识库健康</li>
          <li>在知识库页面查看笔记结构的健康度</li>
          <li>把有价值的查询结果归档为综合论述，让知识复利增长</li>
        </ol>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
const sections = [
  { id: 'intro', num: 1, title: '什么是 ObsidianBrain' },
  { id: 'timeline', num: 2, title: '时光机 — 记录碎片想法' },
  { id: 'wiki-dashboard', num: 3, title: 'Wiki 看板 — 知识库仪表盘' },
  { id: 'wiki-workbench', num: 4, title: 'Wiki 工作台 — 操作知识库' },
  { id: 'explore', num: 5, title: '知识探索 — 发现新视角' },
  { id: 'ingest', num: 6, title: '外部摄入 — 从外部获取知识' },
  { id: 'knowledge', num: 7, title: '知识库 — 笔记结构洞察' },
  { id: 'code-repo', num: 8, title: '代码仓 — 管理代码仓库' },
  { id: 'config', num: 9, title: '首页 — 系统配置' },
  { id: 'workflow', num: 10, title: '典型工作流' },
]
</script>

<style scoped>
.manual-page { max-width: 800px; min-height: 100%; margin: 0 auto; }
.page-header { margin-bottom: 24px; }
.page-title { font-size: 22px; font-weight: 600; color: var(--text-primary); letter-spacing: -0.3px; }
.page-subtitle { margin-top: 4px; color: var(--text-muted); font-size: 14px; }

.toc-card { padding: 20px 24px; border-radius: 16px; margin-bottom: 28px; }
.toc-title { font-size: 15px; font-weight: 600; color: var(--text-primary); margin-bottom: 12px; }
.toc-list { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 16px; }
.toc-item { font-size: 13px; color: var(--accent); text-decoration: none; padding: 3px 0; }
.toc-item:hover { text-decoration: underline; }

.manual-section { margin-bottom: 36px; }
.manual-section h2 { font-size: 18px; font-weight: 700; color: var(--text-primary); margin-bottom: 12px; padding-bottom: 6px; border-bottom: 1px solid var(--border-faint); }
.manual-section h3 { font-size: 15px; font-weight: 600; color: var(--text-primary); margin: 16px 0 8px; }
.manual-section p { font-size: 14px; color: var(--text-secondary); line-height: 1.75; margin-bottom: 10px; }
.manual-section ul, .manual-section ol { padding-left: 22px; margin-bottom: 12px; }
.manual-section li { font-size: 14px; color: var(--text-secondary); line-height: 1.75; margin-bottom: 4px; }
.manual-section code { background: var(--bg-glass-subtle); padding: 2px 6px; border-radius: 4px; font-size: 0.9em; font-family: 'SF Mono', monospace; color: var(--text-tertiary); }

@media (max-width: 768px) {
  .manual-page { max-width: 100%; }
  .toc-list { grid-template-columns: 1fr; }
}
</style>
