# 代码仓聚合面板 (Code Repository Hub) — 需求设计文档

> **模块编号**: 05 | **版本**: v1.0 | **最后更新**: 2026-05-29 | **状态**: 需求评审中  
> **上游文档**: [顶层设计文档](../top_design.md) §5.3

---

## 1. 模块概述

### 1.1 定位

代码仓聚合面板（Code Repository Hub，以下简称 CodeRepo）是 ObsidianBrain 的轻量级代码仓管理模块。它**不涉及代码语义检索**，而是聚焦于以下四个核心能力：

| 能力 | 说明 |
|---|---|
| **仓库元信息展示** | 聚合展示本地 Git 仓库的分支、commit、语言构成、工作区状态等卡片级信息 |
| **笔记关联** | 在 Obsidian 笔记与代码仓库之间建立双向链接，打通知识笔记与工程实践的边界 |
| **跳转集成** | 一键唤起 VSCode 等外部编辑器，实现从知识上下文到工程上下文的无缝切换 |
| **自动文档化** | 借助 LLM 为代码仓库自动生成项目文档笔记，沉淀到 Obsidian vault 中 |

### 1.2 设计哲学

CodeRepo 的设计遵循顶层设计的核心原则——**本引擎是 LLM 的"手"和"眼"**。CodeRepo 不试图成为一个代码 IDE 或代码搜索引擎（这些由专业工具完成），而是做好以下角色：

- **感知**：让 LLM 能够"看到"用户本地有哪些代码仓库、它们的状态如何
- **关联**：让知识笔记与代码仓库之间的连接显式化、可追溯
- **执行**：将仓库信息文档化，沉淀为可被记忆引擎索引的长期知识

### 1.3 目标用户场景

个人开发者/研究者，同时在本地维护 3–20 个代码仓库，使用 Obsidian 做知识管理，希望：
- 快速回顾"我手上有哪些项目、各自什么状态"
- 在写技术笔记时方便关联到对应代码仓库
- 为新项目自动生成一份初始文档笔记
- 在知识回顾时看到代码提交活动与知识演变的关联

### 1.4 边界声明（不做什么）

| 不做 | 理由 |
|---|---|
| 代码语义搜索 | 由专业工具（VSCode、GitHub Search、Sourcegraph）完成 |
| 代码 diff / review | 由 Git 客户端 / IDE 完成 |
| 远端仓库管理（push/pull/PR） | 由 Git CLI / GitHub CLI 完成 |
| 代码文件内容读取 | 由 VSCode / 文件系统直接完成 |
| 多用户协作 | 本系统为单用户本地系统 |

---

## 2. 功能需求

### FR-01: 仓库注册

**工具名**: `add_code_repo`

**功能描述**: 将本地 Git 仓库注册到 ObsidianBrain，提取初始元数据并持久化配置。

**输入参数**:

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `path` | `string` | 是 | 仓库的本地绝对路径，如 `/Users/me/projects/my-app` |
| `name` | `string` | 是 | 仓库的显示名称，全局唯一，如 `my-app` |

**处理流程**:

1. **路径校验**：验证路径存在且为目录
2. **Git 验证**：通过 git2 尝试打开仓库，验证为合法 Git 仓库
3. **唯一性检查**：检查 `name` 和 `path` 在已注册列表中不重复
4. **元数据提取**：通过 git2 提取初始信息——当前分支、最近 10 条 commit、语言统计、工作区状态
5. **持久化存储**：将注册信息和元数据缓存写入 SQLite `code_repos` 表
6. **设置监控**：为该仓库的 `.git/HEAD` 文件设置文件监控（notify）
7. **时间线记录**：向时间线模块发送 `RepoRegistered` 事件

**输出**: 注册成功的仓库完整信息（同 `get_repo_detail` 返回格式）

**异常处理**:

| 异常场景 | 错误码 | 提示信息 |
|---|---|---|
| 路径不存在 | `PATH_NOT_FOUND` | "路径 '{path}' 不存在，请检查路径是否正确" |
| 路径非目录 | `NOT_A_DIRECTORY` | "路径 '{path}' 不是一个目录" |
| 非 Git 仓库 | `NOT_A_GIT_REPO` | "路径 '{path}' 不是一个有效的 Git 仓库（缺少 .git 目录）" |
| 名称已存在 | `NAME_DUPLICATED` | "名称 '{name}' 已被仓库 '{existing_path}' 使用" |
| 路径已注册 | `PATH_DUPLICATED` | "路径 '{path}' 已注册为仓库 '{existing_name}'" |
| 无读取权限 | `PERMISSION_DENIED` | "无法读取路径 '{path}'，请检查目录权限" |

---

### FR-02: 仓库列表

**工具名**: `list_code_repos`

**功能描述**: 返回所有已注册仓库的卡片级摘要信息，用于 LLM 快速了解用户的代码仓全貌。

**输入参数**: 无

**处理流程**:

1. 从 SQLite `code_repos` 表读取所有已注册仓库
2. 对每个仓库，通过 git2 实时刷新关键状态（当前分支、is_dirty、最新 commit）
3. 组装卡片级信息列表

