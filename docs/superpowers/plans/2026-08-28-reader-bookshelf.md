# 阅境轩·书架 (Reader Bookshelf) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阅境轩新增书架视图——文件夹/PDF 登记为书（书名/描述/类别），滑块切换书架↔阅读，每本书独立记住阅读进度（文件+滚动比例 / PDF 页码）并在打开时恢复。

**Architecture:** 数据沿用 `reader_history` 的服务端 `app_state` JSON 模式（key `reader_books`，新增 get/save 两个 tool handler，零迁移）。前端：纯函数工具（utils/readerBooks.ts）+ 可注入依赖的书架状态 composable（DI 便于 node 测试）+ BookshelfView 组件 + Reader.vue 内 `viewMode`（`shelf`/`read`）滑块切换（v-show 保活，复刻 Tasks 的 `.view-switch`）。

**Tech Stack:** Rust (Axum + SqliteStore + serde) / Vue 3 + Element Plus / node --test / playwright-core harness。

**Spec:** `docs/requirement/10-reader-bookshelf.md`

## Global Constraints

- 后端：`Result<T, BrainError>`；生产代码禁止 `.unwrap()`/`.expect()`（测试除外）；`cargo fmt` + `cargo clippy -- -D warnings` 零 warning；新 handler 注册在 reader 模块（`backend/src/tools/handlers/mod.rs`）。
- 前端：**零新增 npm 依赖**；类型经 `npx vue-tsc -b`；测试 `npm test`（node --test --experimental-strip-types）。
- E2E harness：`/tmp/mobile-repro/*.mjs`（playwright-core + `~/Library/Caches/ms-playwright/chromium-1234` 的 Chrome for Testing），dev server `cd frontend && npx vite --port 5174`（代理 /v1 → 127.0.0.1:9876），验证用 DOM 测量不用截图自评。
- 用户 live vault（`/Users/tiercelchow/Documents/Obsidian/TiercelChow's Blog/`）与 `~/.obsidian-brain/brain.db`：只读，绝不写入/修改；E2E 添加的书用 `/tmp` 路径或 vault 既有路径。
- 不碰 `/usr/local/bin`；安装走 `make install`（→ `~/.local/bin`）；服务重启由用户执行。
- Git：Conventional Commits；不推远端；分支 `feat/reader-bookshelf`（已建，含 spec commit）。
- 每个任务完成即 commit；commit 前 `cargo fmt --check && cargo clippy -- -D warnings && cargo test`（后端任务）/ `npx vue-tsc -b && npm test`（前端任务）。

---

### Task 1: 后端 — get/save_reader_books handlers

**Files:**
- Modify: `backend/src/tools/handlers/reader_handlers.rs`（history 段之后、StatLocalPathHandler 之前插入 books 段；文件尾 `mod tests` 内追加测试）
- Modify: `backend/src/tools/handlers/mod.rs`（注册两行 + 顶部注释更新）

**Interfaces:**
- Produces（后端 tool，前端 Task 2 对接）: `get_reader_books` → `{"books": ReaderBook[]}`；`save_reader_books` 入参 `{books: ReaderBook[]}` → `{"ok": true, "count": n}`。
- Produces（Rust）: `struct ReaderBook` / `struct BookProgress` / `enum BookKind`（serde camelCase）；`fn get_books(db: &SqliteStore) -> Result<Vec<ReaderBook>, BrainError>`；`fn save_books(db: &SqliteStore, json: &Value) -> Result<usize, BrainError>`。

- [ ] **Step 1: 写失败测试**（追加到 reader_handlers.rs 的 `mod tests`；模式仿 `sqlite_store.rs:746` 的临时库构造）

```rust
    // ── reader_books ──────────────────────────────────────────────────
    fn books_payload() -> Value {
        json!([
            {
                "id": "b1", "path": "/tmp/docs", "kind": "folder",
                "name": "文档集", "addedAt": 1700000000000i64,
                "description": "", "category": "技术",
                "progress": { "lastFile": "/tmp/docs/a.md", "position": 0.42, "updatedAt": 1700000001000i64 }
            },
            {
                "id": "b2", "path": "/tmp/book.pdf", "kind": "pdf",
                "name": "book", "addedAt": 1700000000001i64,
                "progress": { "position": 12, "pageCount": 180, "updatedAt": 1700000000002i64 }
            }
        ])
    }

    #[test]
    fn test_books_roundtrip_through_state() {
        let tmp = tempfile::tempdir().unwrap();
        let db = SqliteStore::new(&tmp.path().join("t.db")).unwrap();
        // 空 → 空列表
        assert!(get_books(&db).unwrap().is_empty());
        let n = save_books(&db, &books_payload()).unwrap();
        assert_eq!(n, 2);
        let books = get_books(&db).unwrap();
        assert_eq!(books.len(), 2);
        assert_eq!(books[0].id, "b1");
        assert!(matches!(books[0].kind, BookKind::Folder));
        assert_eq!(books[0].progress.as_ref().unwrap().last_file.as_deref(), Some("/tmp/docs/a.md"));
        assert_eq!(books[1].progress.as_ref().unwrap().page_count, Some(180));
    }

    #[test]
    fn test_books_corrupted_json_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let db = SqliteStore::new(&tmp.path().join("t.db")).unwrap();
        db.set_state(BOOKS_KEY, "{not json").unwrap();
        assert!(get_books(&db).unwrap().is_empty());
    }

    #[test]
    fn test_save_books_rejects_missing_required_field() {
        let tmp = tempfile::tempdir().unwrap();
        let db = SqliteStore::new(&tmp.path().join("t.db")).unwrap();
        let bad = json!([{ "id": "x", "path": "/tmp/a" }]); // 缺 kind/name/addedAt
        let err = save_books(&db, &bad).unwrap_err();
        assert!(matches!(err, BrainError::Internal(_)));
    }

    #[test]
    fn test_save_books_rejects_bad_kind() {
        let tmp = tempfile::tempdir().unwrap();
        let db = SqliteStore::new(&tmp.path().join("t.db")).unwrap();
        let bad = json!([{ "id": "x", "path": "/tmp/a", "kind": "video", "name": "n", "addedAt": 1 }]);
        assert!(save_books(&db, &bad).is_err());
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd backend && cargo test reader_books`
Expected: 编译失败——`get_books`/`save_books`/`BOOKS_KEY`/`ReaderBook` 未定义。

- [ ] **Step 3: 实现**（插在 `// ── Reader history` 段之后）

