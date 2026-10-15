import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/WikiSettings.vue', import.meta.url), 'utf8')

test('mobile settings expose six cards while desktop retains its single section navigation', async () => {
  const settings = await source()
  assert.doesNotMatch(settings, /mobile-settings-section/)
  assert.match(settings, /usePhoneViewport\(/)
  assert.match(settings, /:view-key="isMobile \? 'feed' : section"/)
  for (const section of ['runtime', 'providers', 'usage', 'protection', 'documents', 'skills']) {
    assert.match(settings, new RegExp(`v-if="isMobile \\|\\| section === '${section}'"[^>]*class="settings-card"[^>]*id="settings-${section}"`))
  }
  assert.match(settings, /<aside v-if="!isMobile" class="settings-nav knowledge-surface"/)
  assert.match(settings, /\.settings-main\s*\{[^}]*padding:\s*0;[^}]*border:\s*0;[^}]*background:\s*none/s)
})

test('mobile book selectors wrap full names and synchronize visible book-scoped cards', async () => {
  const settings = await source()
  assert.equal((settings.match(/class="settings-base-select knowledge-select/g) || []).length, 3)
  assert.equal((settings.match(/@change="changeSettingsBase"/g) || []).length, 3)
  assert.match(settings, /isMobile\.value \? 'system-select-popper wiki-settings-base-popper' : 'system-select-popper'/)
  assert.match(settings, /\.wiki-settings-base-popper \.el-select-dropdown__item\s*\{[^}]*height:\s*auto;[^}]*white-space:\s*normal;[^}]*overflow-wrap:\s*anywhere/s)
  assert.match(settings, /async function changeSettingsBase\(\)[\s\S]*isMobile\.value[\s\S]*Promise\.all\(\[loadSettings\(true\), loadSkills\(\)\]\)/)
  assert.match(settings, /request !== settingsRequest/)
  assert.match(settings, /request !== skillsRequest/)
})

test('mobile card actions keep icon targets together without full-width form button styling', async () => {
  const settings = await source()
  assert.match(settings, /class="skills-create-actions"/)
  assert.match(settings, /\.skills-create-actions\s*\{[^}]*grid-column:\s*2;[^}]*grid-row:\s*1/)
  assert.match(settings, /\.runtime-actions\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\) auto/)
  assert.match(settings, /<UiAction v-if="isMobile" :icon="Check" label="保存运行配置"/)
  assert.match(settings, /\.skill-card footer \.knowledge-select\s*\{[^}]*width:\s*auto;[^}]*flex:\s*1/)
})

test('mobile Skill content remains fully readable through animation wrappers and shares one scroll owner', async () => {
  const settings = await source()
  assert.match(settings, /<pre v-if="activeSkillFile" class="skill-file-content"/)
  assert.match(settings, /\.skill-detail-content\s*\{[^}]*display:\s*block;[^}]*overflow:\s*visible/)
  assert.match(settings, /:deep\(\.skill-file-content\)\s*\{[^}]*max-height:\s*none;[^}]*white-space:\s*pre-wrap;[^}]*overflow-wrap:\s*anywhere/s)
  assert.match(settings, /\.skill-detail-content :deep\(\.motion-swap__panel\)\s*\{[^}]*max-height:\s*none/)
  assert.match(settings, /\.skill-detail-layout\s*\{[^}]*overflow-y:\s*auto/)
})

test('mobile select height respects the chosen edge and visual viewport', async () => {
  const { settingsSelectMaxHeight } = await import('../src/utils/wikiSettingsLayout.ts')
  assert.equal(settingsSelectMaxHeight({ top: 210, bottom: 360 }, 0, 568, 'bottom-start'), 198)
  assert.equal(settingsSelectMaxHeight({ top: 210, bottom: 360 }, 0, 568, 'top-start'), 200)
  assert.equal(settingsSelectMaxHeight({ top: 170, bottom: 214 }, 100, 350, 'bottom-start'), 226)
  assert.equal(settingsSelectMaxHeight({ top: 50, bottom: 94 }, 0, 844, 'bottom-start'), 320)
  assert.equal(settingsSelectMaxHeight({ top: 0, bottom: 568 }, 0, 568, 'bottom-start'), 0)
})