**输出字段（每个仓库卡片）**:

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | `string` | 仓库显示名称 |
| `path` | `string` | 本地绝对路径 |
| `current_branch` | `string` | 当前分支名 |
| `latest_commit` | `CommitSummary` | 最新一条 commit 摘要 |
| `is_dirty` | `bool` | 工作区是否有未提交更改 |
| `languages` | `map<string, float>` | 语言构成（语言名 → 占比，如 `{"Rust": 0.72}`） |
| `linked_notes_count` | `int` | 关联笔记数量 |
| `last_activity` | `datetime` | 最后活动时间（最新 commit 时间） |
| `status` | `string` | 仓库状态：`active` / `inactive`（路径失效时） |

**刷新策略**:
- 分支、is_dirty、最新 commit：每次查询实时从 git2 获取（轻量操作）
- 语言统计：从 SQLite 缓存读取，仅在仓库 HEAD 变更时刷新
- 若 git2 打开仓库失败（路径已不存在），标记 `status: inactive`，不报错

---

### FR-03: 仓库详情

**工具名**: `get_repo_detail`

**功能描述**: 返回指定仓库的完整详细信息，包括完整的 commit 历史、分支列表、语言统计等。

**输入参数**:

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `name` | `string` | 是 | 已注册仓库的显示名称 |

**处理流程**:

1. 从 SQLite 查询仓库注册信息
2. 通过 git2 提取完整实时信息
3. 合并缓存信息（如关联笔记列表）与实时信息
4. 更新 SQLite 中的元数据缓存

**输出字段（在卡片信息基础上扩展）**:

| 字段 | 类型 | 说明 |
|---|---|---|
| （卡片全部字段） | | |
| `recent_commits` | `CommitSummary[]` | 最近 20 条 commit（hash、author、message、timestamp） |
| `branches` | `string[]` | 本地分支列表 |
| `remote_urls` | `string[]` | 远端 URL 列表 |
| `working_dir_status` | `WorkingDirStatus` | 详细工作区状态（modified/added/deleted/untracked 文件数） |
| `linked_notes` | `string[]` | 关联笔记路径列表 |
| `vscode_uri` | `string` | VSCode 打开链接 |
| `registered_at` | `datetime` | 注册时间 |
| `head_hash` | `string` | 当前 HEAD commit hash |
| `total_commits` | `int` | 总 commit 数量（近似值） |
| `contributors` | `string[]` | 贡献者列表（从最近 commit 中提取） |

**异常处理**:

| 异常场景 | 错误码 | 提示信息 |
|---|---|---|
| 仓库未注册 | `REPO_NOT_FOUND` | "代码仓库 '{name}' 未找到，请先使用 add_code_repo 注册" |
| 路径已失效 | `REPO_PATH_INVALID` | "仓库 '{name}' 的路径 '{path}' 已不可访问，请检查路径或重新注册" |

---

### FR-04: 笔记关联

**工具名**: `link_note_to_repo`

**功能描述**: 在 Obsidian 笔记与代码仓库之间建立关联关系，并在笔记末尾插入标准化的引用块。

**输入参数**:

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `note_path` | `string` | 是 | 笔记在 vault 内的相对路径，如 `projects/my-app-notes.md` |
| `repo_name` | `string` | 是 | 已注册仓库的显示名称 |

**处理流程**:

1. **校验**：验证笔记文件存在、仓库已注册
2. **去重**：检查 `note_repo_links` 表，若关联已存在则直接返回成功
3. **写入引用块**：在笔记文件末尾追加标准 Markdown 引用块（见 §6.1 模板）
4. **记录关联**：在 SQLite `note_repo_links` 表写入关联记录
5. **触发索引更新**：通知记忆引擎重新索引该笔记（因内容已变更）

**输出**:

```json
{
  "note_path": "projects/my-app-notes.md",
  "repo_name": "my-app",
  "linked_at": "2026-05-29T10:30:00Z",
  "vscode_uri": "vscode://file/Users/me/projects/my-app"
}
```

**引用块格式**（插入到笔记末尾）:

```markdown

---
## 🔗 相关代码仓库
- **my-app** — `/Users/me/projects/my-app`
  - [在 VSCode 中打开](vscode://file/Users/me/projects/my-app)
  - 最后活动: 2026-05-28 | 分支: main
```

> 详细模板定义见 §6.1

**幂等性**: 同一笔记对同一仓库重复关联，不重复插入引用块，直接返回已有信息。

**异常处理**:

| 异常场景 | 错误码 | 提示信息 |
|---|---|---|
| 笔记不存在 | `NOTE_NOT_FOUND` | "笔记 '{note_path}' 不存在于 vault 中" |
| 仓库未注册 | `REPO_NOT_FOUND` | "代码仓库 '{repo_name}' 未找到" |
| 笔记无写入权限 | `NOTE_WRITE_DENIED` | "无法写入笔记 '{note_path}'，请检查文件权限" |

---

### FR-05: 自动关联建议

**功能描述**: 当用户新建或修改笔记时，系统自动检测笔记内容是否与已注册仓库相关，并在工具返回结果中附带关联建议。

