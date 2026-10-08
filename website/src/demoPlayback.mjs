// Example workflows only. No application data or model requests are involved.
export const scenarios = {
  reader: [
    { label: '选择书籍', detail: '将本地 Markdown 文集或 PDF 整理为书架，按书籍进入阅读。' },
    { label: '阅读正文', detail: '在同一阅读空间浏览正文、文件与章节目录。' },
    { label: '定位章节', detail: '选择相关文件，直接阅读表格、代码与公式等内容。' },
    { label: '恢复位置', detail: '再次打开书籍，恢复已同步的文档和阅读位置。' },
  ],
  timeline: [
    { label: '编辑小记', detail: '以 Markdown 记录想法，附加图片与标签。' },
    { label: '发布记录', detail: '小记按日期组织，发布后仍可编辑或删除。' },
    { label: '筛选回顾', detail: '结合关键词、日期和标签，定位需要回顾的记录。' },
  ],
  tasks: [
    { label: '长期任务', detail: '明确目标、起止日期和重要程度，集中管理任务信息。' },
    { label: '拆解子任务', detail: '将目标拆为多级子任务，每个节点独立维护信息。' },
    { label: '记录进展', detail: '为任务或子任务追加进展说明，保留推进过程。' },
    { label: '日历安排', detail: '在日历中查看顶层任务的日期安排，展开明细查看子任务。' },
  ],
  knowledge: [
    { label: '同步来源', detail: '每本 Markdown 书籍建立独立知识库，原文保持只读。' },
    { label: '知识编译', detail: '跨章节整理主题、论断与关系，生成带引用的知识候选。' },
    { label: '人工审核', detail: '核对正文、引用与变更，批准后写入正式 Wiki。' },
    { label: '引用问答', detail: '围绕所选书籍连续提问，回答关联本轮读取的证据。' },
    { label: '来源预览', detail: '在原位查看引用片段与位置，核对回答依据。' },
    { label: '研究交付', detail: '分阶段形成研究报告；PPTX 生成后可下载使用。' },
  ],
}

export const scenarioOrder = Object.keys(scenarios)
const dwellTimes = {
  reader: [1600, 1800, 1800, 1300],
  timeline: [1600, 1800, 1400],
  tasks: [1400, 1800, 1800, 1600],
  knowledge: [1600, 1800, 1600, 2200, 2000, 1800],
}

export function createPlayback({ onChange = () => {}, setTimer = setTimeout, clearTimer = clearTimeout, now = () => performance.now(), reducedMotion = false } = {}) {
  let scenario = 'reader'
  let index = 0
  let playing = false
  let visible = false
  let foreground = true
  let begun = reducedMotion
  let disposed = false
  let timer = null
  let generation = 0
  let remaining = dwellTimes.reader[0]
  let startedAt = null

  function getState() {
    const elapsed = startedAt === null ? 0 : now() - startedAt
    return { scenario, index, playing, visible, foreground, running: playing && visible && foreground, count: scenarios[scenario].length, step: scenarios[scenario][index], duration: dwellTimes[scenario][index], remaining: Math.max(0, remaining - elapsed) }
  }

  function cancelTimer(preserve = true) {
    generation++
    if (preserve && startedAt !== null) remaining = Math.max(0, remaining - (now() - startedAt))
    startedAt = null
    if (timer !== null) clearTimer(timer)
    timer = null
  }

  function update() {
    if (disposed) return
    cancelTimer()
    onChange(getState())
    if (!playing || !visible || !foreground) return
    const current = generation
    startedAt = now()
    timer = setTimer(() => {
      if (disposed || current !== generation) return
      timer = null
      startedAt = null
      if (index < scenarios[scenario].length - 1) index++
      else { scenario = adjacentScenario(1); index = 0 }
      remaining = dwellTimes[scenario][index]
      update()
    }, remaining)
  }

  function maybeStart() {
    if (visible && foreground && !begun && !reducedMotion) {
      begun = true
      playing = true
    }
    update()
  }

  function seek(target) {
    if (disposed || !Number.isFinite(target)) return
    playing = false
    begun = true
    cancelTimer(false)
    index = Math.max(0, Math.min(Math.trunc(target), scenarios[scenario].length - 1))
    remaining = dwellTimes[scenario][index]
    update()
  }

  function adjacentScenario(direction) {
    return scenarioOrder[(scenarioOrder.indexOf(scenario) + direction + scenarioOrder.length) % scenarioOrder.length]
  }

  function navigate(direction) {
    if (disposed) return
    const target = index + direction
    if (target >= 0 && target < scenarios[scenario].length) { seek(target); return }
    cancelTimer(false)
    scenario = adjacentScenario(direction)
    seek(direction > 0 ? 0 : scenarios[scenario].length - 1)
  }

  return {
    getState,
    setScenario(value) {
      if (disposed || !Object.hasOwn(scenarios, value) || scenario === value) return
      cancelTimer(false)
      scenario = value
      index = 0
      remaining = dwellTimes[scenario][index]
      maybeStart()
    },
    setVisible(value) {
      if (disposed || visible === value) return
      visible = value
      maybeStart()
    },
    setForeground(value) {
      if (disposed || foreground === value) return
      foreground = value
      maybeStart()
    },
    setReducedMotion(value) {
      if (disposed) return
      reducedMotion = value
      if (value) { playing = false; begun = true }
      update()
    },
    seek,
    next: () => navigate(1),
    previous: () => navigate(-1),
    play() {
      if (disposed) return
      begun = true
      playing = true
      update()
    },
    pause() {
      if (disposed) return
      playing = false
      begun = true
      update()
    },
    dispose() {
      disposed = true
      cancelTimer()
    },
  }
}
