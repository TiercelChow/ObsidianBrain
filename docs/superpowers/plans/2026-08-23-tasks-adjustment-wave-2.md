# 任务管理第二轮调整 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落实用户第二轮 4 项 UI 调整：属性胶囊下沉到事实行、子任务行重要性胶囊、活动条目重排 + 详情弹窗、抽屉精简（仅编辑入口 + 竖条把手 + 编辑表单整合状态/父任务）。

**Architecture:** 纯前端改动（Tasks.vue、SubtaskDrawer.vue；utils 与后端零改动）。规格权威来源为 docs/superpowers/specs/2026-08-22-tasks-activity-drawer-design.md §13。

**Tech Stack:** Vue 3 `<script setup>` + TS、scoped CSS、Element Plus（el-select/el-icon/MotionModal）、node --test。

## Global Constraints

- 后端零改动；`TaskTree.vue` 不改动；不新增前端依赖。
- 分支 `feature/tasks-subtask-drawer`；Conventional Commits；每个任务完成后门禁 `cd frontend && npx vue-tsc -b && npm test` 必须零错误、31 测试全过（本波不新增测试文件，taskActivity.ts 不改动）。
- 不安装、不重启任何进程（用户操作）；不触碰 live vault 与 live DB。
- `.task-pill` 及 `status-*`/`importance-*`/`type-*` 修饰类已定义于 App.vue 全局样式块，直接使用。
- 任务顺序执行（同一文件多处改动，不并行）。

---

### Task 1: 属性胶囊下沉 + 子任务行重要性胶囊

**Files:**
- Modify: `frontend/src/views/Tasks.vue`
- Modify: `frontend/src/components/tasks/SubtaskDrawer.vue`

**Interfaces:**
- Consumes: `.task-pill` 全局类（App.vue）；`taskStatusLabel`/`taskImportanceLabel`（已有导入）。
- Produces: 主面板与抽屉的事实行结构 `[状态胶囊][重要性胶囊][开始卡][结束卡]`；抽屉子任务行 `状态圆点 + 标题 + 重要性胶囊 + ›`。

- [ ] **Step 1: Tasks.vue 模板 — 删除主面板 kicker**

old:
```html
            <div class="detail-title-group">
              <div class="detail-kicker">
                <span class="task-pill" :class="`status-${detail.root.status}`">{{ taskStatusLabel(detail.root.status) }}</span>
                <span class="task-pill" :class="`importance-${detail.root.importance}`">{{ taskImportanceLabel(detail.root.importance) }}</span>
              </div>
              <h2>{{ detail.root.title }}</h2>
```
new:
```html
            <div class="detail-title-group">
              <h2>{{ detail.root.title }}</h2>
```

- [ ] **Step 2: Tasks.vue 模板 — 事实行并入胶囊、删除优先级卡**

old:
```html
          <div class="detail-facts">
            <div><span>开始</span><strong>{{ detail.root.start_date }}</strong></div>
            <div><span>结束</span><strong>{{ detail.root.end_date }}</strong></div>
            <div><span>优先级</span><strong>{{ taskImportanceLabel(detail.root.importance) }}</strong></div>
          </div>
```
new:
```html
          <div class="detail-facts">
            <span class="task-pill" :class="`status-${detail.root.status}`">{{ taskStatusLabel(detail.root.status) }}</span>
            <span class="task-pill" :class="`importance-${detail.root.importance}`">{{ taskImportanceLabel(detail.root.importance) }}</span>
            <div><span>开始</span><strong>{{ detail.root.start_date }}</strong></div>
            <div><span>结束</span><strong>{{ detail.root.end_date }}</strong></div>
          </div>
```

- [ ] **Step 3: Tasks.vue CSS — kicker 行删除、标题上移**

old:
```css
.detail-title-group { min-width: 0; }
.detail-kicker { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
.detail-title-group h2 { margin: 7px 0 5px; font-size: clamp(22px, 3vw, 30px); line-height: 1.18; letter-spacing: var(--tracking-tight); }
```
new:
```css
.detail-title-group { min-width: 0; }
.detail-title-group h2 { margin: 0 0 5px; font-size: clamp(22px, 3vw, 30px); line-height: 1.18; letter-spacing: var(--tracking-tight); }
```

- [ ] **Step 4: Tasks.vue CSS — 事实行 grid 改 flex、label 选择器收窄到 div 内**

old:
```css
.detail-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 26px 0; }
.detail-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.detail-facts span { color: var(--text-faint); font-size: 10px; }
.detail-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
```
new:
```css
.detail-facts { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 26px 0; }
.detail-facts .task-pill { flex: none; }
.detail-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.detail-facts div span { color: var(--text-faint); font-size: 10px; }
.detail-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
```
（`.detail-facts span` 必须收窄为 `.detail-facts div span`，否则胶囊会被 label 样式污染——胶囊是直接子 span。）

- [ ] **Step 5: Tasks.vue CSS — 中间断点的 label 选择器同步收窄**

old:
```css
.detail-facts span { font-size: 11px; }
.detail-facts strong { font-size: 13px; }
```
new:
```css
.detail-facts div span { font-size: 11px; }
.detail-facts div strong { font-size: 13px; }
```

