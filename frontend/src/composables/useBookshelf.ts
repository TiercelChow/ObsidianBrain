/**
 * Shared bookshelf state (see docs/requirement/10-reader-bookshelf.md).
 *
 * createBookshelf takes injectable load/persist for node tests; useBookshelf
 * is the app-wide singleton wired to the real tool API, shared by Reader.vue
 * and BookshelfView.vue. The API is imported dynamically inside the wiring
 * closures so this module's static import graph stays alias-free — that keeps
 * `node --test --experimental-strip-types` able to load it directly.
 */
import { ref, type Ref } from 'vue'
import type { BookProgress, ReaderBook } from '@/api/reader'

export interface BookshelfDeps {
  load: () => Promise<ReaderBook[]>
  persist: (books: ReaderBook[]) => Promise<void>
}

export interface Bookshelf {
  books: Ref<ReaderBook[]>
  loaded: Ref<boolean>
  loadError: Ref<string>
  ensureLoaded: () => Promise<void>
  addBook: (book: ReaderBook) => Promise<boolean>
  updateBook: (book: ReaderBook) => Promise<boolean>
  removeBook: (id: string) => Promise<boolean>
  updateProgress: (id: string, patch: Partial<BookProgress>) => void
  findBook: (path: string) => ReaderBook | undefined
}

export function createBookshelf(deps: BookshelfDeps): Bookshelf {
  const books = ref<ReaderBook[]>([])
  const loaded = ref(false)
  const loadError = ref('')

  async function ensureLoaded() {
    if (loaded.value) return
    try {
      const list = await deps.load()
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
      await deps.persist(next)
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

  /** Progress: optimistic + fire-and-forget. Never rolls back or throws. */
  function updateProgress(id: string, patch: Partial<BookProgress>) {
    const idx = books.value.findIndex((b) => b.id === id)
    if (idx < 0) return
    const book = books.value[idx]
    const next: ReaderBook = {
      ...book,
      progress: {
        lastFile: null,
        position: 0,
        ...(book.progress ?? {}),
        ...patch,
        updatedAt: Date.now(),
      },
    }
    books.value = books.value.map((b, i) => (i === idx ? next : b))
    void deps.persist(books.value).catch((e) => console.warn('进度保存失败:', e))
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
    updateProgress,
    findBook,
  }
}

// ── app-wide singleton ────────────────────────────────────────────────

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
  })
  return singleton
}
