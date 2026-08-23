# Timeline Scroll-Spy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 时光机左侧时间轴高亮随右侧小记列表滚动联动（scroll-spy），初次加载自动高亮最新一天，左轴自动跟随。

**Architecture:** 纯决策逻辑下沉到 `frontend/src/utils/timelineSpy.ts`（node --test 可测）；DOM 测量与 rAF 节流留在 `Timeline.vue`；复用现有 `selectedDate` / `.active` 样式，不新增视觉。

**Tech Stack:** Vue 3 `<script setup>` + TS、scoped CSS、node --test（frontend/tests 目录，`npm test`）。

**Spec:** `docs/superpowers/specs/2026-08-23-timeline-scroll-spy-design.md`

## Global Constraints

- 仅改前端与两份设计文档，**后端零改动**。
- 不新增前端依赖。
- 不改变点击跳转行为、移动端布局（≤768px 左轴隐藏、联动跳过）。
- 阈值固定 `90`（px），rAF 节流（每帧至多一次扫描）。
- 高亮复用 `selectedDate` 与现有 `.active` 样式。
- 测试风格与 `frontend/tests/taskDates.test.ts` 一致：`node:test` + `node:assert/strict`，从 `../src/utils/*.ts` 相对导入。
- 门禁：`cd frontend && npx vue-tsc -b && npm test`，零错误、全过。
- Commit 格式：Conventional Commits。

---

### Task 1: Scroll-spy 实现（utils + Timeline.vue + 测试 + 文档）

**Files:**
- Create: `frontend/src/utils/timelineSpy.ts`
- Create: `frontend/tests/timelineSpy.test.ts`
- Modify: `frontend/src/views/Timeline.vue`
- Modify: `docs/requirement/04-timeline.md`
- Modify: `docs/development/04-timeline.md`

**Interfaces:**
- Produces: `pickActiveDate(headers: SpyHeader[], threshold: number): string | null`（`SpyHeader = { date: string; top: number }`），导出自 `frontend/src/utils/timelineSpy.ts`；仅 `Timeline.vue` 消费。

- [ ] **Step 1: 写失败测试** — 新建 `frontend/tests/timelineSpy.test.ts`：

```ts
import assert from 'node:assert/strict'
import test from 'node:test'

import { pickActiveDate, type SpyHeader } from '../src/utils/timelineSpy.ts'

test('pickActiveDate returns the last header that crossed the threshold', () => {
  const headers: SpyHeader[] = [
    { date: '2026-08-23', top: -40 },
    { date: '2026-08-22', top: 30 },
    { date: '2026-08-21', top: 88 },
    { date: '2026-08-20', top: 91 },
    { date: '2026-08-19', top: 400 },
  ]
  assert.equal(pickActiveDate(headers, 90), '2026-08-21')
})

test('pickActiveDate falls back to the first header when none crossed the threshold', () => {
  const headers: SpyHeader[] = [
    { date: '2026-08-23', top: 120 },
    { date: '2026-08-22', top: 300 },
  ]
  assert.equal(pickActiveDate(headers, 90), '2026-08-23')
})

test('pickActiveDate returns null for an empty header list', () => {
  assert.equal(pickActiveDate([], 90), null)
})
```

- [ ] **Step 2: 跑测试确认失败** — `cd frontend && npm test` → `timelineSpy.test.ts` 报模块不存在（FAIL）。

- [ ] **Step 3: 写实现** — 新建 `frontend/src/utils/timelineSpy.ts`：

```ts
export interface SpyHeader {
  date: string
  /** Header top offset in px, relative to the scroll container's top edge. */
  top: number
}

/**
 * Pick the date whose group header is the last one to have crossed the
 * threshold (distance from the scroll container's top). Headers must be
 * ordered top-to-bottom as rendered; scanning stops at the first header
 * below the fold. Falls back to the first header when none has crossed
 * (scrolled to the very top); returns null for an empty list.
 */
export function pickActiveDate(headers: SpyHeader[], threshold: number): string | null {
  if (headers.length === 0) return null
  let active: string | null = null
  for (const header of headers) {
    if (header.top <= threshold) active = header.date
    else break
  }
  return active ?? headers[0].date
}
```

- [ ] **Step 4: 跑测试确认通过** — `cd frontend && npm test` → 全部通过（既有 31 个 + 新增 3 个）。

- [ ] **Step 5: 接线 Timeline.vue（模板）**

old:
```html
      <!-- Right Memo List -->
      <div class="memo-scroll" @scroll="onMemoScroll">
```
new:
```html
      <!-- Right Memo List -->
      <div class="memo-scroll" ref="memoScrollRef" @scroll="onMemoScroll">
```

old:
```html
              <div
                v-for="day in month.days"
                :key="day.date"
                class="day-link"
                :class="{ active: selectedDate === day.date }"
                @click="scrollToDate(day.date)"
              >
```
new:
```html
              <div
                v-for="day in month.days"
                :key="day.date"
                class="day-link"
                :class="{ active: selectedDate === day.date }"
                :data-date="day.date"
                @click="scrollToDate(day.date)"
              >
```

- [ ] **Step 6: 接线 Timeline.vue（脚本）**

old:
```ts
import { ref, computed, onMounted, onUnmounted } from 'vue'
```
new:
```ts
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
```

old:
```ts
import MotionModal from '@/components/motion/MotionModal.vue'
```
new:
```ts
import MotionModal from '@/components/motion/MotionModal.vue'
import { pickActiveDate, type SpyHeader } from '@/utils/timelineSpy'
```