```rust
// ── Reader bookshelf (server-stored, shared across all users) ────────

/// SQLite `app_state` key holding the bookshelf JSON array.
const BOOKS_KEY: &str = "reader_books";

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
enum BookKind {
    Folder,
    Pdf,
}

/// Reading progress: folder books store lastFile + scroll ratio (0..1);
/// pdf books store the page number (+ pageCount for display).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct BookProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_file: Option<String>,
    position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    page_count: Option<i64>,
    updated_at: i64,
}

/// A bookshelf entry: a local folder (md collection) or a pdf file.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ReaderBook {
    id: String,
    path: String,
    kind: BookKind,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    category: String,
    added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    progress: Option<BookProgress>,
}

/// Read the bookshelf from SQLite. Unset or corrupted JSON → empty list.
fn get_books(db: &SqliteStore) -> Result<Vec<ReaderBook>, BrainError> {
    match db.get_state(BOOKS_KEY)? {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

/// Validate + persist the full book list (whole-list replace). Returns count.
fn save_books(db: &SqliteStore, json: &Value) -> Result<usize, BrainError> {
    let books: Vec<ReaderBook> = serde_json::from_value(json.clone())
        .map_err(|e| BrainError::Internal(format!("books 格式错误: {e}")))?;
    let serialized = serde_json::to_string(&books)
        .map_err(|e| BrainError::Internal(format!("序列化失败: {e}")))?;
    db.set_state(BOOKS_KEY, &serialized)?;
    Ok(books.len())
}

/// Get the bookshelf.
pub struct GetReaderBooksHandler;

#[async_trait]
impl ToolHandler for GetReaderBooksHandler {
    fn name(&self) -> &str {
        "get_reader_books"
    }
    fn description(&self) -> &str {
        "获取阅读器书架（服务端存储，所有用户共享）"
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    fn module(&self) -> &str {
        "reader"
    }
    async fn handle(&self, _args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let books = get_books(&ctx.db)?;
        let books_json = serde_json::to_value(&books)
            .map_err(|e| BrainError::Internal(format!("序列化失败: {e}")))?;
        Ok(json!({ "books": books_json }))
    }
}

/// Save the full bookshelf (replaces the existing list).
pub struct SaveReaderBooksHandler;

#[async_trait]
impl ToolHandler for SaveReaderBooksHandler {
    fn name(&self) -> &str {
        "save_reader_books"
    }
    fn description(&self) -> &str {
        "保存阅读器书架（整体替换，服务端共享）"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "books": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "path": { "type": "string" },
                            "kind": { "type": "string", "enum": ["folder", "pdf"] },
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "category": { "type": "string" },
                            "addedAt": { "type": "number" },
                            "progress": {
                                "type": "object",
                                "properties": {
                                    "lastFile": { "type": ["string", "null"] },
                                    "position": { "type": "number" },
                                    "pageCount": { "type": "number" },
                                    "updatedAt": { "type": "number" }
                                },
                                "required": ["position", "updatedAt"]
                            }
                        },
                        "required": ["id", "path", "kind", "name", "addedAt"]
                    }
                }
            },
            "required": ["books"]
        })
    }
    fn module(&self) -> &str {
        "reader"
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let books_arg = args
            .get("books")
            .ok_or_else(|| BrainError::Internal("缺少必需参数 'books'".to_string()))?;
        let count = save_books(&ctx.db, books_arg)?;
        Ok(json!({ "ok": true, "count": count }))
    }
}
```

mod.rs（116-117 行 history 注册后追加）：

```rust
    registry.register(Arc::new(GetReaderBooksHandler)).await;
    registry.register(Arc::new(SaveReaderBooksHandler)).await;
```

并把 mod.rs 第 6 行注释里的工具清单补上 `get/save_reader_books`。tests 需 `use serde_json::json;`（mod tests 顶部若未引入则加；`Value` 已由 `super::*` 带入——若没有则 `use serde_json::Value;`）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cd backend && cargo test reader_books && cargo fmt --check && cargo clippy -- -D warnings`
Expected: 4 个新测试 PASS；fmt/clippy 干净。

- [ ] **Step 5: Commit**

```bash
git add backend/src/tools/handlers/reader_handlers.rs backend/src/tools/handlers/mod.rs
git commit -m "feat(reader): add get/save_reader_books tool handlers"
```

---

### Task 2: 前端 — 类型 + API 封装 + 纯函数工具

**Files:**
- Modify: `frontend/src/api/reader.ts`（文件尾 `localFileUrl` 前追加类型与两个函数）
- Create: `frontend/src/utils/readerBooks.ts`
- Test: `frontend/tests/readerBooks.test.ts`

**Interfaces:**
- Produces（api）: `interface ReaderBook { id: string; path: string; kind: 'folder' | 'pdf'; name: string; description: string; category: string; addedAt: number; progress?: BookProgress }`；`interface BookProgress { lastFile?: string | null; position: number; pageCount?: number; updatedAt: number }`；`getReaderBooks(): Promise<ToolEnvelope<{ books: ReaderBook[] }>>`；`saveReaderBooks(books: ReaderBook[]): Promise<ToolEnvelope<{ ok: boolean; count: number }>>`
- Produces（utils，Task 3/4/5/6/7 消费）: `deriveKind(path: string): 'folder' | 'pdf'`；`makeBookId(): string`；`bookProgressLabel(book: ReaderBook): string`；`sortBooks(books: ReaderBook[]): ReaderBook[]`；`scrollRatio(scrollTop: number, scrollHeight: number, clientHeight: number): number`；`clampPdfPage(page: number, pageCount: number): number`；`validateBookForm(path: string, editingId: string | null, books: ReaderBook[], stat: { exists: boolean; is_dir: boolean } | null): string | null`；`findBookByPath(books: ReaderBook[], path: string): ReaderBook | undefined`；`defaultBookName(path: string): string`

- [ ] **Step 1: 写失败测试** `frontend/tests/readerBooks.test.ts`（模式仿 markdownImages.test.ts）

```ts
import assert from 'node:assert/strict'
import test from 'node:test'

import {
  bookProgressLabel, clampPdfPage, defaultBookName, deriveKind, findBookByPath,
  makeBookId, scrollRatio, sortBooks, validateBookForm,
} from '../src/utils/readerBooks.ts'

// ── deriveKind / defaultBookName / makeBookId ──────────────────────────────

test('deriveKind maps .pdf (any case) to pdf, everything else to folder', () => {
  assert.equal(deriveKind('/a/b/Book.PDF'), 'pdf')
  assert.equal(deriveKind('/a/b/book.pdf'), 'pdf')
  assert.equal(deriveKind('/a/docs'), 'folder')
  assert.equal(deriveKind('/a/notes.md'), 'folder')
})

test('defaultBookName takes the last path segment', () => {
  assert.equal(defaultBookName('/a/docs'), 'docs')
  assert.equal(defaultBookName('/a/机器学习.pdf'), '机器学习.pdf')
  assert.equal(defaultBookName('/'), '')
})

test('makeBookId yields unique non-empty ids', () => {
  const ids = new Set(Array.from({ length: 200 }, () => makeBookId()))
  assert.equal(ids.size, 200)
})

// ── scrollRatio / clampPdfPage ─────────────────────────────────────────────

test('scrollRatio returns clamped ratio and 0 for non-scrollable content', () => {
  assert.equal(scrollRatio(500, 2000, 1000), 0.5)
  assert.equal(scrollRatio(0, 1000, 1000), 0)
  assert.equal(scrollRatio(9999, 2000, 1000), 1)
})

test('clampPdfPage clamps into [1, pageCount] and tolerates unknown pageCount', () => {
  assert.equal(clampPdfPage(12, 180), 12)
  assert.equal(clampPdfPage(200, 180), 180)
  assert.equal(clampPdfPage(0, 180), 1)
  assert.equal(clampPdfPage(2.7, 180), 2)
  assert.equal(clampPdfPage(7, 0), 7)
})

// ── bookProgressLabel / sortBooks ──────────────────────────────────────────

