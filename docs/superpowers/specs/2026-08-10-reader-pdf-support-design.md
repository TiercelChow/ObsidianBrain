# 阅境轩 PDF 阅读支持 — 设计文档

> 日期：2026-08-10
> 状态：已通过 brainstorming，待 writing-plans
> 范围：在阅境轩（Reader）中新增 PDF 文件阅读，显示效果与 markdown 保持一致（背景/文字颜色随主题），保留 PDF 原版式。

---

## 1. 背景与目标

阅境轩当前仅支持本地 markdown 文件的阅读（`frontend/src/views/Reader.vue`）。用户希望新增 PDF 文件阅读支持，且"显示效果和 markdown 保持一致"——经澄清，指 **背景与文字颜色随应用主题（light / dark / eye-care）变化**，与 markdown 阅读体验统一。

PDF 是固定版式格式，markdown 是可回流格式。经方案选择，采用 **pdf.js 按原版式渲染** + **CSS 滤镜随主题调色**：保留 PDF 的图表/公式/表格/版式，同时让背景/文字颜色跟随主题。深色模式下彩色图片会被反色（CSS `invert` 滤镜的固有代价，已确认接受）。

### 非目标（YAGNI）

- PDF 全文搜索（markdown 也没有）
- PDF 内超链接拦截（pdf.js 自处理）
- PDF 注释 / 表单填写
- 深色模式图片反反色
- PDF → markdown 转换

---

## 2. 现状分析

### 现有 markdown 管道

1. 打开文件夹 → `list_local_dir` 工具返回 `DirEntry` 树，含 `is_markdown` 标记（`.md` / `.markdown`）。
2. FileTree 仅让 `is_markdown` 文件可点击；非 md 文件 disabled/greyed。
3. 点击文件 → `onSelectFile(path)` → `readLocalFile(path)` 工具。
4. 后端 `read_local_file` 把字节当 **UTF-8 文本** 返回（`String::from_utf8_lossy`），5 MB 上限。
5. 前端 `renderMarkdown()`（marked + KaTeX + mermaid + highlight.js）→ `v-html` 注入 `<article class="markdown-body">`。
6. 富外壳：3 栏（文件树 / 内容 / TOC）、主题、全屏沉浸、翻页过渡（directional slide）、scroll-spy TOC、服务端存储历史（pin/rename）、跨文件链接跳转。

### 关键约束

- `read_local_file` 返回 UTF-8 文本 → **对二进制 PDF 会损坏**，必须新增二进制端点。
- 已有二进制端点模式可参考：`GET /v1/vault/images/*path`（`serve_vault_image`）返回原始字节 + content-type，但仅作用于 Obsidian vault 路径，不适用于 reader 使用的任意本地路径。

### 后端路由现状

`backend/src/api/router.rs` 现有路由：`/health`、`/tools`、`/tools/call`、`/upload/images`、`/vault/images/*path`、`/vault/thumbnails/*path`。无面向 reader 的二进制文件端点。

---

## 3. 总体方案

用 **pdf.js** 把 PDF 按页渲染成 canvas，嵌入阅境轩现有 3 栏外壳，共享主题、全屏、历史、翻页过渡。主题调色用 CSS `filter` 作用于 canvas 容器。

### 主题调色策略

| 主题 | CSS filter | 效果 |
|------|-----------|------|
| light | 无 | PDF 原样（白底黑字，与 markdown 浅色一致）|
| dark | `invert(1) hue-rotate(180deg)` | 黑底白字，与 markdown 深色一致（彩色图片会被反色）|
| eye-care | `sepia(0.4) brightness(0.96) saturate(0.85)` | 米黄底，与护眼主题一致 |

主题切换只需改 CSS class，**无需重新渲染 canvas**（pdf.js canvas 不变，滤镜实时生效）。

---

## 4. 后端改动（Rust / Axum）

### 4.1 新增二进制端点

`GET /v1/reader/raw?path=<abs>`

