# 移动端工具栏压缩为悬浮拉手 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 移动端 Reader/Timeline/Tasks 三页工具栏在滚动时压缩为悬浮拉手，点击展开，下滑即收。

**Architecture:** 方案 A — 状态集中在 appStore（三态：展开/折叠成拉手/钉住展开），拉手是 App.vue 单一全局元素，统一折叠 CSS 按 `mobile-scrolled && !toolbar-pinned` 条件驱动。纯状态逻辑抽到 `utils/toolbarCollapsePolicy.ts` 以匹配现有 node:test 单测模式。

**Tech Stack:** Vue 3 + Pinia + Element Plus + node:test + Playwright(playwright-core)

**Spec:** `docs/superpowers/specs/2026-09-02-mobile-collapsing-toolbar-design.md`

## Global Constraints

- 阈值常量：`THRESHOLD = 20`、`RECOLLAPSE_DELTA = 4`（来自 spec §4.1）。
- 现有滚动源保留各自 `scrollTop` 读取，仅替换 `setScrolled(...)` 调用为 `appStore.handleScroll(scrollTop)`。
- 不引入新前端依赖；Playwright 测试用现有 `/tmp/mobile-repro/` 工具与 chromium-core。
- 提交直接进 `main`（已确立工作流），不推送远端。
- CSS token 用项目现有：`--bg-glass`、`--border-glass`、`--text-muted`、`--duration-slow/fast`、`--ease-out/standard`；z-index 用 `1001`（与全局 header 同层）。
- Reader 的 `holdHeaderForJump` 门控包裹 `handleScroll` 调用，不变。
- 工作流：每任务 `cargo`/前端 gates → commit。前端 gates = `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`。

---

## File Structure

- Create: `frontend/src/utils/toolbarCollapsePolicy.ts` — 纯状态机函数（可测）
- Create: `frontend/tests/toolbarCollapsePolicy.test.ts` — node:test 单测
- Modify: `frontend/src/stores/app.ts` — 新增 refs + `handleScroll`/`togglePin`/`setImmersive`，废弃 `setScrolled`
- Modify: `frontend/src/App.vue` — 拉手元素+CSS、`toolbar-pinned` 类、`onMainScroll`→`handleScroll`、路由重置、重写 `:1009` 折叠规则
- Modify: `frontend/src/views/Reader.vue` — `processContentScroll`→`handleScroll`、沉浸/全屏 `setImmersive`、移除 `:1900` 局部规则、书架 `handleScroll(0)`
- Modify: `frontend/src/views/Tasks.vue` — `onPanelScroll`→`handleScroll`
- Create: `/tmp/mobile-repro/verify-collapsing-toolbar.mjs` — 三页端到端 + 桌面回归

---

### Task 1: 纯状态机 + appStore 接线 + 单测

**Files:**
- Create: `frontend/src/utils/toolbarCollapsePolicy.ts`
- Create: `frontend/tests/toolbarCollapsePolicy.test.ts`
- Modify: `frontend/src/stores/app.ts:13-39,56-68`

**Interfaces:**
- Produces: `computeScrollState(prev, scrollTop)` 纯函数，返回 `{ isScrolled, toolbarPinned, pinScrollTop }`；`applyPin(prev)` 返回新状态。

- [ ] **Step 1: 写失败测试**

`frontend/tests/toolbarCollapsePolicy.test.ts`:
```ts
import assert from 'node:assert/strict'
import test from 'node:test'
import { computeScrollState, applyPin } from '../src/utils/toolbarCollapsePolicy.ts'

const base = { isScrolled: false, toolbarPinned: false, pinScrollTop: 0 }

test('at top: handleScroll(0) → not scrolled, not pinned', () => {
  assert.deepEqual(computeScrollState(base, 0), { isScrolled: false, toolbarPinned: false, pinScrollTop: 0 })
})

test('scrolled down: handleScroll(100) → scrolled, not pinned', () => {
  assert.deepEqual(computeScrollState(base, 100), { isScrolled: true, toolbarPinned: false, pinScrollTop: 0 })
})

test('pin: sets pinned true, records pinScrollTop', () => {
  const scrolled = { isScrolled: true, toolbarPinned: false, pinScrollTop: 0 }
  assert.deepEqual(applyPin(scrolled, 100), { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 })
})

test('after pin, small jitter (<4px) stays pinned', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 103).toolbarPinned, true)
})

test('after pin, scroll down >4px re-collapses', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 105).toolbarPinned, false)
})

test('after pin, scroll back to top clears both', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  const r = computeScrollState(pinned, 0)
  assert.equal(r.isScrolled, false)
  assert.equal(r.toolbarPinned, false)
})

test('after pin, scroll up but still >threshold stays pinned', () => {
  const pinned = { isScrolled: true, toolbarPinned: true, pinScrollTop: 100 }
  assert.equal(computeScrollState(pinned, 60).toolbarPinned, true)
})

test('threshold boundary: 20 → not scrolled, 21 → scrolled', () => {
  assert.equal(computeScrollState(base, 20).isScrolled, false)
  assert.equal(computeScrollState(base, 21).isScrolled, true)
})
```

