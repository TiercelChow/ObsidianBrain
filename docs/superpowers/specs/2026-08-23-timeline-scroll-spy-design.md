# 时光机左轴滚动联动高亮（Scroll-Spy）设计

> 日期：2026-08-23 · 状态：已批准（用户口头批准方案与左轴自动跟随建议）

## 1. 背景与问题

`Timeline.vue` 左侧时间轴（`.time-nav`）的 `.active` 高亮目前**仅在点击日期时**设置（`scrollToDate` 同时设置 `selectedDate` 并平滑滚动右侧列表）：

- 初次进入页面无任何高亮，需手动点击；
- 用户滚动右侧小记列表后，左侧高亮停留在上一次点击的日期，与视口实际内容脱节。

用户需求（2026-08-23 原话）：「时光机左侧的时间轴应该要能随着右侧的小记内容对应的时间产生高亮效果」。

## 2. 目标

右侧 `.memo-scroll` 滚动时，左侧时间轴高亮自动指向**当前视口顶部对应的日期**（scroll-spy）；初次加载后自动高亮最新一天；高亮日移出左轴视口时左轴自动跟随滚动（用户已接受此建议）。

## 3. 非目标

- 不改变点击跳转行为（点击仍立即高亮 + 平滑滚动，滚动事件最终收敛到同一日期）。
- 不改变移动端布局：左轴在 ≤768px 隐藏，联动逻辑在移动端直接跳过。
- 不新增视觉样式：复用现有 `.active`（靛蓝胶囊背景 + 发光圆点）。

## 4. 设计

### 4.1 机制

1. `.memo-scroll` 添加 `ref`；滚动事件经 **rAF 节流**（每帧至多一次扫描）后执行检测。
2. 扫描 `groupedMemos` 各日期分组头（`id="date-YYYY-MM-DD"`）相对滚动容器顶部的偏移，取**最后一个 `top ≤ 90px`** 的日期为当前高亮。分组头在 DOM 中自上而下按时间降序排列，位置单调，遇到第一个 `top > 90` 即 break（只扫可见区 + 1）。
3. 无任何分组头过阈值（滚到最顶部、顶部被筛选提示条等占据）→ 回退到第一个（最新）日期。
4. 高亮变化时对左轴对应 `.day-link` 执行 `scrollIntoView({ block: 'nearest' })`——左轴短时无感知，月份多时自动跟随。
5. 阈值 `90px`：吸收顶部筛选提示条与呼吸留白，保证高亮日与视口首屏内容一致。

### 4.2 决策逻辑抽取（可测性）

纯决策逻辑下沉到 `frontend/src/utils/timelineSpy.ts`（沿袭 `taskActivity.ts` / `taskDates.ts` 的下沉模式，node --test 可测）：

```ts
export interface SpyHeader { date: string; top: number }
export function pickActiveDate(headers: SpyHeader[], threshold: number): string | null
```

- 返回最后一个 `top ≤ threshold` 的 `date`；
- 全部未过阈值时回退 `headers[0].date`；
- `headers` 为空返回 `null`。

DOM 测量（`getBoundingClientRect`、rAF、`scrollIntoView`）留在组件内。

### 4.3 数据变化重同步

`watch(groupedMemos)` → `nextTick` 后重跑扫描，覆盖：加载完成、加载更多追加、筛选/搜索切换、新建小记前插。当前 `selectedDate` 不在新列表时按扫描结果重取；列表为空时置空。

### 4.4 与点击的协同

点击 `day-link` 立即设置 `selectedDate`（即时反馈），随后 smooth 滚动产生的滚动事件让高亮沿途经日期逐步收敛到目标日期——联动自然，无冲突。

### 4.5 实现落点

| 文件 | 变更 |
|---|---|
| `frontend/src/utils/timelineSpy.ts` | 新建：`pickActiveDate` 纯函数 |
| `frontend/tests/timelineSpy.test.ts` | 新建：常规 / 全未过阈值 / 空列表 用例 |
| `frontend/src/views/Timeline.vue` | `memo-scroll` 加 ref；`day-link` 加 `data-date`；滚动钩子 + rAF 节流 + 重同步 watch |
| `docs/requirement/04-timeline.md` | §2.1.2 布局说明 + §5.3 UI 验收项 |
| `docs/development/04-timeline.md` | §6.2 组件职责补充 scroll-spy 描述 |

## 5. 验收标准

- [ ] 滚动右侧列表，左侧高亮跟随视口顶部日期变化
- [ ] 初次加载完成后最新一天自动高亮
- [ ] 高亮日移出左轴视口时，左轴自动滚动跟随
- [ ] 点击日期跳转行为与现状一致
- [ ] `pickActiveDate` 单元测试覆盖：常规取最后一个过阈值 / 全未过阈值回退首个 / 空列表返回 null
- [ ] 移动端（≤768px）无回归（左轴隐藏，联动跳过）
- [ ] 前端门禁通过：`vue-tsc` 零错误、`npm test` 全过