**触发时机**:
- 笔记文件监控检测到新增/修改事件时
- 用户调用 `search_memory` 等工具时，结果中可附带关联建议

**匹配策略**:

1. **关键词提取**：从笔记标题和正文中提取关键词（标题权重 ×3）
2. **仓库特征提取**：从仓库的以下信息中构建特征词集合：
   - 仓库名称（分词后）
   - 最近 20 条 commit message 的关键词
   - 主要语言名称
   - 关联笔记的标签
3. **匹配算法**：简单字符串匹配 + 词频加权
   - 精确匹配仓库名：权重 10
   - 匹配 commit message 中的关键词：权重 × 出现次数
   - 匹配语言名称：权重 2
4. **阈值判定**：综合得分 > 阈值（默认 5）时生成建议

**输出格式**（附加在其他工具返回结果中）:

```json
{
  "link_suggestions": [
    {
      "note_path": "programming/rust-async.md",
      "suggested_repo": "my-app",
      "confidence": 0.85,
      "reason": "笔记标题包含 'my-app'，且 commit message 多次提及 'async runtime'"
    }
  ]
}
```

> 此功能为辅助建议，不自动执行关联操作。由 LLM 向用户展示建议，用户确认后通过 `link_note_to_repo` 手动关联。

---

### FR-06: 自动文档化

**工具名**: `generate_docs`

**功能描述**: 为指定代码仓库自动生成项目文档笔记，沉淀到 Obsidian vault 中。

**输入参数**:

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `repo_name` | `string` | 是 | 已注册仓库的显示名称 |
| `target_path` | `string` | 否 | vault 内目标目录路径，默认 `code-docs/` |

**处理流程**:

1. **仓库信息提取**：
   - 目录结构（排除 `.git`、`node_modules`、`target`、`vendor`、`__pycache__` 等，深度限制 3 层）
   - `README.md` / `README` 内容（若存在）
   - 项目配置文件内容：`Cargo.toml` / `package.json` / `pyproject.toml` / `go.mod` 等
   - 核心源文件头部注释（前 20 行，最多 10 个文件）
   - 仓库元数据（分支、语言统计、最近 commit）

2. **Prompt 组装**：将提取的信息填入 LLM prompt 模板（见 §6.2）

3. **LLM 调用**：调用配置的 LLM provider 生成文档

4. **输出写入**：
   - 生成 Markdown 文件到 `<vault>/<target_path>/<repo_name>-docs.md`
   - 若文件已存在，询问覆盖（通过 LLM 交互确认）或生成带时间戳的新文件
   - 在文件 frontmatter 中写入元数据（生成时间、仓库路径、commit hash）

5. **后续处理**：
   - 自动建立笔记-仓库关联（调用 `link_note_to_repo` 逻辑）
   - 通知记忆引擎索引新生成的文档
   - 向时间线发送 `RepoDocumented` 事件

**输出**:

```json
{
  "repo_name": "my-app",
  "doc_path": "code-docs/my-app-docs.md",
  "generated_at": "2026-05-29T10:30:00Z",
  "word_count": 1200,
  "sections": ["项目概述", "目录结构", "核心模块", "技术栈", "依赖列表"]
}
```

**模板可配置**: 用户可通过 `config/doc_template.md` 自定义文档模板，引擎在生成时加载该模板作为 prompt 的一部分。

**异常处理**:

| 异常场景 | 错误码 | 提示信息 |
|---|---|---|
| 仓库未注册 | `REPO_NOT_FOUND` | "代码仓库 '{repo_name}' 未找到" |
| LLM 调用失败 | `LLM_API_ERROR` | "LLM 服务调用失败：{detail}，请稍后重试" |
| 目标路径无写入权限 | `WRITE_DENIED` | "无法写入路径 '{target_path}'，请检查目录权限" |
| 仓库信息提取超时 | `EXTRACT_TIMEOUT` | "仓库信息提取超时，仓库可能过大，请检查排除配置" |

---

### FR-07: VSCode 集成

**工具名**: `open_in_vscode`

**功能描述**: 在 VSCode 中打开指定代码仓库。

**输入参数**:

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `repo_name` | `string` | 是 | 已注册仓库的显示名称 |

**处理流程**:

1. 从 SQLite 查询仓库路径
2. 验证路径仍然存在
3. 生成 VSCode URI：`vscode://file/<absolute_path>`
4. 通过系统调用 `open` (macOS) / `xdg-open` (Linux) / `start` (Windows) 打开 URI
5. 若系统调用失败，回退为执行 `code <path>` 命令

**输出**:

```json
{
  "repo_name": "my-app",
  "vscode_uri": "vscode://file/Users/me/projects/my-app",
  "opened": true
}
```

**跨平台支持**:

| 平台 | 主方式 | 回退方式 |
|---|---|---|
| macOS | `open "vscode://file/..."` | `code /path/to/repo` |
| Linux | `xdg-open "vscode://file/..."` | `code /path/to/repo` |
| Windows | `start "vscode://file/..."` | `code /path/to/repo` |

> 注：VSCode 集成仅负责生成 URI 并尝试唤起，不保证 VSCode 已安装。若唤起失败，返回 URI 供用户手动使用。

---

### FR-08: 仓库状态监控

