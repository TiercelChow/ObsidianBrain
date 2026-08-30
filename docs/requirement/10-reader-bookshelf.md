# 阅境轩·书架 (Reader Bookshelf) — 需求设计文档

> **文档编号**: REQ-10 | **版本**: v1.4 | **状态**: 已实现 | **最后更新**: 2026-08-31
>
> **上游模块**: 阅境轩（本地 Markdown/PDF 阅读器，无独立模块文档，代码见 `frontend/src/views/Reader.vue`）
> **参考实现**: 阅读历史（`reader_history`，`app_state` JSON 存储）；任务中枢视图切换（`frontend/src/views/Tasks.vue` `.view-switch`）

---

## 1. 模块概述

### 1.1 定位

阅境轩新增**书架视图**：把本地文件夹（Markdown 文集）或单个 PDF 文件登记为「一本书」，附书名、描述、类别；点击书卡进入现有阅读界面，并自动恢复上次读到的位置（文件 + 滚动比例 / PDF 页码）。

书架与阅读共用 `/reader` 路由，通过头部滑块切换（复刻任务中枢「任务/日历」模式）。书架是新的默认落地视图。

### 1.2 核心价值

| 用户痛点 | 解决方式 |
|---|---|
| 反复要输入/翻历史找同一个文件夹或 PDF | 登记为书，一步直达 |
| 打开后不记得上次读到哪个文件、哪一页 | 每本书独立记住文件 + 位置，打开即恢复 |
| 阅读材料缺乏整理维度 | 书名 + 描述 + 自由类别标签，可按类筛选 |
| 换浏览器/清缓存丢配置 | 书架存服务端 SQLite，与阅读历史同模式 |

### 1.3 核心使用场景

1. **藏书**：用户把一个 PDF 或一个 Markdown 文件夹加为书，命名、写描述、归类别。
2. **开读即续读**：点书卡进入阅读视图，自动定位到上次阅读的文件和位置；PDF 直接翻到上次页码。
3. **整理**：按类别标签筛选书架；编辑书的信息；删除不再需要的书。
4. **多本并行**：几本书交替读，各自的进度互不干扰。

---

## 2. 范围与关键决策

### 2.1 已确认决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 存储 | 服务端 SQLite `app_state` 表，JSON key `reader_books` | 与 `reader_history` 同模式，零迁移；跨浏览器保留 |
| 进度粒度 | 文件 + 位置（文件夹书：文件 + 滚动比例；PDF：页码） | 打开即回到上次看到的地方 |
| 类别 | 自由文本标签 | 灵活；顶部 chips 按类筛选 |
| 视图组织 | Reader 内部 `viewMode`（`shelf` / `read`），非子路由 | 完全复刻任务中枢模式；切换不销毁阅读状态 |
| 默认视图 | 书架（选择记入 localStorage，同步 `?view=` query） | 书架是新入口；记住用户选择 |

### 2.2 MVP 范围

1. 书架视图：卡片网格、类别筛选、添加/编辑/删除书。
2. 后端工具：`get_reader_books` / `save_reader_books`（整体替换，同 history 模式）。
3. 阅读进度：自动记录（防抖）+ 打开恢复 + 失效静默回退。
4. 头部视图切换滑块，样式与任务中枢一致。
5. 桌面与手机端可用。

### 2.3 非目标（后续扩展）

- PDF 首页缩略图封面。
- 书内全文搜索、书签/笔记。
- 阅读时长统计、进度百分比条形可视化。
- 书架排序自定义（MVP 固定按最近阅读倒序）。

---

## 3. 领域模型

```ts
interface ReaderBook {
  id: string              // 唯一标识：`${Date.now().toString(36)}-${随机 4 位}`，创建后不变
  path: string            // 绝对路径：文件夹（文集书）或 .pdf 文件（单本 PDF）
  kind: 'folder' | 'pdf'  // 由 path 推导但显式存储
  name: string            // 书名；默认取文件/文件夹名
  description: string     // 描述，可空
  category: string        // 类别标签，可空
  addedAt: number         // 收藏时间戳 (ms)
  progress?: BookProgress // 无则视为未开始
}

interface BookProgress {
  lastFile: string | null // folder 书：上次打开的 md 绝对路径；pdf 书恒为 null
  position: number        // folder 书：lastFile 内滚动比例 0~1；pdf 书：页码 (≥1)
  pageCount: number       // pdf 书：总页数（卡片进度文案用）；md 书省略
  updatedAt: number       // 进度更新时间戳 (ms)
}
```

