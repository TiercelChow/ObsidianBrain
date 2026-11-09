<template>
  <MotionModal :model-value="modelValue" size="wide" aria-label="本书书签" :teleport-to="teleportTo" :dismissible="!saving" @update:model-value="requestClose">
    <section v-glass-lens="'css'" class="bookmarks-panel" data-glass="panel" data-glass-rim>
      <header class="bookmarks-header">
        <button v-if="mode!=='list'" data-icon-only type="button" class="ui-icon-action" aria-label="返回书签列表" :disabled="saving" @click="back"><el-icon><ArrowLeft /></el-icon></button>
        <div class="bookmarks-heading"><h2>{{ mode==='edit' ? (draft?.id?'编辑书签':'添加书签') : mode==='detail'?'书签笔记':'本书书签' }}</h2><p :title="bookName">{{ bookName }}</p></div>
        <button v-if="mode==='list'" data-icon-only type="button" class="ui-icon-action" aria-label="添加当前位置书签" :disabled="!canCreate || loading" @click="create"><el-icon><Plus /></el-icon></button>
        <button data-icon-only type="button" class="ui-icon-action" aria-label="关闭书签" :disabled="saving" @click="requestClose"><el-icon><Close /></el-icon></button>
      </header>
      <p v-if="localError || error" class="bookmarks-error" role="alert">{{ localError || error }} <button v-if="error" type="button" @click="$emit('refresh')">重试</button></p>
      <div v-if="discard" class="bookmarks-confirm" role="alert">
        <span>放弃未保存的修改？</span><button type="button" @click="discard=false">继续编辑</button><button type="button" @click="discardChanges">放弃</button>
      </div>
      <div v-if="confirmDelete" class="bookmarks-confirm" role="alert"><span>删除此书签和笔记？</span><button type="button" :disabled="saving" @click="confirmDelete=false">取消</button><button type="button" :disabled="saving" @click="remove">删除</button></div>
      <div class="bookmarks-body">
        <div v-if="mode==='list'" class="bookmarks-list">
          <div v-if="loading" class="bookmarks-empty" role="status" aria-label="加载书签"><el-icon class="is-loading"><Loading /></el-icon></div>
          <p v-else-if="!items.length" class="bookmarks-empty">暂无书签。在正文点按或选中文字后，用 + 记录位置。</p>
          <div v-for="item in items" :key="item.id" class="bookmark-row" data-glass="subcard">
            <button type="button" class="bookmark-open" :aria-label="`跳转书签：${item.title}`" @click="$emit('jump',item)"><el-icon><CollectionTag /></el-icon><span><strong>{{ item.title }}</strong><small>{{ filename(item.file_path) }} · {{ bookmarkPositionLabel(item.location) }}</small></span></button>
            <button data-icon-only type="button" class="ui-icon-action" :aria-label="`查看笔记：${item.title}`" @click="show(item)"><el-icon><View /></el-icon></button>
          </div>
        </div>
        <form v-else-if="mode==='edit' && draft" class="bookmark-editor" @submit.prevent="save">
          <label>标题<input v-model="draft.title" maxlength="240" required placeholder="书签标题" :disabled="saving" /></label>
          <p class="bookmark-position">{{ filename(draft.file_path) }} · {{ bookmarkPositionLabel(draft.location) }}</p>
          <div class="bookmark-note-label"><label for="bookmark-note-input">笔记</label><button data-icon-only type="button" class="ui-icon-action" :aria-label="preview?'编辑 Markdown':'预览 Markdown'" :aria-pressed="preview" @click="preview=!preview"><el-icon><Edit v-if="preview" /><View v-else /></el-icon></button></div>
          <BookmarkMarkdown v-if="preview" :content="draft.note_md || '暂无笔记。'" />
          <textarea v-else id="bookmark-note-input" v-model="draft.note_md" :disabled="saving" placeholder="支持 Markdown、代码块、表格和数学公式…" rows="9" />
          <footer class="bookmarks-footer"><small>支持 Markdown</small><button type="submit" class="bookmark-save" :disabled="saving || !draft.title.trim()"><el-icon v-if="saving" class="is-loading"><Loading /></el-icon>{{ saving?'保存中':'保存' }}</button></footer>
        </form>
        <div v-else-if="selected" class="bookmark-detail">
          <div class="bookmark-detail-title"><h3>{{ selected.title }}</h3><button data-icon-only class="ui-icon-action" type="button" aria-label="编辑书签" :disabled="saving" @click="edit"><el-icon><Edit /></el-icon></button><button data-icon-only class="ui-icon-action" type="button" aria-label="删除书签" :disabled="saving" @click="confirmDelete=true"><el-icon><Delete /></el-icon></button></div>
          <p class="bookmark-position">{{ filename(selected.file_path) }} · {{ bookmarkPositionLabel(selected.location) }}</p>
          <BookmarkMarkdown :content="selected.note_md || '暂无笔记。'" />
          <footer class="bookmarks-footer"><time>{{ new Date(selected.updated_at).toLocaleString() }}</time><button type="button" class="bookmark-save" @click="$emit('jump',selected)"><el-icon><Position /></el-icon>转到原文</button></footer>
        </div>
      </div>
    </section>
  </MotionModal>