old:
```ts
const selectedDate = ref('')
```
new:
```ts
const selectedDate = ref('')
const memoScrollRef = ref<HTMLElement | null>(null)
// Scroll-spy: left-nav highlight follows the day group at the top of the memo list.
const SPY_THRESHOLD = 90
let spyRafId: number | null = null
```

old:
```ts
function scrollToDate(date: string) {
  selectedDate.value = date
  document.getElementById('date-' + date)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

function onMemoScroll(e: Event) {
  const el = e.target as HTMLElement
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 100) loadMore()
}
```
new:
```ts
function scrollToDate(date: string) {
  selectedDate.value = date
  document.getElementById('date-' + date)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

function updateActiveFromScroll() {
  spyRafId = null
  const container = memoScrollRef.value
  if (!container || isMobile.value) return
  const baseTop = container.getBoundingClientRect().top
  const headers: SpyHeader[] = []
  for (const group of groupedMemos.value) {
    const el = document.getElementById('date-' + group.date)
    if (!el) continue
    const top = el.getBoundingClientRect().top - baseTop
    headers.push({ date: group.date, top })
    if (top > SPY_THRESHOLD) break
  }
  const next = pickActiveDate(headers, SPY_THRESHOLD)
  if (next === null) {
    selectedDate.value = ''
    return
  }
  if (next !== selectedDate.value) {
    selectedDate.value = next
    document.querySelector(`.day-link[data-date="${next}"]`)?.scrollIntoView({ block: 'nearest' })
  }
}

function requestSpyUpdate() {
  if (spyRafId !== null) return
  spyRafId = requestAnimationFrame(updateActiveFromScroll)
}

// Re-sync after data changes (load / load-more / filter / create) so the
// highlight never points at a date that left the list.
watch(groupedMemos, () => { nextTick(requestSpyUpdate) })

function onMemoScroll(e: Event) {
  const el = e.target as HTMLElement
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 100) loadMore()
  requestSpyUpdate()
}
```

old:
```ts
onUnmounted(() => {
  document.removeEventListener('keydown', onViewerKeydown)
  window.removeEventListener('resize', onResize)
  if (searchTimer) clearTimeout(searchTimer)
```
new:
```ts
onUnmounted(() => {
  document.removeEventListener('keydown', onViewerKeydown)
  window.removeEventListener('resize', onResize)
  if (spyRafId !== null) cancelAnimationFrame(spyRafId)
  if (searchTimer) clearTimeout(searchTimer)
```

- [ ] **Step 7: 门禁** — `cd frontend && npx vue-tsc -b && npm test` → 零错误、全过（34 tests）。

- [ ] **Step 8: 更新文档**

`docs/requirement/04-timeline.md` §2.1.2 UI 设计：

old:
```markdown
**UI 设计**：
- **布局**：左侧时间线（窄列），右侧内容区（宽列）
- **滚动方式**：无限滚动（向下滚动加载更多）
```
new:
```markdown
**UI 设计**：
- **布局**：左侧时间线（窄列），右侧内容区（宽列）
- **滚动联动**：左侧时间轴高亮跟随右侧视口顶部对应的日期（scroll-spy）；初次加载自动高亮最新一天，高亮日移出左轴视口时左轴自动跟随滚动
- **滚动方式**：无限滚动（向下滚动加载更多）
```

`docs/requirement/04-timeline.md` §5.3 UI 验收：

old:
```markdown
- [ ] 左侧时间线，右侧内容区
- [ ] 无限滚动正常
```
new:
```markdown
- [ ] 左侧时间线，右侧内容区
- [ ] 左侧时间轴高亮随右侧滚动联动，初次加载自动高亮最新一天
- [ ] 无限滚动正常
```

`docs/development/04-timeline.md` §6.2 组件设计：

old:
```markdown
**TimeMachine.vue**：
- 左侧时间线：年月日树形结构
- 右侧内容区：小记列表（无限滚动）
- 顶部工具栏：创建按钮、搜索框、筛选器
```
new:
```markdown
**TimeMachine.vue**：
- 左侧时间线：年月日树形结构；高亮为 scroll-spy——右侧列表滚动（rAF 节流）时取视口顶部最后一个越过 90px 阈值的日期分组头（`pickActiveDate`，utils/timelineSpy.ts），初次加载与数据变化后自动重同步，高亮日移出左轴时 `scrollIntoView({ block: 'nearest' })` 跟随；移动端左轴隐藏、联动跳过
- 右侧内容区：小记列表（无限滚动）
- 顶部工具栏：创建按钮、搜索框、筛选器
```

- [ ] **Step 9: 提交**

```bash
git add frontend/src/utils/timelineSpy.ts frontend/tests/timelineSpy.test.ts frontend/src/views/Timeline.vue docs/requirement/04-timeline.md docs/development/04-timeline.md
git commit -m "feat(timeline): left-nav scroll-spy highlight follows memo list"
```

---

### Task 2: 门禁 + 构建（无安装/重启）

**Files:** 无代码改动（验证任务）。

**Interfaces:** 无。

- [ ] **Step 1: 前端门禁** — `cd frontend && npx vue-tsc -b && npm test` → 零错误、34/34。
- [ ] **Step 2: 构建前端产物** — `cd frontend && npx vite build --outDir dist_new` → 成功，报告文件数。
- [ ] **Step 3: 构建发布二进制** — `cd backend && cargo build --release` → 成功，报告大小与 mtime（含新前端的 rust-embed 产物）。
- [ ] **Step 4: 不安装、不重启** — `sudo install` 与服务重启由用户执行；仅报告产物路径。
- [ ] **Step 5: 报告** — 汇总门禁输出、产物大小、树清洁状态。无需 commit（无代码变更）。
