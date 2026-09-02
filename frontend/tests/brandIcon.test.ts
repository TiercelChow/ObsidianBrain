import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const frontendRoot = new URL('../', import.meta.url)
const projectRoot = new URL('../../', import.meta.url)

async function read(url: URL) {
  return readFile(url, 'utf8')
}

test('product app and website ship the same canonical brand icon', async () => {
  const [appIcon, websiteIcon] = await Promise.all([
    read(new URL('public/favicon.svg', frontendRoot)),
    read(new URL('website/public/favicon.svg', projectRoot)),
  ])

  assert.equal(appIcon.trim(), websiteIcon.trim())
})

test('product favicon and sidebar render the canonical icon asset', async () => {
  const [indexHtml, sidebar] = await Promise.all([
    read(new URL('index.html', frontendRoot)),
    read(new URL('src/components/Sidebar.vue', frontendRoot)),
  ])

  assert.match(indexHtml, /<link rel="icon" type="image\/svg\+xml" href="\/favicon\.svg" \/>/)
  assert.match(sidebar, /<img[^>]+class="logo-mark"[^>]+src="\/favicon\.svg"/)
})
