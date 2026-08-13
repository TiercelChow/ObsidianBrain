import { nextTick, onBeforeUnmount, onMounted, watch, type Ref } from 'vue'

const focusableSelector = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const modalStack: symbol[] = []
let scrollLockCount = 0
let savedBodyOverflow = ''
let savedBodyPaddingRight = ''

function lockPageScroll() {
  scrollLockCount += 1
  if (scrollLockCount !== 1) return
  savedBodyOverflow = document.body.style.overflow
  savedBodyPaddingRight = document.body.style.paddingRight
  const scrollbarWidth = Math.max(0, window.innerWidth - document.documentElement.clientWidth)
  document.body.style.overflow = 'hidden'
  if (scrollbarWidth > 0) document.body.style.paddingRight = `${scrollbarWidth}px`
}

function unlockPageScroll() {
  scrollLockCount = Math.max(0, scrollLockCount - 1)
  if (scrollLockCount !== 0) return
  document.body.style.overflow = savedBodyOverflow
  document.body.style.paddingRight = savedBodyPaddingRight
}

/** Shared dialog behavior: scroll lock, focus trap, Escape, and focus return. */
export function useModalEnvironment(
  isOpen: () => boolean,
  panelRef: Ref<HTMLElement | null>,
  close: () => void,
) {
  const token = Symbol('modal')
  let previousFocus: HTMLElement | null = null
  let active = false

  function isTopmost() {
    return modalStack[modalStack.length - 1] === token
  }

  function activate() {
    if (active) return
    active = true
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    modalStack.push(token)
    lockPageScroll()
    nextTick(() => {
      const panel = panelRef.value
      if (!panel || panel.contains(document.activeElement)) return
      panel.focus({ preventScroll: true })
    })
  }

  function deactivate(restoreFocus = true) {
    if (!active) return
    active = false
    const index = modalStack.lastIndexOf(token)
    if (index >= 0) modalStack.splice(index, 1)
    unlockPageScroll()
    if (restoreFocus) {
      const target = previousFocus
      nextTick(() => {
        if (target?.isConnected) target.focus({ preventScroll: true })
      })
    }
    previousFocus = null
  }

  function onKeydown(event: KeyboardEvent) {
    if (!active || !isTopmost()) return
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
      return
    }
    if (event.key !== 'Tab' || !panelRef.value) return

    const focusable = Array.from(
      panelRef.value.querySelectorAll<HTMLElement>(focusableSelector),
    ).filter((element) => !element.hasAttribute('hidden') && element.offsetParent !== null)

    if (focusable.length === 0) {
      event.preventDefault()
      panelRef.value.focus({ preventScroll: true })
      return
    }

    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  watch(isOpen, (open) => {
    if (open) activate()
    else deactivate()
  }, { immediate: true })

  onMounted(() => document.addEventListener('keydown', onKeydown))
  onBeforeUnmount(() => {
    document.removeEventListener('keydown', onKeydown)
    deactivate(false)
  })
}
