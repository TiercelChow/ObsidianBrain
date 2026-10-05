/**
 * Per-file, type-aware reading state. SQLite is authoritative; localStorage
 * retains a recovery cache for offline/closing-tab writes.
 *
 * Why per-file: a folder book can contain both .md and .pdf files. md files
 * record a 0..1 scroll ratio; pdf files record a 1-based page number. Storing
 * a single `position` per book (the old backend shape) made the file type
 * ambiguous and broke folders containing PDFs — a page number got applied as
 * a scroll ratio. Each file gets its own entry here, carrying its `kind` so the
 * position's meaning is unambiguous.
 *
 * Pure transforms (no localStorage I/O) so this is unit-tested with node:test;
 * the app-wide bookshelf (composables/useBookshelf.ts) owns the actual
 * localStorage read/write.
 *
 * See docs/requirement/10-reader-bookshelf.md (FR-12..16).
 */
import { scrollRatio } from './readerBooks.ts'

export type FileKind = 'md' | 'pdf'

/** Progress for a single file within a book. */
export interface FileProgress {
  /** Disambiguates what `position` means: md → 0..1 scroll ratio; pdf → 1-based page. */
  kind: FileKind
  position: number
  /** pdf only — total pages, for clamping on restore + the "第 X/Y 页" label. */
  pageCount?: number
  updatedAt: number
}

/** Per-book state: which file to reopen + a map of every file's progress. */
export interface BookProgressState {
  /** Which file to reopen when the book is opened (folder: the last file; single-pdf: book.path). */
  lastFile: string
  byFile: Record<string, FileProgress>
  /** Last successful open/read, independent from the position's timestamp. */
  lastReadAt?: number
}

/** Display shape fed to the bookshelf cover bar (bookProgressRatio / bookProgressLabel). */
export interface DisplayProgress {
  lastFile?: string
  position: number
  pageCount?: number
  updatedAt: number
}

/** A file's kind from its path: .pdf (any case) → pdf, everything else → md. */
export function deriveFileKind(filePath: string): FileKind {
  return /\.pdf$/i.test(filePath) ? 'pdf' : 'md'
}

/** Capture an md file's scroll position as a 0..1 ratio. */
export function captureMdFileProgress(
  _filePath: string,
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  updatedAt: number,
): FileProgress {
  return {
    kind: 'md',
    position: scrollRatio(scrollTop, scrollHeight, clientHeight),
    updatedAt,
  }
}

/** Capture a pdf file's current page (+ pageCount for restore/display). */
export function capturePdfFileProgress(
  _filePath: string,
  page: number,
  pageCount: number | undefined,
  updatedAt: number,
): FileProgress {
  return { kind: 'pdf', position: page, ...(pageCount ? { pageCount } : {}), updatedAt }
}

/**
 * Move the `lastFile` pointer to `file`, seeding a fresh entry (md → ratio 0,
 * pdf → page 1) ONLY if the file has no saved progress yet. Opening a file
 * that already has progress must not clobber its position — the restore owns
 * that. Returns a new state (immutable).
 */
export function setLastFile(
  state: BookProgressState | null,
  file: string,
  kind: FileKind,
  updatedAt: number,
): BookProgressState {
  const byFile = { ...(state?.byFile ?? {}) }
  if (!byFile[file]) {
    byFile[file] = { kind, position: kind === 'pdf' ? 1 : 0, updatedAt }
  }
  return { lastFile: file, byFile, lastReadAt: Math.max(readTime(state), updatedAt) }
}

/**
 * Upsert a file's progress into a book's state, setting `lastFile` to it.
 * Never loses other files' progress. Returns a new state (immutable).
 */
export function mergeBookState(
  prev: BookProgressState | null,
  file: string,
  progress: FileProgress,
): BookProgressState {
  const byFile = { ...(prev?.byFile ?? {}), [file]: progress }
  return { lastFile: file, byFile, lastReadAt: Math.max(readTime(prev), progress.updatedAt) }
}