- 仿照 `serve_vault_image` 模式，但作用于**任意本地路径**（reader 的作用域即本地文件系统，与 `list_local_dir` / `read_local_file` 一致）。
- 返回原始字节 + 按 ext 设 `Content-Type`（`.pdf` → `application/pdf`）。
- **支持 HTTP Range 请求**：返回 `206 Partial Content` + `Content-Range`。pdf.js 用 range 请求高效加载大 PDF，不必整包下载。
- 路径安全：必须是绝对路径、拒绝 `..` 穿越、必须是文件。
- size 上限放宽到 ~100 MB（PDF 普遍大于 5 MB 的 `MAX_FILE_SIZE`）。
- 新文件：`backend/src/api/handlers/reader_file.rs`；在 `router.rs` 注册 `GET /reader/raw`，置于 `api_routes` 中。
- 仅 `127.0.0.1`，无 auth——与现有 vault image 端点一致。

### 4.2 文件树标记 PDF

- `backend/src/tools/handlers/reader_handlers.rs` 的 `DirEntry` 增加 `is_pdf: bool`（与现有 `is_markdown` 并列，风格一致）。
- `build_tree` 对 `.pdf`（不区分大小写）设 `is_pdf = true`。
- `list_local_dir` 测试补充 PDF 节点。

### 4.3 不复用 read_local_file

PDF 不走 `read_local_file` 工具（其 UTF-8 文本返回会损坏二进制）。前端通过 `GET /v1/reader/raw` 直接以 URL 形式喂给 pdf.js。

---

## 5. 前端改动（Vue）

### 5.1 新组件 `frontend/src/components/reader/PdfViewer.vue`

- 依赖：`pdfjs-dist`。
- worker：用 Vite `?url` 导入（`import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'`），设 `GlobalWorkerOptions.workerSrc`。
- 加载：`getDocument('/v1/reader/raw?path=...')`。
- 渲染：页面竖向堆叠，每页一个 `<canvas>` + 文字层（pdf.js `TextLayer`，支持文字选择/搜索命中高亮）。
- **懒渲染**：`IntersectionObserver` 只渲染可视区 ± 缓冲页；占位 div 按页面尺寸预留高度，避免滚动跳动。大 PDF 不卡。
- **fit-width 默认**：缩放 = 容器宽 / 页面原生宽；渲染按 `devicePixelRatio` 保证清晰。
- **缩放**：PDF 激活时顶栏出现 `− / fit / +` 小控件 + 支持 Ctrl+滚轮。markdown 无此控件，但 PDF 固定版式需要——属合理的 PDF 专属 affordance。
- `getOutline()` → emit `outline` 给 Reader 填充 TOC。结构 `{text, level, dest}`，其中 `dest` 是 pdf.js outline 项的原始目标（用于解析页码）。**注意**：PDF 的 TOC 项无 DOM `id`（与 markdown 标题的 slug id 不同），滚动目标由 `dest` 解析为页码后定位，不复用 md 的 `scrollToHeading(id)` 机制。
- 监听 `appStore.theme` → 给 canvas 容器加对应 filter class（`pdf-theme-dark` / `pdf-theme-eye-care` 等）。

### 5.2 Reader.vue 改造

- `onSelectFile`：按 ext 分流——`.pdf` 走 PdfViewer，`.md` 走现有 markdown 路径。
- 在同一个翻页 `<transition>` 内用 `v-if/v-else` 切换 `<article v-html>` vs `<PdfViewer>`，保留 page-turn 过渡。
- `flatFiles`：纳入 `.pdf` 文件，翻页方向判定同样适用（markdown 与 pdf 混排时按树序判断 prev/next）。
- TOC：PdfViewer emit outline → 填充 `toc`（结构 `{text, level, dest}`，无 `id`）。点击 → 由 `dest` 解析页码 → 滚到对应页。无 outline 时显示"无目录"。复用右栏 TOC UI 样式。注意 md TOC 项 `{id, text, level}` 与 pdf TOC 项 `{text, level, dest}` 结构不同——Reader 内以可选字段兼容（`id?` / `dest?`），点击处理按存在字段分派。
- 历史 / last-file：path 机制本就与格式无关，自动适用。
- `displayedFile` / `transitionDir` 机制对 PDF 同样生效。

### 5.3 FileTree.vue 改造

- `.pdf` 文件可点击（不再 disabled）。
- 用专属图标区分 PDF 与 markdown（如 PDF 用独立图标，markdown 用现有 `Document`）。

### 5.4 api/reader.ts 改造

