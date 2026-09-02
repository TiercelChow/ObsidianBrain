import './style.css'

const root = document.documentElement
const themeButton = document.querySelector('[data-theme-toggle]')
const themeLabel = themeButton?.querySelector('[data-theme-label]')
const menuButton = document.querySelector('[data-menu-toggle]')
const navigation = document.querySelector('[data-navigation]')
const themes = ['light', 'dark', 'eye-care']
const themeNames = { light: '浅色', dark: '深色', 'eye-care': '护眼' }
const themeColors = { light: '#f0f0f3', dark: '#000000', 'eye-care': '#c5d5b8' }

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

function selectFeature(id, focus = false) {
  featureButtons.forEach((button) => {
    const selected = button.dataset.featureTarget === id
    button.setAttribute('aria-selected', String(selected))
    button.tabIndex = selected ? 0 : -1
    if (selected && focus) button.focus()
  })
  featurePanels.forEach((panel) => {
    panel.hidden = panel.dataset.featurePanel !== id
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

const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
const revealItems = document.querySelectorAll('[data-reveal]')

if (reducedMotion || !('IntersectionObserver' in window)) {
  revealItems.forEach((item) => item.dataset.visible = 'true')
} else {
  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) return
      entry.target.dataset.visible = 'true'
      observer.unobserve(entry.target)
    })
  }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 })
  revealItems.forEach((item) => observer.observe(item))
}

const year = document.querySelector('[data-year]')
if (year) year.textContent = String(new Date().getFullYear())
