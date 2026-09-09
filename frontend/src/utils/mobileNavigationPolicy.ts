export type MobileNavSection = 'reader' | 'timeline' | 'tasks' | 'more'

export function getMobileNavSection(path: string): MobileNavSection {
  if (path === '/reader') return 'reader'
  if (path === '/timeline') return 'timeline'
  if (path === '/tasks') return 'tasks'
  return 'more'
}

export function isMobileFocusRoute(
  path: string,
  query: Record<string, unknown>,
): boolean {
  if (path === '/reader') return query.view === 'read'
  if (path === '/tasks') {
    return query.view !== 'calendar'
      && typeof query.task === 'string'
      && query.task.length > 0
  }
  return false
}
