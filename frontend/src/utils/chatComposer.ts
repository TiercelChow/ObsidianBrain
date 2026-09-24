type ComposerKeyEvent = Pick<KeyboardEvent,
  'key' | 'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey' | 'isComposing' | 'keyCode'>

const IME_COMMIT_GUARD_MS = 120

export function shouldSendComposerOnEnter(
  event: ComposerKeyEvent,
  isComposing: boolean,
  millisecondsSinceCompositionEnd: number,
): boolean {
  return event.key === 'Enter'
    && !event.shiftKey
    && !event.ctrlKey
    && !event.altKey
    && !event.metaKey
    && !event.isComposing
    && !isComposing
    // Some input methods report the confirmation key as Enter after compositionend.
    && event.keyCode !== 229
    && millisecondsSinceCompositionEnd > IME_COMMIT_GUARD_MS
}
