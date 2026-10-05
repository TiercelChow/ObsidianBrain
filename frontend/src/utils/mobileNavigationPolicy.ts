import { isPhoneViewport } from './mobileLayoutPolicy.ts'

export type MobileNavSection = 'home' | 'reader' | 'timeline' | 'tasks' | 'wiki'

export function getMobileNavSection(path: string): MobileNavSection {
  if (path === '/reader') return 'reader'
  if (path === '/timeline') return 'timeline'
  if (path === '/tasks') return 'tasks'
  if (path === '/knowledge' || path.startsWith('/knowledge/')) return 'wiki'
  return 'home'
}

/** Desktop-only work must not appear through phone bookmarks or a resized tab. */
export function getMobileRouteRedirect(path: string, width: number): string | null {
  return path === '/code-repo' && isPhoneViewport(width) ? '/' : null
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
