/**
 * Shared bookshelf state (see docs/requirement/10-reader-bookshelf.md).
 *
 * Book METADATA (id/path/kind/name/category/addedAt) is server-stored via the
 * tool API (load/persist). Reading PROGRESS is per-file and lives in the
 * browser (localStorage) — a folder book can mix .md (scroll ratio) and .pdf
 * (page) files, so progress is stored as a per-file map keyed by book id; the
 * pure transforms are in utils/readerProgress.ts. `createBookshelf` takes
 * injectable load/persist/loadProgress/saveProgress for node tests; useBookshelf
 * is the app-wide singleton wired to the real tool API + localStorage. The API
 * is imported dynamically inside the wiring closures so this module's static
 * import graph stays alias-free — that keeps `node --test --experimental-strip-types`
 * able to load it directly.
 */
import { ref, type Ref } from 'vue'
import type { ReaderBook } from '@/api/reader'
// Relative + .ts so the static import graph stays alias-free — keeps
// `node --test --experimental-strip-types` able to load this module directly.
import {
  getDisplayProgress,
  mergeBookState,
  migrateFromLegacy,
  parseProgressMap,
  serializeProgressMap,
  setLastFile,
  type BookProgressState,
  type FileProgress,
} from '../utils/readerProgress.ts'

export interface BookshelfDeps {
  load: () => Promise<ReaderBook[]>
  persist: (books: ReaderBook[]) => Promise<void>
  loadProgress: () => Record<string, BookProgressState>
  saveProgress: (map: Record<string, BookProgressState>) => void
}

export interface Bookshelf {
  books: Ref<ReaderBook[]>
  loaded: Ref<boolean>
  loadError: Ref<string>
  ensureLoaded: () => Promise<void>
  addBook: (book: ReaderBook) => Promise<boolean>
  updateBook: (book: ReaderBook) => Promise<boolean>
  removeBook: (id: string) => Promise<boolean>
  /** Per-file progress write (localStorage only; never hits the backend). */
  saveFileProgress: (id: string, file: string, progress: FileProgress) => void
  /** Set the lastFile pointer, seeding a fresh entry only if the file is new. */
  openFile: (id: string, file: string, kind: 'md' | 'pdf') => void
  /** Full per-file state for restore (null if none). */
  getFileProgressState: (id: string) => BookProgressState | null
  findBook: (path: string) => ReaderBook | undefined
}

export function createBookshelf(deps: BookshelfDeps): Bookshelf {
  const books = ref<ReaderBook[]>([])
  const loaded = ref(false)
  const loadError = ref('')
  let progressMap: Record<string, BookProgressState> = {}

  async function ensureLoaded() {
    if (loaded.value) return
    try {
      const list = await deps.load()
      progressMap = deps.loadProgress()
      // Attach display progress; one-time-migrate legacy backend progress into
      // localStorage where the local map has no entry for that book.
      let migrated = false
      for (const b of list) {
        if (!progressMap[b.id] && b.progress) {
          const m = migrateFromLegacy(b.progress, b.path, b.kind)
          if (m) {
            progressMap[b.id] = m
            migrated = true
          }
        }
        b.progress = progressMap[b.id] ? getDisplayProgress(progressMap[b.id]) ?? undefined : undefined
      }
      if (migrated) deps.saveProgress(progressMap)
      books.value = list
      loaded.value = true
      loadError.value = ''
    } catch (e) {
      loadError.value = (e as Error)?.message || '书架加载失败'
    }
  }

  /** Strict CRUD: optimistic update, rollback + false on persist failure (FR-11). */
  async function mutate(next: ReaderBook[]): Promise<boolean> {
    const prev = books.value
    books.value = next
    try {
      // Backend stores metadata only; progress lives in localStorage. Strip it
      // so a stale server blob can never overwrite the local source of truth.
      const meta = next.map((b) => ({ ...b, progress: undefined }))
      await deps.persist(meta)
      return true
    } catch (e) {
      books.value = prev
      console.warn('书架保存失败:', e)
      return false
    }
  }

  function addBook(book: ReaderBook) {
    return mutate([...books.value, book])
  }

  function updateBook(book: ReaderBook) {
    return mutate(books.value.map((b) => (b.id === book.id ? book : b)))
  }

  function removeBook(id: string) {
    return mutate(books.value.filter((b) => b.id !== id))
  }

  function refreshDisplay(id: string) {
    const display = progressMap[id] ? getDisplayProgress(progressMap[id]) : null
    books.value = books.value.map((b) =>
      b.id === id ? { ...b, progress: display ?? undefined } : b,
    )
  }

  /** Per-file progress write — localStorage only, fire-and-forget. */
  function saveFileProgress(id: string, file: string, progress: FileProgress) {
    if (!books.value.some((b) => b.id === id)) return
    progressMap[id] = mergeBookState(progressMap[id] ?? null, file, progress)
    deps.saveProgress(progressMap)
    refreshDisplay(id)
  }

  /** Set the lastFile pointer (seeds a fresh entry only for a brand-new file). */
  function openFile(id: string, file: string, kind: 'md' | 'pdf') {
    if (!books.value.some((b) => b.id === id)) return
    progressMap[id] = setLastFile(progressMap[id] ?? null, file, kind, Date.now())
    deps.saveProgress(progressMap)
    refreshDisplay(id)
  }

  function getFileProgressState(id: string): BookProgressState | null {
    return progressMap[id] ?? null
  }

  function findBook(path: string) {
    return books.value.find((b) => b.path === path)
  }

  return {
    books,
    loaded,
    loadError,
    ensureLoaded,
    addBook,
    updateBook,
    removeBook,
    saveFileProgress,
    openFile,
    getFileProgressState,
    findBook,
  }
}

// ── app-wide singleton ────────────────────────────────────────────────

const PROGRESS_KEY = 'obsidian-brain:reader-progress'

let singleton: Bookshelf | null = null

export function useBookshelf(): Bookshelf {
  singleton ??= createBookshelf({
    load: async () => {
      const { getReaderBooks } = await import('@/api/reader')
      const res = await getReaderBooks()
      if (res.status !== 'success' || !res.result) {
        throw new Error(res.error?.message || '书架加载失败')
      }
      return res.result.books
    },
    persist: async (list) => {
      const { saveReaderBooks } = await import('@/api/reader')
      const res = await saveReaderBooks(list)
      if (res.status !== 'success') {
        throw new Error(res.error?.message || '书架保存失败')
      }
    },
    loadProgress: () => parseProgressMap(localStorage.getItem(PROGRESS_KEY)),
    saveProgress: (map) => {
      try {
        localStorage.setItem(PROGRESS_KEY, serializeProgressMap(map))
      } catch (e) {
        console.warn('进度本地存储失败:', e)
      }
    },
  })
  return singleton
}
