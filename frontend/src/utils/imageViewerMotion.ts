import { createSpringTrack } from './springTrack.ts'

interface Point { x: number; y: number }
interface PhotoTransform extends Point { scale: number }
interface PhotoSize { width: number; height: number; stageWidth: number; stageHeight: number }
export interface PhotoFrame extends PhotoTransform, PhotoSize { slide: number; dragging: boolean }
interface Pointer extends Point { id: number; time: number }
type Clock = Parameters<typeof createSpringTrack>[1]
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

export function fitPhoto(width: number, height: number, stageWidth: number, stageHeight: number) {
  if (![width, height, stageWidth, stageHeight].every(value => Number.isFinite(value) && value > 0)) return { width: 0, height: 0 }
  const ratio = Math.min(stageWidth / width, stageHeight / height)
  return { width: width * ratio, height: height * ratio }
}

export function constrainPhoto(value: PhotoTransform, size: PhotoSize): PhotoTransform {
  const scale = clamp(value.scale, 1, 8)
  const x = Math.max(0, (size.width * scale - size.stageWidth) / 2)
  const y = Math.max(0, (size.height * scale - size.stageHeight) / 2)
  return { x: clamp(value.x, -x, x) || 0, y: clamp(value.y, -y, y) || 0, scale }
}

/** Coordinates are always relative to the stage center, never page offsets. */
export function zoomPhoto(value: PhotoTransform, scale: number, anchor: Point, size: PhotoSize) {
  const next = clamp(scale, 1, 8), ratio = next / value.scale
  return constrainPhoto({ x: anchor.x - (anchor.x - value.x) * ratio, y: anchor.y - (anchor.y - value.y) * ratio, scale: next }, size)
}

