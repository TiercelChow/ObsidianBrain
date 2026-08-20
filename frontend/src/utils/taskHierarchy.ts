export interface HierarchyTask {
  id: string
  parent_id: string | null
  role: 'root' | 'subtask'
  position: number
  created_at: string
}

export interface CalendarTask extends HierarchyTask {
  start_date: string
  end_date: string
}

export interface FlattenedTask<T> {
  node: T
  depth: number
  hasChildren: boolean
}

export interface CalendarAgendaEntry<T> {
  task: T
  depth: number
  hasChildren: boolean
}

function compareTasks<T extends HierarchyTask>(left: T, right: T) {
  return left.position - right.position || left.created_at.localeCompare(right.created_at)
}

function childMap<T extends HierarchyTask>(tasks: readonly T[]) {
  const children = new Map<string | null, T[]>()
  for (const task of tasks) {
    const siblings = children.get(task.parent_id) || []
    siblings.push(task)
    children.set(task.parent_id, siblings)
  }
  for (const siblings of children.values()) siblings.sort(compareTasks)
  return children
}

/** The breakdown panel starts with actionable subtasks, not the already-visible root. */
export function flattenVisibleSubtasks<T extends HierarchyTask>(
  tasks: readonly T[],
  expandedIds: ReadonlySet<string>,
): Array<FlattenedTask<T>> {
  const children = childMap(tasks)
  const rows: Array<FlattenedTask<T>> = []
  const visited = new Set<string>()

  function visit(node: T, depth: number) {
    if (visited.has(node.id) || node.role === 'root') return
    visited.add(node.id)
    const nested = children.get(node.id) || []
    rows.push({ node, depth, hasChildren: nested.length > 0 })
    if (expandedIds.has(node.id)) nested.forEach(child => visit(child, depth + 1))
  }

  const roots = tasks.filter(task => task.role === 'root').sort(compareTasks)
  roots.forEach(root => (children.get(root.id) || []).forEach(child => visit(child, 0)))
  tasks.filter(task => task.role === 'subtask' && !visited.has(task.id)).sort(compareTasks).forEach(task => visit(task, 0))
  return rows
}

export function calendarTopLevelTasks<T extends HierarchyTask>(tasks: readonly T[]): T[] {
  return tasks.filter(task => task.role === 'root')
}

/** Keep the agenda rooted by project, revealing dated descendants only on demand. */
export function calendarAgendaEntries<T extends CalendarTask>(
  tasks: readonly T[],
  date: string,
  expandedRootIds: ReadonlySet<string> = new Set(),
): Array<CalendarAgendaEntry<T>> {
  const activeTasks = tasks.filter(task => task.start_date <= date && date <= task.end_date)
  const children = childMap(activeTasks)
  const entries: Array<CalendarAgendaEntry<T>> = []
  const visited = new Set<string>()

  function append(task: T, depth: number) {
    if (visited.has(task.id)) return
    visited.add(task.id)
    const nested = children.get(task.id) || []
    entries.push({ task, depth, hasChildren: nested.length > 0 })
    nested.forEach(child => append(child, depth + 1))
  }

  calendarTopLevelTasks(activeTasks).sort(compareTasks).forEach((root) => {
    visited.add(root.id)
    const nested = children.get(root.id) || []
    entries.push({ task: root, depth: 0, hasChildren: nested.length > 0 })
    if (expandedRootIds.has(root.id)) nested.forEach(child => append(child, 1))
  })
  return entries
}
