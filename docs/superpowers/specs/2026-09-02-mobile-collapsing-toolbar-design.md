# 移动端工具栏压缩为悬浮拉手设计

> 状态：设计已与用户确认，待审查 → 实施计划
> 日期：2026-09-02
> 涉及页面：阅境轩 (Reader)、时光机 (Timeline)、任务中枢 (Tasks)

---

## 1. 目标

移动端三个页面（阅境轩 / 时光机 / 任务中枢）顶部工具栏的统一压缩交互：

1. 页面上滑时，工具栏（及页面标题行）被平滑压缩；
2. 压缩到尽头后，顶部只剩一个细长悬浮「拉手」（两条横线）；
3. 点击拉手 → 工具栏重新展开下来，方便在划到深处时仍能使用工具栏功能；
4. 展开后继续下滑 → 重新收成拉手；滑回顶部 → 恢复完整展开的常态。

把三个页面当前不一致的工具栏行为（Reader 折叠、Timeline 粘性、Tasks 仅换行）统一到同一套机制。

---

## 2. 设计决策（已与用户确认）

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 悬浮条样式 | 极简拉手（两条横线） | 最小视觉占位；点击展开 |
| 压缩范围 | 全收（标题行 + 工具栏 → 拉手） | 标题已由全局 `.mobile-global-header` 显示，收起不丢上下文；内容区最大化 |
| 重新收起行为 | 下滑即收 | 展开是「用一下」，继续下滑即重新收成拉手；滑回顶部恢复完整常态。最贴近「滑动压缩」初衷 |
| 架构 | 全局拉手置于 App.vue，状态集中在 appStore | 单一拉手元素、统一行为、复用现有 `mobile-scrolled` 类管道；顺手统一三页不一致的工具栏 |

不做：悬浮条里放快捷按钮（每页高频动作不同，增加复杂度；YAGNI）。

---

## 3. 现状接线（起点）

单一布尔 `appStore.isScrolled` 是唯一桥梁。三个滚动源以同一 `scrollTop > 20` 阈值喂数据：

| 页面 | 滚动元素 | 喂数据处 | 工具栏元素 | 滚动时折叠？ |
|------|----------|----------|-----------|--------------|
| Reader | `.pane.pane-center`（自有） | `Reader.vue:1123` `processContentScroll` | `.reader-topbar`（`Reader.vue:21`）+ `.page-header`（`:12`） | 是 — `.reader-topbar` 局部规则（`:1900`）+ 全局 `.page-header`（`App.vue:1009`） |
| Tasks | `.task-list-panel`/`.task-detail-panel`（自有） | `Tasks.vue:716` `onPanelScroll` | `.task-toolbar`（`Tasks.vue:16`）+ `.page-header`（`:3`） | 否 — `.task-toolbar` 仅换行（`:1067`），仅 `.page-header` 被全局折叠 |
| Timeline | 移动端 `.app-main`（内层 scroller 关闭，`:2119`） | `App.vue:239` `onMainScroll` | `.toolbar`（`Timeline.vue:22`）+ `.page-header`（`:4`） | 否 — `.toolbar-row` 粘性（`:2131`），不折叠；仅 `.page-header` 被全局折叠 |

`isScrolled` 切换 `.app-main.mobile-scrolled` 类（`App.vue:60`），触发：
- 全局 `.app-main.mobile-scrolled .page-header { max-height:0 !important ... }`（`App.vue:1009`，所有页面）
- Reader 局部 `.app-main.mobile-scrolled .reader-topbar { max-height:0 ... }`（`Reader.vue:1900`）
- `.mobile-global-header.header-scrolled`（玻璃背景，`App.vue:21`）

路由切换重置：`watch(route.path, () => appStore.setScrolled(false))`（`App.vue:92`）。
Reader 程序化跳转跳过：`holdHeaderForJump` 设 `headerHoldUntil = now+1600`（`Reader.vue:1106`），`processContentScroll` 在此窗口内不喂数据。
Reader 书架视图显式 `setScrolled(false)`（`Reader.vue:371`），因书架自有滚动容器不驱动 `setScrolled`。

