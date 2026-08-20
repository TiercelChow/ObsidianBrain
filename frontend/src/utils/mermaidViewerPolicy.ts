import { isPhoneViewport } from './mobileLayoutPolicy.ts'

export interface MermaidViewerPolicy {
  mobile: boolean
  hint: string
  floatingClose: boolean
}

export function getMermaidViewerPolicy(viewportWidth: number): MermaidViewerPolicy {
  const mobile = isPhoneViewport(viewportWidth)
  return mobile
    ? {
        mobile: true,
        hint: '双指缩放 · 单指拖动 · 双击放大',
        floatingClose: true,
      }
    : {
        mobile: false,
        hint: '滚轮缩放 · 拖拽平移 · 双击放大 · Esc 关闭',
        floatingClose: false,
      }
}
