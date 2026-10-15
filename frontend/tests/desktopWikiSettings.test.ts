import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/WikiSettings.vue', import.meta.url), 'utf8')

test('desktop exports bound the select to its grid track instead of its compact fixed width', async () => {
  const settings = await source()
  assert.match(settings, /\.wiki-export-actions \.settings-base-select\s*\{[^}]*min-width:\s*0;[^}]*width:\s*100%;[^}]*max-width:\s*100%/)
  assert.match(settings, /@media \(min-width: 769px\)[\s\S]*\.wiki-export-card\s*\{[^}]*display:\s*grid/)
  assert.match(settings, /\.wiki-export-actions\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\) auto auto/)
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
