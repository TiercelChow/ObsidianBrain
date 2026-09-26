import assert from 'node:assert/strict'
import test from 'node:test'
import { loadKnowledgeCitationPreview } from '../src/utils/knowledgeCitationPreview.ts'

const entry = { id: 'entity-1', knowledge_base_id: 'book-1', entry_type: 'concept', slug: 'term', title: '当前标题', summary: '', status: 'verified', updated_at: '' }

test('run citations resolve the original version even when current entity content differs', async () => {
  let currentReads = 0
  const preview = await loadKnowledgeCitationPreview(entry, 'run-1', 4, {
    async snapshot(runId, sourceIndex) {
      assert.equal(runId, 'run-1')
      assert.equal(sourceIndex, 4)
      return { status: 'success', result: { run_id: runId, citation_index: 5, kind: 'entry', object_id: entry.id, version_id: 'revision-2', entry: { ...entry, title: '原始标题' }, content_md: '原始正文', citations: [], historical: true, read_ranges: [] } }
    },
    async current() { currentReads += 1; throw new Error('不应读取当前版本') },
  })
  assert.equal(preview.content_md, '原始正文')
  assert.equal(preview.entry.title, '原始标题')
  assert.equal(preview.historical, true)
  assert.equal(currentReads, 0)
})

test('missing or inaccessible snapshots never silently fall back to a different version', async () => {
  let currentReads = 0
  await assert.rejects(loadKnowledgeCitationPreview(entry, 'run-legacy', 0, {
    async snapshot() { return { status: 'error', error: { code: 'KNOWLEDGE_VALIDATION', message: '历史运行未记录编号证据快照' } } },
    async current() { currentReads += 1; throw new Error('不应隐式回退') },
  }), /历史运行未记录/)
  assert.equal(currentReads, 0)
})

test('legacy search-only cards explicitly read the current entity without claiming a historical snapshot', async () => {
  const preview = await loadKnowledgeCitationPreview(entry, undefined, 0, {
    async snapshot() { throw new Error('无运行不得请求快照') },
    async current(id) { assert.equal(id, entry.id); return { status: 'success', result: { ...entry, content_md: '当前正文', citations: [], aliases: [], edit_policy: '', revision: 3, claims: [], relations: [], versions: [] } } },
  })
  assert.equal(preview.historical, false)
  assert.equal(preview.content_md, '当前正文')
  assert.equal(preview.version_id, '3')
})