- `DirEntry` 接口增加 `is_pdf: boolean`。
- 新增 `localFileUrl(path: string): string` 辅助函数，返回 `/v1/reader/raw?path=<encodeURIComponent(path)>`。

### 5.5 markdown 内 PDF 链接

- markdown 中指向 `.pdf` 的相对链接（在 root 下）→ 直接在阅读器打开（与 `.md` 跳转逻辑一致）。
- `handleLinkClick` 中把 `.pdf` 也路由到主阅读器（在 root 下时）；`PathPreviewModal` 的路由决策同步纳入 `.pdf`。

---

## 6. 数据流

```
打开文件夹 → list_local_dir（含 is_pdf）→ FileTree 显示 .pdf 可点击
点击 .pdf  → onSelectFile 检测 .pdf → 设 fileKind='pdf'
           → <PdfViewer :src="/v1/reader/raw?path=...">
           → pdfjs.getDocument → 逐页 canvas + 文字层
           → getOutline() → emit → Reader 填 TOC
主题切换   → PdfViewer canvas 容器换 filter class → 颜色实时变（不重渲染 canvas）
TOC 点击   → 解析 dest → 滚到对应页
全屏       → PdfViewer 撑满 center pane（与 markdown 全屏一致）
翻页       → flatFiles 含 .pdf，prev/next 方向按树序
```

---

## 7. 错误处理

| 场景 | 表现 |
|------|------|
| 文件不存在 / 不可访问 | 端点 404 → 前端 error banner |
| PDF 损坏 / 加密 | pdf.js 抛错 → "PDF 解析失败" banner |
| 路径含 `..` / 非绝对路径 | 端点 400 |
| 文件超 size 上限 | 端点 413 |

复用现有 `loadbar`（加载条）+ `error-banner`（错误条）UI。

---

## 8. 测试策略（遵循 CLAUDE.md TDD / 零 warning）

### 后端

- **路径校验**：拒 `..` 穿越、要求绝对路径、文件必须存在、必须是文件（非目录）。
- **content-type 映射**：`.pdf` → `application/pdf`。
- **Range 请求**：带 `Range` header → 返回 206 + `Content-Range` + 正确字节切片；不带 → 200 全量。
- **size 上限**：超限 → 413。
- 用小 PDF fixture（或构造最小合法 PDF 字节）。

### 前端

- **PdfViewer**：mock pdfjs 测页面渲染调用、outline emit、主题 filter class 切换、懒渲染（IntersectionObserver 触发）。
- **Reader.vue**：md/pdf 分流正确、`flatFiles` 含 pdf、TOC 来自 outline、翻页方向含 pdf。

---

## 9. 涉及文件清单

### 新增

- `backend/src/api/handlers/reader_file.rs` — 二进制端点 + Range 支持
- `frontend/src/components/reader/PdfViewer.vue` — pdf.js 渲染组件
- `docs/superpowers/specs/2026-08-10-reader-pdf-support-design.md` — 本文档

### 修改

- `backend/src/api/handlers/mod.rs` — 注册 reader_file 模块
- `backend/src/api/router.rs` — 注册 `GET /reader/raw`
- `backend/src/tools/handlers/reader_handlers.rs` — `DirEntry` 加 `is_pdf`，`build_tree` 标记 PDF，测试补充
- `frontend/src/views/Reader.vue` — md/pdf 分流、PdfViewer 集成、TOC 来自 outline、flatFiles 含 pdf、.pdf 链接跳转
- `frontend/src/components/reader/FileTree.vue` — .pdf 可点击 + 专属图标
- `frontend/src/components/reader/PathPreviewModal.vue` — .pdf 路由到主阅读器
- `frontend/src/api/reader.ts` — `DirEntry` 加 `is_pdf`、`localFileUrl` 辅助
- `frontend/package.json` — 新增 `pdfjs-dist` 依赖

---

## 10. 范围边界小结

**纳入**：本地任意文件夹下的 `.pdf`；pdf.js 逐页渲染；主题滤镜调色；PDF outline 作 TOC；fit-width + 缩放；全屏；历史；翻页过渡；markdown 内 .pdf 链接跳转。

**不纳入**：PDF 全文搜索；PDF 内超链接拦截；PDF 注释/表单；深色模式图片反反色；PDF→markdown 转换。