/** Derive the single display progress (for the cover bar) from the last-opened file. */
export function getDisplayProgress(state: BookProgressState | null): DisplayProgress | null {
  if (!state || !state.byFile[state.lastFile]) return null
  const fp = state.byFile[state.lastFile]
  return {
    lastFile: state.lastFile,
    position: fp.position,
    ...(fp.pageCount ? { pageCount: fp.pageCount } : {}),
    updatedAt: readTime(state),
  }
}

export function readTime(state: BookProgressState | null): number {
  if (!state) return 0
  return Object.values(state.byFile).reduce((time,file) => Math.max(time,file.updatedAt || 0), state.lastReadAt || 0)
}

/** Merge by file clocks, not by whichever network response arrived last. */
export function mergeProgressStates(server: BookProgressState | null, local: BookProgressState | null): BookProgressState | null {
  if (!server && !local) return null
  const byFile = { ...(server?.byFile || {}) }
  for (const [file, value] of Object.entries(local?.byFile || {})) {
    if (!byFile[file] || value.updatedAt >= byFile[file].updatedAt) byFile[file] = value
  }
  const last = readTime(local) >= readTime(server) ? local || server : server || local
  return last ? { lastFile: last.lastFile, byFile, lastReadAt: Math.max(readTime(server), readTime(local)) } : null
}

function normalizeReadingPath(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '')
  return /^[a-z]:/i.test(normalized) ? normalized.toLowerCase() : normalized
}
export function readingFileBelongs(book: {path:string;kind:'folder'|'pdf'}, file: string): boolean {
  const path = normalizeReadingPath(file), root = normalizeReadingPath(book.path)
  if (path.split('/').some(part => part === '..' || part === '.')) return false
  return book.kind === 'pdf' ? path === root : path.startsWith(`${root}/`)
}
export function resolveReadingBookId(books: {id:string;path:string;kind:'folder'|'pdf'}[], root: string, file: string, preferred?: string | null): string | null {
  if (!file) return null
  const selected = books.find(book => book.id === preferred && readingFileBelongs(book, file))
  if (selected) return selected.id
  const pdf = books.find(book => book.kind === 'pdf' && readingFileBelongs(book, file))
  return pdf?.id ?? books.find(book => book.kind === 'folder' && normalizeReadingPath(book.path) === normalizeReadingPath(root) && readingFileBelongs(book, file))?.id ?? null
}

/**
 * One-time migration seed from the legacy single-position backend progress.
 * Folder books had a md lastFile (pdf-in-folder was never saved correctly);
 * single-pdf books stored page + pageCount keyed by the book's own path.
 * Returns null when there's nothing worth migrating (fresh start).
 */
export function migrateFromLegacy(
  legacy: { lastFile?: string | null; position: number; pageCount?: number | null; updatedAt: number } | null,
  bookPath: string,
  bookKind: 'folder' | 'pdf',
): BookProgressState | null {
  if (!legacy) return null
  // Nothing meaningful (position 0 with no lastFile = never started).
  if (!legacy.lastFile && legacy.position === 0) return null

  if (bookKind === 'pdf') {
    const file = bookPath
    return {
      lastFile: file,
      byFile: {
        [file]: {
          kind: 'pdf',
          position: legacy.position,
          ...(legacy.pageCount ? { pageCount: legacy.pageCount } : {}),
          updatedAt: legacy.updatedAt,
        },
      },
    }
  }

  // Folder book: lastFile (when present) is the file to reopen; its kind is
  // derived from the extension. If lastFile is missing, fall back to the
  // first file we can't know — leave nothing to migrate.
  if (!legacy.lastFile) return null
  const file = legacy.lastFile
  return {
    lastFile: file,
    byFile: {
      [file]: {
        kind: deriveFileKind(file),
        position: legacy.position,
        ...(legacy.pageCount ? { pageCount: legacy.pageCount } : {}),
        updatedAt: legacy.updatedAt,
      },
    },
  }
}

/** Serialize the whole bookId → state map to a localStorage string. */
export function serializeProgressMap(map: Record<string, BookProgressState>): string {
  return JSON.stringify(map)
}

/** Parse a localStorage string into the bookId → state map; empty map on any error. */
export function parseProgressMap(raw: string | null): Record<string, BookProgressState> {
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    return parsed as Record<string, BookProgressState>
  } catch {
    return {}
  }
}
