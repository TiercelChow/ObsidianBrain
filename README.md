# ObsidianBrain

本地 Rust 知识引擎，围绕 Obsidian 笔记库提供 LLM Tool API、Markdown 阅读、时光机小记、Wiki 知识库等功能。

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

### Wiki 知识库
- LLM 作为「知识编译器」：读取原始资料 → 编译完整文章 → 合并/新建/级联更新
- 基于已编译 Wiki 回答问题（引用来源）
- 知识健康度检查（孤岛页、枢纽、缺失页）

### 其他
- 知识库结构洞察（孤岛、枢纽、尘封、新生、领域分布）
- 代码仓管理（注册、详情、VSCode 打开）
- 系统配置（Obsidian + LLM），热更新，无需重启

## 快速开始

### 从源码构建

需要 Node.js 18+ 和 Rust 1.75+：

```bash
git clone <repo-url>
cd ObsidianBrain
make build          # 构建前端 + 后端（release 模式）
make install       # 安装到 /usr/local/bin/
```

构建产物是单个二进制文件 `backend/target/release/obsidian-brain`，前端已嵌入其中（`rust-embed`），无需 Node.js 运行时。

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

### 产品官网

产品介绍与使用指南位于独立的 `website/` 静态站中，不连接本地 `/v1` API：

```bash
cd website
npm install
npm run dev
```

官网由 `.github/workflows/deploy-pages.yml` 自动构建并发布到 GitHub Pages。首次发布前需在仓库的 **Settings → Pages** 中将 Source 设置为 **GitHub Actions**。

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
| Markdown | marked + highlight.js + mermaid + KaTeX |
| 交互 | panzoom（图表缩放）、Fullscreen API |
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
│   │   ├── core/               # 业务逻辑（memory, timeline, wiki, reader...）
│   │   ├── infra/              # SQLite, Obsidian client, LLM client
│   │   └── tools/              # Tool 注册 + 各模块 handler
│   ├── migrations/             # SQLite schema（编译时嵌入）
│   └── frontend/dist/          # 前端构建产物（rust-embed 编译时嵌入）
├── frontend/
│   ├── src/
│   │   ├── views/              # 12 个页面
│   │   ├── components/reader/  # 阅境轩组件（FileTree, MermaidViewer, PathPreviewModal）
│   │   ├── composables/        # useMarkdownRender（marked+hljs+mermaid+katex 管线）
│   │   └── stores/             # Pinia（主题、滚动状态）
│   └── vite.config.ts
├── Makefile                    # build / install / clean
└── docs/                       # 设计文档
```

## 数据目录

所有运行时数据在 `~/.obsidian-brain/`：

```
~/.obsidian-brain/
├── brain.db              # SQLite（笔记元数据、小记、配置）
├── thumbnails/           # 图片缩略图
├── tantivy_index/        # 全文索引
├── obsidian-brain.pid    # PID 文件
└── obsidian-brain.log    # 日志
```

## 项目结构

本项目采用前后端一体化架构：Rust 后端编译时将 Vue 前端嵌入二进制（`rust-embed`），分发时只需一个可执行文件，无需 Node.js 运行时。

所有 LLM 交互通过 Tool API 完成——后端是 LLM 的「手」和「眼」，负责感知和执行，对话由 LLM 前端完成。

## License

Private
