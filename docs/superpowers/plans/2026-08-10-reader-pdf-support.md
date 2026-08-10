# 阅境轩 PDF 阅读支持 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在阅境轩（Reader）中新增本地 PDF 文件阅读，用 pdf.js 按原版式渲染，背景/文字颜色随主题（light/dark/eye-care）变化，与 markdown 阅读体验统一。

**Architecture:** 后端新增二进制端点 `GET /v1/reader/raw?path=<abs>`（支持 HTTP Range，pdf.js 高效加载大 PDF）；`DirEntry` 增加 `is_pdf` 标记。前端新增 `PdfViewer.vue` 组件（pdf.js 逐页 canvas + 文字层 + 懒渲染 + outline + 缩放），`Reader.vue` 按 ext 分流 md/pdf，TOC 取自 PDF outline，主题用 CSS `filter` 作用于 canvas 容器（不重渲染）。

**Tech Stack:** Rust/Axum 0.7（后端，已有依赖，无需新增 crate）；Vue 3 + `pdfjs-dist@^4`（前端，新增依赖）；CSS `filter`（主题调色）。

## Global Constraints

- Rust edition 2021；`cargo fmt --check && cargo clippy -- -D warnings && cargo test` 每个 commit 必须通过。
- 生产代码禁止 `.unwrap()`/`.expect()`（测试除外）；错误统一 `BrainError` 或 `(StatusCode, String)`（axum handler）。
- 后端仅 `127.0.0.1`，无 auth（与现有 `serve_vault_image` 一致）。
- 前端无单元测试框架（package.json 无 vitest）——前端任务以 `npm run build`（`vue-tsc` 类型检查 + vite 构建）+ 手动验证为"测试"；后端任务用真实 `cargo test`。
- PDF 二进制端点不复用 `read_local_file`（其 UTF-8 文本返回会损坏二进制）。
- 主题调色 CSS filter：light=无；dark=`invert(1) hue-rotate(180deg)`；eye-care=`sepia(0.4) brightness(0.96) saturate(0.85)`。
- 路径安全：端点要求绝对路径、拒绝 `..` 穿越、必须是文件；size 上限 100 MB。
- 参考 spec：`docs/superpowers/specs/2026-08-10-reader-pdf-support-design.md`。

---

## File Structure

### 新增

- `backend/src/api/handlers/reader_file.rs` — 二进制端点 `serve_reader_file` + Range 支持 + 路径校验。单一职责：把任意本地文件以原始字节 + 正确 content-type 返回，支持 Range。
- `frontend/src/components/reader/PdfViewer.vue` — pdf.js 渲染组件。单一职责：加载并渲染一个 PDF，emit outline，暴露 `scrollToPage`。

### 修改

- `backend/src/api/handlers/mod.rs` — 注册 `reader_file` 模块。
- `backend/src/api/router.rs` — 注册 `GET /reader/raw`。
- `backend/src/tools/handlers/reader_handlers.rs` — `DirEntry` 加 `is_pdf`，`build_tree` 标记 PDF，测试补充。
- `frontend/package.json` — 新增 `pdfjs-dist@^4`。
- `frontend/src/api/reader.ts` — `DirEntry` 加 `is_pdf`，新增 `localFileUrl`。
- `frontend/src/components/reader/FileTree.vue` — `.pdf` 可点击 + 专属图标。
- `frontend/src/views/Reader.vue` — md/pdf 分流、PdfViewer 集成、TOC 来自 outline、`flatFiles` 含 pdf、翻页过渡、`.pdf` 链接跳转。
- `frontend/src/components/reader/PathPreviewModal.vue` — `.pdf` 分类 + open-in-reader 路由。

---

## Task 1: Backend — `DirEntry.is_pdf` 标记 PDF 文件

**Files:**
- Modify: `backend/src/tools/handlers/reader_handlers.rs`（`DirEntry` 结构体 ~L43-51、`is_markdown_ext` ~L53-56、`build_tree` 内 `is_markdown` 赋值 ~L83）
- Test: 同文件 `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `DirEntry` 新增字段 `is_pdf: bool`（`#[derive(serde::Serialize)]` 自动序列化，前端可见）。

- [ ] **Step 1: 写失败测试**

在 `reader_handlers.rs` 的 `tests` 模块中，给 `test_build_tree_returns_structure_and_skips_hidden` 补充 PDF 断言。先在 `make_tree` 里加一个 PDF 文件，再断言它被标记 `is_pdf`。

修改 `make_tree`（异步版 + 同步版两处都加）：

```rust
// 在 make_tree 中追加（异步版 ~L418-431 与同步版 ~L433-444 两处）
tokio::fs::write(root.join("doc.pdf"), b"%PDF-1.4 fake").await.unwrap();
// 同步版：
std::fs::write(root.join("doc.pdf"), b"%PDF-1.4 fake").unwrap();
```

在 `test_build_tree_returns_structure_and_skips_hidden` 末尾追加断言：

```rust
// 顶层现在有 3 个条目：sub (dir)、a.md、doc.pdf
assert_eq!(entries.len(), 3, "should list sub, a.md, doc.pdf");
let pdf = entries.iter().find(|e| e.name == "doc.pdf").expect("doc.pdf present");
assert!(pdf.is_pdf, "doc.pdf should be marked is_pdf");
assert!(!pdf.is_markdown);
assert!(!pdf.is_dir);
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --lib test_build_tree_returns_structure_and_skips_hidden`
Expected: 编译失败——`is_pdf` 字段不存在。

- [ ] **Step 3: 实现**

在 `is_markdown_ext` 后新增：

```rust
fn is_pdf_ext(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".pdf")
}
```

修改 `DirEntry` 结构体（在 `is_markdown` 后加字段）：

```rust
#[derive(serde::Serialize)]
struct DirEntry {
    name: String,
    path: String,
    is_dir: bool,
    is_markdown: bool,
    is_pdf: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<DirEntry>>,
}
```