/** DOM-free gesture engine; one set of interruptible tracks owns all transforms. */
export function createImageViewerMotion(options: {
  clock: Clock
  count: () => number
  change: (frame: PhotoFrame) => void
  navigate: (direction: number) => void
}) {
  const frame: PhotoFrame = { width: 0, height: 0, stageWidth: 0, stageHeight: 0, x: 0, y: 0, scale: 1, slide: 0, dragging: false }
  const emit = () => options.change({ ...frame })
  const track = (key: 'x' | 'y' | 'scale' | 'slide', unit = 1) => createSpringTrack(value => { frame[key] = key === 'scale' ? clamp(value / unit, 1, 8) : value; emit() }, options.clock)
  const x = track('x'), y = track('y'), scale = track('scale', 1000), slide = track('slide')
  scale.jump(1000)
  const pointers = new Map<number, Pointer>()
  let mode: 'pending' | 'swipe' | 'pan' | 'pinch' | 'vertical' = 'pending'
  let origin: Pointer = { id: 0, x: 0, y: 0, time: 0 }
  let start = { ...frame }, pinchDistance = 1, pinchCenter = { x: 0, y: 0 }
  let samples: Pointer[] = [], suppressClick = false
  let zoomTarget = 1

  function pause() { x.pause(); y.pause(); scale.pause(); slide.pause(); zoomTarget = frame.scale }
  function jump(value: PhotoTransform) { x.jump(value.x); y.jump(value.y); scale.jump(value.scale * 1000); zoomTarget = value.scale }
  function settle(value = constrainPhoto(frame, frame)) { x.to(value.x); y.to(value.y); scale.to(value.scale * 1000); zoomTarget = value.scale }
  function cancel(pointerId?: number) {
    if (pointerId !== undefined && !pointers.has(pointerId)) return
    pointers.clear(); frame.dragging = false; suppressClick = true; slide.to(0); settle(); emit()
  }
  function fit(width: number, height: number, stageWidth: number, stageHeight: number) {
    pause(); pointers.clear()
    Object.assign(frame, fitPhoto(width, height, stageWidth, stageHeight), { stageWidth, stageHeight, dragging: false })
    jump({ x: 0, y: 0, scale: 1 }); slide.jump(0); emit()
  }
  function local(point: Point): Point { return { x: point.x - frame.stageWidth / 2, y: point.y - frame.stageHeight / 2 } }
  function pair() {
    const [a, b] = [...pointers.values()]
    return { distance: Math.max(1, Math.hypot(a.x - b.x, a.y - b.y)), center: local({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }) }
  }
  function down(point: Pointer) {
    if (pointers.size >= 2) return false
    pause(); pointers.set(point.id, point)
    if (pointers.size === 2) {
      slide.jump(0)
      const pairStart = pair()
      mode = 'pinch'; pinchDistance = pairStart.distance; pinchCenter = pairStart.center
      suppressClick = true
    } else {
      mode = frame.scale > 1.01 ? 'pan' : 'pending'
      origin = point; samples = [point]; suppressClick = false
    }
    start = { ...frame }; frame.dragging = true; emit()
    return true
  }
  function move(point: Pointer) {
    if (!pointers.has(point.id)) return
    pointers.set(point.id, point)
    if (mode === 'pinch' && pointers.size === 2) {
      const current = pair(), nextScale = clamp(start.scale * current.distance / pinchDistance, 1, 8)
      const ratio = nextScale / start.scale
      jump(constrainPhoto({ x: current.center.x - (pinchCenter.x - start.x) * ratio, y: current.center.y - (pinchCenter.y - start.y) * ratio, scale: nextScale }, frame))
      return
    }
    const dx = point.x - origin.x, dy = point.y - origin.y
    if (Math.hypot(dx, dy) > 8) suppressClick = true
    if (mode === 'pending' && Math.max(Math.abs(dx), Math.abs(dy)) > 10) mode = Math.abs(dx) > Math.abs(dy) * 1.15 ? 'swipe' : 'vertical'
    if (mode === 'swipe') {
      const offset = start.slide + dx
      slide.jump(options.count() > 1 ? offset : offset * .2)
      samples.push(point)
      while (samples.length > 2 && point.time - samples[0].time > 100) samples.shift()
    } else if (mode === 'pan') {
      const desired = { x: start.x + dx, y: start.y + dy, scale: frame.scale }
      const bounded = constrainPhoto(desired, frame)
      const resistance = (delta: number, extent: number) => delta * .35 * extent / (extent + .35 * Math.abs(delta) || 1)
      jump({ ...bounded, x: bounded.x + resistance(desired.x - bounded.x, frame.stageWidth), y: bounded.y + resistance(desired.y - bounded.y, frame.stageHeight) })
    }
  }
  function navigate(direction: number, velocity = 0) {
    if (options.count() < 2) { slide.to(0); return }
    suppressClick = true
    // Re-target from the presentation value; never reset an in-flight slide.
    if (velocity) slide.jump(frame.slide, velocity)
    slide.to(-direction * (frame.stageWidth + 16), () => {
      options.navigate(direction)
      jump({ x: 0, y: 0, scale: 1 }); slide.jump(0)
    })
  }
  function up(point: Pointer) {
    if (!pointers.has(point.id)) return
    move(point); pointers.delete(point.id)
    if (pointers.size) {
      // Pinch -> one-finger pan, even if the pinch ended at fit scale.
      origin = [...pointers.values()][0]; start = { ...frame }; mode = 'pan'
      return
    }
    frame.dragging = false
    if (mode === 'swipe') {
      const recent = samples.filter(sample => point.time - sample.time <= 100)
      const first = recent[0]
      const velocity = first && point.time > first.time ? (point.x - first.x) / (point.time - first.time) * 1000 : 0
      const threshold = Math.max(48, Math.min(100, frame.stageWidth * .18))
      const flick = Math.abs(velocity) > 450 && Math.abs(frame.slide) > 24 && Math.sign(velocity) === Math.sign(frame.slide)
      if (options.count() > 1 && (Math.abs(frame.slide) > threshold || flick)) navigate(frame.slide < 0 ? 1 : -1, velocity)
      else slide.to(0)
    } else { slide.to(0); settle() }
    emit()
  }
  return {
    fit, down, move, up, cancel, navigate,
    load(width: number, height: number) {
      Object.assign(frame, fitPhoto(width, height, frame.stageWidth, frame.stageHeight))
      jump(constrainPhoto(frame, frame))
      emit()
    },
    zoom(factor: number, anchor: Point = { x: 0, y: 0 }) {
      if (!frame.width) return
      const target = zoomPhoto(frame, zoomTarget * factor, anchor, frame)
      zoomTarget = target.scale
      x.to(target.x); y.to(target.y); scale.to(target.scale * 1000)
    },
    reset() { pause(); slide.to(0); settle({ x: 0, y: 0, scale: 1 }) },
    consumeClick() { const value = suppressClick; suppressClick = false; return value },
    dispose() { pause(); pointers.clear(); frame.dragging = false },
  }
}
