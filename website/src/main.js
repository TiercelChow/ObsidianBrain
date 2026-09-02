import './style.css'

const root = document.documentElement
const themeButton = document.querySelector('[data-theme-toggle]')
const themeLabel = themeButton?.querySelector('[data-theme-label]')
const menuButton = document.querySelector('[data-menu-toggle]')
const navigation = document.querySelector('[data-navigation]')
const themes = ['light', 'dark', 'eye-care']
const themeNames = { light: '浅色', dark: '深色', 'eye-care': '护眼' }
const themeColors = { light: '#f0f0f3', dark: '#000000', 'eye-care': '#c5d5b8' }
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

if (reducedMotion) {
  root.classList.add('motion-ready')
} else {
  requestAnimationFrame(() => root.classList.add('motion-ready'))
}

function preferredTheme() {
  const stored = localStorage.getItem('ob-website-theme')
  if (stored && themes.includes(stored)) return stored
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function setTheme(theme) {
  root.dataset.theme = theme
  root.style.colorScheme = theme === 'dark' ? 'dark' : 'light'
  localStorage.setItem('ob-website-theme', theme)
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', themeColors[theme])
  if (themeLabel) themeLabel.textContent = themeNames[theme]
  if (themeButton) themeButton.setAttribute('aria-label', `当前为${themeNames[theme]}主题，切换主题`)
}

setTheme(preferredTheme())

themeButton?.addEventListener('click', () => {
  const next = themes[(themes.indexOf(root.dataset.theme) + 1) % themes.length]
  setTheme(next)
})

function setMenu(open) {
  if (!menuButton || !navigation) return
  menuButton.setAttribute('aria-expanded', String(open))
  menuButton.setAttribute('aria-label', open ? '关闭导航' : '打开导航')
  navigation.dataset.open = String(open)
}

menuButton?.addEventListener('click', () => {
  setMenu(menuButton.getAttribute('aria-expanded') !== 'true')
})

navigation?.querySelectorAll('a').forEach((link) => {
  link.addEventListener('click', () => setMenu(false))
})

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') setMenu(false)
})

const featureButtons = [...document.querySelectorAll('[data-feature-target]')]
const featurePanels = [...document.querySelectorAll('[data-feature-panel]')]

function animateFeaturePanel(panel) {
  if (reducedMotion || typeof panel.animate !== 'function') return
  panel.getAnimations().forEach((animation) => animation.cancel())
  panel.animate([
    { opacity: 0, transform: 'translate3d(0, 18px, 0) scale(.985)', filter: 'blur(8px)' },
    { opacity: 1, transform: 'translate3d(0, 0, 0) scale(1)', filter: 'blur(0)' },
  ], {
    duration: 560,
    easing: 'cubic-bezier(.32, .72, 0, 1)',
  })
}

function selectFeature(id, focus = false) {
  featureButtons.forEach((button) => {
    const selected = button.dataset.featureTarget === id
    button.setAttribute('aria-selected', String(selected))
    button.tabIndex = selected ? 0 : -1
    if (selected && focus) button.focus()
  })
  featurePanels.forEach((panel) => {
    const selected = panel.dataset.featurePanel === id
    const wasHidden = panel.hidden
    panel.hidden = !selected
    if (selected && wasHidden) animateFeaturePanel(panel)
  })
}

featureButtons.forEach((button, index) => {
  button.addEventListener('click', () => selectFeature(button.dataset.featureTarget))
  button.addEventListener('keydown', (event) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    let next = index
    if (event.key === 'ArrowRight') next = (index + 1) % featureButtons.length
    if (event.key === 'ArrowLeft') next = (index - 1 + featureButtons.length) % featureButtons.length
    if (event.key === 'Home') next = 0
    if (event.key === 'End') next = featureButtons.length - 1
    selectFeature(featureButtons[next].dataset.featureTarget, true)
  })
})

document.querySelectorAll('[data-copy]').forEach((button) => {
  button.addEventListener('click', async () => {
    const target = document.querySelector(button.dataset.copy)
    if (!target) return
    const value = target.textContent.trim()
    try {
      await navigator.clipboard.writeText(value)
      button.dataset.state = 'copied'
      button.querySelector('span').textContent = '已复制'
      window.setTimeout(() => {
        button.dataset.state = ''
        button.querySelector('span').textContent = '复制'
      }, 1800)
    } catch {
      window.prompt('复制以下命令', value)
    }
  })
})

document.querySelectorAll('[data-reveal-group]').forEach((group) => {
  const items = [...group.children].filter((item) => item.matches('[data-reveal]'))
  items.forEach((item, index) => {
    item.style.setProperty('--reveal-delay', `${Math.min(index * 75, 300)}ms`)
  })
})

document.querySelectorAll('[data-reveal-sequence]').forEach((sequence) => {
  Array.from(sequence.children).forEach((item, index) => {
    item.style.setProperty('--sequence-delay', `${60 + Math.min(index * 78, 390)}ms`)
  })
})

const revealItems = document.querySelectorAll('[data-reveal]')

if (reducedMotion || !('IntersectionObserver' in window)) {
  revealItems.forEach((item) => item.dataset.visible = 'true')
} else {
  const pendingVisibility = new Map()
  let revealFrame = 0

  function queueVisibility(item, visible) {
    pendingVisibility.set(item, visible)
    if (revealFrame) return
    revealFrame = requestAnimationFrame(() => {
      pendingVisibility.forEach((visible, pending) => {
        pending.dataset.visible = String(visible)
      })
      pendingVisibility.clear()
      revealFrame = 0
    })
  }

  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (entry.intersectionRatio >= 0.08) {
        queueVisibility(entry.target, true)
      } else if (entry.intersectionRatio === 0) {
        queueVisibility(entry.target, false)
      }
    })
  }, { rootMargin: '0px 0px -10% 0px', threshold: [0, 0.08] })
  revealItems.forEach((item) => observer.observe(item))
}

const year = document.querySelector('[data-year]')
if (year) year.textContent = String(new Date().getFullYear())
