export interface DiagramSize { width: number; height: number }
export interface DiagramTransform { x: number; y: number; scale: number }
export type DiagramViewMode = 'fit' | 'readable'

const dimension = (value: number) => Number.isFinite(value) && value > 0 ? value : 1

export function getDiagramViewport(diagram: DiagramSize, stage: DiagramSize, mode: DiagramViewMode) {
  const width = dimension(diagram.width), height = dimension(diagram.height)
  const stageWidth = dimension(stage.width), stageHeight = dimension(stage.height)
  const fitScale = Math.min(1, stageWidth / width, stageHeight / height)
  const scale = mode === 'fit' ? fitScale : Math.max(fitScale, 0.9)
  return {
    scale, fitScale,
    x: width * scale <= stageWidth ? (stageWidth - width * scale) / 2 : 0,
    y: height * scale <= stageHeight ? (stageHeight - height * scale) / 2 : 0,
  }
}

/** Keep a small pan margin without allowing the graph to disappear offscreen. */
export function constrainDiagramTransform(transform: DiagramTransform, diagram: DiagramSize, stage: DiagramSize): DiagramTransform {
  const constrain = (offset: number, content: number, viewport: number) => content <= viewport
    ? (viewport - content) / 2
    : Math.min(64, Math.max(viewport - content - 64, offset))
  return {
    scale: transform.scale,
    x: constrain(transform.x, diagram.width * transform.scale, stage.width),
    y: constrain(transform.y, diagram.height * transform.scale, stage.height),
  }
}
