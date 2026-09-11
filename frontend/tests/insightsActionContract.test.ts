import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const frontendRoot = new URL('../', import.meta.url)

async function source(path: string) {
  return readFile(new URL(path, frontendRoot), 'utf8')
}

test('memory insights actions pass an explicit force flag instead of the click event', async () => {
  const memory = await source('src/views/Memory.vue')

  // `loadInsights(force = false)` reads its first argument as a boolean. A bare
  // `@click="loadInsights"` hands Vue's reactive MouseEvent to `force`, and the
  // backend jsonschema rejects it: `{_vts, isTrusted} is not of type "boolean"`.
  assert.doesNotMatch(memory, /@click="loadInsights"/)
  assert.match(memory, /@click="loadInsights\(true\)"/)
})
