<template>
  <div class="reader-bookmark-rail" :style="{left:`${horizontalOffset+1}px`}" aria-label="文档书签">
    <button v-for="group in markers" :key="group.bookmark.id" type="button" class="reader-bookmark-marker" :style="{top:`${group.top}px`}" :aria-label="`查看书签：${group.bookmark.title}${group.count>1?`（附近 ${group.count} 条）`:''}`" :title="group.bookmark.title" @click="$emit('open',group.bookmark)">
      <el-icon><CollectionTag /></el-icon><small v-if="group.count>1">{{ group.count }}</small>
    </button>
  </div>
</template>
<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { CollectionTag } from '@element-plus/icons-vue'
import {bookmarkTargetResolver,type ReaderBookmark} from '@/utils/readerBookmarks'
const props=defineProps<{pane:HTMLElement|null;file:string;bookmarks:ReaderBookmark[];ready:string|number}>()
defineEmits<{open:[bookmark:ReaderBookmark]}>()
const markers=ref<Array<{bookmark:ReaderBookmark;top:number;count:number}>>([])
const horizontalOffset=ref(0)
let scrollPane:HTMLElement|null=null
function syncHorizontal(){horizontalOffset.value=props.pane?.scrollLeft || 0}
let resize:ResizeObserver|null=null, mutation:MutationObserver|null=null, frame:number|null=null
let disposed=false,generation=0
function update() {
  frame=null
  if(!props.pane) {markers.value=[];return}
  const targetFor=bookmarkTargetResolver(props.pane)
  const result=props.bookmarks.filter(item=>item.file_path===props.file).map(bookmark=>({bookmark,target:targetFor(bookmark.location)})).filter(item=>item.target).sort((a,b)=>a.target!.top-b.target!.top)
  const groups:typeof markers.value=[]
  for(const {bookmark,target} of result) {
    const top=Math.max(0,Math.min(props.pane.scrollHeight-32,target!.top-12))
    const previous=groups[groups.length-1]
    if(previous && top-previous.top<30) previous.count++
    else groups.push({bookmark,top,count:1})
  }
  markers.value=groups
}
function schedule() {if(frame===null) frame=requestAnimationFrame(update)}
watch(()=>[props.pane,props.file,props.bookmarks,props.ready],async()=>{
  const current=++generation
  resize?.disconnect();mutation?.disconnect()
  scrollPane?.removeEventListener('scroll',syncHorizontal)
  await nextTick()
  if(disposed || current!==generation)return
  const pane=props.pane
  if(!pane) return
  scrollPane=pane;pane.addEventListener('scroll',syncHorizontal,{passive:true});syncHorizontal()
  resize=new ResizeObserver(schedule);resize.observe(pane)
  mutation=new MutationObserver(schedule)
  pane.querySelectorAll('article.markdown-body,.pdf-pages,.epub-frame').forEach(element=>{resize!.observe(element);mutation!.observe(element,{subtree:true,childList:true})})
  schedule()
},{immediate:true,flush:'post'})
onBeforeUnmount(()=>{disposed=true;generation++;resize?.disconnect();mutation?.disconnect();scrollPane?.removeEventListener('scroll',syncHorizontal);if(frame!==null)cancelAnimationFrame(frame)})
</script>
<style scoped>
.reader-bookmark-rail { position:absolute; top:0; left:1px; width:24px; height:0; z-index:4; }
.reader-bookmark-marker { position:absolute; display:grid; place-items:center; width:24px; height:32px; padding:0; border:0; border-radius:0 8px 8px 0; background:var(--accent-light); color:var(--accent); cursor:pointer; transition:background var(--motion-fast),transform var(--motion-fast); }
.reader-bookmark-marker .el-icon { font-size:19px; }
.reader-bookmark-marker small { position:absolute; right:0; bottom:0; font-size:9px; background:var(--bg-base); border-radius:5px; padding:0 2px; }
.reader-bookmark-marker:active { transform:scale(.92); }
@media (pointer:coarse) {.reader-bookmark-marker::before{content:'';position:absolute;inset:-6px -10px;}}
@media(prefers-reduced-motion:reduce){.reader-bookmark-marker{transition:none}.reader-bookmark-marker:active{transform:none}}
</style>