---

## 4. 架构（方案 A：全局拉手 + 集中状态）

状态集中在 `appStore`，滚动状态机把现有 `isScrolled` 扩展为三态（展开 / 折叠成拉手 / 钉住展开）。拉手是 App.vue 里的单一全局元素，对三个页面通用。每页工具栏的折叠由统一 CSS 规则按 `mobile-scrolled && !toolbar-pinned` 条件驱动。

### 4.1 appStore 状态机

新增到 `frontend/src/stores/app.ts`（在现有 `isScrolled` 基础上）：

```ts
const THRESHOLD = 20              // px；scrollTop 超过即视为「已滚动」
const RECOLLAPSE_DELTA = 4        // px；钉住后继续下滑超过此量即重新收起

const scrollTop = ref(0)
const isScrolled = ref(false)     // scrollTop > THRESHOLD；驱动全局 header 玻璃
const toolbarPinned = ref(false)  // 用户点拉手在已滚动时重新展开
const pinScrollTop = ref(0)      // 钉住时刻的 scrollTop
const immersiveHidden = ref(false) // Reader 沉浸/全屏时隐藏拉手
```

派生（computed）：
```ts
const collapsed = computed(() => isScrolled.value && !toolbarPinned.value) // 显示拉手
```

### 4.2 滚动处理 `handleScroll`

替换三个 `setScrolled(scrollTop > 20)` 调用点；每处改为 `appStore.handleScroll(el.scrollTop)`：

```ts
function handleScroll(st: number) {
  scrollTop.value = st
  isScrolled.value = st > THRESHOLD
  if (st <= THRESHOLD) {
    toolbarPinned.value = false          // 回到顶部 → 自然完整展开，清除钉住
  } else if (toolbarPinned.value && st > pinScrollTop.value + RECOLLAPSE_DELTA) {
    toolbarPinned.value = false          // 继续下滑 → 重新收成拉手
  }
  // （钉住时向上滑、或小于 delta 的抖动 → 保持展开）
}
```

### 4.3 拉手点击 `togglePin`

```ts
function togglePin() {
  if (toolbarPinned.value) {
    toolbarPinned.value = false          // 安全分支；拉手在钉住时不可见，正常不会走到
  } else {
    toolbarPinned.value = true
    pinScrollTop.value = scrollTop.value // 记录钉住点，作为「继续下滑」的比较基准
  }
}

function setImmersive(v: boolean) { immersiveHidden.value = v }
```

### 4.4 调用点改动（3 处，各一行）

- `App.vue` `onMainScroll` → `appStore.handleScroll(el.scrollTop)`
- `Reader.vue` `processContentScroll` → 在 `performance.now() > headerHoldUntil` 门控内调用 `appStore.handleScroll(contentRef.value.scrollTop)`
- `Tasks.vue` `onPanelScroll` → `appStore.handleScroll(el.scrollTop)`

Timeline 无 JS 改动（沿用 App.vue 的 `onMainScroll`）。

### 4.5 路由切换重置

`App.vue:92` 现有 `watch(route.path, () => appStore.setScrolled(false))` → 改为重置全部：
```ts
watch(() => route.path, () => appStore.handleScroll(0))
```
`handleScroll(0)` 把 `isScrolled`、`toolbarPinned` 都置 false，进入页面即完整展开。

废弃 `setScrolled`：将其单行逻辑并入 `handleScroll`，移除导出（三处调用点已迁移）。

---

## 5. 拉手元素（App.vue）

模板，与 `.mobile-global-header` 同级：
```html
<button
  v-if="isMobile"
  class="mobile-toolbar-grip"
  :class="{ visible: isScrolled && !toolbarPinned && !immersiveHidden }"
  @click="togglePin"
  aria-label="展开工具栏"
>
  <span class="grip-line"></span>
  <span class="grip-line"></span>
</button>
```