- [ ] **Step 6: Tasks.vue CSS — 删除两处媒体查询里的死 grid 规则**

在 ~973 行的单行样式链中删除子串 `.detail-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }`（即把
`.task-workspace { grid-template-columns: 290px minmax(0, 1fr); }.detail-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }.detail-actions { flex-wrap: wrap; justify-content: flex-end; }`
改为
`.task-workspace { grid-template-columns: 290px minmax(0, 1fr); }.detail-actions { flex-wrap: wrap; justify-content: flex-end; }`）。

在 ~977 行移动端单行样式链中把子串
`.detail-facts { grid-template-columns: 1fr 1fr; margin: 14px 0; }.detail-facts div:last-child { grid-column: 1 / -1; }`
改为
`.detail-facts { margin: 14px 0; }`。

- [ ] **Step 7: SubtaskDrawer.vue 模板 — 删除抽屉 kicker**

old:
```html
          <div class="drawer-title-group">
            <div class="drawer-kicker">
              <span class="task-pill" :class="`status-${node.status}`">{{ taskStatusLabel(node.status) }}</span>
              <span class="task-pill" :class="`importance-${node.importance}`">{{ taskImportanceLabel(node.importance) }}</span>
            </div>
            <h3>{{ node.title }}</h3>
          </div>
```
new:
```html
          <div class="drawer-title-group">
            <h3>{{ node.title }}</h3>
          </div>
```

- [ ] **Step 8: SubtaskDrawer.vue 模板 — 事实行并入胶囊、删除优先级卡**

old:
```html
        <div class="drawer-facts">
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
          <div><span>优先级</span><strong>{{ taskImportanceLabel(node.importance) }}</strong></div>
        </div>
```
new:
```html
        <div class="drawer-facts">
          <span class="task-pill" :class="`status-${node.status}`">{{ taskStatusLabel(node.status) }}</span>
          <span class="task-pill" :class="`importance-${node.importance}`">{{ taskImportanceLabel(node.importance) }}</span>
          <div><span>开始</span><strong>{{ node.start_date }}</strong></div>
          <div><span>结束</span><strong>{{ node.end_date }}</strong></div>
        </div>
```

- [ ] **Step 9: SubtaskDrawer.vue 模板 — 子任务行追加重要性胶囊**

old:
```html
              <span class="status-dot" :class="`status-${child.status}`"></span>
              <strong>{{ child.title }}</strong>
              <span class="drawer-child-chevron" aria-hidden="true">›</span>
```
new:
```html
              <span class="status-dot" :class="`status-${child.status}`"></span>
              <strong>{{ child.title }}</strong>
              <span class="task-pill" :class="`importance-${child.importance}`">{{ taskImportanceLabel(child.importance) }}</span>
              <span class="drawer-child-chevron" aria-hidden="true">›</span>
```

- [ ] **Step 10: SubtaskDrawer.vue CSS**

old:
```css
.drawer-title-group h3 { margin: 8px 0 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }
```
new:
```css
.drawer-title-group h3 { margin: 0; font-size: 20px; line-height: 1.25; letter-spacing: var(--tracking-tight); }
```

删除整行（kicker 样式）：
```css
.drawer-kicker { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
```

old:
```css
.drawer-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 18px 0 0; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
```
new:
```css
.drawer-facts { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 18px 0 0; }
.drawer-facts .task-pill { flex: none; }
.drawer-facts div { min-width: 0; display: grid; gap: 5px; padding: 10px 12px; border-radius: 13px; background: color-mix(in srgb, var(--text-primary) 3%, transparent); }
.drawer-facts div span { color: var(--text-faint); font-size: 10px; }
.drawer-facts strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 570; }
```

子任务行 grid 增加胶囊列——old:
```css
  grid-template-columns: 10px minmax(0, 1fr) 16px;
```
new:
```css
  grid-template-columns: 10px minmax(0, 1fr) auto 16px;
```

删除 ≤768px 媒体块中的两行死 grid 规则：
```css
  .drawer-facts { grid-template-columns: 1fr 1fr; }
  .drawer-facts div:last-child { grid-column: 1 / -1; }
```

- [ ] **Step 11: 门禁 + 提交**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 零错误，31/31 通过。

```bash
git add frontend/src/views/Tasks.vue frontend/src/components/tasks/SubtaskDrawer.vue
git commit -m "feat(tasks): sink attribute pills into facts row, pill child rows"
```

---

### Task 2: 抽屉精简 + 竖条收起把手

**Files:**
- Modify: `frontend/src/components/tasks/SubtaskDrawer.vue`
- Modify: `frontend/src/views/Tasks.vue`（仅 SubtaskDrawer 绑定处）

**Interfaces:**
- Consumes: Task 1 后的两文件状态。
- Produces: SubtaskDrawer emits 收敛为 `close/select/edit`；push 槽宽 `min(400px, 30vw)`（无把手槽）。

- [ ] **Step 1: 右上角只留编辑按钮**

