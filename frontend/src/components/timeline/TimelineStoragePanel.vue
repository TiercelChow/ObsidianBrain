<template>
  <section class="timeline-storage-panel">
    <h3>本地图片与缓存</h3>
    <p class="storage-note">小记保存在数据库，原图永久保存。只有可重建的缩略图会按最近使用顺序淘汰。</p>
    <div v-if="storage" class="storage-details">
      <label>原图目录</label><code>{{ storage.directory }}/images</code>
      <div class="storage-metrics"><span>原图 {{ size(storage.originals_bytes) }}</span><span>缓存 {{ size(storage.cache_bytes) }} / {{ size(storage.cache_limit_bytes) }}</span></div>
      <p v-if="storage.pending_cleanup" role="status" class="storage-warning">{{ storage.pending_cleanup }} 张图片等待清理重试，请检查磁盘权限；系统会自动重试。</p>
      <p v-if="storage.missing_images.length" role="status" class="storage-warning">{{ storage.missing_images.length }} 张旧图片尚未迁入，请选择原库根目录重新导入。小记记录已保留。</p>
    </div>
    <p v-else-if="loading" role="status">读取存储状态…</p>
    <div class="storage-capacity">
      <label for="timeline-cache-limit">缩略图缓存上限（MB）</label>
      <el-input-number id="timeline-cache-limit" v-model="limitMb" :min="0" :max="4096" :step="64" :disabled="busy" />
    </div>
    <p class="storage-note">设为 0 不缓存；缩小容量会立即淘汰旧缓存，原图不受影响。</p>
    <div class="storage-actions">
      <el-button :disabled="busy" @click="refresh">刷新</el-button>
      <el-button :disabled="busy" @click="clear">清空缓存</el-button>
      <el-button type="primary" :loading="saving" :disabled="busy && !saving" @click="saveLimit">保存上限</el-button>
    </div>
    <details class="storage-import" :open="!!storage?.missing_images.length">
      <summary>迁移旧图片</summary>
      <p class="storage-note">选择含 Timeline 文件夹的旧库根目录，只复制已被小记引用的图片，不修改或删除旧库文件。</p>
      <el-input v-model="legacyDirectory" placeholder="旧库根目录的绝对路径" :disabled="busy" aria-label="旧库根目录" />
      <el-button :loading="importing" :disabled="!legacyDirectory.trim() || (busy && !importing)" @click="importLegacy">{{ importing ? '正在复制旧图片…' : '复制旧图片' }}</el-button>
    </details>
    <p class="storage-note">完整备份请保留数据库及上方原图目录；默认是 timeline/images。仅下载 SQLite 快照不包含原图，缓存无需备份。</p>
    <p v-if="error" class="storage-warning" role="alert">{{ error }}</p>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { getTimelineStorage, importTimelineImages, clearTimelineImageCache, saveConfig } from '@/api'
import { memoToolResult, type TimelineStorage } from '@/utils/timelineMemo'

const emit = defineEmits<{ migrated: [] }>()
const storage = ref<TimelineStorage | null>(null)
const limitMb = ref(256)
const legacyDirectory = ref('')
const loading = ref(false), saving = ref(false), importing = ref(false), clearing = ref(false)
const error = ref('')
const busy = computed(() => loading.value || saving.value || importing.value || clearing.value)
function size(bytes: number) { return `${(bytes / 1024 / 1024).toFixed(1)} MB` }
function message(e: unknown) { return e instanceof Error ? e.message : '操作失败，请稍后重试' }
async function refresh() {
  loading.value = true
  error.value = ''
  try {
    storage.value = await getTimelineStorage()
    limitMb.value = storage.value.cache_limit_bytes / 1024 / 1024
    if (!legacyDirectory.value) legacyDirectory.value = storage.value.legacy_directory
  } catch (e) { error.value = message(e) }
  finally { loading.value = false }
}
async function saveLimit() {
  saving.value = true
  try {
    memoToolResult(await saveConfig({ timeline: { cache_limit_mb: limitMb.value } }))
    ElMessage.success('缓存上限已生效')
    await refresh()
  } catch (e) { error.value = message(e) }
  finally { saving.value = false }
}
async function clear() {
  clearing.value = true
  try { await clearTimelineImageCache(); await refresh(); ElMessage.success('缓存已清空，原图完整保留') }
  catch (e) { error.value = message(e) }
  finally { clearing.value = false }
}
async function importLegacy() {
  importing.value = true
  error.value = ''
  try {
    const report = await importTimelineImages(legacyDirectory.value.trim())
    if (report.missing.length) ElMessage.warning(`已复制 ${report.copied} 张，仍有 ${report.missing.length} 张未找到或无法读取`)
    else ElMessage.success(`已复制 ${report.copied} 张旧图片，原库未改动`)
    emit('migrated')
    await refresh()
  } catch (e) { error.value = message(e) }
  finally { importing.value = false }
}
onMounted(refresh)
</script>

<style scoped>
.timeline-storage-panel { min-width: 0; display: flex; flex-direction: column; gap: 14px; }
h3 { margin: 0; font-size: 17px; font-weight: 650; color: var(--text-primary); }
.storage-note { margin: 0; color: var(--text-muted); font-size: 13px; line-height: 1.6; }
.storage-details { display: grid; gap: 8px; min-width: 0; }
.storage-details label, .storage-capacity label { font-size: 13px; color: var(--text-secondary); }
code { overflow-wrap: anywhere; font-size: 12px; color: var(--text-secondary); }
.storage-metrics { display: flex; flex-wrap: wrap; gap: 12px; font-size: 13px; color: var(--text-secondary); }
.storage-capacity { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; }
.storage-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.storage-actions :deep(.el-button) { margin: 0; }
.storage-warning { margin: 0; color: var(--glass-warning-label); font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
.storage-import { display: grid; min-width: 0; }
.storage-import summary { cursor: pointer; padding: 10px 0; color: var(--text-primary); font-weight: 600; }
.storage-import > :not(summary) { margin-bottom: 12px; }
@media (max-width: 768px) { .storage-actions :deep(.el-button) { flex: 1 1 auto; min-height: 44px; } }
</style>