test('bookProgressLabel covers md/pdf/unread cases', () => {
  assert.equal(bookProgressLabel({ id: 'a', path: '/d', kind: 'folder', name: 'n', description: '', category: '', addedAt: 1 }), '未开始')
  assert.equal(bookProgressLabel({ id: 'a', path: '/d', kind: 'folder', name: 'n', description: '', category: '', addedAt: 1, progress: { position: 0.424, updatedAt: 2 } }), '读到 42%')
  assert.equal(bookProgressLabel({ id: 'a', path: '/d', kind: 'folder', name: 'n', description: '', category: '', addedAt: 1, progress: { position: 0, updatedAt: 2 } }), '未开始')
  assert.equal(bookProgressLabel({ id: 'b', path: '/x.pdf', kind: 'pdf', name: 'n', description: '', category: '', addedAt: 1, progress: { position: 12, pageCount: 180, updatedAt: 2 } }), '第 12/180 页')
  assert.equal(bookProgressLabel({ id: 'b', path: '/x.pdf', kind: 'pdf', name: 'n', description: '', category: '', addedAt: 1, progress: { position: 12, updatedAt: 2 } }), '第 12 页')
})

test('sortBooks orders by last activity (progress.updatedAt ?? addedAt) descending', () => {
  const b = (id: string, t: number, p?: number) => ({
    id, path: '/' + id, kind: 'folder' as const, name: id, description: '', category: '', addedAt: t,
    ...(p !== undefined ? { progress: { position: p, updatedAt: t + 100 } } : {}),
  })
  assert.deepEqual(sortBooks([b('old', 1), b('recent', 100), b('mid', 10, 0.5)]).map((x) => x.id), ['recent', 'mid', 'old'])
})

// ── findBookByPath / validateBookForm ──────────────────────────────────────

test('findBookByPath matches exact path', () => {
  const books = [{ id: 'a', path: '/docs', kind: 'folder' as const, name: 'n', description: '', category: '', addedAt: 1 }]
  assert.equal(findBookByPath(books, '/docs')?.id, 'a')
  assert.equal(findBookByPath(books, '/doc'), undefined)
})

test('validateBookForm rejects empty, missing, wrong-kind, duplicate paths', () => {
  const books = [{ id: 'a', path: '/docs', kind: 'folder' as const, name: 'n', description: '', category: '', addedAt: 1 }]
  assert.match(validateBookForm('', null, books, null)!, /路径/)
  assert.match(validateBookForm('/nope', null, books, { exists: false, is_dir: false })!, /不存在/)
  assert.match(validateBookForm('/x.txt', null, books, { exists: true, is_dir: false })!, /文件夹或 PDF/)
  assert.match(validateBookForm('/docs', null, books, { exists: true, is_dir: true })!, /已在书架/)
  // 编辑同一本书时路径未变不算重复
  assert.equal(validateBookForm('/docs', 'a', books, { exists: true, is_dir: true }), null)
  assert.equal(validateBookForm('/new', null, books, { exists: true, is_dir: true }), null)
  assert.equal(validateBookForm('/new.pdf', null, books, { exists: true, is_dir: false }), null)
  // stat 未返回（校验中）时不报错，由调用方控制提交时机
  assert.equal(validateBookForm('/new', null, books, null), null)
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm test`
Expected: readerBooks 测试全部 FAIL（模块不存在）。

- [ ] **Step 3: 实现** `frontend/src/utils/readerBooks.ts`

```ts
/**
 * Reader bookshelf pure helpers (see docs/requirement/10-reader-bookshelf.md).
 * No DOM / network here — unit-testable in node.
 */
import type { ReaderBook } from '@/api/reader'

export function deriveKind(path: string): 'folder' | 'pdf' {
  return /\.pdf$/i.test(path) ? 'pdf' : 'folder'
}

export function defaultBookName(path: string): string {
  return path.split('/').filter(Boolean).pop() ?? ''
}

export function makeBookId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`
}

/** Scroll position as a 0..1 ratio of the scrollable range; 0 when nothing to scroll. */
export function scrollRatio(scrollTop: number, scrollHeight: number, clientHeight: number): number {
  const denom = scrollHeight - clientHeight
  if (denom <= 0) return 0
  return Math.min(1, Math.max(0, Math.round((scrollTop / denom) * 10000) / 10000))
}

/** Valid pdf page in [1, pageCount]; unknown pageCount (0) only floors to ≥1. */
export function clampPdfPage(page: number, pageCount: number): number {
  const p = Math.max(1, Math.floor(page))
  return pageCount > 0 ? Math.min(p, pageCount) : p
}

export function bookProgressLabel(book: ReaderBook): string {
  const p = book.progress
  if (!p) return '未开始'
  if (book.kind === 'pdf') {
    return p.pageCount && p.pageCount > 0 ? `第 ${Math.floor(p.position)}/${p.pageCount} 页` : `第 ${Math.floor(p.position)} 页`
  }
  if (p.position <= 0) return '未开始'
  return `读到 ${Math.round(p.position * 100)}%`
}

/** Most recently read (or added) first. Returns a new array. */
export function sortBooks(books: ReaderBook[]): ReaderBook[] {
  return [...books].sort((a, b) => (b.progress?.updatedAt ?? b.addedAt) - (a.progress?.updatedAt ?? a.addedAt))
}

export function findBookByPath(books: ReaderBook[], path: string): ReaderBook | undefined {
  return books.find((b) => b.path === path)
}

/**
 * Form validation for add/edit (FR-7/FR-8). `stat` is the stat_local_path
 * result (null while pending → no path-kind errors, caller gates submit).
 * `editingId` exempts the book being edited from the duplicate check.
 * Returns an error message or null when valid.
 */
export function validateBookForm(
  path: string,
  editingId: string | null,
  books: ReaderBook[],
  stat: { exists: boolean; is_dir: boolean } | null,
): string | null {
  if (!path.trim()) return '请输入路径'
  const duplicate = books.find((b) => b.path === path.trim() && b.id !== editingId)
  if (duplicate) return '该书已在书架'
  if (stat) {
    if (!stat.exists) return '路径不存在'
    if (!(stat.is_dir || /\.pdf$/i.test(path))) return '仅支持文件夹或 PDF 文件'
  }
  return null
}
```

注意：utils 引用 `@/api/reader` 的**纯类型**，node --test 的 strip-types 不解析路径别名——测试文件直接 import '../src/utils/readerBooks.ts' 时 `@/api/reader` 解析会失败。两种解法取一（执行时验证）：(a) `readerBooks.ts` 用相对导入 `import type { ReaderBook } from '../api/reader'`（现有 utils/readerImages.ts 正是这么做的，采用此项）；(b) 类型内联。**采用 (a)。**

reader.ts（`localFileUrl` 前追加）：

```ts
/** A bookshelf entry (server-stored, shared across all users). */
export interface BookProgress {
  lastFile?: string | null
  position: number
  pageCount?: number
  updatedAt: number
}

export interface ReaderBook {
  id: string
  path: string
  kind: 'folder' | 'pdf'
  name: string
  description: string
  category: string
  addedAt: number
  progress?: BookProgress
}

/** Get the bookshelf from the server. */
export function getReaderBooks(): Promise<ToolEnvelope<{ books: ReaderBook[] }>> {
  return callTool('get_reader_books', {}) as unknown as Promise<
    ToolEnvelope<{ books: ReaderBook[] }>
  >
}

/** Save the full bookshelf to the server (replaces existing). */
export function saveReaderBooks(
  books: ReaderBook[],
): Promise<ToolEnvelope<{ ok: boolean; count: number }>> {
  return callTool('save_reader_books', { books }) as unknown as Promise<
    ToolEnvelope<{ ok: boolean; count: number }>
  >
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm test && npx vue-tsc -b`
Expected: 新测试全 PASS，既有测试无回归，类型干净。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/utils/readerBooks.ts frontend/src/api/reader.ts frontend/tests/readerBooks.test.ts
git commit -m "feat(reader): bookshelf types, api wrappers and pure helpers"
```

---

### Task 3: 前端 — useBookshelf composable（共享状态 + 持久化）

**Files:**
- Create: `frontend/src/composables/useBookshelf.ts`
- Test: `frontend/tests/useBookshelf.test.ts`

**Interfaces:**
- Consumes: Task 2 的 `getReaderBooks`/`saveReaderBooks`/`ReaderBook`。
- Produces（Task 4/6/7 消费）:
  - `createBookshelf(deps: { load: () => Promise<ReaderBook[]>; persist: (books: ReaderBook[]) => Promise<void> })` → `{ books: Ref<ReaderBook[]>; loaded: Ref<boolean>; loadError: Ref<string>; ensureLoaded(): Promise<void>; addBook(book: ReaderBook): Promise<boolean>; updateBook(book: ReaderBook): Promise<boolean>; removeBook(id: string): Promise<boolean>; updateProgress(id: string, patch: Partial<BookProgress>): void; findBook(path: string): ReaderBook | undefined }`
  - `useBookshelf()`：模块级单例（真实 API 依赖），Reader 与 BookshelfView 共享同一状态。
- 语义：CRUD 失败回滚并返回 false（FR-11）；`updateProgress` 乐观更新 + 后台保存，失败仅 console.warn（进度不回滚不阻断）。

- [ ] **Step 1: 写失败测试** `frontend/tests/useBookshelf.test.ts`

```ts
import assert from 'node:assert/strict'
import test from 'node:test'

import { createBookshelf } from '../src/composables/useBookshelf.ts'
import type { ReaderBook } from '../src/api/reader.ts'

function book(id: string, path: string): ReaderBook {
  return { id, path, kind: 'folder', name: id, description: '', category: '', addedAt: 1 }
}

function makeDeps() {
  let saved: ReaderBook[] = []
  let fail = false
  const deps = {
    load: async () => [{ ...book('a', '/a') }],
    persist: async (b: ReaderBook[]) => {
      if (fail) throw new Error('boom')
      saved = b
    },
    get saved() { return saved },
    set fail(v: boolean) { fail = v },
  }
  return deps
}

test('ensureLoaded fetches once and caches', async () => {
  const deps = makeDeps()
  let calls = 0
  deps.load = async () => { calls++; return [book('a', '/a')] }
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  await shelf.ensureLoaded()
  assert.equal(calls, 1)
  assert.equal(shelf.books.value.length, 1)
  assert.equal(shelf.loaded.value, true)
})

test('addBook persists; failure rolls back and returns false', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(await shelf.addBook(book('b', '/b')), true)
  assert.equal(shelf.books.value.length, 2)
  deps.fail = true
  assert.equal(await shelf.addBook(book('c', '/c')), false)
  assert.equal(shelf.books.value.length, 2)
})

test('removeBook persists; updateBook replaces by id', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  const renamed = { ...book('a', '/a'), name: 'renamed' }
  assert.equal(await shelf.updateBook(renamed), true)
  assert.equal(shelf.books.value[0].name, 'renamed')
  assert.equal(await shelf.removeBook('a'), true)
  assert.equal(shelf.books.value.length, 0)
  deps.fail = true
  assert.equal(await shelf.removeBook('zzz'), false) // 不存在的 id 也走保存失败路径
})