CSS（fixed，全局 header 下方居中，视觉极细但触控目标足够）：
```css
.mobile-toolbar-grip {
  position: fixed;
  top: calc(var(--mobile-header-height) + var(--safe-top) + 4px);
  left: 50%;
  transform: translateX(-50%) translateY(-6px);
  width: 44px; height: 14px;
  background: var(--bg-glass);
  border: 1px solid var(--border-glass-subtle);
  border-radius: 7px;
  box-shadow: 0 1px 6px rgba(0,0,0,.35);
  display: flex; flex-direction: column;
  justify-content: center; align-items: center; gap: 2px;
  opacity: 0; pointer-events: none;
  transition: opacity var(--duration-fast) var(--ease-out),
              transform var(--duration-fast) var(--ease-out);
  z-index: var(--z-header);
}
.mobile-toolbar-grip.visible {
  opacity: 1; pointer-events: auto;
  transform: translateX(-50%) translateY(0);
}
.grip-line {
  width: 22px; height: 1.5px;
  background: var(--text-muted); border-radius: 1px;
}
```

---

## 6. 折叠 CSS（统一规则）

`App.vue` `.app-main` 类绑定增加一项（`:60`）：
```html
:class="{
  'mobile-full': isMobile,
  'mobile-scrolled': isMobile && isScrolled,
  'toolbar-pinned': isMobile && toolbarPinned,
  'reader-scroll-locked': lockMobileReaderOuterScroll,
}"
```

替换 `App.vue:1009` 现有「仅 `.page-header` + `!important`」规则为统一条件规则（条件 = 已滚动且未钉住）：
```css
.app-main.mobile-scrolled:not(.toolbar-pinned) .page-header,
.app-main.mobile-scrolled:not(.toolbar-pinned) .reader-topbar,
.app-main.mobile-scrolled:not(.toolbar-pinned) .toolbar,
.app-main.mobile-scrolled:not(.toolbar-pinned) .task-toolbar {
  max-height: 0; min-height: 0; opacity: 0;
  margin: 0; padding: 0; border: 0;
  overflow: hidden; pointer-events: none;
}
.page-header, .reader-topbar, .toolbar, .task-toolbar {
  transition: max-height var(--duration-slow) var(--ease-standard),
              opacity var(--duration-normal) var(--ease-out),
              margin var(--duration-slow) var(--ease-standard),
              padding var(--duration-slow) var(--ease-standard);
}
```

`toolbar-pinned` 类使 `:not(.toolbar-pinned)` 失配 → 选择器不命中 → 工具栏 + 标题行保持可见（展开）。移除 `!important`，改用条件选择器，更干净。

移除 `Reader.vue:1900` 的局部 `.reader-topbar` 折叠规则（已由全局规则覆盖）。

---

## 7. 边界情况

1. **Reader 沉浸模式 / 全屏**：`is-mobile-immersive` 与 `:fullscreen` 已 `display:none` 标题行 + topbar（`Reader.vue:1888`、`:1278`）。拉手需同步隐藏：Reader 在切换沉浸/全屏时调用 `appStore.setImmersive(bool)`，拉手 `visible` 计算含 `&& !immersiveHidden`。
2. **Reader 书架视图**：书架自有滚动容器不驱动 `handleScroll`，切到书架显式 `handleScroll(0)`（原 `setScrolled(false)`，`Reader.vue:371`）→ 拉手隐藏、工具栏完整。行为不变。
3. **Timeline 粘性 `.toolbar-row`**（`:2131`）：折叠规则使 `.toolbar` 父级 `max-height:0; overflow:hidden`，粘性子元素被裁切消失，拉手接替；钉住展开时 `.toolbar` 恢复，粘性行随之恢复。需在测试工具中验证裁切生效。
4. **Tasks 换行 `.task-toolbar`**（`:1067`）：换行规则保留，折叠规则叠加；折叠时高度为 0 换行无意义，钉住/展开时照常换行。
5. **Reader 程序化滚动跳转**（`holdHeaderForJump`，`:1106`）：现有 `performance.now() > headerHoldUntil` 门控保留，包裹 `handleScroll` 调用，跳转恢复不抖动拉手。
6. **桌面端**：所有逻辑 `isMobile` 门控，桌面布局不变。
7. **拉手与内容流**：拉手 `position:fixed`，不占文档流，不挤压内容。

