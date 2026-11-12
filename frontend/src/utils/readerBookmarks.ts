export type BookmarkLocation =
  | { kind: 'epub'; chapter:string; ratio:number }
  | { kind: 'md'; ratio: number; block_index: number; text: string; offset: number }
  | { kind: 'pdf'; page: number; offset: number; x: number }
export interface ReaderBookmark {
  id: string; book_id: string; file_path: string; title: string; note_md: string
  location: BookmarkLocation; revision: number; created_at: string; updated_at: string
}
export type BookmarkDraft = Omit<ReaderBookmark, 'id' | 'revision' | 'created_at' | 'updated_at'> & { id?: string; revision?: number }
export const clampBookmarkUnit = (value: number) => Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0
const normalized = (text: string) => text.replace(/\s+/g, ' ').trim()
const excerpt = (text: string) => [...normalized(text)].slice(0, 200).join('')
const last = <T>(items: T[]) => items[items.length - 1]

export function resolveBookmarkBlock(texts: string[], location: Extract<BookmarkLocation, {kind:'md'}>): number {
  const target = normalized(location.text)
  const matches = texts.map((text, index) => excerpt(text) === target ? index : -1).filter(index => index >= 0)
  return matches.sort((a, b) => Math.abs(a - location.block_index) - Math.abs(b - location.block_index))[0] ?? -1
}
export function bookmarkPositionLabel(location: BookmarkLocation): string {
  if(location.kind==='epub')return `${location.chapter.split('/').pop()} · ${Math.round(location.ratio*100)}%`
  return location.kind === 'pdf' ? `第 ${location.page} 页 · ${Math.round(location.offset * 100)}%` : `${Math.round(location.ratio * 100)}%`
}
function blocks(article: HTMLElement) {
  return Array.from(article.querySelectorAll<HTMLElement>('h1,h2,h3,h4,h5,h6,p,li,pre.code-content,table,.mermaid')).filter(el => !el.closest('.katex') && (el.matches('.mermaid') || !el.closest('.mermaid')))
}
export function captureBookmarkLocation(root: HTMLElement, kind: 'md' | 'pdf' | 'epub', point?: {x:number;y:number}): {location: BookmarkLocation; excerpt: string} | null {
  const rootRect = root.getBoundingClientRect()
  if(kind==='epub'){
    const frame=root.querySelector<HTMLIFrameElement>('.epub-frame'),chapter=frame?.dataset.epubChapter
    if(!frame?.contentDocument?.body || !chapter || frame.style.visibility==='hidden')return null
    const selected=frame.contentWindow?.getSelection()
    const rect=selected && !selected.isCollapsed && selected.rangeCount?selected.getRangeAt(0).getBoundingClientRect():null
    const y=rect?frame.getBoundingClientRect().top+rect.top:point?.y
    return {location:{kind:'epub',chapter,ratio:clampBookmarkUnit((root.scrollTop+(y===undefined?0:y-rootRect.top-64))/Math.max(1,root.scrollHeight-root.clientHeight))},excerpt:rect?excerpt(selected?.toString() || ''):'EPUB 书签'}
  }
  const selection = window.getSelection()
  const selected = !point && selection?.rangeCount && !selection.isCollapsed && root.contains(selection.anchorNode) ? selection.getRangeAt(0) : null
  const line = selected?.getBoundingClientRect().top ?? point?.y ?? (rootRect.top + Math.min(64, root.clientHeight / 4))
  if (kind === 'pdf') {
    const pages = Array.from(root.querySelectorAll<HTMLElement>('.pdf-page-wrap'))
    const page = pages.find(el => el.getBoundingClientRect().bottom > line) ?? last(pages)
    if (!page) return null
    const rect = page.getBoundingClientRect()
    const pageNumber = Number(page.dataset.pageNum)
    return {location:{kind:'pdf',page:pageNumber,offset:clampBookmarkUnit((line - rect.top) / rect.height),x:clampBookmarkUnit(((selected?.getBoundingClientRect().left ?? point?.x ?? rootRect.left) - rect.left) / rect.width)},excerpt:`第 ${pageNumber} 页`}
  }
  const article = last(Array.from(root.querySelectorAll<HTMLElement>('article.markdown-body')))
  if (!article) return null
  const items = blocks(article)
  const block = (selected ? [...items].reverse().find(el => el.contains(selected.startContainer)) : null)
    ?? items.find(el => el.getBoundingClientRect().bottom > line) ?? last(items)
  if (!block) return {location:{kind:'md',ratio:0,block_index:0,text:'',offset:0},excerpt:'书签'}
  const rect = block.getBoundingClientRect()
  const text = excerpt(block.textContent || '')
  return {location:{kind:'md',ratio:clampBookmarkUnit(root.scrollTop / Math.max(1,root.scrollHeight-root.clientHeight)),block_index:items.indexOf(block),text,offset:clampBookmarkUnit(((selected?.getBoundingClientRect().top ?? line) - rect.top) / Math.max(1,rect.height))},excerpt:text}
}

/** Content coordinates, independent of viewport width, zoom and current scroll. */
export function bookmarkTargetResolver(root: HTMLElement) {
  const rootRect = root.getBoundingClientRect()
  const article = last(Array.from(root.querySelectorAll<HTMLElement>('article.markdown-body')))
  const items = article ? blocks(article) : []
  const textIndex = new Map<string,number[]>()
  items.forEach((el,index)=>{const key=excerpt(el.textContent||'');const matches=textIndex.get(key)||[];matches.push(index);textIndex.set(key,matches)})
  const pages = new Map(Array.from(root.querySelectorAll<HTMLElement>('.pdf-page-wrap')).map(page=>[Number(page.dataset.pageNum),page]))
  return (location:BookmarkLocation): {top:number; left:number; approximate:boolean} | null => {
  if(location.kind==='epub'){
    const frame=root.querySelector<HTMLIFrameElement>('.epub-frame')
    if(!frame || frame.dataset.epubChapter!==location.chapter || frame.style.visibility==='hidden')return null
    return {top:location.ratio*Math.max(0,root.scrollHeight-root.clientHeight)+64,left:0,approximate:false}
  }
  if (location.kind === 'pdf') {
    const page = pages.get(location.page)
    if (!page) return null
    const rect = page.getBoundingClientRect()
    return {top:root.scrollTop + rect.top-rootRect.top-root.clientTop+rect.height*location.offset,left:Math.max(0,root.scrollLeft+rect.left-rootRect.left+rect.width*location.x),approximate:false}
  }
  if (!article) return null
  const matches = textIndex.get(normalized(location.text)) || []
  const index = matches.reduce((best,index)=>best<0 || Math.abs(index-location.block_index)<Math.abs(best-location.block_index)?index:best,-1)
  if (index < 0) return {top:location.ratio*Math.max(0,root.scrollHeight-root.clientHeight)+64,left:0,approximate:true}
  const rect = items[index].getBoundingClientRect()
  return {top:root.scrollTop+rect.top-rootRect.top-root.clientTop+rect.height*location.offset,left:0,approximate:false}
  }
}
export function bookmarkTarget(root:HTMLElement,location:BookmarkLocation){return bookmarkTargetResolver(root)(location)}
