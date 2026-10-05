import assert from 'node:assert/strict'
import test from 'node:test'
import { readingPosition, wikiItemRoute, wikiStatus, mergeHomeOverview, shouldPollHome, formatUptime, recentReadLabel } from '../src/utils/homeWorkbench.ts'
import type { HomeBook, HomeWikiItem, HomeOverview } from '../src/api/home'

test('reading preserves chapter vs PDF page semantics, never fabricates book percent', () => {
  const book = { kind:'folder', position:.9, last_file:'part/chapter.md', page_count:null } as HomeBook
  assert.equal(readingPosition(book), 'chapter')
  assert.equal(readingPosition({ ...book, kind:'pdf', position:3, page_count:80 }), '第 3 / 80 页')
  assert.equal(readingPosition({ ...book, kind:'pdf', position:90, page_count:80 }), '第 80 / 80 页')
  assert.equal(readingPosition({ ...book, last_file:null }), '继续上次阅读')
})
test('recent reading uses local calendar day, not elapsed 24 hours', () => {
  const now = new Date(2026,9,5,0,10)
  assert.equal(recentReadLabel(new Date(2026,9,4,23,30).getTime(),now),'昨天')
  assert.equal(recentReadLabel(new Date(2026,9,5,0,1).getTime(),now),'今天')
})
test('Wiki links only inspect, never run or cancel work and distinguish partial deliverables', () => {
  const item = { id:'t',base_id:'b',kind:'research',status:'completed',artifact_state:'failed' } as HomeWikiItem
  assert.deepEqual(wikiItemRoute(item), { path:'/knowledge/tasks',query:{base:'b',task:'t'} })
  assert.equal(wikiStatus(item), '报告已完成 · PPT 失败')
  assert.deepEqual(wikiItemRoute({ ...item, kind:'review' }), {path:'/knowledge/wiki',query:{base:'b',review:'t'}})
  assert.equal(wikiStatus({...item, artifact_state:'ready'}), '报告与 PPT 已就绪')
})
test('section failures retain last success explicitly and polling requires visible running Wiki', () => {
  const old = { tasks:{data:{active_count:7},error:null}, wiki:{data:{running_count:2},error:null} } as HomeOverview
  const fresh = { tasks:{data:null,error:'不可用'},reading:{data:[],error:null},memos:{data:null,error:null},wiki:{data:{running_count:0},error:null},storage:{data:null,error:null} } as HomeOverview
  const merged = mergeHomeOverview(old, fresh)
  assert.equal(merged.tasks.data?.active_count, 7)
  assert.equal(merged.tasks.error, '不可用')
  assert.equal(shouldPollHome(old, true), true)
  assert.equal(shouldPollHome(old, false), false)
  assert.equal(shouldPollHome(merged, true), false)
  assert.equal(formatUptime(86461), '1 天 0 小时')
})