test('updateProgress merges patch with updatedAt and keeps book on save failure', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  shelf.updateProgress('a', { lastFile: '/a/x.md', position: 0.5 })
  const p = shelf.books.value[0].progress!
  assert.equal(p.lastFile, '/a/x.md')
  assert.equal(p.position, 0.5)
  assert.ok(p.updatedAt > 0)
  await new Promise((r) => setTimeout(r, 10))
  deps.fail = true
  shelf.updateProgress('a', { position: 0.9 }) // 失败不回滚、不抛出
  assert.equal(shelf.books.value[0].progress!.position, 0.9)
})

test('findBook matches exact path', async () => {
  const deps = makeDeps()
  const shelf = createBookshelf(deps)
  await shelf.ensureLoaded()
  assert.equal(shelf.findBook('/a')?.id, 'a')
  assert.equal(shelf.findBook('/zzz'), undefined)
})
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm test`
Expected: useBookshelf 测试 FAIL（模块不存在）。（注：composable 引入 vue 的 `ref`，node ESM 下 vue 依赖可正常加载；若执行时遇阻，把状态容器从 vue 换成最小 `{ value }` 观察者对象即可——接口不变。）

- [ ] **Step 3: 实现** `frontend/src/composables/useBookshelf.ts`

```ts
/**
 * Shared bookshelf state (see docs/requirement/10-reader-bookshelf.md).
 * createBookshelf takes injectable load/persist for node tests; useBookshelf
 * is the app-wide singleton wired to the real tool API, shared by Reader.vue
 * and BookshelfView.vue.
 */
import { ref, type Ref } from 'vue'
import { getReaderBooks, saveReaderBooks, type BookProgress, type ReaderBook } from '@/api/reader'

export interface BookshelfDeps {
  load: () => Promise<ReaderBook[]>
  persist: (books: ReaderBook[]) => Promise<void>
}

export interface Bookshelf {
  books: Ref<ReaderBook[]>
  loaded: Ref<boolean>
  loadError: Ref<string>
  ensureLoaded: () => Promise<void>
  addBook: (book: ReaderBook) => Promise<boolean>
  updateBook: (book: ReaderBook) => Promise<boolean>
  removeBook: (id: string) => Promise<boolean>
  updateProgress: (id: string, patch: Partial<BookProgress>) => void
  findBook: (path: string) => ReaderBook | undefined
}

export function createBookshelf(deps: BookshelfDeps): Bookshelf {
  const books = ref<ReaderBook[]>([])
  const loaded = ref(false)
  const loadError = ref('')

  async function ensureLoaded() {
    if (loaded.value) return
    try {
      const list = await deps.load()
      books.value = list
      loaded.value = true
      loadError.value = ''
    } catch (e) {
      loadError.value = (e as Error)?.message || '书架加载失败'
    }
  }

  /** Strict CRUD: optimistic update, rollback + false on persist failure (FR-11). */
  async function mutate(next: ReaderBook[]): Promise<boolean> {
    const prev = books.value
    books.value = next
    try {
      await deps.persist(next)
      return true
    } catch (e) {
      books.value = prev
      console.warn('书架保存失败:', e)
      return false
    }
  }

  function addBook(book: ReaderBook) {
    return mutate([...books.value, book])
  }

  function updateBook(book: ReaderBook) {
    return mutate(books.value.map((b) => (b.id === book.id ? book : b)))
  }

  function removeBook(id: string) {
    return mutate(books.value.filter((b) => b.id !== id))
  }

  /** Progress: optimistic + fire-and-forget. Never rolls back or throws. */
  function updateProgress(id: string, patch: Partial<BookProgress>) {
    const idx = books.value.findIndex((b) => b.id === id)
    if (idx < 0) return
    const book = books.value[idx]
    const next: ReaderBook = {
      ...book,
      progress: { ...{ lastFile: null, position: 0, updatedAt: 0 }, ...book.progress, ...patch, updatedAt: Date.now() },
    }
    books.value = books.value.map((b, i) => (i === idx ? next : b))
    void deps.persist(books.value).catch((e) => console.warn('进度保存失败:', e))
  }

  function findBook(path: string) {
    return books.value.find((b) => b.path === path)
  }

  return { books, loaded, loadError, ensureLoaded, addBook, updateBook, removeBook, updateProgress, findBook }
}

