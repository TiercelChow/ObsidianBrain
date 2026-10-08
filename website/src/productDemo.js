import { createPlayback, scenarios } from './demoPlayback.mjs'
import { selectFeature } from './main.js'
import { createDemoMotion, preserveDemoLayout } from './demoMotion.js'

export function initProductDemo(host) {
  const panels = [...host.querySelectorAll('[data-feature-panel]')]
  const stepsHost = host.querySelector('[data-demo-steps]')
  const status = host.querySelector('[data-demo-status]')
  const detail = host.querySelector('[data-demo-detail]')
  const playButton = host.querySelector('[data-demo-play]')
  const previous = host.querySelector('[data-demo-previous]')
  const next = host.querySelector('[data-demo-next]')
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)')
  const controls = host.querySelector('[data-demo-controls]')
  const display = host.querySelector('[data-demo-display]')
  const fluid = createDemoMotion(motion)
  let activeScenario = null
  let activeIndex = -1

  function render(state) {
    const panel = panels.find(p => p.dataset.featurePanel === state.scenario)
    if (!panel) return
    const sceneChange = activeScenario !== state.scenario
    const changed = activeScenario && (sceneChange || activeIndex !== state.index)
    const outgoing = panels.find(p => p.dataset.featurePanel === activeScenario)
    const snapshot = changed ? fluid.capture(sceneChange ? outgoing : outgoing.querySelector('[data-demo-viewport]'), sceneChange ? display : outgoing) : null
    if (activeScenario !== state.scenario) {
      selectFeature(state.scenario, { notify: false, animate: false })
      activeScenario = state.scenario
      stepsHost.dataset.count = String(state.count)
      const buttons = scenarios[state.scenario].map((step, index) => {
        const button = document.createElement('button')
        button.type = 'button'
        button.dataset.demoStep = String(index)
        button.textContent = `${index + 1}. ${step.label}`
        return button
      })
      stepsHost.replaceChildren(...buttons)
    }
    host.dataset.playing = String(state.playing)
    host.dataset.running = String(state.running)
    panel.dataset.demoIndex = String(state.index)
    panel.querySelectorAll('[data-demo-view]').forEach(view => {
      view.hidden = !view.dataset.demoView.split(' ').includes(String(state.index))
    })
    stepsHost.querySelectorAll('button').forEach((button, index) => {
      if (index === state.index) button.setAttribute('aria-current', 'step')
      else button.removeAttribute('aria-current')
    })
    status.setAttribute('aria-live', state.playing ? 'off' : 'polite')
    status.textContent = `${String(state.index + 1).padStart(2, '0')} / ${String(state.count).padStart(2, '0')} · ${state.step.label}`
    detail.textContent = state.step.detail
    playButton.textContent = state.playing ? '暂停轮播' : '播放轮播'
    playButton.setAttribute('aria-pressed', String(state.playing))
    previous.disabled = false
    next.disabled = false
    const readingProgress = panel.querySelector('[data-demo-reading-progress]')
    if (readingProgress) {
      const percent = state.index >= 2 ? 64 : 42
      readingProgress.style.transform = `scaleX(${percent / 100})`
      panel.querySelector('[data-demo-reading-percent]').textContent = `${percent}%`
    }
    const taskProgress = panel.querySelector('[data-demo-task-progress]')
    if (taskProgress) {
      const percent = state.index >= 2 ? 67 : 33
      taskProgress.style.transform = `scaleX(${percent / 100})`
      panel.querySelector('[data-demo-task-percent]').textContent = `${percent}%`
    }
    activeIndex = state.index
    if (snapshot) fluid.transition(snapshot, sceneChange ? panel : panel.querySelector('[data-demo-viewport]'), sceneChange)
  }

  const player = createPlayback({ onChange: render, reducedMotion: motion.matches })
  render(player.getState())
  host.classList.add('demo-ready')
  controls.hidden = false
  host.querySelectorAll('[data-demo-go]').forEach(button => { button.disabled = false })
  const releaseLayout = preserveDemoLayout(host, panels)

  function onClick(event) {
    const button = event.target.closest('button')
    if (!button || !host.contains(button)) return
    if (button.hasAttribute('data-demo-play')) {
      if (player.getState().playing) player.pause()
      else player.play()
    } else if (button.hasAttribute('data-demo-previous')) player.previous()
    else if (button.hasAttribute('data-demo-next')) player.next()
    else if (button.hasAttribute('data-demo-step')) player.seek(Number(button.dataset.demoStep))
    else if (button.hasAttribute('data-demo-go')) player.seek(Number(button.dataset.demoGo))
  }

  function onFeature(event) {
    if (!Object.hasOwn(scenarios, event.detail?.id)) return
    if (player.getState().scenario === event.detail.id) return
    player.setScenario(event.detail.id)
  }
  // Keep a keyboard target from disappearing while someone is inspecting it.
  function onFocus(event) {
    if (event.target.matches('[data-demo-go], [data-demo-step]')) player.pause()
  }
  function onVisibility() { player.setForeground(!document.hidden) }
  function onMotion(event) { player.setReducedMotion(event.matches) }

  // Observe the preview, rather than the whole tall phone section.
  const observer = 'IntersectionObserver' in window ? new IntersectionObserver(entries => {
    const active = player.getState().scenario
    entries.forEach(entry => {
      if (entry.target.closest('[data-feature-panel]')?.dataset.featurePanel === active) {
        player.setVisible(entry.isIntersecting && entry.intersectionRatio >= .25)
      }
    })
  }, { threshold: [0, .25] }) : null
  if (observer) panels.forEach(panel => observer.observe(panel.querySelector('[data-demo-viewport]')))
  else { player.setReducedMotion(true); player.setVisible(true) }

  host.addEventListener('click', onClick)
  host.addEventListener('focusin', onFocus)
  document.addEventListener('ob-feature-change', onFeature)
  document.addEventListener('visibilitychange', onVisibility)
  motion.addEventListener('change', onMotion)
  onVisibility()

  return () => {
    player.dispose()
    fluid.dispose()
    releaseLayout()
    observer?.disconnect()
    host.removeEventListener('click', onClick)
    host.removeEventListener('focusin', onFocus)
    document.removeEventListener('ob-feature-change', onFeature)
    document.removeEventListener('visibilitychange', onVisibility)
    motion.removeEventListener('change', onMotion)
  }
}
