/**
 * Pure helpers for the path picker (see PathPickerModal.vue).
 * No DOM / network — runtime imports would break node --test, so this
 * module only uses type imports.
 */
import type { DirEntry } from '@/api/reader'

interface PathSegment {
  name: string
  path: string
}

function appendSegments(root: PathSegment, rest: string, sep: '/' | '\\'): PathSegment[] {
  const segments = [root]
  let path = root.path
  for (const name of rest.split(sep).filter(Boolean)) {
    path = (path.endsWith(sep) ? path : path + sep) + name
    segments.push({ name, path })
  }
  return segments
}

function parseAbsolutePath(path: string): PathSegment[] | null {
  const windows = path.replace(/\//g, '\\')
  const drive = /^(\\\\\?\\)?([a-z]:)(?:\\+(.*)|$)/is.exec(windows)
  if (drive) {
    const name = drive[2].toUpperCase()
    return appendSegments({ name, path: (drive[1] ?? '') + name + '\\' }, drive[3] ?? '', '\\')
  }
  if (windows.startsWith('\\\\')) {
    const share = /^(\\\\(?:\?\\UNC\\)?)([^\\]+)\\+([^\\]+)(?:\\+(.*))?$/is.exec(windows)
    if (share && !['?', '.', '..'].includes(share[2]) && !['.', '..'].includes(share[3])) {
      const name = '\\\\' + share[2] + '\\' + share[3]
      return appendSegments({ name, path: share[1] + name.slice(2) }, share[4] ?? '', '\\')
    }
    if (path.startsWith('\\')) return null
  }
  return path.startsWith('/') ? appendSegments({ name: '/', path: '/' }, path, '/') : null
}

/** Explorer's “Copy as path” wraps paths in quotes. Bare drive letters
 * are interpreted as roots, never the process's per-drive working directory. */
export function normalizePickerPath(input: string): string | null {
  const path = input.trim().replace(/^"([\s\S]*)"$/, '$1')
  const segments = parseAbsolutePath(path)
  return segments ? segments[segments.length - 1].path : null
}

/** Root paths are their own parent, including Windows network shares. */
export function parentPath(path: string): string {
  const segments = pathSegments(path)
  return segments[Math.max(0, segments.length - 2)].path
}

/** Breadcrumb segments from a real filesystem root to the current directory. */
export function pathSegments(path: string): PathSegment[] {
  return parseAbsolutePath(path) ?? [{ name: '/', path: '/' }]
}

export function localPathName(path: string): string {
  const normalized = normalizePickerPath(path)
  if (!normalized || normalized === '/') return ''
  const parts = normalized.split(normalized.startsWith('/') ? '/' : '\\').filter(Boolean)
  return parts[parts.length - 1] ?? ''
}

/**
 * Entries visible in the picker: directories, PDFs and EPUBs. Markdown/text
 * files are grouped in folder books. Directories first, then files A→Z.
 */
export function pickableEntries(entries: DirEntry[]): DirEntry[] {
  const dirs = entries.filter((e) => e.is_dir)
  const books = entries.filter((e) => !e.is_dir && (e.is_pdf || e.is_epub))
  const byName = (a: DirEntry, b: DirEntry) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
  return [...dirs.sort(byName), ...books.sort(byName)]
}
