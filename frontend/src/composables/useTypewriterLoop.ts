import { onBeforeUnmount, ref, watch, type Ref } from 'vue'

/**
 * Drives a restrained, looping typewriter status. Reduced-motion users get a
 * stable label instead of a continuously changing animation.
 */
export function useTypewriterLoop(active: Ref<boolean>, phrases: () => string[]) {
  const text = ref('')
  let timer = 0
  let phraseIndex = 0
  let characterIndex = 0

  function prefersReducedMotion() {
    return typeof window !== 'undefined'
      && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  }

  function stop() {
    window.clearTimeout(timer)
    timer = 0
  }

  function typeNext() {
    if (!active.value) return
    const available = phrases().filter(Boolean)
    if (!available.length) {
      text.value = ''
      return
    }
    if (prefersReducedMotion()) {
      text.value = available[0]
      return
    }
    const phrase = available[phraseIndex % available.length]
    characterIndex += 1
    text.value = phrase.slice(0, characterIndex)
    if (characterIndex < phrase.length) {
      timer = window.setTimeout(typeNext, 72)
      return
    }
    timer = window.setTimeout(() => {
      phraseIndex = (phraseIndex + 1) % available.length
      characterIndex = 0
      text.value = ''
      typeNext()
    }, 900)
  }

  watch(active, (isActive) => {
    stop()
    phraseIndex = 0
    characterIndex = 0
    text.value = ''
    if (isActive) typeNext()
  })

  onBeforeUnmount(stop)
  return { text }
}