old:
```html
          <div class="drawer-corner">
            <button type="button" class="corner-button" aria-label="编辑任务" title="编辑" @click="emit('edit', node)"><el-icon><EditPen /></el-icon></button>
            <button type="button" class="corner-button" aria-label="更改状态" title="更改状态" @click="emit('status', node)"><el-icon><Refresh /></el-icon></button>
            <button type="button" class="corner-button" aria-label="移动任务" title="移动" @click="emit('move', node)"><el-icon><Rank /></el-icon></button>
          </div>
```
new:
```html
          <div class="drawer-corner">
            <button type="button" class="corner-button" aria-label="编辑任务" title="编辑" @click="emit('edit', node)"><el-icon><EditPen /></el-icon></button>
          </div>
```

图标导入收缩——old:
```ts
import { EditPen, Rank, Refresh } from '@element-plus/icons-vue'
```
new:
```ts
import { EditPen } from '@element-plus/icons-vue'
```

- [ ] **Step 2: emits 收敛**

old:
```ts
const emit = defineEmits<{
  close: []
  select: [taskId: string]
  progress: [task: TaskNode]
  add: [task: TaskNode]
  status: [task: TaskNode]
  edit: [task: TaskNode]
  move: [task: TaskNode]
}>()
```
new:
```ts
const emit = defineEmits<{
  close: []
  select: [taskId: string]
  edit: [task: TaskNode]
}>()
```

- [ ] **Step 3: 删除两个栏目标题行的 ＋ 按钮**

old:
```html
            <div><span>子任务</span><strong>{{ children.length }} 个</strong></div>
            <button type="button" class="drawer-add" aria-label="添加子任务" title="添加子任务" @click="emit('add', node)">＋</button>
```
new:
```html
            <div><span>子任务</span><strong>{{ children.length }} 个</strong></div>
```

old:
```html
            <div><span>进展与记录</span><strong>{{ activity.length }} 条</strong></div>
            <button type="button" class="drawer-add" aria-label="记录进展" title="记录进展" @click="emit('progress', node)">＋</button>
```
new:
```html
            <div><span>进展与记录</span><strong>{{ activity.length }} 条</strong></div>
```

- [ ] **Step 4: 收起把手改竖条**

old:
```html
      <button ref="collapseRef" type="button" class="drawer-collapse" aria-label="收起子任务详情" title="收起" @click="emit('close')">‹</button>
```
new:
```html
      <button ref="collapseRef" type="button" class="drawer-collapse" aria-label="收起子任务详情" title="收起" @click="emit('close')"><span aria-hidden="true">›</span></button>
```

CSS——old（整块替换）：
```css
/* Collapse handle: rides the drawer's left edge, vertically centered. */
.drawer-collapse {
  position: absolute;
  left: -24px;
  top: 50%;
  transform: translateY(-50%);
  width: 24px;
  height: 64px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-glass);
  border-right: 0;
  border-radius: 12px 0 0 12px;
  background: var(--bg-glass);
  box-shadow: var(--shadow-sm), var(--inset-highlight);
  backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturate));
  color: var(--text-muted);
  font-size: 15px;
  line-height: 1;
  cursor: pointer;
  transition: color var(--motion-fast) ease, background var(--motion-fast) ease;
}
.drawer-collapse:hover { color: var(--text-primary); background: var(--bg-glass-strong); }
```
new:
```css
/* Collapse handle: a thin bar on the drawer's inner left edge; hover widens it into a › tab. */
.drawer-collapse {
  position: absolute;
  left: 0;
  top: 22px;
  bottom: 22px;
  width: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: color-mix(in srgb, var(--text-primary) 12%, transparent);
  color: var(--text-muted);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  transition: width var(--motion-fast) ease, background var(--motion-fast) ease;
}
.drawer-collapse > span { opacity: 0; transition: opacity var(--motion-fast) ease; }
.drawer-collapse:hover, .drawer-collapse:focus-visible { width: 24px; background: color-mix(in srgb, var(--text-primary) 6%, transparent); }
.drawer-collapse:hover > span, .drawer-collapse:focus-visible > span { opacity: 1; }
```

≤768px 媒体块内追加（触摸无 hover，常显）：
```css
  .drawer-collapse { width: 24px; background: color-mix(in srgb, var(--text-primary) 6%, transparent); }
  .drawer-collapse > span { opacity: 1; }
```

reduced-motion 块——old:
```css
  .subtask-drawer-slot, .drawer-backdrop { transition-duration: 1ms !important; }
```
new:
```css
  .subtask-drawer-slot, .drawer-backdrop, .drawer-collapse, .drawer-collapse > span { transition-duration: 1ms !important; }
```

- [ ] **Step 5: push 槽取消把手槽**

old:
```css
/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone.
   The 24px padding strip left of the drawer hosts the collapse handle. */
.subtask-drawer-slot {
  flex: 0 0 auto;
  width: 0;
  padding-left: 0;
  overflow: hidden;
  transition: width var(--motion-normal) var(--ease-emphasized), padding-left var(--motion-normal) var(--ease-emphasized);
}
.subtask-drawer-slot.open { width: calc(min(400px, 30vw) + 24px); padding-left: 24px; }
```
new:
```css
/* Desktop (>=1150px): in-flow push — the slot is a flex item of .task-detail-zone. */
.subtask-drawer-slot {
  flex: 0 0 auto;
  width: 0;
  overflow: hidden;
  transition: width var(--motion-normal) var(--ease-emphasized);
}
.subtask-drawer-slot.open { width: min(400px, 30vw); }
```