// ── app-wide singleton ────────────────────────────────────────────────
let singleton: Bookshelf | null = null

export function useBookshelf(): Bookshelf {
  singleton ??= createBookshelf({
    load: async () => {
      const res = await getReaderBooks()
      if (res.status !== 'success' || !res.result) throw new Error(res.error?.message || '书架加载失败')
      return res.result.books
    },
    persist: async (list) => {
      const res = await saveReaderBooks(list)
      if (res.status !== 'success') throw new Error(res.error?.message || '书架保存失败')
    },
  })
  return singleton
}
```

注意（同 Task 2 原因）：测试用相对导入；源文件内 `@/api/reader` 别名由 vite/vue-tsc 正常解析，但 node --test 直接 import 本文件时别名会失败——**本文件对 `@/api/reader` 使用相对路径 `../api/reader`**（与 `@/utils/markdownImages` 在 useMarkdownRender 中的既有做法相反，但 readerImages.ts 已证明 utils/composables 层用相对导入可行且无害）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm test && npx vue-tsc -b`
Expected: 全部 PASS，类型干净。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/composables/useBookshelf.ts frontend/tests/useBookshelf.test.ts
git commit -m "feat(reader): shared bookshelf composable with rollback semantics"
```

---

### Task 4: 前端 — BookshelfView 组件（卡片/筛选/增删改）

**Files:**
- Create: `frontend/src/components/reader/BookshelfView.vue`
- Modify: `frontend/src/views/Reader.vue`（仅临时挂载验证用，正式接线在 Task 5；本任务结束前撤掉临时挂载）——**改为不挂载**：本任务只交付组件 + `vue-tsc` 通过，渲染验证统一放 Task 8 E2E。

**Interfaces:**
- Consumes: `useBookshelf()`、`statLocalPath`、Task 2 utils、`ElMessage`。
- Produces: `<BookshelfView @open="(book: ReaderBook) => void" />`（组件 emits `open`；增删改在组件内部完成）。Reader.vue Task 7 消费 `open`。

- [ ] **Step 1: 组件实现** `frontend/src/components/reader/BookshelfView.vue`

```vue
<template>
  <div class="bookshelf">
    <div class="shelf-toolbar">
      <div class="shelf-chips" role="tablist" aria-label="类别筛选">
        <button
          v-for="c in categories"
          :key="c"
          type="button"
          class="shelf-chip"
          :class="{ active: activeCategory === c }"
          @click="activeCategory = c"
        >{{ c }}</button>
      </div>
      <el-button type="primary" @click="openAdd()">+ 添加</el-button>
    </div>

    <div v-if="loadError" class="shelf-state error">⚠️ {{ loadError }}</div>
    <div v-else-if="!books.length" class="shelf-state">
      <el-icon class="ss-icon"><Collection /></el-icon>
      <p>书架还是空的</p>
      <p class="ss-hint">把一个 Markdown 文件夹或 PDF 登记为书，点击即回到上次读到的位置</p>
      <el-button type="primary" @click="openAdd()">添加第一本书</el-button>
    </div>
    <div v-else class="shelf-grid">
      <div
        v-for="b in visibleBooks"
        :key="b.id"
        class="book-card glass-surface"
        :title="b.path"
        @click="emit('open', b)"
      >
        <div class="bc-head">
          <el-icon class="bc-kind-icon"><FolderOpened v-if="b.kind === 'folder'" /><Document v-else /></el-icon>
          <span class="bc-name">{{ b.name }}</span>
        </div>
        <span v-if="b.category" class="bc-category">{{ b.category }}</span>
        <p class="bc-desc">{{ b.description || '　' }}</p>
        <div class="bc-foot">
          <span class="bc-progress">{{ progressLabel(b) }}</span>
          <span class="bc-actions" @click.stop>
            <button type="button" title="编辑" aria-label="编辑" @click="openEdit(b)"><el-icon><EditPen /></el-icon></button>
            <button type="button" title="删除" aria-label="删除" @click="confirmRemove(b)"><el-icon><Delete /></el-icon></button>
          </span>
        </div>
      </div>
    </div>

    <el-dialog
      v-model="formVisible"
      :title="editingId ? '编辑书籍' : '添加书籍'"
      width="min(480px, 92vw)"
      :close-on-click-modal="false"
      append-to-body
    >
      <el-form label-position="top" @submit.prevent>
        <el-form-item label="路径（文件夹或 PDF 文件）" :error="formError || undefined">
          <el-input v-model="form.path" placeholder="/Users/you/Documents/book.pdf" @blur="onPathBlur" />
        </el-form-item>
        <el-form-item label="书名">
          <el-input v-model="form.name" placeholder="留空则使用文件名" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="2" placeholder="这本书讲什么（可空）" />
        </el-form-item>
        <el-form-item label="类别">
          <el-input v-model="form.category" placeholder="如：技术 / 论文 / 小说（可空）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="formVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="submit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Collection, Delete, Document, EditPen, FolderOpened } from '@element-plus/icons-vue'
import { statLocalPath, type PathStat, type ReaderBook } from '@/api/reader'
import { useBookshelf } from '@/composables/useBookshelf'
import {
  bookProgressLabel, defaultBookName, deriveKind, makeBookId, sortBooks, validateBookForm,
} from '@/utils/readerBooks'

const emit = defineEmits<{ open: [book: ReaderBook] }>()

const shelf = useBookshelf()
const books = computed(() => sortBooks(shelf.books.value))
const loadError = computed(() => shelf.loadError.value)

const activeCategory = ref('全部')
const categories = computed(() => ['全部', ...new Set(shelf.books.value.map((b) => b.category).filter(Boolean))])
const visibleBooks = computed(() =>
  activeCategory.value === '全部' ? books.value : books.value.filter((b) => b.category === activeCategory.value),
)

function progressLabel(b: ReaderBook) {
  return bookProgressLabel(b)
}

// ── add / edit dialog ────────────────────────────────────────────────
const formVisible = ref(false)
const saving = ref(false)
const editingId = ref<string | null>(null)
const form = reactive({ path: '', name: '', description: '', category: '' })
const formError = ref('')
const pathStat = ref<PathStat | null>(null)

function openAdd() {
  editingId.value = null
  form.path = ''
  form.name = ''
  form.description = ''
  form.category = ''
  formError.value = ''
  pathStat.value = null
  formVisible.value = true
}

function openEdit(b: ReaderBook) {
  editingId.value = b.id
  form.path = b.path
  form.name = b.name
  form.description = b.description
  form.category = b.category
  formError.value = ''
  pathStat.value = null
  formVisible.value = true
  void refreshStat()
}

async function refreshStat() {
  if (!form.path.trim()) { pathStat.value = null; return }
  try {
    const res = await statLocalPath(form.path.trim())
    pathStat.value = res.status === 'success' && res.result ? res.result : null
  } catch {
    pathStat.value = null
  }
  validate()
}

function onPathBlur() {
  // 书名留空时，随路径默认取文件名
  if (!editingId.value && !form.name.trim()) form.name = defaultBookName(form.path.trim())
  void refreshStat()
}

function validate() {
  formError.value = validateBookForm(form.path.trim(), editingId.value, shelf.books.value, pathStat.value) ?? ''
}

watch(() => form.path, () => { if (formVisible.value) validate() })

