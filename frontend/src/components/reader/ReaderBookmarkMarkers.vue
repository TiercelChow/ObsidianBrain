<template>
  <div v-if="markers.length" class="reader-bookmark-rail" role="group" aria-label="文档书签" :style="{ '--bookmark-hit-size': `${hitSize}px` }">
    <button
      v-for="group in markers"
      :key="group.bookmark.id"
      type="button"
      class="reader-bookmark-marker"
      :style="{ top: `${group.top}px` }"
      :aria-label="`查看书签：${group.bookmark.title}${group.count > 1 ? `（附近 ${group.count} 条）` : ''}`"
      :title="group.bookmark.title"
      @click="$emit('open', group.bookmark)"
    >
      <span class="reader-bookmark-ribbon" aria-hidden="true">
        <span class="reader-bookmark-tail"></span>
        <small v-if="group.count > 1" class="reader-bookmark-count">{{ group.count }}</small>
      </span>
    </button>
  </div>
</template>
<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {bookmarkTargetResolver,type ReaderBookmark} from '@/utils/readerBookmarks'
const props=defineProps<{pane:HTMLElement|null;file:string;bookmarks:ReaderBookmark[];ready:string|number}>()
defineEmits<{open:[bookmark:ReaderBookmark]}>()
const markers=ref<Array<{bookmark:ReaderBookmark;top:number;count:number}>>([])
const hitSize = 44
let resize:ResizeObserver|null=null, mutation:MutationObserver|null=null, frame:number|null=null
let disposed=false,generation=0
function update() {
  frame=null
  const bookmarks = props.bookmarks.filter(item => item.file_path === props.file)
  if(!props.pane || !bookmarks.length) {markers.value=[];return}
  const targetFor=bookmarkTargetResolver(props.pane)
  const result=bookmarks.map(bookmark=>({bookmark,target:targetFor(bookmark.location)})).filter(item=>item.target).sort((a,b)=>a.target!.top-b.target!.top)
  const groups:typeof markers.value=[]
  for(const {bookmark,target} of result) {
    // Text selection anchors are at the start of the line; PDF anchors are points.
    const center = target!.top + (bookmark.location.kind === 'pdf' ? 0 : 6)
    const top=Math.max(0,Math.min(props.pane.scrollHeight-hitSize,center-hitSize/2))
    const previous=groups[groups.length-1]
    if(previous && top-previous.top<hitSize) previous.count++
    else groups.push({bookmark,top,count:1})
  }
  markers.value=groups
}
function schedule() {if(frame===null) frame=requestAnimationFrame(update)}
watch(()=>[props.pane,props.file,props.bookmarks,props.ready],async()=>{
  const current=++generation
  resize?.disconnect();mutation?.disconnect()
  await nextTick()
  if(disposed || current!==generation)return
  const pane=props.pane
  if(!pane) return
  resize=new ResizeObserver(schedule);resize.observe(pane)
  mutation=new MutationObserver(schedule)
  pane.querySelectorAll('article.markdown-body,.pdf-pages,.epub-frame').forEach(element=>{resize!.observe(element);mutation!.observe(element,{subtree:true,childList:true})})
  schedule()
},{immediate:true,flush:'post'})
onBeforeUnmount(()=>{disposed=true;generation++;resize?.disconnect();mutation?.disconnect();if(frame!==null)cancelAnimationFrame(frame)})
</script>
<style scoped>
/* First in the scrollport, with no vertical sticky inset: each ribbon follows
   its line vertically while the rail stays at the viewport edge on PDF pans. */
.reader-bookmark-rail {
  position: sticky;
  inset-inline-start: 0;
  flex: none;
  inline-size: 100%;
  block-size: 0;
  z-index: 4;
  pointer-events: none;
}
.reader-bookmark-marker {
  position: absolute;
  inset-inline-end: 0;
  inline-size: var(--bookmark-hit-size);
  block-size: var(--bookmark-hit-size);
  padding: 0;
  border: 0;
  border-radius: 8px 0 0 8px;
  background: transparent;
  color: var(--reader-bookmark-ink);
  cursor: pointer;
  pointer-events: auto;
}
.reader-bookmark-ribbon {
  --bookmark-tail-width: 15px;
  position: absolute;
  inset-block: 10px;
  inset-inline-end: 0;
  inline-size: clamp(56px, 12vw, 76px);
  border-radius: 7px 0 0 7px;
  background-image: linear-gradient(to right,
    color-mix(in srgb, currentColor 14%, transparent),
    color-mix(in srgb, currentColor 30%, transparent));
  background-size: calc(100% - var(--bookmark-tail-width)) 100%;
  background-repeat: no-repeat;
  pointer-events: none;
  transform-origin: right center;
  transition: transform var(--motion-fast) var(--ease-out);
}
.reader-bookmark-tail {
  --bookmark-notch: polygon(0 0, 100% 0, 63% 50%, 100% 100%, 0 100%);
  position: absolute;
  inset-block: 0;
  inset-inline-end: 0;
  inline-size: var(--bookmark-tail-width);
  clip-path: var(--bookmark-notch);
  background: var(--reader-bookmark-on-ink);
}
.reader-bookmark-tail::after {
  content: '';
  position: absolute;
  inset: 1px 1px 1px 0;
  clip-path: var(--bookmark-notch);
  background: currentColor;
}
.reader-bookmark-count {
  position: absolute;
  inset-block-start: 50%;
  inset-inline-end: calc(var(--bookmark-tail-width) + 2px);
  transform: translateY(-50%);
  min-inline-size: 15px;
  padding-inline: 3px;
  border-radius: 999px;
  background: var(--reader-bookmark-ink);
  color: var(--reader-bookmark-on-ink);
  font-size: 10px;
  line-height: 16px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.reader-bookmark-marker:focus-visible { outline: 2px solid currentColor; outline-offset: -2px; }
.reader-bookmark-marker:active .reader-bookmark-ribbon { transform: scaleX(.94); }
@media (hover: hover) {
  .reader-bookmark-marker:hover .reader-bookmark-ribbon {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, currentColor 32%, transparent);
  }
}
@media (prefers-contrast: more), (prefers-reduced-transparency: reduce) {
  .reader-bookmark-ribbon { background: var(--bg-base); box-shadow: inset 0 0 0 1px currentColor; }
}
@media (forced-colors: active) {
  .reader-bookmark-marker { --reader-bookmark-ink: Highlight; --reader-bookmark-on-ink: HighlightText; color: Highlight; }
  .reader-bookmark-ribbon { forced-color-adjust: none; background: Canvas; box-shadow: inset 0 0 0 1px Highlight; }
}
@media (prefers-reduced-motion: reduce) {
  .reader-bookmark-ribbon { transition: none; }
  .reader-bookmark-marker:active .reader-bookmark-ribbon { transform: none; }
}
</style>