<1150px 媒体块内——old:
```css
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; padding: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
```
new:
```css
  .subtask-drawer-slot { position: fixed; top: 0; right: 0; bottom: 0; z-index: 2301; width: min(400px, 92vw); margin: 0; overflow: visible; transform: translateX(100%); transition: transform var(--motion-normal) var(--ease-emphasized); }
```

- [ ] **Step 6: 删除 .drawer-add 样式**

删除这两块：
```css
.drawer-add {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  background: var(--bg-glass);
  color: var(--accent);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
  transition: background var(--motion-fast) ease;
}
.drawer-add:hover { background: var(--bg-glass-strong); }
```
以及 ≤768px 媒体块内的：
```css
  .drawer-add { width: 36px; height: 36px; }
```

- [ ] **Step 7: Tasks.vue 收敛抽屉绑定**

old:
```html
        <SubtaskDrawer
          ref="drawerRef"
          :node="drawerNode"
          :activity="drawerActivity"
          :children="drawerChildren"
          :parent="drawerParent"
          @close="closeDrawer"
          @select="focusTask"
          @progress="openProgress"
          @add="openSubtask"
          @status="openStatus"
          @edit="openEdit"
          @move="openMove"
        />
```
new:
```html
        <SubtaskDrawer
          ref="drawerRef"
          :node="drawerNode"
          :activity="drawerActivity"
          :children="drawerChildren"
          :parent="drawerParent"
          @close="closeDrawer"
          @select="focusTask"
          @edit="openEdit"
        />
```
（openProgress/openSubtask/openStatus/openMove 在主面板与 TaskTree 绑定处仍在使用，保留函数本身。）

- [ ] **Step 8: 门禁 + 提交**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 零错误，31/31 通过（emits 收敛后无残留引用）。

```bash
git add frontend/src/views/Tasks.vue frontend/src/components/tasks/SubtaskDrawer.vue
git commit -m "feat(tasks): slim drawer to edit-only actions with bar collapse handle"
```

---

### Task 3: 活动条目重排 + 记录详情弹窗

**Files:**
- Modify: `frontend/src/views/Tasks.vue`
- Modify: `frontend/src/components/tasks/SubtaskDrawer.vue`

**Interfaces:**
- Consumes: `TaskActivityEntry`（frontend/src/utils/taskActivity.ts，含 type/taskId/taskTitle/title/detail/note/time）；MotionModal。
- Produces: Tasks.vue 状态 `activityDetail: ref<TaskActivityEntry | null>` + `activityDetailOpen` computed（v-model 适配）；抽屉新增 emit `inspect: [entry: TaskActivityEntry]`。

- [ ] **Step 1: Tasks.vue — 导入类型**

old:
```ts
import { buildTaskActivity, taskImportanceLabel, taskStatusLabel } from '@/utils/taskActivity'
```
new:
```ts
import { buildTaskActivity, taskImportanceLabel, taskStatusLabel, type TaskActivityEntry } from '@/utils/taskActivity'
```

- [ ] **Step 2: Tasks.vue — 状态**

在 `const archiveConfirmOpen = ref(false)` 之后添加：
```ts
const activityDetail = ref<TaskActivityEntry | null>(null)
const activityDetailOpen = computed({
  get: () => activityDetail.value !== null,
  set: (open: boolean) => { if (!open) activityDetail.value = null },
})
```

- [ ] **Step 3: Tasks.vue — 主面板条目模板**

old:
```html
              <article v-for="item in activity" :key="item.id" class="activity-item">
                <span class="activity-dot" :class="item.type"></span>
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong><span class="task-pill" :class="`type-${item.type}`">{{ item.title }}</span></strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
                  <p class="activity-meta">
                    <button type="button" class="activity-task" :title="item.taskTitle" @click="focusTask(item.taskId)">{{ item.taskTitle }}</button>
                    <span v-if="item.detail" class="activity-detail">{{ item.detail }}</span>
                  </p>
                  <p v-if="item.note" class="activity-note">{{ item.note }}</p>
                </div>
              </article>
```
new:
```html
              <article v-for="item in activity" :key="item.id" class="activity-item" @click="activityDetail = item">
                <span class="activity-dot" :class="item.type"></span>
                <div class="activity-copy">
                  <div class="activity-head">
                    <strong>
                      <span class="task-pill" :class="`type-${item.type}`">{{ item.title }}</span>
                      <button type="button" class="activity-task" :title="item.taskTitle" @click.stop="focusTask(item.taskId)">{{ item.taskTitle }}</button>
                    </strong>
                    <time>{{ formatTimestamp(item.time) }}</time>
                  </div>
                  <p v-if="item.detail" class="activity-detail">{{ item.detail }}</p>
                  <p v-if="item.note" class="activity-note">{{ item.note }}</p>
                </div>
              </article>
```

- [ ] **Step 4: Tasks.vue — 详情弹窗（模板）**

