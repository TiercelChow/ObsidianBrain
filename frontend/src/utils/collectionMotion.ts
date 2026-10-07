/** Leaving grid tiles retain their own slot, not the width of the whole grid. */
export function freezeCollectionLeave(element: Element) {
  if (!(element instanceof HTMLElement)) return
  const { offsetWidth, offsetHeight, offsetTop, offsetLeft } = element
  Object.assign(element.style, { width:`${offsetWidth}px`, height:`${offsetHeight}px`, top:`${offsetTop}px`, left:`${offsetLeft}px` })
  element.inert = true
}
export function resetCollectionEnter(element: Element) {
  if (!(element instanceof HTMLElement)) return
  for (const property of ['width', 'height', 'top', 'left']) element.style.removeProperty(property)
  element.inert = false
}