**功能描述**: 定时或触发式更新已注册仓库的元信息缓存，确保信息新鲜度。

**监控机制**:

1. **文件监控（触发式）**：
   - 使用 `notify` crate 监控每个已注册仓库的 `.git/HEAD` 文件
   - 当 HEAD 变更（切换分支、新 commit）时，触发元信息刷新
   - 刷新内容：当前分支、最新 commit、is_dirty 状态

2. **定时刷新（兜底式）**：
   - 可配置的定时刷新间隔（默认 5 分钟）
   - 通过 `tokio-cron-scheduler` 调度
   - 全量刷新所有 active 仓库的元信息缓存
   - 语言统计每小时刷新一次（较重操作）

3. **手动触发**：
   - `get_repo_detail` 调用时强制刷新该仓库的全部信息
   - `list_code_repos` 调用时轻量刷新关键状态

**缓存更新策略**:

| 信息项 | 刷新触发 | 刷新方式 |
|---|---|---|
| 当前分支 | HEAD 变更 / 定时 | git2 实时读取 |
| is_dirty | HEAD 变更 / 定时 | git2 status 检查 |
| 最新 commit | HEAD 变更 / 定时 | git2 revparse |
| 语言统计 | 定时（每小时） / 手动 | 文件遍历统计 |
| 分支列表 | 定时 / get_repo_detail | git2 branches |
| 工作区状态 | 定时 / get_repo_detail | git2 status |

**失效处理**:
- 若仓库路径不可访问（如外接硬盘拔出），标记 `status: inactive`
- 不删除注册信息，待路径恢复后自动恢复为 `active`
- 在 `list_code_repos` 结果中标注 inactive 状态

---

## 3. 用户故事

### US-01: 注册新仓库

> **作为**一名使用 Obsidian 管理知识的 Rust 开发者，  
> **我希望**能将本地新建的项目仓库注册到 ObsidianBrain，  
> **以便**LLM 助手能感知到我的项目存在并为我提供关联服务。

**场景**:
1. 用户在 `/Users/me/projects/new-api` 创建了一个新的 Rust 项目
2. 用户通过 LLM 对话："帮我注册一下 new-api 这个仓库，路径在 /Users/me/projects/new-api"
3. LLM 调用 `add_code_repo(path="/Users/me/projects/new-api", name="new-api")`
4. 系统验证路径、提取元数据、持久化配置
5. LLM 回复用户："已注册仓库 new-api，当前在 main 分支，语言构成：Rust 95%、TOML 5%，工作区干净。"

**验收条件**:
- 仓库成功注册并可后续查询
- 元数据完整且准确
- 重复注册同一路径或名称时给出明确错误提示

---

### US-02: 查看仓库全貌

> **作为**同时维护多个项目的开发者，  
> **我希望**通过一次对话快速了解我所有代码仓库的当前状态，  
> **以便**在晨间回顾时知道哪些项目有未提交的修改、哪些项目最近活跃。

**场景**:
1. 用户每天早上对 LLM 说："看看我的代码仓库都什么状态了"
2. LLM 调用 `list_code_repos`
3. 系统返回所有仓库的卡片信息
4. LLM 整理为自然语言："你有 5 个仓库。其中 my-app 和 data-pipeline 有未提交的修改；obsidian-brain 最近活跃（昨天 3 次提交）；ml-experiment 已 2 周没有活动。"

**验收条件**:
- 返回信息包含分支、is_dirty、最新 commit 等关键字段
- inactive 仓库有明确标注
- 响应延迟 < 500ms（5 个仓库以内）

---

### US-03: 深入查看仓库详情

> **作为**项目的主人，  
> **我希望**能查看某个仓库的详细信息——包括最近的 commit 历史、分支列表、贡献者等，  
> **以便**在写周报或做回顾时有准确的数据支撑。

**场景**:
1. 用户说："给我看看 my-app 仓库的详细情况"
2. LLM 调用 `get_repo_detail(name="my-app")`
3. 系统返回完整详情
4. LLM 整理回复："my-app 仓库当前在 main 分支，共有 5 个本地分支。最近 5 次提交都由你完成，最新一次是昨天 'feat: add auth module'。语言构成以 Rust 为主（72%），还有 TypeScript（18%）和 Python（10%）。工作区干净，无未提交修改。已关联 2 篇笔记。"

**验收条件**:
- commit 历史包含 hash、author、message、timestamp
- 分支列表完整
- 语言统计占比准确（误差 < 5%）

---

### US-04: 笔记关联仓库

> **作为**习惯在 Obsidian 中记录项目设计思路的开发者，  
> **我希望**能将项目设计笔记与对应的代码仓库关联起来，  
> **以便**日后从笔记可以快速跳转到代码仓库，从仓库也能回溯到设计笔记。

**场景**:
1. 用户写了一篇 `projects/my-app-design.md` 设计笔记
2. 用户对 LLM 说："把这篇设计笔记和 my-app 仓库关联一下"
3. LLM 调用 `link_note_to_repo(note_path="projects/my-app-design.md", repo_name="my-app")`
4. 系统在笔记末尾插入引用块，包含仓库路径和 VSCode 打开链接
5. LLM 回复："已关联。笔记末尾已添加仓库引用块，你可以点击链接直接在 VSCode 中打开 my-app。"
6. 后续查看 `get_repo_detail` 时，`linked_notes` 中包含该笔记路径