修改 `build_tree` 中的 `filter_map`，计算 `is_pdf` 并赋值（~L83 附近）：

```rust
let is_markdown = !is_dir && is_markdown_ext(&name);
let is_pdf = !is_dir && is_pdf_ext(&name);
// ...
Some(DirEntry {
    name,
    path: entry_path.to_string_lossy().to_string(),
    is_dir,
    is_markdown,
    is_pdf,
    children,
})
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test --lib reader_handlers`
Expected: PASS（含 PDF 断言）。

- [ ] **Step 5: 质量检查 + 提交**

```bash
cd backend && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test --lib reader_handlers
git add backend/src/tools/handlers/reader_handlers.rs
git commit -m "feat(reader): mark .pdf files with is_pdf in DirEntry"
```

---

## Task 2: Backend — 二进制端点 `GET /v1/reader/raw` + Range 支持

**Files:**
- Create: `backend/src/api/handlers/reader_file.rs`
- Modify: `backend/src/api/handlers/mod.rs`（L1-3）
- Modify: `backend/src/api/router.rs`（L11, L24-32）
- Test: `backend/src/api/handlers/reader_file.rs` 内 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::AppContext`（仅作为 axum `State`，handler 内不使用其字段——与 `serve_vault_image` 一致）。
- Produces: `pub async fn serve_reader_file(State<Arc<AppContext>>, Query<ReaderRawQuery>, HeaderMap) -> Result<Response, (StatusCode, String)>`；路由 `GET /v1/reader/raw?path=<abs>`。前端以 URL 形式 `/v1/reader/raw?path=...` 喂给 pdf.js。

- [ ] **Step 1: 写失败测试**

创建 `backend/src/api/handlers/reader_file.rs`，先写测试（测试调用纯函数 `validate_path` 与 `parse_range`，避免起完整服务器）：

```rust
//! Reader binary file endpoint: `GET /v1/reader/raw?path=<abs>`.
//!
//! Serves arbitrary local files as raw bytes with correct Content-Type and
//! HTTP Range support (pdf.js uses range requests to stream large PDFs).
//! Path must be absolute, contain no `..`, and be a file. 100 MB cap.

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use crate::AppContext;

/// Max file size served by the reader binary endpoint (100 MB).
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ReaderRawQuery {
    pub path: String,
}

/// Validate a local path: absolute, no `..` traversal, exists, is a file.
fn validate_path(raw: &str) -> Result<PathBuf, (StatusCode, String)> {
    let p = PathBuf::from(raw);
    if !p.is_absolute() {
        return Err((StatusCode::BAD_REQUEST, "路径必须是绝对路径".to_string()));
    }
    for comp in p.components() {
        if matches!(comp, Component::ParentDir) {
            return Err((StatusCode::BAD_REQUEST, "路径禁止包含 `..`".to_string()));
        }
    }
    let meta = match std::fs::metadata(&p) {
        Ok(m) => m,
        Err(_) => return Err((StatusCode::NOT_FOUND, "文件不存在".to_string())),
    };
    if !meta.is_file() {
        return Err((StatusCode::BAD_REQUEST, "路径不是文件".to_string()));
    }
    Ok(p)
}

/// Content-Type by extension (only PDF is special-cased; others octet-stream).
fn content_type_for(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Parse a single-range `bytes=start-end` header into (start, end_inclusive).
/// `bytes=0-` (open-ended) and `bytes=0-99` are supported; suffix `bytes=-99` is
/// not (pdf.js uses start-end form). Returns None on malformed input.
fn parse_range(range: &str, total: u64) -> Option<(u64, u64)> {
    let bytes = range.strip_prefix("bytes=")?;
    let (s, e) = bytes.split_once('-')?;
    let start: u64 = s.parse().ok()?;
    let end: u64 = if e.is_empty() {
        total.saturating_sub(1)
    } else {
        e.parse().ok()?
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end.min(total - 1)))
}