不变式：

- `kind === 'pdf'` ⇔ `path` 以 `.pdf` 结尾（不区分大小写）；`kind === 'folder'` ⇔ 其余。
- `progress.position`：md 书 ∈ [0,1]；pdf 书为 ≥1 整数。
- 同一 `path` 不允许重复登记（添加时校验，后端不强制）。
- `id` 是唯一身份；重命名、改路径外的字段都不变。

---

## 4. 功能需求

### 4.1 书架视图

- **FR-1 卡片网格**：桌面自适应多列（`repeat(auto-fill, minmax(240px, 1fr))`），手机两列。卡片显示：书名、类别 chip（有则显示）、描述（两行截断）、进度提示、路径（title tooltip）。
- **FR-2 进度提示文案**：folder 书「读到 42%」；pdf 书「第 12/180 页」（`pageCount` 缺失时「第 12 页」）；未开始「未开始」。百分比四舍五入取整。
- **FR-3 排序**：默认按 `progress.updatedAt ?? addedAt` 倒序（最近读的在前）。
- **FR-4 类别筛选**：顶部 chips = 「全部」+ 去重后的类别集合；点选过滤；默认「全部」。chips 单选。
- **FR-5 空态**：无书时显示引导文案 + 添加按钮。
- **FR-6 手机端**：卡片两列自适应，添加/编辑弹窗全屏化（同现有弹窗移动端模式）。

### 4.2 书的增删改

- **FR-7 添加**：「+ 添加」按钮 → 弹窗表单：路径（必填）、书名（默认文件名）、描述、类别。路径失焦/提交时用 `stat_local_path` 校验：必须存在且（`is_dir` 或 `.pdf` 文件），否则表单报错不允许提交。
- **FR-8 重复校验**：路径已登记时提示「已在书架」，不允许重复添加。
- **FR-9 编辑**：卡片悬浮「编辑」→ 同一弹窗预填；可改全部字段（含路径，路径改动同样走 FR-7 校验）。
- **FR-10 删除**：卡片悬浮「删除」→ 确认对话（el-messagebox 或同风格确认）→ 删除（进度随之消失，不删磁盘文件）。
- **FR-11 保存时机**：增/删/改成功后立即整体保存（`save_reader_books`）；保存失败提示错误且本地状态回滚。

### 4.3 阅读与恢复

- **FR-12 打开书**：点卡片主体 → 切到阅读视图 → 打开 `path`（folder：`openPath`；pdf：直接选中该文件）→ 恢复进度。
- **FR-13 md 恢复**：先选中 `progress.lastFile`（文件存在时），渲染完成后将 pane-center 滚到 `scrollHeight` 比例位置；`lastFile` 不存在（被删/移动）→ 从文件夹首个文件、顶部开始，静默回退。
- **FR-14 pdf 恢复**：渲染完成后 `scrollToPage(position)`；页码超出当前总页数 → 收敛到末页。
- **FR-15 进度记录**：阅读视图内，当前打开的文件/文件夹命中某本书时：
  - md：pane-center 滚动防抖 1.5s 记 `position = scrollTop / (scrollHeight - clientHeight)`（分母 0 时记 0）；
  - pdf：`pagechange` 事件记页码，`pagecount` 事件记总页数；
  - `lastFile` 在文件打开时记录。
- **FR-16 落盘时机**：防抖到期、切换文件、切视图、离开路由（`onBeforeUnmount`/route watch）时保存。
- **FR-17 并存**：现有 `reader.lastFolder`/`reader.lastFile` localStorage 与历史记录机制不变；书架进度是独立体系。

### 4.4 视图切换

- **FR-18 滑块**：`page-header` 右侧 `.view-switch`（书架｜阅读），样式复刻任务中枢（switch-indicator 滑块动画）。
- **FR-19 状态持久**：viewMode 写 localStorage `reader.view` 并同步 `?view=` query；进入时优先 query，其次 localStorage，默认 `shelf`。
- **FR-20 切换保活**：切到书架再切回阅读，已打开的文件夹/文件/滚动位置不丢失（同一组件树，v-show 切换）。

---

## 5. 存储规范