- [ ] **Step 2: 运行确认失败**

Run: `cd frontend && node --test --experimental-strip-types tests/toolbarCollapsePolicy.test.ts`
Expected: FAIL — 模块不存在

- [ ] **Step 3: 实现纯函数**

`frontend/src/utils/toolbarCollapsePolicy.ts`:
```ts
export interface ScrollState {
  isScrolled: boolean
  toolbarPinned: boolean
  pinScrollTop: number
}

export const SCROLL_THRESHOLD = 20
export const RECOLLAPSE_DELTA = 4

/** Pure transition for a scroll event. */
export function computeScrollState(prev: ScrollState, scrollTop: number): ScrollState {
  const isScrolled = scrollTop > SCROLL_THRESHOLD
  let toolbarPinned = prev.toolbarPinned
  let pinScrollTop = prev.pinScrollTop
  if (scrollTop <= SCROLL_THRESHOLD) {
    toolbarPinned = false            // back to top → natural full expand, pin cleared
  } else if (toolbarPinned && scrollTop > pinScrollTop + RECOLLAPSE_DELTA) {
    toolbarPinned = false            // continued scrolling down → re-collapse
  }
  return { isScrolled, toolbarPinned, pinScrollTop }
}

/** Pure transition for a grip click (expand). */
export function applyPin(prev: ScrollState, currentScrollTop: number): ScrollState {
  if (prev.toolbarPinned) {
    return { ...prev, toolbarPinned: false }   // safety branch; grip hidden when pinned
  }
  return { isScrolled: prev.isScrolled, toolbarPinned: true, pinScrollTop: currentScrollTop }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cd frontend && node --test --experimental-strip-types tests/toolbarCollapsePolicy.test.ts`
Expected: PASS 全部

- [ ] **Step 5: appStore 接线**

修改 `frontend/src/stores/app.ts`：
- 顶部 `import { ref, watch } from 'vue'` 不变；加 `import { computeScrollState, applyPin, type ScrollState } from '@/utils/toolbarCollapsePolicy'`
- 在 `isScrolled` ref 旁加：
```ts
  // Three-state mobile toolbar collapse: expanded / collapsed-to-grip / pinned-expanded.
  const scrollTop = ref(0)
  const toolbarPinned = ref(false)
  const pinScrollTop = ref(0)
  const immersiveHidden = ref(false) // Reader immersive/fullscreen hides the grip
```
- 替换 `setScrolled` 为：
```ts
  function handleScroll(st: number) {
    scrollTop.value = st
    const next = computeScrollState(
      { isScrolled: isScrolled.value, toolbarPinned: toolbarPinned.value, pinScrollTop: pinScrollTop.value },
      st,
    )
    isScrolled.value = next.isScrolled
    toolbarPinned.value = next.toolbarPinned
    pinScrollTop.value = next.pinScrollTop
  }
  function togglePin() {
    const next = applyPin(
      { isScrolled: isScrolled.value, toolbarPinned: toolbarPinned.value, pinScrollTop: pinScrollTop.value },
      scrollTop.value,
    )
    toolbarPinned.value = next.toolbarPinned
    pinScrollTop.value = next.pinScrollTop
  }
  function setImmersive(v: boolean) { immersiveHidden.value = v }
```
- return 块：移除 `setScrolled`，加 `scrollTop, toolbarPinned, pinScrollTop, immersiveHidden, handleScroll, togglePin, setImmersive`。

- [ ] **Step 6: gates + commit**

Run: `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`
Expected: PASS
```bash
git add frontend/src/utils/toolbarCollapsePolicy.ts frontend/tests/toolbarCollapsePolicy.test.ts frontend/src/stores/app.ts
git commit -m "feat(mobile): add toolbar collapse state machine to appStore"
```

---

### Task 2: App.vue 拉手元素 + 折叠 CSS 重写

