import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

test('product copy describes capabilities without conversational headlines', async () => {
  const home = await read('index.html')
  assert.match(home, /阅读、记录与任务管理/)
  assert.match(home, /可追溯的书籍知识库/)
  assert.doesNotMatch(home, /读文档，记小记|管任务。|你可以用它做什么|先把想法留下|想读懂一份|选一本书读|一步步用起来|具体怎么用，在这里查/)
  assert.match(home, /功能演示/)
  assert.match(home, /示例数据/)
})

test('each scene exposes functional steps and local-only controls', async () => {
  const [home, script] = await Promise.all([read('index.html'), read('src/productDemo.js')])
  for (const scene of ['reader', 'timeline', 'tasks', 'knowledge']) {
    const panel = home.match(new RegExp(`<article[^>]*data-feature-panel="${scene}"[\\s\\S]*?(?=<article[^>]*data-feature-panel|<!-- tour:end -->)`))?.[0]
    assert.ok(panel, scene)
    assert.match(panel, /data-demo-viewport/)
    assert.match(panel, /data-demo-view/)
    assert.match(panel, /data-demo-go/)
  }
  for (const control of ['play', 'previous', 'next', 'steps', 'status']) assert.ok(home.includes(`data-demo-${control}`), control)
  assert.match(script, /IntersectionObserver/)
  assert.match(script, /visibilitychange/)
  assert.match(script, /prefers-reduced-motion/)
  assert.match(script, /dispose/)
  assert.doesNotMatch(script, /(?:fetch|axios)\s*\(|localStorage|innerHTML|\.focus\(/)
})

test('responsive layout keeps inactive scenes hidden and repeated tabs preserve playback', async () => {
  const [css, script] = await Promise.all([read('src/productDemo.css'), read('src/productDemo.js')])
  assert.match(css, /\.product-demo \.showcase-panel\[hidden\][^{]*\{[^}]*display:\s*none\s*!important/)
  assert.match(script, /if \(player\.getState\(\)\.scenario === event\.detail\.id\) return/)
  assert.match(css, /\.demo-steps\[data-count='4'\][^{]*\{[^}]*repeat\(2,/)
})

test('all scene views and clickable destinations refer to valid workflow steps', async () => {
  const { scenarios } = await import('../src/demoPlayback.mjs')
  const home = await read('index.html')
  for (const [name, steps] of Object.entries(scenarios)) {
    const panel = home.match(new RegExp(`<article[^>]*data-feature-panel="${name}"[\\s\\S]*?(?=<article[^>]*data-feature-panel|<!-- tour:end -->)`))?.[0]
    const visibleSteps = new Set()
    for (const [, attribute, value] of panel.matchAll(/data-demo-(view|go)="([\d ]+)"/g)) {
      for (const raw of value.split(' ')) {
        const index = Number(raw)
        assert.ok(index >= 0 && index < steps.length, `${name}: ${attribute}=${raw}`)
        if (attribute === 'view') visibleSteps.add(index)
      }
    }
    assert.equal(visibleSteps.size, steps.length, `${name}: every step has a preview`)
  }
})

function clock() {
  let serial = 0
  let time = 0
  const pending = new Map()
  return {
    now: () => time,
    setTimer: (callback, delay) => { pending.set(++serial, { callback, delay }); return serial },
    clearTimer: id => pending.delete(id),
    tick: () => { const [id, timer] = pending.entries().next().value ?? []; if (timer) { time += timer.delay; pending.delete(id); timer.callback() } },
    elapse: ms => { time += ms },
    delay: () => pending.values().next().value?.delay,
    size: () => pending.size,
  }
}

test('playback loops every feature in order with exactly one visible timer', async () => {
  const { createPlayback, scenarios } = await import('../src/demoPlayback.mjs')
  const timers = clock()
  const changes = []
  const player = createPlayback({ ...timers, onChange: s => changes.push(s) })
  assert.equal(timers.size(), 0)
  player.setVisible(true)
  assert.equal(player.getState().playing, true)
  assert.equal(timers.size(), 1)
  timers.tick()
  assert.equal(player.getState().index, 1)
  player.seek(2)
  assert.equal(player.getState().playing, false)
  assert.equal(timers.size(), 0)
  player.play()
  player.seek(0)
  player.play()
  for (let cycle = 0; cycle < 2; cycle++) {
    for (const [name, steps] of Object.entries(scenarios)) {
      for (let step = 0; step < steps.length; step++) {
        assert.equal(player.getState().scenario, name)
        assert.equal(player.getState().index, step)
        assert.equal(player.getState().playing, true)
        assert.equal(timers.size(), 1)
        assert.ok(timers.delay() >= 1300 && timers.delay() <= 2200)
        timers.tick()
      }
    }
    assert.equal(player.getState().scenario, 'reader')
    assert.equal(player.getState().index, 0)
  }
  player.dispose()
  assert.equal(timers.size(), 0)
  const count = changes.length
  player.next()
  assert.equal(changes.length, count)
})

test('pausing, backgrounding and leaving the viewport preserve remaining step time', async () => {
  const { createPlayback } = await import('../src/demoPlayback.mjs')
  const timers = clock()
  const player = createPlayback(timers)
  player.setVisible(true)
  const duration = timers.delay()
  timers.elapse(500)
  player.setVisible(false)
  timers.elapse(10000)
  player.setVisible(true)
  assert.equal(timers.delay(), duration - 500)
  timers.elapse(300)
  player.pause()
  timers.elapse(10000)
  player.play()
  assert.equal(timers.delay(), duration - 800)
  timers.elapse(100)
  player.setForeground(false)
  timers.elapse(10000)
  player.setForeground(true)
  assert.equal(timers.delay(), duration - 900)
  timers.tick()
  assert.equal(player.getState().index, 1)
})

test('manual navigation crosses feature boundaries and resumes rather than replaying the last step', async () => {
  const { createPlayback, scenarios } = await import('../src/demoPlayback.mjs')
  const timers = clock()
  const player = createPlayback(timers)
  player.setVisible(true)
  player.seek(scenarios.reader.length - 1)
  player.play()
  assert.equal(player.getState().index, scenarios.reader.length - 1)
  timers.tick()
  assert.equal(player.getState().scenario, 'timeline')
  assert.equal(player.getState().index, 0)
  player.previous()
  assert.equal(player.getState().scenario, 'reader')
  assert.equal(player.getState().index, scenarios.reader.length - 1)
  assert.equal(player.getState().playing, false)
  player.next()
  assert.equal(player.getState().scenario, 'timeline')
  player.setScenario('knowledge')
  player.seek(scenarios.knowledge.length - 1)
  player.next()
  assert.equal(player.getState().scenario, 'reader')
  assert.equal(player.getState().index, 0)
})

test('renderer owns automatic tab synchronization, stable sizing and overlapping transitions', async () => {
  const [script, transitions, css] = await Promise.all([read('src/productDemo.js'), read('src/demoMotion.js'), read('src/productDemo.css')])
  assert.match(script, /selectFeature\(state\.scenario,\s*\{\s*notify:\s*false,\s*animate:\s*false/)
  assert.match(script, /preserveDemoLayout/)
  assert.match(transitions, /cloneNode/)
  assert.match(transitions, /aria-hidden/)
  assert.match(transitions, /inert/)
  assert.match(transitions, /getComputedStyle/)
  assert.match(transitions, /cancel/)
  assert.match(transitions, /data-demo-detail/)
  assert.match(transitions, /detail\.style\.minHeight/)
  assert.match(transitions, /layoutHeight/)
  assert.match(css, /demo-transition-layer/)
  assert.match(css, /\.demo-steps[^}]*min-height:/)
})

test('hidden pages and offscreen scenes pause without restarting or stealing progress', async () => {
  const { createPlayback } = await import('../src/demoPlayback.mjs')
  const timers = clock()
  const player = createPlayback(timers)
  player.setVisible(true)
  timers.tick()
  player.setForeground(false)
  assert.equal(timers.size(), 0)
  assert.equal(player.getState().index, 1)
  player.setForeground(true)
  assert.equal(timers.size(), 1)
  player.setVisible(false)
  assert.equal(timers.size(), 0)
  player.setVisible(true)
  assert.equal(player.getState().index, 1)
  player.pause()
  player.setVisible(false)
  player.setVisible(true)
  assert.equal(timers.size(), 0)
  player.setScenario('timeline')
  assert.equal(player.getState().index, 0)
  assert.equal(timers.size(), 0)
  player.setScenario('unknown')
  assert.equal(player.getState().scenario, 'timeline')
  player.seek(999)
  assert.equal(player.getState().index, player.getState().count - 1)
  player.seek(-999)
  assert.equal(player.getState().index, 0)
})

test('reduced motion has no automatic playback and new scenes remain navigable', async () => {
  const { createPlayback, scenarios } = await import('../src/demoPlayback.mjs')
  const timers = clock()
  const player = createPlayback({ ...timers, reducedMotion: true })
  player.setVisible(true)
  assert.equal(timers.size(), 0)
  for (const name of Object.keys(scenarios)) {
    player.setScenario(name)
    assert.equal(player.getState().playing, false)
    for (let i = 1; i < scenarios[name].length; i++) player.next()
    assert.equal(player.getState().index, scenarios[name].length - 1)
  }
  player.play()
  assert.equal(timers.size(), 1, 'explicit playback is an intentional user action')
  player.setReducedMotion(true)
  assert.equal(timers.size(), 0)
})

test('cancelled callbacks cannot advance a new scene or disposed playback', async () => {
  const { createPlayback } = await import('../src/demoPlayback.mjs')
  const callbacks = []
  const player = createPlayback({ setTimer: callback => { callbacks.push(callback); return callbacks.length }, clearTimer: () => {} })
  player.setVisible(true)
  player.setScenario('knowledge')
  callbacks[0]()
  assert.equal(player.getState().scenario, 'knowledge')
  assert.equal(player.getState().index, 0)
  player.dispose()
  callbacks.at(-1)()
  assert.equal(player.getState().index, 0)
})
