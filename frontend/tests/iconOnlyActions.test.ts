import assert from 'node:assert/strict'
import { readFile, readdir } from 'node:fs/promises'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { iconActions } from './helpers/iconActions.mjs'

const root = new URL('../', import.meta.url)
const read = (path: string) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')

async function vuePaths(directory: URL): Promise<URL[]> {
  const children = await readdir(directory, { withFileTypes: true })
  return (await Promise.all(children.map(child => child.isDirectory()
    ? vuePaths(new URL(`${child.name}/`, directory))
    : child.name.endsWith('.vue') ? [new URL(child.name, directory)] : []))).flat()
}

test('icon-only audit recognizes symbols and wrapped icons but leaves labeled and state controls alone', () => {
  const actions = iconActions(`<template><button><el-icon><Plus /></el-icon></button><button>＋</button><button><span class="stop-square" /></button><button><el-icon><Plus /></el-icon>新建</button><button>{{ title }}</button><button class="status-orb"><span>✓</span></button><el-button :icon="Plus" /></template>`)
  assert.equal(actions.length, 4)
  const text = '<template><button>＋</button></template>'
  assert.equal(text.slice(iconActions(text)[0].offset, iconActions(text)[0].offset + 7), '<button')
})

test('every native icon-only action is explicitly marked across the app', async () => {
  const paths = await vuePaths(new URL('src/', root))
  const missing: string[] = []
  let count = 0
  for (const path of paths) {
    for (const action of iconActions(await readFile(path, 'utf8'))) {
      count += 1
      if (!action.marked) missing.push(`${fileURLToPath(path)}:${action.line} ${action.tag} ${action.classes}`)
    }
  }
  assert.ok(count > 70, 'audit should cover the entire app, not just shared buttons')
  assert.deepEqual(missing, [])
})

test('icon paint is opt-in and retains feedback without flattening text buttons or glass containers', async () => {
  const css = await read('src/styles/materials.css')
  const section = css.split('/* Borderless icon actions:')[1]?.split('/* A sidebar')[0] || ''
  assert.match(section, /\[data-icon-only\]/)
  assert.match(section, /\.ui-icon-action/)
  assert.match(section, /\.glass-icon-btn/)
  assert.match(section, /\.el-dialog__headerbtn/)
  assert.match(section, /border:\s*0\s*!important/)
  assert.match(section, /background:\s*transparent\s*!important/)
  assert.match(section, /box-shadow:\s*none\s*!important/)
  assert.match(section, /:focus-visible/)
  assert.match(section, /:disabled/)
  assert.match(section, /hover:\s*hover/)
  assert.doesNotMatch(section, /button:has|\.dock-glass\s*\{|\.glass-surface\s*\{|\.status-orb\s*\{/)
})