**验收条件**:
- 引用块正确插入到笔记末尾
- 重复关联不重复插入
- 关联关系在 `get_repo_detail` 中可查询
- 记忆引擎重新索引了该笔记

---

### US-05: 自动关联建议

> **作为**经常在笔记中引用代码项目的知识工作者，  
> **我希望**系统能自动发现我的笔记和某个代码仓库相关并给出建议，  
> **以便**我不会忘记建立有价值的关联。

**场景**:
1. 用户新建笔记 `programming/rust-error-handling.md`，内容多次提到 "my-app" 项目中的错误处理实践
2. 文件监控检测到新笔记，后台执行关联分析
3. 系统发现笔记关键词 "my-app" 与已注册仓库 `my-app` 高度匹配
4. 当用户下次与 LLM 对话时，LLM 从系统获取关联建议并提示："我注意到你的新笔记 'rust-error-handling.md' 似乎与 my-app 仓库相关，需要我帮你建立关联吗？"
5. 用户确认后，LLM 调用 `link_note_to_repo` 完成关联

**验收条件**:
- 建议的 confidence 分数合理
- 不自动执行关联，仅建议
- 建议原因（reason）可解释

---

### US-06: 自动生成项目文档

> **作为**刚接手一个复杂项目的开发者（或项目搁置许久后重新拾起），  
> **我希望**系统能为代码仓库自动生成一份结构化的项目文档笔记，  
> **以便**快速回忆项目结构、技术栈和核心模块。

**场景**:
1. 用户说："帮我给 my-app 这个项目生成一份文档笔记"
2. LLM 调用 `generate_docs(repo_name="my-app")`
3. 系统提取仓库信息（目录结构、README、Cargo.toml、源文件头部注释）
4. 调用 LLM 生成结构化文档
5. 写入 `code-docs/my-app-docs.md`
6. LLM 回复："已为 my-app 生成项目文档，保存在 code-docs/my-app-docs.md。文档包含项目概述、目录结构说明、核心模块介绍、技术栈和依赖列表。已自动关联到 my-app 仓库。"

**验收条件**:
- 文档包含完整的结构化信息
- 自动建立笔记-仓库关联
- 文档被记忆引擎索引
- 模板可通过 `config/doc_template.md` 自定义

---

### US-07: VSCode 跳转

> **作为**在 Obsidian 知识系统和代码编辑器之间频繁切换的开发者，  
> **我希望**能从笔记上下文一键跳转到对应代码仓库的 VSCode 窗口，  
> **以便**减少上下文切换的认知负担。

**场景**:
1. 用户在浏览仓库详情时，LLM 展示了 VSCode 打开链接
2. 用户说："用 VSCode 打开 my-app"
3. LLM 调用 `open_in_vscode(repo_name="my-app")`
4. 系统通过 `vscode://file/...` URI 唤起 VSCode
5. VSCode 打开对应项目目录
6. LLM 回复："已在 VSCode 中打开 my-app。"

**验收条件**:
- macOS / Linux 平台均可正常唤起
- VSCode 未安装时返回 URI 供手动使用
- 路径不存在时给出明确错误

---

### US-08: 仓库状态自动更新

> **作为**不经常手动刷新状态的开发者，  
> **我希望**仓库的元信息能在后台自动更新，  
> **以便**我每次查询时看到的都是最新状态。

**场景**:
1. 用户在终端中对 my-app 执行了 `git commit`
2. `.git/HEAD` 文件变更被 notify 监控到
3. 系统自动刷新 my-app 的元信息缓存（最新 commit、分支状态）
4. 5 分钟后用户询问仓库状态，看到的是已更新的信息
5. 若仓库在外接硬盘上，硬盘拔出后系统标记仓库为 inactive，插入后自动恢复

**验收条件**:
- HEAD 变更后 < 5 秒完成缓存刷新
- inactive 仓库不影响 `list_code_repos` 正常返回
- 定时刷新兜底间隔可配置

---

### US-09: 周报中集成代码活动

> **作为**习惯每周做知识回顾的用户，  
> **我希望**时间线/周报中能自然包含代码仓库的活动摘要，  
> **以便**回顾时能看到"这周我在代码上做了什么"。

**场景**:
1. 用户调用 `weekly_review` 技能或 `get_timeline` 工具
2. 时间线模块从 CodeRepo 获取本周各仓库的 commit 事件
3. 整合笔记变更和代码提交，生成完整的周报
4. LLM 回复："本周你修改了 8 篇笔记，在 my-app 仓库提交了 12 次（主要是 auth 模块），data-pipeline 有 3 次提交。"

**验收条件**:
- commit 事件被正确记录到时间线
- 时间线查询可按日期范围过滤 commit 事件
- 事件包含仓库名、commit message、时间

---

### US-10: 批量仓库管理

> **作为**拥有大量本地仓库的资深开发者，  
> **我希望**能方便地批量注册和管理多个仓库，  
> **以便**快速完成初始化配置。

