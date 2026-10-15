import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const source = () => readFile(new URL('../src/views/knowledge/KnowledgeBases.vue', import.meta.url), 'utf8')

test('only mobile initialized knowledge cards use a native full-card Wiki link', async () => {
  const page = await source()
  assert.match(page, /const \{ isMobile \} = usePhoneViewport\(\)/)
  assert.match(page, /<RouterLink\s+v-if="isMobile && card\.knowledge_base"[\s\S]*?class="book-wiki-link"[\s\S]*?path: '\/knowledge\/wiki', query: \{ base: card\.knowledge_base\.id \}/)
  assert.match(page, /\.book-wiki-link::after\s*\{[^}]*position: absolute;[^}]*inset: 0;[^}]*z-index: 1/)
  assert.match(page, /\.book-wiki-card\s*\{[^}]*position: relative/)
  assert.doesNotMatch(page, /<article[^>]*role="button"|<article[^>]*@click=/)
})

test('only mobile card more actions occupy the upper right and sit above the card link', async () => {
  const page = await source()
  const card = page.match(/<article[\s\S]*?<\/article>/)?.[0] ?? ''
  const actions = card.match(/<footer v-if="!isMobile" class="book-actions"[\s\S]*?<\/footer>/)?.[0] ?? ''
  assert.match(card, /<button v-if="isMobile"[^>]*class="book-more"[\s\S]*?aria-haspopup="dialog"[\s\S]*?@click\.stop=/)
  assert.doesNotMatch(actions, /book-more/)
  const mobileStyles = page.slice(page.indexOf('@media (max-width: 768px)'))
  assert.match(mobileStyles, /\.book-more\s*\{[^}]*position: absolute;[^}]*top: 8px;[^}]*right: 8px;[^}]*z-index: 2/)
  assert.match(mobileStyles, /\.book-card-title-row\s*\{[^}]*padding-inline-end: 44px/)
  assert.match(mobileStyles, /\.compile-controls > \*\s*\{[^}]*position: relative;[^}]*z-index: 2/)
})

test('mobile initialization is available only in the more sheet and linked cards have accessible focus', async () => {
  const page = await source()
  const sheet = page.match(/<MotionModal v-model="actionSheetVisible"[\s\S]*?<\/MotionModal>/)?.[0] ?? ''
  assert.match(sheet, /<el-button v-else[^>]*:loading="busyBookId === actionCard\.book\.id"[^>]*@click="actionSheetVisible = false; initialize\(actionCard\)">建立知识库<\/el-button>/)
  assert.match(page, /\.book-wiki-card:has\(\.book-wiki-link:focus-visible\)\s*\{[^}]*outline: 2px solid var\(--accent\)/)
})

test('desktop card footer retains open, initialization and maintenance actions', async () => {
  const page = await source()
  const actions = page.match(/<footer v-if="!isMobile" class="book-actions"[\s\S]*?<\/footer>/)?.[0] ?? ''
  assert.match(actions, /v-if="!card\.knowledge_base"[\s\S]*?@click="initialize\(card\)"/)
  assert.match(actions, /<el-button type="primary" @click="openWiki\(card\.knowledge_base\.id\)">打开 Wiki<\/el-button>/)
  assert.match(actions, /label="管理知识库"/)
  const desktopStyles = page.slice(page.indexOf('<style scoped>'), page.indexOf('@media (max-width: 768px)'))
  assert.doesNotMatch(desktopStyles, /padding-inline-end: 44px|\.book-wiki-link::after|\.book-more\s*\{/)
})

test('desktop actions span the whole card outside the content and align at the bottom', async () => {
  const page = await source()
  assert.match(page, /class="book-error">[\s\S]*?<\/p>\s*<\/div>\s*<footer v-if="!isMobile" class="book-actions">/)
  assert.match(page, /\.book-actions\s*\{[^}]*grid-column: 1 \/ -1;[^}]*align-self: end;[^}]*justify-content: space-between/)
  assert.match(page, /@media \(min-width: 769px\)\s*\{\s*\.book-wiki-card\s*\{[^}]*grid-template-rows: minmax\(0, 1fr\) auto/)
})

test('desktop footer separates borderless utility controls from primary navigation', async () => {
  const page = await source()
  const utilities = page.match(/<div class="book-utility-actions"[\s\S]*?<\/div>/)?.[0] ?? ''
  for (const label of ['阅读原书', '同步知识库来源', '查看编译报告', '管理知识库']) {
    assert.ok(utilities.includes(`label="${label}"`), label)
  }
  assert.match(utilities, /role="group"[^>]*aria-label=/)
  assert.doesNotMatch(utilities, /<el-button|appearance="glass"/)
  const primary = page.match(/<div class="book-primary-actions">[\s\S]*?<\/div>/)?.[0] ?? ''
  assert.match(primary, /@click="compileWiki\(card\)"[\s\S]*?@click="openWiki\(card\.knowledge_base\.id\)"/)
  assert.match(primary, /@click="initialize\(card\)"/)
  assert.match(page, /\.book-utility-actions \.ui-action\s*\{[^}]*width: 36px;[^}]*height: 36px/)
  assert.match(page, /\.book-primary-actions\s*\{[^}]*margin-inline-start: auto/)
})
