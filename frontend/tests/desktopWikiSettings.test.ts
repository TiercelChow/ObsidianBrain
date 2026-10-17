import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/WikiSettings.vue', import.meta.url), 'utf8')

test('settings retire data protection without leaving a route, dialog, request or style behind', async () => {
  const settings = await source()
  assert.doesNotMatch(settings, /protection|数据保护|restoreVisible|loadBackups|KnowledgeBackup|wiki-export/)
  assert.match(settings, /const section = ref<'runtime' \| 'providers' \| 'usage' \| 'documents' \| 'skills'>\('runtime'\)/)
  assert.match(settings, /\['runtime','providers','usage','documents','skills'\]\.includes\(requestedSection\)/)
  const api = await readFile(new URL('../src/api/knowledge.ts', import.meta.url), 'utf8')
  assert.doesNotMatch(api, /DatabaseBackup|DatabaseValidationReport|KnowledgeBackup|knowledgeBackupDownloadUrl|bookWikiExportDownloadUrl|knowledge\/backups/)
})

test('desktop Skill reading separates a file sidebar and a bounded body through motion wrappers', async () => {
  const settings = await source()
  assert.match(settings, /<nav v-if="!isMobile" class="skill-file-tabs desktop-skill-files"/)
  assert.match(settings, /<nav v-if="isMobile" class="skill-file-tabs"/)
  assert.match(settings, /<MotionSwap class="skill-body-swap"/)
  assert.match(settings, /\.skill-body-swap\s*\{[^}]*flex:\s*1;[^}]*min-height:\s*0/)
  assert.match(settings, /:deep\(\.skill-file-content\)\s*\{[^}]*overflow:\s*auto/)
  assert.match(settings, /\.motion-modal__panel\.is-wide:has\(> \.skill-detail-modal\)\s*\{[^}]*width:\s*min\(1080px/)
})

test('desktop Skill metadata is secondary without removing mobile metadata controls', async () => {
  const settings = await source()
  assert.match(settings, /<details v-if="activeSkillVersion" class="skill-version-audit" :open="isMobile"/)
  assert.match(settings, /<summary class="skill-audit-toggle">版本与质量信息/)
  assert.match(settings, /class="skill-audit-grid"/)
  assert.match(settings, /\.skill-version-audit\s*\{[^}]*order:\s*2/)
  assert.match(settings, /\.mobile-skill-metadata-toggle[^>]*display:\s*none/)
})
