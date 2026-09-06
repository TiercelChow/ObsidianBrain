import './manual.css'

const links = [...document.querySelectorAll('[data-chapter-link]')]
const sections = [...document.querySelectorAll('.manual-section[id]')]
const mobileDirectory = document.querySelector('.mobile-chapters')
const currentLabel = document.querySelector('[data-current-chapter]')
let currentChapter = ''

function markChapter(id) {
  if (id === currentChapter) return
  currentChapter = id
  links.forEach((link) => {
    const active = link.hash === `#${id}`
    if (active) link.setAttribute('aria-current', 'location')
    else link.removeAttribute('aria-current')
    if (active && currentLabel) currentLabel.textContent = link.textContent.trim().replace(/^(?:\d+|↗)\s*/, '')
  })
}

// Native anchor navigation also works without JavaScript. No nested reading scroller.
links.forEach((link) => link.addEventListener('click', () => {
  if (mobileDirectory?.open) {
    mobileDirectory.open = false
    mobileDirectory.querySelector('summary').focus({ preventScroll: true })
  }
  markChapter(link.hash.slice(1))
}))

let frame = 0
function updateChapter() {
  frame = 0
  const offset = mobileDirectory?.getClientRects().length
    ? mobileDirectory.getBoundingClientRect().bottom + 24
    : 140
  let active = sections[0]
  for (const section of sections) {
    if (section.getBoundingClientRect().top <= offset) active = section
    else break
  }
  if (active) markChapter(active.id)
}
function scheduleUpdate() {
  if (!frame) frame = requestAnimationFrame(updateChapter)
}
window.addEventListener('scroll', scheduleUpdate, { passive: true })
window.addEventListener('resize', scheduleUpdate)
window.addEventListener('hashchange', scheduleUpdate)
updateChapter()
