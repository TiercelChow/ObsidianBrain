# 任务进展归因 + 子任务详情抽屉 — 设计文档

- 日期：2026-08-22
- 状态：已批准（用户确认交互模型与抽屉内容）
- 影响模块：Tasks 前端（`frontend/src/views/Tasks.vue`、新组件、新 util）；**后端零改动**
- 分支：`feature/tasks-subtask-drawer`（基于 `refactor/tasks-decouple-obsidian`）

## 1. 背景与目标

任务详情页的两个体验缺口：

1. **进展与记录无归因**：详情面板的活动流严格过滤为「当前选中节点」自己的条目，标题是通用文案（"记录了新进展"、"状态变为进行中"），不带任务名。查看根任务时，子任务上新增的进展/状态变更**完全不可见**。
2. **子任务点击体验差**：点击子任务会把整个右侧详情面板切换成该子任务，根任务的总览、拆解、进展记录全部消失，上下文丢失。

**目标**：主面板固定为根任务总览视图，活动流聚合全树条目并逐条标注所属任务；点击子任务改为在右侧滑出抽屉展示其详情，主面板内容被压缩让位。

## 2. 决策记录

| 决策点 | 结论 |
|---|---|
| 主面板活动流范围 | **全树聚合**：根任务 + 所有子任务的全部 progress/audit 条目，逐条带任务名标注 |
| 抽屉内容 | **详情 + 专属进展流**：标题/状态/描述/起止日期/优先级 + 操作按钮 + 该子任务自己的进展与记录 |
| 桌面端布局 | 抽屉 push 压缩主面板（三栏效果），非遮罩浮层 |
| 原面板切换行为 | **移除**——点击子任务不再切换详情面板，改为驱动抽屉 |

## 3. 交互模型

```
桌面端 ≥1150px（抽屉打开）：
┌─────────┬──────────────────┬─────────────┐
│ 任务列表 │  根任务详情(被压缩) │ 子任务抽屉    │
│ 270-340 │  总览/拆解/进展记录 │ 详情+专属进展流│
└─────────┴──────────────────┴─────────────┘
```

- 主详情面板**始终渲染根任务**：头部、事实行、进度总览（long）、任务拆解树、全树聚合活动流。
- 点击子任务行 → 打开抽屉；抽屉已开时点击另一子任务 → 抽屉内容直接切换（单抽屉，不叠层）。
- 关闭抽屉的途径：点根任务行、抽屉 ✕ 按钮、Esc 键（仅抽屉打开时监听）、窄屏/移动端点遮罩。
- 左侧列表切换到其他任务（`selectedDetail` 变化）→ 抽屉自动关闭。
- 任务拆解树的选中高亮：`drawerNodeId ?? 根任务 id`（抽屉关闭时高亮根行，打开时高亮抽屉节点）。

## 4. 活动流聚合 — `utils/taskActivity.ts`

纯函数模块，沿用 `taskDates`/`taskHierarchy` 的「utils + node --test」模式：

```ts
import type { AuditEvent, ProgressEntry, TaskNode, TaskStatus } from '@/api/tasks'

export interface TaskActivityEntry {
  id: string                 // 'progress:{uuid}' / 'audit:{uuid}'
  type: 'progress' | 'audit'
  taskId: string
  taskTitle: string          // 所属任务名，id→title 映射；未知 id 回退 '未知任务'
  title: string              // 动作文案
  note: string | null
  time: string               // progress.recorded_at / audit.occurred_at
}

export function taskStatusLabel(status: TaskStatus): string
export function buildTaskActivity(
  nodes: TaskNode[],
  progress: ProgressEntry[],
  audit: AuditEvent[],
  scopeTaskId?: string,      // 省略 = 全树；传 id = 仅该任务（抽屉用）
): TaskActivityEntry[]
```

- 文案沿用现状：progress `percent_after == null ? '记录了新进展' : '进展更新为 X%'`；audit 沿用现有映射（status_changed → `状态变为{label}`，created/updated/moved/archived/reopened → 既有文案，兜底 `任务发生变化`）。
- 排序：时间倒序（`b.time.localeCompare(a.time)`，与现状一致）。
- `taskStatusLabel` 从 Tasks.vue 的本地函数移入本模块并导出；Tasks.vue 改为导入（顺带消除与 TaskTree.vue 的重复，TaskTree.vue 本次不动）。
- 主面板条目渲染：`任务名（可点击高亮标签）· 动作文案`；点击任务名 → 该任务是子任务则打开其抽屉，是根任务则关闭抽屉回总览。任务名过长省略号截断。

