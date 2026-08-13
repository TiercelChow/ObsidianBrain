import { onBeforeUnmount, shallowRef, type Ref } from 'vue'

export function useUndoableRemoval<T>(
  items: Ref<T[]>,
  commit: (item: T) => Promise<unknown>,
  onError: () => void,
  delayMs = 5000,
) {
  const pending = shallowRef<{ item: T; index: number; timer: number } | null>(null)

  function restore(item: T, index: number) {
    const list = items.value as T[]
    if (list.includes(item)) return
    list.splice(Math.min(index, list.length), 0, item)
  }

  function commitPending() {
    const current = pending.value
    if (!current) return
    window.clearTimeout(current.timer)
    pending.value = null
    void commit(current.item).catch(() => {
      restore(current.item, current.index)
      onError()
    })
  }

  function remove(item: T) {
    commitPending()
    const list = items.value as T[]
    const index = list.indexOf(item)
    if (index < 0) return
    list.splice(index, 1)
    const timer = window.setTimeout(commitPending, delayMs)
    pending.value = { item, index, timer }
  }

  function undo() {
    const current = pending.value
    if (!current) return
    window.clearTimeout(current.timer)
    pending.value = null
    restore(current.item, current.index)
  }

  onBeforeUnmount(commitPending)
  return { pending, remove, undo, commitPending }
}
