/**
 * Per-file, type-aware reader progress stored in localStorage.
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
}

/** Display shape fed to the bookshelf cover bar (bookProgressRatio / bookProgressLabel). */
export interface DisplayProgress {
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
  filePath: string,
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
  filePath: string,
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
  return { lastFile: file, byFile }
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
  return { lastFile: file, byFile }
}

/** Derive the single display progress (for the cover bar) from the last-opened file. */
export function getDisplayProgress(state: BookProgressState | null): DisplayProgress | null {
  if (!state || !state.byFile[state.lastFile]) return null
  const fp = state.byFile[state.lastFile]
  return {
    position: fp.position,
    ...(fp.pageCount ? { pageCount: fp.pageCount } : {}),
    updatedAt: fp.updatedAt,
  }
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