## 5. 子任务抽屉 — `components/tasks/SubtaskDrawer.vue`

```ts
props: {
  node: TaskNode | null           // null = 关闭态；组件常驻挂载（非 v-if），以支持过渡动画
  activity: TaskActivityEntry[]   // 已过滤到该节点的专属进展流（node 为 null 时为空数组）
}
emits: {
  close: []
  progress: [node: TaskNode]
  add: [node: TaskNode]           // 添加子任务
  status: [node: TaskNode]
  edit: [node: TaskNode]
  move: [node: TaskNode]
}
expose: { revealActivity(): void }   // 供父级在操作成功后滚动到进展流
```

结构（自上而下）：

1. 头部：✕ 关闭按钮、「子任务 · {状态}」眉标、标题、描述（空则不渲染该块）
2. 事实行：开始 / 结束 / 优先级（沿用 detail-facts 样式）
3. 操作按钮：记录进展、添加子任务、更改状态、编辑、移动 —— emit 给父级，**全部复用现有 sheet 系统**（`openProgress`/`openSubtask`/`openStatus`/`openEdit`/`openMove`，`targetNode` 指向抽屉节点）
4. 「进展与记录」列表：仅该任务条目，样式沿用 activity-list；空态显示「暂无进展记录」文案

无障碍：`role="dialog"` + `aria-label="子任务详情"`；打开时聚焦关闭按钮；Esc 关闭。

## 6. 布局与响应式

现状：`.task-workspace` 为两栏 grid（`minmax(270px,340px) minmax(0,1fr)`）。

新结构：第二栏改为 `.task-detail-zone`（flex 容器）包住详情面板与抽屉：

```
.task-workspace          grid: 列表 | task-detail-zone
.task-detail-zone        flex: 详情面板(flex:1, min-width:0) + 抽屉
```

| 断点 | 抽屉形态 |
|---|---|
| ≥1150px | **in-flow push**：抽屉为 flex 定宽子项（`min(400px, 30vw)`），打开时详情面板被压缩；约 280ms 过渡动画，`prefers-reduced-motion` 下直接切换 |
| <1150px | **fixed 浮层**：右侧固定定位（`inset-block: 0; right: 0; width: min(400px, 92vw)`），transform 滑入，带半透明遮罩，点遮罩关闭 |
| ≤768px | 同上浮层形态，宽度 `min(88vw, 400px)`，全屏高 |

- push 过渡实现：抽屉常驻挂载，关闭态宽度 0 + `overflow: hidden` + 不可聚焦；内容定宽避免动画期间被压扁。
- z-index：浮层形态高于面板、低于 `.task-sheet`（sheet 弹层优先于抽屉）。
- 现有 1050px/768px 断点行为不变；1150px 断点仅决定 push vs 浮层。

## 7. 数据流与状态

- **零后端改动**：progress/audit 条目已带 `task_id`，任务名从 `detail.tasks` 建 id→title 映射。
- Tasks.vue 新增状态：`drawerNodeId: Ref<string | null>`；`drawerNode`/`drawerActivity` 均为基于已加载 detail 的 computed，无额外请求。
- 删除：`activeNodeId`、`activeNode`、`selectNode` 的面板切换与滚动逻辑。
- TaskTree `@select` 新语义：根任务 → 关抽屉；子任务 → 设 `drawerNodeId`。树行内联操作按钮（进展/＋/状态/移动）不变。
- 操作成功后的滚动揭示：抽屉打开时滚动抽屉内进展流（`revealActivity`），否则沿用现有 `revealDetailEl` 行为。抽屉在操作后保持打开，数据经现有 detail 重载自动刷新。
- 版本冲突自动重载等既有机制不受影响。

## 8. 测试