**场景**:
1. 用户说："把 /Users/me/projects/ 下面所有的 Git 仓库都注册一下，用目录名作为仓库名"
2. LLM 扫描目录，识别 Git 仓库
3. LLM 逐个调用 `add_code_repo`（或通过未来的批量接口）
4. 跳过已注册的和非 Git 目录
5. LLM 回复："扫描完成，发现 8 个 Git 仓库，新注册了 5 个，3 个已注册过。"

**验收条件**:
- 单个注册失败不影响其他仓库
- 已注册的仓库不重复注册
- 非 Git 目录被自动跳过

---

## 4. 非功能需求

### NFR-01: 性能要求

| 操作 | 目标延迟 | 备注 |
|---|---|---|
| `add_code_repo` | < 2 秒 | 含元数据提取和持久化 |
| `list_code_repos` (5 仓库) | < 500ms | 含实时刷新 |
| `list_code_repos` (20 仓库) | < 2 秒 | 含实时刷新 |
| `get_repo_detail` | < 1 秒 | 含完整 git2 信息提取 |
| `link_note_to_repo` | < 500ms | 含文件写入和索引触发 |
| `open_in_vscode` | < 200ms | 仅 URI 生成和系统调用 |
| `generate_docs` | < 30 秒 | 含 LLM 调用（依赖 API 速度） |
| 仓库状态缓存刷新 | < 500ms | HEAD 变更触发 |

### NFR-02: 资源消耗

| 指标 | 限制 |
|---|---|
| 内存占用（10 仓库） | < 10MB（不含缓存数据） |
| 定时任务 CPU 占用 | < 1%（空闲时） |
| SQLite 存储增长 | < 1MB / 100 仓库 / 月 |
| 文件监控文件描述符 | 每仓库 1–2 个（仅监控 .git/HEAD） |

### NFR-03: 元信息更新频率

| 信息类型 | 更新频率 |
|---|---|
| 当前分支 / is_dirty / 最新 commit | HEAD 变更触发 + 5 分钟兜底 |
| 语言统计 | 1 小时 + 手动触发 |
| 分支列表 | 5 分钟 + get_repo_detail 触发 |
| 工作区详细状态 | 5 分钟 + get_repo_detail 触发 |

### NFR-04: 可靠性

- 仓库路径失效不导致系统崩溃，仅标记 inactive
- 单个仓库的 git2 操作失败不影响其他仓库的查询
- SQLite 写入失败时有重试机制（3 次）
- 文件监控断开后自动重连 + 全量扫描补偿

### NFR-05: 可扩展性

- 支持至少 100 个注册仓库（性能可适当降低）
- 语言检测支持扩展新的文件扩展名映射
- 文档模板系统支持用户自定义

---

## 5. 与其他模块的接口约定

### 5.1 与记忆引擎 (Memory Service) 的接口

| 交互方向 | 接口 | 说明 |
|---|---|---|
| CodeRepo → Memory | `reindex_note(note_path)` | 笔记关联或文档生成后，通知记忆引擎重新索引该笔记 |
| Memory → CodeRepo | `get_repo_context(note_path)` | 记忆引擎在索引笔记时，查询该笔记关联的仓库信息，作为记忆的附加上下文 |

**约定**:
- `generate_docs` 生成的文档笔记会被记忆引擎自动索引（通过文件监控触发）
- 文档笔记的 memory 携带 `source_repo` 标签，方便通过仓库名检索相关记忆

### 5.2 与时间线 (Timeline Service) 的接口

| 交互方向 | 接口 | 说明 |
|---|---|---|
| CodeRepo → Timeline | `emit_event(TimelineEvent)` | 仓库注册、文档生成时发送事件 |
| Timeline → CodeRepo | `get_commits_in_range(start, end)` | 时间线查询时获取指定日期范围内的 commit 事件 |

**事件类型**:

```rust
// 仓库注册事件
TimelineEvent {
    date: today,
    event_type: EventType::RepoRegistered,
    title: "注册代码仓库: {name}",
    summary: "路径: {path}, 语言: {primary_language}",
    tags: vec!["code-repo", "setup"],
    related_paths: vec![repo_path],
}

// 仓库 commit 事件（从 git 历史同步）
TimelineEvent {
    date: commit_date,
    event_type: EventType::RepoCommit,
    title: "{repo_name}: {commit_message}",
    summary: "分支: {branch}, 作者: {author}",
    tags: vec!["code-repo", "commit", repo_name],
    related_paths: vec![repo_path],
}

// 仓库文档化事件
TimelineEvent {
    date: today,
    event_type: EventType::RepoDocumented,
    title: "生成文档: {repo_name}",
    summary: "文档路径: {doc_path}",
    tags: vec!["code-repo", "documentation"],
    related_paths: vec![doc_path],
}
```

### 5.3 与灵感熔炉 (Inspiration Service) 的接口

| 交互方向 | 接口 | 说明 |
|---|---|---|
| Inspiration → CodeRepo | `get_all_repo_names()` | 灵感熔炉获取仓库名称列表，作为概念池的素材 |
| Inspiration → CodeRepo | `get_repo_tags(repo_name)` | 获取仓库的主要技术关键词，用于概念距离计算 |