</template>
<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowLeft,Close,Plus,Edit,Delete,View,Loading,CollectionTag,Position } from '@element-plus/icons-vue'
import MotionModal from '@/components/motion/MotionModal.vue'
import BookmarkMarkdown from './BookmarkMarkdown.vue'
import { saveReaderBookmark,deleteReaderBookmark } from '@/api/reader'
import {bookmarkPositionLabel,type BookmarkDraft,type ReaderBookmark} from '@/utils/readerBookmarks'
const props=defineProps<{modelValue:boolean;bookId:string;bookName:string;items:ReaderBookmark[];loading:boolean;error:string;canCreate:boolean;capture:()=>BookmarkDraft|null;initialId?:string;teleportTo:string|HTMLElement}>()
const emit=defineEmits<{'update:modelValue':[value:boolean];saved:[bookmark:ReaderBookmark];deleted:[bookmark:ReaderBookmark];refresh:[];jump:[bookmark:ReaderBookmark]}>()
const mode=ref<'list'|'detail'|'edit'>('list'),selectedId=ref(''),draft=ref<BookmarkDraft|null>(null)
const saving=ref(false),preview=ref(false),localError=ref(''),confirmDelete=ref(false),discard=ref(false)
let original='',closeAfterDiscard=false
const selected=computed(()=>props.items.find(item=>item.id===selectedId.value))
const dirty=computed(()=>mode.value==='edit' && JSON.stringify(draft.value)!==original)
const filename=(path:string)=>path.split(/[\\/]/).pop() || path
watch(()=>props.modelValue,open=>{if(open){mode.value=props.initialId?'detail':'list';selectedId.value=props.initialId||'';localError.value='';discard.value=false;confirmDelete.value=false;draft.value=null}},{immediate:true})
function show(item:ReaderBookmark){selectedId.value=item.id;mode.value='detail';localError.value='';confirmDelete.value=false}
function setDraft(value:BookmarkDraft){draft.value=JSON.parse(JSON.stringify(value));original=JSON.stringify(draft.value);mode.value='edit';preview.value=false;localError.value=''}
function create(){const value=props.capture();if(value)setDraft(value);else localError.value='请先打开可阅读的文档。'}
function edit(){if(!selected.value)return;const {created_at,updated_at,...value}=selected.value;setDraft(value)}
function requestClose(){if(saving.value)return;if(dirty.value){closeAfterDiscard=true;discard.value=true}else emit('update:modelValue',false)}
function back(){if(saving.value)return;if(dirty.value){closeAfterDiscard=false;discard.value=true;return}mode.value='list';localError.value='';confirmDelete.value=false}
function discardChanges(){discard.value=false;if(closeAfterDiscard)emit('update:modelValue',false);else mode.value='list'}
async function save(){
  if(!draft.value || saving.value)return
  if(new TextEncoder().encode(draft.value.note_md).length>128*1024){localError.value='笔记超过 128KiB，请精简后保存。';return}
  saving.value=true;localError.value=''
  const snapshot=JSON.parse(JSON.stringify(draft.value)) as BookmarkDraft
  try{const saved=await saveReaderBookmark(snapshot);emit('saved',saved);selectedId.value=saved.id;mode.value='detail';draft.value=null}
  catch(e){localError.value=(e as Error).message || '保存失败，草稿仍保留。'}
  finally{saving.value=false}
}
async function remove(){
  const item=selected.value;if(!item || saving.value)return
  saving.value=true;localError.value=''
  try{await deleteReaderBookmark(item);emit('deleted',item);confirmDelete.value=false;mode.value='list'}
  catch(e){localError.value=(e as Error).message || '删除失败。'}
  finally{saving.value=false}
}
</script>
<style scoped>
.bookmarks-panel { display:flex; flex-direction:column; width:100%; max-height:min(82dvh,820px); color:var(--text-primary); border:1px solid var(--border-glass); border-radius:26px; overflow:hidden; }
.bookmarks-header { display:flex; align-items:center; gap:8px; padding:18px 22px 12px; flex:none; }
.bookmarks-heading { min-width:0; flex:1; }
.bookmarks-heading h2 { margin:0; font-size:20px; font-weight:750; letter-spacing:-.02em; }
.bookmarks-heading p { margin:4px 0 0; font-size:12px; color:var(--text-muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.bookmarks-body { min-height:120px; overflow:auto; overscroll-behavior:contain; padding:4px 22px 22px; }
.bookmarks-list { display:grid; gap:10px; }
.bookmark-row { display:flex; align-items:center; gap:12px; width:100%; padding:15px; border:1px solid var(--border-faint); border-radius:16px; color:inherit; cursor:pointer; text-align:left; transition:background var(--motion-fast),transform var(--motion-fast); }
.bookmark-open { display:flex; align-items:center; gap:12px; min-width:0; flex:1; min-height:44px; padding:0; border:0; background:transparent; color:inherit; text-align:left; cursor:pointer; }
.bookmark-open>span { min-width:0; flex:1; display:grid; gap:5px; }
.bookmark-row strong { font-size:15px; font-weight:650; overflow-wrap:anywhere; }
.bookmark-row small,.bookmark-position,.bookmarks-footer small,.bookmarks-footer time { font-size:12px; color:var(--text-muted); overflow-wrap:anywhere; }
.bookmark-chevron { color:var(--text-faint); }
.bookmark-row:active { transform:scale(.985); }
.bookmark-editor { display:grid; gap:12px; }
.bookmark-editor label { display:grid; gap:8px; font-size:14px; font-weight:600; }
.bookmark-editor input,.bookmark-editor textarea { width:100%; box-sizing:border-box; min-width:0; border:1px solid var(--border-subtle); border-radius:12px; padding:12px; background:var(--glass-control-fill); color:var(--text-primary); font:inherit; }
.bookmark-editor textarea { resize:vertical; line-height:1.7; min-height:180px; font-family:var(--font-mono); }
.bookmark-position { margin:0 0 14px; }
.bookmark-note-label,.bookmark-detail-title { display:flex; align-items:center; gap:8px; }
.bookmark-note-label label { flex:1; }
.bookmark-detail-title h3 { flex:1; min-width:0; font-size:20px; line-height:1.4; overflow-wrap:anywhere; margin:4px 0 10px; }
.bookmarks-footer { display:flex; align-items:center; justify-content:space-between; gap:12px; padding-top:20px; margin-top:8px; }
.bookmark-save { display:flex; align-items:center; justify-content:center; gap:7px; min-height:44px; padding:0 18px; border:1px solid var(--accent-border); border-radius:14px; background:var(--accent-light); color:var(--accent); font:inherit; cursor:pointer; }
.bookmarks-error { margin:0 22px 12px; padding:10px; color:var(--danger); overflow-wrap:anywhere; font-size:14px; }
.bookmarks-empty { text-align:center; padding:40px 12px; font-size:14px; color:var(--text-muted); }
.bookmarks-confirm { display:flex; align-items:center; flex-wrap:wrap; gap:10px; margin:4px 22px 16px; padding:12px; border-radius:12px; background:var(--accent-light); font-size:14px; }
.bookmarks-confirm span { flex:1 0 100%; }
.bookmarks-confirm button,.bookmarks-error button { background:transparent; border:0; min-height:40px; padding:0 8px; color:var(--accent); font:inherit; cursor:pointer; }
.bookmarks-panel :deep(button:disabled) { opacity:.45; cursor:default; }
@media(max-width:768px){.bookmarks-panel{max-height:82dvh;width:100%;border-radius:24px 24px 0 0}.bookmarks-header{padding:22px 16px 12px}.bookmarks-body{padding:4px 16px 20px}.bookmark-editor input,.bookmark-editor textarea{font-size:16px}.bookmarks-footer{padding-bottom:var(--safe-bottom)}}
@media(prefers-reduced-motion:reduce){.bookmark-row{transition:none}.bookmark-row:active{transform:none}}
</style>