- 新增 `frontend/tests/taskActivity.test.ts`（node --test）：
  - 全树聚合（root + 多个子任务的 progress/audit 全部出现）
  - taskTitle 归因正确（id→title 映射；未知 task_id 回退 '未知任务'）
  - `scopeTaskId` 过滤
  - 时间倒序排序
  - 文案：percent_after 空/非空、audit 各 event_type、status_changed 状态名
- 组件与布局由 `vue-tsc` 类型检查覆盖 + 用户手动验收（含移动端）。
- 门禁不变：`npx vue-tsc -b && npm test`。

## 9. 文档更新

- `docs/requirement/09-task-management.md`：进展与记录的归因要求、子任务抽屉交互及验收标准。
- `docs/development/09-task-management.md`：前端交互模型（主面板固定根任务、抽屉状态机、taskActivity util、1150px 断点）。

## 10. 明确不做（YAGNI）

- 不做抽屉内嵌套二级抽屉（主面板的树可点任意层级，抽屉内容直接切换）。
- 抽屉状态不做路由/query 持久化（刷新即关）。
- 抽屉不做子任务进度百分比总览（专属进展流中的「进展更新为 X%」已覆盖）。
- 不改动 TaskTree.vue 的行内操作与缩进逻辑（仅消费其 select 语义变化）。
- 后端、API、数据模型零改动。

## 11. 分支与部署

- 分支 `feature/tasks-subtask-drawer` 叠在 `refactor/tasks-decouple-obsidian` 之上；合并顺序：先合 refactor，再合本分支。
- 纯前端改动，部署路径与 decoupling 相同：`npx vite build --outDir dist_new` → `cargo build --release` → sudo 安装重启。可与 decoupling 的安装合并为一次操作（同一枚举二进制）。
- 开发期验证：vite dev server（代理 :9876）交互验收 + 门禁。

## 12. 调整波（2026-08-23）：体验细化

用户验收后提出的 8 项调整；本节优先于前文与之冲突的描述。

### 12.1 抽屉结构（修订 §5）

- 新增「子任务」栏目：列出当前任务的直接子任务（按 `position` 排序，含状态圆点与标题，点击行内箭头下钻）；点击即在抽屉内切换到该子任务。抽屉头部显示「‹ {父任务名}」返回行——父任务为根任务时点击收起抽屉回总览（沿用 `focusTask` 语义）。仍不做二级抽屉（§10.1 维持）。
- 操作按钮重组：删除五按钮堆叠。编辑/更改状态/移动 → 抽屉右上角三个紧凑图标按钮（EditPen/Refresh/Rank，带 `title` 与 `aria-label`）；记录进展/添加子任务 → 各自栏目标题行的「＋」按钮。
- 收起交互：删除 ✕ 关闭按钮。抽屉左缘中部一个常驻「‹」收起符号（竖长圆角把手：桌面端置于抽屉与主面板之间的 26px 把手槽内，窄屏浮层形态下悬浮于遮罩之上）。Esc 与点遮罩关闭保留。

### 12.2 活动流条目（修订 §4）

- 条目改为「类型胶囊 + 明细」结构：标题行只显示类型胶囊与时间——类型标签为名词式短语：`进展` / `状态变更` / `重新打开` / `归档` / `取消归档` / `级联完成` / `移动` / `创建` / `更新`，未知 event_type 兜底 `变更`。替换原「记录了新进展」「状态变为X」类口语化文案。
- 下方明细行 = 任务名（可点击）+ 具体变化：progress 条目带 `percent_after` 时为「完成度 X%」；status 族条目为「{from} → {to}」（缺 from 时只显示 to）。备注独占一行。抽屉内专属流省略任务名。
- `TaskActivityEntry` 增加 `detail: string | null` 字段；`title` 语义改为类型标签。
- 间隔线：分隔线两侧各留约 14px 呼吸空间（后续条目 `border-top` + 条目上下 padding 实现）。

### 12.3 属性胶囊（新增）

任务状态、重要程度等关键属性用软色胶囊（`.task-pill` 及 `status-*`/`importance-*`/`type-*` 修饰类，定义于 App.vue 全局样式块）：状态沿用既有状态色系（accent/橙/绿/灰），重要程度 low/normal/high/urgent 为灰/中性/橙/红；活动流类型标签同样胶囊化（progress=accent 色、audit=中性）。应用于主面板眉标、抽屉眉标、活动流类型标签。

