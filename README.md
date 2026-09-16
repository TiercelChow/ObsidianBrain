# ObsidianBrain

本地优先的 Rust 知识引擎与个人工作台，把 Markdown/PDF 阅读、时光机小记、个人任务和按书维护的 LLM Wiki 放在同一个空间，并通过 Tool API / MCP 向外提供能力。

**[访问产品官网与完整使用指南](https://tiercelchow.github.io/ObsidianBrain/)**

## 功能

### 阅境轩（Markdown 阅读器）
- 浏览本地任意文件夹，递归文件树导航
- Markdown 全格式渲染 + 语法高亮 + 行号
- Mermaid 图表渲染，点击全屏缩放/拖拽
- LaTeX 数学公式（KaTeX）
- 文档内链接跳转（相对路径解析、代码文件预览弹窗、锚点滚动）
- 全屏沉浸阅读（H1 毛玻璃固定 + 文字穿过效果）
- 翻页切换动画（方向跟随文件顺序）
- 历史记录（服务端共享、命名、置顶、删除）

### 时光机
- 类似朋友圈的碎片想法记录，支持 Markdown + 图片
- 关键词搜索 + 时间筛选 + 标签
- 与 Obsidian 双向同步
- 图片缩略图加速加载

### 阅境轩·书籍知识库（Book Wiki）
- 阅境轩书架中的每个 Markdown 文集都可建立一个相互隔离的知识库；PDF 继续用于阅读，不进入 Book Wiki
- Markdown 文件夹先同步为不可变来源版本和章节证据，再由智能编译跨章节归并概念、论断、关系与别名；原始文件保持只读
- 智能编译只生成候选变更，用户审核后才会原子写入正式 Wiki；revision、引用与历史版本均可追溯
- Wiki 工作台支持检索主题与来源章节，并查看实体正文、论断、关系、引用、别名和版本历史；知识体检可发现缺失引用与重复主题
- 书籍问答优先召回语义 Wiki、再以章节证据兜底，通过 DeepSeek Harness ACP 生成带 `[S#]` 引用的 Markdown 回答；会话可恢复、来源原位预览、有价值的回答可保存为待审核知识候选
- 研究任务通过持久化后台队列执行，支持离开页面后继续、取消、失败重试、报告恢复，并可选择生成真实可编辑的 `.pptx` 文件
- 专题研究可逐任务显式授权外部 HTTPS 文本：必须指定域名与次数，默认无网络权限，访问记录可审计
- Wiki 配置集中管理 Runtime Profile、Token 用量、每本书的 Purpose / Schema 配置文档，以及版本化的内置/自定义/ZIP 导入 Skill；候选 Skill 支持结构评测和当前版/候选版真实模型基准
- 数据保护支持 SQLite 一致快照、校验后恢复，以及单书 JSONL/Markdown Wiki 导出；内置 Skill 已覆盖跨章综合和 Markdown 汇编

> 当前边界：Markdown 的“来源 → 语义 Wiki → 审核 → 问答/研究 → PPTX/知识回写”闭环已经可用。问答通过 ACP 原生文本增量流式显示；PPTX 完成 OOXML 结构校验后即可下载，不提供应用内预览。脚本型 Skill 在独立沙箱、审批和资源配额完成前不可执行。

### 任务中枢
- 短期待办记录标题、描述、日期和重要程度
- 长期任务支持多级子任务、进展记录、状态审计和归档
- 任务视图与月历视图适配桌面和手机端
- 任务数据以本地 SQLite 为唯一事实来源

### 其他
- 代码仓管理（注册、详情、VS Code 打开）
- 灵感熔炉与智识雷达
- 系统配置（Obsidian + 通用 LLM Provider），热更新，无需重启

## 快速开始

当前仓库尚未提供预编译 Release，需要在目标电脑上从源码构建。支持情况如下：

| 系统 | 架构 | 构建方式 |
|---|---|---|
| macOS | Apple Silicon（arm64） | 在 Apple Silicon Mac 上原生构建 |
| macOS | Intel（x86_64） | 在 Intel Mac 上原生构建 |
| Windows 10 / 11 | x86_64 | 使用 MSVC 工具链原生构建 |

Windows ARM64 当前未经验证。macOS 会按当前终端架构生成二进制；Apple Silicon 用户可通过 `uname -m` 确认终端没有运行在 Rosetta 的 x86_64 模式下。

### 从源码构建

macOS 需要 Git、Node.js 18+、Rust 1.75+ 和 Xcode Command Line Tools：

```bash
git clone https://github.com/TiercelChow/ObsidianBrain.git
cd ObsidianBrain
make build          # 构建前端 + 后端（release 模式）
make install        # 安装到 ~/.local/bin/
```

构建产物是单个二进制文件 `backend/target/release/obsidian-brain`，前端已嵌入其中（`rust-embed`）。阅境轩、时光机和任务中枢运行时不需要 Node.js；若启用默认的 DeepSeek Harness 知识问答/研究任务，系统中仍需有可用的 `npm` / `npx`。

Windows x86_64 需要另外安装 Visual Studio Build Tools 的“使用 C++ 的桌面开发”工作负载，然后在 PowerShell 中执行：

```powershell
git clone https://github.com/TiercelChow/ObsidianBrain.git
Set-Location ObsidianBrain\frontend
npm ci
npx vue-tsc -b
npx vite build --outDir dist_new --emptyOutDir
Set-Location ..\backend
cargo build --release
```

产物位于 `backend\target\release\obsidian-brain.exe`。将它复制到固定目录并把该目录加入当前用户的 `Path`。

### 拿到二进制后怎么用

如果你拿到了编译好的 `obsidian-brain` 二进制（无需从源码构建），直接放到 PATH 里即可：

```bash
# 方式 1：手动复制
cp obsidian-brain /usr/local/bin/

# 方式 2：放到用户目录
mkdir -p ~/.local/bin
cp obsidian-brain ~/.local/bin/
export PATH="$HOME/.local/bin:$PATH"  # 加到 ~/.zshrc 或 ~/.bashrc
```

然后验证安装：

```bash
obsidian-brain version
# 输出：
# obsidian-brain 0.1.0
# Data directory: /Users/yourname/.obsidian-brain
```

### CLI 命令

```bash
# 启动（后台守护进程模式，默认绑定 0.0.0.0:9876）
obsidian-brain start

# 前台运行（调试用，日志直接输出到终端）
obsidian-brain start --foreground

# 指定绑定地址和端口
obsidian-brain start --host 127.0.0.1 --port 8080

# 停止
obsidian-brain stop

# 查看运行状态（PID、运行时间、工具数、Vault 路径）
obsidian-brain status

# 查看版本
obsidian-brain version
```

启动后浏览器访问 `http://localhost:9876`（本机）或 `http://<你的IP>:9876`（局域网）。

### 开发模式

```bash
# 终端 1：后端（前台运行）
cd backend && cargo run

# 终端 2：前端（HMR 热更新）
cd frontend && npm run dev
```

访问 `http://localhost:5173`（Vite dev server，自动代理 `/v1` 到后端）。

需要使用隔离数据库进行预览或手工测试时，环境变量前缀后也要保留双下划线：

```bash
cd backend
OBRAIN__SERVER__PORT=9988 \
OBRAIN__STORAGE__DB_PATH=/tmp/obsidianbrain-preview.db \
cargo run -- start --foreground
```

也可以设置 `OBRAIN_DATA_DIR=/tmp/obsidianbrain-preview`，一次隔离数据库、日志和缩略图。不要使用少一个下划线的 `OBRAIN_STORAGE__DB_PATH`，它不会覆盖正式数据库路径。

### 产品官网

产品介绍与使用指南位于独立的 `website/` 静态站中，不连接本地 `/v1` API：

```bash
cd website
npm install
npm run dev
```

官网由 `.github/workflows/deploy-pages.yml` 自动构建并发布到 GitHub Pages。首次发布前需在仓库的 **Settings → Pages** 中将 Source 设置为 **GitHub Actions**。

### 启用书籍知识库与 DeepSeek Harness

1. 在阅境轩把 Markdown 文件夹加入书架。PDF 仍可正常阅读，但不会出现在 Book Wiki 中。
2. 打开“书籍知识库”，为目标 Markdown 文集初始化知识库；来源会立即扫描并写入 SQLite。
3. 确保本机可以运行 `npx`。模型既可以使用 Harness 默认供应商，也可以在 Wiki 配置中接入 OpenAI Chat Completions、OpenAI Responses 或 Anthropic Messages 兼容接口。API Key 只通过环境变量交给启动进程，不写入数据库。

例如接入阿里云百炼兼容接口时，先在本机设置自定义凭据变量：

```bash
export CUSTOM_LLM_API_KEY="你的百炼 Key"
obsidian-brain start
```

Windows PowerShell：

```powershell
$env:CUSTOM_LLM_API_KEY = "你的百炼 Key"
obsidian-brain.exe start
```

4. 在“Wiki 配置 → Agent Runtime”中启用“第三方模型供应商”，填写供应商、协议、Base URL、模型 ID 和环境变量名，再保存配置。阿里云百炼示例：

```text
供应商名称：阿里云百炼
供应商 ID：aliyun-bailian
API 协议：OpenAI Chat Completions
API Base URL：https://dashscope.aliyuncs.com/compatible-mode/v1
模型 ID：glm-5.2
API Key 环境变量：CUSTOM_LLM_API_KEY
```

默认 Harness 启动命令固定为：

```text
npx -y @deepseek-ai/dsh@0.1.5-rc.1 --profile acp
```

保存后再点击“验证已保存配置”，该操作会发起一次最小模型请求。

5. 在“书籍问答”选择知识库进行带引用问答，或在“研究任务”创建并显式运行一项单书研究任务。

6. 对 Markdown 知识库，可在“书籍知识库”点击“智能编译”。系统会分批读取全部当前来源并生成跨章节候选；进入 Wiki 工作台检查差异，点击“批准并应用”后才会写入正式主题、论断、关系、引用和版本。

7. 在“Wiki 配置 → Skills”为当前书启用问答/研究 Skill，也可以导入根目录含 `SKILL.md` 的 ZIP。内置候选版本需依次通过“固定评测”和“真实基准”后才能发布；真实基准会在无工具权限的只读 Harness Run 中对比当前版与候选版。为保证安全，当前只接受 UTF-8 指令与参考文本，不执行包内脚本；Skill 本身不能授予 Shell、文件系统或网络权限。

8. 在“研究任务”把交付物设为“PPTX 演示文稿”并运行。任务进入本地持久化队列；刷新或离开页面不会丢失状态。完成后可查看 Markdown 报告、下载 PPTX，并单独审核研究结论是否写回 Wiki。

9. 只有确需补充书外资料时，才在创建“专题研究”任务时开启“授权外部资料研究”，填写允许域名和最多读取次数。后端只允许 HTTPS 文本，不跟随重定向，并拒绝本机、内网、IP 直连和超额请求；不开启时 Harness 不会看到外部读取工具。

10. 在“Wiki 配置 → 数据保护”创建或下载 SQLite 一致快照；恢复前需输入 `RESTORE`。单本书还可导出为流式 JSONL 结构包或可浏览 Markdown Wiki。

Harness 只接收后端召回的当前书籍证据。默认安全 Patch 会关闭文件系统、Shell、Web 和子 Agent 等能力；正式知识实体仍由 Rust 服务与 SQLite 管理。

> 旧版说明：原“知识库 / Wiki 看板 / Wiki 工作台 / 知识探索 / 外部摄入”实现已经退役，旧地址暂时跳转到新的 Book Wiki 页面。Vault 中既有的 `Wiki/*.md` 不会被删除，也不会自动导入或与 SQLite 双写。

## 配置

### 配置优先级

从低到高：

1. **代码默认值** — 内置在二进制中
2. **`config/default.toml`** — 开发时从 `backend/config/` 读取（安装后的二进制不依赖此文件）
3. **环境变量** — `OBRAIN__SERVER__HOST`、`OBRAIN__SERVER__PORT` 等（`__` 分隔层级）
4. **数据库配置** — 通过 `obsidian-brain config set` 或首页控制面板设置，持久化到 `~/.obsidian-brain/brain.db`
5. **CLI 参数** — `--host`、`--port`（仅当次启动有效，优先级最高）

### 可配置项

| 配置项 | CLI 设置命令 | 说明 | 默认值 |
|---|---|---|---|
| `server.host` | `config set server.host "0.0.0.0"` | 绑定地址。`0.0.0.0` 允许局域网访问，`127.0.0.1` 仅本机 | `0.0.0.0` |
| `server.port` | `config set server.port 9876` | 服务端口 | `9876` |
| `storage.db_path` | `config set storage.db_path "/path/to/brain.db"` | SQLite 数据库路径 | `~/.obsidian-brain/brain.db` |
| `vault.path` | `config set vault.path "/path/to/vault"` | Obsidian Vault 路径 | 空 |
| `vault.name` | `config set vault.name "my-vault"` | Vault 名称 | `brain` |
| `obsidian.enabled` | `config set obsidian.enabled true` | 启用 Obsidian REST API | `false` |
| `obsidian.url` | `config set obsidian.url "https://127.0.0.1:27124"` | Obsidian REST API 地址 | `https://127.0.0.1:27124` |
| `obsidian.api_key` | `config set obsidian.api_key "ey..."` | Obsidian REST API Key | 空 |
| `llm.provider` | `config set llm.provider "openai"` | LLM 提供商（`openai` 或 `ollama`） | `openai` |
| `llm.model` | `config set llm.model "gpt-4o-mini"` | 模型名称 | `gpt-4o-mini` |
| `llm.api_key` | `config set llm.api_key "sk-xxx"` | LLM API Key | 空 |
| `llm.base_url` | `config set llm.base_url "https://..."` | API Base URL（第三方兼容服务） | 空（用官方） |
| `llm.max_tokens` | `config set llm.max_tokens 2048` | 最大 Token | `2048` |
| `llm.temperature` | `config set llm.temperature 0.7` | 温度 | `0.7` |

### 配置命令

```bash
# 查看所有已保存的配置
obsidian-brain config show

# 获取单个配置项
obsidian-brain config get llm.model

# 设置配置项（持久化到数据库，重启后生效）
obsidian-brain config set server.host "0.0.0.0"
obsidian-brain config set llm.api_key "sk-xxx"
obsidian-brain config set llm.model "gpt-4o-mini"
```

也可以通过首页控制面板（Web UI）配置 Obsidian 和 LLM，保存后热更新生效。

### 局域网访问

默认绑定 `0.0.0.0`，局域网内其他设备可直接访问 `http://<你的IP>:9876`。

如果无法访问：
1. 确认配置：`obsidian-brain config get server.host` 应为 `0.0.0.0`
2. 检查 macOS 防火墙：系统设置 → 网络 → 防火墙 → 允许 `obsidian-brain` 入站连接
3. 确认 IP 地址：`ifconfig | grep inet` 获取局域网 IP

如需仅本机访问：`obsidian-brain config set server.host "127.0.0.1"`，然后重启。

### Obsidian Local REST API

1. 在 Obsidian 中安装 [Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) 插件
2. 启用插件，复制 API Key
3. 配置：`obsidian-brain config set obsidian.enabled true` + `obsidian-brain config set obsidian.api_key "你的Key"`
4. 或通过首页控制面板填写

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust + Axum + Tokio + rusqlite (bundled) |
| 前端 | Vue 3 + Element Plus + Pinia + Vite |
| Markdown | unified + remark/rehype + highlight.js + Mermaid + KaTeX |
| 交互 | panzoom（图表缩放）、Fullscreen API |
| Agent Runtime | DeepSeek Harness + Agent Client Protocol（ACP） |
| 打包 | rust-embed（前端嵌入单二进制）+ clap（CLI） |

## 架构

```
ObsidianBrain/
├── backend/
│   ├── src/
│   │   ├── main.rs              # CLI 入口（start/stop/status/config/version）
│   │   ├── daemon.rs           # 后台进程管理（fork/setsid/PID/日志）
│   │   ├── paths.rs            # 数据目录解析（~/.obsidian-brain/）
│   │   ├── frontend_assets.rs  # rust-embed 嵌入前端
│   │   ├── api/                # HTTP 路由 + 工具调用
│   │   ├── core/               # 业务逻辑（Book Wiki, timeline, tasks, reader...）
│   │   ├── infra/              # SQLite、Obsidian/LLM 客户端、Harness ACP 适配器
│   │   └── tools/              # Tool 注册 + 各模块 handler
│   ├── migrations/             # SQLite schema（编译时嵌入）
│   └── config/                 # Harness 安全 Patch 等运行配置
├── frontend/
│   ├── src/
│   │   ├── views/              # 页面与知识工作台
│   │   ├── components/reader/  # 阅境轩组件（FileTree, MermaidViewer, PathPreviewModal）
│   │   ├── markdown/           # unified 语法树、Obsidian 扩展、公式与安全 HTML
│   │   ├── workers/            # Markdown 后台解析 Worker
│   │   ├── composables/        # useMarkdownRender（异步渲染与懒增强管线）
│   │   └── stores/             # Pinia（主题、滚动状态）
│   ├── dist_new/               # 前端构建产物（rust-embed 编译时嵌入）
│   └── vite.config.ts
├── Makefile                    # build / install / clean
└── docs/                       # 设计文档
```

## 数据目录

所有运行时数据在 `~/.obsidian-brain/`：

```
~/.obsidian-brain/
├── brain.db              # SQLite（书架、Book Wiki 实体/引用/任务、个人任务、配置）
├── backups/              # 受管 SQLite 一致快照
├── exports/              # 单书 JSONL / Markdown Wiki 导出
├── artifacts/            # PPTX 等研究成果
├── thumbnails/           # 图片缩略图
├── tantivy_index/        # 全文索引
├── obsidian-brain.pid    # PID 文件
└── obsidian-brain.log    # 日志
```

## 项目结构

本项目采用前后端一体化架构：Rust 后端编译时将 Vue 前端嵌入二进制（`rust-embed`），核心工作台分发时只需一个可执行文件；只有启用默认 Harness sidecar 时才额外依赖 npm / npx。

前端统一通过 Tool API 调用 Rust 后端。Book Wiki 由后端限定知识库边界、召回数据库证据并审计运行，再通过 ACP 调用可替换的 Agent Runtime；Runtime 不直接操作正式数据库或原始书籍。

## License

Private