---

## 8. 涉及文件

| 文件 | 改动 |
|------|------|
| `frontend/src/stores/app.ts` | 加 `scrollTop/toolbarPinned/pinScrollTop/immersiveHidden` ref；加 `handleScroll`/`togglePin`/`setImmersive`；废弃 `setScrolled` |
| `frontend/src/App.vue` | 拉手元素 + CSS；`toolbar-pinned` 类；`onMainScroll` → `handleScroll`；路由重置 → `handleScroll(0)`；重写 `:1009` 折叠规则 |
| `frontend/src/views/Reader.vue` | `processContentScroll` → `handleScroll`；沉浸/全屏切换处 `setImmersive`；移除 `:1900` 局部 topbar 折叠规则；书架视图 `handleScroll(0)` |
| `frontend/src/views/Tasks.vue` | `onPanelScroll` → `handleScroll` |
| `frontend/src/views/Timeline.vue` | 无 JS 改动；折叠规则为全局，无需局部规则；验证粘性裁切 |

---

## 9. 测试策略

### 9.1 单元测试（stores/app.ts）

`frontend/src/stores/__tests__/app.spec.ts`（或现有测试文件位置），覆盖 `handleScroll` / `togglePin` 状态机：

- `handleScroll(0)` → `isScrolled=false, toolbarPinned=false`（顶部完整展开）
- `handleScroll(100)` → `isScrolled=true, toolbarPinned=false`（折叠成拉手）
- `handleScroll(100)` 后 `togglePin()` → `toolbarPinned=true, pinScrollTop=100`（展开）
- 钉住后 `handleScroll(100+2)`（< delta）→ 仍钉住（抖动不收）
- 钉住后 `handleScroll(100+5)`（> delta）→ `toolbarPinned=false`（下滑即收）
- 钉住后 `handleScroll(0)`（滑回顶）→ `isScrolled=false, toolbarPinned=false`
- `setImmersive(true)` 后 `collapsed` 仍可为 true，但拉手 visible 计算为 false

### 9.2 Playwright 端到端（/tmp/mobile-repro/，390×844 触摸上下文）

对三个页面各跑一遍：
- **折叠**：滚动超过 20px → `.mobile-toolbar-grip.visible` 出现，`.page-header` 与工具栏 `getBoundingClientRect().height ≈ 0`。
- **展开**：点 `.mobile-toolbar-grip` → 拉手 `visible` 消失，工具栏 `height` 恢复正常。
- **下滑即收**：展开后继续下滑 >4px → 拉手重现、工具栏折叠。
- **回顶恢复**：滑回 `scrollTop ≤ 20` → 工具栏完整、拉手隐藏。
- **Reader 沉浸/全屏**：进入沉浸 → 拉手不显示（即便 `isScrolled`）。
- **Timeline 粘性裁切**：折叠时 `.toolbar-row` 不可见（被父级 `overflow:hidden` 裁切）。
- **桌面回归**：宽 1280 视口 → 无 `.mobile-toolbar-grip` 元素，工具栏不折叠。
- **路由切换**：Reader 折叠后导航到 Tasks → Tasks 工具栏完整展开（重置生效）。

---

## 10. 非目标

- 不改桌面端布局。
- 不在拉手里放快捷按钮。
- 不做滚动比例驱动的连续高度插值（用 CSS 过渡动画给出「逐渐」感即可；按比例插值会裁切工具栏内容且抖动）。
- 不改三页工具栏的内部结构与功能。