在归档确认 MotionModal 的 `</MotionModal>` 之后、`</div>`（根容器结束）之前插入：
```html
    <MotionModal v-model="activityDetailOpen" aria-label="记录详情">
      <section v-if="activityDetail" class="activity-dialog glass-surface-heavy">
        <div class="activity-dialog-head">
          <span class="task-pill" :class="`type-${activityDetail.type}`">{{ activityDetail.title }}</span>
          <strong>{{ activityDetail.taskTitle }}</strong>
        </div>
        <time>{{ formatTimestamp(activityDetail.time) }}</time>
        <p v-if="activityDetail.detail">{{ activityDetail.detail }}</p>
        <p v-if="activityDetail.note">{{ activityDetail.note }}</p>
        <p v-else class="activity-dialog-empty">这条记录没有备注。</p>
      </section>
    </MotionModal>
```

- [ ] **Step 5: Tasks.vue — CSS**

old:
```css
.activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 14px 0; }
```
new:
```css
.activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 14px 0; border-radius: 12px; cursor: pointer; transition: background var(--motion-fast) ease; }
.activity-item:hover { background: color-mix(in srgb, var(--text-primary) 4%, transparent); }
```

old:
```css
.activity-meta { display: flex; align-items: baseline; flex-wrap: wrap; gap: 7px; margin: 7px 0 0; }
.activity-task { display: inline-block; max-width: 11em; padding: 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-task:hover { text-decoration: underline; }
.activity-detail { color: var(--text-secondary); }
.activity-note { margin: 5px 0 0; color: var(--text-muted); line-height: 1.5; white-space: pre-wrap; }
```
new:
```css
.activity-head strong .task-pill { flex: none; }
.activity-task { min-width: 0; padding: 0; border: 0; background: transparent; color: var(--accent); font: inherit; font-weight: 650; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-task:hover { text-decoration: underline; }
.activity-detail { margin: 6px 0 0; color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-note { margin: 4px 0 0; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
```

中间断点字体规则——old:
```css
.activity-meta, .activity-note { font-size: 13px; }
```
new:
```css
.activity-detail, .activity-note { font-size: 13px; }
```

在 activity CSS 块之后追加弹窗样式：
```css
.activity-dialog { display: grid; gap: 10px; min-width: min(420px, 86vw); max-width: 86vw; padding: 22px; border-radius: 20px; }
.activity-dialog-head { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
.activity-dialog-head strong { font-size: 15px; }
.activity-dialog time { color: var(--text-faint); font-size: 11px; font-variant-numeric: tabular-nums; }
.activity-dialog p { margin: 0; color: var(--text-secondary); font-size: 13px; line-height: 1.6; white-space: pre-wrap; }
.activity-dialog p.activity-dialog-empty { color: var(--text-faint); }
```

- [ ] **Step 6: SubtaskDrawer.vue — 抽屉条目点击 + 单行省略**

模板——old:
```html
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item">
```
new:
```html
            <article v-for="item in activity" :key="item.id" class="drawer-activity-item" @click="emit('inspect', item)">
```

emits——old:
```ts
const emit = defineEmits<{
  close: []
  select: [taskId: string]
  edit: [task: TaskNode]
}>()
```
new:
```ts
const emit = defineEmits<{
  close: []
  select: [taskId: string]
  edit: [task: TaskNode]
  inspect: [entry: TaskActivityEntry]
}>()
```

CSS——old:
```css
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 12px 0; }
```
new:
```css
.drawer-activity-item { display: grid; grid-template-columns: 16px minmax(0, 1fr); gap: 9px; padding: 12px 0; border-radius: 12px; cursor: pointer; transition: background var(--motion-fast) ease; }
.drawer-activity-item:hover { background: color-mix(in srgb, var(--text-primary) 4%, transparent); }
```

old:
```css
.drawer-activity-detail { margin: 6px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.5; }
.drawer-activity-note { margin: 5px 0 0; color: var(--text-muted); font-size: 12px; line-height: 1.5; white-space: pre-wrap; }
```
new:
```css
.drawer-activity-detail { margin: 6px 0 0; color: var(--text-secondary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.drawer-activity-note { margin: 4px 0 0; color: var(--text-muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
```

- [ ] **Step 7: Tasks.vue — 抽屉 inspect 绑定 + 处理函数**

SubtaskDrawer 绑定追加（`@edit="openEdit"` 之后）：
```html
          @inspect="openActivityDetail"
```

在 `function openEdit` 之前添加：
```ts
function openActivityDetail(entry: TaskActivityEntry) {
  activityDetail.value = entry
}
```

- [ ] **Step 8: 门禁 + 提交**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 零错误，31/31 通过。

```bash
git add frontend/src/views/Tasks.vue frontend/src/components/tasks/SubtaskDrawer.vue
git commit -m "feat(tasks): inline task names in activity heads with detail modal"
```

---

### Task 4: 编辑表单整合（状态 + 父任务）

**Files:**
- Modify: `frontend/src/views/Tasks.vue`

