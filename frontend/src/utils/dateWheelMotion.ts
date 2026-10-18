import { createSpringTrack } from './springTrack.ts'
import { wheelIndex } from './dateWheel.ts'

interface WheelPointer { id: number; y: number; time: number }
export interface DateWheelFrame { position: number; index: number; dragging: boolean; moving: boolean }
type Clock = Parameters<typeof createSpringTrack>[1]
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value))
const dimension = 132, resistance = .55
const rubber = (distance: number) => distance * dimension * resistance / (dimension + distance * resistance)
const unRubber = (distance: number) => distance * dimension / (resistance * Math.max(1, dimension - distance))

/** A single interruptible track owns drag, momentum, edge resistance and snap. */
export function createDateWheelMotion(options: { clock: Clock; count: () => number; change: (frame: DateWheelFrame) => void }) {
  const frame: DateWheelFrame = { position: 0, index: 0, dragging: false, moving: false }
  const emit = () => options.change({ ...frame })
  const track = createSpringTrack(position => {
    frame.position = position
    frame.index = wheelIndex(position, options.count())
    emit()
  }, options.clock)
  let pointer: number | null = null, originY = 0, originPosition = 0, moved = false, intended = 0
  let samples: Array<{ position: number; time: number }> = []
  const max = () => Math.max(0, options.count() - 1) * 44
  const target = (position: number) => wheelIndex(position, options.count()) * 44

  function animate(position: number) {
    frame.moving = true
    emit()
    track.to(position, () => { frame.moving = false; emit() })
  }
  function sync(index: number) {
    pointer = null; frame.dragging = false; frame.moving = false
    intended = target(index * 44)
    track.jump(intended)
  }
  function select(index: number) {
    intended = target(index * 44)
    animate(intended)
  }
  function down(point: WheelPointer) {
    if (pointer !== null) return false
    track.pause()
    pointer = point.id; originY = point.y; moved = false
    const value = frame.position, end = max()
    // Re-grabbing a rebounding edge must start at its live presentation value.
    originPosition = value < 0 ? -unRubber(-value) : value > end ? end + unRubber(value - end) : value
    samples = [{ position: value, time: point.time }]
    frame.dragging = true; frame.moving = false; emit()
    return true
  }
  function move(point: WheelPointer) {
    if (point.id !== pointer) return
    moved ||= Math.abs(point.y - originY) > 6
    const raw = originPosition + originY - point.y, end = max()
    const position = raw < 0 ? -rubber(-raw) : raw > end ? end + rubber(raw - end) : raw
    samples.push({ position, time: point.time })
    samples = samples.filter(sample => point.time - sample.time <= 90)
    const first = samples[0]
    const velocity = first ? clamp((position - first.position) * 1000 / Math.max(1, point.time - first.time), -3000, 3000) : 0
    track.jump(position, velocity)
  }
  function up(point: WheelPointer) {
    if (point.id !== pointer) return false
    move(point)
    pointer = null; frame.dragging = false
    const first = samples[0]
    const velocity = first ? clamp((frame.position - first.position) * 1000 / Math.max(1, point.time - first.time), -3000, 3000) : 0
    const outside = frame.position < 0 || frame.position > max()
    const releaseVelocity = outside ? velocity * .35 : velocity
    // Stronger friction than free page scrolling; project then quantize once.
    const projected = frame.position + releaseVelocity / 1000 * .99 / (1 - .99)
    intended = target(projected)
    track.jump(frame.position, releaseVelocity)
    animate(intended)
    return moved
  }
  function cancel(id: number) {
    if (pointer !== id) return
    pointer = null; frame.dragging = false
    intended = target(frame.position)
    animate(intended)
  }
  function settle() { sync(wheelIndex(frame.position, options.count())) }
  function wheel(pixels: number) {
    if (!Number.isFinite(pixels)) return
    intended = clamp(intended + pixels, 0, max())
    animate(target(intended))
  }
  return { state: () => ({ ...frame }), targetIndex: () => wheelIndex(intended, options.count()), sync, select, down, move, up, cancel, settle, wheel, dispose: () => {
    pointer = null; frame.dragging = false; frame.moving = false; track.dispose()
  } }
}