async function submit() {
  await refreshStat()
  validate()
  if (formError.value) return
  const path = form.path.trim()
  const base = {
    path,
    kind: deriveKind(path),
    name: form.name.trim() || defaultBookName(path),
    description: form.description.trim(),
    category: form.category.trim(),
  }
  saving.value = true
  try {
    const ok = editingId.value
      ? await shelf.updateBook({ ...(shelf.findBook(shelf.books.value.find((b) => b.id === editingId.value)!.path) ?? shelf.books.value.find((b) => b.id === editingId.value)!), ...base, id: editingId.value })
      : await shelf.addBook({ ...base, id: makeBookId(), addedAt: Date.now() })
    if (!ok) {
      ElMessage.error('保存失败，请重试')
      return
    }
    formVisible.value = false
  } finally {
    saving.value = false
  }
}

async function confirmRemove(b: ReaderBook) {
  try {
    await ElMessageBox.confirm(`删除「${b.name}」？阅读进度会一并移除（不会动磁盘文件）。`, '删除书籍', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
  } catch {
    return
  }
  const ok = await shelf.removeBook(b.id)
  if (!ok) ElMessage.error('删除失败，请重试')
}
</script>

<style scoped>
.bookshelf { min-height: 100%; display: flex; flex-direction: column; gap: 16px; }
.shelf-toolbar { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.shelf-chips { display: flex; gap: 8px; flex-wrap: wrap; flex: 1; min-width: 0; }
.shelf-chip {
  min-height: 32px; padding: 0 14px; border-radius: 999px; border: 1px solid var(--border-glass);
  background: var(--bg-glass); color: var(--text-muted); font-size: 13px; font-weight: 550; cursor: pointer;
  transition: var(--transition-interactive);
}
.shelf-chip:hover { color: var(--text-primary); }
.shelf-chip.active { color: var(--accent); border-color: var(--accent-border); background: color-mix(in srgb, var(--accent) 12%, transparent); }

.shelf-state { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--text-faint); padding: 60px 0; }
.shelf-state.error { color: #f87171; }
.ss-icon { font-size: 40px; }
.ss-hint { font-size: 13px; }

.shelf-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 14px; }
.book-card {
  display: flex; flex-direction: column; gap: 8px; padding: 16px; border-radius: 16px; cursor: pointer;
  transition: transform var(--motion-normal) var(--ease-spring-gentle), box-shadow var(--motion-normal) ease;
}
.book-card:hover { transform: translateY(-2px); box-shadow: var(--shadow-md); }
.bc-head { display: flex; align-items: center; gap: 8px; min-width: 0; }
.bc-kind-icon { flex: none; color: var(--accent); font-size: 18px; }
.bc-name { font-weight: 620; font-size: 15px; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bc-category {
  align-self: flex-start; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  font-size: 11px; font-weight: 560; padding: 2px 9px; border-radius: 999px;
  color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.bc-desc {
  margin: 0; font-size: 13px; line-height: 1.5; color: var(--text-secondary); min-height: 2.9em;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
}
.bc-foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: auto; }
.bc-progress { font-size: 12px; color: var(--text-faint); }
.bc-actions { display: flex; gap: 2px; opacity: 0.75; }
.bc-actions button {
  display: grid; place-items: center; width: 28px; height: 28px; border: 0; border-radius: 8px;
  background: transparent; color: var(--text-muted); cursor: pointer; transition: var(--transition-interactive);
}
.bc-actions button:hover { color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, transparent); }

@media (max-width: 768px) {
  .shelf-grid { grid-template-columns: repeat(2, 1fr); gap: 10px; }
  .book-card { padding: 12px; border-radius: 14px; }
  .bc-desc { font-size: 12px; }
  .shelf-toolbar .el-button { min-height: var(--tap-target); }
}
</style>
```

（`submit` 中编辑分支的 `findBook(...)` 写复杂了——执行时简化为：`const current = shelf.books.value.find(b => b.id === editingId.value)!; await shelf.updateBook({ ...current, ...base }`，保留 `progress`/`addedAt`。）

- [ ] **Step 2: 类型与构建检查**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 类型干净、测试全绿（组件无新单测——逻辑在 Task 2/3 已覆盖，渲染验证在 Task 8）。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/reader/BookshelfView.vue
git commit -m "feat(reader): bookshelf view with cards, category chips and CRUD dialog"
```

---

### Task 5: Reader 集成 — viewMode 滑块切换

**Files:**
- Modify: `frontend/src/views/Reader.vue`（template page-header / topbar+body 包裹；script 增 viewMode/changeView；style 增 .view-switch）

**Interfaces:**
- Consumes: `BookshelfView`（Task 4）。
- Produces（Task 7 消费）: `viewMode: Ref<'shelf' | 'read'>`、`changeView(mode)`；BookshelfView 挂载点 `<BookshelfView v-show="viewMode === 'shelf'" @open="openBook" />`（`openBook` Task 7 实现，本任务先给占位空函数以过类型）。

- [ ] **Step 1: script 增加 viewMode 状态**（放在 `const tocDrawer = ref(false)` 附近；确认顶部已 import `useRoute`/`useRouter`，缺则补 `import { useRoute, useRouter } from 'vue-router'`）

```ts
// ── view switch (shelf ↔ read), Tasks-style slider ───────────────────
const VIEW_STORAGE_KEY = 'reader.view'
type ReaderView = 'shelf' | 'read'

function initialViewMode(): ReaderView {
  const q = route.query.view
  if (q === 'shelf' || q === 'read') return q
  const saved = localStorage.getItem(VIEW_STORAGE_KEY)
  return saved === 'read' ? 'read' : 'shelf'
}

const viewMode = ref<ReaderView>(initialViewMode())

function changeView(mode: ReaderView) {
  viewMode.value = mode
  localStorage.setItem(VIEW_STORAGE_KEY, mode)
  void router.replace({ query: { ...route.query, view: mode } })
  // Entering the shelf from an immersive/fullscreen reading session restores the shell.
  if (mode === 'shelf') {
    if (isMobileImmersive.value) leaveMobileImmersive()
    else if (isFullscreen.value) void toggleFullscreen()
  }
}
```

（若 `route`/`router` 尚未存在于组件：`const route = useRoute(); const router = useRouter()`。执行时核对 `toggleFullscreen`/`leaveMobileImmersive`/`isFullscreen`/`isMobileImmersive` 的实际名称——它们已在文件中存在。）

- [ ] **Step 2: template 接线**

page-header（12-17 行）改为：

```html
    <header class="page-header">
      <div>
        <h1 class="page-title">阅境轩</h1>
        <p class="page-subtitle">浏览本地 Markdown 与 PDF，沉浸阅读</p>
      </div>
      <div class="view-switch" aria-label="视图切换">
        <span class="switch-indicator" :class="{ read: viewMode === 'read' }"></span>
        <button type="button" :class="{ active: viewMode === 'shelf' }" @click="changeView('shelf')">书架</button>
        <button type="button" :class="{ active: viewMode === 'read' }" @click="changeView('read')">阅读</button>
      </div>
    </header>
```

`.reader-topbar` 与 `.reader-body` 两个元素各加 `v-show="viewMode === 'read'"`；紧随其后（两个 history overlay 之前或之后均可，选 reader-body 之后）插入：

```html
    <!-- Bookshelf view (kept alive via v-show alongside the reading panes) -->
    <BookshelfView v-show="viewMode === 'shelf'" @open="openBook" />
```

script 增加：`import BookshelfView from '@/components/reader/BookshelfView.vue'` 与占位 `function openBook(_book: ReaderBook) {}`（Task 7 替换实现；`import type { ReaderBook } from '@/api/reader'`）。

- [ ] **Step 3: style 增滑块样式**（scoped style 尾部，复制自 Tasks.vue 并改第二个位置类名为 `read`；`.reader-page .page-header` 需要横向布局——先查现有全局 `.page-header` 是否已 flex（其他页面有 header-actions），若是则无需改）

```css
/* Tasks-style view switch */
.view-switch { position: relative; display: grid; grid-template-columns: 1fr 1fr; width: 174px; padding: 3px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 5%, transparent); isolation: isolate; align-self: center; flex-shrink: 0; }
.switch-indicator { position: absolute; inset: 3px auto 3px 3px; width: calc(50% - 3px); border-radius: 10px; background: var(--bg-glass-strong); box-shadow: var(--shadow-sm), var(--inset-highlight); transition: transform var(--motion-normal) var(--ease-spring-gentle); z-index: -1; }
.switch-indicator.read { transform: translateX(100%); }
.view-switch button { min-height: 36px; border: 0; background: transparent; color: var(--text-muted); font-weight: 570; cursor: pointer; }
.view-switch button.active { color: var(--text-primary); }

/* Bookshelf fills the body area like reader-body does */
.bookshelf-root { flex: 1 1 auto; min-height: 0; overflow-y: auto; }
```

给 BookshelfView 的挂载点加 class：`<BookshelfView v-show="viewMode === 'shelf'" class="bookshelf-root" ... />`（组件根元素 class 透传）。移动端 `@media (max-width: 768px)` 块内补 `.view-switch { width: 150px; } .view-switch button { min-height: 34px; }`。

- [ ] **Step 4: 手动验证（dev server）+ gates**

Run: `cd frontend && npx vite --port 5174 &`，浏览器/harness 打开 `http://localhost:5174/reader`：
- 默认落书架（空态文案）；点「阅读」滑块切换到原有界面且已打开文件夹还在（v-show 保活）；`?view=read` 直链进阅读。
Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 切换正常、类型/测试干净。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/views/Reader.vue
git commit -m "feat(reader): shelf/read view switch with keep-alive slider"
```

---

### Task 6: Reader 集成 — 进度记录

**Files:**
- Modify: `frontend/src/views/Reader.vue`

**Interfaces:**
- Consumes: `useBookshelf()`（Task 3）、`scrollRatio`（Task 2）。
- Produces: 进度写入书架（服务端可见 `reader_books` 变化）；`currentShelfBookId: Ref<string | null>`（Task 7 复用匹配逻辑）。

- [ ] **Step 1: 接入 composable 与当前书匹配**（script，viewMode 段之后）

```ts
// ── bookshelf progress tracking ──────────────────────────────────────
const shelf = useBookshelf()

/** The shelf book the current reading session belongs to, if any. */
const currentShelfBookId = computed<string | null>(() => {
  const books = shelf.books.value
  if (fileKind.value === 'pdf') {
    return books.find((b) => b.kind === 'pdf' && b.path === displayedFile.value)?.id ?? null
  }
  return books.find((b) => b.kind === 'folder' && b.path === rootPath.value)?.id ?? null
})
```

`onMounted` 里追加 `void shelf.ensureLoaded()`。

- [ ] **Step 2: md 滚动防抖记录**（`processContentScroll` 附近）

```ts
const PROGRESS_DEBOUNCE_MS = 1500
let progressTimer: ReturnType<typeof setTimeout> | null = null

/** Debounced capture of the md scroll ratio (FR-15). */
function scheduleProgressCapture() {
  if (fileKind.value === 'pdf') return
  if (progressTimer) clearTimeout(progressTimer)
  progressTimer = setTimeout(() => {
    progressTimer = null
    const bookId = currentShelfBookId.value
    const el = contentRef.value
    if (!bookId || !el || !displayedFile.value) return
    shelf.updateProgress(bookId, {
      lastFile: displayedFile.value,
      position: scrollRatio(el.scrollTop, el.scrollHeight, el.clientHeight),
    })
  }, PROGRESS_DEBOUNCE_MS)
}

function flushProgressNow() {
  if (progressTimer) { clearTimeout(progressTimer); progressTimer = null }
  const bookId = currentShelfBookId.value
  const el = contentRef.value
  if (!bookId || !el || !displayedFile.value) return
  if (fileKind.value === 'pdf') return // pdf 进度由 pagechange 即时记录
  shelf.updateProgress(bookId, { lastFile: displayedFile.value, position: scrollRatio(el.scrollTop, el.scrollHeight, el.clientHeight) })
}
```

`processContentScroll()` 内（TOC 跟踪段之前）追加 `scheduleProgressCapture()`。

- [ ] **Step 3: pdf 页码即时记录**

template 的 `@pagechange="pdfCurrentPage = $event"` 与 `@pagecount="pdfPageCount = $event"` 改为方法：

```html
            @pagechange="onPdfPageChange"
            @pagecount="onPdfPageCount"
```

```ts
function onPdfPageChange(page: number) {
  pdfCurrentPage.value = page
  const bookId = currentShelfBookId.value
  if (bookId) shelf.updateProgress(bookId, { position: page, ...(pdfPageCount.value ? { pageCount: pdfPageCount.value } : {}) })
}

function onPdfPageCount(count: number) {
  pdfPageCount.value = count
  const bookId = currentShelfBookId.value
  if (bookId) shelf.updateProgress(bookId, { pageCount: count })
}
```

- [ ] **Step 4: 文件切换与退出时落盘**

`onSelectFile` 成功路径（`displayedFile.value = path` 赋值处之后）追加：

```ts
    // 文件切换：md 书进度重置到新文件顶部（FR-15 lastFile 记录）
    if (fileKind.value !== 'pdf') {
      const bookId = books?.find((b) => b.kind === 'folder' && b.path === rootPath.value)?.id
      if (bookId) shelf.updateProgress(bookId, { lastFile: path, position: 0 })
    }
```

（实现时按 onSelectFile 实际结构调整——用 `currentShelfBookId.value` 亦可，注意它依赖的 `displayedFile` 已更新；简化为 `const bookId = currentShelfBookId.value; if (bookId && fileKind.value !== 'pdf') shelf.updateProgress(bookId, { lastFile: path, position: 0 })`。）

`changeView` 内追加 `flushProgressNow()`（切视图先落盘）；`onBeforeUnmount`（若无则新增）：

```ts
onBeforeUnmount(() => {
  flushProgressNow()
})
```

（与既有 onMounted/onBeforeUnmount 合并，勿重复注册。）

- [ ] **Step 5: 验证 + gates**

Harness（/tmp/mobile-repro 新脚本 verify-shelf-progress.mjs，desktop 1440×900）：打开既有文件夹书（先经 Task 8 的加书流程——本任务验证改用手动注入：`page.evaluate` 调 `fetch('/v1/tools/call', {method:'POST', body: JSON.stringify({tool:'save_reader_books', args:{books:[…vault 文件夹书]}})})` 预置书架），进入阅读滚动 → 等 2s → 再 `get_reader_books` 断言 `progress.position ∈ (0,1]` 且 `lastFile` 正确；PDF 同理断言页码。
Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 断言通过、类型/测试干净。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/views/Reader.vue
git commit -m "feat(reader): record per-book reading progress (md ratio, pdf page)"
```

---

### Task 7: Reader 集成 — 从书架开书 + 进度恢复

**Files:**
- Modify: `frontend/src/views/Reader.vue`

**Interfaces:**
- Consumes: `openBook`（BookshelfView `open` 事件）、`clampPdfPage`、`holdHeaderForJump`（既有）、`flatFiles`（既有 computed）。
- Produces: `openBook(book: ReaderBook): Promise<void>`；`pendingRestoreRatio: number | null`、`pendingPdfPage: number | null` 内部状态。

- [ ] **Step 1: 实现 openBook**（替换 Task 5 占位）

```ts
// Pending restore targets consumed after the file finishes rendering.
let pendingRestoreRatio: number | null = null
let pendingPdfPage: number | null = null

/** Open a shelf book and restore its progress (FR-12..14). */
async function openBook(book: ReaderBook) {
  changeView('read')
  if (book.kind === 'pdf') {
    const dir = book.path.substring(0, book.path.lastIndexOf('/'))
    await openPath(dir)
    pendingPdfPage = book.progress ? clampPdfPage(book.progress.position, book.progress.pageCount ?? 0) : null
    await onSelectFile(book.path)
    return
  }
  await openPath(book.path)
  const p = book.progress
  if (p?.lastFile && flatFiles.value.includes(p.lastFile)) {
    pendingRestoreRatio = p.position
    await onSelectFile(p.lastFile)
  } else if (flatFiles.value.length) {
    // Fallback (FR-13): stale/missing lastFile → first file, from the top.
    await onSelectFile(flatFiles.value[0])
  }
}
```

- [ ] **Step 2: md 恢复消费点**（`onArticleEnter` 内 `await enhance(el as HTMLElement)` 之后）

```ts
  // Book progress restore (FR-13): scroll to the saved ratio after enhance,
  // so images/code highlighting have settled into the layout.
  if (pendingRestoreRatio !== null) {
    const ratio = pendingRestoreRatio
    pendingRestoreRatio = null
    await nextTick()
    const el2 = contentRef.value
    if (el2) {
      el2.scrollTop = Math.round(ratio * (el2.scrollHeight - el2.clientHeight))
      // 防止恢复滚动立即触发一次进度回写（值相同，无害，但跳过更干净）
      if (progressTimer) { clearTimeout(progressTimer); progressTimer = null }
    }
  }
```

（`onArticleEnter` 只对 ARTICLE 生效——md 路径正确。）

- [ ] **Step 3: pdf 恢复消费点**（`onPdfPageCount` 内追加）

```ts
function onPdfPageCount(count: number) {
  pdfPageCount.value = count
  const bookId = currentShelfBookId.value
  if (bookId) shelf.updateProgress(bookId, { pageCount: count })
  // Book-open restore (FR-14): page wraps mount with pageMetas, so one rAF
  // after count arrives the target wrap is addressable.
  if (pendingPdfPage !== null) {
    const target = clampPdfPage(pendingPdfPage, count)
    pendingPdfPage = null
    requestAnimationFrame(() => {
      holdHeaderForJump() // 恢复跳转同样不让头部塌陷（与 TOC 跳转同族）
      pdfViewerRef.value?.scrollToPage(target)
    })
  }
}
```

- [ ] **Step 4: 验证 + gates**

Harness（verify-shelf-restore.mjs）：
1. 预置书架（tools/call 注入）：vault Timeline 文件夹书（progress: lastFile=某 md, position=0.5）+ `/tmp/mobile-repro/pdfs/test-outline.pdf` 书（position=5, pageCount=6）。
2. `/reader` 落书架 → 点文件夹书卡 → 断言：viewMode=read、displayedFile=lastFile、`pane.scrollTop/(scrollHeight-clientHeight)` ∈ [0.48, 0.52]。
3. 切回书架 → 点 PDF 书卡 → 等 1.5s → 断言 `pdf-page-wrap[data-page-num="5"]` 的 boundingRect.top 距 pane 顶 < 60px，且 page indicator 显示 5。
4. lastFile 失效回退：注入 lastFile=不存在的路径 → 点开 → 断言打开的是 flatFiles[0] 且 scrollTop=0。
Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 全部断言通过。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/views/Reader.vue
git commit -m "feat(reader): open shelf books and restore md scroll / pdf page"
```

---

### Task 8: E2E 全流程验证 + 文档 + 合并安装

**Files:**
- Create: `/tmp/mobile-repro/verify-bookshelf-e2e.mjs`（harness，不入库）
- Modify: `docs/requirement/10-reader-bookshelf.md`（状态 → 已实现；修订历史）
- Modify: `docs/development/02-tool-protocol.md`（若其中列有 reader 工具清单则补两个新工具；grep 确认）

- [ ] **Step 1: 全流程 E2E（desktop 1440×900 + mobile 390×844）**

脚本覆盖验收标准 1-7：
1. 书架空态 → 「+ 添加」→ 输入 `/tmp/mobile-repro/pdfs` 文件夹路径 → 保存 → 卡片出现（类别 chip、进度「未开始」）。
2. 再添加 `/tmp/mobile-repro/pdfs/test-outline.pdf`（同名类别的去重 chips、进度文案）。
3. 重复添加同路径 → 表单报「已在书架」。
4. 非法路径 `/tmp/definitely-missing` → 「路径不存在」。
5. 点 PDF 书卡 → 阅读视图 → `pagechange` 后切书架 → 卡片显示「第 N/6 页」→ 点回 → 恢复第 N 页。
6. vault 文件夹书：滚动到中部 → 等 2s → 切书架（进度落盘）→ 点回 → 滚动比例恢复 ±2%。
7. `?view=read` 直链与 localStorage 记忆断言；刷新后书架仍在（服务端）。
8. 移动端：滑块可点、卡片两列、弹窗可用、加书成功。
全部以 DOM 断言输出 PASS/FAIL 明细。

- [ ] **Step 2: 全量 gates**

```bash
cd backend && cargo fmt --check && cargo clippy -- -D warnings && cargo test
cd ../frontend && npx vue-tsc -b && npm test
```

- [ ] **Step 3: 文档更新 + spec 状态**（10-reader-bookshelf.md 状态改「已实现」、修订历史加行；02-tool-protocol.md 视 grep 结果）

- [ ] **Step 4: Commit + 合并 + 安装**

```bash
git add docs/
git commit -m "docs(reader): bookshelf implemented, tool list updated"
git checkout main && git merge --ff-only feat/reader-bookshelf && git branch -d feat/reader-bookshelf
make install   # ~/.local/bin，重启由用户执行
```

并提醒用户重启 `obsidian-brain` 生效；杀掉 5174 dev server。

---

## Self-Review 记录

- **Spec 覆盖**：FR-1..6 → Task 4；FR-7..11 → Task 4（validateBookForm/updateProgress 回滚）；FR-12..14 → Task 7；FR-15..17 → Task 6；FR-18..20 → Task 5；存储规范/Tool API → Task 1；验收 1-8 → Task 8。无缺口。
- **占位符扫描**：无 TBD/TODO；Task 4 的 submit 编辑分支已注明执行时简化；Task 5/6/7 的锚点均给出代码。
- **类型一致性**：`ReaderBook`/`BookProgress` 字段在 Task 1（Rust camelCase serde）与 Task 2（TS）一致（id/path/kind/name/description/category/addedAt/progress{lastFile,position,pageCount,updatedAt}）；`useBookshelf` 返回签名 Task 3 定义、Task 4/6/7 消费一致；`clampPdfPage`/`scrollRatio`/`validateBookForm` 签名前后一致。