**Interfaces:**
- Consumes: 既有 `statusForm`（{status, note, cascade}）、`moveForm`（{parentId}）、`statusSheetOptions`、`isTerminalStatus`、`moveCandidates` computeds；store 写方法 `update(taskId, patch)`、`setStatus(taskId, status, closureNote?, cascade?)`、`moveSubtask(taskId, parentId, position)`——每次写入后 store 自动 applyDetail 刷新 document_version，链式调用无需手工传版本。
- Produces: 编辑模式表单含状态/父任务字段；保存链 更新 → 改状态（仅变化时）→ 移动（仅变化时）。

- [ ] **Step 1: openEdit 预填状态与父任务**

old:
```ts
function openEdit(task: TaskNode) {
  sheetMode.value = 'edit'
  targetNode.value = task
  form.value = { kind: task.kind, title: task.title, description: task.description, start_date: task.start_date, end_date: task.end_date, importance: task.importance }
  sheetOpen.value = true
}
```
new:
```ts
function openEdit(task: TaskNode) {
  sheetMode.value = 'edit'
  targetNode.value = task
  form.value = { kind: task.kind, title: task.title, description: task.description, start_date: task.start_date, end_date: task.end_date, importance: task.importance }
  statusForm.value = { status: task.status, note: '', cascade: false }
  moveForm.value.parentId = task.parent_id || ''
  sheetOpen.value = true
}
```

- [ ] **Step 2: 表单模板追加编辑专属字段**

在编辑/创建共用分支（`v-else` form-grid）的「重要程度」字段（`.field full` 含 importanceOptions 的 el-select）之后、`</div>`（form-grid 结束）之前插入：
```html
          <label v-if="sheetMode === 'edit'" class="field full">
            <span>状态</span>
            <el-select
              v-model="statusForm.status"
              popper-class="task-select-popper"
              placement="bottom-start"
              :offset="0"
              :fit-input-width="true"
            >
              <el-option v-for="option in statusSheetOptions" :key="option.value" :value="option.value" :label="option.label" />
            </el-select>
          </label>
          <label v-if="sheetMode === 'edit' && isTerminalStatus" class="field full">
            <span>关闭说明（可选）</span>
            <textarea v-model="statusForm.note" rows="4" placeholder="总结结果、原因或后续安排…"></textarea>
          </label>
          <label v-if="sheetMode === 'edit' && targetNode?.role === 'subtask'" class="field full">
            <span>父任务</span>
            <el-select
              v-model="moveForm.parentId"
              popper-class="task-select-popper"
              placement="bottom-start"
              :offset="0"
              :fit-input-width="true"
              required
            >
              <el-option v-for="candidate in moveCandidates" :key="candidate.id" :value="candidate.id" :label="candidate.title" />
            </el-select>
          </label>
          <label v-if="sheetMode === 'edit' && targetNode?.role === 'root' && isTerminalStatus" class="check-field">
            <input v-model="statusForm.cascade" type="checkbox" aria-label="同时关闭所有未完成的子任务" />
            <span>同时关闭所有未完成的子任务</span>
          </label>
```

- [ ] **Step 3: submitSheet 编辑分支改链式写入**

old:
```ts
    else if (mode === 'edit' && writeTarget) await store.update(writeTarget.id, fields)
```
new:
```ts
    else if (mode === 'edit' && writeTarget) {
      await store.update(writeTarget.id, fields)
      // Each store write refreshes selectedDetail (and its version), so chained
      // calls pick up the fresh version automatically.
      if (statusForm.value.status !== writeTarget.status) {
        await store.setStatus(writeTarget.id, statusForm.value.status, statusForm.value.note || undefined, statusForm.value.cascade)
      }
      if (writeTarget.role === 'subtask' && moveForm.value.parentId && moveForm.value.parentId !== writeTarget.parent_id) {
        await store.moveSubtask(writeTarget.id, moveForm.value.parentId, 9999)
      }
    }
```

- [ ] **Step 4: 门禁 + 提交**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 零错误，31/31 通过。

```bash
git add frontend/src/views/Tasks.vue
git commit -m "feat(tasks): consolidate status and parent editing into the edit sheet"
```

---

### Task 5: 文档更新

**Files:**
- Modify: `docs/requirement/09-task-management.md`
- Modify: `docs/development/09-task-management.md`

**Interfaces:**
- Consumes: Task 1-4 的最终实现；spec §13。
- Produces: 两份文档与本波最终状态一致（顺带消除上一轮终审指出的「‹ 返回上级」措辞偏差）。

- [ ] **Step 1: requirement §4.3.1 条目 4**

old:
```
4. 全树聚合的进展与记录时间线：根任务与所有子任务的进展、审计条目按时间倒序展示。条目标题为记录类型（如「进展」「状态变更」），明细行标注所属任务名与具体变化（完成度、状态流转），备注另起一行；点击任务名可聚焦该任务（子任务打开详情抽屉，根任务回到总览）。
```
new:
```
4. 全树聚合的进展与记录时间线：根任务与所有子任务的进展、审计条目按时间倒序展示。条目头行为「记录类型胶囊 + 所属任务名 + 时间」，明细与备注各单行省略；点击条目弹出详情弹窗查看全文（类型、任务名、时间、变化明细、备注）；点击任务名可聚焦该任务（子任务打开详情抽屉，根任务回到总览）。
```

