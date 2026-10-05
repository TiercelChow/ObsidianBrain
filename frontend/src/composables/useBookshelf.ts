/**
 * Shared bookshelf state (see docs/requirement/10-reader-bookshelf.md).
 *
 * Book METADATA (id/path/kind/name/category/addedAt) is server-stored via the
 * tool API (load/persist). Reading PROGRESS is per-file and server-stored with
 * a browser recovery cache — a folder book can mix .md (scroll ratio) and .pdf
 * (page) files, so progress is stored as a per-file map keyed by book id; the
 * pure transforms are in utils/readerProgress.ts. `createBookshelf` takes
 * injectable metadata, progress and cache adapters for node tests; useBookshelf
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
  mergeProgressStates,
  readTime,
  readingFileBelongs,
  type BookProgressState,
  type FileProgress,
} from '../utils/readerProgress.ts'

export interface BookshelfDeps {
  load: () => Promise<ReaderBook[]>
  persist: (books: ReaderBook[]) => Promise<void>
  loadProgress: () => Record<string, BookProgressState>
  saveProgress: (map: Record<string, BookProgressState>) => void
  persistProgress: (id: string, state: BookProgressState) => Promise<BookProgressState>
  onProgressSaved?: () => void
}

export interface Bookshelf {
  books: Ref<ReaderBook[]>
  loaded: Ref<boolean>
  loadError: Ref<string>
  progressError: Ref<string>
  ensureLoaded: (refresh?: boolean) => Promise<void>
  addBook: (book: ReaderBook) => Promise<boolean>
  updateBook: (book: ReaderBook) => Promise<boolean>
  removeBook: (id: string) => Promise<boolean>
  /** Synchronous recovery cache plus coalesced single-book server patch. */
  saveFileProgress: (id: string, file: string, progress: FileProgress) => void
  /** Set the lastFile pointer, seeding a fresh entry only if the file is new. */
  openFile: (id: string, file: string, kind: 'md' | 'pdf') => void
  /** Full per-file state for restore (null if none). */
  getFileProgressState: (id: string) => BookProgressState | null
  findBook: (path: string) => ReaderBook | undefined
  flushProgress: () => Promise<void>
}

export function createBookshelf(deps: BookshelfDeps): Bookshelf {
  const books = ref<ReaderBook[]>([])
  const loaded = ref(false)
  const loadError = ref('')
  const progressError = ref('')
  let progressMap: Record<string, BookProgressState> = {}
  const pending = new Map<string, BookProgressState>()
  let writing: Promise<void> | null = null
  let loading: Promise<void> | null = null

  function queue(id: string, state: BookProgressState, full = false) {
    const patch = {
      ...state,
      lastReadAt: readTime(state),
      byFile: full ? { ...state.byFile } : { [state.lastFile]: state.byFile[state.lastFile] },
    }
    pending.set(id, mergeProgressStates(pending.get(id) || null, patch)!)
    void flushProgress()
  }

  async function flushProgress(): Promise<void> {
    if (writing) return writing
    if (!pending.size) return
    writing = (async () => {
      while (pending.size) {
        const [id, state] = pending.entries().next().value!
        pending.delete(id)
        if (!books.value.some(book => book.id === id)) continue
        try {
          const saved = await deps.persistProgress(id, state)
          progressMap[id] = mergeProgressStates(saved, progressMap[id] || null)!
          deps.saveProgress(progressMap)
          refreshDisplay(id)
          progressError.value = ''
          deps.onProgressSaved?.()
        } catch {
          // Newer interactions during the request are not lost by a failed patch.
          pending.set(id, mergeProgressStates(state, pending.get(id) || null)!)
          progressError.value = '阅读进度暂未同步，已保留在本机。'
          break
        }
      }
    })()
    try {
      await writing
    } finally {
      writing = null
    }
  }

  async function ensureLoaded(refresh = false) {
    if (loaded.value && !refresh) return
    if (loading) return loading
    loading = (async () => {
      try {
        const list = await deps.load()
        const cache = deps.loadProgress()
        progressMap = {}
        const imports: [string, BookProgressState][] = []
        for (const b of list) {
          const old = b.progress
          const server = old?.byFile && old.lastFile
            ? { lastFile: old.lastFile, lastReadAt: old.lastReadAt || old.updatedAt, byFile: old.byFile }
            : migrateFromLegacy(old || null, b.path, b.kind)
          const cached = cache[b.id]
          // An edited book path must not import unrelated old-file positions.
          const localFiles = Object.fromEntries(
            Object.entries(cached?.byFile || {}).filter(([file, value]) =>
              readingFileBelongs(b, file) && Number.isFinite(value?.position) && value.updatedAt > 0,
            ),
          )
          const local = cached && localFiles[cached.lastFile] ? { ...cached, byFile: localFiles } : null
          const merged = mergeProgressStates(server, local)
          if (merged) {
            progressMap[b.id] = merged
            if (local && JSON.stringify(merged) !== JSON.stringify(mergeProgressStates(server, null))) {
              imports.push([b.id, merged])
            }
          }
          b.progress = progressMap[b.id] ? getDisplayProgress(progressMap[b.id]) ?? undefined : undefined
        }
        deps.saveProgress(progressMap)
        books.value = list
        loaded.value = true
        loadError.value = ''
        for (const [id, state] of imports) queue(id, state, true)
      } catch (e) {
        loadError.value = (e as Error)?.message || '书架加载失败'
      }
    })()
    try {
      await loading
    } finally {
      loading = null
    }
  }

  /** Strict CRUD: optimistic update, rollback + false on persist failure (FR-11). */
  async function mutate(next: ReaderBook[]): Promise<boolean> {
    const prev = books.value
    books.value = next
    try {
      // Reading updates have a dedicated atomic API. Metadata never overwrites them.
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

  /** Cache immediately; queue only this file, not the complete shelf or book map. */
  function saveFileProgress(id: string, file: string, progress: FileProgress) {
    if (!books.value.some((b) => b.id === id)) return
    progressMap[id] = mergeBookState(progressMap[id] ?? null, file, progress)
    deps.saveProgress(progressMap)
    refreshDisplay(id)
    queue(id, progressMap[id])
  }

  /** Set the lastFile pointer (seeds a fresh entry only for a brand-new file). */
  function openFile(id: string, file: string, kind: 'md' | 'pdf') {
    if (!books.value.some((b) => b.id === id)) return
    progressMap[id] = setLastFile(progressMap[id] ?? null, file, kind, Date.now())
    deps.saveProgress(progressMap)
    refreshDisplay(id)
    queue(id, progressMap[id])
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
    progressError,
    ensureLoaded,
    addBook,
    updateBook,
    removeBook,
    saveFileProgress,
    openFile,
    getFileProgressState,
    findBook,
    flushProgress,
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
        const raw = localStorage.getItem(PROGRESS_KEY)
        const backupKey = `${PROGRESS_KEY}:legacy-backup`
        if (raw && !localStorage.getItem(backupKey)) localStorage.setItem(backupKey, raw)
        localStorage.setItem(PROGRESS_KEY, serializeProgressMap(map))
      } catch (e) {
        console.warn('进度本地存储失败:', e)
      }
    },
    persistProgress: async (id, state) => {
      const { saveReaderProgress } = await import('@/api/reader')
      const result = await saveReaderProgress(id, state)
      if (result.status !== 'success' || !result.result) {
        throw new Error(result.error?.message || '阅读进度保存失败')
      }
      return result.result.state
    },
    onProgressSaved: () => window.dispatchEvent(new Event('reader-progress-saved')),
  })
  return singleton
}