### 5.4 与文件监控 (FileWatcher) 的接口

| 交互方向 | 接口 | 说明 |
|---|---|---|
| CodeRepo → FileWatcher | `watch(path, callback)` | 注册 `.git/HEAD` 文件监控 |
| CodeRepo → FileWatcher | `unwatch(path)` | 取消文件监控（仓库注销时） |
| FileWatcher → CodeRepo | `on_change(path)` | 文件变更回调，触发元信息刷新 |

### 5.5 与 LLM Client 的接口

| 交互方向 | 接口 | 说明 |
|---|---|---|
| CodeRepo → LlmClient | `generate(prompt, max_tokens)` | 文档生成时调用 LLM |

### 5.6 与配置系统 (Config) 的接口

CodeRepo 读取以下配置项：

```toml
[code_repo]
refresh_interval_seconds = 300     # 定时刷新间隔
language_refresh_interval_seconds = 3600  # 语言统计刷新间隔
max_recent_commits = 20            # 详情中展示的最近 commit 数
doc_target_dir = "code-docs"       # 默认文档输出目录
doc_template_path = "config/doc_template.md"  # 文档模板路径
auto_suggest_enabled = true        # 是否启用自动关联建议
auto_suggest_threshold = 5         # 自动关联建议的匹配阈值
exclude_dirs = [".git", "node_modules", "target", "vendor", "__pycache__", ".venv", "dist", "build"]
language_sample_max_files = 500    # 语言统计最大采样文件数
```

---

## 6. 模板定义

### 6.1 笔记引用块 Markdown 模板

当调用 `link_note_to_repo` 时，在笔记末尾插入以下内容：

```markdown

---
## 🔗 相关代码仓库
- **{repo_name}** — `{repo_path}`
  - [在 VSCode 中打开](vscode://file/{repo_path})
  - 最后活动: {last_activity_date} | 分支: {current_branch}
```

**模板变量说明**:

| 变量 | 来源 | 示例 |
|---|---|---|
| `{repo_name}` | 仓库注册名称 | `my-app` |
| `{repo_path}` | 仓库绝对路径 | `/Users/me/projects/my-app` |
| `{last_activity_date}` | 最新 commit 日期 | `2026-05-28` |
| `{current_branch}` | 当前分支名 | `main` |

**多次关联**: 若笔记已有关联其他仓库，新关联追加到已有的列表中：

```markdown

---
## 🔗 相关代码仓库
- **repo-a** — `/Users/me/projects/repo-a`
  - [在 VSCode 中打开](vscode://file/Users/me/projects/repo-a)
  - 最后活动: 2026-05-27 | 分支: main
- **repo-b** — `/Users/me/projects/repo-b`
  - [在 VSCode 中打开](vscode://file/Users/me/projects/repo-b)
  - 最后活动: 2026-05-28 | 分支: develop
```

### 6.2 文档生成 LLM Prompt 模板

```
你是一个项目文档生成助手。请根据以下代码仓库的信息，生成一份结构化的项目文档。
文档将保存为 Obsidian Markdown 笔记。

## 仓库基本信息
- 名称: {repo_name}
- 路径: {repo_path}
- 当前分支: {current_branch}
- 语言构成: {language_stats}
- 最近活动: {last_activity}

## 目录结构
```
{directory_tree}
```

## README 内容
{readme_content}

## 项目配置文件
### {config_file_name} (如 Cargo.toml / package.json)
```
{config_file_content}
```

## 核心源文件头部注释
{source_file_headers}

## 最近提交历史
{recent_commits_text}

---

请生成一份 Markdown 格式的项目文档，包含以下章节：

1. **项目概述**：基于 README 和项目配置，总结项目的目的、核心功能和定位
2. **技术栈**：列出使用的主要语言、框架和工具
3. **目录结构说明**：解释主要目录和文件的用途
4. **核心模块**：识别并描述项目的核心模块/组件及其职责
5. **依赖列表**：从项目配置文件中提取关键依赖及其用途
6. **开发状态**：基于最近提交历史，总结当前的开发动态

要求：
- 使用中文撰写
- 保持简洁，每个章节 3-10 行
- 在适当位置使用 Obsidian 标签（如 #project, #{primary_language}）
- 在文档开头添加 YAML frontmatter，包含:
  - title: {repo_name} 项目文档
  - source_repo: {repo_path}
  - generated_at: {current_timestamp}
  - head_commit: {head_hash}
  - tags: [project-doc, {primary_language}]
```

### 6.3 文档模板文件 (`config/doc_template.md`)

用户可自定义的文档模板，引擎在生成文档时将用户模板附加到 prompt 末尾作为格式参考：

```markdown
---
title: "{{repo_name}} 项目文档"
source_repo: "{{repo_path}}"
generated_at: "{{generated_at}}"
head_commit: "{{head_hash}}"
tags:
  - project-doc
  - {{primary_language}}
---

# {{repo_name}} 项目文档

> 自动生成于 {{generated_at}}，基于 commit {{head_hash_short}}

## 📋 项目概述

{由 LLM 生成}

## 🛠 技术栈

{由 LLM 生成}

## 📁 目录结构说明

{由 LLM 生成}

## 🧩 核心模块

{由 LLM 生成}

## 📦 依赖列表

{由 LLM 生成}

## 📊 开发状态

{由 LLM 生成}
```