- [ ] **Step 2: requirement §4.3.4 整块替换**

old:
```
- 详情面板固定展示根任务总览；点击任务树中的子任务，从右侧滑出抽屉展示该子任务的标题、状态、描述、日期、优先级、直属子任务列表与其专属进展记录。
- 抽屉头部提供「返回上级」入口；点击直属子任务列表在抽屉内下钻，父任务为根任务时返回即收起抽屉回总览。
- 桌面端（≥1150px）抽屉压缩详情面板让位；较窄屏幕与手机端以浮层从右侧滑入，点击遮罩关闭。抽屉左缘中部常驻收起符号，Esc 亦可收起。
- 抽屉右上角提供编辑、更改状态、移动操作；「记录进展」「添加子任务」分别为进展与子任务栏目标题行的加号按钮，复用统一的表单弹层。
- 关键属性（任务状态、重要程度、记录类型）以软色胶囊标注。
- 任务视图整页不滚动：仅任务列表、详情面板与抽屉内部各自滚动（桌面与移动端一致）；详情面板下半部分保留与阅境轩一致的疏朗留白。
```
new:
```
- 详情面板固定展示根任务总览；点击任务树中的子任务，从右侧滑出抽屉展示该子任务的标题、描述、日期、直属子任务列表与其专属进展记录。
- 抽屉头部提供「‹ 父任务名」返回入口；点击直属子任务列表在抽屉内下钻，父任务为根任务时返回即收起抽屉回总览；子任务行含状态圆点、标题与重要性胶囊。
- 桌面端（≥1150px）抽屉压缩详情面板让位；较窄屏幕与手机端以浮层从右侧滑入，点击遮罩关闭。抽屉左缘内侧为竖条收起把手（悬浮加宽显示 ›，移动端常显），Esc 亦可收起。
- 抽屉右上角仅提供编辑操作；编辑表单可一并修改状态与父任务等信息（保存时按 更新字段 → 改状态 → 移动 顺序执行）。记录进展、添加子任务、更改状态、移动均通过任务树行的内联按钮完成。
- 关键属性（任务状态、重要程度、记录类型）以软色胶囊标注，置于头部事实行（开始/结束之前）；「优先级」事实卡不再单独展示。
- 任务视图整页不滚动：仅任务列表、详情面板与抽屉内部各自滚动（桌面与移动端一致）；详情面板下半部分保留与阅境轩一致的疏朗留白。
```

- [ ] **Step 3: requirement §10.5 验收条目更新**

old:
```
- [ ] 抽屉展示直属子任务列表并可逐级下钻；编辑/更改状态/移动位于抽屉右上角，栏目加号触发记录进展与添加子任务；左缘收起符号、Esc、遮罩点击均可收起抽屉。
```
new:
```
- [ ] 抽屉展示直属子任务列表（行内含重要性胶囊）并可逐级下钻；抽屉右上角仅编辑入口，编辑表单可修改状态与父任务；左缘竖条把手、Esc、遮罩点击均可收起抽屉。
- [ ] 活动流条目点击可弹出详情弹窗查看完整记录（类型、任务名、时间、明细、备注全文）。
```

- [ ] **Step 4: development §11.3 整块替换**