/// `GET /v1/reader/raw?path=<abs>` — serve a local file as raw bytes with Range.
pub async fn serve_reader_file(
    State(_ctx): State<Arc<AppContext>>,
    Query(q): Query<ReaderRawQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let path = validate_path(&q.path)?;
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("读取元数据失败: {e}")))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "文件过大 ({:.1} MB)，上限 {} MB",
                meta.len() as f64 / 1_048_576.0,
                MAX_FILE_SIZE / 1_048_576
            ),
        ));
    }
    let total = meta.len();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let ct = content_type_for(ext);

    // Read full file into memory, then slice for Range. Acceptable for a
    // local single-user tool with a 100 MB cap; a future optimization can
    // use File::seek for true streaming.
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("读取文件失败: {e}")))?;

    if let Some(range_hdr) = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
    {
        if let Some((start, end)) = parse_range(range_hdr, total) {
            let slice = bytes[start as usize..=end as usize].to_vec();
            return Ok((
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE, ct.to_string()),
                    (header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}")),
                    (header::CONTENT_LENGTH, (end - start + 1).to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                ],
                slice,
            )
                .into_response());
        }
    }

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, ct.to_string()),
            (header::CONTENT_LENGTH, total.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(name: &str, content: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        (dir, p)
    }

    #[test]
    fn test_validate_path_rejects_relative() {
        let err = validate_path("relative/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_path_rejects_parent_dir() {
        let err = validate_path("/Users/x/../y/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_path_rejects_missing() {
        let err = validate_path("/nonexistent/absolute/path/file.pdf").unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_validate_path_accepts_existing_file() {
        let (_d, p) = write_tmp("file.pdf", b"%PDF-1.4");
        let got = validate_path(p.to_str().unwrap()).unwrap();
        assert_eq!(got, p);
    }

    #[test]
    fn test_content_type_pdf() {
        assert_eq!(content_type_for("pdf"), "application/pdf");
        assert_eq!(content_type_for("PDF"), "application/pdf");
        assert_eq!(content_type_for("txt"), "application/octet-stream");
    }

    #[test]
    fn test_parse_range_full_open() {
        // bytes=0- on a 1000-byte file → 0..=999
        assert_eq!(parse_range("bytes=0-", 1000), Some((0, 999)));
    }

    #[test]
    fn test_parse_range_partial() {
        assert_eq!(parse_range("bytes=100-199", 1000), Some((100, 199)));
    }

    #[test]
    fn test_parse_range_clamps_end() {
        // end beyond total clamps to total-1
        assert_eq!(parse_range("bytes=900-2000", 1000), Some((900, 999)));
    }

    #[test]
    fn test_parse_range_rejects_out_of_range_start() {
        assert_eq!(parse_range("bytes=2000-", 1000), None);
    }

    #[test]
    fn test_parse_range_rejects_malformed() {
        assert_eq!(parse_range("items=0-100", 1000), None);
        assert_eq!(parse_range("bytes=abc", 1000), None);
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cd backend && cargo test --lib reader_file`
Expected: 编译失败——模块未注册（`mod reader_file` 未声明）。

- [ ] **Step 3: 注册模块 + 路由**

修改 `backend/src/api/handlers/mod.rs`（追加一行）：

```rust
pub mod health;
pub mod reader_file;
pub mod tool_handler;
pub mod upload;
```

修改 `backend/src/api/router.rs`：在 import 中加 `serve_reader_file`，在 `api_routes` 中加路由。

import 行（L11 原 `use crate::api::handlers::upload::{serve_thumbnail, serve_vault_image, upload_images};`）改为：

```rust
use crate::api::handlers::reader_file::serve_reader_file;
use crate::api::handlers::upload::{serve_thumbnail, serve_vault_image, upload_images};
```

在 `api_routes` 的 `.route("/vault/thumbnails/*path", get(serve_thumbnail))` 后（L31 之后）加：

```rust
        .route("/reader/raw", get(serve_reader_file))
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cd backend && cargo test --lib reader_file`
Expected: PASS（全部 9 个测试）。

- [ ] **Step 5: 质量检查 + 提交**

```bash
cd backend && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test --lib
git add backend/src/api/handlers/reader_file.rs backend/src/api/handlers/mod.rs backend/src/api/router.rs
git commit -m "feat(reader): add GET /v1/reader/raw binary endpoint with Range support"
```

---

## Task 3: Frontend — 新增 pdfjs-dist 依赖 + api/reader.ts 改造

**Files:**
- Modify: `frontend/package.json`（dependencies）
- Modify: `frontend/src/api/reader.ts`（`DirEntry` 接口 ~L4-10，末尾追加 `localFileUrl`）

**Interfaces:**
- Produces: `DirEntry.is_pdf: boolean`；`localFileUrl(path: string): string` 返回 `/v1/reader/raw?path=<encoded>`。

- [ ] **Step 1: 安装 pdfjs-dist**

Run:
```bash
cd frontend && npm install pdfjs-dist@^4.0.0
```

确认 `package.json` 的 `dependencies` 中出现 `"pdfjs-dist": "^4.x.x"`。

- [ ] **Step 2: 改造 api/reader.ts**

修改 `DirEntry` 接口（L4-10，加 `is_pdf`）：

```ts
export interface DirEntry {
  name: string
  path: string
  is_dir: boolean
  is_markdown: boolean
  is_pdf: boolean
  children?: DirEntry[]
}
```

在文件末尾（L82 之后）追加：

```ts
/** Build the binary file URL for the reader endpoint (used by pdf.js). */
export function localFileUrl(path: string): string {
  return `/v1/reader/raw?path=${encodeURIComponent(path)}`
}
```

- [ ] **Step 3: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: `vue-tsc` 类型检查通过，vite 构建成功。注意：`FileTree.vue` 等消费 `DirEntry` 的地方可能因新必填字段 `is_pdf` 报类型错误——若 vue-tsc 报错，仅在这些读取处用 `entry.is_pdf ?? false` 兼容即可（Task 4 会正式使用该字段）。

- [ ] **Step 4: 提交**

```bash
git add frontend/package.json frontend/package-lock.json frontend/src/api/reader.ts
git commit -m "feat(reader): add pdfjs-dist dep + DirEntry.is_pdf + localFileUrl helper"
```

---

## Task 4: Frontend — FileTree.vue PDF 可点击 + 专属图标

**Files:**
- Modify: `frontend/src/components/reader/FileTree.vue`（template ~L20-31 file 分支、script import ~L47、`onFileClick` ~L88-90）

**Interfaces:**
- Consumes: `DirEntry.is_pdf`（Task 3）。
- Produces: `.pdf` 文件行可点击，emit `select`；视觉上与 md 区分（不同图标）。

- [ ] **Step 1: 改造 FileTree.vue**

template 中 file 行（L20-31）改为区分 pdf 图标 + 放宽 disabled 条件（仅非 md 且非 pdf 才 disabled）：

```vue
      <!-- File -->
      <div
        v-else
        class="ft-row ft-file"
        :class="{ active: entry.path === activePath, disabled: !entry.is_markdown && !entry.is_pdf }"
        :style="{ paddingLeft: indent }"
        :title="entry.path"
        @click="onFileClick(entry)"
      >
        <span class="ft-caret-spacer"></span>
        <el-icon class="ft-icon">
          <Document v-if="entry.is_markdown" />
          <Files v-else-if="entry.is_pdf" />
          <Document v-else />
        </el-icon>
        <span class="ft-name">{{ entry.name }}</span>
      </div>
```

script 中 import 加 `Files` 图标（L47）：

```ts
import { CaretBottom, CaretRight, Folder, FolderOpened, Document, Files } from '@element-plus/icons-vue'
```

`onFileClick` 放宽可点击条件（L88-90）：

```ts
function onFileClick(entry: DirEntry) {
  if (entry.is_markdown || entry.is_pdf) emit('select', entry.path)
}
```

- [ ] **Step 2: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: 通过。

- [ ] **Step 3: 手动验证**

启动后端 `cargo run`（或已有进程）+ 前端 `cd frontend && npm run dev`，打开阅境轩，输入一个含 `.pdf` 的文件夹。确认：`.pdf` 文件显示 `Files` 图标、可点击（不再灰）、md 仍显示 `Document`。点击 pdf 会报错（PdfViewer 尚未集成，预期）。

- [ ] **Step 4: 提交**

```bash
git add frontend/src/components/reader/FileTree.vue
git commit -m "feat(reader): FileTree allows clicking .pdf files with distinct icon"
```

---

## Task 5: Frontend — PdfViewer.vue 核心渲染（加载 + 懒渲染 + 主题调色）

**Files:**
- Create: `frontend/src/components/reader/PdfViewer.vue`

**Interfaces:**
- Consumes: `localFileUrl(path)`（Task 3）；`useAppStore().theme`。
- Produces: `<PdfViewer :src="path" />` 渲染 PDF；CSS class `pdf-theme-<light|dark|eye-care>` 作用于 canvas 容器。本任务不 emit outline、不暴露方法（Task 6 加）。

- [ ] **Step 1: 创建 PdfViewer.vue 核心实现**

创建 `frontend/src/components/reader/PdfViewer.vue`：

```vue
<template>
  <div ref="scrollRef" class="pdf-viewer" :class="`pdf-theme-${theme}`">
    <div v-if="loading" class="pdf-state">
      <el-icon class="is-loading"><Loading /></el-icon><span>PDF 加载中…</span>
    </div>
    <div v-else-if="error" class="pdf-state error">⚠️ {{ error }}</div>
    <div v-else class="pdf-pages">
      <div
        v-for="p in pageMetas"
        :key="p.num"
        class="pdf-page-wrap"
        :data-page-num="p.num"
        :style="{ width: p.width + 'px', height: p.height + 'px' }"
      >
        <canvas :ref="(el) => setCanvasRef(p.num, el as HTMLCanvasElement | null)" class="pdf-canvas"></canvas>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import * as pdfjsLib from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { useAppStore } from '@/stores/app'
import { localFileUrl } from '@/api/reader'

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

const props = defineProps<{ src: string }>()
const appStore = useAppStore()
const theme = ref(appStore.theme)

interface PageMeta { num: number; width: number; height: number }

const scrollRef = ref<HTMLElement | null>(null)
const loading = ref(true)
const error = ref('')
const pageMetas = ref<PageMeta[]>([])
const canvasRefs: Record<number, HTMLCanvasElement | null> = {}
let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null
let baseScale = 1 // fit-width scale derived from container width
let renderedPages = new Set<number>()
let observer: IntersectionObserver | null = null

function setCanvasRef(num: number, el: HTMLCanvasElement | null) {
  canvasRefs[num] = el
}

/** Compute fit-width scale so the PDF page fills the container width. */
function computeFitScale(page: pdfjsLib.PDFPageProxy): number {
  const containerWidth = scrollRef.value?.clientWidth ?? 800
  const viewport0 = page.getViewport({ scale: 1 })
  return containerWidth / viewport0.width
}

/** Render a single page to its canvas (idempotent — skips if already rendered). */
async function renderPage(num: number) {
  if (!pdfDoc || renderedPages.has(num)) return
  const canvas = canvasRefs[num]
  if (!canvas) return
  try {
    const page = await pdfDoc.getPage(num)
    const viewport = page.getViewport({ scale: baseScale })
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.floor(viewport.width * dpr)
    canvas.height = Math.floor(viewport.height * dpr)
    canvas.style.width = `${viewport.width}px`
    canvas.style.height = `${viewport.height}px`
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const transform = dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined
    await page.render({ canvasContext: ctx, viewport, transform }).promise
    renderedPages.add(num)
  } catch (e) {
    console.warn(`渲染第 ${num} 页失败:`, e)
  }
}

/** Set up lazy rendering: observe each page wrapper, render when near viewport. */
function setupObserver() {
  if (!scrollRef.value) return
  observer?.disconnect()
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const num = Number((entry.target as HTMLElement).dataset.pageNum)
          void renderPage(num)
        }
      }
    },
    { root: null, rootMargin: '800px 0px' },
  )
  const wraps = scrollRef.value.querySelectorAll<HTMLElement>('.pdf-page-wrap')
  wraps.forEach((w) => observer?.observe(w))
}

async function load() {
  loading.value = true
  error.value = ''
  renderedPages = new Set<number>()
  pageMetas.value = []
  try {
    const task = pdfjsLib.getDocument(localFileUrl(props.src))
    pdfDoc = await task.promise
    // Use page 1 to derive the fit-width scale; record every page's placeholder
    // size at that scale so the scroll area has correct height before render.
    const page1 = await pdfDoc.getPage(1)
    baseScale = computeFitScale(page1)
    const vp1 = page1.getViewport({ scale: baseScale })
    const metas: PageMeta[] = []
    for (let i = 1; i <= pdfDoc.numPages; i++) {
      // Assume uniform page size (common case); page 1 dimensions for all.
      // Non-uniform PDFs will have slightly mismatched placeholders — acceptable
      // for v1; lazy render corrects the actual canvas size on render.
      metas.push({ num: i, width: vp1.width, height: vp1.height })
    }
    pageMetas.value = metas
    loading.value = false
    // Wait for placeholders to mount, then observe + render visible pages.
    await nextTickAsync()
    setupObserver()
    // Render the first page immediately so the user sees content at once.
    await renderPage(1)
  } catch (e) {
    error.value = (e as Error)?.message || 'PDF 解析失败'
    loading.value = false
  }
}

/** Minimal nextTick promise (avoid importing vue's nextTick name clash). */
function nextTickAsync(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

watch(() => props.src, () => { void load() })
watch(() => appStore.theme, (t) => { theme.value = t })

onMounted(() => { void load() })
onBeforeUnmount(() => {
  observer?.disconnect()
  void pdfDoc?.destroy()
  pdfDoc = null
})
</script>

<style scoped>
.pdf-viewer {
  /* NOT its own scroll container — flows in .pane-center so onContentScroll
     (FAB hide, mobile header collapse) and page-turn transitions are reused. */
  padding: 12px 20px 120px;
}
/* Theme color via CSS filter on the canvas pages — no re-render needed. */
.pdf-theme-light .pdf-pages { filter: none; }
.pdf-theme-dark .pdf-pages { filter: invert(1) hue-rotate(180deg); }
.pdf-theme-eye-care .pdf-pages { filter: sepia(0.4) brightness(0.96) saturate(0.85); }

.pdf-pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}
.pdf-page-wrap {
  background: var(--bg-glass-subtle);
  border: 1px solid var(--border-faint);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
}
.pdf-canvas { display: block; }

.pdf-state {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-faint);
  font-size: 14px;
}
.pdf-state.error { color: #f87171; }
.pdf-state .is-loading { animation: spin 1s linear infinite; color: var(--accent); }
</style>
```

- [ ] **Step 2: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: 通过。若 `pdfjs-dist/build/pdf.worker.min.mjs?url` 类型报错，在 `frontend/src/` 加一个 `shims.d.ts` 声明：

```ts
declare module '*?url' {
  const url: string
  export default url
}
```

（仅当 vue-tsc 报错时才加。）

- [ ] **Step 3: 手动验证**

后端运行 + 前端 `npm run dev`。临时在 `Reader.vue` 的 `onSelectFile` 末尾加一行调试（或直接在浏览器用 vue devtools）——更简单：临时把 `Reader.vue` 的 `<article>` 分支替换为 `<PdfViewer :src="activeFile" />`（仅验证用，Task 7 正式集成）。打开一个 `.pdf`，确认：
- 页面竖向堆叠显示、原版式保留（图表/表格/公式可见）。
- 滚动时下方页面懒渲染（观察 Network/Console）。
- 切换主题（light/dark/eye-care）：背景/文字颜色实时变化，canvas 不重渲染。
- 深色模式图片被反色（预期代价）。

- [ ] **Step 4: 提交**

```bash
git add frontend/src/components/reader/PdfViewer.vue frontend/src/shims.d.ts 2>/dev/null || git add frontend/src/components/reader/PdfViewer.vue
git commit -m "feat(reader): PdfViewer core — pdf.js lazy canvas render + theme filter"
```

---

## Task 6: Frontend — PdfViewer 文字层 + outline + 缩放

**Files:**
- Modify: `frontend/src/components/reader/PdfViewer.vue`（Task 5 产物）

**Interfaces:**
- Produces: emit `outline` 事件 `{ text: string; level: number; page: number }[]`（dest 已解析为页码）；`defineExpose({ scrollToPage, setZoom })`。
- Consumes: 无新依赖。

- [ ] **Step 1: 加文字层（best-effort）**

在 `renderPage` 中渲染 canvas 后追加文字层。修改 `PdfViewer.vue` template 的 page-wrap（在 `<canvas>` 后加文字层 div）：

```vue
        <canvas :ref="(el) => setCanvasRef(p.num, el as HTMLCanvasElement | null)" class="pdf-canvas"></canvas>
        <div :ref="(el) => setTextRef(p.num, el as HTMLDivElement | null)" class="pdf-text-layer"></div>
```

script 中加 `textRefs` + 在 `renderPage` 内渲染文字层（best-effort，失败不影响 canvas）：

```ts
const textRefs: Record<number, HTMLDivElement | null> = {}
function setTextRef(num: number, el: HTMLDivElement | null) { textRefs[num] = el }
```

在 `renderPage` 的 `await page.render(...).promise` 之后追加：

```ts
    // Best-effort text layer for selection/search; failure is non-fatal.
    try {
      const textContent = await page.getTextContent()
      const textDiv = textRefs[num]
      if (textDiv) {
        textDiv.innerHTML = ''
        textDiv.style.width = `${viewport.width}px`
        textDiv.style.height = `${viewport.height}px`
        // pdf.js v4 TextLayer class
        const textLayer = new pdfjsLib.TextLayer({
          textContentSource: textContent,
          container: textDiv,
          viewport,
        })
        void textLayer.render()
      }
    } catch (e) {
      console.warn(`文字层渲染失败 (第 ${num} 页):`, e)
    }
```

style 中追加（scoped）：

```css
.pdf-page-wrap { position: relative; }
.pdf-text-layer {
  position: absolute;
  inset: 0;
  overflow: hidden;
  line-height: 1;
  opacity: 0.25;
  /* text layer must not invert with the canvas filter — counter-filter */
}
.pdf-theme-dark .pdf-text-layer { filter: invert(1) hue-rotate(180deg); opacity: 1; }
.pdf-theme-eye-care .pdf-text-layer { filter: sepia(0.4) brightness(0.96) saturate(0.85); opacity: 1; }
.pdf-text-layer ::selection { background: var(--accent); color: transparent; }
```

> 注意：文字层颜色/反相需实际渲染时微调——`opacity` 与 counter-filter 的组合以保证深色模式下文字可选且不可见（canvas 已显示文字）。手动验证步骤会检查。

- [ ] **Step 2: 加 outline emit**

script 中加 emit 声明 + 在 `load()` 中 `pdfDoc` 就绪后提取 outline：

```ts
const emit = defineEmits<{
  outline: [items: { text: string; level: number; page: number }[]]
}>()

/** Recursively flatten pdf.js outline, resolving each dest to a 1-based page. */
async function buildOutline(
  raw: pdfjsLib.OutlineNode[] | null,
  level: number,
): Promise<{ text: string; level: number; page: number }[]> {
  if (!raw || !raw.length) return []
  const out: { text: string; level: number; page: number }[] = []
  for (const node of raw) {
    let page = 1
    try {
      let dest: unknown = node.dest
      if (typeof dest === 'string' && pdfDoc) {
        dest = await pdfDoc.getDestination(dest)
      }
      const idx = await pdfDoc?.getPageIndex(dest as pdfjsLib.ExplicitDest)
      if (typeof idx === 'number') page = idx + 1
    } catch {
      page = 1
    }
    out.push({ text: node.title, level, page })
    if (node.items?.length) {
      out.push(...await buildOutline(node.items, level + 1))
    }
  }
  return out
}
```

在 `load()` 的 `await renderPage(1)` 之后追加：

```ts
    const rawOutline = await pdfDoc.getOutline()
    const outline = await buildOutline(rawOutline, 1)
    emit('outline', outline)
```

- [ ] **Step 3: 加缩放 + scrollToPage + 暴露**

script 顶部加缩放状态：

```ts
const zoomMode = ref<'fit' | number>('fit') // 'fit' = fit-width; number = explicit scale factor
```

新增 `computeScaleFor`、`rerenderAll`、`setZoom`、`scrollToPage`：

```ts
function currentScale(): number {
  if (zoomMode.value === 'fit') return baseScale
  return zoomMode.value
}

/** Re-render all already-observed pages at the current scale (zoom change). */
async function rerenderAll() {
  if (!pdfDoc) return
  // Recompute placeholder sizes for the new scale.
  const page1 = await pdfDoc.getPage(1)
  const s = zoomMode.value === 'fit' ? computeFitScale(page1) : zoomMode.value
  baseScale = s
  const vp = page1.getViewport({ scale: s })
  pageMetas.value = pageMetas.value.map((p) => ({ ...p, width: vp.width, height: vp.height }))
  renderedPages = new Set<number>()
  await nextTickAsync()
  // Re-render currently visible pages.
  const visible = scrollRef.value?.querySelectorAll<HTMLElement>('.pdf-page-wrap') ?? []
  for (const w of Array.from(visible)) {
    const num = Number(w.dataset.pageNum)
    void renderPage(num)
  }
}

function setZoom(mode: 'fit' | number) {
  zoomMode.value = mode
  void rerenderAll()
}

function scrollToPage(num: number) {
  const el = scrollRef.value?.querySelector<HTMLElement>(`.pdf-page-wrap[data-page-num="${num}"]`)
  el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

defineExpose({ scrollToPage, setZoom })
```

修改 `renderPage` 中 `const viewport = page.getViewport({ scale: baseScale })` → `const viewport = page.getViewport({ scale: currentScale() })`。

- [ ] **Step 4: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: 通过。注意 `pdfjsLib.OutlineNode`/`ExplicitDest` 类型可能因 pdfjs-dist 版本不同不存在——若报错，把相关类型改为 `any`（`let dest: any`、`raw: any[]`、`dest as any`），并在该行加 `// eslint-disable-next-line @typescript-eslint/no-explicit-any`。pdf.js 的类型导出在不同版本间不稳定，用 `any` 是务实选择。

- [ ] **Step 5: 手动验证**

打开 `.pdf`，确认：
- 文字可用鼠标选中（深色模式下亦然）。
- 切换主题文字层颜色正确（不可见残留不影响阅读）。
- 控制台无 outline 报错；用 vue devtools 查看 PdfViewer emit 的 `outline` 事件（Task 7 会接到 TOC）。
- 临时在浏览器 console 调 `$0`（选中 PdfViewer 根元素后）或 devtools 调 `setZoom(2)` / `scrollToPage(3)` 验证缩放与跳转。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/reader/PdfViewer.vue
git commit -m "feat(reader): PdfViewer text layer + outline emit + zoom/scrollToPage"
```

---

## Task 7: Frontend — Reader.vue 集成（md/pdf 分流 + TOC + 翻页 + 链接）

**Files:**
- Modify: `frontend/src/views/Reader.vue`（template ~L116-135 center pane、script import ~L218-226、`onSelectFile` ~L540-569、`flatFiles` ~L323-333、`handleLinkClick` ~L369-392、新增 `fileKind`/`pdfViewerRef`/`onPdfOutline`）

**Interfaces:**
- Consumes: `PdfViewer`（Task 5/6）的 `outline` emit + `scrollToPage`；`DirEntry.is_pdf`。
- Produces: 阅境轩支持 `.pdf`——点击 PDF 在 center pane 用 PdfViewer 渲染，TOC 取自 outline，翻页过渡含 pdf，markdown 内 `.pdf` 链接跳转。

- [ ] **Step 1: 改造 Reader.vue template — center pane 分流**

修改 center pane（L116-135），在 `<transition>` 内加 PdfViewer 分支：

```vue
      <main ref="contentRef" class="pane pane-center" @scroll="onContentScroll">
        <transition :name="transitionDir" mode="out-in" @enter="onArticleEnter">
          <PdfViewer
            v-if="fileKind === 'pdf' && renderedHtml === ''"
            ref="pdfViewerRef"
            :key="displayedFile"
            :src="displayedFile"
            @outline="onPdfOutline"
          />
          <article
            v-else-if="renderedHtml"
            :key="displayedFile"
            class="markdown-body"
            v-html="renderedHtml"
          ></article>
          <div v-else key="empty" class="center-state">
            <el-icon class="cs-icon"><Document /></el-icon>
            <p>选择左侧的文件开始阅读</p>
          </div>
        </transition>

        <div v-if="fileLoading" class="loadbar"><span></span></div>
        <div v-if="error" class="error-banner">⚠️ {{ error }}</div>
      </main>
```

> 说明：`fileKind === 'pdf'` 时 `renderedHtml` 保持空字符串，PdfViewer 分支生效。`onArticleEnter` 对 PdfViewer 的根元素（`.markdown-body`）也会触发——需在 `onArticleEnter` 内跳过非 article 的处理（见 Step 4）。

- [ ] **Step 2: script 改造 — import + 状态**

import 加 `PdfViewer`（L224 附近）：

```ts
import PdfViewer from '@/components/reader/PdfViewer.vue'
```

在 `const displayedFile = ref('')` 附近（L313）加：

```ts
const fileKind = ref<'md' | 'pdf'>('md')
const pdfViewerRef = ref<{ scrollToPage: (n: number) => void; setZoom: (m: 'fit' | number) => void } | null>(null)
```

- [ ] **Step 3: 改造 flatFiles — 纳入 pdf**

修改 `flatFiles` computed（L323-333），收集 pdf：

```ts
const flatFiles = computed(() => {
  const out: string[] = []
  const walk = (entries: DirEntry[]) => {
    for (const e of entries) {
      if (e.is_dir) e.children && walk(e.children)
      else if (e.is_markdown || e.is_pdf) out.push(e.path)
    }
  }
  walk(tree.value)
  return out
})
```

- [ ] **Step 4: 改造 onSelectFile — 按 ext 分流**

修改 `onSelectFile`（L540-569）。pdf 分支不调 `readLocalFile`/`renderMarkdown`，只设 `fileKind`/`displayedFile`：

```ts
async function onSelectFile(path: string) {
  activeFile.value = path
  if (path === displayedFile.value) return
  fileLoading.value = true
  error.value = ''

  const isPdf = /\.pdf$/i.test(path)
  const oldIdx = flatFiles.value.indexOf(displayedFile.value)
  const newIdx = flatFiles.value.indexOf(path)
  transitionDir.value =
    oldIdx >= 0 && newIdx >= 0 && newIdx < oldIdx ? 'page-prev' : 'page-next'

  try {
    if (isPdf) {
      fileKind.value = 'pdf'
      renderedHtml.value = '' // ensure PdfViewer branch shows
      displayedFile.value = path
      toc.value = [] // outline arrives via onPdfOutline after load
      localStorage.setItem(LAST_FILE_KEY, path)
    } else {
      const res = await readLocalFile(path)
      if (res.status === 'error' || !res.result) {
        error.value = res.error?.message || '读取失败'
        ElMessage.error(error.value)
        return
      }
      fileKind.value = 'md'
      renderedHtml.value = renderMarkdown(res.result.content)
      displayedFile.value = path
      localStorage.setItem(LAST_FILE_KEY, path)
    }
  } catch (e) {
    error.value = (e as Error)?.message || '读取失败'
  } finally {
    fileLoading.value = false
  }
}
```

修改 `onArticleEnter`（L572-583），跳过 PdfViewer（其根元素含 `markdown-body` 但非 `<article>`）：

```ts
async function onArticleEnter(el: Element) {
  // Only the markdown <article> needs enhancing; PdfViewer handles itself.
  if (el.tagName !== 'ARTICLE') return
  if (contentRef.value && !pendingAnchor.value) contentRef.value.scrollTop = 0
  buildToc()
  await enhance(el as HTMLElement)
  if (pendingAnchor.value) {
    scrollToHeading(pendingAnchor.value)
    pendingAnchor.value = ''
  }
}
```

新增 `onPdfOutline`（PdfViewer emit 的 outline 填 TOC）：

```ts
function onPdfOutline(items: { text: string; level: number; page: number }[]) {
  // Only accept outline for the currently displayed file (avoid stale races).
  toc.value = items.map((it) => ({ id: `pdf-${it.page}`, text: it.text, level: it.level, page: it.page }))
}
```

修改 `TocItem` 接口（L230）加可选 `page`：

```ts
interface TocItem { id: string; text: string; level: number; page?: number }
```

修改 TOC 点击处理——template 中 `@click="scrollToHeading(t.id)"`（L149 desktop、L177 mobile）改为 `@click="onTocClick(t)"`，新增 `onTocClick`：

```ts
function onTocClick(t: TocItem) {
  if (t.page !== undefined && fileKind.value === 'pdf') {
    pdfViewerRef.value?.scrollToPage(t.page)
  } else {
    scrollToHeading(t.id)
  }
}
```

- [ ] **Step 5: 改造 handleLinkClick — .pdf 链接跳转**

修改 `handleLinkClick`（L378），把 `.pdf` 也路由到主阅读器（在 root 下时）：

```ts
  if (/\.(md|markdown|pdf)$/i.test(resolved) && isUnderRoot(resolved)) {
```

- [ ] **Step 6: 主题切换时 PdfViewer 内部已自处理（watch appStore.theme）**

无需额外改动——PdfViewer 内部 watch theme 切 filter class。确认 Reader 不需要在主题切换时通知 PdfViewer。

- [ ] **Step 7: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: 通过。

- [ ] **Step 8: 手动验证**

打开含 md + pdf 的文件夹：
- 点击 `.pdf` → PdfViewer 渲染，翻页过渡动画正常。
- 右栏 TOC 显示 PDF outline；点击 TOC 项 → 跳到对应页。
- 点击 `.md` → markdown 渲染，TOC 来自标题。
- md 与 pdf 混排翻页方向（上/下文件）正确。
- md 内一个指向 `.pdf`（在 root 下）的相对链接 → 点击后在阅读器打开 PDF。
- 全屏模式 PDF 撑满 center pane。
- 切换主题 md/pdf 颜色均跟随。
- 历史记录含 pdf（pin/rename 正常）。

- [ ] **Step 9: 提交**

```bash
git add frontend/src/views/Reader.vue
git commit -m "feat(reader): integrate PdfViewer — md/pdf branching, outline TOC, .pdf links"
```

---

## Task 8: Frontend — PathPreviewModal 支持 .pdf 路由

**Files:**
- Modify: `frontend/src/components/reader/PathPreviewModal.vue`（`classify` ~L128-135、`canOpenInReader` ~L87-89、`onLinkClick` ~L119、`onTreeSelect` ~L264-270、template body ~L29-35）

**Interfaces:**
- Consumes: `statLocalPath` 返回的 `ext`。
- Produces: `.pdf` 链接目标（在 root 下）→ emit `open-in-reader`；不在 root 下 → 显示提示 + "阅读器"按钮（`canOpenInReader` 含 pdf）。

- [ ] **Step 1: 改造 classify**

修改 `classify`（L128-135），加 pdf 分支：

```ts
function classify(s: PathStat): string {
  if (!s.exists) return 'notfound'
  if (s.is_dir) return 'folder'
  const ext = s.ext.toLowerCase()
  if (ext === 'md' || ext === 'markdown') return 'md'
  if (ext === 'pdf') return 'pdf'
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico'].includes(ext)) return 'image'
  return 'code'
}
```

- [ ] **Step 2: 改造 canOpenInReader**

修改 `canOpenInReader`（L87-89）含 pdf：

```ts
const canOpenInReader = computed(
  () => (kind.value === 'md' || kind.value === 'pdf') && isUnderRoot(currentPath.value),
)
```

- [ ] **Step 3: 改造 onLinkClick / onTreeSelect**

修改 `onLinkClick`（L119）与 `onTreeSelect`（L265）的 md 正则含 pdf：

```ts
  if (/\.(md|markdown|pdf)$/i.test(resolved) && isUnderRoot(resolved)) {
    emit('open-in-reader', resolved, pendingAnchor.value)
```

`onTreeSelect` 同理（L265）：

```ts
  if (/\.(md|markdown|pdf)$/i.test(path) && isUnderRoot(path)) {
    emit('open-in-reader', path)
```

- [ ] **Step 4: template body — pdf 状态**

在 template body（L29-35 的 `v-else-if="kind === 'image'"` 后、`v-else` 前）加 pdf 状态：

```vue
        <div v-else-if="kind === 'pdf'" class="ppm-state">
          <el-icon><Document /></el-icon><span>PDF 预览请点击右上「阅读器」按钮打开</span>
        </div>
```

- [ ] **Step 5: 类型检查 + 构建 + 手动验证**

Run: `cd frontend && npm run build`
Expected: 通过。

手动验证：在阅境轩打开一个 md，其中有一个指向**不在当前 root 下**的 `.pdf` 的相对链接 → 点击 → PathPreviewModal 弹出，显示"PDF 预览请点击阅读器按钮"→ 点"阅读器"按钮 → 在主阅读器打开该 PDF。指向 root 下的 `.pdf` 链接 → 直接在阅读器打开（不弹 modal）。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/reader/PathPreviewModal.vue
git commit -m "feat(reader): PathPreviewModal routes .pdf links to the reader"
```

---

## Task 9: 全量验证 + 收尾

**Files:**
- 无新增/修改（仅验证）；若发现遗漏则补。

- [ ] **Step 1: 后端全量质量检查**

Run:
```bash
cd backend && cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```
Expected: 全部通过，零 warning。

- [ ] **Step 2: 前端构建**

Run: `cd frontend && npm run build`
Expected: vue-tsc 类型检查 + vite 构建通过。

- [ ] **Step 3: 前端嵌入后端（若发布流程需要）**

确认 `backend/frontend/dist` 是否需要更新（项目用 rust-embed 嵌入）。若 README/Makefile 要求同步 dist：

Run: `cat Makefile`（确认前端构建产物路径），按需 `cp -r frontend/dist/* backend/frontend/dist/` 或运行 Makefile 目标。

- [ ] **Step 4: 端到端手动验证**

启动后端 + 前端，打开阅境轩，按以下清单验证：
1. 打开含 `.pdf` 的文件夹 → 文件树显示 pdf（Files 图标，可点击）。
2. 点击 pdf → 渲染、原版式保留、翻页过渡。
3. 懒渲染：大 PDF 滚动流畅，下方页面按需渲染。
4. 主题：light/dark/eye-care 切换，背景/文字颜色实时变（深色图片反色，预期）。
5. 文字可选（深色模式亦然）。
6. TOC：pdf 有 outline 时显示，点击跳页；无 outline 显示"无目录"。
7. md/pdf 混排翻页方向正确。
8. md 内 `.pdf` 链接（root 下）→ 阅读器打开；不在 root 下 → PathPreviewModal。
9. 全屏：PDF 撑满，FAB/TOC 可用，Esc 退出。
10. 历史：pdf 入历史，pin/rename/重开正常。
11. 错误：删除一个 pdf 后点击 → error banner；超大 pdf（>100MB）→ 413。

- [ ] **Step 5: 提交收尾（如有补丁）**

```bash
git add -A
git commit -m "test(reader): e2e verification of PDF reading support" --allow-empty
```

（若 Step 1-4 全通过且无补丁，可跳过此步或用 `--allow-empty` 记录验证完成。）

---

## Self-Review 记录

（plan 作者在写作后自检；执行者无需操作此节。）

- **Spec 覆盖**：spec 第 4 节（后端端点 + is_pdf）→ Task 1/2；第 5.1（PdfViewer）→ Task 5/6；第 5.2（Reader 集成）→ Task 7；第 5.3（FileTree）→ Task 4；第 5.4（api/reader.ts）→ Task 3；第 5.5（markdown 内 pdf 链接）→ Task 7(handleLinkClick) + Task 8(PathPreviewModal)；第 7 节错误处理 → Task 2 测试 + Task 9 验证；第 8 节测试 → Task 1/2 真测试 + Task 5-8 手动验证。全覆盖。
- **占位符扫描**：无 TBD/TODO；所有 code step 含实际代码。
- **类型一致**：`DirEntry.is_pdf`（Task 1 后端 struct ↔ Task 3 前端 interface）一致；`localFileUrl`（Task 3 定义 ↔ Task 5 使用）一致；PdfViewer emit `outline` `{text,level,page}`（Task 6 定义 ↔ Task 7 `onPdfOutline` 消费）一致；`defineExpose({ scrollToPage, setZoom })`（Task 6 ↔ Task 7 `pdfViewerRef` 类型）一致；`TocItem.page?`（Task 7 定义 ↔ `onTocClick` 消费）一致。
- **已知风险**（已在对应 task 标注）：pdf.js v4 `TextLayer`/`OutlineNode` 类型导出不稳定 → Task 6 Step 4 注明退化为 `any`；文字层反相滤镜需手动微调 → Task 6 Step 5 验证；read-all-then-slice 的 Range 性能 → Task 2 代码注释标注未来优化。
