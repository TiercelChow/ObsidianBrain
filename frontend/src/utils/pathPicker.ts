/**
 * Pure helpers for the path picker (see PathPickerModal.vue).
 * No DOM / network — runtime imports would break node --test, so this
 * module only uses type imports.
 */
import type { DirEntry } from '@/api/reader'

/** Parent directory of an absolute path. Root "/" is its own parent. */
export function parentPath(path: string): string {
  if (!path || path === '/') return '/'
  const parts = path.replace(/\/+$/, '').split('/').filter(Boolean)
  if (parts.length <= 1) return '/'
  return '/' + parts.slice(0, -1).join('/')
}

/** Breadcrumb segments from root to the path, each with its absolute path. */
export function pathSegments(path: string): { name: string; path: string }[] {
  const parts = (path || '/').replace(/\/+$/, '').split('/').filter(Boolean)
  const segs: { name: string; path: string }[] = [{ name: '/', path: '/' }]
  let acc = ''
  for (const p of parts) {
    acc += '/' + p
    segs.push({ name: p, path: acc })
  }
  return segs
}

/**
 * Entries visible in the picker: directories + PDFs only (markdown/text
 * files are not bookable as single files — only folders and PDFs are).
 * Directories first, then PDFs, both case-insensitive A→Z.
 */
export function pickableEntries(entries: DirEntry[]): DirEntry[] {
  const dirs = entries.filter((e) => e.is_dir)
  const pdfs = entries.filter((e) => !e.is_dir && e.is_pdf)
  const byName = (a: DirEntry, b: DirEntry) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
  return [...dirs.sort(byName), ...pdfs.sort(byName)]
}