- 表：`app_state`（现有），key = `reader_books`，value = `ReaderBook[]` JSON 序列化（camelCase 字段，同 history 模式）。
- 读取失败（JSON 损坏）→ 视为空列表（同 history 的 `unwrap_or_default`）。
- 无迁移、无新表；并发写覆盖语义与 history 一致（整体替换，单用户本地场景可接受）。

## 6. Tool API

| 工具 | 入参 | 出参 | 说明 |
|---|---|---|---|
| `get_reader_books` | `{}` | `{ books: ReaderBook[] } | { status: 'error', error }` | 读书架 |
| `save_reader_books` | `{ books: ReaderBook[] }` | `{ status: 'success' } | error` | 整体替换保存 |

- Schema 校验 `books` 数组元素必填字段：`id/path/kind/name/addedAt`（`description/category/progress` 可选）。
- 注册于 reader 模块（`reader_handlers.rs`），走现有 `/v1/tools/call`。

## 7. 交互与视觉

- 滑块、chips、卡片风格对齐现有玻璃拟态体系（glass-surface、圆角、阴影变量）。
- 卡片悬浮显示操作（编辑/删除），桌面 hover、手机长按或常显小图标（取常显，简单可靠）。
- 添加/编辑弹窗复用 Element Plus 表单控件（el-input/el-button），移动端全屏（复用 PathPreviewModal 的移动端处理思路）。

## 8. 非功能需求

- 书架列表 ≤ 数百本规模，整存整取无性能问题；防抖保证滚动时无频繁网络写。
- 进度恢复在图片/增强渲染后仍需准确：恢复滚动在 `enhance` 完成后执行；如布局仍变化（图片懒加载），比例位置可能有小偏差，可接受（MVP 不做二次校正）。

## 9. 验收标准

1. 添加一个文件夹书与一个 PDF 书，信息正确显示；重复路径被拒绝；非法路径被拒绝。
2. 阅读 md 书滚动到中部 → 切书架 → 重进 → 恢复到同比例位置（偏差 ≤ 2%）。
3. PDF 书翻到第 N 页 → 重进 → 直接定位第 N 页。
4. 删除书后再阅读，进度不再记录（无对应书条目）。
5. 刷新页面/换 query `?view=read` 进入，视图与选择符合 FR-19。
6. 换一个浏览器访问同一服务，书架数据完整（服务端存储）。
7. 现有阅读功能（历史、路径打开、沉浸模式、移动端）回归无异常。
8. `cargo test`（新 handler 单测）与 `npm test`（进度换算纯函数）全绿。

## 10. 后续扩展

见 2.3 非目标。

## 11. 修订历史

| 版本 | 日期 | 说明 |
|---|---|---|
| v1.0 | 2026-08-28 | 初版：设计评审通过 |
| v1.1 | 2026-08-28 | 已实现。补充实现说明：md 进度恢复增加 1.8s 比例保持校正窗（恢复后图片/增强渲染仍会改变 scrollHeight，实测 0.5 → 0.37 漂移；用户任意滚动输入即取消校正），以满足验收标准 2 的 ±2% 精度 |
| v1.2 | 2026-08-30 | 视图切换移入常驻工具栏（任务中枢同款玻璃容器，移动端切换器独占整行），书架工具栏增加书籍搜索（书名/描述/类别/路径 不区分大小写子串匹配，纯前端过滤；无匹配时独立空态） |
| v1.3 | 2026-08-30 | 书架展示重设计：卡片改为"封面立于玻璃层板"（用户选定方案）。封面横排、书名取 `coverTone(name)` 哈希确定的 8 种深色调之一、左侧书脊阴影+前缘高光、底部进度条；层板由单个 repeating-linear-gradient 按固定行距绘制（末行不满也有层板）。悬停抽起并浮现编辑/删除（触屏常显）；描述与路径移入原生 tooltip。历史文件夹批量迁移入书架（`addedAt` 取 `lastUsed`，按路径去重幂等） |
| v1.4 | 2026-08-31 | 添加/编辑弹窗对齐应用弹窗语言：遮罩改为暗色磨砂（`modal-class` 挂 `.book-form-overlay`，压过 index.html 的白色 `--el-mask-color` 覆盖）、借 EP `dialog-fade` transition + motion.css 过渡实现面板弹簧缩放入场、移动端以更高优先级重申 App.vue 顶贴弹层圆角（22px 顶 / 底部直角）。面板玻璃样式沿用 index.html 的 Glass Dialog 全局规则。另：移动端封面比例与桌面一致（165×214 ≈ 177×230） |
