/**
 * Reader bookshelf pure helpers (see docs/requirement/10-reader-bookshelf.md).
 * No DOM / network here — runtime imports would break node --test, so this
 * module only uses type imports.
 */
import type { ReaderBook } from '@/api/reader'

export function deriveKind(path: string): 'folder' | 'pdf' {
  return /\.pdf$/i.test(path) ? 'pdf' : 'folder'
}

export function defaultBookName(path: string): string {
  return path.split('/').filter(Boolean).pop() ?? ''
}

export function makeBookId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`
}

/** Scroll position as a 0..1 ratio of the scrollable range; 0 when nothing to scroll. */
export function scrollRatio(scrollTop: number, scrollHeight: number, clientHeight: number): number {
  const denom = scrollHeight - clientHeight
  if (denom <= 0) return 0
  return Math.min(1, Math.max(0, Math.round((scrollTop / denom) * 10000) / 10000))
}

/** Valid pdf page in [1, pageCount]; unknown pageCount (0) only floors to ≥1. */
export function clampPdfPage(page: number, pageCount: number): number {
  const p = Math.max(1, Math.floor(page))
  return pageCount > 0 ? Math.min(p, pageCount) : p
}

export function bookProgressLabel(book: ReaderBook): string {
  const p = book.progress
  if (!p) return '未开始'
  if (book.kind === 'pdf') {
    return p.pageCount && p.pageCount > 0
      ? `第 ${Math.floor(p.position)}/${p.pageCount} 页`
      : `第 ${Math.floor(p.position)} 页`
  }
  if (p.position <= 0) return '未开始'
  return `读到 ${Math.round(p.position * 100)}%`
}

/** Most recently read (or added) first. Returns a new array. */
export function sortBooks(books: ReaderBook[]): ReaderBook[] {
  return [...books].sort((a, b) => (b.progress?.updatedAt ?? b.addedAt) - (a.progress?.updatedAt ?? a.addedAt))
}

export function findBookByPath(books: ReaderBook[], path: string): ReaderBook | undefined {
  return books.find((b) => b.path === path)
}

/**
 * Form validation for add/edit (FR-7/FR-8). `stat` is the stat_local_path
 * result (null while pending → no path-kind errors, caller gates submit).
 * `editingId` exempts the book being edited from the duplicate check.
 * Returns an error message or null when valid.
 */
export function validateBookForm(
  path: string,
  editingId: string | null,
  books: ReaderBook[],
  stat: { exists: boolean; is_dir: boolean } | null,
): string | null {
  if (!path.trim()) return '请输入路径'
  const duplicate = books.find((b) => b.path === path.trim() && b.id !== editingId)
  if (duplicate) return '该书已在书架'
  if (stat) {
    if (!stat.exists) return '路径不存在'
    if (!(stat.is_dir || /\.pdf$/i.test(path))) return '仅支持文件夹或 PDF 文件'
  }
  return null
}