old:
```
- 详情面板恒渲染根任务（总览、拆解树、聚合活动流）；`Tasks.vue` 以 `drawerNodeId` ref 驱动子任务抽屉，不再有整面板切换。
- `utils/taskActivity.ts` 的 `buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 负责全树聚合（省略 scope）与单任务过滤（传入 scope，供抽屉）；条目 `title` 为名词式类型标签（进展/状态变更/移动…），`detail` 携带具体变化（完成度 X%、状态 A → B），`taskTitle` 归因；`taskStatusLabel`/`taskImportanceLabel`/`formatTimestamp` 一并从视图层下沉到 utils。
- `SubtaskDrawer.vue` 常驻挂载：桌面端（≥1150px）以 flex 定宽 slot 实现 push 压缩（slot 总宽 `min(400px, 30vw) + 24px`，其中 24px 为左缘把手槽）；<1150px 转为 fixed 浮层 + 遮罩；≤768px 宽度 min(88vw, 400px)。z-index 2300/2301，低于 MotionModal 的 2400。
- 抽屉结构：头部「‹ 返回上级」行（emit `select`，父级绑 `focusTask`）+ 眉标属性胶囊 + 右上角编辑/更改状态/移动图标按钮（EditPen/Refresh/Rank）；正文为「子任务」栏目（直属子任务列表，点击下钻）与「进展与记录」栏目（栏目标题行「＋」分别为添加子任务/记录进展）；左缘中部「‹」收起把手替代关闭按钮（打开时键盘焦点落于把手）。
- 抽屉操作全部 emit 回 `Tasks.vue` 复用 sheet 表单体系；写入成功后子任务目标自动打开其抽屉，根任务目标回到总览。
- 布局锁定：`.tasks-page.view-tasks` 为 `calc(100dvh - 64px)` 的 flex 列（移动端 `calc(100dvh - 96px - safe-top - safe-bottom)`），workspace `flex: 1 1 auto; min-height: 0`，仅面板内部滚动；属性胶囊 `.task-pill`（status-*/importance-*/type-*）定义于 App.vue 全局样式块，主面板与抽屉共用。
```
new:
```
- 详情面板恒渲染根任务（总览、拆解树、聚合活动流）；`Tasks.vue` 以 `drawerNodeId` ref 驱动子任务抽屉，不再有整面板切换。
- `utils/taskActivity.ts` 的 `buildTaskActivity(nodes, progress, audit, scopeTaskId?)` 负责全树聚合（省略 scope）与单任务过滤（传入 scope，供抽屉）；条目 `title` 为名词式类型标签（进展/状态变更/移动…），`detail` 携带具体变化（完成度 X%、状态 A → B），`taskTitle` 归因；`taskStatusLabel`/`taskImportanceLabel`/`formatTimestamp` 一并从视图层下沉到 utils。活动条目头行为 类型胶囊 + 任务名 + 时间，明细/备注单行省略；点击条目（抽屉经 emit `inspect`）由 `Tasks.vue` 的 MotionModal 详情弹窗展示全文。
- `SubtaskDrawer.vue` 常驻挂载：桌面端（≥1150px）以 flex 定宽 slot 实现 push 压缩（slot 总宽 `min(400px, 30vw)`）；<1150px 转为 fixed 浮层 + 遮罩；≤768px 宽度 min(88vw, 400px)。z-index 2300/2301，低于 MotionModal 的 2400。
- 抽屉结构：头部「‹ {父任务名}」返回行（emit `select`，父级绑 `focusTask`）+ 右上角唯一编辑图标按钮（EditPen，emit `edit`）；正文为「子任务」栏目（直属子任务列表，行 = 状态圆点 + 标题 + 重要性胶囊，点击下钻）与「进展与记录」栏目；左缘内侧竖条收起把手（静止 4px，悬浮/移动端加宽至 24px 显 ›，打开时键盘焦点落于把手）。
- 抽屉操作 emit 回 `Tasks.vue` 复用 sheet 表单体系（emits 为 close/select/edit/inspect）；写入成功后子任务目标自动打开其抽屉，根任务目标回到总览。
- 编辑表单整合：编辑模式新增「状态」（沿用 statusSheetOptions 过滤；终态显示关闭说明，根任务另显示级联勾选）与「父任务」（仅子任务，候选沿用 moveCandidates）字段；保存依次执行 更新字段 → 改状态（仅变化时）→ 移动（仅变化时），版本号由 store 写后自动刷新衔接。
- 布局锁定：`.tasks-page.view-tasks` 为 `calc(100dvh - 64px)` 的 flex 列（移动端 `calc(100dvh - 96px - safe-top - safe-bottom)`），workspace `flex: 1 1 auto; min-height: 0`，仅面板内部滚动；属性胶囊 `.task-pill`（status-*/importance-*/type-*）定义于 App.vue 全局样式块，主面板与抽屉共用，置于头部事实行最前（「优先级」事实卡已移除）。
```

- [ ] **Step 5: 提交**

```bash
git add docs/requirement/09-task-management.md docs/development/09-task-management.md
git commit -m "docs(tasks): record wave-2 drawer and activity interactions"
```

---

### Task 6: 验证与构建产物

**Files:**
- Build: `frontend/dist_new/`（vite 产物）、`backend/target/release/obsidian-brain`（rust-embed 嵌入新前端）

**Interfaces:**
- Consumes: Task 1-5 的全部提交。

- [ ] **Step 1: 完整门禁**

Run: `cd frontend && npx vue-tsc -b && npm test`
Expected: 类型零错误；31 个测试全部通过。

- [ ] **Step 2: 构建前端产物**

Run: `cd frontend && npx vite build --outDir dist_new`
Expected: 构建成功；`dist_new/` 文件数与此前同量级（168 左右）。

- [ ] **Step 3: 构建 release 二进制**

Run: `cd backend && cargo build --release`
Expected: 编译成功。**不要安装、不要重启任何进程**（安装由用户执行）。

- [ ] **Step 4: 报告**

报告 `dist_new` 文件数、二进制大小与 mtime、门禁结果。无需 commit（构建产物不入库）。

---

## Self-Review 结论

- Spec §13 四小节全覆盖：13.1→Task 1；13.2→Task 1；13.3→Task 3；13.4→Task 2+4。
- 无占位符；所有 old/new 均为对当前文件的逐字锚点（含单行样式链的子串级替换说明）。
- 类型一致性：`TaskActivityEntry`（utils 已有，Task 3 导入）；emit `inspect: [entry: TaskActivityEntry]`（SubtaskDrawer 定义 → Tasks.vue `openActivityDetail` 消费）；`statusForm`/`moveForm` 复用既有 refs（Task 4 预填 + 模板绑定 + 链式提交）。
- 中间态说明：Task 2 删除 emits 后若 vue-tsc 报 Tasks.vue 绑定处多余 handler 错误，属预期，Task 2 Step 7 同任务内消除（Step 7 在同一提交内完成）。