**Files:**
- Modify: `frontend/src/App.vue` (template :21 区、:60-67、:92、:236-240、:1008-1020 CSS、新增拉手 CSS)

**Interfaces:**
- Consumes: `appStore.handleScroll`, `appStore.togglePin`, `appStore.toolbarPinned`, `appStore.immersiveHidden`

- [ ] **Step 1: 模板加拉手元素**

在 `App.vue` 的 `.mobile-global-header` div 之后（`:33` 后）插入：
```html
    <!-- Mobile collapsing toolbar grip: click to re-expand the page toolbar -->
    <button
      v-if="isMobile"
      class="mobile-toolbar-grip"
      :class="{ visible: isScrolled && !appStore.toolbarPinned && !appStore.immersiveHidden }"
      type="button"
      aria-label="展开工具栏"
      @click="appStore.togglePin()"
    >
      <span class="grip-line"></span>
      <span class="grip-line"></span>
    </button>
```

- [ ] **Step 2: `.app-main` 类绑定加 `toolbar-pinned`**

`:62-66` 改为：
```html
        :class="{
          'mobile-full': isMobile,
          'mobile-scrolled': isMobile && isScrolled,
          'toolbar-pinned': isMobile && appStore.toolbarPinned,
          'reader-scroll-locked': lockMobileReaderOuterScroll,
        }"
```

- [ ] **Step 3: onMainScroll → handleScroll + 路由重置**

`:237-240` 改为：
```ts
function onMainScroll(e: Event) {
  const el = e.target as HTMLElement
  appStore.handleScroll(el.scrollTop)
}
```
`:92` 改为：
```ts
watch(() => route.path, () => appStore.handleScroll(0))
```
（`const isScrolled = computed(() => appStore.isScrolled)` 保留，模板/header 仍用。）

- [ ] **Step 4: 重写折叠 CSS（替换 :1008-1020）**

把 `App.vue:1008-1020` 整块替换为：
```css
/* ── Mobile scroll: collapse page header + toolbar into grip (unless pinned) ── */
.app-main.mobile-scrolled:not(.toolbar-pinned) .page-header,
.app-main.mobile-scrolled:not(.toolbar-pinned) .reader-topbar,
.app-main.mobile-scrolled:not(.toolbar-pinned) .toolbar,
.app-main.mobile-scrolled:not(.toolbar-pinned) .task-toolbar {
  max-height: 0;
  min-height: 0;
  opacity: 0;
  margin: 0;
  padding: 0;
  border: 0;
  overflow: hidden;
  pointer-events: none;
}
.page-header,
.reader-topbar,
.toolbar,
.task-toolbar {
  transition: max-height var(--duration-slow) var(--ease-standard),
              opacity var(--duration-fast) var(--ease-out),
              margin var(--duration-slow) var(--ease-standard),
              padding var(--duration-slow) var(--ease-standard);
}
```

- [ ] **Step 5: 加拉手 CSS**

在折叠 CSS 之后插入：
```css
/* ── Mobile toolbar grip (thin floating handle) ── */
.mobile-toolbar-grip {
  position: fixed;
  top: calc(var(--mobile-header-height) + var(--safe-top) + 4px);
  left: 50%;
  transform: translateX(-50%) translateY(-6px);
  width: 44px;
  height: 14px;
  background: var(--bg-glass);
  border: 1px solid var(--border-glass);
  border-radius: 7px;
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.35);
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  gap: 2px;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out);
  z-index: 1001;
}
.mobile-toolbar-grip.visible {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(-50%) translateY(0);
}
.grip-line {
  width: 22px;
  height: 1.5px;
  background: var(--text-muted);
  border-radius: 1px;
}
```

- [ ] **Step 6: gates + commit**

Run: `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`
Expected: PASS
```bash
git add frontend/src/App.vue
git commit -m "feat(mobile): add toolbar grip + unified collapse CSS in App.vue"
```

---

### Task 3: Reader.vue 接线

**Files:**
- Modify: `frontend/src/views/Reader.vue` (`processContentScroll` ~:1115-1124、`:371` 书架、`:1888-1902` CSS、沉浸/全屏切换处)

**Interfaces:**
- Consumes: `appStore.handleScroll`, `appStore.setImmersive`

- [ ] **Step 1: processContentScroll → handleScroll**

`Reader.vue` `processContentScroll` 中（约 :1120-1124），把：
```ts
    appStore.setScrolled(contentRef.value.scrollTop > 20)
```
改为：
```ts
    appStore.handleScroll(contentRef.value.scrollTop)
```
（保留外层 `if (contentRef.value && performance.now() > headerHoldUntil)` 门控。）