---

## 7. 约束与假设

### 7.1 约束

| 编号 | 约束 | 说明 |
|---|---|---|
| C-01 | 仅支持 Git 仓库 | 不支持 SVN、Mercurial 等其他 VCS |
| C-02 | 仅支持本地仓库 | 不支持远端仓库（需先 clone 到本地） |
| C-03 | 仓库路径需为绝对路径 | 相对路径在注册时转换为绝对路径 |
| C-04 | 仓库名称全局唯一 | SQLite 主键约束 |
| C-05 | 单用户系统 | 无多用户权限管理 |
| C-06 | VSCode 集成为非保证功能 | 仅尝试唤起，不保证 VSCode 已安装 |
| C-07 | 文档生成依赖 LLM | 需要可用的 LLM API（OpenAI/Claude/Ollama） |

### 7.2 假设

| 编号 | 假设 | 说明 |
|---|---|---|
| A-01 | 用户本地已安装 Git | git2 crate 依赖 libgit2，无需 Git CLI |
| A-02 | 仓库为合法 Git 仓库 | 至少有一个 commit（bare 仓库可注册但信息有限） |
| A-03 | 用户对仓库路径有读写权限 | 注册和元数据提取需要读权限，笔记关联需要对 vault 有写权限 |
| A-04 | 仓库数量在合理范围内 | 预期 < 50 个，极端情况支持 100 个 |
| A-05 | Obsidian vault 路径已配置 | 笔记关联和文档输出依赖 vault 路径配置 |
| A-06 | 网络环境可访问 LLM API | 文档生成功能需要网络（使用本地 Ollama 时无此要求） |
| A-07 | 仓库不会频繁移动路径 | 路径变更需重新注册 |

---

## 8. 验收标准

### AC-01: 仓库注册验收

- [ ] 合法 Git 仓库注册成功，返回完整元信息
- [ ] 非法路径返回 `PATH_NOT_FOUND` 错误
- [ ] 非 Git 目录返回 `NOT_A_GIT_REPO` 错误
- [ ] 重复名称返回 `NAME_DUPLICATED` 错误
- [ ] 重复路径返回 `PATH_DUPLICATED` 错误
- [ ] 注册后 SQLite `code_repos` 表中有对应记录
- [ ] 注册后 `.git/HEAD` 文件监控已设置

### AC-02: 仓库列表验收

- [ ] 返回所有已注册仓库的卡片信息
- [ ] 每个仓库包含：name、path、current_branch、latest_commit、is_dirty、languages、linked_notes_count
- [ ] 不可访问的仓库标记为 `status: inactive`
- [ ] 空列表时返回空数组而非错误
- [ ] 5 个仓库以内响应延迟 < 500ms

### AC-03: 仓库详情验收

- [ ] 返回完整详情，包含 recent_commits（最多 20 条）
- [ ] branches 列表包含所有本地分支
- [ ] working_dir_status 包含 modified/added/deleted/untracked 文件计数
- [ ] linked_notes 包含所有关联笔记路径
- [ ] vscode_uri 格式正确

### AC-04: 笔记关联验收

- [ ] 关联成功后笔记末尾包含标准引用块
- [ ] 引用块包含仓库名、路径、VSCode 链接、最后活动日期、分支名
- [ ] 重复关联不重复插入引用块
- [ ] `note_repo_links` 表中有对应记录
- [ ] 记忆引擎被通知重新索引该笔记

### AC-05: 自动文档化验收

- [ ] 生成的文档包含：项目概述、技术栈、目录结构说明、核心模块、依赖列表、开发状态
- [ ] 文档文件有正确的 YAML frontmatter
- [ ] 文档被写入指定目标路径（默认 `code-docs/`）
- [ ] 文档与仓库自动建立关联
- [ ] 文档被记忆引擎索引
- [ ] LLM 调用失败时返回明确错误

### AC-06: VSCode 集成验收

- [ ] macOS 上通过 `open` 命令成功唤起 VSCode
- [ ] 返回的 vscode_uri 格式为 `vscode://file/<absolute_path>`
- [ ] VSCode 未安装时返回 URI 供手动使用
- [ ] 仓库路径不存在时返回明确错误

### AC-07: 状态监控验收

- [ ] `.git/HEAD` 变更后 < 5 秒触发缓存刷新
- [ ] 定时刷新按配置间隔执行
- [ ] 仓库路径失效后标记为 inactive
- [ ] 路径恢复后自动恢复为 active
- [ ] 单个仓库刷新失败不影响其他仓库

### AC-08: 跨模块集成验收

- [ ] 仓库注册事件被时间线记录
- [ ] commit 事件被时间线记录并可按日期范围查询
- [ ] 文档生成事件被时间线记录
- [ ] 灵感熔炉可获取仓库名称和技术关键词
- [ ] 记忆引擎可索引生成的文档并携带 source_repo 标签