### 12.4 留白与页面锁定（修订 §6）

- 留白：详情面板下半部分（事实行、拆解、进展记录）节奏放宽——facts 上下边距 26px、区块 `margin-top` 18px、区块 padding 18px、面板底部 padding 34px，对齐阅境轩（Reader.vue）的疏朗节奏。
- 页面锁定：任务视图下整页不滚动——`.tasks-page.view-tasks` 锁定为 `calc(100dvh - 64px)`（桌面 = app-main 上下 padding 之和）的 flex 列，workspace 为 `flex: 1 1 auto; min-height: 0`；只有左侧列表、详情面板、抽屉内部各自滚动。移动端锁定高度 `calc(100dvh - 96px - var(--safe-top) - var(--safe-bottom))`，列表/详情二选一占满剩余高度。日历视图不受影响（锁定类仅在任务视图分支挂载）。
- 抽屉 push 布局微调：slot 打开总宽 = `min(400px, 30vw) + 24px`（`padding-left: 24px` 把手槽），`margin-left` 移除；<1150px 浮层形态不变（把手绝对定位 `left: -24px` 悬于遮罩上）。

### 12.5 组件契约变更（修订 §5 props/emits）

`SubtaskDrawer` props 增加 `children: TaskNode[]`（当前任务的直接子任务，按 `position` 排序）与 `parent: TaskNode | null`（父节点，含根任务；找不到时为 null 且不渲染返回行）；emits 增加 `select: [taskId: string]`（点击子任务行或返回行，父级绑定为 `focusTask`）。✕ 关闭按钮与其 focus 逻辑移除，抽屉打开时的键盘焦点落点改为收起把手。

## 13. 第二轮调整（2026-08-23）：信息层级精简

用户第二轮验收反馈（4 项）；本节优先于 §12 及前文与之冲突的描述。

### 13.1 属性胶囊位置（修订 §12.3 应用位置；2026-08-23 用户修正）

- 删除主面板与抽屉头部的 kicker 胶囊行，标题上移至头部最上方。
- 状态 + 重要性胶囊置于标题之后、描述之前，独立一行。
- 「优先级」事实卡删除（与重要性胶囊重复）；事实行仅剩「开始/结束」两张等宽卡片。

### 13.2 子任务行重要性胶囊（修订 §12.1）

- 抽屉「子任务」栏目行在任务名后追加重要性胶囊（`.task-pill importance-*`）：状态圆点 + 标题 + 重要性胶囊 + ›。
- 主面板「任务拆解」树行不变。

### 13.3 活动条目重排与详情弹窗（修订 §12.2）

- 主面板聚合流条目头行 = 类型胶囊 + 任务名 + 时间（任务名紧跟类型胶囊，时间仍右对齐）；明细行与备注行各自单行省略号截断。
- 抽屉专属流条目结构相同，但省略任务名（整流均属抽屉当前任务）。
- 点击任意条目打开 MotionModal 详情弹窗：类型胶囊 + 任务名 + 时间 + 明细（完成度/状态流转）+ 备注全文多行展示；抽屉流的弹窗同样含任务名。
- 列表内任务名点击聚焦行为保留（focusTask）。

### 13.4 抽屉精简（修订 §12.1/§12.5）

- 抽屉右上角仅保留「编辑」（EditPen）图标按钮；更改状态/移动图标与「子任务」「进展与记录」栏目 ＋ 按钮删除；组件 emits 收敛为 close/select/edit。
- 收起把手为贴抽屉左缘内侧、垂直居中的短竖条（4px × 40px）：悬浮变为「›」形折线（chevron），移动端常显折线；点击收起。push 槽总宽回到 `min(400px, 30vw)`，无把手槽；Esc/遮罩关闭与「‹ {父任务名}」返回行保留。
- 编辑表单整合：编辑模式新增「状态」下拉（沿用既有状态过滤规则；终态时显示关闭说明，根任务另显示级联完成勾选）与「父任务」下拉（仅子任务显示，候选沿用移动候选逻辑）；保存时依次执行 更新字段 → 改状态（仅变化时）→ 移动（仅变化时），各写操作版本号由 store 写后刷新机制自动衔接。主面板「更改状态」按钮保留为快捷入口。