- [ ] **Step 2: 书架视图 handleScroll(0)**

`:371-373` 把 `appStore.setScrolled(false)` 改为 `appStore.handleScroll(0)`。

- [ ] **Step 3: 沉浸/全屏 → setImmersive**

定位 Reader 的沉浸模式切换（搜索 `isMobileImmersive` / `mobileImmersive` 的 toggle 处）与全屏 `fullscreenchange` 监听处。在进入沉浸/全屏时调 `appStore.setImmersive(true)`，退出时 `appStore.setImmersive(false)`。
（执行时按实际函数名定位；若沉浸与全屏分别在不同 watcher，两处都加。）

- [ ] **Step 4: 移除局部 topbar 折叠规则**

`Reader.vue:1892-1902`（`.reader-topbar { max-height:120px; transition... }` 与 `.app-main.mobile-scrolled .reader-topbar {...}`）整块删除——折叠已由 App.vue 全局规则覆盖。保留 `.reader-topbar` 的基础样式（min-height/padding 等）不动。

- [ ] **Step 5: gates + commit**

Run: `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`
Expected: PASS
```bash
git add frontend/src/views/Reader.vue
git commit -m "feat(mobile): wire Reader scroll/immersive to handleScroll + setImmersive"
```

---

### Task 4: Tasks.vue 接线

**Files:**
- Modify: `frontend/src/views/Tasks.vue:712-717`

- [ ] **Step 1: onPanelScroll → handleScroll**

`Tasks.vue:712-717` 把：
```ts
function onPanelScroll(event: Event) {
  const el = event.target as HTMLElement
  appStore.setScrolled(el.scrollTop > 20)
}
```
改为：
```ts
function onPanelScroll(event: Event) {
  const el = event.target as HTMLElement
  appStore.handleScroll(el.scrollTop)
}
```

- [ ] **Step 2: gates + commit**

Run: `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`
Expected: PASS
```bash
git add frontend/src/views/Tasks.vue
git commit -m "feat(mobile): wire Tasks panel scroll to handleScroll"
```

---

### Task 5: 端到端验证（Playwright）

**Files:**
- Create: `/tmp/mobile-repro/verify-collapsing-toolbar.mjs`

- [ ] **Step 1: 写验证脚本**

`/tmp/mobile-repro/verify-collapsing-toolbar.mjs`：对 Reader/Timeline/Tasks 三页，390×844 触摸上下文，断言：
1. 滚动 >20px → `.mobile-toolbar-grip.visible` 存在；`.page-header` + 对应工具栏 `height < 2`。
2. 点 `.mobile-toolbar-grip` → grip 不再 visible；工具栏 `height > 30`。
3. 继续下滑 >4px → grip 重现 + 工具栏折叠。
4. 滑回 `scrollTop ≤ 20` → 工具栏完整、grip 隐藏。
5. 桌面 1280 视口：无 `.mobile-toolbar-grip` 元素，工具栏不折叠。
6. 路由切换：Reader 折叠后导航到 Tasks → Tasks 工具栏完整。
（模板参考 `/tmp/mobile-repro/verify-filetree-expand.mjs` 的 `chromium-core` + CHROME 路径与 check() 模式。）

- [ ] **Step 2: 跑验证**

Run: `node /tmp/mobile-repro/verify-collapsing-toolbar.mjs`
Expected: ALL PASS
若 Timeline 粘性 `.toolbar-row` 未被裁切（折叠时仍可见），在 App.vue 折叠规则加 `.toolbar-row { display: none }` 于折叠态，重跑。

- [ ] **Step 3: 截图归档**

`node /tmp/mobile-repro/verify-collapsing-toolbar.mjs` 已输出截图到 `/tmp/mobile-repro/`。

---

### Task 6: gates + commit + 安装

- [ ] **Step 1: 前端全 gates**

Run: `cd frontend && npx vue-tsc --noEmit && node --test --experimental-strip-types tests/*.test.ts`
Expected: PASS（含既有 76 单测无回归）

- [ ] **Step 2: 构建 + 安装**

Run: `cd /Users/tiercelchow/Documents/WorkSpace/MyProjects/ObsidianBrain && make install`
（`make install` = vite build + cargo build --release + 复制到 ~/.local/bin/obsidian-brain）

- [ ] **Step 3: 确认最终 commit 序列**

`git log --oneline -5` 确认 4 个 feat commit 已在 main。用户自行重启服务。
